//! Simulation of the whole schema in the editor: the schema runs on this computer,
//! values are drawn on the sheets like online debugging does; inputs are set by hand.

use std::time::{Duration, Instant};

use eframe::egui::{self, Ui};
use flp_core::{Library, PortRef, Project, SignalType};
use flp_debug::Value;
use flp_sim::{BenchInput, Computed, Signal, Simulation};

use crate::theme::Palette;

/// Simulated time of one scan while running.
const SCAN_MS: u32 = 10;
/// Scans per frame at most, so a high speed cannot freeze the window.
const MAX_SCANS: u32 = 500;
/// How often the module files are checked for changes.
const RELOAD_POLL: Duration = Duration::from_millis(500);
/// Lines of messages kept in the window.
const NOTE_LINES: usize = 50;

pub struct SimState {
    sim: Simulation,
    /// Schema revision the simulation was built from.
    revision: u64,
    running: bool,
    /// Simulated seconds per real second.
    speed: f64,
    /// Simulated time not yet played, ms.
    pending_ms: f64,
    last_tick: Option<Instant>,
    last_reload: Instant,
    /// Length of the manual step button, ms.
    step_ms: u32,
    notes: Vec<String>,
}

fn build(project: &Project, lib: &Library) -> Result<Simulation, String> {
    Simulation::new(&project.without_disabled().logic, lib)
}

impl SimState {
    pub fn start(project: &Project, lib: &Library, revision: u64) -> Result<Self, String> {
        let sim = build(project, lib)?;
        let notes = sim.notes.clone();
        Ok(Self {
            sim,
            revision,
            running: true,
            speed: 1.0,
            pending_ms: 0.0,
            last_tick: None,
            last_reload: Instant::now(),
            step_ms: 100,
            notes,
        })
    }

    fn note(&mut self, text: String) {
        self.notes.push(text);
        if self.notes.len() > NOTE_LINES {
            self.notes.drain(..self.notes.len() - NOTE_LINES);
        }
    }

    /// Rebuilds the simulation after the schema changed; hand-set inputs stay.
    pub fn follow(&mut self, project: &Project, lib: &Library, revision: u64) {
        if revision == self.revision {
            return;
        }
        self.revision = revision;
        match build(project, lib) {
            Ok(mut sim) => {
                for (block, port, value) in self.sim.manual_outputs() {
                    sim.set_output(&block, &port, value);
                }
                for (block, port, text) in self.sim.manual_texts() {
                    sim.set_output_text(&block, &port, &text);
                }
                sim.keep_vars(&self.sim);
                sim.keep_bench(&self.sim);
                self.notes = sim.notes.clone();
                self.sim = sim;
            }
            Err(e) => {
                self.running = false;
                self.note(e);
            }
        }
    }

    /// Plays the time that passed since the last frame.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if self.last_reload.elapsed() >= RELOAD_POLL {
            self.last_reload = Instant::now();
            for line in self.sim.reload_changed() {
                self.note(line);
            }
        }
        let now = Instant::now();
        let dt = self.last_tick.replace(now).map(|t| now - t);
        if self.running {
            self.pending_ms += dt.unwrap_or_default().as_secs_f64() * 1000.0 * self.speed;
            let mut scans = 0;
            while self.pending_ms >= f64::from(SCAN_MS) && scans < MAX_SCANS {
                self.pending_ms -= f64::from(SCAN_MS);
                scans += 1;
                if let Err(e) = self.sim.step(SCAN_MS) {
                    self.running = false;
                    self.note(e);
                    break;
                }
            }
            // Time the window could not play is dropped, not piled up.
            self.pending_ms = self.pending_ms.min(f64::from(SCAN_MS));
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(RELOAD_POLL);
        }
    }

    /// Live value of a project variable, typed by its declaration.
    pub fn var(&self, logic: &flp_core::Logic, name: &str) -> Option<(Value, bool)> {
        if logic.var(name)?.ty.signal() == SignalType::String {
            return None;
        }
        let v = self.sim.var(name)?;
        Some((
            match logic.var(name)?.ty.signal() {
                SignalType::Bool => Value::Bool(v != 0.0),
                SignalType::Int => Value::Int(v as i32),
                _ => Value::Float(v as f32),
            },
            false,
        ))
    }

    /// The pin an output block drives on the virtual bench and its state.
    pub fn output_pin(&self, block: &str) -> Option<(i64, f64)> {
        self.sim.output_pin(block)
    }

    /// Network faults of the virtual bench.
    pub fn offline(&self) -> bool {
        self.sim.bench().offline()
    }

    pub fn broker_down(&self) -> bool {
        self.sim.bench().broker_down()
    }

    pub fn set_offline(&mut self, on: bool) {
        self.sim.set_offline(on);
    }

    pub fn set_broker_down(&mut self, on: bool) {
        self.sim.set_broker_down(on);
    }

    /// Live text of a string output.
    pub fn text_output(&self, port: &PortRef) -> Option<&str> {
        self.sim.output_text(&port.block, &port.port)
    }

    /// Live value of a block output: (value, forced) as the debugging view expects.
    pub fn output(&self, port: &PortRef) -> Option<(Value, bool)> {
        let v = self.sim.output(&port.block, &port.port)?;
        let ty = self.sim.output_type(&port.block, &port.port)?;
        Some((
            match ty {
                SignalType::String => return None,
                SignalType::Bool => Value::Bool(v != 0.0),
                SignalType::Int => Value::Int(v as i32),
                _ => Value::Float(v as f32),
            },
            false,
        ))
    }
}

