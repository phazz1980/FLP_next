//! A sheet of the schema as an ABI v3 WASM module, written straight with `wasm-encoder`.
//!
//! Values are 32 bit, as in the numeric model (`specs/numeric-model`): `bool` and `int` are `i32`
//! (wrapping), `float` is `f32`, `now` is an `i32` of milliseconds that wraps; so a module gives
//! the same bits as `crab-sim`. Block outputs are mutable globals that keep their value between
//! steps; values that cross the sheet border go through `tag_get_*` / `tag_set_*`.

use std::collections::HashMap;

use crab_core::value::parse_duration_ms;
use crab_core::{ParamKind, ParamValue, SignalType};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};

use crate::program::{Instance, Program, Source};
use crate::sheet::{signal_label, SheetError, SheetModule, TagKind};

mod mqtt;
mod net;
mod text;
mod web;
use text::{Buf, MEM_START};

const TAG_GET_I: u32 = 0;
const TAG_GET_F: u32 = 1;
const TAG_SET_I: u32 = 2;
const TAG_SET_F: u32 = 3;
const OUTPUT_I: u32 = 4;
const OUTPUT_F: u32 = 5;
/// Imports every module declares; the hardware ones follow.
const FIXED_IMPORTS: u32 = 6;
const GLOBAL_NOW: u32 = 0;

/// Hardware imports are numbered after the fixed ones once the used set is known; code is
/// emitted with these placeholders (`HW_BASE + position in HW_NAMES`) and remapped in `finish`.
const HW_BASE: u32 = 100;
const HW_GPIO_READ: u32 = HW_BASE;
const HW_GPIO_WRITE: u32 = HW_BASE + 1;
const HW_ADC_READ: u32 = HW_BASE + 2;
const HW_PWM_WRITE: u32 = HW_BASE + 3;
const HW_SENSOR_READ: u32 = HW_BASE + 4;
const HW_PCF_READ: u32 = HW_BASE + 5;
const HW_PCF_WRITE: u32 = HW_BASE + 6;
const HW_MODBUS: u32 = HW_BASE + 7;
const HW_UART: u32 = HW_BASE + 8;
const HW_TEXT: u32 = HW_BASE + 9;
const HW_NET: u32 = HW_BASE + 10;
const HW_NATIVE: u32 = HW_BASE + 11;
const HW_NAMES: [&str; 12] = [
    "gpio_read",
    "gpio_write",
    "adc_read",
    "pwm_write",
    "sensor_read",
    "pcf_read",
    "pcf_write",
    "modbus",
    "uart",
    "text",
    "net",
    "native",
];
/// Bit of the pin number passed to `gpio_read` that asks for the pull-up.
const PIN_PULLUP: i32 = 0x100;
/// `f32` scratch locals are numbered from here until `finish` knows how many `i32` ones there are.
const F_LOCAL_BASE: u32 = 1000;

/// A finished module: the `.wasm` bytes and the text of `module.toml`.
#[derive(Debug, Clone)]
pub struct WasmModule {
    pub name: String,
    pub wasm: Vec<u8>,
    pub manifest: String,
    /// With `watch`: `block.port` of every signal, in the order of the `output` index.
    pub watched: Vec<String>,
}

/// Why a sheet cannot become a WASM module.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum WasmError {
    #[error(transparent)]
    Sheet(#[from] SheetError),
    #[error("блок «{block}» ({kind}): {reason}")]
    Unsupported {
        block: String,
        kind: String,
        reason: String,
    },
}

impl Program<'_> {
    /// Builds sheet `sheet` as a module.
    pub fn wasm_module(&self, sheet: u32) -> Result<WasmModule, WasmError> {
        self.wasm_module_with(sheet, false)
    }

    /// With `watch` the module reports every block output through `output_i/f(i, v)`:
    /// for tests and the simulator, not for the board.
    pub fn wasm_module_with(&self, sheet: u32, watch: bool) -> Result<WasmModule, WasmError> {
        let module = self.sheet_module(sheet)?;
        let mut em = Emitter::new(self, &module);
        em.watch = watch;
        em.step_body()?;
        if em.mem_overflow {
            return Err(WasmError::Unsupported {
                block: String::new(),
                kind: String::new(),
                reason: "строки не помещаются в 64 КБ памяти модуля".to_owned(),
            });
        }
        Ok(em.finish())
    }
}

/// Watched signals per module the master firmware keeps (`kMaxOutputs`).
const MAX_WATCHED: usize = 128;

/// The two machine types of the model: `i32` carries `bool` and `int`, `f32` carries `float`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ty {
    I,
    F,
}

impl Ty {
    fn of(ty: SignalType) -> Ty {
        if ty == SignalType::Float {
            Ty::F
        } else {
            Ty::I
        }
    }

    fn of_kind(kind: ParamKind) -> Ty {
        if kind == ParamKind::Float {
            Ty::F
        } else {
            Ty::I
        }
    }

    fn val(self) -> ValType {
        match self {
            Ty::I => ValType::I32,
            Ty::F => ValType::F32,
        }
    }

    /// `float` if either is.
    fn widen(self, other: Ty) -> Ty {
        if self == Ty::F || other == Ty::F {
            Ty::F
        } else {
            Ty::I
        }
    }
}

fn pv(value: &ParamValue) -> f64 {
    match value {
        ParamValue::Bool(b) => f64::from(u8::from(*b)),
        ParamValue::Int(i) => *i as f64,
        ParamValue::Float(x) => *x,
        ParamValue::Text(_) => 0.0,
    }
}

/// Parameter value as a number; durations become milliseconds.
fn num(value: &ParamValue, kind: ParamKind) -> f64 {
    match value {
        ParamValue::Text(s) if kind == ParamKind::Duration => {
            parse_duration_ms(s).map_or(0.0, |ms| ms as f64)
        }
        other => pv(other),
    }
}

/// A global: its type and the bits of the initial value.
struct Global {
    ty: Ty,
    bits: u32,
}

struct Emitter<'a, 'p> {
    program: &'a Program<'p>,
    module: &'a SheetModule,
    table: Vec<crate::sheet::Tag>,
    code: Vec<I<'static>>,
    /// Globals; index 0 is `now`.
    globals: Vec<Global>,
    signals: HashMap<usize, u32>,
    vars: HashMap<String, u32>,
    /// `i32` and `f32` scratch locals in use by the current block (`i32` ones start at 1: local 0 is `dt`).
    scratch_i: u32,
    scratch_f: u32,
    /// Most scratch locals any block needed at once; blocks reuse the same locals.
    peak_i: u32,
    peak_f: u32,
    watch: bool,
    watched: Vec<String>,
    /// Hardware imports the code calls (placeholders, see `HW_BASE`).
    hw: std::collections::BTreeSet<u32>,
    /// Offsets in `code` where `step` is cut into separate functions (block boundaries only):
    /// wasm3 refuses a function that overruns its stack height limit.
    cuts: Vec<usize>,
    /// Next free byte of the module memory (strings live there), see `text.rs`.
    mem_top: u32,
    mem_overflow: bool,
    /// Constant bytes of the data segment: (offset, bytes) and the same by content.
    data: Vec<(u32, Vec<u8>)>,
    consts: HashMap<Vec<u8>, u32>,
    /// Buffers of string signals by signal index.
    bufs: HashMap<usize, Buf>,
}

/// Blocks per generated function. wasm3 without tail-call optimization (ESP32 build) recurses once
/// per operation of a function, so a long function overflows the task stack; short ones do not.
const BLOCKS_PER_CHUNK: usize = 8;
/// Pieces called straight from `step`; beyond that they go through groups of this size.
const PIECES_PER_GROUP: u32 = 16;

/// Group functions needed for `pieces` pieces after the first (0: `step` calls them itself).
fn group_count(pieces: usize) -> usize {
    if pieces as u32 <= PIECES_PER_GROUP {
        0
    } else {
        (pieces as u32).div_ceil(PIECES_PER_GROUP) as usize
    }
}

impl<'a, 'p> Emitter<'a, 'p> {
    fn new(program: &'a Program<'p>, module: &'a SheetModule) -> Self {
        let mut em = Self {
            program,
            module,
            table: program.tag_table(),
            code: Vec::new(),
            globals: vec![Global { ty: Ty::I, bits: 0 }],
            signals: HashMap::new(),
            vars: HashMap::new(),
            scratch_i: 0,
            scratch_f: 0,
            peak_i: 0,
            peak_f: 0,
            watch: false,
            watched: Vec::new(),
            hw: Default::default(),
            cuts: Vec::new(),
            mem_top: MEM_START,
            mem_overflow: false,
            data: Vec::new(),
            consts: HashMap::new(),
            bufs: HashMap::new(),
        };
        for &bi in &module.blocks {
            for &signal in program.blocks[bi].outputs.values() {
                if program.signals[signal].ty == SignalType::String {
                    em.string_signal(&program.blocks[bi], signal);
                    continue;
                }
                let g = em.global(Ty::of(program.signals[signal].ty), &ParamValue::Int(0));
                em.signals.insert(signal, g);
            }
        }
        for local in &module.locals {
            let g = em.global(Ty::of(local.ty), &local.init);
            em.vars.insert(local.name.clone(), g);
        }
        em
    }

    /// A new global of type `ty` with the initial value `init` converted as the model does.
    fn global(&mut self, ty: Ty, init: &ParamValue) -> u32 {
        let x = pv(init);
        let bits = match ty {
            Ty::I => (x as i32) as u32,
            Ty::F => (x as f32).to_bits(),
        };
        self.globals.push(Global { ty, bits });
        (self.globals.len() - 1) as u32
    }

    fn state_i(&mut self) -> u32 {
        self.global(Ty::I, &ParamValue::Int(0))
    }

