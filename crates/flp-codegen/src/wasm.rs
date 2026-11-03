//! A sheet of the schema as an ABI v3 WASM module, written straight with `wasm-encoder`.
//!
//! Values are 32 bit, as in the numeric model (`specs/numeric-model`): `bool` and `int` are `i32`
//! (wrapping), `float` is `f32`, `now` is an `i32` of milliseconds that wraps; so a module gives
//! the same bits as `flp-sim`. Block outputs are mutable globals that keep their value between
//! steps; values that cross the sheet border go through `tag_get_*` / `tag_set_*`.

use std::collections::HashMap;

use flp_core::value::parse_duration_ms;
use flp_core::{ParamKind, ParamValue, SignalType};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};

use crate::program::{Instance, Program, Source};
use crate::sheet::{signal_label, SheetError, SheetModule, TagKind};

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
const HW_NAMES: [&str; 4] = ["gpio_read", "gpio_write", "adc_read", "pwm_write"];
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
        Ok(em.finish())
    }
}

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
}

/// Blocks per generated function. wasm3 without tail-call optimization (ESP32 build) recurses once
/// per operation of a function, so a long function overflows the task stack; short ones do not.
const BLOCKS_PER_CHUNK: usize = 8;

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
        };
        for &bi in &module.blocks {
            for &signal in program.blocks[bi].outputs.values() {
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
        if self.watch {
            let k = self
                .watched
                .iter()
                .position(|l| *l == label)
                .unwrap_or_else(|| {
                    self.watched.push(label.clone());
                    self.watched.len() - 1
                });
            self.c_i(k as i32);
            self.op(I::GlobalGet(g));
            self.op(I::Call(if ty == Ty::F { OUTPUT_F } else { OUTPUT_I }));
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

    /// Digital/analog/PWM pins of the board: calls into the master.
    fn hardware_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
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

        let mut imports = ImportSection::new();
        imports.import("flp", "tag_get_i", EntityType::Function(0));
        imports.import("flp", "tag_get_f", EntityType::Function(1));
        imports.import("flp", "tag_set_i", EntityType::Function(2));
        imports.import("flp", "tag_set_f", EntityType::Function(3));
        imports.import("flp", "output_i", EntityType::Function(2));
        imports.import("flp", "output_f", EntityType::Function(3));
        // Hardware imports follow in placeholder order; defined functions come after them.
        let used: Vec<u32> = self.hw.iter().copied().collect();
        let mut allow = Vec::new();
        for &h in &used {
            let name = HW_NAMES[(h - HW_BASE) as usize];
            let ty = if h == HW_GPIO_READ || h == HW_ADC_READ {
                0
            } else {
                2
            };
            imports.import("flp", name, EntityType::Function(ty));
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

        let mut memory = MemorySection::new();
        memory.memory(MemoryType {
            // No strings or messages in sheet modules: a page (64 KB) would be taken from the board's heap.
            minimum: 0,
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
        for k in 0..pieces.len() as u32 {
            step.instruction(&I::LocalGet(0));
            step.instruction(&I::Call(first + 3 + k));
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

        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&funcs)
            .section(&memory)
            .section(&globals)
            .section(&exports)
            .section(&code);

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