/// Whether the output is driven by hand (so a click on its value may change it).
pub fn editable(sim: &SimState, port: &PortRef) -> bool {
    sim.sim.computed(&port.block) == Some(Computed::Manual)
        || sim.sim.block_var(&port.block).is_some()
        || sim.sim.bench_input(&port.block, &port.port).is_some()
}

/// Menu of an output that reads the virtual bench: the pin level or the sensor signal.
fn bench_menu(ui: &mut Ui, sim: &mut SimState, port: &PortRef, input: BenchInput) {
    ui.label(egui::RichText::new(port.to_string()).strong());
    match input {
        BenchInput::Digital { pin } => {
            let high = sim.sim.bench().level(pin).map(|v| v != 0.0);
            ui.label(crate::i18n::fmt("Пин {0}", &[&pin]));
            for (level, name) in [(1.0, "Высокий уровень"), (0.0, "Низкий уровень")]
            {
                let current = high == Some(level != 0.0);
                if ui
                    .selectable_label(current, crate::i18n::tr(name))
                    .clicked()
                {
                    sim.sim.set_pin_level(pin, level);
                    ui.close();
                }
            }
        }
        BenchInput::Analog { pin } => {
            let mut counts = sim.sim.bench().level(pin).unwrap_or(0.0);
            ui.label(crate::i18n::fmt("Пин {0}: отсчёты АЦП", &[&pin]));
            if ui
                .add(
                    egui::DragValue::new(&mut counts)
                        .range(0.0..=4095.0)
                        .speed(10.0),
                )
                .changed()
            {
                sim.sim.set_pin_level(pin, counts.round());
            }
        }
        BenchInput::Link => {
            let mut offline = sim.sim.bench().offline();
            if ui
                .checkbox(&mut offline, crate::i18n::tr("Нет связи с сетью"))
                .changed()
            {
                sim.sim.set_offline(offline);
            }
        }
        BenchInput::Broker => {
            let mut down = sim.sim.bench().broker_down();
            if ui
                .checkbox(&mut down, crate::i18n::tr("Брокер не отвечает"))
                .changed()
            {
                sim.sim.set_broker_down(down);
            }
            let mut offline = sim.sim.bench().offline();
            if ui
                .checkbox(&mut offline, crate::i18n::tr("Нет связи с сетью"))
                .changed()
            {
                sim.sim.set_offline(offline);
            }
        }
        BenchInput::Modbus { slave, table, reg } => {
            let mut value = sim.sim.bench().register(slave, &table, reg);
            let own = slave == flp_sim::OWN_SLAVE;
            if own {
                ui.label(crate::i18n::fmt(
                    "Запись мастера: {0}, регистр {1}",
                    &[&table, &reg],
                ));
            } else {
                ui.label(crate::i18n::fmt(
                    "Устройство {0}, {1}, регистр {2}",
                    &[&slave, &table, &reg],
                ));
            }
            if ui
                .add(egui::DragValue::new(&mut value).speed(0.1))
                .changed()
            {
                sim.sim.set_register(slave, &table, reg, value);
            }
            let mut silent = sim.sim.bench().silent(slave);
            if !own
                && ui
                    .checkbox(&mut silent, crate::i18n::tr("Нет ответа"))
                    .changed()
            {
                sim.sim.set_silent(slave, silent);
            }
        }
        BenchInput::Uart { port } => {
            let id = ui.make_persistent_id(("uart_feed", port));
            let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
            ui.label(crate::i18n::fmt("Ввод в порт {0}", &[&port]));
            let edit = ui.text_edit_singleline(&mut text);
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(crate::i18n::tr("Отправить")).clicked() || enter {
                sim.sim.send_uart(port, &text);
            }
            ui.data_mut(|d| d.insert_temp(id, text));
        }
        BenchInput::Sensor { pin, channel } => {
            let mut sig = sim.sim.bench().signal(pin, &channel);
            let mut over_s = sig.duration_ms as f64 / 1000.0;
            let mut changed = false;
            ui.label(crate::i18n::fmt("Пин {0}", &[&pin]));
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("От"));
                changed |= ui
                    .add(egui::DragValue::new(&mut sig.from).speed(0.1))
                    .changed();
                ui.label(crate::i18n::tr("до"));
                changed |= ui
                    .add(egui::DragValue::new(&mut sig.to).speed(0.1))
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("За, с"));
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut over_s)
                            .range(0.0..=86_400.0)
                            .speed(0.5),
                    )
                    .changed();
            });
            changed |= ui
                .checkbox(&mut sig.fault, crate::i18n::tr("Ошибка датчика"))
                .changed();
            if changed {
                sim.sim.set_signal(
                    pin,
                    &channel,
                    Signal {
                        duration_ms: (over_s * 1000.0).round() as u64,
                        ..sig
                    },
                );
            }
        }
    }
}