    fn tmp_i(&mut self) -> u32 {
        self.scratch_i += 1;
        self.peak_i = self.peak_i.max(self.scratch_i);
        self.scratch_i
    }

    fn tmp_f(&mut self) -> u32 {
        self.scratch_f += 1;
        self.peak_f = self.peak_f.max(self.scratch_f);
        F_LOCAL_BASE + self.scratch_f - 1
    }

    fn tmp(&mut self, ty: Ty) -> u32 {
        match ty {
            Ty::I => self.tmp_i(),
            Ty::F => self.tmp_f(),
        }
    }

    fn op(&mut self, i: I<'static>) {
        self.code.push(i);
    }

    fn c_i(&mut self, x: i32) {
        self.op(I::I32Const(x));
    }

    fn c_f(&mut self, x: f32) {
        self.op(I::F32Const(x.into()));
    }

    /// A constant `x` of type `ty` (as the model casts a number to it).
    fn c(&mut self, ty: Ty, x: f64) {
        match ty {
            Ty::I => self.c_i(x as i32),
            Ty::F => self.c_f(x as f32),
        }
    }

    /// Converts the value on the stack: `int` → `float` rounds, `float` → `int` saturates (NaN is 0).
    fn coerce(&mut self, from: Ty, to: Ty) {
        match (from, to) {
            (Ty::I, Ty::F) => self.op(I::F32ConvertI32S),
            (Ty::F, Ty::I) => self.op(I::I32TruncSatF32S),
            _ => {}
        }
    }

    /// Value on the stack → `1` if it is not zero, else `0` (an `i32`).
    fn flagify(&mut self, ty: Ty) {
        match ty {
            Ty::I => {
                self.c_i(0);
                self.op(I::I32Ne);
            }
            Ty::F => {
                self.c_f(0.0);
                self.op(I::F32Ne);
            }
        }
    }

    /// Comparison `op` (0 `==`, 1 `!=`, 2 `<`, 3 `<=`, 4 `>`, else `>=`) of two values of type `ty`.
    fn compare(&mut self, ty: Ty, op: i64) {
        self.op(match (ty, op) {
            (Ty::I, 0) => I::I32Eq,
            (Ty::I, 1) => I::I32Ne,
            (Ty::I, 2) => I::I32LtS,
            (Ty::I, 3) => I::I32LeS,
            (Ty::I, 4) => I::I32GtS,
            (Ty::I, _) => I::I32GeS,
            (Ty::F, 0) => I::F32Eq,
            (Ty::F, 1) => I::F32Ne,
            (Ty::F, 2) => I::F32Lt,
            (Ty::F, 3) => I::F32Le,
            (Ty::F, 4) => I::F32Gt,
            (Ty::F, _) => I::F32Ge,
        });
    }

    /// Two values of type `ty` on the stack → the smaller (`a < b ? a : b`) or the greater
    /// (`a > b ? a : b`): the same select as in the simulation, NaN included.
    fn pick(&mut self, ty: Ty, greater: bool) {
        let (a, b) = (self.tmp(ty), self.tmp(ty));
        self.op(I::LocalSet(b));
        self.op(I::LocalSet(a));
        self.op(I::LocalGet(a));
        self.op(I::LocalGet(b));
        self.op(I::LocalGet(a));
        self.op(I::LocalGet(b));
        self.compare(ty, if greater { 4 } else { 2 });
        self.op(I::Select);
    }

    fn unsupported(&self, inst: &Instance, reason: &str) -> WasmError {
        WasmError::Unsupported {
            block: inst.block.id.clone(),
            kind: inst.block.kind.clone(),
            reason: reason.to_owned(),
        }
    }

    fn tag_of(&self, kind: &TagKind) -> Option<(u32, Ty)> {
        self.table
            .iter()
            .find(|t| &t.kind == kind)
            .map(|t| (t.number, Ty::of(t.ty)))
    }

    fn var_ty(&self, name: &str) -> Ty {
        if let Some((_, ty)) = self.tag_of(&TagKind::Var(name.to_owned())) {
            ty
        } else if let Some(&g) = self.vars.get(name) {
            self.globals[g as usize].ty
        } else {
            Ty::I
        }
    }

    fn tag_get(&mut self, n: u32, ty: Ty) {
        self.c_i(n as i32);
        self.op(I::Call(if ty == Ty::F { TAG_GET_F } else { TAG_GET_I }));
    }

    /// The tag number `n` and the value of type `ty` are on the stack.
    fn tag_set_call(&mut self, ty: Ty) {
        self.op(I::Call(if ty == Ty::F { TAG_SET_F } else { TAG_SET_I }));
    }

    fn var_read(&mut self, name: &str) -> Ty {
        if let Some((n, ty)) = self.tag_of(&TagKind::Var(name.to_owned())) {
            self.tag_get(n, ty);
            ty
        } else if let Some(&g) = self.vars.get(name) {
            self.op(I::GlobalGet(g));
            self.globals[g as usize].ty
        } else {
            self.c_i(0);
            Ty::I
        }
    }

    /// Writes the value of type `from` on the stack into a variable.
    fn var_write(&mut self, name: &str, from: Ty) {
        let ty = self.var_ty(name);
        self.coerce(from, ty);
        if let Some((n, _)) = self.tag_of(&TagKind::Var(name.to_owned())) {
            let t = self.tmp(ty);
            self.op(I::LocalSet(t));
            self.c_i(n as i32);
            self.op(I::LocalGet(t));
            self.tag_set_call(ty);
        } else if let Some(&g) = self.vars.get(name) {
            self.op(I::GlobalSet(g));
        } else {
            self.op(I::Drop);
        }
    }

    /// Pushes the value on port `port` converted to the real type of the port.
    fn operand(&mut self, inst: &Instance, port: &str) -> Ty {
        let input = &inst.inputs[port];
        let want = Ty::of(input.ty);
        let from = match &input.source {
            Source::Default(v) => {
                self.c(want, pv(v));
                want
            }
            Source::Var(name) => self.var_read(name),
            Source::Signal { index, invert } => {
                let ty = Ty::of(self.program.signals[*index].ty);
                if let Some(&g) = self.signals.get(index) {
                    self.op(I::GlobalGet(g));
                } else {
                    let label = signal_label(self.program, *index);
                    let (n, tag_ty) = self
                        .tag_of(&TagKind::Signal(label))
                        .expect("cross-sheet link has a tag");
                    self.tag_get(n, tag_ty);
                    self.coerce(tag_ty, ty);
                }
                if *invert {
                    self.op(I::I32Eqz);
                }
                ty
            }
        };
        self.coerce(from, want);
        if inst.block.inverted_inputs.contains(port) {
            self.op(I::I32Eqz);
        }
        want
    }

    /// Type of input `i` in `block.toml` order (what `input` pushes); a port that is off counts as 0.
    fn input_ty(&self, inst: &Instance, i: usize) -> Ty {
        match inst.lib.def.inputs.get(i) {
            Some(p) => match inst.inputs.get(p.name.as_str()) {
                Some(input) => Ty::of(input.ty),
                None => Ty::of(p.ty).widen(Ty::I),
            },
            None => Ty::I,
        }
    }

    /// Input number `i` in `block.toml` order; zero if the port is off.
    fn input(&mut self, inst: &Instance, i: usize) -> Ty {
        match inst.lib.def.inputs.get(i) {
            Some(p) if inst.inputs.contains_key(p.name.as_str()) => self.operand(inst, &p.name),
            _ => {
                let ty = self.input_ty(inst, i);
                self.c(ty, 0.0);
                ty
            }
        }
    }

    /// Input `i` converted to `want`.
    fn input_as(&mut self, inst: &Instance, i: usize, want: Ty) {
        let ty = self.input(inst, i);
        self.coerce(ty, want);
    }

    /// Input `i` as an `i32` flag (not zero).
    fn flag(&mut self, inst: &Instance, i: usize) {
        let ty = self.input(inst, i);
        self.flagify(ty);
    }

    /// Type of parameter `i`: by its kind, or of the variable that feeds it.
    fn param_ty(&self, inst: &Instance, i: usize) -> Ty {
        let Some(p) = inst.lib.def.params.get(i) else {
            return Ty::F;
        };
        match inst.param_vars.get(p.name.as_str()) {
            Some(var) => self.var_ty(var),
            None => Ty::of_kind(p.kind),
        }
    }

    fn param(&mut self, inst: &Instance, i: usize) -> Ty {
        let Some(p) = inst.lib.def.params.get(i) else {
            self.c_f(0.0);
            return Ty::F;
        };
        if let Some(var) = inst.param_vars.get(p.name.as_str()) {
            self.var_read(var)
        } else {
            let ty = Ty::of_kind(p.kind);
            let x = inst
                .params
                .get(p.name.as_str())
                .map_or(0.0, |v| num(v, p.kind));
            self.c(ty, x);
            ty
        }
    }

    fn param_as(&mut self, inst: &Instance, i: usize, want: Ty) {
        let ty = self.param(inst, i);
        self.coerce(ty, want);
    }

    /// Parameter `i` as a flag (not zero).
    fn param_flag(&mut self, inst: &Instance, i: usize) {
        let ty = self.param(inst, i);
        self.flagify(ty);
    }

    /// A time parameter in milliseconds as an `i32` that stands for a `u32` (negative counts as 0).
    fn param_ms(&mut self, inst: &Instance, i: usize) {
        let Some(p) = inst.lib.def.params.get(i) else {
            return self.c_i(0);
        };
        if let Some(var) = inst.param_vars.get(p.name.as_str()) {
            match self.var_read(var) {
                Ty::F => self.op(I::I32TruncSatF32U),
                Ty::I => {
                    let x = self.tmp_i();
                    self.op(I::LocalSet(x));
                    self.op(I::LocalGet(x));
                    self.c_i(0);
                    self.op(I::LocalGet(x));
                    self.c_i(0);
                    self.op(I::I32GtS);
                    self.op(I::Select);
                }
            }
        } else {
            let ms = inst
                .params
                .get(p.name.as_str())
                .map_or(0.0, |v| num(v, p.kind));
            self.c_i((ms as u32) as i32);
        }
    }

