//! Background task: blocks of the sheets marked "background" run in a FreeRTOS task
//! on core 0, the rest in `loop()`. Signals that cross the boundary are passed as
//! snapshots: the producer publishes its outputs at the end of a pass, the consumer
//! takes them at the start of its own, both under one mutex.

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
}

impl Tasks {
    pub fn new(program: &Program) -> Result<Self, GenError> {
        let layout = &program.project.layout;
        let background: HashSet<String> = program
            .blocks
            .iter()
            .filter(|i| layout.is_background(crate::stats::sheet_of(program, i)))
            .map(|i| i.block.id.clone())
            .collect();
        let mut tasks = Self {
            background,
            crossing: BTreeMap::new(),
        };
        if tasks.background.is_empty() {
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

    /// There is a background task.
    pub fn any(&self) -> bool {
        !self.background.is_empty()
    }

    pub fn is_background(&self, inst: &Instance) -> bool {
        self.background.contains(&inst.block.id)
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
    pub fn publish(&self, program: &Program, background: bool) -> Vec<String> {
        self.crossing
            .iter()
            .filter(|(_, &from)| from == background)
            .map(|(&s, _)| {
                let name = &program.signals[s].name;
                format!("{} = {name};", pub_name(name))
            })
            .collect()
    }

    /// Lines taking the signals the other task has published for the given one.
    pub fn take(&self, program: &Program, background: bool) -> Vec<String> {
        self.crossing
            .iter()
            .filter(|(_, &from)| from != background)
            .map(|(&s, _)| {
                let name = &program.signals[s].name;
                format!("{} = {};", in_name(name), pub_name(name))
            })
            .collect()
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

fn pub_name(name: &str) -> String {
    format!("{name}_pub")
}

fn in_name(name: &str) -> String {
    format!("{name}_in")
}
