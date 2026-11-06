//! MQTT in modules (`specs/wasm-io/mqtt.md`): the master owns the client (`CRAB_MQTT` in
//! `master.ino`); a module sets it up once, queues publications and reads the last message of
//! each filter through ops 9..16 of the `net` import.

use super::text::{Buf, A};
use super::*;

/// Longest topic, message and string of a module.
const MSG_CAP: u32 = 128;

impl Emitter<'_, '_> {
    /// `net(op, a, b, c, d)`; leaves the result on the stack.
    pub(super) fn net_call4(&mut self, op: i32, args: [A; 4]) {
        self.c_i(op);
        for a in args {
            self.push_arg(a);
        }
        self.hw_call(HW_NET);
    }

    fn mqtt_num(&mut self, code: i32, value: i32) {
        self.net_call4(9, [A::C(code), A::C(value), A::C(0), A::C(0)]);
        self.op(I::Drop);
    }

    fn mqtt_text(&mut self, inst: &Instance, code: i32, name: &str) -> Result<(), WasmError> {
        if inst.param_vars.contains_key(name) {
            return Err(self.unsupported(
                inst,
                &format!("параметр «{name}» из переменной пока не поддержан"),
            ));
        }
        let text = self.text_param(inst, name);
        let s = self.const_str(text.as_bytes());
        self.net_call4(
            10,
            [
                A::C(code),
                A::C(s.ptr as i32),
                A::C(text.len() as i32),
                A::C(0),
            ],
        );
        self.op(I::Drop);
        Ok(())
    }

    pub(super) fn mqtt_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        match inst.block.kind.as_str() {
            "mqtt" => self.mqtt_client(inst)?,
            "mqtt_publish" => self.mqtt_publish(inst)?,
            "mqtt_subscribe" => self.mqtt_subscribe(inst)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn mqtt_client(&mut self, inst: &Instance) -> Result<(), WasmError> {
        let eth = self.text_param(inst, "net") == "ethernet";
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        for (code, name) in [
            (1, "broker"),
            (2, "client_id"),
            (3, "user"),
            (4, "password"),
        ] {
            self.mqtt_text(inst, code, name)?;
        }
        let port = self.named_param(inst, "port")?;
        let keepalive = self.named_param(inst, "keepalive")?;
        let subs = self.named_param(inst, "max_subscriptions")?;
        self.mqtt_num(1, port);
        self.mqtt_num(2, keepalive);
        self.mqtt_num(3, eth as i32);
        self.mqtt_num(4, subs);
        self.mqtt_num(63, 0);
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);
        for (name, field) in [("connected", 0), ("error", 1)] {
            if inst.outputs.contains_key(name) {
                self.net_call4(11, [A::C(field), A::C(0), A::C(0), A::C(0)]);
                self.set_named(inst, name, Ty::I);
            }
        }
        Ok(())
    }

    /// The topic: a constant, or the string input of the parameter.
    fn mqtt_topic(&mut self, inst: &Instance) -> Result<(A, A), WasmError> {
        if inst.inputs.contains_key("topic") {
            let s = self.str_in(inst, "topic")?;
            Ok((s.ptr(), s.len()))
        } else {
            let text = self.text_param(inst, "topic");
            let s = self.const_str(text.as_bytes());
            Ok((s.ptr(), s.len()))
        }
    }