    /// Constant value of parameter `i`, for blocks that cannot take it from a variable.
    fn const_param(&self, inst: &Instance, i: usize) -> Result<f64, WasmError> {
        let p = &inst.lib.def.params[i];
        if inst.param_vars.contains_key(p.name.as_str()) {
            return Err(self.unsupported(
                inst,
                &format!("параметр «{}» из переменной пока не поддержан", p.name),
            ));
        }
        Ok(inst
            .params
            .get(p.name.as_str())
            .map_or(0.0, |v| num(v, p.kind)))
    }

    /// Current value of output `i` (the previous one while the block has not written it).
    fn out_get(&mut self, inst: &Instance, i: usize) -> Ty {
        let name = inst.lib.def.outputs[i].name.as_str();
        let g = self.signals[&inst.outputs[name]];
        self.op(I::GlobalGet(g));
        self.globals[g as usize].ty
    }

    /// Stores the value of type `from` on the stack as output `i`: global, tag for other sheets,
    /// bound variable.
    fn set_out(&mut self, inst: &Instance, i: usize, from: Ty) {
        let name = inst.lib.def.outputs[i].name.as_str();
        let signal = inst.outputs[name];
        let g = self.signals[&signal];
        let ty = self.globals[g as usize].ty;
        self.coerce(from, ty);
        self.op(I::GlobalSet(g));
        let label = signal_label(self.program, signal);
        // The master keeps `MAX_WATCHED` signals per module; reporting more only grows the code.
        let slot = self.watched.iter().position(|l| *l == label).or_else(|| {
            (self.watch && self.watched.len() < MAX_WATCHED).then(|| {
                self.watched.push(label.clone());
                self.watched.len() - 1
            })
        });
        if let (true, Some(k)) = (self.watch, slot) {
            self.c_i(k as i32);
            self.op(I::GlobalGet(g));
            // The host answers with the value to use (a forced one while forcing).
            self.op(I::Call(if ty == Ty::F { OUTPUT_F } else { OUTPUT_I }));
            self.op(I::GlobalSet(g));
        }
        if let Some((n, tag_ty)) = self.tag_of(&TagKind::Signal(label)) {
            self.c_i(n as i32);
            self.op(I::GlobalGet(g));
            self.coerce(ty, tag_ty);
            self.tag_set_call(tag_ty);
        }
        if let Some(var) = inst.output_vars.get(name) {
            self.op(I::GlobalGet(g));
            self.var_write(var, ty);
        }
    }

