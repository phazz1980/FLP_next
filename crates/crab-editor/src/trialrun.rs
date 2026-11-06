//! "Trial run" tab of the block editor: the block's simulation module `block.wasm`
//! runs on this computer; inputs and parameters are typed by hand, outputs are
//! shown after each step. The file is watched and reloaded when it changes.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crab_core::library::WASM_FILE;
use crab_sim::SimModule;
use eframe::egui::{self, Color32, RichText, Ui};

use crate::theme::Palette;

/// How often the module file is checked for changes.
const POLL: Duration = Duration::from_millis(300);
/// Lines of the module log kept on screen.
const LOG_LINES: usize = 200;

/// What the tab needs to know about the block form: names with starting values.
pub struct Shape<'a> {
    pub inputs: Vec<(&'a str, f64)>,
    pub params: Vec<(&'a str, f64)>,
    pub outputs: Vec<&'a str>,
}

pub struct TrialRun {
    module: Option<SimModule>,
    path: Option<PathBuf>,
    stamp: Option<SystemTime>,
    last_poll: Option<Instant>,
    /// Note about the last load: success or error (on error the previous module stays).
    status: Result<String, String>,
    inputs: Vec<f64>,
    params: Vec<f64>,
    outputs: Vec<f64>,
    dt_ms: u32,
    running: bool,
    last_step: Option<Instant>,
    elapsed_ms: u64,
    log: Vec<String>,
}

impl Default for TrialRun {
    fn default() -> Self {
        Self {
            module: None,
            path: None,
            stamp: None,
            last_poll: None,
            status: Ok(String::new()),
            inputs: Vec::new(),
            params: Vec::new(),
            outputs: Vec::new(),
            dt_ms: 100,
            running: false,
            last_step: None,
            elapsed_ms: 0,
            log: Vec::new(),
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl TrialRun {
    /// Loads or reloads `block.wasm` when the file appeared or changed.
    fn refresh(&mut self, dir: Option<&Path>, n_outputs: usize) {
        let path = dir.map(|d| d.join(WASM_FILE));
        if path != self.path {
            *self = Self {
                path: path.clone(),
                dt_ms: self.dt_ms,
                ..Self::default()
            };
        }
        let Some(path) = path else { return };
        if self.last_poll.is_some_and(|t| t.elapsed() < POLL) {
            return;
        }
        self.last_poll = Some(Instant::now());
        let stamp = modified(&path);
        if stamp.is_none() {
            self.stamp = None;
            self.module = None;
            return;
        }
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        let loaded = std::fs::read(&path)
            .map_err(|e| format!("не удалось прочитать {WASM_FILE}: {e}"))
            .and_then(|bytes| SimModule::load(&bytes, n_outputs));
        match loaded {
            Ok(module) => {
                self.module = Some(module);
                self.outputs = vec![0.0; n_outputs];
                self.elapsed_ms = 0;
                self.status = Ok(crate::i18n::tr("Модуль загружен").to_owned());
            }
            // The previous module keeps working.
            Err(e) => self.status = Err(e),
        }
    }

    fn step(&mut self) {
        let Some(module) = &mut self.module else {
            return;
        };
        match module.step(self.dt_ms, &self.inputs, &self.params) {
            Ok(out) => {
                self.outputs = out;
                self.elapsed_ms += u64::from(self.dt_ms);
            }
            Err(e) => {
                self.status = Err(e);
                self.running = false;
            }
        }
        self.log.extend(module.take_log());
        if self.log.len() > LOG_LINES {
            self.log.drain(..self.log.len() - LOG_LINES);
        }
    }

    fn restart(&mut self) {
        if let Some(module) = &mut self.module {
            if let Err(e) = module.reset() {
                self.status = Err(e);
            }
            self.outputs.iter_mut().for_each(|v| *v = 0.0);
            self.elapsed_ms = 0;
            self.log.clear();
        }
    }
}

pub fn show(ui: &mut Ui, run: &mut TrialRun, dir: Option<&Path>, shape: &Shape, pal: &Palette) {
    run.refresh(dir, shape.outputs.len());
    // Entered values stay while the port lists keep their size; new rows get defaults.
    sync(&mut run.inputs, &shape.inputs);
    sync(&mut run.params, &shape.params);
    if run.outputs.len() != shape.outputs.len() {
        run.outputs.resize(shape.outputs.len(), 0.0);
    }

    if dir.is_none() {
        ui.weak(crate::i18n::tr(
            "Сначала сохраните блок: модуль block.wasm кладётся в папку блока.",
        ));
        return;
    }
    if run.module.is_none() {
        ui.label(crate::i18n::fmt(
            "Положите {0} в папку блока — он загрузится сам и перезагрузится при каждой пересборке.",
            &[&WASM_FILE],
        ));
        if let Err(e) = &run.status {
            ui.colored_label(pal.error, e);
        }
        ui.ctx().request_repaint_after(POLL);
        return;
    }

    match &run.status {
        Ok(note) if !note.is_empty() => {
            ui.colored_label(pal.ok, note);
        }
        Ok(_) => {}
        Err(e) => {
            ui.colored_label(pal.error, e);
        }
    }
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Шаг, мс"));
        ui.add(egui::DragValue::new(&mut run.dt_ms).range(1..=60_000));
        if ui.button(crate::i18n::tr("Шаг")).clicked() {
            run.step();
        }
        let label = if run.running {
            crate::i18n::tr("Стоп")
        } else {
            crate::i18n::tr("Пуск")
        };
        if ui.button(label).clicked() {
            run.running = !run.running;
            run.last_step = Some(Instant::now());
        }
        if ui.button(crate::i18n::tr("Сначала")).clicked() {
            run.restart();
        }
        ui.weak(crate::i18n::fmt("Время: {0} мс", &[&run.elapsed_ms]));
    });
    if run.running {
        let due = run
            .last_step
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(u64::from(run.dt_ms)));
        if due {
            run.last_step = Some(Instant::now());
            run.step();
        }
        ui.ctx()
            .request_repaint_after(Duration::from_millis(u64::from(run.dt_ms)));
    } else {
        ui.ctx().request_repaint_after(POLL);
    }

    ui.add_space(6.0);
    let outs: Vec<(&str, f64)> = shape.outputs.iter().map(|n| (*n, 0.0)).collect();
    ui.columns(3, |cols| {
        column(&mut cols[0], "Входы", &shape.inputs, &mut run.inputs, true);
        column(
            &mut cols[1],
            "Параметры",
            &shape.params,
            &mut run.params,
            true,
        );
        column(&mut cols[2], "Выходы", &outs, &mut run.outputs, false);
    });
    if !run.log.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new(crate::i18n::tr("Журнал модуля")).strong());
        let mut text = run.log.join("\n");
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .code_editor()
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .text_color(Color32::GRAY),
        );
    }
}

fn sync(values: &mut Vec<f64>, rows: &[(&str, f64)]) {
    if values.len() != rows.len() {
        let old = std::mem::take(values);
        *values = rows
            .iter()
            .enumerate()
            .map(|(i, r)| old.get(i).copied().unwrap_or(r.1))
            .collect();
    }
}

fn column(ui: &mut Ui, title: &str, rows: &[(&str, f64)], values: &mut [f64], editable: bool) {
    ui.label(RichText::new(crate::i18n::tr(title)).strong());
    if rows.is_empty() {
        ui.weak("—");
    }
    egui::Grid::new(title).num_columns(2).show(ui, |ui| {
        for ((name, _), value) in rows.iter().zip(values.iter_mut()) {
            ui.label(*name);
            if editable {
                ui.add(egui::DragValue::new(value).speed(0.1));
            } else {
                ui.monospace(format!("{value}"));
            }
            ui.end_row();
        }
    });
}
