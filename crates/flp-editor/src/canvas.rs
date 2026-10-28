//! Working area: numbered sheets stacked top to bottom, as in FLProg.
//!
//! `layout.toml` stores block positions relative to their sheet; here the sheets
//! are laid out one under another and share one pan and zoom.

use std::collections::HashMap;

use eframe::egui::containers::menu::{MenuConfig, SubMenuButton};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{
    Block, BlockDef, BlockPos, Logic, ParamValue, PortDef, PortRef, Severity, SignalType,
};

use crate::app::{EditorApp, LogLevel, Selection};
use crate::debugview::{self, Place};
use crate::dialogs::Dialog;
use crate::theme::Palette;

// Sizes are multiples of the grid so that every port sits on a grid line.
const HEAD_H: f32 = 20.0;
const ROW_H: f32 = 20.0;
const TAG_H: f32 = 20.0;
const PORT_R: f32 = 3.0;
const GRID: f32 = 10.0;
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
    /// Variable: the side the first link comes to decides read or write.
    Var(String),
    /// Web widget (block id) from the page list: a copy of it is placed on the sheet.
    Widget(String),
    /// Setting or port of a service block (Wi-Fi...) taken out to the board.
    Setting {
        id: String,
        name: String,
        ty: SignalType,
        feed: bool,
    },
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
    /// Screen corner where a selection frame (left drag) started.
    band: Option<Pos2>,
    /// Canvas point under the mouse: where pasted blocks go.
    hover: Option<BlockPos>,
    /// Text of the constant being typed in the menu of an input.
    const_text: String,
}

impl CanvasState {
    pub fn paste_pos(&self) -> Option<BlockPos> {
        self.hover
    }
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
    /// `en`: a write with the enable input, drawn twice as tall with «En» below.
    Tag {
        write: bool,
        en: bool,
    },
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

fn kind_of(block: &Block, def: Option<&BlockDef>) -> Kind {
    match block.kind.as_str() {
        VAR_GET => Kind::Tag {
            write: false,
            en: false,
        },
        VAR_SET => Kind::Tag {
            write: true,
            en: def.is_some_and(|d| d.inputs.len() > 1),
        },
        _ => Kind::Box,
    }
}

/// Size of a block in world units.
fn size(block: &Block, def: Option<&BlockDef>) -> Vec2 {
    let text = label(block, def);
    match kind_of(block, def) {
        Kind::Tag { en, .. } => Vec2::new(
            ((text_w(&text, 11.0) + 18.0) / GRID).ceil() * GRID,
            if en { TAG_H * 2.0 } else { TAG_H },
        ),
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
                    .map(|p| text_w(p.title(), 9.5))
                    .fold(0.0, f32::max)
            };
            let ports_w = longest(&ins) + longest(&outs) + 22.0;
            let w = (text_w(&text, 11.0) + 16.0).max(ports_w).max(64.0);
            Vec2::new(
                (w / GRID).ceil() * GRID,
                HEAD_H + rows as f32 * ROW_H + 10.0,
            )
        }
    }
}

