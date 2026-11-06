//! Simulation of a whole schema on the PC: one scan per step of simulated time,
//! in the execution order the generator uses. Built-in blocks are computed by
//! `builtin`, blocks with a `block.wasm` by their module, the rest (hardware) keep
//! outputs set by hand.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crab_blocks::{ds18b20_channel, Bench, Signal, Types, OWN_SLAVE};
use crab_core::value::{parse_duration_ms, ParamValue};
use crab_core::vars::{VAR_GET, VAR_SET};
use crab_core::{check, Library, Logic, Severity};
use crab_core::{BlockDef, ParamKind, SignalType};

use crate::builtin::{self, Behavior, Ctx};
use crate::SimModule;

/// Who computes the outputs of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Computed {
    /// Rust code of the program.
    Builtin,
    /// The block's own `block.wasm`.
    Module,
    /// Rust code of the program, fed by the virtual bench (pin levels, sensor signals).
    Bench,
    /// Nobody: the outputs are set by hand (hardware blocks, blocks without a module).
    Manual,
}

enum Source {
    /// Output `port` of node `node`; `invert` for outputs marked inverted.
    Link {
        node: usize,
        port: usize,
        invert: bool,
    },
    Value(f64),
    Text(String),
}

struct Node {
    id: String,
    def: BlockDef,
    behavior: Option<Box<dyn Behavior>>,
    /// Real port types and parameter kinds: the numeric model works by them.
    types: Types,
    module: Option<SimModule>,
    /// `block.wasm` of the block and its modification time at the last load.
    wasm: Option<(PathBuf, Option<SystemTime>)>,
    /// Variable a `var_get` / `var_set` block accesses.
    var: Option<String>,
    sources: Vec<Source>,
    invert_in: Vec<bool>,
    active: Vec<bool>,
    params: Vec<f64>,
    /// Text of string parameters (empty for the others), in parameter order.
    param_texts: Vec<String>,
    /// Parameters read from an input on every scan: (parameter, input) indexes.
    param_inputs: Vec<(usize, usize)>,
    inputs: Vec<f64>,
    outputs: Vec<f64>,
    /// Text of string ports (empty for the others), in port order.
    texts_in: Vec<String>,
    texts_out: Vec<String>,
    /// Simulated time left until the `received` pulse of a hand-fed MQTT block ends.
    pulse_ms: u64,
}

/// Blocks whose outputs follow what the bench shows on their pin.
fn reads_bench(kind: &str) -> bool {
    matches!(
        kind,
        "digital_in"
            | "analog_in"
            | "dht22"
            | "ds18b20"
            | "wifi"
            | "ethernet"
            | "mqtt"
            | "modbus_master_read"
            | "modbus_read"
            | "uart_read"
    )
}

/// What the bench lets the user set for an output of a block.
#[derive(Debug, Clone, PartialEq)]
pub enum BenchInput {
    /// Level of a digital pin: `0`/`1`.
    Digital { pin: i64 },
    /// Counts of an analog pin: `0…4095`.
    Analog { pin: i64 },
    /// A sensor channel: a value that may change in time, or a fault.
    Sensor { pin: i64, channel: String },
    /// The link of Wi-Fi or Ethernet.
    Link,
    /// The MQTT broker: reachable or not (the link is set on the network block).
    Broker,
    /// A register of a Modbus slave and whether the slave answers.
    Modbus { slave: i64, table: String, reg: i64 },
    /// Text typed into a UART port: a line, or bytes one by one.
    Uart { port: i64 },
}

impl Node {
    /// The pin the block works with.
    fn pin(&self) -> Option<i64> {
        let p = self.def.params.iter().position(|p| p.name == "pin")?;
        Some(self.params[p] as i64)
    }

    /// A value was fed to an MQTT subscription by hand: `received` is on for a while,
    /// long enough to be seen.
    fn message_arrived(&mut self) {
        if self.def.info.kind == "mqtt_subscribe" {
            self.set_received(1.0);
            self.pulse_ms = builtin::PULSE_MS;
        }
    }