    /// Live (active) inputs combined with `join`; `empty` when there are none.
    fn fold_flags(&mut self, inst: &Instance, join: I<'static>, empty: i32) {
        let live: Vec<usize> = inst
            .lib
            .def
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, p)| inst.inputs.contains_key(p.name.as_str()))
            .map(|(i, _)| i)
            .collect();
        if live.is_empty() {
            self.c_i(empty);
        }
        for (k, &i) in live.iter().enumerate() {
            self.flag(inst, i);
            if k > 0 {
                self.op(join.clone());
            }
        }
    }

    /// `if cond { then } else { otherwise }` giving a value of type `ty`; the condition (`i32`)
    /// is on the stack.
    fn choose(&mut self, ty: Ty, then: impl FnOnce(&mut Self), otherwise: impl FnOnce(&mut Self)) {
        self.op(I::If(BlockType::Result(ty.val())));
        then(self);
        self.op(I::Else);
        otherwise(self);
        self.op(I::End);
    }

    /// Milliseconds since the global `start` (an `i32` that stands for a `u32`).
    fn since(&mut self, start: u32) {
        self.op(I::GlobalGet(GLOBAL_NOW));
        self.op(I::GlobalGet(start));
        self.op(I::I32Sub);
    }

    fn step_body(&mut self) -> Result<(), WasmError> {
        self.op(I::GlobalGet(GLOBAL_NOW));
        self.op(I::LocalGet(0));
        self.op(I::I32Add);
        self.op(I::GlobalSet(GLOBAL_NOW));
        let module = self.module;
        let program = self.program;
        if let Some(ui) = module
            .blocks
            .iter()
            .map(|&bi| &program.blocks[bi])
            .find(|i| i.block.kind == "web_ui")
            .filter(|ui| self.web_enabled(ui))
        {
            // The page is set up before any widget reads or writes a value.
            self.scratch_i = 0;
            self.scratch_f = 0;
            self.web_setup(ui)?;
        }
        let blocks = self.module.blocks.clone();
        for (k, bi) in blocks.into_iter().enumerate() {
            if k > 0 && k % BLOCKS_PER_CHUNK == 0 {
                self.cuts.push(self.code.len());
            }
            // A block's temporaries never outlive it: state lives in globals.
            self.scratch_i = 0;
            self.scratch_f = 0;
            let program = self.program;
            self.block(&program.blocks[bi])?;
        }
        Ok(())
    }

    fn hw_call(&mut self, import: u32) {
        self.hw.insert(import);
        self.op(I::Call(import));
    }

    /// Writes an output block: `store` runs only when `en` allows.
    fn hold_unless_enabled(&mut self, inst: &Instance, store: impl FnOnce(&mut Self)) {
        let gated = inst.inputs.contains_key("en");
        if gated {
            self.flag(inst, 1);
            self.op(I::If(BlockType::Empty));
        }
        store(self);
        if gated {
            self.op(I::End);
        }
    }

    /// `i32` on the stack → clamped into `[lo, hi]`.
    fn clamp_int(&mut self, lo: i32, hi: i32) {
        self.c_i(lo);
        self.pick(Ty::I, true);
        self.c_i(hi);
        self.pick(Ty::I, false);
    }

    /// Sensors on 1-Wire/DHT pins: the master polls them on its own, the module takes the last
    /// values (`sensor_read(key, period_ms, field)`: 0 temperature, 1 humidity, 2 read ok).
    fn sensor_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        // (kind code, index of the pin param, index of the period param, outputs as fields)
        let (kind, index, period, fields): (i32, i32, usize, &[(usize, i32)]) =
            match inst.block.kind.as_str() {
                "ds18b20" => (0, self.const_param(inst, 1)? as i32, 2, &[(0, 0), (1, 2)]),
                "dht22" => (1, 0, 1, &[(0, 0), (1, 1), (2, 2)]),
                _ => return Ok(false),
            };
        if inst.block.expander.is_some() {
            return Err(self.unsupported(inst, "датчик на расширителе не поддержан"));
        }
        let pin = self.const_param(inst, 0)? as i32;
        let period_ms = self.const_param(inst, period)? as i32;
        // One byte of pin and one nibble of index fit the key the master tells sensors apart by.
        let key = kind | (index & 15) << 4 | (pin & 0xFF) << 8;
        for &(output, field) in fields {
            self.c_i(key);
            self.c_i(period_ms);
            self.c_i(field);
            self.hw_call(HW_SENSOR_READ);
            if inst.lib.def.outputs[output].name == "ok" {
                self.c_f(0.0);
                self.op(I::F32Ne);
                self.set_out(inst, output, Ty::I);
            } else {
                self.set_out(inst, output, Ty::F);
            }
        }
        Ok(true)
    }

    /// A block with a native part (`native.cpp`, `specs/drivers`): the master runs its `CRAB_STEP`.
    /// The module writes the inputs, then the parameters, as 4-byte words into its memory and calls
    /// `native(kind, inst, in_ptr, out_ptr)`; the outputs come back as words. `kind` is the index of
    /// the block type among the native types of the project (sorted by name), `inst` the number of
    /// this block among those of its type.
    fn native_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        if inst.lib.native_path().is_none() {
            return Ok(false);
        }
        let def = &inst.lib.def;
        let mut kinds: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
        for other in &self.program.blocks {
            if other.lib.native_path().is_some() {
                kinds
                    .entry(other.block.kind.as_str())
                    .or_default()
                    .push(other.block.id.as_str());
            }
        }
        let kind = kinds
            .keys()
            .position(|k| *k == inst.block.kind.as_str())
            .expect("this block is among the native ones");
        let number = kinds[inst.block.kind.as_str()]
            .iter()
            .position(|id| *id == inst.block.id.as_str())
            .expect("this block is among its type");
        let in_words = def.inputs.len() + def.params.len();
        let in_ptr = self.alloc(4 * in_words.max(1) as u32);
        let out_ptr = self.alloc(4 * def.outputs.len().max(1) as u32);
        let ty_of = |t: SignalType| if t == SignalType::Float { Ty::F } else { Ty::I };
        let store = |ty: Ty| {
            let arg = Self::mem_arg(0, 2);
            if ty == Ty::F {
                I::F32Store(arg)
            } else {
                I::I32Store(arg)
            }
        };
        let mut at = in_ptr;
        for (i, port) in def.inputs.iter().enumerate() {
            let ty = ty_of(port.ty);
            self.c_i(at as i32);
            self.input_as(inst, i, ty);
            self.op(store(ty));
            at += 4;
        }
        for (i, p) in def.params.iter().enumerate() {
            let ty = if p.kind == ParamKind::Float {
                Ty::F
            } else {
                Ty::I
            };
            self.c_i(at as i32);
            let got = self.param(inst, i);
            self.coerce(got, ty);
            self.op(store(ty));
            at += 4;
        }
        self.c_i(kind as i32);
        self.c_i(number as i32);
        self.c_i(in_ptr as i32);
        self.c_i(out_ptr as i32);
        self.hw_call(HW_NATIVE);
        // The master answers 0 when it cannot run the part: the outputs stay as they were.
        self.op(I::If(BlockType::Empty));
        for (k, port) in def.outputs.iter().enumerate() {
            if !inst.outputs.contains_key(port.name.as_str()) {
                continue;
            }
            let ty = ty_of(port.ty);
            self.c_i((out_ptr + 4 * k as u32) as i32);
            let arg = Self::mem_arg(0, 2);
            self.op(if ty == Ty::F {
                I::F32Load(arg)
            } else {
                I::I32Load(arg)
            });
            self.set_out(inst, k, ty);
        }
        self.op(I::End);
        Ok(true)
    }

    /// PCF8574 on I²C: the master talks to the chip, the module takes the last byte
    /// (`pcf_read(key, period_ms, field)`: 0 byte, 1 answered) or hands over the byte to write
    /// (`pcf_write(key, byte)` returns whether the chip answered the last write).
    fn pcf_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        let reading = match inst.block.kind.as_str() {
            "pcf8574_in" => true,
            "pcf8574_out" => false,
            _ => return Ok(false),
        };
        if inst.block.expander.is_some() {
            return Err(self.unsupported(inst, "микросхема PCF8574 на расширителе не поддержана"));
        }
        let address = self.const_param(inst, 0)? as i32;
        let sda = self.const_param(inst, 1)? as i32;
        let scl = self.const_param(inst, 2)? as i32;
        let inverted = self.const_param(inst, 3)? != 0.0;
        let count = (self.const_param(inst, 4)? as usize).clamp(1, 8);
        // One byte per pin; a negative pin (board default) becomes 0xFF.
        let key = address & 0x7F | (sda & 0xFF) << 8 | (scl & 0xFF) << 16;
        if reading {
            let period_ms = self.const_param(inst, 5)? as i32;
            let raw = self.tmp_i();
            self.c_i(key);
            self.c_i(period_ms);
            self.c_i(0);
            self.hw_call(HW_PCF_READ);
            if inverted {
                self.c_i(0xFF);
                self.op(I::I32Xor);
            }
            self.op(I::LocalSet(raw));
            for i in 0..count {
                if !inst
                    .outputs
                    .contains_key(inst.lib.def.outputs[i].name.as_str())
                {
                    continue;
                }
                self.op(I::LocalGet(raw));
                self.c_i(1 << i);
                self.op(I::I32And);
                self.c_i(0);
                self.op(I::I32Ne);
                self.set_out(inst, i, Ty::I);
            }
            self.c_i(key);
            self.c_i(period_ms);
            self.c_i(1);
            self.hw_call(HW_PCF_READ);
            self.set_out(inst, 8, Ty::I);
        } else {
            self.c_i(key);
            self.c_i(0);
            for i in 0..count {
                self.input_as(inst, i, Ty::I);
                self.c_i(1 << i);
                self.op(I::I32Mul);
                self.op(I::I32Or);
            }
            if inverted {
                self.c_i(0xFF);
                self.op(I::I32Xor);
            }
            self.hw_call(HW_PCF_WRITE);
            self.set_out(inst, 0, Ty::I);
        }
        Ok(true)
    }

    /// Text of a parameter (`table`, `kind`), empty when it is not set.
    fn text_param(&self, inst: &Instance, name: &str) -> String {
        match inst.params.get(name) {
            Some(ParamValue::Text(s)) => s.clone(),
            _ => String::new(),
        }
    }

    /// Number of a parameter by name, see `const_param`.
    fn named_param(&self, inst: &Instance, name: &str) -> Result<i32, WasmError> {
        let i = inst
            .lib
            .def
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or(0);
        Ok(self.const_param(inst, i)? as i32)
    }

    /// Number of a parameter by name as it is (a variable feeding it counts as 0).
    fn named_param_f(&self, inst: &Instance, name: &str) -> f64 {
        let i = inst
            .lib
            .def
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or(0);
        self.const_param(inst, i).unwrap_or(0.0)
    }

    /// `modbus(op, a, b, c, d)` with the first four as constants and `d` pushed by `d`.
    fn modbus_call(&mut self, op: i32, a: i32, b: i32, c: i32, d: impl FnOnce(&mut Self)) {
        self.c_i(op);
        self.c_i(a);
        self.c_i(b);
        self.c_i(c);
        d(self);
        self.hw_call(HW_MODBUS);
    }

    /// The `ip` parameter (dotted, as in the C++ output) as `IPAddress` bits: the first octet is
    /// the lowest byte.
    fn ip_bits(&self, inst: &Instance) -> Result<i32, WasmError> {
        let text = self.text_param(inst, "ip");
        let octets: Vec<u8> = text
            .split('.')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        let Ok(o): Result<[u8; 4], _> = octets.try_into() else {
            return Err(self.unsupported(inst, "адрес сервера должен быть вида 192.168.1.10"));
        };
        Ok(i32::from_le_bytes(o))
    }

    /// One setting of the master's Modbus (the answer is of no use).
    fn modbus_cfg(&mut self, code: i32, value: i32) {
        self.modbus_call(0, code, value, 0, |e| e.c_i(0));
        self.op(I::Drop);
    }

    /// Modbus RTU: the master firmware runs the protocol, blocks set it up (once) and read and
    /// write the tables. See `modbus` in `master.ino`.
    fn modbus_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        let kind = inst.block.kind.as_str();
        if !matches!(
            kind,
            "modbus_rtu_master"
                | "modbus_rtu_slave"
                | "modbus_tcp_master"
                | "modbus_tcp_slave"
                | "modbus_master_read"
                | "modbus_master_write"
                | "modbus_read"
                | "modbus_write"
        ) {
            return Ok(false);
        }
        let out_index = |name: &str| {
            inst.lib
                .def
                .outputs
                .iter()
                .position(|p| p.name == name)
                .unwrap_or(0)
        };
        if matches!(
            kind,
            "modbus_rtu_master" | "modbus_rtu_slave" | "modbus_tcp_master" | "modbus_tcp_slave"
        ) {
            let master = kind.ends_with("master");
            let tcp = kind.starts_with("modbus_tcp");
            // Code 0 comes first: it starts the list.
            let mut settings = if tcp {
                let eth = self.text_param(inst, "net") == "ethernet";
                let mut s = vec![
                    (0, if master { 3 } else { 4 }),
                    (1, i32::from(eth)),
                    (2, self.named_param(inst, "port")?),
                ];
                if master {
                    s.push((3, self.ip_bits(inst)?));
                }
                s
            } else {
                let Some((uart, _)) = crate::arduino::uart_of(self.program, inst) else {
                    return Err(self.unsupported(inst, "не найден UART блока"));
                };
                vec![
                    (0, if master { 1 } else { 2 }),
                    (1, self.named_param(inst, "uart")?),
                    (2, uart.speed as i32),
                    (3, i32::from(uart.parity_code())),
                    (4, uart.rx as i32),
                    (5, uart.tx as i32),
                    (6, uart.de as i32),
                ]
            };
            let tables = ["holding", "input", "coils", "discrete"];
            if master {
                let count = self.named_param(inst, "count")?.clamp(1, 4);
                settings.push((7, count));
                for n in 0..count {
                    let b = 16 + 8 * n;
                    settings.push((b, self.named_param(inst, &format!("slave{}", n + 1))?));
                    settings.push((b + 1, self.named_param(inst, "period")?));
                    settings.push((b + 2, self.named_param(inst, "timeout")?));
                    for (t, name) in tables.iter().enumerate() {
                        settings.push((b + 3 + t as i32, self.named_param(inst, name)?));
                    }
                }
            } else {
                settings.push((7, self.named_param(inst, "address")?));
                for (t, name) in tables.iter().enumerate() {
                    settings.push((8 + t as i32, self.named_param(inst, name)?));
                }
            }
            let done = self.state_i();
            self.op(I::GlobalGet(done));
            self.op(I::I32Eqz);
            self.op(I::If(BlockType::Empty));
            for (code, value) in settings {
                self.modbus_cfg(code, value);
            }
            self.modbus_cfg(63, 0);
            self.c_i(1);
            self.op(I::GlobalSet(done));
            self.op(I::End);
            if !master && inst.outputs.contains_key("error") {
                self.modbus_call(3, -1, i32::from(tcp) << 9, 0, |e| e.c_i(0));
                self.set_out(inst, out_index("error"), Ty::I);
            }
            return Ok(true);
        }
        let master = kind.starts_with("modbus_master");
        let writing = kind.ends_with("write");
        // Bit 9 of `b` addresses the TCP role (see `modbus` in `master.ino`).
        let tcp = if master {
            self.text_param(inst, "master") == "tcp"
        } else {
            self.text_param(inst, "slave") != "rtu"
        };
        let slave = if master {
            self.named_param(inst, "slave")?
        } else {
            0
        };
        let table = match self.text_param(inst, "table").as_str() {
            "input" => 1,
            "coil" => 2,
            "discrete" => 3,
            _ => 0,
        };
        let k = match self.text_param(inst, "kind").as_str() {
            "float" => 1,
            "bool" => 2,
            _ => 0,
        };
        let b = table | k << 4 | i32::from(master) << 8 | i32::from(tcp) << 9;
        let reg = self.named_param(inst, "reg")?;
        let port = ["", "_f", "_b"][k as usize];
        if writing {
            let name = format!("in{port}");
            let i = inst
                .lib
                .def
                .inputs
                .iter()
                .position(|p| p.name == name)
                .unwrap_or(0);
            self.modbus_call(2, slave, b, reg, |e| {
                if k == 1 {
                    e.input_as(inst, i, Ty::F);
                    e.op(I::I32ReinterpretF32);
                } else {
                    e.input_as(inst, i, Ty::I);
                }
            });
            self.op(I::Drop);
            return Ok(true);
        }
        let name = format!("out{port}");
        if inst.outputs.contains_key(name.as_str()) {
            self.modbus_call(1, slave, b, reg, |e| e.c_i(0));
            if k == 1 {
                self.op(I::F32ReinterpretI32);
                self.set_out(inst, out_index(&name), Ty::F);
            } else {
                self.set_out(inst, out_index(&name), Ty::I);
            }
        }
        if master && inst.outputs.contains_key("ok") {
            self.modbus_call(3, slave, i32::from(tcp) << 9, 0, |e| e.c_i(0));
            self.set_out(inst, out_index("ok"), Ty::I);
        }
        Ok(true)
    }

    /// `uart(op, port, a, b)` with `a` pushed by `a`.
    fn uart_call(&mut self, op: i32, port: i32, a: impl FnOnce(&mut Self), b: i32) {
        self.c_i(op);
        self.c_i(port);
        a(self);
        self.c_i(b);
        self.hw_call(HW_UART);
    }

    /// The port start-up, once per block: the master opens it with the block's settings.
    fn uart_setup(&mut self, port: i32, u: &crab_core::uart::UartConfig) {
        let (data, _, stop) = u.frame().unwrap_or((8, 'N', 1));
        let settings = [
            (0, 0),
            (1, u.speed as i32),
            (2, i32::from(data)),
            (3, i32::from(u.parity_code())),
            (4, i32::from(stop)),
            (5, u.rx as i32),
            (6, u.tx as i32),
            (7, u.de as i32),
            (8, u.rx_buffer as i32),
            (63, 0),
        ];
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        for (code, value) in settings {
            self.uart_call(0, port, |e| e.c_i(code), value);
            self.op(I::Drop);
        }
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);
    }

    /// UART bytes: the master owns the port; a module takes one byte per step or hands bytes
    /// over (see `uart` in `master.ino`).
    fn uart_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        let kind = inst.block.kind.as_str();
        if !matches!(kind, "uart_read" | "uart_send" | "uart_config") {
            return Ok(false);
        }
        let Some((u, _)) = crate::arduino::uart_of(self.program, inst) else {
            return Err(self.unsupported(inst, "не найден UART блока"));
        };
        if !(1..=2).contains(&u.number) {
            return Err(self.unsupported(
                inst,
                "USB и UART0 заняты протоколом мастера: в модуле доступны UART1 и UART2",
            ));
        }
        let port = i32::from(u.number);
        let out_index = |name: &str| {
            inst.lib
                .def
                .outputs
                .iter()
                .position(|p| p.name == name)
                .unwrap_or(0)
        };
        match kind {
            "uart_read" => {
                self.uart_setup(port, &u);
                if self.named_param(inst, "mode")? == 0 {
                    self.uart_read_line(inst, port)?;
                    return Ok(true);
                }
                let r = self.tmp_i();
                self.uart_call(1, port, |e| e.c_i(0), 0);
                self.op(I::LocalSet(r));
                if inst.outputs.contains_key("new") {
                    self.op(I::LocalGet(r));
                    self.c_i(0);
                    self.op(I::I32GeS);
                    self.set_out(inst, out_index("new"), Ty::I);
                }
                if inst.outputs.contains_key("byte") {
                    // The last byte stays while nothing new comes.
                    self.op(I::LocalGet(r));
                    self.c_i(0);
                    self.op(I::I32GeS);
                    self.op(I::If(BlockType::Empty));
                    self.op(I::LocalGet(r));
                    self.set_out(inst, out_index("byte"), Ty::I);
                    self.op(I::End);
                }
                if inst.outputs.contains_key("available") {
                    self.uart_call(2, port, |e| e.c_i(0), 0);
                    self.set_out(inst, out_index("available"), Ty::I);
                }
            }
            "uart_config" => {
                let format_used = inst.inputs.get("new_format").is_some_and(
                    |i| !matches!(&i.source, Source::Default(ParamValue::Text(t)) if t.is_empty()),
                );
                if format_used {
                    return Err(
                        self.unsupported(inst, "смена формата UART пока не поддержана в модулях")
                    );
                }
                self.uart_setup(port, &u);
                if inst.inputs.contains_key("apply") {
                    // On the rising edge of `apply` the speed changes.
                    let prev = self.state_i();
                    let apply = self.tmp_i();
                    let speed = inst
                        .lib
                        .def
                        .inputs
                        .iter()
                        .position(|p| p.name == "new_speed")
                        .unwrap_or(0);
                    let apply_in = inst
                        .lib
                        .def
                        .inputs
                        .iter()
                        .position(|p| p.name == "apply")
                        .unwrap_or(0);
                    self.flag(inst, apply_in);
                    self.op(I::LocalSet(apply));
                    self.op(I::LocalGet(apply));
                    self.op(I::GlobalGet(prev));
                    self.op(I::I32Eqz);
                    self.op(I::I32And);
                    self.op(I::If(BlockType::Empty));
                    self.uart_call(5, port, |e| e.input_as(inst, speed, Ty::I), 0);
                    self.op(I::Drop);
                    self.op(I::End);
                    self.op(I::LocalGet(apply));
                    self.op(I::GlobalSet(prev));
                    if inst.outputs.contains_key("ok") {
                        self.c_i(1);
                        self.set_out(inst, out_index("ok"), Ty::I);
                    }
                }
                self.uart_call(4, port, |e| e.c_i(0), 0);
                self.set_out(inst, out_index("baud"), Ty::I);
            }
            _ => {
                self.uart_setup(port, &u);
                self.uart_send(inst, port)?;
            }
        }
        Ok(true)
    }

    /// `uart_send` in binary mode: the value goes out as 1, 2 or 4 bytes when the trigger fires.
    fn uart_send(&mut self, inst: &Instance, port: i32) -> Result<(), WasmError> {
        let value_ty = inst.inputs.get("value").map(|i| i.ty);
        let text_mode =
            self.named_param(inst, "send_as")? == 0 || value_ty == Some(SignalType::String);
        let is_str = value_ty == Some(SignalType::String);
        let trigger = self.named_param(inst, "trigger")?;
        let ty = self.input_ty(inst, 1);
        let is_bool = value_ty == Some(SignalType::Bool);
        let int_size = self.named_param(inst, "int_size")?;
        let size = if is_bool {
            0
        } else if ty == Ty::F {
            2
        } else {
            int_size.clamp(0, 2)
        };
        let fire = self.tmp_i();
        let en = self.tmp_i();
        self.flag(inst, 0);
        self.op(I::LocalSet(en));
        let (started, last) = (self.state_i(), self.global(ty, &ParamValue::Int(0)));
        let last_str = if is_str && trigger == 2 {
            Some(self.str_last(inst)?)
        } else {
            None
        };
        match trigger {
            0 => {
                let prev = self.state_i();
                self.op(I::LocalGet(en));
                self.op(I::GlobalGet(prev));
                self.op(I::I32Eqz);
                self.op(I::I32And);
                self.op(I::LocalSet(fire));
                self.op(I::LocalGet(en));
                self.op(I::GlobalSet(prev));
            }
            1 => {
                self.op(I::LocalGet(en));
                self.op(I::LocalSet(fire));
            }
            _ => {
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
                self.op(I::LocalSet(fire));
            }
        }
        let sent = self.tmp_i();
        self.c_i(0);
        self.op(I::LocalSet(sent));
        self.op(I::LocalGet(fire));
        self.op(I::If(BlockType::Empty));
        if text_mode {
            self.uart_send_text(inst, port)?;
        } else {
            self.uart_call(
                3,
                port,
                |e| {
                    if ty == Ty::F {
                        e.input_as(inst, 1, Ty::F);
                        e.op(I::I32ReinterpretF32);
                    } else {
                        e.input_as(inst, 1, Ty::I);
                        if is_bool {
                            e.c_i(0);
                            e.op(I::I32Ne);
                        } else if size == 0 {
                            e.clamp_int(0, 255);
                        } else if size == 1 {
                            e.clamp_int(-32768, 32767);
                        }
                    }
                },
                size,
            );
        }
        self.op(I::LocalSet(sent));
        if trigger == 2 {
            self.op(I::LocalGet(sent));
            self.op(I::If(BlockType::Empty));
            if let Some(keep) = last_str {
                self.str_keep(inst, keep)?;
            } else {
                self.input_as(inst, 1, ty);
                self.op(I::GlobalSet(last));
            }
            self.c_i(1);
            self.op(I::GlobalSet(started));
            self.op(I::End);
        }
        self.op(I::End);
        if inst.outputs.contains_key("sent") {
            self.op(I::LocalGet(sent));
            let o = inst
                .lib
                .def
                .outputs
                .iter()
                .position(|p| p.name == "sent")
                .unwrap_or(0);
            self.set_out(inst, o, Ty::I);
        }
        Ok(())
    }

    /// Digital/analog/PWM pins of the board: calls into the master.
    fn hardware_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        if self.sensor_block(inst)?
            || self.pcf_block(inst)?
            || self.modbus_block(inst)?
            || self.uart_block(inst)?
            || self.text_block(inst)?
            || self.net_block(inst)?
            || self.native_block(inst)?
            || self.mqtt_block(inst)?
            || self.web_block(inst)?
        {
            return Ok(true);
        }
        let kind = inst.block.kind.as_str();
        if !matches!(kind, "digital_in" | "analog_in" | "digital_out" | "pwm_out") {
            return Ok(false);
        }
        if inst.block.expander.is_some() {
            return Err(self.unsupported(inst, "вывод на расширителе пока не поддержан в модулях"));
        }
        let pin = self.const_param(inst, 0)? as i32;
        match kind {
            "digital_in" => {
                let pullup = self.const_param(inst, 1)? != 0.0;
                let inverted = self.const_param(inst, 2)? != 0.0;
                self.c_i(pin | if pullup { PIN_PULLUP } else { 0 });
                self.hw_call(HW_GPIO_READ);
                self.c_i(0);
                self.op(I::I32Ne);
                if inverted {
                    self.op(I::I32Eqz);
                }
            }
            "analog_in" => {
                self.c_i(pin);
                self.hw_call(HW_ADC_READ);
                self.clamp_int(0, 4095);
            }
            "digital_out" => {
                let inverted = self.const_param(inst, 1)? != 0.0;
                let logic = self.state_i();
                self.hold_unless_enabled(inst, |e| {
                    e.flag(inst, 0);
                    e.op(I::GlobalSet(logic));
                });
                self.c_i(pin);
                self.op(I::GlobalGet(logic));
                if inverted {
                    self.op(I::I32Eqz);
                }
                self.hw_call(HW_GPIO_WRITE);
                return Ok(true);
            }
            _ => {
                let duty = self.global(Ty::F, &ParamValue::Float(0.0));
                self.hold_unless_enabled(inst, |e| {
                    e.input_as(inst, 0, Ty::F);
                    let x = e.tmp_f();
                    e.op(I::LocalSet(x));
                    // clamp(x, 0, 100) as selects
                    e.c_f(100.0);
                    e.op(I::LocalGet(x));
                    e.op(I::LocalGet(x));
                    e.c_f(100.0);
                    e.op(I::F32Gt);
                    e.op(I::Select);
                    e.op(I::LocalSet(x));
                    e.c_f(0.0);
                    e.op(I::LocalGet(x));
                    e.op(I::LocalGet(x));
                    e.c_f(0.0);
                    e.op(I::F32Lt);
                    e.op(I::Select);
                    e.op(I::GlobalSet(duty));
                });
                // The master passes the value to `analogWrite`: 8 bits, 0..255, rounded half up.
                self.c_i(pin);
                self.op(I::GlobalGet(duty));
                self.c_f(2.55);
                self.op(I::F32Mul);
                let y = self.tmp_f();
                self.op(I::LocalSet(y));
                self.op(I::LocalGet(y));
                self.op(I::F32Trunc);
                let t = self.tmp_f();
                self.op(I::LocalSet(t));
                self.op(I::LocalGet(t));
                self.c_f(1.0);
                self.c_f(0.0);
                self.op(I::LocalGet(y));
                self.op(I::LocalGet(t));
                self.op(I::F32Sub);
                self.c_f(0.5);
                self.op(I::F32Ge);
                self.op(I::Select);
                self.op(I::F32Add);
                self.op(I::I32TruncSatF32U);
                self.hw_call(HW_PWM_WRITE);
                return Ok(true);
            }
        }
        self.set_out(inst, 0, Ty::I);
        Ok(true)
    }

    /// Emits the block; the value of type `Ty` it leaves on the stack is output 0.
    fn block(&mut self, inst: &Instance) -> Result<(), WasmError> {
        if self.hardware_block(inst)? {
            return Ok(());
        }
        let has_string = inst.inputs.values().any(|i| i.ty == SignalType::String)
            || inst
                .outputs
                .values()
                .any(|&s| self.program.signals[s].ty == SignalType::String);
        if has_string {
            return Err(self.unsupported(inst, "строки в этом блоке пока не поддержаны в модулях"));
        }
        let kind = inst.block.kind.as_str();
        let ty = match kind {
            "and" => {
                self.fold_flags(inst, I::I32And, 1);
                Ty::I
            }
            "or" => {
                self.fold_flags(inst, I::I32Or, 0);
                Ty::I
            }
            "xor" => {
                self.fold_flags(inst, I::I32Xor, 0);
                Ty::I
            }
            "not" => {
                self.flag(inst, 0);
                self.op(I::I32Eqz);
                Ty::I
            }
            "rs_trigger" | "sr_trigger" => {
                // rs: reset wins; sr: set wins. The output keeps its value otherwise.
                let (first, second) = if kind == "rs_trigger" { (1, 0) } else { (0, 1) };
                let (first_value, second_value) =
                    if kind == "rs_trigger" { (0, 1) } else { (1, 0) };
                self.flag(inst, first);
                self.choose(
                    Ty::I,
                    |e| e.c_i(first_value),
                    |e| {
                        e.flag(inst, second);
                        e.choose(
                            Ty::I,
                            |e| e.c_i(second_value),
                            |e| {
                                e.out_get(inst, 0);
                            },
                        );
                    },
                );
                Ty::I
            }
            "hysteresis" => {
                let (x, on, off) = (self.tmp_f(), self.tmp_f(), self.tmp_f());
                self.input_as(inst, 0, Ty::F);
                self.op(I::LocalSet(x));
                self.param_as(inst, 0, Ty::F);
                self.op(I::LocalSet(on));
                self.param_as(inst, 1, Ty::F);
                self.op(I::LocalSet(off));
                let get = |e: &mut Self, l: u32| e.op(I::LocalGet(l));
                // Comparison direction depends on which threshold is higher.
                let band = |e: &mut Self, hi_on: bool| {
                    // set when x crosses `on`, reset when it crosses `off`
                    get(e, x);
                    get(e, on);
                    e.op(if hi_on { I::F32Ge } else { I::F32Le });
                    e.choose(
                        Ty::I,
                        |e| e.c_i(1),
                        |e| {
                            get(e, x);
                            get(e, off);
                            e.op(if hi_on { I::F32Le } else { I::F32Ge });
                            e.choose(
                                Ty::I,
                                |e| e.c_i(0),
                                |e| {
                                    e.out_get(inst, 0);
                                },
                            );
                        },
                    );
                };
                get(self, on);
                get(self, off);
                self.op(I::F32Ge);
                self.choose(Ty::I, |e| band(e, true), |e| band(e, false));
                Ty::I
            }
            "compare" => {
                let mode = self.const_param(inst, 0)? as i64;
                let w = self.input_ty(inst, 0).widen(self.input_ty(inst, 1));
                self.input_as(inst, 0, w);
                self.input_as(inst, 1, w);
                self.compare(w, mode);
                Ty::I
            }
            "greater" | "less" => {
                let w = self.input_ty(inst, 0).widen(self.param_ty(inst, 0));
                self.input_as(inst, 0, w);
                self.param_as(inst, 0, w);
                self.compare(w, if kind == "greater" { 4 } else { 2 });
                Ty::I
            }
            "add" | "sub" | "mul" => {
                let w = self.input_ty(inst, 0).widen(self.input_ty(inst, 1));
                self.input_as(inst, 0, w);
                self.input_as(inst, 1, w);
                self.op(match (kind, w) {
                    ("add", Ty::I) => I::I32Add,
                    ("add", Ty::F) => I::F32Add,
                    ("sub", Ty::I) => I::I32Sub,
                    ("sub", Ty::F) => I::F32Sub,
                    (_, Ty::I) => I::I32Mul,
                    (_, Ty::F) => I::F32Mul,
                });
                w
            }
            "min" | "max" => {
                let w = self.input_ty(inst, 0).widen(self.input_ty(inst, 1));
                self.input_as(inst, 0, w);
                self.input_as(inst, 1, w);
                self.pick(w, kind == "max");
                w
            }
            "div" => {
                // Always `float`; a zero divisor gives 0.
                let (a, b) = (self.tmp_f(), self.tmp_f());
                self.input_as(inst, 0, Ty::F);
                self.op(I::LocalSet(a));
                self.input_as(inst, 1, Ty::F);
                self.op(I::LocalSet(b));
                self.op(I::LocalGet(b));
                self.c_f(0.0);
                self.op(I::F32Ne);
                self.choose(
                    Ty::F,
                    |e| {
                        e.op(I::LocalGet(a));
                        e.op(I::LocalGet(b));
                        e.op(I::F32Div);
                    },
                    |e| e.c_f(0.0),
                );
                Ty::F
            }
            "mod" => {
                let w = self.input_ty(inst, 0).widen(self.input_ty(inst, 1));
                let (a, b) = (self.tmp(w), self.tmp(w));
                self.input_as(inst, 0, w);
                self.op(I::LocalSet(a));
                self.input_as(inst, 1, w);
                self.op(I::LocalSet(b));
                self.op(I::LocalGet(b));
                self.flagify(w);
                self.choose(
                    w,
                    |e| {
                        e.op(I::LocalGet(a));
                        if w == Ty::I {
                            e.op(I::LocalGet(b));
                            e.op(I::I32RemS);
                        } else {
                            // fmod through `f64`: a - b * trunc(a / b), with the sign of `a`
                            e.op(I::F64PromoteF32);
                            e.op(I::LocalGet(b));
                            e.op(I::F64PromoteF32);
                            e.op(I::LocalGet(a));
                            e.op(I::F64PromoteF32);
                            e.op(I::LocalGet(b));
                            e.op(I::F64PromoteF32);
                            e.op(I::F64Div);
                            e.op(I::F64Trunc);
                            e.op(I::F64Mul);
                            e.op(I::F64Sub);
                            e.op(I::LocalGet(a));
                            e.op(I::F64PromoteF32);
                            e.op(I::F64Copysign);
                            e.op(I::F32DemoteF64);
                        }
                    },
                    |e| e.c(w, 0.0),
                );
                w
            }
            "limit" => {
                let tx = self.input_ty(inst, 0);
                let w = tx
                    .widen(self.param_ty(inst, 0))
                    .widen(self.param_ty(inst, 1));
                let (lo, hi) = (self.tmp(w), self.tmp(w));
                self.param_as(inst, 0, w);
                self.param_as(inst, 1, w);
                self.pick(w, false);
                self.op(I::LocalSet(lo));
                self.param_as(inst, 0, w);
                self.param_as(inst, 1, w);
                self.pick(w, true);
                self.op(I::LocalSet(hi));
                self.input_as(inst, 0, w);
                self.op(I::LocalGet(lo));
                self.pick(w, true);
                self.op(I::LocalGet(hi));
                self.pick(w, false);
                self.coerce(w, tx);
                tx
            }
            "scale" => {
                let (in_min, in_max, out_min, out_max, span, y) = (
                    self.tmp_f(),
                    self.tmp_f(),
                    self.tmp_f(),
                    self.tmp_f(),
                    self.tmp_f(),
                    self.tmp_f(),
                );
                for (i, l) in [in_min, in_max, out_min, out_max].into_iter().enumerate() {
                    self.param_as(inst, i, Ty::F);
                    self.op(I::LocalSet(l));
                }
                self.op(I::LocalGet(in_max));
                self.op(I::LocalGet(in_min));
                self.op(I::F32Sub);
                self.op(I::LocalSet(span));
                self.op(I::LocalGet(span));
                self.c_f(0.0);
                self.op(I::F32Ne);
                self.choose(
                    Ty::F,
                    |e| {
                        e.op(I::LocalGet(out_min));
                        e.input_as(inst, 0, Ty::F);
                        e.op(I::LocalGet(in_min));
                        e.op(I::F32Sub);
                        e.op(I::LocalGet(out_max));
                        e.op(I::LocalGet(out_min));
                        e.op(I::F32Sub);
                        e.op(I::F32Mul);
                        e.op(I::LocalGet(span));
                        e.op(I::F32Div);
                        e.op(I::F32Add);
                    },
                    |e| e.op(I::LocalGet(out_min)),
                );
                self.op(I::LocalSet(y));
                // Optional clamp to the output range (parameter 4):
                // `y < lo ? lo : y > hi ? hi : y` with lo = min(out_min, out_max), hi = max(..).
                let (lo, hi) = (self.tmp_f(), self.tmp_f());
                for (l, greater) in [(lo, false), (hi, true)] {
                    self.op(I::LocalGet(out_min));
                    self.op(I::LocalGet(out_max));
                    self.pick(Ty::F, greater);
                    self.op(I::LocalSet(l));
                }
                self.op(I::LocalGet(lo));
                self.op(I::LocalGet(hi));
                self.op(I::LocalGet(y));
                self.op(I::LocalGet(y));
                self.op(I::LocalGet(hi));
                self.op(I::F32Gt);
                self.op(I::Select);
                self.op(I::LocalGet(y));
                self.op(I::LocalGet(lo));
                self.op(I::F32Lt);
                self.op(I::Select);
                self.op(I::LocalGet(y));
                self.param_flag(inst, 4);
                self.op(I::Select);
                Ty::F
            }
            "t_trigger" => {
                let prev = self.state_i();
                self.flag(inst, 1);
                self.choose(
                    Ty::I,
                    |e| e.c_i(0),
                    |e| {
                        e.flag(inst, 0);
                        e.op(I::GlobalGet(prev));
                        e.op(I::I32Eqz);
                        e.op(I::I32And);
                        e.choose(
                            Ty::I,
                            |e| {
                                e.out_get(inst, 0);
                                e.op(I::I32Eqz);
                            },
                            |e| {
                                e.out_get(inst, 0);
                            },
                        );
                    },
                );
                self.flag(inst, 0);
                self.op(I::GlobalSet(prev));
                Ty::I
            }
            "rising_edge" | "falling_edge" => {
                let prev = self.state_i();
                let n = self.tmp_i();
                self.flag(inst, 0);
                self.op(I::LocalSet(n));
                if kind == "rising_edge" {
                    self.op(I::LocalGet(n));
                    self.op(I::GlobalGet(prev));
                    self.op(I::I32Eqz);
                } else {
                    self.op(I::LocalGet(n));
                    self.op(I::I32Eqz);
                    self.op(I::GlobalGet(prev));
                }
                self.op(I::I32And);
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(prev));
                Ty::I
            }
            "timer_on" => {
                let (running, start) = (self.state_i(), self.state_i());
                self.flag(inst, 0);
                self.choose(
                    Ty::I,
                    |e| {
                        e.op(I::GlobalGet(running));
                        e.op(I::I32Eqz);
                        e.op(I::If(BlockType::Empty));
                        e.c_i(1);
                        e.op(I::GlobalSet(running));
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(start));
                        e.op(I::End);
                        e.since(start);
                        e.param_ms(inst, 0);
                        e.op(I::I32GeU);
                    },
                    |e| {
                        e.c_i(0);
                        e.op(I::GlobalSet(running));
                        e.c_i(0);
                    },
                );
                Ty::I
            }
            "timer_off" => {
                let (on, off_at) = (self.state_i(), self.state_i());
                self.flag(inst, 0);
                self.choose(
                    Ty::I,
                    |e| {
                        e.c_i(1);
                        e.op(I::GlobalSet(on));
                        e.c_i(1);
                    },
                    |e| {
                        e.op(I::GlobalGet(on));
                        e.op(I::If(BlockType::Empty));
                        e.c_i(0);
                        e.op(I::GlobalSet(on));
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(off_at));
                        e.op(I::End);
                        e.out_get(inst, 0);
                        e.c_i(0);
                        e.op(I::I32Ne);
                        e.since(off_at);
                        e.param_ms(inst, 0);
                        e.op(I::I32LtU);
                        e.op(I::I32And);
                    },
                );
                Ty::I
            }
            "pulse" => {
                let (prev, active, start) = (self.state_i(), self.state_i(), self.state_i());
                let n = self.tmp_i();
                self.flag(inst, 0);
                self.op(I::LocalSet(n));
                self.op(I::LocalGet(n));
                self.op(I::GlobalGet(prev));
                self.op(I::I32Eqz);
                self.op(I::I32And);
                self.op(I::GlobalGet(active));
                self.op(I::I32Eqz);
                self.op(I::I32And);
                self.op(I::If(BlockType::Empty));
                self.c_i(1);
                self.op(I::GlobalSet(active));
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalSet(start));
                self.op(I::End);
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(prev));
                self.op(I::GlobalGet(active));
                self.since(start);
                self.param_ms(inst, 0);
                self.op(I::I32GeU);
                self.op(I::I32And);
                self.op(I::If(BlockType::Empty));
                self.c_i(0);
                self.op(I::GlobalSet(active));
                self.op(I::End);
                self.op(I::GlobalGet(active));
                Ty::I
            }
            "generator" => {
                let start = self.state_i();
                let (t_on, period) = (self.tmp_i(), self.tmp_i());
                self.flag(inst, 0);
                self.choose(
                    Ty::I,
                    |e| {
                        e.param_ms(inst, 0);
                        e.op(I::LocalSet(t_on));
                        // period = max(on + off, 1)
                        e.op(I::LocalGet(t_on));
                        e.param_ms(inst, 1);
                        e.op(I::I32Add);
                        e.op(I::LocalSet(period));
                        e.c_i(1);
                        e.op(I::LocalGet(period));
                        e.op(I::LocalGet(period));
                        e.op(I::I32Eqz);
                        e.op(I::Select);
                        e.op(I::LocalSet(period));
                        e.since(start);
                        e.op(I::LocalGet(period));
                        e.op(I::I32RemU);
                        e.op(I::LocalGet(t_on));
                        e.op(I::I32LtU);
                    },
                    |e| {
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(start));
                        e.c_i(0);
                    },
                );
                Ty::I
            }
            "debounce" => {
                let (last, changed) = (self.state_i(), self.state_i());
                let n = self.tmp_i();
                self.flag(inst, 0);
                self.op(I::LocalSet(n));
                self.op(I::LocalGet(n));
                self.op(I::GlobalGet(last));
                self.op(I::I32Ne);
                self.op(I::If(BlockType::Empty));
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(last));
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalSet(changed));
                self.op(I::End);
                self.since(changed);
                self.param_ms(inst, 0);
                self.op(I::I32GeU);
                self.choose(
                    Ty::I,
                    |e| e.op(I::GlobalGet(last)),
                    |e| {
                        e.out_get(inst, 0);
                    },
                );
                Ty::I
            }
            "counter" => {
                let (up, down) = (self.state_i(), self.state_i());
                self.flag(inst, 2);
                self.choose(
                    Ty::I,
                    |e| e.c_i(0),
                    |e| {
                        e.out_get(inst, 0);
                        e.flag(inst, 0);
                        e.op(I::GlobalGet(up));
                        e.op(I::I32Eqz);
                        e.op(I::I32And);
                        e.op(I::I32Add);
                        e.flag(inst, 1);
                        e.op(I::GlobalGet(down));
                        e.op(I::I32Eqz);
                        e.op(I::I32And);
                        e.op(I::I32Sub);
                    },
                );
                self.flag(inst, 0);
                self.op(I::GlobalSet(up));
                self.flag(inst, 1);
                self.op(I::GlobalSet(down));
                Ty::I
            }
            "constant" => self.param(inst, 0),
            "switch" => {
                let w = self.input_ty(inst, 1).widen(self.input_ty(inst, 2));
                self.input_as(inst, 2, w);
                self.input_as(inst, 1, w);
                self.flag(inst, 0);
                self.op(I::Select);
                w
            }
            "mux" => {
                let count = inst
                    .lib
                    .def
                    .inputs
                    .iter()
                    .skip(1)
                    .filter(|p| inst.inputs.contains_key(p.name.as_str()))
                    .count()
                    .max(1);
                let w = (1..=count).fold(Ty::I, |t, j| t.widen(self.input_ty(inst, j)));
                let s = self.tmp_i();
                let r = self.tmp(w);
                self.input_as(inst, 0, Ty::I);
                self.c_i(0);
                self.pick(Ty::I, true);
                self.op(I::LocalSet(s));
                self.input_as(inst, count, w);
                self.op(I::LocalSet(r));
                for j in (0..count - 1).rev() {
                    self.input_as(inst, 1 + j, w);
                    self.op(I::LocalGet(r));
                    self.op(I::LocalGet(s));
                    self.c_i((j + 1) as i32);
                    self.op(I::I32LtS);
                    self.op(I::Select);
                    self.op(I::LocalSet(r));
                }
                self.op(I::LocalGet(r));
                w
            }
            "var_get" => {
                let name = inst.block.var_name().unwrap_or_default().to_owned();
                self.var_read(&name)
            }
            "var_set" => {
                let name = inst.block.var_name().unwrap_or_default().to_owned();
                let enable = inst
                    .lib
                    .def
                    .inputs
                    .get(1)
                    .is_some_and(|p| inst.inputs.contains_key(p.name.as_str()));
                if enable {
                    self.flag(inst, 1);
                    self.op(I::If(BlockType::Empty));
                }
                let from = self.input(inst, 0);
                self.var_write(&name, from);
                if enable {
                    self.op(I::End);
                }
                return Ok(());
            }
            _ => {
                let reason = if inst.lib.def.info.kind.is_empty() {
                    "блок пока нельзя выразить в модуле"
                } else {
                    "блок пока не поддержан в модулях"
                };
                return Err(self.unsupported(inst, reason));
            }
        };
        if inst.lib.def.outputs.is_empty() {
            self.op(I::Drop);
        } else {
            self.set_out(inst, 0, ty);
        }
        Ok(())
    }

    fn finish(self) -> WasmModule {
        let mut types = TypeSection::new();
        types.ty().function([ValType::I32], [ValType::I32]); // 0: tag_get_i, gpio_read, adc_read
        types.ty().function([ValType::I32], [ValType::F32]); // 1: tag_get_f
        types.ty().function([ValType::I32, ValType::I32], []); // 2: tag_set_i, output_i, gpio_write, pwm_write
        types.ty().function([ValType::I32, ValType::F32], []); // 3: tag_set_f, output_f
        types.ty().function([], [ValType::I32]); // 4: abi_version
        types.ty().function([ValType::I32], []); // 5: step
        types.ty().function([], []); // 6: init
        types
            .ty()
            .function([ValType::I32, ValType::I32], [ValType::I32]); // 7: output_i
        types
            .ty()
            .function([ValType::I32, ValType::F32], [ValType::F32]); // 8: output_f
        types
            .ty()
            .function([ValType::I32, ValType::I32, ValType::I32], [ValType::F32]); // 9: sensor_read
        types
            .ty()
            .function([ValType::I32, ValType::I32, ValType::I32], [ValType::I32]); // 10: pcf_read
        types.ty().function([ValType::I32; 5], [ValType::I32]); // 11: modbus
        types.ty().function([ValType::I32; 4], [ValType::I32]); // 12: uart
        types.ty().function([ValType::I32; 7], [ValType::I32]); // 13: text

        let mut imports = ImportSection::new();
        imports.import("crab", "tag_get_i", EntityType::Function(0));
        imports.import("crab", "tag_get_f", EntityType::Function(1));
        imports.import("crab", "tag_set_i", EntityType::Function(2));
        imports.import("crab", "tag_set_f", EntityType::Function(3));
        imports.import("crab", "output_i", EntityType::Function(7));
        imports.import("crab", "output_f", EntityType::Function(8));
        // Hardware imports follow in placeholder order; defined functions come after them.
        let used: Vec<u32> = self.hw.iter().copied().collect();
        let mut allow = Vec::new();
        for &h in &used {
            let name = HW_NAMES[(h - HW_BASE) as usize];
            let ty = match h {
                HW_GPIO_READ | HW_ADC_READ => 0,
                HW_SENSOR_READ => 9,
                HW_PCF_READ => 10,
                HW_PCF_WRITE => 7,
                HW_MODBUS | HW_NET => 11,
                HW_UART | HW_NATIVE => 12,
                HW_TEXT => 13,
                _ => 2,
            };
            imports.import("crab", name, EntityType::Function(ty));
            allow.push(format!("\"{name}\""));
        }
        let first = FIXED_IMPORTS + used.len() as u32;
        let remap = |n: u32| match used.iter().position(|&h| h == n) {
            Some(k) => FIXED_IMPORTS + k as u32,
            None => n,
        };
        let peak_i = self.peak_i;
        let local = |n: u32| {
            if n >= F_LOCAL_BASE {
                1 + peak_i + (n - F_LOCAL_BASE)
            } else {
                n
            }
        };

        let mut funcs = FunctionSection::new();
        funcs.function(4);
        funcs.function(5);
        funcs.function(6);
        for _ in &self.cuts {
            funcs.function(5);
        }
        let groups = group_count(self.cuts.len());
        for _ in 0..groups {
            funcs.function(5);
        }

        let mut memory = MemorySection::new();
        memory.memory(MemoryType {
            // A page (64 KB) is taken from the board's heap, so only modules with strings ask for it.
            minimum: u64::from(self.mem_top > MEM_START || self.hw.contains(&HW_TEXT)),
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut globals = GlobalSection::new();
        for g in &self.globals {
            let init = match g.ty {
                Ty::I => ConstExpr::i32_const(g.bits as i32),
                Ty::F => ConstExpr::f32_const(f32::from_bits(g.bits).into()),
            };
            globals.global(
                GlobalType {
                    val_type: g.ty.val(),
                    mutable: true,
                    shared: false,
                },
                &init,
            );
        }

        let mut exports = ExportSection::new();
        exports.export("memory", ExportKind::Memory, 0);
        exports.export("abi_version", ExportKind::Func, first);
        exports.export("step", ExportKind::Func, first + 1);
        exports.export("init", ExportKind::Func, first + 2);

        let mut abi = Function::new([]);
        abi.instruction(&I::I32Const(3)).instruction(&I::End);

        // `step` runs the first piece and calls the rest, each a function of its own with the same locals.
        let mut locals = vec![ValType::I32; self.peak_i as usize];
        locals.extend(vec![ValType::F32; self.peak_f as usize]);
        let mut bounds = vec![0];
        bounds.extend(self.cuts.iter().copied());
        bounds.push(self.code.len());
        let mut pieces: Vec<Function> = Vec::new();
        for w in bounds.windows(2) {
            let mut f = Function::new_with_locals_types(locals.clone());
            for i in &self.code[w[0]..w[1]] {
                match i {
                    I::Call(n) if *n >= HW_BASE => f.instruction(&I::Call(remap(*n))),
                    I::LocalGet(n) => f.instruction(&I::LocalGet(local(*n))),
                    I::LocalSet(n) => f.instruction(&I::LocalSet(local(*n))),
                    other => f.instruction(other),
                };
            }
            pieces.push(f);
        }
        let mut step = pieces.remove(0);
        // The interpreter does not tail-call between wasm functions: every call in a row takes
        // C stack on the board. Many pieces are therefore called through groups of `PIECES_PER_GROUP`.
        let rest = pieces.len() as u32;
        let mut group_fns: Vec<Function> = Vec::new();
        if groups == 0 {
            for k in 0..rest {
                step.instruction(&I::LocalGet(0));
                step.instruction(&I::Call(first + 3 + k));
            }
        } else {
            for g in 0..groups as u32 {
                let mut f = Function::new([]);
                let from = g * PIECES_PER_GROUP;
                for k in from..rest.min(from + PIECES_PER_GROUP) {
                    f.instruction(&I::LocalGet(0));
                    f.instruction(&I::Call(first + 3 + k));
                }
                f.instruction(&I::End);
                group_fns.push(f);
                step.instruction(&I::LocalGet(0));
                step.instruction(&I::Call(first + 3 + rest + g));
            }
        }
        step.instruction(&I::End);
        for p in &mut pieces {
            p.instruction(&I::End);
        }

        // The owner of a tag writes its initial value once.
        let mut init = Function::new([]);
        for t in &self.module.tags {
            if let Some(value) = &t.init {
                init.instruction(&I::I32Const(t.tag.number as i32));
                let ty = Ty::of(t.tag.ty);
                match ty {
                    Ty::I => init.instruction(&I::I32Const(pv(value) as i32)),
                    Ty::F => init.instruction(&I::F32Const((pv(value) as f32).into())),
                };
                init.instruction(&I::Call(if ty == Ty::F { TAG_SET_F } else { TAG_SET_I }));
            }
        }
        init.instruction(&I::End);

        let mut code = CodeSection::new();
        code.function(&abi);
        code.function(&step);
        code.function(&init);
        for p in &pieces {
            code.function(p);
        }
        for g in &group_fns {
            code.function(g);
        }

        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&funcs)
            .section(&memory)
            .section(&globals)
            .section(&exports)
            .section(&code);
        if !self.data.is_empty() {
            let mut data = wasm_encoder::DataSection::new();
            for (at, bytes) in &self.data {
                data.active(0, &ConstExpr::i32_const(*at as i32), bytes.iter().copied());
            }
            module.section(&data);
        }

        let mut manifest = format!(
            "name = \"{}\"\nabi = 3\nallow = [{}]\n",
            self.module.name,
            allow.join(", ")
        );
        for t in &self.module.tags {
            let name = match &t.tag.kind {
                TagKind::Var(n) | TagKind::Signal(n) => n.replace('"', "'"),
            };
            manifest.push_str(&format!(
                "\n[[tag]]\nnumber = {}\nname = \"{}\"\ntype = \"{}\"\n",
                t.tag.number,
                name,
                format!("{:?}", t.tag.ty).to_lowercase()
            ));
        }
        WasmModule {
            name: self.module.name.clone(),
            wasm: module.finish(),
            manifest,
            watched: self.watched,
        }
    }
}
