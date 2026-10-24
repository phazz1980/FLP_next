//! Working area: numbered sheets stacked top to bottom, as in FLProg.
//!
//! `layout.toml` stores block positions relative to their sheet; here the sheets
//! are laid out one under another and share one pan and zoom.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{Block, BlockDef, BlockPos, ParamValue, PortDef, PortRef, Severity, SignalType};

use crate::app::{EditorApp, LogLevel, Selection};
use crate::debugview;
use crate::dialogs::Dialog;
use crate::theme::{palette, Palette};

const HEAD_H: f32 = 16.0;
const ROW_H: f32 = 14.0;
const TAG_H: f32 = 18.0;
const PORT_R: f32 = 3.0;
const GRID: f32 = 20.0;
const SHEET_HEADER: f32 = 40.0;
const SHEET_MIN_BODY: f32 = 300.0;
const SHEET_MIN_W: f32 = 1200.0;
const SHEET_GAP: f32 = 14.0;
const SHEET_PAD: f32 = 60.0;
const ADD_SHEET_H: f32 = 28.0;

/// Payload dragged from the library or the variable list onto a sheet.
#[derive(Debug, Clone)]
pub enum DragItem {
    Block(String),
    /// Variable: read, or write when Shift is held on drop.
    Var(String),
}

#[derive(Debug, Clone, Copy)]
struct View {
    pan: Vec2,
    zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        Self {
            pan: Vec2::new(16.0, 12.0),
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct WireDrag {
    port: PortRef,
    from_output: bool,
}

#[derive(Debug, Default)]
pub struct CanvasState {
    view: View,
    wire: Option<WireDrag>,
    /// Block being dragged: sheet heights ignore it, so it can move to another sheet.
    moving: Option<String>,
    /// Block to scroll into view on the next frame.
    pub focus: Option<String>,
}

#[derive(Clone, Copy)]
struct Xf {
    origin: Pos2,
    view: View,
}

impl Xf {
    fn pos(self, p: Pos2) -> Pos2 {
        self.origin + self.view.pan + p.to_vec2() * self.view.zoom
    }

    fn world(self, p: Pos2) -> Pos2 {
        ((p - self.origin - self.view.pan) / self.view.zoom).to_pos2()
    }

    fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }
}

/// Sheet laid out in world coordinates.
#[derive(Debug, Clone, Copy)]
struct Band {
    sheet: u32,
    top: f32,
    height: f32,
}

impl Band {
    fn body_top(self) -> f32 {
        self.top + SHEET_HEADER
    }
}

/// Drawn shape of a block: a box with ports, or a variable tag.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Box,
    Tag { write: bool },
}

/// Approximate text width; exact layout is not needed for box sizing.
fn text_w(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.58
}

fn label(block: &Block, def: Option<&BlockDef>) -> String {
    if let Some(var) = block.var_name() {
        return var.to_owned();
    }
    def.map_or_else(
        || block.kind.clone(),
        |d| d.info.symbol.clone().unwrap_or_else(|| d.info.name.clone()),
    )
}

fn kind_of(block: &Block) -> Kind {
    match block.kind.as_str() {
        VAR_GET => Kind::Tag { write: false },
        VAR_SET => Kind::Tag { write: true },
        _ => Kind::Box,
    }
}

/// Size of a block in world units.
fn size(block: &Block, def: Option<&BlockDef>) -> Vec2 {
    let text = label(block, def);
    match kind_of(block) {
        Kind::Tag { .. } => Vec2::new(text_w(&text, 11.0) + 18.0, TAG_H),
        Kind::Box => {
            let (ins, outs) = def.map_or((Vec::new(), Vec::new()), |d| {
                (
                    d.active_inputs(&block.params).collect(),
                    d.active_outputs(&block.params).collect(),
                )
            });
            let rows = ins.len().max(outs.len()).max(1);
            let longest = |ports: &[&PortDef]| {
                ports
                    .iter()
                    .map(|p| text_w(&p.name, 9.5))
                    .fold(0.0, f32::max)
            };
            let ports_w = longest(&ins) + longest(&outs) + 22.0;
            let w = (text_w(&text, 11.0) + 16.0).max(ports_w).max(64.0);
            Vec2::new((w / 4.0).ceil() * 4.0, HEAD_H + rows as f32 * ROW_H + 4.0)
        }
    }
}