    fn set_received(&mut self, value: f64) {
        if let Some(p) = self.def.outputs.iter().position(|o| o.name == "received") {
            self.outputs[p] = value;
        }
    }

    /// Web controls (switch, slider, number input) start at their `initial` parameter
    /// and are then changed by hand like any other input of the board.
    fn apply_initial(&mut self) {
        if !matches!(
            self.def.info.kind.as_str(),
            "web_switch" | "web_slider" | "web_input"
        ) {
            return;
        }
        if let Some(p) = self.def.params.iter().position(|p| p.name == "initial") {
            if let Some(out) = self.outputs.first_mut() {
                *out = self.params[p];
            }
        }
    }

    fn computed(&self) -> Computed {
        if self.module.is_some() {
            Computed::Module
        } else if self.behavior.is_some() && reads_bench(&self.def.info.kind) {
            Computed::Bench
        } else if self.behavior.is_some() || self.var.is_some() {
            Computed::Builtin
        } else {
            Computed::Manual
        }
    }
}

pub struct Simulation {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    order: Vec<usize>,
    /// Values of the project variables, by name.
    vars: HashMap<String, f64>,
    /// Initial values of the variables, for `reset`.
    var_init: HashMap<String, f64>,
    /// Values of the string variables, by name (and their values after power-on).
    var_texts: HashMap<String, String>,
    var_text_init: HashMap<String, String>,
    time_ms: u64,
    /// The world outside the board.
    bench: Bench,
    /// Load problems of modules: the block falls back to manual outputs.
    pub notes: Vec<String>,
}

