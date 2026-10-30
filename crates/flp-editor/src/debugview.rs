//! Online debugging in the editor: live values on the sheets, forcing,
//! setpoints, a window with inputs and variables and a simple chart window.

use std::time::Instant;

use eframe::egui::{self, Color32, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use flp_codegen::Program;
use flp_core::{Layout, Library, ParamKind, PortRef, Project, SignalType};
use flp_debug::{Command, DebugTable, Live, Session, Source, Value};

use crate::theme::Palette;

pub struct DebugState {
    session: Session,
    pub table: DebugTable,
    pub port: String,
    /// Snapshot taken at the start of each frame.
    pub live: Live,
    /// Entries shown in the chart window.
    pub chart: Vec<usize>,
    /// The cycle time window is open.
    pub diagnostics: bool,
    /// The window with inputs and variables is open.
    pub values: bool,
    /// Outputs of input tags (blocks with a pin that produce values), in schema order.
    pub inputs: Vec<(PortRef, usize)>,
    /// Cycle time reports of the last two minutes, for the diagnostics charts.
    stat_history: Vec<StatSample>,
    started: Instant,
}

/// One cycle time report with the time it arrived (seconds since the session start).
struct StatSample {
    t: f64,
    stats: flp_debug::Stats,
}

/// How long the diagnostics charts keep their samples, seconds.
const STAT_HISTORY_SECS: f64 = 120.0;

impl DebugState {
    pub fn start(
        project: &Project,
        lib: &Library,
        port: &str,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let project = &project.without_disabled();
        let program = Program::build(project, lib).map_err(|e| e.to_string())?;
        let table = flp_codegen::debug::debug_table(&program);
        let inputs = input_ports(project, lib, &table);
        let ctx = ctx.clone();
        let session = Session::start(
            table.clone(),
            flp_debug::transport::serial(port),
            move || ctx.request_repaint(),
        );
        Ok(Self {
            live: session.snapshot(),
            session,
            table,
            port: port.to_owned(),
            chart: Vec::new(),
            diagnostics: false,
            values: false,
            inputs,
            stat_history: Vec::new(),
            started: Instant::now(),
        })
    }

    pub fn refresh(&mut self) {
        self.live = self.session.snapshot();
        self.record_stats();
    }

    /// Keeps each new cycle time report for the diagnostics charts.
    fn record_stats(&mut self) {
        let Some(stats) = self.live.stats.clone().filter(|_| self.online()) else {
            return;
        };
        if self.stat_history.last().is_some_and(|l| l.stats == stats) {
            return;
        }
        let t = self.started.elapsed().as_secs_f64();
        self.stat_history.push(StatSample { t, stats });
        self.stat_history.retain(|s| s.t >= t - STAT_HISTORY_SECS);
    }

    /// Points of one figure of the stored reports.
    fn series(&self, value: impl Fn(&flp_debug::Stats) -> Option<f64>) -> Vec<(f64, f64)> {
        self.stat_history
            .iter()
            .filter_map(|s| value(&s.stats).map(|v| (s.t, v)))
            .collect()
    }

    pub fn stop(self) {
        self.session.stop();
    }

    /// Values are meaningful only when the board runs firmware for this schema.
    pub fn online(&self) -> bool {
        self.live.connected && self.live.schema_matches == Some(true)
    }

    /// Live value of a block output: (value, forced).
    pub fn output(&self, port: &PortRef) -> Option<(Value, bool)> {
        if !self.online() {
            return None;
        }
        let i = self.table.output_index(port)?;
        Some((self.live.values[i]?, self.live.forced[i].is_some()))
    }

    pub fn var(&self, name: &str) -> Option<(Value, bool)> {
        if !self.online() {
            return None;
        }
        let i = self.table.var_index(name)?;
        Some((self.live.values[i]?, self.live.forced[i].is_some()))
    }

    /// Live string on a block output (not forceable).
    pub fn text_output(&self, port: &PortRef) -> Option<&str> {
        if !self.online() {
            return None;
        }
        let i = self.table.text_output_index(port)?;
        self.live.texts[i].as_deref()
    }

    /// IP address the board reports on the `address` output of a Wi-Fi block, if any.
    pub fn device_address(&self, blocks: &[flp_core::Block]) -> Option<String> {
        self.table.texts.iter().enumerate().find_map(|(i, t)| {
            let Source::Output(p) = &t.source else {
                return None;
            };
            let is_wifi =
                p.port == "address" && blocks.iter().any(|b| b.id == p.block && b.kind == "wifi");
            let ip = self.live.texts.get(i)?.as_deref()?.trim();
            (self.online() && is_wifi && !ip.is_empty()).then(|| ip.to_owned())
        })
    }

    pub fn text_var(&self, name: &str) -> Option<&str> {
        if !self.online() {
            return None;
        }
        let i = self.table.text_var_index(name)?;
        self.live.texts[i].as_deref()
    }

    pub fn send(&self, command: Command) {
        self.session.send(command);
    }

    pub fn name(&self, index: usize) -> String {
        match &self.table.entries[index].source {
            Source::Output(p) => p.to_string(),
            Source::Var(v) => v.clone(),
        }
    }
}

/// Outputs of blocks with a pin parameter: hardware inputs and sensors.
fn input_ports(project: &Project, lib: &Library, table: &DebugTable) -> Vec<(PortRef, usize)> {
    let logic = &project.logic;
    let mut out = Vec::new();
    for b in &logic.blocks {
        let Some(def) = lib.resolve(logic, b).map(|l| &l.def) else {
            continue;
        };
        if !def.params.iter().any(|p| p.kind == ParamKind::Pin) {
            continue;
        }
        for port in def.active_outputs(&b.params) {
            let port = PortRef::new(&b.id, &port.name);
            if let Some(i) = table.output_index(&port) {
                out.push((port, i));
            }
        }
    }
    out
}

fn as_f64(v: Value) -> f64 {
    match v {
        Value::Bool(b) => f64::from(u8::from(b)),
        Value::Int(i) => f64::from(i),
        Value::Float(f) => f64::from(f),
    }
}

/// Number field for a new value of entry `index`; remembered between frames,
/// starts from the forced (or current) value.
fn number_edit(ui: &mut Ui, dbg: &DebugState, index: usize, salt: &str) -> Value {
    let ty = dbg.table.entries[index].ty;
    let id = Id::unique(("debug_new_value", salt, index));
    let current = dbg.live.forced[index]
        .or(dbg.live.values[index])
        .map_or(0.0, as_f64);
    let mut v: f64 = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || current));
    let speed = if ty == SignalType::Int { 1.0 } else { 0.1 };
    ui.add(egui::DragValue::new(&mut v).speed(speed))
        .on_hover_text(crate::i18n::tr(
            "Перетащите или щёлкните дважды, чтобы ввести число",
        ));
    ui.data_mut(|d| d.insert_temp(id, v));
    if ty == SignalType::Int {
        Value::Int(v.round() as i32)
    } else {
        Value::Float(v as f32)
    }
}

