//! Wi-Fi and NTP in modules (`specs/wasm-io/network.md`): the master owns the interface and the
//! SNTP service (`FLP_NET` in `master.ino`), a module sets them up once and every step writes the
//! radio request and reads the last snapshot through the `net` import.

use super::text::{Buf, A};
use super::*;

impl Emitter<'_, '_> {
    /// `net(op, a, b, c, 0)`; leaves the result on the stack.
    fn net_call(&mut self, op: i32, a: i32, b: A, c: A) {
        self.c_i(op);
        self.c_i(a);
        self.push_arg(b);
        self.push_arg(c);
        self.c_i(0);
        self.hw_call(HW_NET);
    }

    fn net_num(&mut self, code: i32, value: i32) {
        self.net_call(0, code, A::C(value), A::C(0));
        self.op(I::Drop);
    }

    /// A text setting of the master (`ssid`, `ip`...), sent once.
    fn net_text(&mut self, inst: &Instance, code: i32, name: &str) -> Result<(), WasmError> {
        if inst.param_vars.contains_key(name) {
            return Err(self.unsupported(
                inst,
                &format!("параметр «{name}» из переменной пока не поддержан"),
            ));
        }
        let text = self.text_param(inst, name);
        let s = self.const_str(text.as_bytes());
        self.net_call(1, code, A::C(s.ptr as i32), A::C(text.len() as i32));
        self.op(I::Drop);
        Ok(())
    }

    pub(super) fn net_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        match inst.block.kind.as_str() {
            "wifi" => self.wifi_block(inst)?,
            "ethernet" => self.ethernet_block(inst)?,
            "ntp" => self.ntp_block(inst)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn wifi_block(&mut self, inst: &Instance) -> Result<(), WasmError> {
        self.string_outputs_ok(inst)?;
        // The web page and the firmware upload of the block come with the web blocks.
        let mode = self.named_param(inst, "wifi_mode")?;
        let (client, ap) = (mode != 1, mode != 0);
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        if client {
            self.net_text(inst, 1, "ssid")?;
            self.net_text(inst, 2, "password")?;
            let dhcp = self.named_param(inst, "dhcp")?;
            self.net_num(1, dhcp);
            if dhcp == 0 {
                self.net_text(inst, 3, "ip")?;
                self.net_text(inst, 4, "gateway")?;
                self.net_text(inst, 5, "subnet")?;
            }
        }
        if ap {
            self.net_text(inst, 6, "ap_ssid")?;
            self.net_text(inst, 7, "ap_password")?;
        }
        self.net_num(63, 0);
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);

        // Radio request: mode + 1 (client 1, access point 2, both 3), 0 while `en` is off.
        if mode == 3 {
            self.input_as(inst, Self::in_pos(inst, "mode"), Ty::I);
            let m = self.tmp_i();
            self.op(I::LocalSet(m));
            // Out of 0..=2 means the client, as in the C++ block.
            self.op(I::LocalGet(m));
            self.c_i(0);
            self.op(I::LocalGet(m));
            self.c_i(2);
            self.op(I::I32LeU);
            self.op(I::Select);
        } else {
            self.c_i(mode);
        }
        self.c_i(1);
        self.op(I::I32Add);
        if inst.inputs.contains_key("en") {
            self.flag(inst, Self::in_pos(inst, "en"));
            self.op(I::I32Mul);
        }
        let state = self.tmp_i();
        self.op(I::LocalSet(state));
        self.c_i(2);
        self.op(I::LocalGet(state));
        self.c_i(0);
        self.c_i(0);
        self.c_i(0);
        self.hw_call(HW_NET);
        self.op(I::Drop);

        for (name, field) in [("ready", 0), ("ap_ready", 1), ("status", 2), ("error", 3)] {
            if inst.outputs.contains_key(name) {
                self.net_call(3, field, A::C(0), A::C(0));
                self.set_named(inst, name, Ty::I);
            }
        }
        if let Some(out) = self.out_buf(inst, "address") {
            self.net_call(3, 4, A::C(0), A::C(0));
            let ip = self.tmp_i();
            self.op(I::LocalSet(ip));
            self.op(I::LocalGet(ip));
            self.op(I::If(BlockType::Empty));
            self.text_call(
                11,
                [out.ptr(), out.cap(), A::L(ip), A::C(0), A::C(0), A::C(0)],
            );
            self.op(I::GlobalSet(out.len));
            self.op(I::Else);
            self.c_i(0);
            self.op(I::GlobalSet(out.len));
            self.op(I::End);
        }
        Ok(())
    }

