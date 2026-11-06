//! Browser demo: view an example schema, build it (check and sketch) and simulate it.
//! View only: the schema cannot be edited here.

mod data {
    include!(concat!(env!("OUT_DIR"), "/data.rs"));
}
mod sheet;
#[cfg(target_arch = "wasm32")]
mod web;

use crab_codegen::ArduinoCpp;
use crab_core::{Library, ParamValue, Project};
use crab_sim::{BenchInput, Computed, Simulation};
use eframe::egui;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Sheet,
    Build,
}

pub struct DemoApp {
    lib: Library,
    project: Project,
    example: usize,
    tab: Tab,
    /// Result of the build: report lines and the sketch.
    report: String,
    sketch: String,
    sim: Option<Simulation>,
    running: bool,
    speed: f64,
    pending_ms: f64,
    error: String,
    /// Block whose parameters the right panel shows.
    selected: Option<String>,
}

impl DemoApp {
    pub fn new() -> Self {
        let (lib, _) = Library::from_sources(data::BLOCKS.iter().copied());
        let project = load_example(0);
        let mut app = Self {
            lib,
            project,
            example: 0,
            tab: Tab::Sheet,
            report: String::new(),
            sketch: String::new(),
            sim: None,
            running: true,
            speed: 1.0,
            pending_ms: 0.0,
            error: String::new(),
            selected: None,
        };
        app.start_sim();
        app
    }

    fn open(&mut self, project: Project) {
        self.project = project;
        self.selected = None;
        self.report.clear();
        self.sketch.clear();
        self.tab = Tab::Sheet;
        self.start_sim();
    }

    fn start_sim(&mut self) {
        self.pending_ms = 0.0;
        match Simulation::new(&self.project.without_disabled().logic, &self.lib) {
            Ok(sim) => {
                self.sim = Some(sim);
                self.error.clear();
            }
            Err(e) => {
                self.sim = None;
                self.error = e;
            }
        }
    }

    fn build(&mut self) {
        let report = crab_core::check(&self.project.logic, &self.lib, Some(&self.project.layout));
        self.report = if report.diagnostics.is_empty() {
            "Схема без замечаний.".to_owned()
        } else {
            report
                .diagnostics
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.sketch = match crab_codegen::generate(&self.project, &self.lib, &ArduinoCpp::RELEASE) {
            Ok(code) => code,
            Err(e) => format!("// Скетч не собран: {e}"),
        };
        self.tab = Tab::Build;
    }

    fn tick(&mut self, dt: f64) {
        let Some(sim) = self.sim.as_mut() else { return };
        if !self.running {
            return;
        }
        self.pending_ms = (self.pending_ms + dt * 1000.0 * self.speed).min(2000.0);
        while self.pending_ms >= 10.0 {
            self.pending_ms -= 10.0;
            if let Err(e) = sim.step(10) {
                self.error = e;
                self.running = false;
                break;
            }
        }
    }

    /// Parameters and outputs of the selected block; both can be changed by hand.
    fn block_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(block) = self.project.logic.block(&id).cloned() else {
            return;
        };
        let Some(resolved) = self.lib.resolve(&self.project.logic, &block) else {
            return;
        };
        let def = resolved.def.clone();
        ui.heading(format!("Блок «{id}»"));
        ui.label(egui::RichText::new(&def.info.name).strong());
        if !def.info.description.is_empty() {
            ui.label(&def.info.description);
        }
        let mut changed: Option<(String, ParamValue)> = None;
        let params: Vec<_> = def.active_params(&block.params).collect();
        if !params.is_empty() {
            ui.separator();
            ui.strong("Параметры");
            egui::Grid::new("block_params")
                .num_columns(2)
                .show(ui, |ui| {
                    for p in params {
                        let current = block.params.get(&p.name).or(p.default.as_ref()).cloned();
                        ui.label(p.title()).on_hover_text(&p.description);
                        if let Some(v) = param_editor(ui, p, current) {
                            changed = Some((p.name.clone(), v));
                        }
                        ui.end_row();
                    }
                });
        }
        if let Some((name, value)) = changed {
            if let Some(b) = self.project.logic.blocks.iter_mut().find(|b| b.id == id) {
                b.params.insert(name, value);
            }
            // The schema is rebuilt with the new value; the run starts over.
            self.start_sim();
        }
        let Some(sim) = self.sim.as_mut() else { return };
        let outs: Vec<_> = def.active_outputs(&block.params).collect();
        if !outs.is_empty() {
            ui.separator();
            ui.strong("Выходы");
            egui::Grid::new("block_outs").num_columns(2).show(ui, |ui| {
                for o in outs {
                    ui.label(o.title());
                    if !output_control(ui, sim, &id, o, None) {
                        let v = sim.output(&id, &o.name);
                        ui.label(v.map_or("—".to_owned(), |v| format!("{v}")));
                    }
                    ui.end_row();
                }
            });
        }
        ui.separator();
    }