/// Context menu items for one observed value (block output or variable).
pub fn value_menu(ui: &mut Ui, dbg: &mut DebugState, index: usize) {
    let entry = &dbg.table.entries[index];
    let ty = entry.ty;
    let is_var = matches!(entry.source, Source::Var(_));
    let forced = dbg.live.forced[index];
    ui.label(egui::RichText::new(dbg.name(index)).strong());
    match ty {
        SignalType::Bool => {
            for b in [true, false] {
                if ui
                    .button(crate::i18n::fmt("Форсировать {0}", &[&u8::from(b)]))
                    .clicked()
                {
                    dbg.send(Command::Force {
                        index,
                        value: Some(Value::Bool(b)),
                    });
                }
            }
            if is_var {
                for b in [true, false] {
                    if ui
                        .button(crate::i18n::fmt("Записать {0}", &[&u8::from(b)]))
                        .on_hover_text(crate::i18n::tr(
                            "Значение остаётся, пока его не перезапишет схема",
                        ))
                        .clicked()
                    {
                        dbg.send(Command::Set {
                            index,
                            value: Value::Bool(b),
                        });
                    }
                }
            }
        }
        SignalType::Int | SignalType::Float => {
            let value = ui
                .horizontal(|ui| {
                    ui.label(crate::i18n::tr("Значение"));
                    number_edit(ui, dbg, index, "menu")
                })
                .inner;
            if ui.button(crate::i18n::tr("Форсировать")).clicked() {
                dbg.send(Command::Force {
                    index,
                    value: Some(value),
                });
            }
            if is_var
                && ui
                    .button(crate::i18n::tr("Записать один раз"))
                    .on_hover_text(crate::i18n::tr(
                        "Уставка: значение остаётся, пока его не перезапишет схема",
                    ))
                    .clicked()
            {
                dbg.send(Command::Set { index, value });
            }
        }
        // Not in the debug table.
        SignalType::String | SignalType::Any | SignalType::Number => {}
    }
    if forced.is_some() && ui.button(crate::i18n::tr("Снять форсировку")).clicked() {
        dbg.send(Command::Force { index, value: None });
    }
    ui.separator();
    let on_chart = dbg.chart.contains(&index);
    if !on_chart && ui.button(crate::i18n::tr("Добавить на график")).clicked() {
        dbg.chart.push(index);
    }
    if on_chart && ui.button(crate::i18n::tr("Убрать с графика")).clicked() {
        dbg.chart.retain(|&i| i != index);
    }
}

