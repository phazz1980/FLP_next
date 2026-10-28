//! Background task: blocks of the sheets marked "background" run in a FreeRTOS task
//! on core 0, the rest in `loop()`. Signals that cross the boundary are passed as
//! snapshots: the producer publishes its outputs at the end of a pass, the consumer
//! takes them at the start of its own, both under one mutex.
//!
//! On two cores the network blocks (`shared = "network"`) always run in the background
//! task, wherever their sheet is. A bool that crosses the boundary also keeps a "was true
//! since the last take" flag, so a one-pass pulse is seen by the other task exactly once.
//!
//! A single-core chip gains nothing from a second task, so there the blocks of a
//! background sheet stay in `loop()` but run on a period of their own, at most one
//! sheet per pass and with the sheets' phases spread over the period.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use flp_core::vars::storage_name;
use flp_core::SignalType;

use crate::{GenError, Instance, Program, Source};

#[derive(Debug, Default)]
pub struct Tasks {
    /// Ids of the blocks that run in the background task.
    background: HashSet<String>,
    /// Signal → the producer runs in the background task.
    crossing: BTreeMap<usize, bool>,
    /// A real second task (two cores); otherwise the sheets are gated in `loop()`.
    threaded: bool,
    /// Gated sheets on a single-core chip: (sheet, period in ms).
    gates: Vec<(u32, u32)>,
}

/// `shared` tag of the blocks that use the network libraries.
const NETWORK: &str = "network";

/// Chips with one core (ESP32-S2, C2, C3, C6, H2).
fn single_core(fqbn: &str) -> bool {
    let chip = fqbn.rsplit(':').next().unwrap_or("");
    ["esp32s2", "esp32c2", "esp32c3", "esp32c6", "esp32h2"]
        .iter()
        .any(|c| chip.starts_with(c))
}

impl Tasks {
    pub fn new(program: &Program) -> Result<Self, GenError> {
        let layout = &program.project.layout;
        let threaded = !single_core(&program.project.config.board.fqbn);
        // The network libraries are not thread-safe and block on connects, so with two
        // cores the whole network family lives in the background task, whatever the sheet.
        let background: HashSet<String> = program
            .blocks
            .iter()
            .filter(|i| {
                layout.is_background(crate::stats::sheet_of(program, i))
                    || (threaded && i.lib.def.info.shared == NETWORK)
            })
            .map(|i| i.block.id.clone())
            .collect();
        let mut tasks = Self {
            background,
            crossing: BTreeMap::new(),
            threaded,
            gates: Vec::new(),
        };
        if tasks.background.is_empty() {
            return Ok(tasks);
        }
        if !tasks.threaded {
            let sheets: BTreeSet<u32> = program
                .blocks
                .iter()
                .filter(|i| tasks.background.contains(&i.block.id))
                .map(|i| crate::stats::sheet_of(program, i))
                .collect();
            tasks.gates = sheets
                .into_iter()
                .map(|s| (s, layout.background_period(s)))
                .collect();
            return Ok(tasks);
        }
        tasks.check(program)?;

        let mut producer: BTreeMap<usize, bool> = BTreeMap::new();
        for inst in &program.blocks {
            for &signal in inst.outputs.values() {
                producer.insert(signal, tasks.is_background(inst));
            }
        }
        for inst in &program.blocks {
            for input in inst.inputs.values() {
                if let Source::Signal { index, .. } = input.source {
                    if producer.get(&index) != Some(&tasks.is_background(inst)) {
                        tasks.crossing.insert(index, producer[&index]);
                    }
                }
            }
        }
        Ok(tasks)
    }

    /// The chip has two cores, so a second task can run in parallel.
    pub fn threaded(&self) -> bool {
        self.threaded
    }

    /// There is a background task.
    pub fn any(&self) -> bool {
        self.threaded && !self.background.is_empty()
    }

    pub fn is_background(&self, inst: &Instance) -> bool {
        self.threaded && self.background.contains(&inst.block.id)
    }