fn port_pos(rect: Rect, kind: Kind, index: usize, output: bool) -> Pos2 {
    match kind {
        Kind::Tag { .. } => {
            if output {
                rect.right_center()
            } else {
                rect.left_center()
            }
        }
        Kind::Box => {
            let x = if output { rect.max.x } else { rect.min.x };
            Pos2::new(x, rect.min.y + HEAD_H + ROW_H * (index as f32 + 0.5))
        }
    }
}

fn signal_color(p: &Palette, ty: SignalType) -> Color32 {
    match ty {
        SignalType::Bool => p.bool_signal,
        SignalType::Int => p.int_signal,
        SignalType::Float => p.float_signal,
    }
}

fn value_text(v: &ParamValue) -> String {
    match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Float(f) => format!("{f}"),
        ParamValue::Text(s) => s.clone(),
    }
}

/// Orthogonal route from an output to an input, in screen space.
fn route(a: Pos2, b: Pos2, step: f32) -> Vec<Pos2> {
    if b.x - a.x >= 2.0 * step {
        let mx = (a.x + b.x) * 0.5;
        vec![a, Pos2::new(mx, a.y), Pos2::new(mx, b.y), b]
    } else {
        // Backwards: go around below the lower end.
        let my = if (b.y - a.y).abs() > 3.0 * step {
            (a.y + b.y) * 0.5
        } else {
            a.y.max(b.y) + 2.5 * step
        };
        vec![
            a,
            Pos2::new(a.x + step, a.y),
            Pos2::new(a.x + step, my),
            Pos2::new(b.x - step, my),
            Pos2::new(b.x - step, b.y),
            b,
        ]
    }
}

/// Replaces sharp corners of a polyline with short arcs.
fn rounded(points: &[Pos2], radius: f32) -> Vec<Pos2> {
    let mut out = vec![points[0]];
    for w in points.windows(3) {
        let (p0, p, p1) = (w[0], w[1], w[2]);
        let (d0, d1) = (p - p0, p1 - p);
        let (l0, l1) = (d0.length(), d1.length());
        if l0 < 0.5 || l1 < 0.5 {
            out.push(p);
            continue;
        }
        let r = radius.min(l0 * 0.5).min(l1 * 0.5);
        let s = p - d0 / l0 * r;
        let e = p + d1 / l1 * r;
        for k in 0..=6 {
            let t = k as f32 / 6.0;
            let q = s.to_vec2() * (1.0 - t) * (1.0 - t)
                + p.to_vec2() * 2.0 * (1.0 - t) * t
                + e.to_vec2() * t * t;
            out.push(q.to_pos2());
        }
    }
    out.push(*points.last().expect("route has points"));
    out
}