    fn inputs_panel(&mut self, ui: &mut egui::Ui) {
        let Some(sim) = self.sim.as_mut() else { return };
        ui.heading("Входы");
        let mut any = false;
        for block in &self.project.logic.blocks {
            let Some(def) = self.lib.resolve(&self.project.logic, block) else {
                continue;
            };
            for port in &def.def.outputs {
                let caption = format!("{} · {}", block.id, port.title());
                any |= output_control(ui, sim, &block.id, port, Some(&caption));
            }
        }
        if !any {
            ui.label("У этой схемы нет входов, которыми можно управлять.");
        }
    }
}

/// Control that sets an output by hand: a pin level for board inputs, a value for blocks
/// nobody computes. `caption` is put above it when given. Returns whether the output
/// can be set at all.
fn output_control(
    ui: &mut egui::Ui,
    sim: &mut Simulation,
    block: &str,
    port: &crab_core::PortDef,
    caption: Option<&str>,
) -> bool {
    let input = sim.bench_input(block, &port.name);
    let label = |ui: &mut egui::Ui| {
        if let Some(c) = caption {
            ui.label(c);
        }
    };
    match &input {
        Some(BenchInput::Digital { pin }) => {
            let mut on = sim.bench().level(*pin).unwrap_or(0.0) > 0.5;
            if ui.checkbox(&mut on, caption.unwrap_or("")).changed() {
                sim.set_pin_level(*pin, f64::from(u8::from(on)));
            }
            true
        }
        Some(BenchInput::Analog { pin }) => {
            let mut v = sim.bench().level(*pin).unwrap_or(0.0);
            label(ui);
            if ui.add(egui::Slider::new(&mut v, 0.0..=4095.0)).changed() {
                sim.set_pin_level(*pin, v);
            }
            true
        }
        Some(BenchInput::Sensor { pin, channel }) => {
            let mut v = sim.bench().signal(*pin, channel).at(0);
            label(ui);
            if ui.add(egui::Slider::new(&mut v, -20.0..=100.0)).changed() {
                sim.set_signal(*pin, channel, crab_sim::Signal::constant(v));
            }
            true
        }
        _ if sim.computed(block) == Some(Computed::Manual)
            && port.ty != crab_core::SignalType::String =>
        {
            let mut v = sim.output(block, &port.name).unwrap_or(0.0);
            label(ui);
            if ui.add(egui::DragValue::new(&mut v)).changed() {
                sim.set_output(block, &port.name, v);
            }
            true
        }
        _ => false,
    }
}

/// Editor of a parameter value by its kind; returns the new value when it changed.
fn param_editor(
    ui: &mut egui::Ui,
    p: &crab_core::ParamDef,
    current: Option<ParamValue>,
) -> Option<ParamValue> {
    use crab_core::ParamKind;
    if !p.choices.is_empty() {
        let mut picked = None;
        let shown = p
            .choices
            .iter()
            .find(|c| Some(&c.value) == current.as_ref())
            .map_or_else(|| "—".to_owned(), |c| c.label.clone());
        egui::ComboBox::from_id_salt(("param", &p.name))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for c in &p.choices {
                    if ui
                        .selectable_label(Some(&c.value) == current.as_ref(), &c.label)
                        .clicked()
                    {
                        picked = Some(c.value.clone());
                    }
                }
            });
        return picked;
    }
    match p.kind {
        ParamKind::Bool => {
            let mut v = matches!(current, Some(ParamValue::Bool(true)));
            ui.checkbox(&mut v, "")
                .changed()
                .then_some(ParamValue::Bool(v))
        }
        ParamKind::Int | ParamKind::Pin => {
            let mut v = match current {
                Some(ParamValue::Int(i)) => i,
                Some(ParamValue::Float(f)) => f as i64,
                _ => 0,
            };
            let mut drag = egui::DragValue::new(&mut v);
            if let (Some(lo), Some(hi)) = (p.min, p.max) {
                drag = drag.range(lo as i64..=hi as i64);
            }
            ui.add(drag).changed().then_some(ParamValue::Int(v))
        }
        ParamKind::Float => {
            let mut v = match current {
                Some(ParamValue::Float(f)) => f,
                Some(ParamValue::Int(i)) => i as f64,
                _ => 0.0,
            };
            ui.add(egui::DragValue::new(&mut v).speed(0.1))
                .changed()
                .then_some(ParamValue::Float(v))
        }
        ParamKind::Text | ParamKind::Duration => {
            let mut v = current.map(|c| c.to_string()).unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0))
                .changed()
                .then_some(ParamValue::Text(v))
        }
    }
}

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