fn number(v: &ParamValue, kind: ParamKind) -> f64 {
    match v {
        ParamValue::Bool(x) => f64::from(u8::from(*x)),
        ParamValue::Int(x) => *x as f64,
        ParamValue::Float(x) => *x,
        ParamValue::Text(s) if kind == ParamKind::Duration => {
            parse_duration_ms(s).map_or(0.0, |ms| ms as f64)
        }
        ParamValue::Text(_) => 0.0,
    }
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn plain_number(v: &ParamValue) -> f64 {
    number(v, ParamKind::Float)
}

impl Simulation {
    /// Builds the simulation of a schema that passes the check.
    pub fn new(logic: &Logic, lib: &Library) -> Result<Self, String> {
        let report = check(logic, lib, None);
        if let Some(e) = report
            .diagnostics
            .iter()
            .find(|d| d.severity == Severity::Error)
        {
            return Err(format!("в схеме есть ошибка: {}", e.message));
        }

        let mut nodes = Vec::with_capacity(logic.blocks.len());
        let mut index = HashMap::new();
        let mut notes = Vec::new();
        let (port_in, port_out) = crab_core::port_types(logic, lib, &report.order);
        for (bi, block) in logic.blocks.iter().enumerate() {
            let lb = lib
                .resolve(logic, block)
                .ok_or_else(|| format!("нет блока «{}» в библиотеке", block.kind))?;
            let def = lb.def.clone();
            let params: Vec<f64> = def
                .params
                .iter()
                .map(|p| {
                    block
                        .params
                        .get(&p.name)
                        .or(p.default.as_ref())
                        .map_or(0.0, |v| number(v, p.kind))
                })
                .collect();
            let param_texts: Vec<String> = def
                .params
                .iter()
                .map(|p| match block.params.get(&p.name).or(p.default.as_ref()) {
                    Some(ParamValue::Text(s)) if p.kind != ParamKind::Duration => s.clone(),
                    _ => String::new(),
                })
                .collect();
            let param_inputs: Vec<(usize, usize)> = block
                .param_in
                .iter()
                .filter_map(|name| {
                    let p = def.params.iter().position(|p| &p.name == name)?;
                    let i = def.inputs.iter().position(|i| &i.name == name)?;
                    Some((p, i))
                })
                .collect();
            let active: Vec<bool> = def
                .inputs
                .iter()
                .map(|p| {
                    def.enabled(p.when.as_deref(), &block.params)
                        .unwrap_or(true)
                })
                .collect();
            let mut module = None;
            let mut wasm = None;
            if let Some(path) = lb.wasm_path().filter(|p| p.is_file()) {
                wasm = Some((path.clone(), modified(&path)));
                match std::fs::read(&path)
                    .map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))
                    .and_then(|bytes| SimModule::load(&bytes, def.outputs.len()))
                {
                    Ok(m) => module = Some(m),
                    Err(e) => notes.push(format!("{}: {e}", block.id)),
                }
            }
            let types = Types {
                ins: def
                    .inputs
                    .iter()
                    .map(|p| port_in.get(&(bi, p.name.clone())).copied().unwrap_or(p.ty))
                    .collect(),
                outs: def
                    .outputs
                    .iter()
                    .map(|p| port_out.get(&(bi, p.name.clone())).copied().unwrap_or(p.ty))
                    .collect(),
                params: def.params.iter().map(|p| p.kind).collect(),
            };
            let behavior = if module.is_some() {
                None
            } else {
                builtin::behavior(&def.info.kind).map(|mut b| {
                    b.set_types(&types);
                    b
                })
            };
            index.insert(block.id.clone(), nodes.len());
            nodes.push(Node {
                id: block.id.clone(),
                invert_in: def
                    .inputs
                    .iter()
                    .map(|p| block.inverted_inputs.contains(&p.name))
                    .collect(),
                sources: Vec::new(),
                inputs: vec![0.0; def.inputs.len()],
                outputs: vec![0.0; def.outputs.len()],
                texts_in: vec![String::new(); def.inputs.len()],
                texts_out: vec![String::new(); def.outputs.len()],
                pulse_ms: 0,
                active,
                params,
                param_texts,
                param_inputs,
                behavior,
                types,
                module,
                wasm,
                var: block.var_name().map(str::to_owned),
                def,
            });
        }

        // Input sources: a link, else a constant of the block, else the port default.
        for (i, block) in logic.blocks.iter().enumerate() {
            let mut sources = Vec::new();
            for port in &nodes[i].def.inputs {
                let link = logic
                    .links
                    .iter()
                    .find(|l| l.to.block == block.id && l.to.port == port.name);
                let source = link
                    .and_then(|l| {
                        let node = *index.get(&l.from.block)?;
                        let out = nodes[node]
                            .def
                            .outputs
                            .iter()
                            .position(|o| o.name == l.from.port)?;
                        Some(Source::Link {
                            node,
                            port: out,
                            invert: logic.blocks[node].inverted.contains(&l.from.port),
                        })
                    })
                    .or_else(|| {
                        block
                            .consts
                            .get(&port.name)
                            .or(port.default.as_ref())
                            .map(|v| match v {
                                ParamValue::Text(s) if port.ty == SignalType::String => {
                                    Source::Text(s.clone())
                                }
                                v => Source::Value(plain_number(v)),
                            })
                    })
                    .unwrap_or(Source::Value(0.0));
                sources.push(source);
            }
            nodes[i].sources = sources;
        }

        let var_init: HashMap<String, f64> = logic
            .vars
            .iter()
            .map(|v| (v.name.clone(), v.init.as_ref().map_or(0.0, plain_number)))
            .collect();
        let var_text_init: HashMap<String, String> = logic
            .vars
            .iter()
            .filter(|v| v.ty.signal() == SignalType::String)
            .map(|v| {
                let text = match &v.init {
                    Some(ParamValue::Text(s)) => s.clone(),
                    _ => String::new(),
                };
                (v.name.clone(), text)
            })
            .collect();
        nodes.iter_mut().for_each(Node::apply_initial);
        Ok(Self {
            nodes,
            index,
            order: report.order,
            vars: var_init.clone(),
            var_init,
            var_texts: var_text_init.clone(),
            var_text_init,
            time_ms: 0,
            bench: Bench::new(),
            notes,
        })
    }

    /// Simulated time since the start.
    pub fn time_ms(&self) -> u64 {
        self.time_ms
    }

    /// Back to the start: states, outputs and time are cleared (manual outputs too).
    pub fn reset(&mut self) -> Result<(), String> {
        self.time_ms = 0;
        self.vars = self.var_init.clone();
        self.var_texts = self.var_text_init.clone();
        self.bench.clear_driven();
        for node in &mut self.nodes {
            node.pulse_ms = 0;
            node.outputs.iter_mut().for_each(|v| *v = 0.0);
            node.texts_out.iter_mut().for_each(String::clear);
            node.inputs.iter_mut().for_each(|v| *v = 0.0);
            node.apply_initial();
            if let Some(mut b) =
                builtin::behavior(&node.def.info.kind).filter(|_| node.module.is_none())
            {
                b.set_types(&node.types);
                node.behavior = Some(b);
            }
            if let Some(m) = &mut node.module {
                m.reset()?;
            }
        }
        Ok(())
    }

    /// One scan of the schema after `dt_ms` of simulated time. A failing module stops
    /// the scan with its error; the outputs computed so far stay visible.
    pub fn step(&mut self, dt_ms: u32) -> Result<(), String> {
        self.time_ms += u64::from(dt_ms);
        for node in &mut self.nodes {
            if node.pulse_ms > 0 {
                node.pulse_ms = node.pulse_ms.saturating_sub(u64::from(dt_ms));
                if node.pulse_ms == 0 {
                    node.set_received(0.0);
                }
            }
        }
        for &i in &self.order.clone() {
            self.read_inputs(i);
            let now_ms = self.time_ms;
            let node = &mut self.nodes[i];
            let pin = node.pin();
            for &(p, input) in &node.param_inputs {
                node.params[p] = node.inputs[input];
            }
            if let Some(var) = &node.var {
                let text = self.var_texts.contains_key(var);
                if node.def.info.kind == VAR_GET {
                    if text {
                        node.texts_out[0] = self.var_texts[var].clone();
                    } else {
                        node.outputs[0] = self.vars.get(var).copied().unwrap_or(0.0);
                    }
                } else if node.def.info.kind == VAR_SET
                    && node.inputs.get(1).is_none_or(|en| *en != 0.0)
                {
                    if text {
                        self.var_texts.insert(var.clone(), node.texts_in[0].clone());
                    } else {
                        self.vars.insert(var.clone(), node.inputs[0]);
                    }
                }
            } else if let Some(m) = &mut node.module {
                let (outputs, texts) = m
                    .step_text(
                        dt_ms,
                        &node.inputs,
                        &node.params,
                        &node.texts_in,
                        &node.param_texts,
                    )
                    .map_err(|e| format!("{}: {e}", node.id))?;
                node.outputs = outputs;
                for (slot, text) in node.texts_out.iter_mut().zip(texts) {
                    *slot = text;
                }
            } else if let Some(b) = &mut node.behavior {
                let ctx = Ctx {
                    now_ms,
                    inputs: &node.inputs,
                    active: &node.active,
                    params: &node.params,
                    texts: &node.texts_in,
                    param_texts: &node.param_texts,
                    bench: &self.bench,
                };
                b.step(&ctx, &mut node.outputs);
                b.step_text(&ctx, &mut node.outputs, &mut node.texts_out);
                // Whatever the block computed, the signal is a 32-bit value of its type.
                for (k, v) in node.outputs.iter_mut().enumerate() {
                    *v = node.types.quantize(k, *v);
                }
                if let (Some(pin), Some(state)) = (pin, b.drive()) {
                    self.bench.set_driven(pin, state);
                } else if let (Some(state), "modbus_write") =
                    (b.drive(), node.def.info.kind.as_str())
                {
                    let at = |name: &str| node.def.params.iter().position(|p| p.name == name);
                    if let (Some(t), Some(r)) = (at("table"), at("reg")) {
                        self.bench.set_register(
                            OWN_SLAVE,
                            &node.param_texts[t],
                            node.params[r] as i64,
                            state,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn read_inputs(&mut self, i: usize) {
        let values: Vec<f64> = self.nodes[i]
            .sources
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let v = match s {
                    Source::Value(v) => *v,
                    Source::Text(_) => 0.0,
                    Source::Link { node, port, invert } => {
                        let v = self.nodes[*node].outputs[*port];
                        if *invert {
                            f64::from(u8::from(v == 0.0))
                        } else {
                            v
                        }
                    }
                };
                if self.nodes[i].invert_in[k] {
                    f64::from(u8::from(v == 0.0))
                } else {
                    v
                }
            })
            .collect();
        self.nodes[i].inputs = values;
        let texts: Vec<String> = self.nodes[i]
            .sources
            .iter()
            .map(|s| match s {
                Source::Text(t) => t.clone(),
                Source::Link { node, port, .. } => self.nodes[*node].texts_out[*port].clone(),
                Source::Value(_) => String::new(),
            })
            .collect();
        self.nodes[i].texts_in = texts;
    }

    /// Sets an output by hand. Only blocks nobody computes accept it (inputs of
    /// the board, hardware blocks); returns `false` otherwise.
    pub fn set_output(&mut self, block: &str, port: &str, value: f64) -> bool {
        let Some(node) = self.index.get(block).map(|i| &mut self.nodes[*i]) else {
            return false;
        };
        let Some(p) = node.def.outputs.iter().position(|o| o.name == port) else {
            return false;
        };
        if node.computed() != Computed::Manual {
            return false;
        }
        node.outputs[p] = value;
        if node.def.outputs[p].name != "received" {
            node.message_arrived();
        }
        true
    }

    /// The virtual bench: pin levels, sensor signals and the state of output pins.
    pub fn bench(&self) -> &Bench {
        &self.bench
    }

    /// The bench for setting pin levels and sensor signals from a test or the editor.
    pub fn bench_mut(&mut self) -> &mut Bench {
        &mut self.bench
    }

    /// Takes over the bench settings of an older simulation of the same project.
    pub fn keep_bench(&mut self, old: &Simulation) {
        self.bench = old.bench.clone();
    }

    /// What the bench can feed into this output, if the block reads it.
    pub fn bench_input(&self, block: &str, port: &str) -> Option<BenchInput> {
        let node = &self.nodes[*self.index.get(block)?];
        if node.computed() != Computed::Bench {
            return None;
        }
        let kind = node.def.info.kind.as_str();
        let net = match (kind, port) {
            ("wifi", _) | ("ethernet", _) => Some(BenchInput::Link),
            ("mqtt", _) => Some(BenchInput::Broker),
            ("uart_read", _) => {
                let at = node.def.params.iter().position(|p| p.name == "port")?;
                Some(BenchInput::Uart {
                    port: node.params[at] as i64,
                })
            }
            ("modbus_read", _) => {
                let at = |name: &str| node.def.params.iter().position(|p| p.name == name);
                Some(BenchInput::Modbus {
                    slave: OWN_SLAVE,
                    table: node.param_texts[at("table")?].clone(),
                    reg: node.params[at("reg")?] as i64,
                })
            }
            ("modbus_master_read", _) => {
                let at = |name: &str| node.def.params.iter().position(|p| p.name == name);
                Some(BenchInput::Modbus {
                    slave: node.params[at("slave")?] as i64,
                    table: node.param_texts[at("table")?].clone(),
                    reg: node.params[at("reg")?] as i64,
                })
            }
            _ => None,
        };
        if net.is_some() {
            return net;
        }
        let pin = node.pin()?;
        match (kind, port) {
            ("digital_in", "out") => Some(BenchInput::Digital { pin }),
            ("analog_in", "out") => Some(BenchInput::Analog { pin }),
            ("dht22", "temp" | "humidity") => Some(BenchInput::Sensor {
                pin,
                channel: port.to_owned(),
            }),
            ("ds18b20", "temp") => {
                let index = node.def.params.iter().position(|p| p.name == "index")?;
                Some(BenchInput::Sensor {
                    pin,
                    channel: ds18b20_channel(node.params[index] as i64),
                })
            }
            _ => None,
        }
    }

    /// The pin an output block drives and its state (`None` while the block does not write).
    pub fn output_pin(&self, block: &str) -> Option<(i64, f64)> {
        let node = &self.nodes[*self.index.get(block)?];
        if !matches!(node.def.info.kind.as_str(), "digital_out" | "pwm_out") {
            return None;
        }
        let pin = node.pin()?;
        Some((pin, self.bench.driven(pin)?))
    }

    /// The network has no link (Wi-Fi or Ethernet).
    pub fn set_offline(&mut self, offline: bool) {
        self.bench.set_offline(offline);
    }

    /// The MQTT broker does not answer.
    pub fn set_broker_down(&mut self, down: bool) {
        self.bench.set_broker_down(down);
    }

    /// A Modbus slave answers or not.
    pub fn set_silent(&mut self, slave: i64, silent: bool) {
        self.bench.set_silent(slave, silent);
    }

    /// Sets a register of a Modbus slave.
    pub fn set_register(&mut self, slave: i64, table: &str, reg: i64, value: f64) {
        self.bench.set_register(slave, table, reg, value);
    }

    /// Types text into a UART port (a line for `uart_read`).
    pub fn send_uart(&mut self, port: i64, text: &str) {
        self.bench.send_uart(port, text);
    }

    /// Sets the level of an input pin.
    pub fn set_pin_level(&mut self, pin: i64, value: f64) {
        self.bench.set_level(pin, value);
    }

    /// Sets a sensor reading; a change over time starts now.
    pub fn set_signal(&mut self, pin: i64, channel: &str, mut signal: Signal) {
        signal.start_ms = self.time_ms;
        self.bench.set_signal(pin, channel, signal);
    }

    /// Takes over the variable values of an older simulation of the same project.
    pub fn keep_vars(&mut self, old: &Simulation) {
        for (name, value) in &old.vars {
            if let Some(v) = self.vars.get_mut(name) {
                *v = *value;
            }
        }
        for (name, text) in &old.var_texts {
            if let Some(t) = self.var_texts.get_mut(name) {
                t.clone_from(text);
            }
        }
    }

    /// Current value of a variable.
    pub fn var(&self, name: &str) -> Option<f64> {
        self.vars.get(name).copied()
    }

    /// Sets a variable by hand (a setpoint nobody writes).
    pub fn set_var(&mut self, name: &str, value: f64) -> bool {
        self.vars.get_mut(name).map(|v| *v = value).is_some()
    }

    /// Current text of a string variable.
    pub fn var_text(&self, name: &str) -> Option<&str> {
        self.var_texts.get(name).map(String::as_str)
    }

    /// Sets a string variable by hand.
    pub fn set_var_text(&mut self, name: &str, text: &str) -> bool {
        self.var_texts
            .get_mut(name)
            .map(|t| *t = text.to_owned())
            .is_some()
    }

    /// Variable a block accesses, if it is a variable block.
    pub fn block_var(&self, block: &str) -> Option<&str> {
        self.nodes[*self.index.get(block)?].var.as_deref()
    }

    /// Value of an output of the last scan.
    pub fn output(&self, block: &str, port: &str) -> Option<f64> {
        let node = &self.nodes[*self.index.get(block)?];
        let p = node.def.outputs.iter().position(|o| o.name == port)?;
        Some(node.outputs[p])
    }

    /// Text on an output of the last scan; `None` for ports that are not strings.
    pub fn output_text(&self, block: &str, port: &str) -> Option<&str> {
        let node = &self.nodes[*self.index.get(block)?];
        let p = node.def.outputs.iter().position(|o| o.name == port)?;
        (node.def.outputs[p].ty == SignalType::String).then(|| node.texts_out[p].as_str())
    }

    /// Sets a string output by hand (same rules as [`Simulation::set_output`]).
    pub fn set_output_text(&mut self, block: &str, port: &str, text: &str) -> bool {
        let Some(node) = self.index.get(block).map(|i| &mut self.nodes[*i]) else {
            return false;
        };
        let Some(p) = node.def.outputs.iter().position(|o| o.name == port) else {
            return false;
        };
        if node.computed() != Computed::Manual || node.def.outputs[p].ty != SignalType::String {
            return false;
        }
        node.texts_out[p] = text.to_owned();
        node.message_arrived();
        true
    }

    /// Value that reached an input in the last scan.
    pub fn input(&self, block: &str, port: &str) -> Option<f64> {
        let node = &self.nodes[*self.index.get(block)?];
        let p = node.def.inputs.iter().position(|o| o.name == port)?;
        Some(node.inputs[p])
    }

    pub fn computed(&self, block: &str) -> Option<Computed> {
        Some(self.nodes[*self.index.get(block)?].computed())
    }

    /// Log lines written by the module of a block since the last call.
    pub fn take_log(&mut self, block: &str) -> Vec<String> {
        let Some(i) = self.index.get(block) else {
            return Vec::new();
        };
        self.nodes[*i]
            .module
            .as_mut()
            .map(SimModule::take_log)
            .unwrap_or_default()
    }

    /// Replaces the module of a block with a freshly built one; its state starts anew.
    pub fn load_module(&mut self, block: &str, wasm: &[u8]) -> Result<(), String> {
        let i = *self
            .index
            .get(block)
            .ok_or_else(|| format!("нет блока «{block}»"))?;
        let node = &mut self.nodes[i];
        node.module = Some(SimModule::load(wasm, node.def.outputs.len())?);
        node.behavior = None;
        Ok(())
    }

    /// Type of an output as the block declares it.
    pub fn output_type(&self, block: &str, port: &str) -> Option<SignalType> {
        let node = &self.nodes[*self.index.get(block)?];
        Some(node.def.outputs.iter().find(|o| o.name == port)?.ty)
    }

    /// Outputs nobody computes, with their values: (block, port, value).
    pub fn manual_outputs(&self) -> Vec<(String, String, f64)> {
        self.nodes
            .iter()
            .filter(|n| n.computed() == Computed::Manual)
            .flat_map(|n| {
                n.def
                    .outputs
                    .iter()
                    .zip(&n.outputs)
                    .map(|(p, v)| (n.id.clone(), p.name.clone(), *v))
            })
            .collect()
    }

    /// String outputs nobody computes, with their texts: (block, port, text).
    pub fn manual_texts(&self) -> Vec<(String, String, String)> {
        self.nodes
            .iter()
            .filter(|n| n.computed() == Computed::Manual)
            .flat_map(|n| {
                n.def
                    .outputs
                    .iter()
                    .zip(&n.texts_out)
                    .filter(|(p, _)| p.ty == SignalType::String)
                    .map(|(p, t)| (n.id.clone(), p.name.clone(), t.clone()))
            })
            .collect()
    }

    /// Reloads the modules whose `block.wasm` changed on disk; the state of a reloaded
    /// block starts anew. A file that does not load keeps the previous module.
    /// Returns what happened, one line per block, for the log.
    pub fn reload_changed(&mut self) -> Vec<String> {
        let mut report = Vec::new();
        for node in &mut self.nodes {
            let Some((path, stamp)) = &mut node.wasm else {
                continue;
            };
            let now = modified(path);
            if now.is_none() || now == *stamp {
                continue;
            }
            *stamp = now;
            let loaded = std::fs::read(&*path)
                .map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))
                .and_then(|bytes| SimModule::load(&bytes, node.def.outputs.len()));
            match loaded {
                Ok(module) => {
                    node.module = Some(module);
                    node.behavior = None;
                    report.push(format!("{}: модуль перезагружен", node.id));
                }
                Err(e) => report.push(format!("{}: {e}", node.id)),
            }
        }
        report
    }
}
