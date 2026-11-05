//! Browser demo: view an example schema, build it (check and sketch) and simulate it.
//! View only: the schema cannot be edited here.

mod data {
    include!(concat!(env!("OUT_DIR"), "/data.rs"));
}
mod sheet;
#[cfg(target_arch = "wasm32")]
mod web;

use eframe::egui;
use flp_codegen::ArduinoCpp;
use flp_core::{Library, Project};
use flp_sim::{BenchInput, Computed, Simulation};

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
        };
        app.start_sim();
        app
    }

    fn open(&mut self, project: Project) {
        self.project = project;
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
        let report = flp_core::check(&self.project.logic, &self.lib, Some(&self.project.layout));
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
        self.sketch = match flp_codegen::generate(&self.project, &self.lib, &ArduinoCpp::RELEASE) {
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

    fn inputs_panel(&mut self, ui: &mut egui::Ui) {
        let Some(sim) = self.sim.as_mut() else { return };
        ui.heading("Входы");
        let mut any = false;
        for block in &self.project.logic.blocks {
            let Some(def) = self.lib.resolve(&self.project.logic, block) else {
                continue;
            };
            for port in &def.def.outputs {
                let input = sim.bench_input(&block.id, &port.name);
                match &input {
                    Some(BenchInput::Digital { pin }) => {
                        any = true;
                        let mut on = sim.bench().level(*pin).unwrap_or(0.0) > 0.5;
                        if ui.checkbox(&mut on, &block.id).changed() {
                            sim.set_pin_level(*pin, f64::from(u8::from(on)));
                        }
                    }
                    Some(BenchInput::Analog { pin }) => {
                        any = true;
                        let mut v = sim.bench().level(*pin).unwrap_or(0.0);
                        ui.label(&block.id);
                        if ui.add(egui::Slider::new(&mut v, 0.0..=4095.0)).changed() {
                            sim.set_pin_level(*pin, v);
                        }
                    }
                    Some(BenchInput::Sensor { pin, channel }) => {
                        any = true;
                        let mut v = sim.bench().signal(*pin, channel).at(0);
                        ui.label(format!("{} · {}", block.id, port.title()));
                        if ui.add(egui::Slider::new(&mut v, -20.0..=100.0)).changed() {
                            sim.set_signal(*pin, channel, flp_sim::Signal::constant(v));
                        }
                    }
                    _ => {}
                }
                if input.is_none()
                    && sim.computed(&block.id) == Some(Computed::Manual)
                    && port.ty != flp_core::SignalType::String
                {
                    any = true;
                    let mut v = sim.output(&block.id, &port.name).unwrap_or(0.0);
                    ui.label(format!("{} · {}", block.id, port.title()));
                    if ui.add(egui::DragValue::new(&mut v)).changed() {
                        sim.set_output(&block.id, &port.name, v);
                    }
                }
            }
        }
        if !any {
            ui.label("У этой схемы нет входов, которыми можно управлять.");
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
                if ui.button("Открыть .flpn…").clicked() {
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
                egui::ScrollArea::vertical().show(ui, |ui| self.inputs_panel(ui));
            });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.error);
            }
            match self.tab {
                Tab::Sheet => sheet::show(ui, &self.project, &self.lib, self.sim.as_ref()),
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
            let code = flp_codegen::generate(&project, &lib, &ArduinoCpp::RELEASE)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(code.contains("void loop()"), "{name}");
            let mut sim = Simulation::new(&project.without_disabled().logic, &lib)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            sim.step(100).unwrap();
        }
    }
}
