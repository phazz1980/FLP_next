//! Strings in modules (`specs/wasm-io/strings.md`): a string signal is a fixed buffer in the
//! module memory plus a global with its length. The master does the operations through the
//! `text` import (`h_text` in `master.ino`); opcodes are listed there.

use super::*;

/// One page of memory, the most a module may use.
const MEM_PAGE: u32 = 65536;
/// The first bytes of the memory are scratch for results of `text` (a float, four IP bytes).
pub(super) const MEM_START: u32 = 16;
/// Buffer size of a string signal; longer text is cut (the built-in simulator keeps all of it).
const STR_CAP: u32 = 128;
const TEXT_KINDS: [&str; 12] = [
    "ip_to_text",
    "text_to_ip",
    "text_const",
    "text_concat",
    "text_length",
    "num_to_text",
    "text_compare",
    "text_to_num",
    "text_substring",
    "text_find",
    "text_char",
    "text_add_char",
];
const CONCAT_PORTS: [&str; 16] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
];

/// Length of a string operand: known while building, or kept in a global.
#[derive(Clone, Copy)]
pub(super) enum Len {
    Const(u32),
    Global(u32),
}

/// A string operand: where the bytes are and how many.
#[derive(Clone, Copy)]
pub(super) struct Str {
    pub(super) ptr: u32,
    pub(super) len: Len,
}

/// The buffer of a string signal: `cap` bytes at `ptr`, the length is in global `len`.
#[derive(Clone, Copy)]
pub(super) struct Buf {
    pub(super) ptr: u32,
    pub(super) cap: u32,
    pub(super) len: u32,
}

/// An argument of `text`: a constant, a local, a global or a float local (as bits).
#[derive(Clone, Copy)]
pub(super) enum A {
    C(i32),
    L(u32),
    G(u32),
    F(u32),
}

impl Str {
    fn ptr(self) -> A {
        A::C(self.ptr as i32)
    }

    fn len(self) -> A {
        match self.len {
            Len::Const(n) => A::C(n as i32),
            Len::Global(g) => A::G(g),
        }
    }
}

impl Buf {
    pub(super) fn ptr(self) -> A {
        A::C(self.ptr as i32)
    }

    pub(super) fn cap(self) -> A {
        A::C(self.cap as i32)
    }
}

impl Emitter<'_, '_> {
    /// `n` bytes of module memory; `mem_overflow` is set when the page is full.
    pub(super) fn alloc(&mut self, n: u32) -> u32 {
        let at = (self.mem_top + 3) & !3;
        self.mem_top = at + n;
        if self.mem_top > MEM_PAGE {
            self.mem_overflow = true;
        }
        at
    }

    /// Constant bytes in the data segment (equal constants share one copy).
    fn data(&mut self, bytes: &[u8]) -> u32 {
        if bytes.is_empty() {
            return 0;
        }
        if let Some(&at) = self.consts.get(bytes) {
            return at;
        }
        let at = self.alloc(bytes.len() as u32);
        self.data.push((at, bytes.to_vec()));
        self.consts.insert(bytes.to_vec(), at);
        at
    }

    pub(super) fn const_str(&mut self, bytes: &[u8]) -> Str {
        Str {
            ptr: self.data(bytes),
            len: Len::Const(bytes.len() as u32),
        }
    }

    /// Creates the buffer of a string output signal (called while the emitter is built).
    pub(super) fn string_signal(&mut self, inst: &Instance, signal: usize) {
        let buf = if inst.block.kind == "text_const" {
            let text = self.text_param(inst, "value");
            let ptr = self.data(text.as_bytes());
            let len = self.global(Ty::I, &ParamValue::Int(text.len() as i64));
            Buf {
                ptr,
                cap: text.len() as u32,
                len,
            }
        } else {
            let cap = match (inst.block.kind.as_str(), inst.params.get("max_len")) {
                ("uart_read", Some(ParamValue::Int(n))) => (*n).clamp(1, 1024) as u32,
                _ => STR_CAP,
            };
            Buf {
                ptr: self.alloc(cap),
                cap,
                len: self.global(Ty::I, &ParamValue::Int(0)),
            }
        };
        self.bufs.insert(signal, buf);
        self.signals.insert(signal, buf.len);
    }