    /// Same as `net_text` for the Ethernet settings (op 7).
    fn eth_text(&mut self, inst: &Instance, code: i32, name: &str) -> Result<(), WasmError> {
        if inst.param_vars.contains_key(name) {
            return Err(self.unsupported(
                inst,
                &format!("параметр «{name}» из переменной пока не поддержан"),
            ));
        }
        let text = self.text_param(inst, name);
        let s = self.const_str(text.as_bytes());
        self.net_call(7, code, A::C(s.ptr as i32), A::C(text.len() as i32));
        self.op(I::Drop);
        Ok(())
    }

    fn ethernet_block(&mut self, inst: &Instance) -> Result<(), WasmError> {
        self.string_outputs_ok(inst)?;
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        // Pins and MAC are taken by the master when it creates the interface.
        for (code, name) in [(2, "sck"), (3, "miso"), (4, "mosi"), (5, "cs"), (6, "mac")] {
            let value = self.named_param(inst, name)?;
            self.net_call(6, code, A::C(value), A::C(0));
            self.op(I::Drop);
        }
        let dhcp = self.named_param(inst, "dhcp")?;
        self.net_call(6, 1, A::C(dhcp), A::C(0));
        self.op(I::Drop);
        if dhcp == 0 {
            self.eth_text(inst, 1, "ip")?;
            self.eth_text(inst, 2, "gateway")?;
            self.eth_text(inst, 3, "subnet")?;
        }
        self.net_call(6, 63, A::C(0), A::C(0));
        self.op(I::Drop);
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);

        if inst.outputs.contains_key("ready") {
            self.net_call(8, 0, A::C(0), A::C(0));
            self.set_named(inst, "ready", Ty::I);
        }
        if let Some(out) = self.out_buf(inst, "address") {
            self.net_call(8, 1, A::C(0), A::C(0));
            let ip = self.tmp_i();
            self.op(I::LocalSet(ip));
            self.op(I::LocalGet(ip));
            self.op(I::If(BlockType::Empty));
            self.text_call(
                11,
                [out.ptr(), out.cap(), A::L(ip), A::C(0), A::C(0), A::C(0)],
            );
            self.op(I::GlobalSet(out.len));
            self.op(I::Else);
            self.c_i(0);
            self.op(I::GlobalSet(out.len));
            self.op(I::End);
        }
        Ok(())
    }

    fn ntp_block(&mut self, inst: &Instance) -> Result<(), WasmError> {
        self.string_outputs_ok(inst)?;
        let eth = self.text_param(inst, "net") == "ethernet";
        let period = self.named_param(inst, "period")?;
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        self.net_text(inst, 8, "server")?;
        self.net_num(2, period);
        self.net_num(3, eth as i32);
        self.net_num(64, 0);
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);

        // Zone and daylight saving are added here, the service keeps UTC.
        let zone = self.named_param_f(inst, "zone");
        let dst = if self.named_param(inst, "dst")? != 0 {
            3600
        } else {
            0
        };
        let offset = (zone * 3600.0) as i32 + dst;

        self.net_call(4, 0, A::C(0), A::C(0));
        let ok = self.tmp_i();
        self.op(I::LocalSet(ok));
        if inst.outputs.contains_key("valid") {
            self.op(I::LocalGet(ok));
            self.set_named(inst, "valid", Ty::I);
        }
        if inst.outputs.contains_key("age") {
            self.net_call(4, 1, A::C(0), A::C(0));
            self.set_named(inst, "age", Ty::I);
        }
        let parts: Vec<(&str, i32)> = [
            ("hour", 2),
            ("minute", 3),
            ("second", 4),
            ("day", 5),
            ("month", 6),
            ("year", 7),
            ("weekday", 8),
        ]
        .into_iter()
        .filter(|(name, _)| inst.outputs.contains_key(name))
        .collect();
        let text: Option<Buf> = self.out_buf(inst, "text");
        let unix = inst.outputs.contains_key("unix");
        if parts.is_empty() && text.is_none() && !unix {
            return Ok(());
        }
        // Like the C++ block, the outputs keep their last values until the time is known.
        self.op(I::LocalGet(ok));
        self.op(I::If(BlockType::Empty));
        if unix {
            self.net_call(4, 9, A::C(0), A::C(0));
            self.set_named(inst, "unix", Ty::I);
        }
        for (name, field) in parts {
            self.net_call(4, field, A::C(offset), A::C(0));
            self.set_named(inst, name, Ty::I);
        }
        if let Some(out) = text {
            self.net_call(5, out.ptr as i32, A::C(out.cap as i32), A::C(offset));
            self.op(I::GlobalSet(out.len));
        }
        self.op(I::End);
        Ok(())
    }
}