fn near_polyline(points: &[Pos2], p: Pos2, tolerance: f32) -> bool {
    points.windows(2).any(|w| {
        let (s, d) = (w[0], w[1] - w[0]);
        let t = ((p - s).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
        (s + d * t).distance(p) <= tolerance
    })
}

/// Sheets stacked top to bottom; heights follow their content.
fn bands(app: &EditorApp, sizes: &HashMap<&str, Vec2>) -> Vec<Band> {
    let doc = app.doc.as_ref().expect("canvas shown with a document");
    let mut top = 0.0;
    app.sheets()
        .into_iter()
        .map(|sheet| {
            let bottom = doc
                .project
                .layout
                .blocks
                .iter()
                .filter(|(id, p)| p.sheet == sheet && app.canvas.moving.as_deref() != Some(id))
                .map(|(id, p)| p.y + sizes.get(id.as_str()).map_or(0.0, |s| s.y))
                .fold(0.0, f32::max);
            let band = Band {
                sheet,
                top,
                height: SHEET_HEADER + (bottom + SHEET_PAD).max(SHEET_MIN_BODY),
            };
            top += band.height + SHEET_GAP;
            band
        })
        .collect()
}

pub fn show(app: &mut EditorApp, ui: &mut Ui) {
    let Some(doc) = &app.doc else {
        ui.centered_and_justified(|ui| {
            ui.label("Откройте проект: Файл → Открыть, или создайте новый.");
        });
        return;
    };
    let pal = palette(app.theme);
    let project = &doc.project;
    let logic = &project.logic;

    // Geometry in world space.
    let defs: HashMap<&str, Option<&BlockDef>> = logic
        .blocks
        .iter()
        .map(|b| (b.id.as_str(), app.lib.resolve(logic, b).map(|l| &l.def)))
        .collect();
    let sizes: HashMap<&str, Vec2> = logic
        .blocks
        .iter()
        .map(|b| (b.id.as_str(), size(b, defs[b.id.as_str()])))
        .collect();
    let bands = bands(app, &sizes);
    let band_of = |sheet: u32| bands.iter().find(|b| b.sheet == sheet).copied();
    let width = project
        .layout
        .blocks
        .iter()
        .map(|(id, p)| p.x + sizes.get(id.as_str()).map_or(0.0, |s| s.x) + SHEET_PAD)
        .fold(SHEET_MIN_W, f32::max);
    let world_rect: HashMap<&str, Rect> = logic
        .blocks
        .iter()
        .filter_map(|b| {
            let pos = project.layout.blocks.get(&b.id)?;
            let band = band_of(pos.sheet)?;
            let min = Pos2::new(pos.x, band.body_top() + pos.y);
            Some((
                b.id.as_str(),
                Rect::from_min_size(min, sizes[b.id.as_str()]),
            ))
        })
        .collect();

    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let bg = ui.interact(rect, Id::unique("canvas_bg"), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let mut view = app.canvas.view;

    if let Some(id) = app.canvas.focus.take() {
        if let Some(r) = world_rect.get(id.as_str()) {
            view.pan = rect.size() * 0.5 - r.center().to_vec2() * view.zoom;
        }
    }
    let xf = Xf {
        origin: rect.min,
        view,
    };
    let z = view.zoom;
    let font = |size: f32| FontId::proportional(size * z.clamp(0.5, 2.0));
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let shift = ui.input(|i| i.modifiers.shift);

    // Sheets.
    painter.rect_filled(rect, 0.0, pal.canvas);
    let mut sheet_dialog = None;
    let mut active_sheet = None;
    for band in &bands {
        let sheet_rect = xf.rect(Rect::from_min_size(
            Pos2::new(0.0, band.top),
            Vec2::new(width, band.height),
        ));
        painter.rect_filled(sheet_rect, 2.0, pal.sheet);
        let header = Rect::from_min_size(
            sheet_rect.min,
            Vec2::new(sheet_rect.width(), SHEET_HEADER * z),
        );
        painter.rect_filled(header, 2.0, pal.sheet_header);
        let info = project.layout.sheet(band.sheet);
        let lines = [
            (format!("Плата: {}", band.sheet), pal.accent),
            (
                format!("Наименование: {}", info.map_or("", |i| i.name.as_str())),
                pal.text,
            ),
            (
                format!("Комментарии: {}", info.map_or("", |i| i.comment.as_str())),
                pal.weak,
            ),
        ];
        for (i, (text, color)) in lines.into_iter().enumerate() {
            painter.text(
                header.min + Vec2::new(10.0, (4.0 + 11.5 * i as f32) * z),
                Align2::LEFT_TOP,
                text,
                font(10.0),
                color,
            );
        }
        let resp = ui.interact(
            header,
            Id::unique(("sheet_header", band.sheet)),
            Sense::click(),
        );
        if resp.double_clicked() {
            sheet_dialog = Some(band.sheet);
        }
        if resp.clicked() {
            active_sheet = Some(band.sheet);
        }
        resp.on_hover_text("Двойной щелчок — название и комментарий платы");

        // Grid.
        if GRID * z > 8.0 {
            let body =
                Rect::from_min_max(Pos2::new(sheet_rect.min.x, header.max.y), sheet_rect.max)
                    .intersect(rect);
            let step = GRID * z;
            let mut x = sheet_rect.min.x + step;
            while x < body.max.x {
                if x > body.min.x {
                    painter.vline(x, body.y_range(), Stroke::new(1.0, pal.grid));
                }
                x += step;
            }
            let mut y = header.max.y + step;
            while y < body.max.y {
                if y > body.min.y {
                    painter.hline(body.x_range(), y, Stroke::new(1.0, pal.grid));
                }
                y += step;
            }
        }
    }
    // "Add sheet" strip under the last sheet.
    let last_bottom = bands.last().map_or(0.0, |b| b.top + b.height + SHEET_GAP);
    let add_rect = xf.rect(Rect::from_min_size(
        Pos2::new(0.0, last_bottom),
        Vec2::new(width, ADD_SHEET_H),
    ));
    let add_resp = ui.interact(add_rect, Id::unique("add_sheet"), Sense::click());
    let add_color = if add_resp.hovered() {
        pal.accent
    } else {
        pal.weak
    };
    painter.rect_stroke(
        add_rect,
        2.0,
        Stroke::new(1.0, add_color),
        StrokeKind::Inside,
    );
    painter.text(
        add_rect.center(),
        Align2::CENTER_CENTER,
        "+ Добавить плату",
        font(11.0),
        add_color,
    );
    let add_sheet = add_resp.clicked();

    // Ports in screen space.
    let errors: Vec<&str> = doc
        .report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .filter_map(|d| d.block.as_deref())
        .collect();
    let screen_rect = |id: &str| world_rect.get(id).map(|r| xf.rect(*r));
    let port_screen = |r: &PortRef, output: bool| -> Option<(Pos2, SignalType)> {
        let block = logic.block(&r.block)?;
        let def = defs[r.block.as_str()]?;
        let ports: Vec<&PortDef> = if output {
            def.active_outputs(&block.params).collect()
        } else {
            def.active_inputs(&block.params).collect()
        };
        let i = ports.iter().position(|q| q.name == r.port)?;
        Some((
            port_pos(screen_rect(&r.block)?, kind_of(block), i, output),
            ports[i].ty,
        ))
    };

    // Links.
    let selected = app.selection.clone();
    let dbg = app.debug.as_ref();
    let click_pos = bg.clicked().then(|| bg.interact_pointer_pos()).flatten();
    let mut clicked_link = None;
    for link in &logic.links {
        let (Some((a, ty)), Some((b, _))) =
            (port_screen(&link.from, true), port_screen(&link.to, false))
        else {
            continue;
        };
        let pts = rounded(&route(a, b, 12.0 * z), 6.0 * z);
        let is_sel = selected.as_ref() == Some(&Selection::Link(link.to.clone()));
        let color = signal_color(&pal, ty);
        // Online: logical lines light up when true and fade when false.
        let live = dbg.and_then(|d| d.output(&link.from));
        let (width, color) = match live {
            Some((flp_debug::Value::Bool(true), _)) => (2.6, pal.ok),
            Some((flp_debug::Value::Bool(false), _)) => (1.4, color.gamma_multiply(0.45)),
            _ => (1.6, color),
        };
        let stroke = Stroke::new(if is_sel { width + 1.4 } else { width }, color);
        painter.line(pts.clone(), stroke);
        if click_pos.is_some_and(|p| near_polyline(&pts, p, 5.0)) {
            clicked_link = Some(link.to.clone());
        }
    }

    // Blocks.
    let mut select = None;
    let mut moves: Vec<(String, Vec2)> = Vec::new();
    let mut drag_end = None;
    let mut open_dialog = None;
    let mut responses: Vec<(String, egui::Response)> = Vec::new();
    for block in &logic.blocks {
        let id = block.id.as_str();
        let Some(r) = screen_rect(id) else { continue };
        let def = defs[id];
        let kind = kind_of(block);
        let resp = ui.interact(r, Id::unique(("block", id)), Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
            select = Some(Selection::Block(id.to_owned()));
        }
        if resp.drag_started() {
            app.canvas.moving = Some(id.to_owned());
        }
        if resp.dragged() && app.canvas.wire.is_none() {
            moves.push((id.to_owned(), resp.drag_delta() / z));
        }
        if resp.drag_stopped() {
            drag_end = Some(id.to_owned());
        }
        if resp.double_clicked() {
            open_dialog = Some(id.to_owned());
        }

        let is_sel = selected.as_ref() == Some(&Selection::Block(id.to_owned()));
        let border = if errors.contains(&id) {
            Stroke::new(1.8, pal.error)
        } else if is_sel {
            Stroke::new(1.8, pal.accent)
        } else {
            Stroke::new(1.0, pal.block_border)
        };
        match kind {
            Kind::Tag { write } => {
                let fill = if write {
                    pal.block_header
                } else {
                    pal.tag_fill
                };
                painter.rect(
                    r,
                    CornerRadius::same((TAG_H * 0.5 * z) as u8),
                    fill,
                    border,
                    StrokeKind::Inside,
                );
                let live = block.var_name().and_then(|v| dbg.and_then(|d| d.var(v)));
                let (text, color) = match live {
                    Some((v, true)) => (format!("{} = {v} F", label(block, def)), pal.warning),
                    Some((v, false)) => (format!("{} = {v}", label(block, def)), pal.text),
                    None => (label(block, def), pal.text),
                };
                painter.text(r.center(), Align2::CENTER_CENTER, text, font(11.0), color);
                if let Some(port) = def.and_then(|d| d.inputs.first().or(d.outputs.first())) {
                    let p = port_pos(r, kind, 0, !write);
                    painter.circle_filled(p, PORT_R * z.sqrt(), signal_color(&pal, port.ty));
                }
            }
            Kind::Box => {
                painter.rect(
                    r,
                    CornerRadius::same(2),
                    pal.block_fill,
                    border,
                    StrokeKind::Inside,
                );
                let head = Rect::from_min_size(r.min, Vec2::new(r.width(), HEAD_H * z)).shrink(1.0);
                painter.rect_filled(
                    head,
                    CornerRadius {
                        nw: 2,
                        ne: 2,
                        sw: 0,
                        se: 0,
                    },
                    pal.block_header,
                );
                painter.text(
                    head.center(),
                    Align2::CENTER_CENTER,
                    label(block, def),
                    font(10.5),
                    pal.text,
                );
                // Block label (id) above the box.
                painter.text(
                    r.left_top() - Vec2::new(0.0, 2.0),
                    Align2::LEFT_BOTTOM,
                    id,
                    font(9.5),
                    pal.weak,
                );
                if let Some(def) = def {
                    for (i, port) in def.active_inputs(&block.params).enumerate() {
                        let p = port_pos(r, kind, i, false);
                        painter.circle_filled(p, PORT_R * z.sqrt(), signal_color(&pal, port.ty));
                        painter.text(
                            p + Vec2::new(5.0 * z, 0.0),
                            Align2::LEFT_CENTER,
                            &port.name,
                            font(9.0),
                            pal.weak,
                        );
                        let fed = logic
                            .links
                            .iter()
                            .any(|l| l.to.block == id && l.to.port == port.name);
                        if !fed {
                            if let Some(d) = &port.default {
                                painter.text(
                                    p - Vec2::new(5.0 * z, 0.0),
                                    Align2::RIGHT_CENTER,
                                    value_text(d),
                                    font(9.0),
                                    pal.weak,
                                );
                            }
                        }
                    }
                    for (i, port) in def.active_outputs(&block.params).enumerate() {
                        let p = port_pos(r, kind, i, true);
                        painter.circle_filled(p, PORT_R * z.sqrt(), signal_color(&pal, port.ty));
                        painter.text(
                            p - Vec2::new(5.0 * z, 0.0),
                            Align2::RIGHT_CENTER,
                            &port.name,
                            font(9.0),
                            pal.weak,
                        );
                    }
                    // Parameter values under the box, like constants in FLProg.
                    let summary: Vec<String> = def
                        .params
                        .iter()
                        .filter_map(|p| {
                            block
                                .params
                                .get(&p.name)
                                .map(|v| format!("{}={}", p.name, value_text(v)))
                        })
                        .collect();
                    if !summary.is_empty() {
                        painter.text(
                            r.left_bottom() + Vec2::new(0.0, 2.0),
                            Align2::LEFT_TOP,
                            summary.join(" "),
                            font(9.0),
                            pal.weak,
                        );
                    }
                } else {
                    painter.text(
                        r.center() + Vec2::new(0.0, 6.0 * z),
                        Align2::CENTER_CENTER,
                        "?",
                        font(12.0),
                        pal.error,
                    );
                }
            }
        }
        responses.push((id.to_owned(), resp));
    }

    // Ports: drag a wire from any port.
    struct Hit {
        port: PortRef,
        output: bool,
        ty: SignalType,
        pos: Pos2,
    }
    let mut hits: Vec<Hit> = Vec::new();
    let mut port_menus: Vec<(PortRef, egui::Response)> = Vec::new();
    for block in &logic.blocks {
        let (Some(r), Some(def)) = (screen_rect(&block.id), defs[block.id.as_str()]) else {
            continue;
        };
        let kind = kind_of(block);
        let ports = def
            .active_inputs(&block.params)
            .enumerate()
            .map(|(i, q)| (i, q, false))
            .chain(
                def.active_outputs(&block.params)
                    .enumerate()
                    .map(|(i, q)| (i, q, true)),
            );
        for (i, q, output) in ports {
            let pos = port_pos(r, kind, i, output);
            let area = Rect::from_center_size(pos, Vec2::splat(10.0));
            let resp = ui.interact(
                area,
                Id::unique(("port", &block.id, &q.name)),
                Sense::click_and_drag(),
            );
            if output && kind == Kind::Box {
                let port = PortRef::new(&block.id, &q.name);
                if let Some((v, forced)) = dbg.and_then(|d| d.output(&port)) {
                    debugview::value_label(&painter, &pal, pos, v, forced, z, true);
                }
                if dbg.is_some() {
                    port_menus.push((port, resp.clone()));
                }
            }
            if resp.hovered() || resp.dragged() {
                painter.circle_stroke(
                    pos,
                    PORT_R * 2.0,
                    Stroke::new(1.4, signal_color(&pal, q.ty)),
                );
            }
            let port = PortRef::new(&block.id, &q.name);
            if resp.drag_started() {
                app.canvas.wire = Some(WireDrag {
                    port: port.clone(),
                    from_output: output,
                });
            }
            hits.push(Hit {
                port,
                output,
                ty: q.ty,
                pos,
            });
        }
    }

    // Wire being drawn.
    let released = ui.input(|i| i.pointer.any_released());
    let mut connect = None;
    if let Some(wire) = &app.canvas.wire {
        let start = hits
            .iter()
            .find(|h| h.port == wire.port && h.output == wire.from_output);
        if let (Some(h), Some(p)) = (start, pointer) {
            let (a, b) = if wire.from_output {
                (h.pos, p)
            } else {
                (p, h.pos)
            };
            let pts = rounded(&route(a, b, 12.0 * z), 6.0 * z);
            painter.line(
                pts,
                Stroke::new(1.6, signal_color(&pal, h.ty).gamma_multiply(0.7)),
            );
        }
        if released {
            let target = pointer.and_then(|p| {
                hits.iter()
                    .filter(|h| h.output != wire.from_output && h.pos.distance(p) < 10.0)
                    .min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))
            });
            if let Some(h) = target {
                connect = Some(if wire.from_output {
                    (wire.port.clone(), h.port.clone())
                } else {
                    (h.port.clone(), wire.port.clone())
                });
            }
            app.canvas.wire = None;
        }
    }

    // Drop from the library or the variable list.
    let dropped = bg.dnd_release_payload::<DragItem>();
    let drop_at = |p: Pos2| -> Option<BlockPos> {
        let w = xf.world(p);
        let band = bands
            .iter()
            .find(|b| w.y >= b.top && w.y < b.top + b.height)
            .or(bands.last())?;
        Some(BlockPos {
            sheet: band.sheet,
            x: (w.x / GRID).round().max(0.0) * GRID,
            y: ((w.y - band.body_top()) / GRID).round().max(0.0) * GRID,
        })
    };
    let drop = dropped.and_then(|item| Some(((*item).clone(), drop_at(pointer?)?)));
    // Where a moved block ends up: the sheet under its top-left corner.
    let landing = drag_end.as_ref().and_then(|id| {
        let r = world_rect.get(id.as_str())?;
        let band = bands
            .iter()
            .find(|b| r.min.y >= b.top && r.min.y < b.top + b.height)
            .or(if r.min.y < 0.0 {
                bands.first()
            } else {
                bands.last()
            })?;
        Some(BlockPos {
            sheet: band.sheet,
            x: (r.min.x / GRID).round().max(0.0) * GRID,
            y: ((r.min.y - band.body_top()) / GRID).round().max(0.0) * GRID,
        })
    });

    // Pan and zoom.
    if bg.dragged() && app.canvas.wire.is_none() {
        view.pan += bg.drag_delta();
    }
    if ui.rect_contains_pointer(rect) {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if zoom != 1.0 {
            let anchor = pointer.unwrap_or(rect.center());
            let world = xf.world(anchor);
            view.zoom = (view.zoom * zoom).clamp(0.3, 3.0);
            view.pan = anchor - rect.min - world.to_vec2() * view.zoom;
        } else {
            view.pan += scroll;
        }
    }
    app.canvas.view = view;

    // Apply changes (the document was borrowed for drawing above).
    if bg.clicked() {
        app.select(clicked_link.map(Selection::Link));
    }
    if let Some(s) = select {
        if let Selection::Block(id) = &s {
            if let Some(p) = app
                .doc
                .as_ref()
                .and_then(|d| d.project.layout.blocks.get(id))
            {
                app.sheet = p.sheet;
            }
        }
        app.select(Some(s));
    }
    if let Some(s) = active_sheet {
        app.sheet = s;
    }
    let lib = app.lib.clone();
    let mut problems = Vec::new();
    let mut added = None;
    if let Some(doc) = &mut app.doc {
        for (id, delta) in moves {
            if let Some(&pos) = doc.project.layout.blocks.get(&id) {
                let moved = BlockPos {
                    x: pos.x + delta.x,
                    y: pos.y + delta.y,
                    ..pos
                };
                let _ = doc.edit(&lib, Some(format!("move:{id}")), |p| {
                    p.move_block(&id, moved)
                });
            }
        }
        if let (Some(id), Some(pos)) = (&drag_end, landing) {
            let _ = doc.edit(&lib, Some(format!("move:{id}")), |p| p.move_block(id, pos));
            doc.end_edit();
        }
        if let Some((from, to)) = connect {
            if let Err(e) = doc.edit(&lib, None, |p| p.connect(&lib, from, to)) {
                problems.push(e.to_string());
            }
        }
        if let Some((item, pos)) = drop {
            let result = match item {
                DragItem::Block(kind) => doc.edit(&lib, None, |p| p.add_block(&lib, &kind, pos)),
                DragItem::Var(name) => doc.edit(&lib, None, |p| p.add_var_block(&name, shift, pos)),
            };
            match result {
                Ok(id) => added = Some(id),
                Err(e) => problems.push(e.to_string()),
            }
        }
    }
    if drag_end.is_some() {
        app.canvas.moving = None;
    }
    if add_sheet {
        let next = app.sheets().last().copied().unwrap_or(1) + 1;
        app.extra_sheets.insert(next);
        app.sheet = next;
    }
    if let Some(id) = added {
        app.select(Some(Selection::Block(id)));
    }
    for e in problems {
        app.info(LogLevel::Error, e);
    }
    if let Some(id) = open_dialog {
        app.dialog = Dialog::for_block(app, &id);
    }
    if let Some(sheet) = sheet_dialog {
        app.dialog = Some(Dialog::for_sheet(app, sheet));
    }
    for (id, resp) in responses {
        block_context_menu(app, &id, &resp);
    }
    // Online: right click on an output forces it or puts it on the chart.
    for (port, resp) in port_menus {
        let Some(d) = &mut app.debug else { break };
        let Some(index) = d.table.output_index(&port) else {
            continue;
        };
        resp.context_menu(|ui| debugview::value_menu(ui, d, index));
    }
}