    /// Name of the flag that says the (gated) sheet of the block runs in this pass.
    pub fn gate(&self, program: &Program, inst: &Instance) -> Option<String> {
        if self.threaded || !self.background.contains(&inst.block.id) {
            return None;
        }
        Some(run_name(crate::stats::sheet_of(program, inst)))
    }

    /// Timers of the gated sheets; the first run of sheet `i` of `n` is delayed by
    /// `i / n` of its period.
    pub fn gate_globals(&self) -> Vec<String> {
        let n = self.gates.len() as u32;
        self.gates
            .iter()
            .enumerate()
            .map(|(i, &(sheet, period))| {
                let offset = i as u32 * period / n;
                format!(
                    "uint32_t flp_bg{sheet}_t = {offset}u - {period}u;  // плата {sheet}: раз в {period} мс"
                )
            })
            .collect()
    }

    /// Start of `loop()`: decides which gated sheet runs in this pass (at most one).
    pub fn gate_prologue(&self) -> Vec<String> {
        if self.gates.is_empty() {
            return Vec::new();
        }
        let mut out = vec![
            "// Фоновые платы на одноядерном МК: каждая раз в свой период, не больше одной за проход"
                .to_owned(),
            "uint32_t flp_now = millis();".to_owned(),
        ];
        let many = self.gates.len() > 1;
        if many {
            out.push("bool flp_bg_one = false;".to_owned());
        }
        for &(sheet, period) in &self.gates {
            out.push(format!("bool {} = false;", run_name(sheet)));
            let free = if many { "!flp_bg_one && " } else { "" };
            let taken = if many { " flp_bg_one = true;" } else { "" };
            out.push(format!(
                "if ({free}(uint32_t)(flp_now - flp_bg{sheet}_t) >= {period}u) {{ flp_bg{sheet}_t = flp_now; {}= true;{taken} }}",
                format_args!("{} ", run_name(sheet))
            ));
        }
        out
    }

    /// Name of the signal as the consumer reads it: a copy taken at the start of its pass
    /// when the producer runs in the other task.
    pub fn read_name(&self, program: &Program, inst: &Instance, signal: usize) -> String {
        let name = &program.signals[signal].name;
        match self.crossing.get(&signal) {
            Some(&from_background) if from_background != self.is_background(inst) => in_name(name),
            _ => name.clone(),
        }
    }

    /// Lines publishing the signals the given task produces for the other one.
    /// A bool also sets a "seen true" flag, so a pulse shorter than a pass of the
    /// other task is not lost.
    pub fn publish(&self, program: &Program, background: bool) -> Vec<String> {
        let mut out = Vec::new();
        for (&s, _) in self.crossing.iter().filter(|(_, &from)| from == background) {
            let signal = &program.signals[s];
            let name = &signal.name;
            out.push(format!("{} = {name};", pub_name(name)));
            if signal.ty == SignalType::Bool {
                out.push(format!("{0} = {0} || {name};", pend_name(name)));
            }
        }
        out
    }

    /// Lines taking the signals the other task has published for the given one;
    /// a bool is true when it is true now or was true since the last take.
    pub fn take(&self, program: &Program, background: bool) -> Vec<String> {
        let mut out = Vec::new();
        for (&s, _) in self.crossing.iter().filter(|(_, &from)| from != background) {
            let signal = &program.signals[s];
            let name = &signal.name;
            if signal.ty == SignalType::Bool {
                out.push(format!(
                    "{} = {} || {};",
                    in_name(name),
                    pub_name(name),
                    pend_name(name)
                ));
                out.push(format!("{} = false;", pend_name(name)));
            } else {
                out.push(format!("{} = {};", in_name(name), pub_name(name)));
            }
        }
        out
    }