/// Window with all inputs and variables: live values, writing and forcing.
pub fn values_window(dbg: &mut DebugState, pal: &Palette, ctx: &egui::Context) {
    if !dbg.values {
        return;
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Входы и переменные"))
        .open(&mut open)
        .default_width(440.0)
        .show(ctx, |ui| {
            if !dbg.online() {
                ui.label(crate::i18n::tr(
                    "Нет данных: плата не на связи или прошивка для другой схемы",
                ));
                return;
            }
            let texts: Vec<usize> = (0..dbg.table.texts.len())
                .filter(|&i| matches!(dbg.table.texts[i].source, Source::Var(_)))
                .collect();
            let vars: Vec<usize> = (0..dbg.table.entries.len())
                .filter(|&i| matches!(dbg.table.entries[i].source, Source::Var(_)))
                .collect();
            let inputs: Vec<usize> = dbg.inputs.iter().map(|&(_, i)| i).collect();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (title, rows) in [
                    (crate::i18n::tr("Входы"), inputs),
                    (crate::i18n::tr("Переменные"), vars),
                ] {
                    ui.strong(title);
                    if rows.is_empty() {
                        ui.weak(crate::i18n::tr("нет"));
                    }
                    egui::Grid::new(("debug_values", title))
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            for index in rows {
                                value_row(ui, dbg, pal, index);
                                ui.end_row();
                            }
                        });
                    ui.add_space(6.0);
                }
                if !texts.is_empty() {
                    ui.strong(crate::i18n::tr("Строки"));
                    egui::Grid::new("debug_texts")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for i in texts {
                                let Source::Var(name) = &dbg.table.texts[i].source else {
                                    continue;
                                };
                                ui.label(name);
                                let value = dbg.live.texts[i].as_deref().map_or("—".into(), quoted);
                                ui.monospace(value);
                                ui.end_row();
                            }
                        });
                }
            });
            ui.separator();
            ui.weak(crate::i18n::tr(
                "«Записать» меняет переменную один раз — дальше её может перезаписать схема. \
                 «Форс.» держит значение, пока форсировку не снимут; форсированный вход \
                 не зависит от сигнала на выводе платы.",
            ));
        });
    if !open {
        dbg.values = false;
    }
}