/// Menu of a port in simulation: set the value of a hand-driven output.
pub fn port_menu(ui: &mut Ui, sim: &mut SimState, port: &PortRef) {
    if let Some(input) = sim.sim.bench_input(&port.block, &port.port) {
        bench_menu(ui, sim, port, input);
        return;
    }
    let var = sim.sim.block_var(&port.block).map(str::to_owned);
    if var.is_none() && sim.sim.computed(&port.block) != Some(Computed::Manual) {
        return;
    }
    if sim.sim.output_type(&port.block, &port.port) == Some(SignalType::String) {
        let Some(mut text) = sim
            .sim
            .output_text(&port.block, &port.port)
            .map(str::to_owned)
        else {
            return;
        };
        ui.label(egui::RichText::new(port.to_string()).strong());
        // A click in the field must not close the menu: it lives in a submenu that
        // closes only on a click outside.
        let config = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        egui::containers::menu::SubMenuButton::new(crate::i18n::tr("Текст"))
            .config(config)
            .ui(ui, |ui| {
                ui.set_min_width(180.0);
                if ui.text_edit_singleline(&mut text).changed() {
                    match &var {
                        Some(var) => sim.sim.set_var_text(var, &text),
                        None => sim.sim.set_output_text(&port.block, &port.port, &text),
                    };
                }
            });
        return;
    }
    let (Some(ty), Some(mut v)) = (
        sim.sim.output_type(&port.block, &port.port),
        sim.sim.output(&port.block, &port.port),
    ) else {
        return;
    };
    ui.label(egui::RichText::new(port.to_string()).strong());
    if ty == SignalType::Bool {
        for on in [true, false] {
            if ui
                .button(crate::i18n::fmt("Задать {0}", &[&u8::from(on)]))
                .clicked()
            {
                let value = f64::from(u8::from(on));
                match &var {
                    Some(var) => sim.sim.set_var(var, value),
                    None => sim.sim.set_output(&port.block, &port.port, value),
                };
                ui.close();
            }
        }
    } else if ui
        .add(egui::DragValue::new(&mut v).speed(if ty == SignalType::Int { 1.0 } else { 0.1 }))
        .changed()
    {
        let v = if ty == SignalType::Int { v.round() } else { v };
        match &var {
            Some(var) => sim.sim.set_var(var, v),
            None => sim.sim.set_output(&port.block, &port.port, v),
        };
    }
}

/// Control window: run, pause, step, speed, time, messages. Returns `false` when closed.
pub fn window(sim: &mut SimState, pal: &Palette, ctx: &egui::Context) -> bool {
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Симуляция"))
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label = if sim.running {
                    crate::i18n::tr("Пауза")
                } else {
                    crate::i18n::tr("Пуск")
                };
                if ui.button(label).clicked() {
                    sim.running = !sim.running;
                }
                if ui
                    .add_enabled(!sim.running, egui::Button::new(crate::i18n::tr("Шаг")))
                    .clicked()
                {
                    if let Err(e) = sim.sim.step(sim.step_ms) {
                        sim.note(e);
                    }
                }
                if ui.button(crate::i18n::tr("Сначала")).clicked() {
                    if let Err(e) = sim.sim.reset() {
                        sim.note(e);
                    }
                    sim.pending_ms = 0.0;
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Скорость"));
                for speed in [0.1, 1.0, 10.0, 100.0] {
                    ui.selectable_value(&mut sim.speed, speed, format!("×{speed}"));
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Шаг, мс"));
                ui.add(egui::DragValue::new(&mut sim.step_ms).range(1..=60_000));
                ui.weak(crate::i18n::fmt(
                    "Время: {0} с",
                    &[&format!("{:.1}", sim.sim.time_ms() as f64 / 1000.0)],
                ));
            });
            ui.weak(crate::i18n::tr(
                "Входы платы и блоки с железом задаются правой кнопкой по их выходу.",
            ));
            for line in &sim.notes {
                ui.colored_label(pal.error, line);
            }
        });
    open
}