fn load_example(i: usize) -> Project {
    let (name, project, logic, layout) = data::EXAMPLES[i];
    Project::from_sources(name, project, logic, layout).expect("embedded example")
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.1);
        self.tick(dt);
        ctx.request_repaint();
        #[cfg(target_arch = "wasm32")]
        if let Some(p) = web::take_loaded() {
            self.open(p);
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("CrabLogic · демо");
                egui::ComboBox::from_id_salt("example")
                    .selected_text(data::EXAMPLES[self.example].0)
                    .show_ui(ui, |ui| {
                        for (i, e) in data::EXAMPLES.iter().enumerate() {
                            if ui.selectable_label(i == self.example, e.0).clicked() {
                                self.example = i;
                                self.open(load_example(i));
                            }
                        }
                    });
                #[cfg(target_arch = "wasm32")]
                if ui.button("Открыть .crab…").clicked() {
                    web::pick_project();
                }
                ui.separator();
                if ui.button("Сборка").clicked() {
                    self.build();
                }
                if ui.button("Схема").clicked() {
                    self.tab = Tab::Sheet;
                }
                ui.separator();
                let label = if self.running {
                    "Пауза"
                } else {
                    "Пуск"
                };
                if ui.button(label).clicked() {
                    self.running = !self.running;
                }
                if ui.button("Шаг 100 мс").clicked() {
                    if let Some(sim) = self.sim.as_mut() {
                        let _ = sim.step(100);
                    }
                }
                if ui.button("Сброс").clicked() {
                    self.start_sim();
                }
                ui.label("Скорость");
                for s in [1.0, 10.0, 100.0] {
                    ui.selectable_value(&mut self.speed, s, format!("{s}×"));
                }
                if let Some(sim) = &self.sim {
                    ui.label(format!("t = {:.1} с", sim.time_ms() as f64 / 1000.0));
                }
            });
        });

        egui::Panel::right("inputs")
            .default_size(240.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.block_panel(ui);
                    self.inputs_panel(ui);
                });
            });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.error);
            }
            match self.tab {
                Tab::Sheet => {
                    let hit = sheet::show(
                        ui,
                        &self.project,
                        &self.lib,
                        self.sim.as_ref(),
                        self.selected.as_deref(),
                    );
                    if let Some(id) = hit {
                        self.selected = id;
                    }
                }
                Tab::Build => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.label(egui::RichText::new(&self.report).monospace());
                        ui.separator();
                        #[cfg(target_arch = "wasm32")]
                        if ui.button("Скачать .ino").clicked() {
                            web::download(
                                &format!("{}.ino", self.project.file_stem()),
                                &self.sketch,
                            );
                        }
                        ui.add(
                            egui::TextEdit::multiline(&mut self.sketch.as_str())
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_checks_builds_and_simulates() {
        let (lib, errors) = Library::from_sources(data::BLOCKS.iter().copied());
        assert!(errors.is_empty(), "{errors:?}");
        for i in 0..data::EXAMPLES.len() {
            let name = data::EXAMPLES[i].0;
            let project = load_example(i);
            let code = crab_codegen::generate(&project, &lib, &ArduinoCpp::RELEASE)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(code.contains("void loop()"), "{name}");
            let mut sim = Simulation::new(&project.without_disabled().logic, &lib)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            sim.step(100).unwrap();
        }
    }
}