/// Name, live value and controls of one entry, as three grid cells.
fn value_row(ui: &mut Ui, dbg: &mut DebugState, pal: &Palette, index: usize) {
    let entry = &dbg.table.entries[index];
    let ty = entry.ty;
    let is_var = matches!(entry.source, Source::Var(_));
    let forced = dbg.live.forced[index];
    let current = dbg.live.values[index];
    ui.label(dbg.name(index))
        .context_menu(|ui| value_menu(ui, dbg, index));
    let text = current.map_or("—".to_owned(), |v| v.to_string());
    if forced.is_some() {
        ui.label(
            egui::RichText::new(format!("{text} F"))
                .monospace()
                .color(pal.warning),
        )
        .on_hover_text(crate::i18n::tr("Значение форсировано"));
    } else {
        ui.monospace(text);
    }
    ui.horizontal(|ui| {
        match ty {
            SignalType::Bool => {
                let on = current == Some(Value::Bool(true));
                if is_var
                    && ui
                        .button(if on {
                            crate::i18n::tr("Записать 0")
                        } else {
                            crate::i18n::tr("Записать 1")
                        })
                        .on_hover_text(crate::i18n::tr("Переключить переменную один раз"))
                        .clicked()
                {
                    dbg.send(Command::Set {
                        index,
                        value: Value::Bool(!on),
                    });
                }
                for b in [true, false] {
                    let selected = forced == Some(Value::Bool(b));
                    if ui
                        .selectable_label(selected, crate::i18n::fmt("Форс. {0}", &[&u8::from(b)]))
                        .clicked()
                    {
                        dbg.send(Command::Force {
                            index,
                            value: Some(Value::Bool(b)),
                        });
                    }
                }
            }
            SignalType::Int | SignalType::Float => {
                let value = number_edit(ui, dbg, index, "window");
                if is_var && ui.button(crate::i18n::tr("Записать")).clicked() {
                    dbg.send(Command::Set { index, value });
                }
                if ui.button(crate::i18n::tr("Форс.")).clicked() {
                    dbg.send(Command::Force {
                        index,
                        value: Some(value),
                    });
                }
            }
            SignalType::String | SignalType::Any | SignalType::Number => {}
        }
        if forced.is_some()
            && ui
                .small_button(crate::i18n::tr("Снять"))
                .on_hover_text(crate::i18n::tr("Снять форсировку"))
                .clicked()
        {
            dbg.send(Command::Force { index, value: None });
        }
    });
}

/// Where a value label goes relative to its anchor point.
#[derive(Clone, Copy)]
pub enum Place {
    /// Above and to the right of a port.
    Right,
    /// Above and to the left of a port (the input end of a wire).
    Left,
}

/// Small label with a live value next to a port.
pub fn value_label(
    painter: &egui::Painter,
    pal: &Palette,
    anchor: Pos2,
    value: Value,
    forced: bool,
    zoom: f32,
    place: Place,
) {
    let text = if forced {
        format!("{value} F")
    } else {
        value.to_string()
    };
    label(painter, pal, anchor, text, forced, zoom, place);
}

/// Live string next to a port, in quotes and cut to a readable length.
pub fn text_label(
    painter: &egui::Painter,
    pal: &Palette,
    anchor: Pos2,
    text: &str,
    zoom: f32,
    place: Place,
) {
    label(painter, pal, anchor, quoted(text), false, zoom, place);
}

/// `"text"` with control characters made visible, at most 24 characters.
pub fn quoted(text: &str) -> String {
    const MAX: usize = 24;
    let shown: String = text
        .chars()
        .take(MAX)
        .map(|c| if c.is_control() { '·' } else { c })
        .collect();
    let more = if text.chars().count() > MAX {
        "…"
    } else {
        ""
    };
    format!("\"{shown}{more}\"")
}

