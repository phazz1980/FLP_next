//! A sheet of the schema as an ABI v2 WASM module, written straight with `wasm-encoder`.
//!
//! Every value is an `f64` (bool 0/1, int whole), the same as in `flp-blocks`, so a module
//! gives the same numbers as `flp-sim`. Block outputs are mutable globals that keep their
//! value between steps; values that cross the sheet border go through `tag_get` / `tag_set`.

use std::collections::HashMap;

use flp_core::value::parse_duration_ms;
use flp_core::{ParamKind, ParamValue};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction as I, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};

use crate::program::{Instance, Program, Source};
use crate::sheet::{signal_label, SheetError, SheetModule, TagKind};

const TAG_GET: u32 = 0;
const TAG_SET: u32 = 1;
const OUTPUT: u32 = 2;
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

    /// With `watch` the module reports every block output through `output(i, v)`:
    /// for tests and the simulator, not for the board.
    pub fn wasm_module_with(&self, sheet: u32, watch: bool) -> Result<WasmModule, WasmError> {
        let module = self.sheet_module(sheet)?;
        let mut em = Emitter::new(self, &module);
        em.watch = watch;
        em.step_body()?;
        Ok(em.finish())
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

struct Emitter<'a, 'p> {
    program: &'a Program<'p>,
    module: &'a SheetModule,
    table: Vec<crate::sheet::Tag>,
    code: Vec<I<'static>>,
    /// Initial values of the globals; index 0 is `now`.
    globals: Vec<f64>,
    signals: HashMap<usize, u32>,
    vars: HashMap<String, u32>,
    /// Number of `f64` scratch locals (local 0 is `dt`).
    scratch: u32,
    /// Most scratch locals any block needed at once; blocks reuse the same locals.
    scratch_peak: u32,
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
            globals: vec![0.0],
            signals: HashMap::new(),
            vars: HashMap::new(),
            scratch: 0,
            scratch_peak: 0,
            watch: false,
            watched: Vec::new(),
            hw: Default::default(),
            cuts: Vec::new(),
        };
        for &bi in &module.blocks {
            for &signal in program.blocks[bi].outputs.values() {
                let g = em.global(0.0);
                em.signals.insert(signal, g);
            }
        }
        for local in &module.locals {
            let g = em.global(pv(&local.init));
            em.vars.insert(local.name.clone(), g);
        }
        em
    }

    fn global(&mut self, init: f64) -> u32 {
        self.globals.push(init);
        (self.globals.len() - 1) as u32
    }

    fn tmp(&mut self) -> u32 {
        self.scratch += 1;
        self.scratch_peak = self.scratch_peak.max(self.scratch);
        self.scratch
    }

    fn op(&mut self, i: I<'static>) {
        self.code.push(i);
    }

    fn c(&mut self, x: f64) {
        self.op(I::F64Const(x.into()));
    }

    /// `f64` on the stack → `1.0` if it was `0.0`, else `0.0`.
    fn not(&mut self) {
        self.c(0.0);
        self.op(I::F64Eq);
        self.op(I::F64ConvertI32U);
    }

    fn unsupported(&self, inst: &Instance, reason: &str) -> WasmError {
        WasmError::Unsupported {
            block: inst.block.id.clone(),
            kind: inst.block.kind.clone(),
            reason: reason.to_owned(),
        }
    }

    fn tag_of(&self, kind: &TagKind) -> Option<u32> {
        self.table
            .iter()
            .find(|t| &t.kind == kind)
            .map(|t| t.number)
    }

    fn var_read(&mut self, name: &str) {
        if let Some(n) = self.tag_of(&TagKind::Var(name.to_owned())) {
            self.op(I::I32Const(n as i32));
            self.op(I::Call(TAG_GET));
        } else if let Some(&g) = self.vars.get(name) {
            self.op(I::GlobalGet(g));
        } else {
            self.c(0.0);
        }
    }

    /// Writes the `f64` on the stack into a variable.
    fn var_write(&mut self, name: &str) {
        if let Some(n) = self.tag_of(&TagKind::Var(name.to_owned())) {
            let t = self.tmp();
            self.op(I::LocalSet(t));
            self.op(I::I32Const(n as i32));
            self.op(I::LocalGet(t));
            self.op(I::Call(TAG_SET));
        } else if let Some(&g) = self.vars.get(name) {
            self.op(I::GlobalSet(g));
        } else {
            self.op(I::Drop);
        }
    }

    fn operand(&mut self, inst: &Instance, port: &str) -> Result<(), WasmError> {
        let input = &inst.inputs[port];
        match &input.source {
            Source::Default(v) => self.c(pv(v)),
            Source::Var(name) => self.var_read(name),
            Source::Signal { index, invert } => {
                if let Some(&g) = self.signals.get(index) {
                    self.op(I::GlobalGet(g));
                } else {
                    let label = signal_label(self.program, *index);
                    let n = self
                        .tag_of(&TagKind::Signal(label))
                        .expect("cross-sheet link has a tag");
                    self.op(I::I32Const(n as i32));
                    self.op(I::Call(TAG_GET));
                }
                if *invert {
                    self.not();
                }
            }
        }
        if inst.block.inverted_inputs.contains(port) {
            self.not();
        }
        Ok(())
    }

    /// Input number `i` in `block.toml` order; zero if the port is off.
    fn input(&mut self, inst: &Instance, i: usize) -> Result<(), WasmError> {
        match inst.lib.def.inputs.get(i) {
            Some(p) if inst.inputs.contains_key(p.name.as_str()) => self.operand(inst, &p.name),
            _ => {
                self.c(0.0);
                Ok(())
            }
        }
    }

    /// Input `i` as an `i32` flag.
    fn flag(&mut self, inst: &Instance, i: usize) -> Result<(), WasmError> {
        self.input(inst, i)?;
        self.c(0.0);
        self.op(I::F64Ne);
        Ok(())
    }

    fn param(&mut self, inst: &Instance, i: usize) {
        let Some(p) = inst.lib.def.params.get(i) else {
            return self.c(0.0);
        };
        if let Some(var) = inst.param_vars.get(p.name.as_str()) {
            self.var_read(var);
        } else {
            self.c(inst
                .params
                .get(p.name.as_str())
                .map_or(0.0, |v| num(v, p.kind)));
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
    fn out_get(&mut self, inst: &Instance, i: usize) {
        let name = inst.lib.def.outputs[i].name.as_str();
        let g = self.signals[&inst.outputs[name]];
        self.op(I::GlobalGet(g));
    }

    /// Stores the `f64` on the stack as output `i`: global, tag for other sheets, bound variable.
    fn set_out(&mut self, inst: &Instance, i: usize) {
        let name = inst.lib.def.outputs[i].name.as_str();
        let signal = inst.outputs[name];
        let g = self.signals[&signal];
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
            self.op(I::I32Const(k as i32));
            self.op(I::GlobalGet(g));
            self.op(I::Call(OUTPUT));
        }
        if let Some(n) = self.tag_of(&TagKind::Signal(label)) {
            self.op(I::I32Const(n as i32));
            self.op(I::GlobalGet(g));
            self.op(I::Call(TAG_SET));
        }
        if let Some(var) = inst.output_vars.get(name) {
            self.op(I::GlobalGet(g));
            self.var_write(var);
        }
    }

    /// Live (active) inputs combined with `join`; `empty` when there are none.
    fn fold_flags(
        &mut self,
        inst: &Instance,
        join: I<'static>,
        empty: i32,
    ) -> Result<(), WasmError> {
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
            self.op(I::I32Const(empty));
        }
        for (k, &i) in live.iter().enumerate() {
            self.flag(inst, i)?;
            if k > 0 {
                self.op(join.clone());
            }
        }
        self.op(I::F64ConvertI32U);
        Ok(())
    }

    /// `if cond { then } else { otherwise }` on `f64`; the condition (`i32`) is on the stack.
    fn choose(
        &mut self,
        then: impl FnOnce(&mut Self) -> Result<(), WasmError>,
        otherwise: impl FnOnce(&mut Self) -> Result<(), WasmError>,
    ) -> Result<(), WasmError> {
        self.op(I::If(BlockType::Result(ValType::F64)));
        then(self)?;
        self.op(I::Else);
        otherwise(self)?;
        self.op(I::End);
        Ok(())
    }

    fn step_body(&mut self) -> Result<(), WasmError> {
        self.op(I::GlobalGet(GLOBAL_NOW));
        self.op(I::LocalGet(0));
        self.op(I::F64ConvertI32S);
        self.op(I::F64Add);
        self.op(I::GlobalSet(GLOBAL_NOW));
        let blocks = self.module.blocks.clone();
        for (k, bi) in blocks.into_iter().enumerate() {
            if k > 0 && k % BLOCKS_PER_CHUNK == 0 {
                self.cuts.push(self.code.len());
            }
            // A block's temporaries never outlive it: state lives in globals.
            self.scratch = 0;
            let program = self.program;
            self.block(&program.blocks[bi])?;
        }
        Ok(())
    }

    fn hw_call(&mut self, import: u32) {
        self.hw.insert(import);
        self.op(I::Call(import));
    }

    /// Writes an output block: `level` (an `f64` global) goes to the pin when `en` allows.
    fn hold_unless_enabled(
        &mut self,
        inst: &Instance,
        store: impl FnOnce(&mut Self) -> Result<(), WasmError>,
    ) -> Result<(), WasmError> {
        let gated = inst.inputs.contains_key("en");
        if gated {
            self.flag(inst, 1)?;
            self.op(I::If(BlockType::Empty));
        }
        store(self)?;
        if gated {
            self.op(I::End);
        }
        Ok(())
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
                self.op(I::I32Const(pin | if pullup { PIN_PULLUP } else { 0 }));
                self.hw_call(HW_GPIO_READ);
                self.op(I::I32Const(0));
                self.op(I::I32Ne);
                if inverted {
                    self.op(I::I32Eqz);
                }
                self.op(I::F64ConvertI32U);
            }
            "analog_in" => {
                self.op(I::I32Const(pin));
                self.hw_call(HW_ADC_READ);
                self.op(I::F64ConvertI32S);
                self.c(0.0);
                self.op(I::F64Max);
                self.c(4095.0);
                self.op(I::F64Min);
            }
            "digital_out" => {
                let inverted = self.const_param(inst, 1)? != 0.0;
                let logic = self.global(0.0);
                self.hold_unless_enabled(inst, |e| {
                    e.flag(inst, 0)?;
                    e.op(I::F64ConvertI32U);
                    e.op(I::GlobalSet(logic));
                    Ok(())
                })?;
                self.op(I::I32Const(pin));
                self.op(I::GlobalGet(logic));
                self.c(0.0);
                self.op(I::F64Ne);
                if inverted {
                    self.op(I::I32Eqz);
                }
                self.hw_call(HW_GPIO_WRITE);
                return Ok(true);
            }
            _ => {
                let duty = self.global(0.0);
                self.hold_unless_enabled(inst, |e| {
                    e.input(inst, 0)?;
                    e.c(0.0);
                    e.op(I::F64Max);
                    e.c(100.0);
                    e.op(I::F64Min);
                    e.op(I::GlobalSet(duty));
                    Ok(())
                })?;
                // The master passes the value to `analogWrite`: 8 bits, 0..255.
                self.op(I::I32Const(pin));
                self.op(I::GlobalGet(duty));
                self.c(2.55);
                self.op(I::F64Mul);
                self.op(I::F64Nearest);
                self.op(I::I32TruncF64U);
                self.hw_call(HW_PWM_WRITE);
                return Ok(true);
            }
        }
        self.set_out(inst, 0);
        Ok(true)
    }

    fn block(&mut self, inst: &Instance) -> Result<(), WasmError> {
        if self.hardware_block(inst)? {
            return Ok(());
        }
        let kind = inst.block.kind.as_str();
        match kind {
            "and" => self.fold_flags(inst, I::I32And, 1)?,
            "or" => self.fold_flags(inst, I::I32Or, 0)?,
            "xor" => self.fold_flags(inst, I::I32Xor, 0)?,
            "not" => {
                self.flag(inst, 0)?;
                self.op(I::I32Eqz);
                self.op(I::F64ConvertI32U);
            }
            "rs_trigger" | "sr_trigger" => {
                // rs: reset wins; sr: set wins. The output keeps its value otherwise.
                let (first, second) = if kind == "rs_trigger" { (1, 0) } else { (0, 1) };
                let (first_value, second_value) = if kind == "rs_trigger" {
                    (0.0, 1.0)
                } else {
                    (1.0, 0.0)
                };
                self.flag(inst, first)?;
                self.choose(
                    |e| {
                        e.c(first_value);
                        Ok(())
                    },
                    |e| {
                        e.flag(inst, second)?;
                        e.choose(
                            |e| {
                                e.c(second_value);
                                Ok(())
                            },
                            |e| {
                                e.out_get(inst, 0);
                                Ok(())
                            },
                        )
                    },
                )?;
            }
            "hysteresis" => {
                let (x, on, off) = (self.tmp(), self.tmp(), self.tmp());
                self.input(inst, 0)?;
                self.op(I::LocalSet(x));
                self.param(inst, 0);
                self.op(I::LocalSet(on));
                self.param(inst, 1);
                self.op(I::LocalSet(off));
                let get = |e: &mut Self, l: u32| e.op(I::LocalGet(l));
                // Comparison direction depends on which threshold is higher.
                let band = |e: &mut Self, hi_on: bool| -> Result<(), WasmError> {
                    // set when x crosses `on`, reset when it crosses `off`
                    get(e, x);
                    get(e, on);
                    e.op(if hi_on { I::F64Ge } else { I::F64Le });
                    e.choose(
                        |e| {
                            e.c(1.0);
                            Ok(())
                        },
                        |e| {
                            get(e, x);
                            get(e, off);
                            e.op(if hi_on { I::F64Le } else { I::F64Ge });
                            e.choose(
                                |e| {
                                    e.c(0.0);
                                    Ok(())
                                },
                                |e| {
                                    e.out_get(inst, 0);
                                    Ok(())
                                },
                            )
                        },
                    )
                };
                get(self, on);
                get(self, off);
                self.op(I::F64Ge);
                self.choose(|e| band(e, true), |e| band(e, false))?;
            }
            "compare" => {
                let mode = self.const_param(inst, 0)? as i64;
                self.input(inst, 0)?;
                self.input(inst, 1)?;
                self.op(match mode {
                    0 => I::F64Eq,
                    1 => I::F64Ne,
                    2 => I::F64Lt,
                    3 => I::F64Le,
                    4 => I::F64Gt,
                    _ => I::F64Ge,
                });
                self.op(I::F64ConvertI32U);
            }
            "greater" | "less" => {
                self.input(inst, 0)?;
                self.param(inst, 0);
                self.op(if kind == "greater" {
                    I::F64Gt
                } else {
                    I::F64Lt
                });
                self.op(I::F64ConvertI32U);
            }
            "add" | "sub" | "mul" | "min" | "max" => {
                self.input(inst, 0)?;
                self.input(inst, 1)?;
                self.op(match kind {
                    "add" => I::F64Add,
                    "sub" => I::F64Sub,
                    "mul" => I::F64Mul,
                    "min" => I::F64Min,
                    _ => I::F64Max,
                });
            }
            "div" | "mod" => {
                let (a, b) = (self.tmp(), self.tmp());
                self.input(inst, 0)?;
                self.op(I::LocalSet(a));
                self.input(inst, 1)?;
                self.op(I::LocalSet(b));
                self.op(I::LocalGet(b));
                self.c(0.0);
                self.op(I::F64Ne);
                self.choose(
                    |e| {
                        e.op(I::LocalGet(a));
                        if kind == "div" {
                            e.op(I::LocalGet(b));
                            e.op(I::F64Div);
                        } else {
                            // a % b = a - b * trunc(a / b), as the C remainder
                            e.op(I::LocalGet(b));
                            e.op(I::LocalGet(a));
                            e.op(I::LocalGet(b));
                            e.op(I::F64Div);
                            e.op(I::F64Trunc);
                            e.op(I::F64Mul);
                            e.op(I::F64Sub);
                        }
                        Ok(())
                    },
                    |e| {
                        e.c(0.0);
                        Ok(())
                    },
                )?;
            }
            "limit" => {
                let (lo, hi) = (self.tmp(), self.tmp());
                self.param(inst, 0);
                self.param(inst, 1);
                self.op(I::F64Min);
                self.op(I::LocalSet(lo));
                self.param(inst, 0);
                self.param(inst, 1);
                self.op(I::F64Max);
                self.op(I::LocalSet(hi));
                self.input(inst, 0)?;
                self.op(I::LocalGet(hi));
                self.op(I::F64Min);
                self.op(I::LocalGet(lo));
                self.op(I::F64Max);
            }
            "scale" => {
                let (in_min, in_max, out_min, out_max, span) =
                    (self.tmp(), self.tmp(), self.tmp(), self.tmp(), self.tmp());
                for (i, l) in [in_min, in_max, out_min, out_max].into_iter().enumerate() {
                    self.param(inst, i);
                    self.op(I::LocalSet(l));
                }
                self.op(I::LocalGet(in_max));
                self.op(I::LocalGet(in_min));
                self.op(I::F64Sub);
                self.op(I::LocalSet(span));
                self.op(I::LocalGet(span));
                self.c(0.0);
                self.op(I::F64Ne);
                self.choose(
                    |e| {
                        e.op(I::LocalGet(out_min));
                        e.input(inst, 0)?;
                        e.op(I::LocalGet(in_min));
                        e.op(I::F64Sub);
                        e.op(I::LocalGet(out_max));
                        e.op(I::LocalGet(out_min));
                        e.op(I::F64Sub);
                        e.op(I::F64Mul);
                        e.op(I::LocalGet(span));
                        e.op(I::F64Div);
                        e.op(I::F64Add);
                        Ok(())
                    },
                    |e| {
                        e.op(I::LocalGet(out_min));
                        Ok(())
                    },
                )?;
                // Optional clamp to the output range (parameter 4).
                let y = self.tmp();
                self.op(I::LocalSet(y));
                self.op(I::LocalGet(y));
                self.op(I::LocalGet(out_max));
                self.op(I::LocalGet(out_min));
                self.op(I::F64Max);
                self.op(I::F64Min);
                self.op(I::LocalGet(out_max));
                self.op(I::LocalGet(out_min));
                self.op(I::F64Min);
                self.op(I::F64Max);
                self.op(I::LocalGet(y));
                self.param(inst, 4);
                self.c(0.0);
                self.op(I::F64Ne);
                self.op(I::Select);
            }
            "t_trigger" => {
                let prev = self.global(0.0);
                self.flag(inst, 1)?;
                self.choose(
                    |e| {
                        e.c(0.0);
                        Ok(())
                    },
                    |e| {
                        e.flag(inst, 0)?;
                        e.op(I::GlobalGet(prev));
                        e.c(0.0);
                        e.op(I::F64Eq);
                        e.op(I::I32And);
                        e.choose(
                            |e| {
                                e.out_get(inst, 0);
                                e.not();
                                Ok(())
                            },
                            |e| {
                                e.out_get(inst, 0);
                                Ok(())
                            },
                        )
                    },
                )?;
                self.flag(inst, 0)?;
                self.op(I::F64ConvertI32U);
                self.op(I::GlobalSet(prev));
            }
            "rising_edge" | "falling_edge" => {
                let prev = self.global(0.0);
                let n = self.tmp();
                self.flag(inst, 0)?;
                self.op(I::F64ConvertI32U);
                self.op(I::LocalSet(n));
                if kind == "rising_edge" {
                    self.op(I::LocalGet(n));
                    self.c(0.0);
                    self.op(I::F64Ne);
                    self.op(I::GlobalGet(prev));
                    self.c(0.0);
                    self.op(I::F64Eq);
                } else {
                    self.op(I::LocalGet(n));
                    self.c(0.0);
                    self.op(I::F64Eq);
                    self.op(I::GlobalGet(prev));
                    self.c(0.0);
                    self.op(I::F64Ne);
                }
                self.op(I::I32And);
                self.op(I::F64ConvertI32U);
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(prev));
            }
            "timer_on" => {
                let (running, start) = (self.global(0.0), self.global(0.0));
                self.flag(inst, 0)?;
                self.choose(
                    |e| {
                        e.op(I::GlobalGet(running));
                        e.c(0.0);
                        e.op(I::F64Eq);
                        e.op(I::If(BlockType::Empty));
                        e.c(1.0);
                        e.op(I::GlobalSet(running));
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(start));
                        e.op(I::End);
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalGet(start));
                        e.op(I::F64Sub);
                        e.param(inst, 0);
                        e.op(I::F64Ge);
                        e.op(I::F64ConvertI32U);
                        Ok(())
                    },
                    |e| {
                        e.c(0.0);
                        e.op(I::GlobalSet(running));
                        e.c(0.0);
                        Ok(())
                    },
                )?;
            }
            "timer_off" => {
                let (on, off_at) = (self.global(0.0), self.global(0.0));
                self.flag(inst, 0)?;
                self.choose(
                    |e| {
                        e.c(1.0);
                        e.op(I::GlobalSet(on));
                        e.c(1.0);
                        Ok(())
                    },
                    |e| {
                        e.op(I::GlobalGet(on));
                        e.c(0.0);
                        e.op(I::F64Ne);
                        e.op(I::If(BlockType::Empty));
                        e.c(0.0);
                        e.op(I::GlobalSet(on));
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(off_at));
                        e.op(I::End);
                        e.out_get(inst, 0);
                        e.c(0.0);
                        e.op(I::F64Ne);
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalGet(off_at));
                        e.op(I::F64Sub);
                        e.param(inst, 0);
                        e.op(I::F64Lt);
                        e.op(I::I32And);
                        e.op(I::F64ConvertI32U);
                        Ok(())
                    },
                )?;
            }
            "pulse" => {
                let (prev, active, start) = (self.global(0.0), self.global(0.0), self.global(0.0));
                let n = self.tmp();
                self.flag(inst, 0)?;
                self.op(I::F64ConvertI32U);
                self.op(I::LocalSet(n));
                self.op(I::LocalGet(n));
                self.c(0.0);
                self.op(I::F64Ne);
                self.op(I::GlobalGet(prev));
                self.c(0.0);
                self.op(I::F64Eq);
                self.op(I::I32And);
                self.op(I::GlobalGet(active));
                self.c(0.0);
                self.op(I::F64Eq);
                self.op(I::I32And);
                self.op(I::If(BlockType::Empty));
                self.c(1.0);
                self.op(I::GlobalSet(active));
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalSet(start));
                self.op(I::End);
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(prev));
                self.op(I::GlobalGet(active));
                self.c(0.0);
                self.op(I::F64Ne);
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalGet(start));
                self.op(I::F64Sub);
                self.param(inst, 0);
                self.op(I::F64Ge);
                self.op(I::I32And);
                self.op(I::If(BlockType::Empty));
                self.c(0.0);
                self.op(I::GlobalSet(active));
                self.op(I::End);
                self.op(I::GlobalGet(active));
            }
            "generator" => {
                let start = self.global(0.0);
                let (t_on, period, x) = (self.tmp(), self.tmp(), self.tmp());
                self.flag(inst, 0)?;
                self.choose(
                    |e| {
                        e.param(inst, 0);
                        e.op(I::LocalSet(t_on));
                        e.op(I::LocalGet(t_on));
                        e.param(inst, 1);
                        e.op(I::F64Add);
                        e.c(1.0);
                        e.op(I::F64Max);
                        e.op(I::LocalSet(period));
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalGet(start));
                        e.op(I::F64Sub);
                        e.op(I::LocalSet(x));
                        // phase = x % period = x - period * trunc(x / period)
                        e.op(I::LocalGet(x));
                        e.op(I::LocalGet(period));
                        e.op(I::LocalGet(x));
                        e.op(I::LocalGet(period));
                        e.op(I::F64Div);
                        e.op(I::F64Trunc);
                        e.op(I::F64Mul);
                        e.op(I::F64Sub);
                        e.op(I::LocalGet(t_on));
                        e.op(I::F64Lt);
                        e.op(I::F64ConvertI32U);
                        Ok(())
                    },
                    |e| {
                        e.op(I::GlobalGet(GLOBAL_NOW));
                        e.op(I::GlobalSet(start));
                        e.c(0.0);
                        Ok(())
                    },
                )?;
            }
            "debounce" => {
                let (last, changed) = (self.global(0.0), self.global(0.0));
                let n = self.tmp();
                self.flag(inst, 0)?;
                self.op(I::F64ConvertI32U);
                self.op(I::LocalSet(n));
                self.op(I::LocalGet(n));
                self.op(I::GlobalGet(last));
                self.op(I::F64Ne);
                self.op(I::If(BlockType::Empty));
                self.op(I::LocalGet(n));
                self.op(I::GlobalSet(last));
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalSet(changed));
                self.op(I::End);
                self.op(I::GlobalGet(GLOBAL_NOW));
                self.op(I::GlobalGet(changed));
                self.op(I::F64Sub);
                self.param(inst, 0);
                self.op(I::F64Ge);
                self.choose(
                    |e| {
                        e.op(I::GlobalGet(last));
                        Ok(())
                    },
                    |e| {
                        e.out_get(inst, 0);
                        Ok(())
                    },
                )?;
            }
            "counter" => {
                let (up, down) = (self.global(0.0), self.global(0.0));
                self.flag(inst, 2)?;
                self.choose(
                    |e| {
                        e.c(0.0);
                        Ok(())
                    },
                    |e| {
                        e.out_get(inst, 0);
                        e.flag(inst, 0)?;
                        e.op(I::GlobalGet(up));
                        e.c(0.0);
                        e.op(I::F64Eq);
                        e.op(I::I32And);
                        e.op(I::F64ConvertI32U);
                        e.op(I::F64Add);
                        e.flag(inst, 1)?;
                        e.op(I::GlobalGet(down));
                        e.c(0.0);
                        e.op(I::F64Eq);
                        e.op(I::I32And);
                        e.op(I::F64ConvertI32U);
                        e.op(I::F64Sub);
                        Ok(())
                    },
                )?;
                self.flag(inst, 0)?;
                self.op(I::F64ConvertI32U);
                self.op(I::GlobalSet(up));
                self.flag(inst, 1)?;
                self.op(I::F64ConvertI32U);
                self.op(I::GlobalSet(down));
            }
            "constant" => self.param(inst, 0),
            "switch" => {
                self.input(inst, 2)?;
                self.input(inst, 1)?;
                self.flag(inst, 0)?;
                self.op(I::Select);
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
                let (s, r) = (self.tmp(), self.tmp());
                self.input(inst, 0)?;
                self.c(0.0);
                self.op(I::F64Max);
                self.op(I::LocalSet(s));
                self.input(inst, count)?;
                self.op(I::LocalSet(r));
                for j in (0..count - 1).rev() {
                    self.input(inst, 1 + j)?;
                    self.op(I::LocalGet(r));
                    self.op(I::LocalGet(s));
                    self.c((j + 1) as f64);
                    self.op(I::F64Lt);
                    self.op(I::Select);
                    self.op(I::LocalSet(r));
                }
                self.op(I::LocalGet(r));
            }
            "var_get" => {
                let name = inst.block.var_name().unwrap_or_default().to_owned();
                self.var_read(&name);
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
                    self.flag(inst, 1)?;
                    self.op(I::If(BlockType::Empty));
                }
                self.input(inst, 0)?;
                self.var_write(&name);
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
        }
        if inst.lib.def.outputs.is_empty() {
            self.op(I::Drop);
        } else {
            self.set_out(inst, 0);
        }
        Ok(())
    }

    fn finish(self) -> WasmModule {
        let mut types = TypeSection::new();
        types.ty().function([ValType::I32], [ValType::F64]); // tag_get
        types.ty().function([ValType::I32, ValType::F64], []); // tag_set
        types.ty().function([], [ValType::I32]); // abi_version
        types.ty().function([ValType::I32], []); // step
        types.ty().function([], []); // init
        types.ty().function([ValType::I32], [ValType::I32]); // gpio_read, adc_read
        types.ty().function([ValType::I32, ValType::I32], []); // gpio_write, pwm_write

        let mut imports = ImportSection::new();
        imports.import("flp", "tag_get", EntityType::Function(0));
        imports.import("flp", "tag_set", EntityType::Function(1));
        imports.import("flp", "output", EntityType::Function(1));
        // Hardware imports follow in placeholder order; defined functions come after them.
        let used: Vec<u32> = self.hw.iter().copied().collect();
        let mut allow = Vec::new();
        for &h in &used {
            let name = HW_NAMES[(h - HW_BASE) as usize];
            let ty = if h == HW_GPIO_READ || h == HW_ADC_READ {
                5
            } else {
                6
            };
            imports.import("flp", name, EntityType::Function(ty));
            allow.push(format!("\"{name}\""));
        }
        let first = 3 + used.len() as u32;
        let remap = |n: u32| match used.iter().position(|&h| h == n) {
            Some(k) => 3 + k as u32,
            None => n,
        };

        let mut funcs = FunctionSection::new();
        funcs.function(2);
        funcs.function(3);
        funcs.function(4);
        for _ in &self.cuts {
            funcs.function(3);
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
        for &init in &self.globals {
            globals.global(
                GlobalType {
                    val_type: ValType::F64,
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::f64_const(init.into()),
            );
        }

        let mut exports = ExportSection::new();
        exports.export("memory", ExportKind::Memory, 0);
        exports.export("abi_version", ExportKind::Func, first);
        exports.export("step", ExportKind::Func, first + 1);
        exports.export("init", ExportKind::Func, first + 2);

        let mut abi = Function::new([]);
        abi.instruction(&I::I32Const(2)).instruction(&I::End);

        // `step` runs the first piece and calls the rest, each a function of its own with the same locals.
        let locals = vec![ValType::F64; self.scratch_peak as usize];
        let mut bounds = vec![0];
        bounds.extend(self.cuts.iter().copied());
        bounds.push(self.code.len());
        let mut pieces: Vec<Function> = Vec::new();
        for w in bounds.windows(2) {
            let mut f = Function::new_with_locals_types(locals.clone());
            for i in &self.code[w[0]..w[1]] {
                match i {
                    I::Call(n) if *n >= HW_BASE => f.instruction(&I::Call(remap(*n))),
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
                init.instruction(&I::F64Const(pv(value).into()));
                init.instruction(&I::Call(TAG_SET));
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
            "name = \"{}\"\nabi = 2\nallow = [{}]\n",
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