    /// A string signal can be used only inside its sheet and not copied to a variable.
    pub(super) fn string_outputs_ok(&self, inst: &Instance) -> Result<(), WasmError> {
        for (&name, &signal) in &inst.outputs {
            if !self.bufs.contains_key(&signal) {
                continue;
            }
            let label = signal_label(self.program, signal);
            if self.tag_of(&TagKind::Signal(label)).is_some() || inst.output_vars.contains_key(name)
            {
                return Err(self.unsupported(
                    inst,
                    "строку нельзя передать на другой лист или в переменную: в модулях это пока не поддержано",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn out_buf(&self, inst: &Instance, name: &str) -> Option<Buf> {
        let signal = inst.outputs.get(name)?;
        self.bufs.get(signal).copied()
    }

    /// The string on input `port`; an input that is off is an empty string.
    fn str_in(&mut self, inst: &Instance, port: &str) -> Result<Str, WasmError> {
        let empty = Str {
            ptr: 0,
            len: Len::Const(0),
        };
        let Some(input) = inst.inputs.get(port) else {
            return Ok(empty);
        };
        match &input.source {
            Source::Default(ParamValue::Text(t)) => Ok(self.const_str(t.as_bytes())),
            Source::Default(_) => Ok(empty),
            Source::Var(_) => {
                Err(self.unsupported(inst, "строковые переменные пока не поддержаны в модулях"))
            }
            Source::Signal { index, .. } => match self.bufs.get(index) {
                Some(b) => Ok(Str {
                    ptr: b.ptr,
                    len: Len::Global(b.len),
                }),
                None => Err(self.unsupported(inst, "строка с другого листа пока не поддержана")),
            },
        }
    }

    /// Room a copy of the string on `value` needs.
    fn value_cap(&mut self, inst: &Instance) -> Result<u32, WasmError> {
        Ok(match self.str_in(inst, "value")?.len {
            Len::Global(g) => self
                .bufs
                .values()
                .find(|b| b.len == g)
                .map_or(STR_CAP, |b| b.cap),
            Len::Const(n) => n.max(1),
        })
    }

    pub(super) fn push_arg(&mut self, a: A) {
        match a {
            A::C(x) => self.c_i(x),
            A::L(l) => self.op(I::LocalGet(l)),
            A::G(g) => self.op(I::GlobalGet(g)),
            A::F(l) => {
                self.op(I::LocalGet(l));
                self.op(I::I32ReinterpretF32);
            }
        }
    }

    /// `text(op, out, cap, a, alen, b, blen)`; leaves the result on the stack.
    pub(super) fn text_call(&mut self, op: i32, args: [A; 6]) {
        self.c_i(op);
        for a in args {
            self.push_arg(a);
        }
        self.hw_call(HW_TEXT);
    }

    /// Appends `s` to `out` at the length in local `at` (0 when `None`); the new length stays on the stack.
    fn append(&mut self, out: Buf, s: Str, at: Option<u32>) {
        let cur = at.map_or(A::C(0), A::L);
        self.text_call(1, [out.ptr(), out.cap(), s.ptr(), s.len(), cur, A::C(0)]);
    }

    pub(super) fn in_pos(inst: &Instance, name: &str) -> usize {
        inst.lib
            .def
            .inputs
            .iter()
            .position(|p| p.name == name)
            .unwrap_or(0)
    }

    /// Stores the value of type `ty` on the stack as output `name`, or drops it when it is off.
    pub(super) fn set_named(&mut self, inst: &Instance, name: &str, ty: Ty) {
        if inst.outputs.contains_key(name) {
            let i = inst
                .lib
                .def
                .outputs
                .iter()
                .position(|p| p.name == name)
                .unwrap_or(0);
            self.set_out(inst, i, ty);
        } else {
            self.op(I::Drop);
        }
    }

    fn mem_arg(offset: u64, align: u32) -> wasm_encoder::MemArg {
        wasm_encoder::MemArg {
            offset,
            align,
            target: wasm_encoder::MemArgTarget::Memory(0),
            ordering: None,
        }
    }

    /// The blocks of the "Строки" group.
    pub(super) fn text_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        let kind = inst.block.kind.as_str();
        if !TEXT_KINDS.contains(&kind) {
            return Ok(false);
        }
        self.string_outputs_ok(inst)?;
        match kind {
            "text_const" => {}
            "text_concat" => {
                let Some(out) = self.out_buf(inst, "out") else {
                    return Ok(true);
                };
                let count = self.named_param(inst, "count")?.clamp(1, 16) as usize;
                let sep = self.text_param(inst, "separator");
                let sep = self.const_str(sep.as_bytes());
                let len = self.tmp_i();
                for (k, port) in CONCAT_PORTS[..count].iter().enumerate() {
                    if k > 0 && !matches!(sep.len, Len::Const(0)) {
                        self.append(out, sep, Some(len));
                        self.op(I::LocalSet(len));
                    }
                    let s = self.str_in(inst, port)?;
                    self.append(out, s, (k > 0).then_some(len));
                    self.op(I::LocalSet(len));
                }
                self.op(I::LocalGet(len));
                self.op(I::GlobalSet(out.len));
            }
            "text_length" => {
                let s = self.str_in(inst, "in")?;
                self.push_arg(s.len());
                self.set_named(inst, "out", Ty::I);
            }
            "num_to_text" => {
                let Some(out) = self.out_buf(inst, "out") else {
                    return Ok(true);
                };
                let decimals = self.named_param(inst, "decimals")?.clamp(0, 9);
                let prefix = self.text_param(inst, "prefix");
                let prefix = self.const_str(prefix.as_bytes());
                let suffix = self.text_param(inst, "suffix");
                let suffix = self.const_str(suffix.as_bytes());
                let (x, len) = (self.tmp_f(), self.tmp_i());
                self.input_as(inst, Self::in_pos(inst, "in"), Ty::F);
                self.op(I::LocalSet(x));
                self.append(out, prefix, None);
                self.op(I::LocalSet(len));
                self.text_call(
                    2,
                    [
                        out.ptr(),
                        out.cap(),
                        A::F(x),
                        A::C(decimals),
                        A::L(len),
                        A::C(0),
                    ],
                );
                self.op(I::LocalSet(len));
                if !matches!(suffix.len, Len::Const(0)) {
                    self.append(out, suffix, Some(len));
                    self.op(I::LocalSet(len));
                }
                self.op(I::LocalGet(len));
                self.op(I::GlobalSet(out.len));
            }
            "text_compare" => {
                let a = self.str_in(inst, "a")?;
                let b = self.text_param(inst, "b");
                let b = self.const_str(b.as_bytes());
                let mode = self.named_param(inst, "mode")?;
                let nocase = i32::from(self.named_param(inst, "ignore_case")? != 0);
                self.text_call(
                    5,
                    [A::C(mode), A::C(nocase), a.ptr(), a.len(), b.ptr(), b.len()],
                );
                self.set_named(inst, "out", Ty::I);
            }
            "text_to_num" => {
                let s = self.str_in(inst, "in")?;
                let ok = self.tmp_i();
                self.text_call(6, [A::C(0), A::C(4), s.ptr(), s.len(), A::C(0), A::C(0)]);
                self.op(I::LocalSet(ok));
                if inst.outputs.contains_key("out") {
                    // The last number stays when the text is not one.
                    self.op(I::LocalGet(ok));
                    self.op(I::If(BlockType::Empty));
                    self.c_i(0);
                    self.op(I::F32Load(Self::mem_arg(0, 2)));
                    self.set_named(inst, "out", Ty::F);
                    self.op(I::End);
                }
                self.op(I::LocalGet(ok));
                self.set_named(inst, "ok", Ty::I);
            }
            "text_substring" => {
                let Some(out) = self.out_buf(inst, "out") else {
                    return Ok(true);
                };
                let s = self.str_in(inst, "in")?;
                let start = self.named_param(inst, "start")?;
                let len = self.named_param(inst, "len")?;
                self.text_call(
                    7,
                    [
                        out.ptr(),
                        out.cap(),
                        s.ptr(),
                        s.len(),
                        A::C(start),
                        A::C(len),
                    ],
                );
                self.op(I::GlobalSet(out.len));
            }
            "text_find" => {
                let text = self.str_in(inst, "text")?;
                let sub = self.str_in(inst, "sub")?;
                let from = self.named_param(inst, "from")?;
                let pos = self.tmp_i();
                self.text_call(
                    8,
                    [
                        A::C(from),
                        A::C(0),
                        text.ptr(),
                        text.len(),
                        sub.ptr(),
                        sub.len(),
                    ],
                );
                self.op(I::LocalSet(pos));
                self.op(I::LocalGet(pos));
                self.set_named(inst, "pos", Ty::I);
                self.op(I::LocalGet(pos));
                self.c_i(0);
                self.op(I::I32GeS);
                self.set_named(inst, "found", Ty::I);
            }
            "text_char" => {
                let text = self.str_in(inst, "text")?;
                let (index, code) = (self.tmp_i(), self.tmp_i());
                self.input_as(inst, Self::in_pos(inst, "index"), Ty::I);
                self.op(I::LocalSet(index));
                self.text_call(
                    9,
                    [
                        A::C(0),
                        A::C(0),
                        text.ptr(),
                        text.len(),
                        A::L(index),
                        A::C(0),
                    ],
                );
                self.op(I::LocalSet(code));
                self.op(I::LocalGet(code));
                self.c_i(0);
                self.op(I::I32GeS);
                self.op(I::If(BlockType::Empty));
                if let Some(out) = self.out_buf(inst, "char") {
                    self.text_call(
                        7,
                        [
                            out.ptr(),
                            out.cap(),
                            text.ptr(),
                            text.len(),
                            A::L(index),
                            A::C(1),
                        ],
                    );
                    self.op(I::GlobalSet(out.len));
                }
                self.op(I::Else);
                self.c_i(0);
                self.op(I::LocalSet(code));
                if let Some(out) = self.out_buf(inst, "char") {
                    self.c_i(0);
                    self.op(I::GlobalSet(out.len));
                }
                self.op(I::End);
                self.op(I::LocalGet(code));
                self.set_named(inst, "code", Ty::I);
            }
            "text_add_char" => {
                let Some(out) = self.out_buf(inst, "out") else {
                    return Ok(true);
                };
                let text = self.str_in(inst, "text")?;
                let (code, len) = (self.tmp_i(), self.tmp_i());
                self.input_as(inst, Self::in_pos(inst, "code"), Ty::I);
                self.op(I::LocalSet(code));
                self.append(out, text, None);
                self.op(I::LocalSet(len));
                self.text_call(
                    4,
                    [
                        out.ptr(),
                        out.cap(),
                        A::L(code),
                        A::C(0),
                        A::L(len),
                        A::C(0),
                    ],
                );
                self.op(I::GlobalSet(out.len));
            }
            "ip_to_text" => {
                let Some(out) = self.out_buf(inst, "out") else {
                    return Ok(true);
                };
                // The four bytes travel packed into one number, the first in the low byte.
                for (k, port) in ["a", "b", "c", "d"].iter().enumerate() {
                    self.input_as(inst, Self::in_pos(inst, port), Ty::I);
                    self.c_i(255);
                    self.op(I::I32And);
                    if k > 0 {
                        self.c_i(8 * k as i32);
                        self.op(I::I32Shl);
                        self.op(I::I32Or);
                    }
                }
                let packed = self.tmp_i();
                self.op(I::LocalSet(packed));
                self.text_call(
                    11,
                    [
                        out.ptr(),
                        out.cap(),
                        A::L(packed),
                        A::C(0),
                        A::C(0),
                        A::C(0),
                    ],
                );
                self.op(I::GlobalSet(out.len));
            }
            _ => {
                // text_to_ip
                let s = self.str_in(inst, "in")?;
                let ok = self.tmp_i();
                self.text_call(10, [A::C(0), A::C(4), s.ptr(), s.len(), A::C(0), A::C(0)]);
                self.op(I::LocalSet(ok));
                self.op(I::LocalGet(ok));
                self.op(I::If(BlockType::Empty));
                for (k, port) in ["a", "b", "c", "d"].iter().enumerate() {
                    if inst.outputs.contains_key(port) {
                        self.c_i(0);
                        self.op(I::I32Load8U(Self::mem_arg(k as u64, 0)));
                        self.set_named(inst, port, Ty::I);
                    }
                }
                self.op(I::End);
                self.op(I::LocalGet(ok));
                self.set_named(inst, "ok", Ty::I);
            }
        }
        Ok(true)
    }

    /// Copies the collected bytes to the `line` output and starts a new line.
    fn line_ready(&mut self, line: Option<Buf>, buf: u32, buf_len: u32) {
        if let Some(l) = line {
            self.text_call(
                1,
                [
                    l.ptr(),
                    l.cap(),
                    A::C(buf as i32),
                    A::G(buf_len),
                    A::C(0),
                    A::C(0),
                ],
            );
            self.op(I::GlobalSet(l.len));
        }
        self.c_i(0);
        self.op(I::GlobalSet(buf_len));
    }

    /// `uart_read` in line mode: the module collects bytes until the separator, as the C++ block does.
    pub(super) fn uart_read_line(&mut self, inst: &Instance, port: i32) -> Result<(), WasmError> {
        self.string_outputs_ok(inst)?;
        let line = self.out_buf(inst, "line");
        let end_mode = self.named_param(inst, "end")?;
        let max_len = self.named_param(inst, "max_len")?.clamp(1, 1024);
        let timeout = self.named_param(inst, "timeout")?;
        let end_char = match end_mode {
            0 => 10,
            1 => 13,
            _ => i32::from(
                self.text_param(inst, "end_char")
                    .bytes()
                    .next()
                    .unwrap_or(0),
            ),
        };
        let skip = match end_mode {
            0 => Some(13),
            1 => Some(10),
            _ => None,
        };
        let buf = self.alloc(max_len as u32);
        let (buf_len, last) = (self.state_i(), self.state_i());
        let (ready, c, end) = (self.tmp_i(), self.tmp_i(), self.tmp_i());
        self.c_i(0);
        self.op(I::LocalSet(ready));
        self.op(I::Block(BlockType::Empty));
        self.op(I::Loop(BlockType::Empty));
        self.uart_call(2, port, |e| e.c_i(0), 0);
        self.c_i(0);
        self.op(I::I32LeS);
        self.op(I::BrIf(1));
        self.uart_call(1, port, |e| e.c_i(0), 0);
        self.op(I::LocalSet(c));
        if timeout > 0 {
            self.op(I::GlobalGet(GLOBAL_NOW));
            self.op(I::GlobalSet(last));
        }
        if let Some(s) = skip {
            self.op(I::LocalGet(c));
            self.c_i(s);
            self.op(I::I32Eq);
            self.op(I::BrIf(0));
        }
        self.op(I::LocalGet(c));
        self.c_i(end_char);
        self.op(I::I32Eq);
        self.op(I::LocalSet(end));
        self.op(I::LocalGet(end));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        self.text_call(
            4,
            [
                A::C(buf as i32),
                A::C(max_len),
                A::L(c),
                A::C(0),
                A::G(buf_len),
                A::C(0),
            ],
        );
        self.op(I::GlobalSet(buf_len));
        self.op(I::GlobalGet(buf_len));
        self.c_i(max_len);
        self.op(I::I32GeS);
        self.op(I::LocalSet(end));
        self.op(I::End);
        self.op(I::LocalGet(end));
        self.op(I::If(BlockType::Empty));
        self.line_ready(line, buf, buf_len);
        self.c_i(1);
        self.op(I::LocalSet(ready));
        self.op(I::Br(2));
        self.op(I::End);
        self.op(I::Br(0));
        self.op(I::End);
        self.op(I::End);
        if timeout > 0 {
            // No separator for a while: what came is a line too.
            self.op(I::LocalGet(ready));
            self.op(I::I32Eqz);
            self.op(I::GlobalGet(buf_len));
            self.c_i(0);
            self.op(I::I32GtS);
            self.op(I::I32And);
            self.op(I::GlobalGet(GLOBAL_NOW));
            self.op(I::GlobalGet(last));
            self.op(I::I32Sub);
            self.c_i(timeout);
            self.op(I::I32GeU);
            self.op(I::I32And);
            self.op(I::If(BlockType::Empty));
            self.line_ready(line, buf, buf_len);
            self.c_i(1);
            self.op(I::LocalSet(ready));
            self.op(I::End);
        }
        if inst.outputs.contains_key("ready") {
            self.op(I::LocalGet(ready));
            self.set_named(inst, "ready", Ty::I);
        }
        if inst.outputs.contains_key("available") {
            self.uart_call(2, port, |e| e.c_i(0), 0);
            self.set_named(inst, "available", Ty::I);
        }
        Ok(())
    }

    /// Memory for the previous value of a string that `uart_send` sends on change.
    pub(super) fn str_last(&mut self, inst: &Instance) -> Result<Buf, WasmError> {
        let cap = self.value_cap(inst)?;
        Ok(Buf {
            ptr: self.alloc(cap),
            cap,
            len: self.state_i(),
        })
    }

    /// Pushes 1 when the string on `value` equals the one kept in `last`.
    pub(super) fn str_same(&mut self, inst: &Instance, last: Buf) -> Result<(), WasmError> {
        let v = self.str_in(inst, "value")?;
        self.text_call(
            5,
            [
                A::C(0),
                A::C(0),
                v.ptr(),
                v.len(),
                last.ptr(),
                A::G(last.len),
            ],
        );
        Ok(())
    }

    /// Keeps the string on `value` in `last`.
    pub(super) fn str_keep(&mut self, inst: &Instance, last: Buf) -> Result<(), WasmError> {
        let v = self.str_in(inst, "value")?;
        self.append(last, v, None);
        self.op(I::GlobalSet(last.len));
        Ok(())
    }

    /// `uart_send` in text mode, inside the `if fire` branch: leaves 1 (it went out) or 0 on the stack.
    pub(super) fn uart_send_text(&mut self, inst: &Instance, port: i32) -> Result<(), WasmError> {
        let value_ty = inst.inputs.get("value").map(|i| i.ty);
        let ending: &[u8] = match self.named_param(inst, "ending")? {
            0 => b"",
            1 => b"\n",
            _ => b"\r\n",
        };
        let cap = match value_ty {
            Some(SignalType::String) => self.value_cap(inst)?,
            _ => 48,
        } + 2;
        let compose = Buf {
            ptr: self.alloc(cap),
            cap,
            len: 0,
        };
        let len = self.tmp_i();
        match value_ty {
            Some(SignalType::String) => {
                let v = self.str_in(inst, "value")?;
                self.append(compose, v, None);
            }
            Some(SignalType::Float) => {
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
            }
            _ => {
                let n = self.tmp_i();
                self.input_as(inst, 1, Ty::I);
                if value_ty == Some(SignalType::Bool) {
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
        }
        self.op(I::LocalSet(len));
        if !ending.is_empty() {
            let e = self.const_str(ending);
            self.append(compose, e, Some(len));
            self.op(I::LocalSet(len));
        }
        self.c_i(6);
        self.c_i(port);
        self.c_i(compose.ptr as i32);
        self.op(I::LocalGet(len));
        self.hw_call(HW_UART);
        Ok(())
    }
}