fn label(
    painter: &egui::Painter,
    pal: &Palette,
    anchor: Pos2,
    text: String,
    forced: bool,
    zoom: f32,
    place: Place,
) {
    let font = egui::FontId::monospace(9.5 * zoom.clamp(0.6, 1.6));
    let galley = painter.layout_no_wrap(text, font, if forced { Color32::WHITE } else { pal.text });
    let size = galley.size() + Vec2::new(6.0, 2.0);
    let min = match place {
        Place::Right => anchor + Vec2::new(4.0, -size.y - 1.0),
        Place::Left => anchor + Vec2::new(-size.x - 4.0, -size.y - 1.0),
    };
    let rect = Rect::from_min_size(min, size);
    let fill = if forced {
        pal.warning
    } else {
        pal.sheet_header
    };
    painter.rect(
        rect,
        3.0,
        fill,
        Stroke::new(1.0, pal.block_border),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + Vec2::new(3.0, 1.0), galley, pal.text);
}

/// Window with a strip chart per selected value (last minute).
pub fn chart_window(dbg: &mut DebugState, pal: &Palette, ctx: &egui::Context) {
    if dbg.chart.is_empty() {
        return;
    }
    let mut open = true;
    let mut remove = None;
    egui::Window::new(crate::i18n::tr("Графики"))
        .open(&mut open)
        .default_size([460.0, 120.0 * dbg.chart.len() as f32])
        .show(ctx, |ui| {
            for &index in &dbg.chart.clone() {
                ui.horizontal(|ui| {
                    ui.strong(dbg.name(index));
                    if let Some(v) = dbg.live.values[index] {
                        ui.monospace(v.to_string());
                    }
                    if ui.small_button("×").clicked() {
                        remove = Some(index);
                    }
                });
                strip(ui, pal, &dbg.live.history[index]);
            }
        });
    if !open {
        dbg.chart.clear();
    }
    if let Some(i) = remove {
        dbg.chart.retain(|&c| c != i);
    }
}

