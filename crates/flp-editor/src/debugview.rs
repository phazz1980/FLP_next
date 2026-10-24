//! Online debugging in the editor: live values on the sheets, forcing,
//! setpoints and a simple chart window.

use eframe::egui::{self, Color32, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use flp_codegen::Program;
use flp_core::{Library, PortRef, Project, SignalType};
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
    /// Console lines already copied into the message panel.
    pub console_seen: usize,
}

impl DebugState {
    pub fn start(
        project: &Project,
        lib: &Library,
        port: &str,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let program = Program::build(project, lib).map_err(|e| e.to_string())?;
        let table = flp_codegen::debug::debug_table(&program);
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
            console_seen: 0,
        })
    }

    pub fn refresh(&mut self) {
        self.live = self.session.snapshot();
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

/// Context menu items for one observed value (block output or variable).
pub fn value_menu(ui: &mut Ui, dbg: &mut DebugState, index: usize) {
    let entry = &dbg.table.entries[index];
    let ty = entry.ty;
    let is_var = matches!(entry.source, Source::Var(_));
    let forced = dbg.live.forced[index];
    ui.label(egui::RichText::new(dbg.name(index)).strong());
    match ty {
        SignalType::Bool => {
            if ui.button("Форсировать 1").clicked() {
                dbg.send(Command::Force {
                    index,
                    value: Some(Value::Bool(true)),
                });
            }
            if ui.button("Форсировать 0").clicked() {
                dbg.send(Command::Force {
                    index,
                    value: Some(Value::Bool(false)),
                });
            }
        }
        SignalType::Int | SignalType::Float => {
            let id = Id::unique(("force_value", index));
            let current = dbg.live.values[index].map_or(0.0, |v| match v {
                Value::Bool(b) => f64::from(u8::from(b)),
                Value::Int(i) => f64::from(i),
                Value::Float(f) => f64::from(f),
            });
            let mut v: f64 = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || current));
            ui.horizontal(|ui| {
                ui.label("Значение");
                let speed = if ty == SignalType::Int { 1.0 } else { 0.1 };
                ui.add(egui::DragValue::new(&mut v).speed(speed));
            });
            ui.data_mut(|d| d.insert_temp(id, v));
            let value = if ty == SignalType::Int {
                Value::Int(v.round() as i32)
            } else {
                Value::Float(v as f32)
            };
            if ui.button("Форсировать").clicked() {
                dbg.send(Command::Force {
                    index,
                    value: Some(value),
                });
            }
            if is_var
                && ui
                    .button("Записать один раз")
                    .on_hover_text("Уставка: значение остаётся, пока его не перезапишет схема")
                    .clicked()
            {
                dbg.send(Command::Set { index, value });
            }
        }
    }
    if forced.is_some() && ui.button("Снять форсировку").clicked() {
        dbg.send(Command::Force { index, value: None });
    }
    ui.separator();
    let on_chart = dbg.chart.contains(&index);
    if !on_chart && ui.button("Добавить на график").clicked() {
        dbg.chart.push(index);
    }
    if on_chart && ui.button("Убрать с графика").clicked() {
        dbg.chart.retain(|&i| i != index);
    }
}

/// Small label with a live value next to a port.
pub fn value_label(
    painter: &egui::Painter,
    pal: &Palette,
    anchor: Pos2,
    value: Value,
    forced: bool,
    zoom: f32,
    to_right: bool,
) {
    let text = if forced {
        format!("{value} F")
    } else {
        value.to_string()
    };
    let font = egui::FontId::monospace(9.5 * zoom.clamp(0.6, 1.6));
    let galley = painter.layout_no_wrap(text, font, if forced { Color32::WHITE } else { pal.text });
    let size = galley.size() + Vec2::new(6.0, 2.0);
    let min = if to_right {
        anchor + Vec2::new(4.0, -size.y - 1.0)
    } else {
        anchor + Vec2::new(-size.x - 4.0, -size.y - 1.0)
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
    egui::Window::new("Графики")
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

fn strip(ui: &mut Ui, pal: &Palette, points: &[(f64, f64)]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 80.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, pal.sheet);
    let Some(&(t_end, _)) = points.last() else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "нет данных",
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