fn block_context_menu(app: &mut EditorApp, id: &str, resp: &egui::Response) {
    let sheets = app.sheets();
    let current = app
        .doc
        .as_ref()
        .and_then(|d| d.project.layout.blocks.get(id))
        .map_or(1, |p| p.sheet);
    let var = app
        .doc
        .as_ref()
        .and_then(|d| d.project.logic.block(id))
        .and_then(|b| b.var_name().map(str::to_owned));
    let mut move_to = None;
    let mut delete = false;
    let mut props = false;
    let mut edit_var = false;
    let debug_index = var
        .as_deref()
        .and_then(|v| app.debug.as_ref().and_then(|d| d.table.var_index(v)));
    let mut debug = app.debug.take();
    resp.context_menu(|ui| {
        if let (Some(d), Some(index)) = (&mut debug, debug_index) {
            ui.menu_button("Отладка", |ui| debugview::value_menu(ui, d, index));
            ui.separator();
        }
        if ui.button("Свойства...").clicked() {
            props = true;
        }
        if var.is_some() && ui.button("Изменить переменную...").clicked() {
            edit_var = true;
        }
        ui.menu_button("Перенести на плату", |ui| {
            let new = sheets.last().copied().unwrap_or(1) + 1;
            for s in sheets.iter().copied().chain([new]) {
                if s != current && ui.button(format!("Плата {s}")).clicked() {
                    move_to = Some(s);
                }
            }
        });
        ui.separator();
        if ui.button("Удалить").clicked() {
            delete = true;
        }
    });
    app.debug = debug;
    if props {
        app.dialog = Dialog::for_block(app, id);
    }
    if edit_var {
        if let Some(name) = &var {
            app.dialog = Dialog::for_var(app, Some(name));
        }
    }
    if let Some(sheet) = move_to {
        let lib = app.lib.clone();
        if let Some(doc) = &mut app.doc {
            if let Some(&pos) = doc.project.layout.blocks.get(id) {
                let _ = doc.edit(&lib, None, |p| p.move_block(id, BlockPos { sheet, ..pos }));
            }
        }
        app.extra_sheets.insert(sheet);
    }
    if delete {
        app.select(Some(Selection::Block(id.to_owned())));
        app.delete_selection();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_route_has_two_bends() {
        let r = route(Pos2::new(0.0, 0.0), Pos2::new(100.0, 40.0), 12.0);
        assert_eq!(r.len(), 4);
        assert_eq!(r[1], Pos2::new(50.0, 0.0));
    }

    #[test]
    fn backward_route_goes_around() {
        let r = route(Pos2::new(100.0, 0.0), Pos2::new(0.0, 0.0), 12.0);
        assert_eq!(r.len(), 6);
        assert!(r[2].y > 0.0);
    }

    #[test]
    fn rounding_keeps_ends() {
        let pts = [
            Pos2::new(0.0, 0.0),
            Pos2::new(50.0, 0.0),
            Pos2::new(50.0, 40.0),
        ];
        let r = rounded(&pts, 6.0);
        assert_eq!(r.first(), Some(&pts[0]));
        assert_eq!(r.last(), Some(&pts[2]));
        assert!(r.len() > 3);
    }
}