/// Window with cycle time of the whole program and of each sheet.
pub fn diagnostics_window(
    dbg: &mut DebugState,
    pal: &Palette,
    layout: Option<&Layout>,
    ctx: &egui::Context,
) {
    if !dbg.diagnostics {
        return;
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Диагностика"))
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            let Some(s) = dbg.live.stats.clone().filter(|_| dbg.online()) else {
                ui.label(crate::i18n::tr(
                    "Нет данных: плата не на связи или прошивка без диагностики",
                ));
                return;
            };
            let us = |v: u32| crate::i18n::fmt("{0} мкс", &[&v]);
            egui::Grid::new("diag_total")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(crate::i18n::tr("Цикл, последний"));
                    ui.monospace(us(s.last_us));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Цикл, средний"));
                    ui.monospace(us(s.avg_us));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Цикл, максимальный"));
                    ui.strong(us(s.max_us));
                    ui.end_row();
                    if let Some(bg) = &s.background {
                        ui.label(crate::i18n::tr("Ядро 0, проход последний"))
                            .on_hover_text(crate::i18n::tr(
                                "Фоновая задача: сеть и платы с признаком «фоновая»",
                            ));
                        ui.monospace(us(bg.last_us));
                        ui.end_row();
                        ui.label(crate::i18n::tr("Ядро 0, проход максимальный"));
                        ui.strong(us(bg.max_us));
                        ui.end_row();
                    }
                    ui.label(crate::i18n::tr("Период loop(), макс"))
                        .on_hover_text(crate::i18n::tr(
                            "Время между запусками loop(): схема, отладка и система",
                        ));
                    ui.monospace(us(s.period_max_us));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Циклов за 0,5 с"));
                    ui.monospace(s.cycles.to_string());
                    ui.end_row();
                    if s.free_heap > 0 {
                        ui.label(crate::i18n::tr("Свободная память"));
                        ui.monospace(crate::i18n::fmt("{0} КБ", &[&(s.free_heap / 1024)]));
                        ui.end_row();
                    }
                });
            if !s.sheets.is_empty() {
                ui.separator();
                egui::Grid::new("diag_sheets")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(crate::i18n::tr("Лист"));
                        ui.strong(crate::i18n::tr("Последний"));
                        ui.strong(crate::i18n::tr("Макс"));
                        ui.end_row();
                        for sh in &s.sheets {
                            let name = layout
                                .and_then(|l| l.sheet(sh.sheet))
                                .filter(|i| !i.name.is_empty())
                                .map_or_else(
                                    || sh.sheet.to_string(),
                                    |i| format!("{} {}", sh.sheet, i.name),
                                );
                            ui.label(name);
                            ui.monospace(us(sh.last_us));
                            ui.monospace(us(sh.max_us));
                            ui.end_row();
                        }
                    });
            }
            ui.separator();
            let charts = [
                (
                    crate::i18n::tr("Цикл, средний, мкс"),
                    dbg.series(|s| Some(f64::from(s.avg_us))),
                ),
                (
                    crate::i18n::tr("Цикл, максимальный, мкс"),
                    dbg.series(|s| Some(f64::from(s.max_us))),
                ),
                (
                    crate::i18n::tr("Ядро 0, проход, мкс"),
                    dbg.series(|s| s.background.as_ref().map(|b| f64::from(b.last_us))),
                ),
                (
                    crate::i18n::tr("Свободная память, КБ"),
                    dbg.series(|s| (s.free_heap > 0).then(|| f64::from(s.free_heap) / 1024.0)),
                ),
            ];
            for (title, points) in charts.iter().filter(|(_, p)| !p.is_empty()) {
                ui.horizontal(|ui| {
                    ui.strong(*title);
                    if let Some(&(_, v)) = points.last() {
                        ui.monospace(format!("{v:.0}"));
                    }
                });
                strip(ui, pal, points);
            }
            ui.separator();
            if ui
                .button(crate::i18n::tr("Сбросить максимумы"))
                .on_hover_text(crate::i18n::tr(
                    "Начать фиксацию максимального времени заново",
                ))
                .clicked()
            {
                dbg.send(Command::ResetStats);
            }
        });
    if !open {
        dbg.diagnostics = false;
    }
}

fn strip(ui: &mut Ui, pal: &Palette, points: &[(f64, f64)]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 80.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, pal.sheet);
    let Some(&(t_end, _)) = points.last() else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            crate::i18n::tr("нет данных"),
            egui::FontId::proportional(11.0),
            pal.weak,
        );
        return;
    };
    let t_start = t_end - 60.0;
    let visible: Vec<&(f64, f64)> = points.iter().filter(|p| p.0 >= t_start).collect();
    let (lo, hi) = visible.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.1), hi.max(p.1))
    });
    let span = if hi - lo < 1e-9 { 1.0 } else { hi - lo };
    let to_screen = |t: f64, y: f64| {
        Pos2::new(
            rect.min.x + ((t - t_start) / 60.0) as f32 * rect.width(),
            rect.max.y - 6.0 - ((y - lo) / span) as f32 * (rect.height() - 12.0),
        )
    };
    let line: Vec<Pos2> = visible.iter().map(|p| to_screen(p.0, p.1)).collect();
    painter.line(line, Stroke::new(1.5, pal.accent));
    let font = egui::FontId::monospace(9.0);
    painter.text(
        rect.left_top() + Vec2::new(3.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("{hi:.2}"),
        font.clone(),
        pal.weak,
    );
    painter.text(
        rect.left_bottom() + Vec2::new(3.0, -2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{lo:.2}"),
        font,
        pal.weak,
    );
}