    /// Declarations of the transfer variables.
    pub fn globals(&self, program: &Program) -> Vec<String> {
        let mut out = Vec::new();
        for &s in self.crossing.keys() {
            let signal = &program.signals[s];
            let (ty, zero) = (
                crate::arduino::cpp_type(signal.ty),
                crate::arduino::zero(signal.ty),
            );
            out.push(format!("{ty} {} = {zero};", pub_name(&signal.name)));
            out.push(format!("{ty} {} = {zero};", in_name(&signal.name)));
            if signal.ty == SignalType::Bool {
                out.push(format!("bool {} = false;", pend_name(&signal.name)));
            }
        }
        out
    }

    /// Blocks of one task may not share a library object or a variable of type string
    /// with the other: the libraries are not thread-safe.
    fn check(&self, program: &Program) -> Result<(), GenError> {
        let task = |bg: bool| {
            if bg {
                "фоновой"
            } else {
                "основной"
            }
        };
        let mut shared: BTreeMap<&str, BTreeMap<bool, &str>> = BTreeMap::new();
        let mut uarts: BTreeMap<u8, BTreeMap<bool, &str>> = BTreeMap::new();
        let mut texts: BTreeMap<String, BTreeMap<bool, &str>> = BTreeMap::new();
        let text_vars: BTreeSet<&str> = program
            .project
            .logic
            .vars
            .iter()
            .filter(|v| v.ty.signal() == SignalType::String)
            .map(|v| v.name.as_str())
            .collect();
        for inst in &program.blocks {
            let bg = self.is_background(inst);
            let id = inst.block.id.as_str();
            let tag = inst.lib.def.info.shared.as_str();
            if !tag.is_empty() {
                shared.entry(tag).or_default().insert(bg, id);
            }
            if let Some((uart, _)) = crate::arduino::uart_of(program, inst) {
                uarts.entry(uart.number).or_default().insert(bg, id);
            }
            let used = inst
                .block
                .var_name()
                .map(str::to_owned)
                .into_iter()
                .chain(inst.inputs.values().filter_map(|i| match &i.source {
                    Source::Var(name) => Some(name.clone()),
                    _ => None,
                }))
                .chain(inst.param_vars.values().map(|v| (*v).to_owned()))
                .chain(inst.output_vars.values().map(|v| (*v).to_owned()));
            for name in used {
                if text_vars.contains(name.as_str()) {
                    texts.entry(name).or_default().insert(bg, id);
                }
            }
        }
        for (tag, by_task) in &shared {
            if let (Some(a), Some(b)) = (by_task.get(&false), by_task.get(&true)) {
                let what = if *tag == "network" { "сеть" } else { tag };
                return Err(GenError::Task(format!(
                    "блоки «{a}» ({}) и «{b}» ({}) работают с общим ресурсом «{what}» (Wi-Fi, MQTT, веб, Modbus) — \
                     разместите их на платах одной задачи",
                    task(false),
                    task(true)
                )));
            }
        }
        for (number, by_task) in &uarts {
            if let (Some(a), Some(b)) = (by_task.get(&false), by_task.get(&true)) {
                return Err(GenError::Task(format!(
                    "блоки «{a}» ({}) и «{b}» ({}) используют один порт UART {number} — \
                     разместите их на платах одной задачи",
                    task(false),
                    task(true)
                )));
            }
        }
        for (name, by_task) in &texts {
            if let (Some(a), Some(b)) = (by_task.get(&false), by_task.get(&true)) {
                return Err(GenError::Task(format!(
                    "переменная-строка «{}» используется в блоках «{a}» ({}) и «{b}» ({}); \
                     строки нельзя безопасно делить между задачами — передайте значение связью между платами",
                    storage_name(name),
                    task(false),
                    task(true)
                )));
            }
        }
        Ok(())
    }
}

fn run_name(sheet: u32) -> String {
    format!("flp_bg{sheet}_run")
}

fn pub_name(name: &str) -> String {
    format!("{name}_pub")
}

fn in_name(name: &str) -> String {
    format!("{name}_in")
}

fn pend_name(name: &str) -> String {
    format!("{name}_pend")
}