    fn mqtt_publish(&mut self, inst: &Instance) -> Result<(), WasmError> {
        let value_ty = inst.inputs.get("value").map(|i| i.ty);
        let is_str = value_ty == Some(SignalType::String);
        let is_bool = value_ty == Some(SignalType::Bool);
        let trigger = self.named_param(inst, "trigger")?;
        let retain = self.named_param(inst, "retain")?;
        let ty = self.input_ty(inst, 1);
        let (topic_ptr, topic_len) = self.mqtt_topic(inst)?;

        let (pending, started, stamp) = (self.state_i(), self.state_i(), self.state_i());
        let last = self.global(ty, &ParamValue::Int(0));
        let last_str = if is_str && trigger == 1 {
            Some(self.str_last(inst)?)
        } else {
            None
        };
        let en = self.tmp_i();
        self.flag(inst, 0);
        self.op(I::LocalSet(en));
        match trigger {
            0 => {
                let prev = self.state_i();
                self.op(I::LocalGet(en));
                self.op(I::GlobalGet(prev));
                self.op(I::I32Eqz);
                self.op(I::I32And);
                self.mqtt_set_pending(pending);
                self.op(I::LocalGet(en));
                self.op(I::GlobalSet(prev));
            }
            1 => {
                let deadband = self.named_param_f(inst, "deadband");
                self.op(I::GlobalGet(started));
                self.op(I::I32Eqz);
                if let Some(keep) = last_str {
                    self.str_same(inst, keep)?;
                    self.op(I::I32Eqz);
                } else {
                    self.input_as(inst, 1, ty);
                    self.op(I::GlobalGet(last));
                    if ty == Ty::F && deadband > 0.0 {
                        self.op(I::F32Sub);
                        self.op(I::F32Abs);
                        self.c_f(deadband as f32);
                        self.op(I::F32Ge);
                    } else if ty == Ty::F {
                        self.op(I::F32Ne);
                    } else {
                        self.op(I::I32Ne);
                    }
                }
                self.op(I::I32Or);
                self.op(I::LocalGet(en));
                self.op(I::I32And);
                self.mqtt_set_pending(pending);
            }
            _ => {
                let period = inst
                    .lib
                    .def
                    .params
                    .iter()
                    .position(|p| p.name == "period")
                    .unwrap_or(0);
                self.since(stamp);
                self.param_ms(inst, period);
                self.op(I::I32GeU);
                self.op(I::LocalGet(en));
                self.op(I::I32And);
                self.mqtt_set_pending(pending);
            }
        }

        let sent = self.tmp_i();
        self.c_i(0);
        self.op(I::LocalSet(sent));
        self.op(I::GlobalGet(pending));
        self.op(I::If(BlockType::Empty));
        // The text of the value.
        let cap = if is_str { self.value_cap(inst)? } else { 48 };
        let compose = Buf {
            ptr: self.alloc(cap),
            cap,
            len: 0,
        };
        let len = self.tmp_i();
        if is_str {
            let v = self.str_in(inst, "value")?;
            self.append(compose, v, None);
        } else if ty == Ty::F {
            let x = self.tmp_f();
            self.input_as(inst, 1, Ty::F);
            self.op(I::LocalSet(x));
            let decimals = self.named_param(inst, "decimals")?.clamp(0, 9);
            self.text_call(
                2,
                [
                    compose.ptr(),
                    compose.cap(),
                    A::F(x),
                    A::C(decimals),
                    A::C(0),
                    A::C(0),
                ],
            );
        } else {
            let n = self.tmp_i();
            self.input_as(inst, 1, Ty::I);
            if is_bool {
                self.c_i(0);
                self.op(I::I32Ne);
            }
            self.op(I::LocalSet(n));
            self.text_call(
                3,
                [
                    compose.ptr(),
                    compose.cap(),
                    A::L(n),
                    A::C(0),
                    A::C(0),
                    A::C(0),
                ],
            );
        }
        self.op(I::LocalSet(len));
        self.net_call4(
            12 + (retain != 0) as i32,
            [topic_ptr, topic_len, compose.ptr(), A::L(len)],
        );
        self.op(I::LocalSet(sent));
        self.op(I::LocalGet(sent));
        self.op(I::If(BlockType::Empty));
        self.c_i(0);
        self.op(I::GlobalSet(pending));
        match trigger {
            1 => {
                if let Some(keep) = last_str {
                    self.str_keep(inst, keep)?;
                } else {
                    self.input_as(inst, 1, ty);
                    self.op(I::GlobalSet(last));
                }
                self.c_i(1);
                self.op(I::GlobalSet(started));
            }
            2 => {
                // The period counts from the last publication.
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalSet(stamp));
            }
            _ => {}
        }
        self.op(I::End);
        self.op(I::End);
        if inst.outputs.contains_key("sent") {
            self.op(I::LocalGet(sent));
            self.set_named(inst, "sent", Ty::I);
        }
        Ok(())
    }

    /// `pending |= <flag on the stack>`.
    fn mqtt_set_pending(&mut self, pending: u32) {
        self.op(I::GlobalGet(pending));
        self.op(I::I32Or);
        self.op(I::GlobalSet(pending));
    }

    fn mqtt_subscribe(&mut self, inst: &Instance) -> Result<(), WasmError> {
        let kind = self.text_param(inst, "kind");
        let slot = self.global(Ty::I, &ParamValue::Int(-1));
        let seen = self.state_i();
        let msg = Buf {
            ptr: self.alloc(MSG_CAP),
            cap: MSG_CAP,
            len: 0,
        };
        let (topic_ptr, topic_len) = self.mqtt_topic(inst)?;
        if inst.inputs.contains_key("topic") {
            // The topic comes from an input: subscribe again when it changes.
            let kept = Buf {
                ptr: self.alloc(MSG_CAP),
                cap: MSG_CAP,
                len: self.state_i(),
            };
            let s = self.str_in(inst, "topic")?;
            self.text_call(
                5,
                [
                    A::C(0),
                    A::C(0),
                    s.ptr(),
                    s.len(),
                    kept.ptr(),
                    A::G(kept.len),
                ],
            );
            self.op(I::I32Eqz);
            self.op(I::If(BlockType::Empty));
            self.append(kept, s, None);
            self.op(I::GlobalSet(kept.len));
            self.c_i(-1);
            self.op(I::GlobalSet(slot));
            self.op(I::End);
        }
        // A new subscription skips what the filter received before.
        self.op(I::GlobalGet(slot));
        self.c_i(0);
        self.op(I::I32LtS);
        self.op(I::If(BlockType::Empty));
        self.net_call4(14, [topic_ptr, topic_len, A::C(0), A::C(0)]);
        self.op(I::GlobalSet(slot));
        self.op(I::GlobalGet(slot));
        self.c_i(0);
        self.op(I::I32GeS);
        self.op(I::If(BlockType::Empty));
        self.net_call4(16, [A::G(slot), A::C(0), A::C(0), A::C(0)]);
        self.op(I::GlobalSet(seen));
        self.op(I::End);
        self.op(I::End);

        let received = self.tmp_i();
        self.c_i(0);
        self.op(I::LocalSet(received));
        self.op(I::GlobalGet(slot));
        self.c_i(0);
        self.op(I::I32GeS);
        self.op(I::If(BlockType::Empty));
        let count = self.tmp_i();
        self.net_call4(16, [A::G(slot), A::C(0), A::C(0), A::C(0)]);
        self.op(I::LocalSet(count));
        self.op(I::LocalGet(count));
        self.op(I::GlobalGet(seen));
        self.op(I::I32Ne);
        self.op(I::If(BlockType::Empty));
        self.op(I::LocalGet(count));
        self.op(I::GlobalSet(seen));
        self.c_i(1);
        self.op(I::LocalSet(received));
        match kind.as_str() {
            "string" => {
                if let Some(out) = self.out_buf(inst, "out") {
                    self.net_call4(15, [A::G(slot), out.ptr(), out.cap(), A::C(0)]);
                    self.op(I::GlobalSet(out.len));
                }
            }
            _ => {
                let len = self.tmp_i();
                self.net_call4(15, [A::G(slot), msg.ptr(), msg.cap(), A::C(0)]);
                self.op(I::LocalSet(len));
                if kind == "bool" {
                    self.mqtt_bool(inst, msg, len);
                } else {
                    let (name, ty) = if kind == "float" {
                        ("out_f", Ty::F)
                    } else {
                        ("out_i", Ty::I)
                    };
                    // Text that is not a number gives 0, like toInt() / toFloat().
                    self.text_call(
                        6,
                        [A::C(0), A::C(4), msg.ptr(), A::L(len), A::C(0), A::C(0)],
                    );
                    self.op(I::If(BlockType::Result(ty.val())));
                    self.c_i(0);
                    self.op(I::F32Load(Self::mem_arg(0, 2)));
                    if ty == Ty::I {
                        self.op(I::I32TruncSatF32S);
                    }
                    self.op(I::Else);
                    self.c(ty, 0.0);
                    self.op(I::End);
                    self.set_named(inst, name, ty);
                }
            }
        }
        self.op(I::End);
        self.op(I::End);
        if inst.outputs.contains_key("received") {
            self.op(I::LocalGet(received));
            self.set_named(inst, "received", Ty::I);
        }
        Ok(())
    }

    /// `out_b`: the message is 1, true or on (any case).
    fn mqtt_bool(&mut self, inst: &Instance, msg: Buf, len: u32) {
        let mut first = true;
        for word in ["1", "true", "on"] {
            let w = self.const_str(word.as_bytes());
            self.text_call(
                5,
                [A::C(0), A::C(1), msg.ptr(), A::L(len), w.ptr(), w.len()],
            );
            if !first {
                self.op(I::I32Or);
            }
            first = false;
        }
        self.set_named(inst, "out_b", Ty::I);
    }
}