/// Draws a block box: header with the label, id above, ports with their names,
/// defaults of unconnected inputs and parameter values below.
#[allow(clippy::too_many_arguments)]
fn paint_box(
    painter: &egui::Painter,
    r: Rect,
    z: f32,
    block: &Block,
    def: Option<&BlockDef>,
    pal: &Palette,
    border: Stroke,
    fed: impl Fn(&str) -> bool,
) {
    let kind = Kind::Box;
    let font = |size: f32| FontId::proportional(size * z.clamp(0.5, 2.0));
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
    // Block label (id) above the box; a web widget also shows its caption.
    let caption = match block.params.get("caption") {
        Some(flp_core::ParamValue::Text(c)) if block.kind.starts_with("web_") && !c.is_empty() => {
            format!("{} · {c}", block.id)
        }
        _ => block.id.clone(),
    };
    painter.text(
        r.left_top() - Vec2::new(0.0, 2.0),
        Align2::LEFT_BOTTOM,
        &caption,
        font(9.5),
        pal.weak,
    );
    if let Some(def) = def {
        for (i, port) in def.active_inputs(&block.params).enumerate() {
            let p = port_pos(r, kind, i, false);
            painter.circle_filled(p, PORT_R * z.sqrt(), signal_color(pal, port.ty));
            painter.text(
                p + Vec2::new(5.0 * z, 0.0),
                Align2::LEFT_CENTER,
                port.title(),
                font(9.0),
                pal.weak,
            );
            if !fed(&port.name) {
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
            painter.circle_filled(p, PORT_R * z.sqrt(), signal_color(pal, port.ty));
            painter.text(
                p - Vec2::new(5.0 * z, 0.0),
                Align2::RIGHT_CENTER,
                port.title(),
                font(9.0),
                pal.weak,
            );
        }
        // Parameter values under the box, like constants in FLProg.
        let summary: Vec<String> = def
            .params
            .iter()
            .filter_map(|p| block.params.get(&p.name).and_then(|v| p.caption(v)))
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

/// Block as it will look on a sheet, for the block editor: an instance with default
/// parameters and nothing connected.
pub fn block_preview(ui: &mut Ui, def: &BlockDef, pal: &Palette) {
    let params = def
        .params
        .iter()
        .filter_map(|p| Some((p.name.clone(), p.default.clone()?)))
        .collect();
    let block = Block {
        id: flp_codegen::preview_id(def),
        kind: def.info.kind.clone(),
        params,
        ..Default::default()
    };
    let body = size(&block, Some(def));
    let margin = Vec2::new(48.0, 20.0);
    let (rect, _) = ui.allocate_exact_size(body + margin * 2.0, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, pal.sheet);
    let r = Rect::from_min_size(rect.min + margin, body);
    let border = Stroke::new(1.0, pal.block_border);
    paint_box(&painter, r, 1.0, &block, Some(def), pal, border, |_| false);
}

/// Whether a variable block already has a link on either side. A block of a variable
/// bound to a port of a service block (Wi-Fi `En`, `IP`...) has its side fixed from the
/// start: a read-only value gets no writing side. Parameters work both ways.
fn tag_linked(logic: &Logic, lib: &flp_core::Library, id: &str) -> bool {
    logic
        .block(id)
        .and_then(|b| b.var_name())
        .is_some_and(|v| flp_core::vars::var_fixed(logic, lib, v))
        || logic
            .links
            .iter()
            .any(|l| l.from.block == id || l.to.block == id)
}

fn port_pos(rect: Rect, kind: Kind, index: usize, output: bool) -> Pos2 {
    match kind {
        Kind::Tag { en: false, .. } => {
            if output {
                rect.right_center()
            } else {
                rect.left_center()
            }
        }
        // Value on the upper half, `en` on the lower one.
        Kind::Tag { en: true, .. } => {
            let x = if output { rect.max.x } else { rect.min.x };
            let row = if output { 0.0 } else { index as f32 };
            Pos2::new(x, rect.min.y + rect.height() * (0.25 + 0.5 * row))
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
        SignalType::String => p.string_signal,
        SignalType::Any => p.text,
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

/// Whether a link runs left to right (one vertical segment) or has to go around.
fn is_forward(a: Pos2, b: Pos2, step: f32) -> bool {
    b.x - a.x >= 2.0 * step
}

/// Orthogonal route from an output to an input, in screen space. `bend` moves the
/// middle segment: its x for a forward link, its y for a backward one.
fn route(a: Pos2, b: Pos2, step: f32, bend: Option<f32>) -> Vec<Pos2> {
    if is_forward(a, b, step) {
        let mx = bend.map_or((a.x + b.x) * 0.5, |x| x.clamp(a.x + 2.0, b.x - 2.0));
        vec![a, Pos2::new(mx, a.y), Pos2::new(mx, b.y), b]
    } else {
        // Backwards: go around below the lower end.
        let my = bend.unwrap_or(if (b.y - a.y).abs() > 3.0 * step {
            (a.y + b.y) * 0.5
        } else {
            a.y.max(b.y) + 2.5 * step
        });
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
    let pal = app.theme.palette;
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
            // Service blocks (Wi-Fi, web interface) live in the left panel only.
            if defs[b.id.as_str()].is_some_and(|d| d.info.hidden) {
                return None;
            }
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
    let ctrl = ui.input(|i| i.modifiers.command);

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
        let background = info.is_some_and(|i| i.background);
        if background {
            // Violet tint and a frame: a background sheet is visible at a glance.
            let tint = Color32::from_rgb(150, 90, 220);
            painter.rect_filled(sheet_rect, 2.0, tint.gamma_multiply(0.10));
            painter.rect_filled(header, 2.0, tint.gamma_multiply(0.30));
            painter.rect_stroke(
                sheet_rect,
                2.0,
                Stroke::new(2.0, tint),
                StrokeKind::Inside,
            );
            painter.text(
                header.right_top() + Vec2::new(-10.0, 4.0 * z),
                Align2::RIGHT_TOP,
                "ФОНОВАЯ ЗАДАЧА · ядро 0",
                font(10.0),
                tint,
            );
        }
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
            port_pos(screen_rect(&r.block)?, kind_of(block, Some(def)), i, output),
            ports[i].ty,
        ))
    };

    // Links.
    let selected = app.selection.clone();
    let dbg = app.debug.as_ref();
    let click_pos = bg.clicked().then(|| bg.interact_pointer_pos()).flatten();
    let mut clicked_link = None;
    let mut wire_click = None;
    let mut wire_edit: Option<(flp_core::PortRef, Option<f32>)> = None;
    let mut wire_end = false;
    let mut wire_break: Option<(flp_core::PortRef, bool)> = None;
    let ctx = ui.ctx().clone();
    // Live values above the input end of each wire; drawn after the blocks to stay on top.
    let mut wire_labels = Vec::new();
    let mut text_labels = Vec::new();
    for link in &logic.links {
        let (Some((a, ty)), Some((b, _))) =
            (port_screen(&link.from, true), port_screen(&link.to, false))
        else {
            continue;
        };
        let sheet_of = |id: &str| project.layout.blocks.get(id).map(|p| p.sheet);
        let (from_sheet, to_sheet) = (sheet_of(&link.from.block), sheet_of(&link.to.block));
        // Links between sheets are broken into two labelled stubs, as in FLProg.
        let key = flp_core::Layout::wire_key(&link.to);
        let route_cfg = project.layout.wires.get(&key).copied().unwrap_or_default();
        let cross = from_sheet != to_sheet;
        // Links inside a sheet can be broken by hand in the same way.
        let broken = cross || route_cfg.broken;
        let stub = 24.0 * z;
        let mut handle = None;
        let mut menu_areas: Vec<Rect> = Vec::new();
        let lines: Vec<Vec<Pos2>> = if broken {
            if !cross {
                menu_areas = vec![
                    Rect::from_two_pos(a, a + Vec2::new(stub, 0.0)).expand(5.0),
                    Rect::from_two_pos(b - Vec2::new(stub, 0.0), b).expand(5.0),
                ];
            }
            vec![
                vec![a, a + Vec2::new(stub, 0.0)],
                vec![b - Vec2::new(stub, 0.0), b],
            ]
        } else {
            let forward = is_forward(a, b, 12.0 * z);
            let band = bands.iter().find(|band| Some(band.sheet) == from_sheet);
            let bend = route_cfg.bend.zip(band).map(|(v, band)| {
                if forward {
                    xf.pos(Pos2::new(v, 0.0)).x
                } else {
                    xf.pos(Pos2::new(0.0, band.body_top() + v)).y
                }
            });
            let raw = route(a, b, 12.0 * z, bend);
            // The middle segment can be dragged to reroute the link.
            let seg = if forward {
                (raw[1], raw[2])
            } else {
                (raw[2], raw[3])
            };
            if let (Some(band), true) = (band, seg.0.distance(seg.1) > 4.0) {
                handle = Some((Rect::from_two_pos(seg.0, seg.1).expand(4.0), forward, *band));
            }
            menu_areas = vec![Rect::from_center_size(
                seg.0.lerp(seg.1, 0.5),
                Vec2::splat(12.0),
            )];
            vec![rounded(&raw, 6.0 * z)]
        };
        let is_sel = selected.as_ref() == Some(&Selection::Link(link.to.clone()));
        let color = signal_color(&pal, ty);
        // Online: logical lines light up when true and fade when false.
        // An inverted output feeds the inverted value to its links.
        let live = dbg
            .and_then(|d| d.output(&link.from))
            .map(|(v, forced)| (flip(v, inverted_output(logic, &link.from)), forced));
        let (width, color) = match live {
            Some((flp_debug::Value::Bool(true), _)) => (2.6, pal.ok),
            Some((flp_debug::Value::Bool(false), _)) => (1.4, color.gamma_multiply(0.45)),
            _ => (1.6, color),
        };
        if let Some((v, forced)) = live {
            wire_labels.push((b, v, forced));
        }
        if let Some(text) = dbg.and_then(|d| d.text_output(&link.from)) {
            text_labels.push((b, text.to_owned()));
        }
        let stroke = Stroke::new(if is_sel { width + 1.4 } else { width }, color);
        if let Some((area, forward, band)) = handle {
            let resp = ui.interact(
                area,
                Id::unique(("wire", &link.to)),
                Sense::click_and_drag(),
            );
            if resp.hovered() || resp.dragged() {
                ctx.set_cursor_icon(if forward {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            }
            if resp.clicked() {
                wire_click = Some(link.to.clone());
            }
            if resp.double_clicked() {
                wire_edit = Some((link.to.clone(), None));
            } else if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
                let w = xf.world(p);
                let snap = |v: f32| (v / GRID).round() * GRID;
                let bend = if forward {
                    snap(w.x)
                } else {
                    snap(w.y - band.body_top())
                };
                wire_edit = Some((link.to.clone(), Some(bend)));
            }
            if resp.drag_stopped() {
                wire_end = true;
            }
            resp.on_hover_text("Потяните, чтобы изменить путь; двойной щелчок — путь по умолчанию");
        }
        if !cross {
            for (i, area) in menu_areas.iter().enumerate() {
                let resp = ui.interact(*area, Id::unique(("wire-menu", &key, i)), Sense::click());
                resp.context_menu(|ui| {
                    let label = if route_cfg.broken {
                        "Соединить линией"
                    } else {
                        "Разорвать связь"
                    };
                    if ui.button(label).clicked() {
                        wire_break = Some((link.to.clone(), !route_cfg.broken));
                        ui.close();
                    }
                });
            }
        }
        for pts in &lines {
            painter.line(pts.clone(), stroke);
            if click_pos.is_some_and(|p| near_polyline(pts, p, 5.0)) {
                clicked_link = Some(link.to.clone());
            }
        }
        if cross {
            let sheet = |s: Option<u32>| s.map_or("?".to_owned(), |s| s.to_string());
            painter.text(
                a + Vec2::new(stub + 3.0 * z, 0.0),
                Align2::LEFT_CENTER,
                format!("→ П{}: {}.{}", sheet(to_sheet), link.to.block, link.to.port),
                font(10.0),
                color,
            );
            painter.text(
                b - Vec2::new(stub + 3.0 * z, 0.0),
                Align2::RIGHT_CENTER,
                format!(
                    "П{}: {}.{} →",
                    sheet(from_sheet),
                    link.from.block,
                    link.from.port
                ),
                font(10.0),
                color,
            );
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
        let kind = kind_of(block, def);
        let resp = ui.interact(r, Id::unique(("block", id)), Sense::click_and_drag());
        let in_sel = selected.as_ref().is_some_and(|s| s.has_block(id));
        if resp.clicked() && ctrl {
            // Ctrl + click adds the block to the selection or removes it.
            let mut ids = selected
                .as_ref()
                .map(|s| s.blocks().to_vec())
                .unwrap_or_default();
            if in_sel {
                ids.retain(|b| b != id);
            } else {
                ids.push(id.to_owned());
            }
            select = Some(Selection::of_blocks(ids));
        } else if resp.clicked() || resp.secondary_clicked() || (resp.drag_started() && !in_sel) {
            select = Some(Selection::of_blocks(vec![id.to_owned()]));
        }
        if resp.drag_started() {
            app.canvas.moving = Some(id.to_owned());
        }
        if resp.dragged() && app.canvas.wire.is_none() {
            // Dragging a block of a group moves the whole group.
            match selected.as_ref().filter(|s| in_sel && s.blocks().len() > 1) {
                Some(s) => {
                    for b in s.blocks() {
                        moves.push((b.clone(), resp.drag_delta() / z));
                    }
                }
                None => moves.push((id.to_owned(), resp.drag_delta() / z)),
            }
        }
        if resp.drag_stopped() {
            drag_end = Some(id.to_owned());
        }
        if resp.double_clicked() {
            open_dialog = Some(id.to_owned());
        }

        let is_sel = in_sel;
        let border = if errors.contains(&id) {
            Stroke::new(1.8, pal.error)
        } else if is_sel {
            Stroke::new(1.8, pal.accent)
        } else {
            Stroke::new(1.0, pal.block_border)
        };
        match kind {
            Kind::Tag { write, en } => {
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
                // The live value is drawn at the output pin, like for any block.
                let text = label(block, def);
                let center = if en {
                    Pos2::new(r.center().x, r.min.y + r.height() * 0.25)
                } else {
                    r.center()
                };
                painter.text(center, Align2::CENTER_CENTER, text, font(11.0), pal.text);
                if en {
                    let p = port_pos(r, kind, 1, false);
                    painter.text(
                        p + Vec2::new(6.0 * z, 0.0),
                        Align2::LEFT_CENTER,
                        "En",
                        font(9.5),
                        pal.text,
                    );
                    painter.circle_filled(
                        p,
                        PORT_R * z.sqrt(),
                        signal_color(&pal, SignalType::Bool),
                    );
                }
                if let Some(port) = def.and_then(|d| d.inputs.first().or(d.outputs.first())) {
                    let color = signal_color(&pal, port.ty);
                    painter.circle_filled(port_pos(r, kind, 0, !write), PORT_R * z.sqrt(), color);
                    // Not connected yet: the other side is free too.
                    if !tag_linked(logic, &app.lib, &block.id) {
                        painter.circle_filled(
                            port_pos(r, kind, 0, write),
                            PORT_R * z.sqrt(),
                            color,
                        );
                    }
                }
            }
            Kind::Box => {
                // A constant on an input is drawn with the ports, like a link.
                let fed = |port: &str| {
                    block.consts.contains_key(port)
                        || logic
                            .links
                            .iter()
                            .any(|l| l.to.block == id && l.to.port == port)
                };
                // Passwords are kept by the program: show them from there (masked).
                let filled = def
                    .filter(|d| d.params.iter().any(|p| p.local.is_some()))
                    .map(|d| {
                        let mut b = block.clone();
                        app.local.fill_block(d, &mut b);
                        b
                    });
                let block = filled.as_ref().unwrap_or(block);
                paint_box(&painter, r, z, block, def, &pal, border, fed);
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
    let mut port_menus: Vec<(PortRef, bool, SignalType, egui::Response)> = Vec::new();
    for block in &logic.blocks {
        let (Some(r), Some(def)) = (screen_rect(&block.id), defs[block.id.as_str()]) else {
            continue;
        };
        let kind = kind_of(block, Some(def));
        // An unconnected variable block offers both sides; the link decides
        // whether it reads (output `out`) or writes (input `in`).
        let write = block.kind == VAR_SET;
        // With the write enable input the block only writes: no output side.
        let tag_port = (block.var_name().is_some()
            && !tag_linked(logic, &app.lib, &block.id)
            && kind
                != (Kind::Tag {
                    write: true,
                    en: true,
                }))
        .then(|| def.inputs.first().or(def.outputs.first()))
        .flatten()
        .map(|q| {
            let q = PortDef {
                name: if write { "out" } else { "in" }.to_owned(),
                ..q.clone()
            };
            (q, write)
        });
        if let Some((q, output)) = &tag_port {
            let pos = port_pos(r, kind, 0, *output);
            painter.circle_filled(pos, PORT_R * xf.view.zoom.sqrt(), signal_color(&pal, q.ty));
        }
        let ports = def
            .active_inputs(&block.params)
            .enumerate()
            .map(|(i, q)| (i, q, false))
            .chain(
                def.active_outputs(&block.params)
                    .enumerate()
                    .map(|(i, q)| (i, q, true)),
            )
            .map(|(i, q, output)| (i, q, output, true))
            .chain(tag_port.as_ref().map(|(q, output)| (0, q, *output, false)));
        for (i, q, output, real) in ports {
            let pos = port_pos(r, kind, i, output);
            let area = Rect::from_center_size(pos, Vec2::splat(10.0));
            let resp = ui.interact(
                area,
                Id::unique(("port", &block.id, &q.name)),
                Sense::click_and_drag(),
            );
            let inverted = real
                && if output {
                    &block.inverted
                } else {
                    &block.inverted_inputs
                }
                .contains(&q.name);
            if inverted {
                // Small circle on the outer side of the port, as in FLProg.
                let rr = 3.2 * z.sqrt();
                let side = if output { rr } else { -rr };
                painter.circle(
                    pos + Vec2::new(side, 0.0),
                    rr,
                    pal.sheet,
                    Stroke::new(1.2, signal_color(&pal, q.ty)),
                );
            }
            if let Some(v) = (!output && real)
                .then(|| block.consts.get(&q.name))
                .flatten()
            {
                // Constant on an input: its value on a short stub, as in FLProg.
                let start = pos - Vec2::new(14.0 * z, 0.0);
                painter.line_segment([start, pos], Stroke::new(1.4, pal.text));
                painter.text(
                    start - Vec2::new(2.0 * z, 0.0),
                    Align2::RIGHT_CENTER,
                    value_text(v),
                    font(9.5),
                    pal.text,
                );
            }
            if let (true, true, Kind::Tag { write: false, .. }, Some(var)) =
                (output, real, kind, block.var_name())
            {
                if let Some((v, forced)) = dbg.and_then(|d| d.var(var)) {
                    debugview::value_label(&painter, &pal, pos, v, forced, z, Place::Right);
                } else if let Some(text) = dbg.and_then(|d| d.text_var(var)) {
                    debugview::text_label(&painter, &pal, pos, text, z, Place::Right);
                }
            }
            if output && kind == Kind::Box {
                let port = PortRef::new(&block.id, &q.name);
                if let Some((v, forced)) = dbg.and_then(|d| d.output(&port)) {
                    let v = flip(v, inverted);
                    debugview::value_label(&painter, &pal, pos, v, forced, z, Place::Right);
                } else if let Some(text) = dbg.and_then(|d| d.text_output(&port)) {
                    debugview::text_label(&painter, &pal, pos, text, z, Place::Right);
                }
            }
            if real {
                port_menus.push((PortRef::new(&block.id, &q.name), output, q.ty, resp.clone()));
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

    for (anchor, text) in text_labels {
        debugview::text_label(&painter, &pal, anchor, &text, z, Place::Left);
    }
    for (anchor, v, forced) in wire_labels {
        debugview::value_label(&painter, &pal, anchor, v, forced, z, Place::Left);
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
            let pts = rounded(&route(a, b, 12.0 * z, None), 6.0 * z);
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
    app.canvas.hover = pointer.filter(|p| rect.contains(*p)).and_then(drop_at);

    // Selection frame: left drag on empty space.
    let mut framed = None;
    if bg.drag_started_by(egui::PointerButton::Primary) && app.canvas.wire.is_none() {
        app.canvas.band = bg.interact_pointer_pos();
    }
    if let (Some(a), Some(b)) = (app.canvas.band, pointer) {
        let frame = Rect::from_two_pos(a, b);
        painter.rect(
            frame,
            CornerRadius::ZERO,
            pal.accent.gamma_multiply(0.12),
            Stroke::new(1.0, pal.accent),
            StrokeKind::Inside,
        );
        if bg.drag_stopped() || released {
            let ids: Vec<String> = logic
                .blocks
                .iter()
                .filter(|bl| screen_rect(&bl.id).is_some_and(|r| frame.intersects(r)))
                .map(|bl| bl.id.clone())
                .collect();
            framed = Some(Selection::of_blocks(ids));
            app.canvas.band = None;
        }
    }
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
    // Pan with the right or middle button; the left one draws a selection frame.
    let panning =
        bg.dragged_by(egui::PointerButton::Secondary) || bg.dragged_by(egui::PointerButton::Middle);
    if panning && app.canvas.wire.is_none() && app.canvas.band.is_none() {
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
    // Keep the sheets on screen: no empty space beyond their edges.
    let height = bands.last().map_or(0.0, |b| b.top + b.height);
    let content = Vec2::new(width, height) * view.zoom;
    let margin = Vec2::new(16.0, 12.0);
    let lo = rect.size() - margin - content;
    view.pan.x = view.pan.x.min(margin.x).max(lo.x.min(margin.x));
    view.pan.y = view.pan.y.min(margin.y).max(lo.y.min(margin.y));
    app.canvas.view = view;

    // Apply changes (the document was borrowed for drawing above).
    if bg.clicked() {
        app.select(clicked_link.map(Selection::Link));
    }
    if let Some(to) = wire_click {
        app.select(Some(Selection::Link(to)));
    }
    if let Some(s) = framed {
        app.select(s);
    }
    if let Some(s) = select {
        if let Some(Selection::Block(id)) = &s {
            if let Some(p) = app
                .doc
                .as_ref()
                .and_then(|d| d.project.layout.blocks.get(id))
            {
                app.sheet = p.sheet;
            }
        }
        app.select(s);
    }
    if let Some(s) = active_sheet {
        app.sheet = s;
    }
    let lib = app.lib.clone();
    let mut problems = Vec::new();
    let mut added = None;
    if let Some(doc) = &mut app.doc {
        let group = moves.len() > 1;
        for (id, delta) in moves {
            if let Some(&pos) = doc.project.layout.blocks.get(&id) {
                let moved = BlockPos {
                    x: pos.x + delta.x,
                    y: pos.y + delta.y,
                    ..pos
                };
                let key = if group {
                    "move:group".to_owned()
                } else {
                    format!("move:{id}")
                };
                let _ = doc.edit(&lib, Some(key), |p| p.move_block(&id, moved));
            }
        }
        let group_end = drag_end.as_ref().and_then(|id| {
            let ids = app.selection.as_ref()?.blocks();
            (ids.len() > 1 && ids.contains(id)).then(|| ids.to_vec())
        });
        if let Some(ids) = group_end {
            // A group stays on its sheets; positions snap to the grid.
            let _ = doc.edit(&lib, Some("move:group".to_owned()), |p| {
                for id in &ids {
                    if let Some(&pos) = p.layout.blocks.get(id) {
                        let snap = |v: f32| (v / GRID).round().max(0.0) * GRID;
                        p.move_block(
                            id,
                            BlockPos {
                                x: snap(pos.x),
                                y: snap(pos.y),
                                ..pos
                            },
                        )?;
                    }
                }
                Ok::<_, flp_core::EditError>(())
            });
            doc.end_edit();
        } else if let (Some(id), Some(pos)) = (&drag_end, landing) {
            let _ = doc.edit(&lib, Some(format!("move:{id}")), |p| p.move_block(id, pos));
            doc.end_edit();
        }
        if let Some((to, bend)) = wire_edit {
            let key = format!("wire:{}.{}", to.block, to.port);
            let _ = doc.edit(&lib, Some(key), |p| {
                p.set_wire_bend(&to, bend);
                Ok::<_, flp_core::EditError>(())
            });
        }
        if wire_end {
            doc.end_edit();
        }
        if let Some((to, broken)) = wire_break {
            let _ = doc.edit(&lib, None, |p| {
                p.set_wire_broken(&to, broken);
                Ok::<_, flp_core::EditError>(())
            });
        }
        if let Some((from, to)) = connect {
            if let Err(e) = doc.edit(&lib, None, |p| p.connect(&lib, from, to)) {
                problems.push(e.to_string());
            }
        }
        if let Some((item, pos)) = drop {
            let result = match item {
                DragItem::Block(kind) => doc.edit(&lib, None, |p| p.add_block(&lib, &kind, pos)),
                DragItem::Var(name) => doc.edit(&lib, None, |p| p.add_var_block(&name, false, pos)),
                DragItem::Widget(id) => doc.edit(&lib, None, |p| {
                    crate::netpanel::copy_widget(p, &lib, &id, pos)
                }),
                DragItem::Setting { id, name, ty, feed } => doc.edit(&lib, None, |p| {
                    crate::netpanel::pull_out(p, &lib, &id, &name, ty, feed, pos)
                }),
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
    for (port, output, ty, resp) in port_menus {
        port_context_menu(app, &port, output, ty, &resp);
    }
}

fn inverted_output(logic: &Logic, port: &PortRef) -> bool {
    logic
        .block(&port.block)
        .is_some_and(|b| b.inverted.contains(&port.port))
}

/// Live value as a consumer sees it: logical values flip when inverted.
fn flip(v: flp_debug::Value, invert: bool) -> flp_debug::Value {
    match v {
        flp_debug::Value::Bool(b) if invert => flp_debug::Value::Bool(!b),
        v => v,
    }
}

/// Constant typed in the menu of an input of type `ty`; `None` if it does not fit.
fn parse_const(ty: SignalType, text: &str) -> Option<ParamValue> {
    let t = text.trim();
    match ty {
        SignalType::Bool => match t {
            "1" | "true" => Some(ParamValue::Bool(true)),
            "0" | "false" => Some(ParamValue::Bool(false)),
            _ => None,
        },
        SignalType::Int => t.parse().ok().map(ParamValue::Int),
        SignalType::Float => t.replace(',', ".").parse().ok().map(ParamValue::Float),
        SignalType::String => Some(ParamValue::Text(text.to_owned())),
        SignalType::Any => Some(SignalType::guess_value(text)),
    }
}

/// Right click on a port: a constant on an input, inversion of a logical port
/// and, online, forcing an output or putting it on the chart.
fn port_context_menu(
    app: &mut EditorApp,
    port: &PortRef,
    output: bool,
    ty: SignalType,
    resp: &egui::Response,
) {
    let Some(block) = app
        .doc
        .as_ref()
        .and_then(|d| d.project.logic.block(&port.block))
    else {
        return;
    };
    let inverted = if output {
        &block.inverted
    } else {
        &block.inverted_inputs
    }
    .contains(&port.port);
    let constant = block.consts.get(&port.port).filter(|_| !output).cloned();
    if resp.secondary_clicked() {
        app.canvas.const_text = constant.as_ref().map(value_text).unwrap_or_default();
    }
    // An input has no storage of its own: it is forced through its source, which
    // changes the value for every consumer of that source.
    let debug_index = app.debug.as_ref().and_then(|d| {
        if output {
            return d.table.output_index(port);
        }
        let logic = &app.doc.as_ref()?.project.logic;
        let link = logic.links.iter().find(|l| &l.to == port)?;
        match logic.block(&link.from.block).and_then(|b| b.var_name()) {
            Some(var) => d.table.var_index(var),
            None => d.table.output_index(&link.from),
        }
    });
    let mut debug = app.debug.take();
    let mut set_const = None;
    let mut invert = None;
    let mut bad_const = false;
    let text = &mut app.canvas.const_text;
    resp.context_menu(|ui| {
        if let (Some(d), Some(index)) = (&mut debug, debug_index) {
            debugview::value_menu(ui, d, index);
            if !output {
                ui.weak("Форсируется источник входа (у всех потребителей)");
            }
            ui.separator();
        }
        if !output {
            let title = if constant.is_some() {
                "Изменить константу"
            } else {
                "Вставить константу"
            };
            // A click in the input field must not close the submenu.
            let config =
                MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            SubMenuButton::new(title).config(config).ui(ui, |ui| {
                if ty == SignalType::Bool {
                    for b in [true, false] {
                        if ui.button(b.to_string()).clicked() {
                            set_const = Some(Some(ParamValue::Bool(b)));
                            ui.close();
                        }
                    }
                    return;
                }
                // The menu style drops the vertical padding: give the field its own, and
                // centre the text, or it sticks to the top of a taller field.
                let edit = ui.add(
                    egui::TextEdit::singleline(text)
                        .margin(egui::Margin::symmetric(4, 3))
                        .align(egui::Align2::LEFT_CENTER),
                );
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Применить").clicked() || enter {
                    match parse_const(ty, text) {
                        Some(v) => {
                            set_const = Some(Some(v));
                            ui.close();
                        }
                        None => bad_const = true,
                    }
                }
            });
            if constant.is_some() && ui.button("Удалить константу").clicked() {
                set_const = Some(None);
            }
        }
        if ty == SignalType::Bool {
            let mut on = inverted;
            if ui.checkbox(&mut on, "Инвертировать").clicked() {
                invert = Some(on);
                ui.close();
            }
        }
    });
    app.debug = debug;
    if bad_const {
        let text = app.canvas.const_text.clone();
        app.info(
            LogLevel::Error,
            format!("«{text}» не подходит ко входу {port}: нужно число"),
        );
    }
    let lib = app.lib.clone();
    let Some(doc) = &mut app.doc else { return };
    let result = match (set_const, invert) {
        (Some(value), _) => doc.edit(&lib, None, |p| p.set_input_const(&lib, port, value)),
        (_, Some(on)) => doc.edit(&lib, None, |p| p.set_port_inverted(&lib, port, output, on)),
        _ => Ok(()),
    };
    if let Err(e) = result {
        app.info(LogLevel::Error, e.to_string());
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
    let mut edit_type = false;
    let library_kind = app
        .doc
        .as_ref()
        .and_then(|d| d.project.logic.block(id))
        .filter(|b| b.var_name().is_none())
        .and_then(|b| app.lib.get(&b.kind))
        .cloned();
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
        if let Some(block) = &library_kind {
            let text = if app.lib.is_user(&block.def.info.kind) {
                "Изменить тип блока..."
            } else {
                "Посмотреть тип блока..."
            };
            if ui.button(text).clicked() {
                edit_type = true;
            }
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
    if let (true, Some(block)) = (edit_type, &library_kind) {
        app.block_editor = Some(crate::blockedit::BlockEditor::edit(app, block));
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
        let r = route(Pos2::new(0.0, 0.0), Pos2::new(100.0, 40.0), 12.0, None);
        assert_eq!(r.len(), 4);
        assert_eq!(r[1], Pos2::new(50.0, 0.0));
    }

    #[test]
    fn bend_moves_the_middle_segment() {
        let r = route(
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 40.0),
            12.0,
            Some(30.0),
        );
        assert_eq!((r[1].x, r[2].x), (30.0, 30.0));
        // Never leaves the span between the ports.
        let r = route(
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 40.0),
            12.0,
            Some(500.0),
        );
        assert_eq!(r[1].x, 98.0);
        let r = route(Pos2::new(100.0, 0.0), Pos2::new(0.0, 0.0), 12.0, Some(77.0));
        assert_eq!((r[2].y, r[3].y), (77.0, 77.0));
    }

    #[test]
    fn backward_route_goes_around() {
        let r = route(Pos2::new(100.0, 0.0), Pos2::new(0.0, 0.0), 12.0, None);
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
