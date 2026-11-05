//! Read-only drawing of a sheet: blocks, ports, links; live values when simulating.

use std::collections::{BTreeSet, HashMap};

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use flp_core::{vars, Layout, Library, Project};
use flp_sim::Simulation;

// Geometry mirrors the editor canvas so a schema looks the way it was laid out.
const HEAD_H: f32 = 20.0;
const ROW_H: f32 = 20.0;
const TAG_H: f32 = 20.0;
const GRID: f32 = 10.0;
const SHEET_HEADER: f32 = 40.0;
const SHEET_MIN_BODY: f32 = 300.0;
const SHEET_GAP: f32 = 14.0;
const SHEET_PAD: f32 = 60.0;
const MIN_WIDTH: f32 = 1200.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Box,
    Tag { en: bool },
}

struct Placed<'a> {
    rect: Rect,
    kind: Kind,
    sheet: u32,
    ins: Vec<&'a str>,
    outs: Vec<&'a str>,
}

/// A block with its position inside its sheet, before the sheets are stacked.
struct Local<'a> {
    pos: Pos2,
    size: Vec2,
    kind: Kind,
    sheet: u32,
    ins: Vec<&'a str>,
    outs: Vec<&'a str>,
}

fn text_w(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.58
}

fn text_of(v: f64, bool_like: bool) -> String {
    if bool_like {
        if v > 0.5 { "1" } else { "0" }.to_owned()
    } else if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    }
}

/// Orthogonal route from an output to an input; `bend` moves the middle segment.
fn route(a: Pos2, b: Pos2, step: f32, bend: Option<f32>) -> Vec<Pos2> {
    if b.x - a.x >= 2.0 * step {
        let mx = bend.map_or((a.x + b.x) * 0.5, |x| x.clamp(a.x + 2.0, b.x - 2.0));
        vec![a, Pos2::new(mx, a.y), Pos2::new(mx, b.y), b]
    } else {
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

fn port_pos(rect: Rect, kind: Kind, index: usize, output: bool) -> Pos2 {
    let x = if output { rect.max.x } else { rect.min.x };
    match kind {
        Kind::Tag { en: false } => Pos2::new(x, rect.center().y),
        Kind::Tag { en: true } => {
            let row = if output { 0.0 } else { index as f32 };
            Pos2::new(x, rect.min.y + rect.height() * (0.25 + 0.5 * row))
        }
        Kind::Box => Pos2::new(x, rect.min.y + HEAD_H + ROW_H * (index as f32 + 0.5)),
    }
}

pub fn show(ui: &mut egui::Ui, project: &Project, lib: &Library, sim: Option<&Simulation>) {
    let logic = &project.logic;
    let layout = &project.layout;
    let dark = ui.visuals().dark_mode;
    let (fg, box_bg, line, frame) = if dark {
        (
            Color32::from_gray(225),
            Color32::from_gray(38),
            Color32::from_gray(120),
            Color32::from_gray(26),
        )
    } else {
        (
            Color32::from_gray(30),
            Color32::from_gray(250),
            Color32::from_gray(110),
            Color32::from_gray(240),
        )
    };
    let on = Color32::from_rgb(46, 160, 80);

    // Sheets in number order, each one a band below the previous.
    let mut sheets: BTreeSet<u32> = BTreeSet::from([1]);
    sheets.extend(layout.blocks.values().map(|p| p.sheet));
    sheets.extend(layout.sheets.iter().map(|s| s.number));
    sheets.retain(|n| !layout.is_disabled(*n));

    let mut locals: Vec<(&str, Local)> = Vec::new();
    for (n, block) in logic.blocks.iter().enumerate() {
        let Some(def) = lib.resolve(logic, block) else {
            continue;
        };
        let at = layout.blocks.get(&block.id);
        let sheet = at.map_or(1, |p| p.sheet);
        if !sheets.contains(&sheet) {
            continue;
        }
        let (x, y) = at.map_or(
            (40.0 + 180.0 * (n % 5) as f32, 40.0 + 90.0 * (n / 5) as f32),
            |p| (p.x, p.y),
        );
        let ins: Vec<&str> = def
            .def
            .active_inputs(&block.params)
            .map(|p| p.name.as_str())
            .collect();
        let outs: Vec<&str> = def
            .def
            .active_outputs(&block.params)
            .map(|p| p.name.as_str())
            .collect();
        let label = block.var_name().map(str::to_owned).unwrap_or_else(|| {
            def.def
                .info
                .symbol
                .clone()
                .unwrap_or_else(|| def.def.info.name.clone())
        });
        let (kind, size) = match block.kind.as_str() {
            vars::VAR_GET | vars::VAR_SET => {
                let en = block.kind == vars::VAR_SET && def.def.inputs.len() > 1;
                (
                    Kind::Tag { en },
                    Vec2::new(
                        ((text_w(&label, 11.0) + 18.0) / GRID).ceil() * GRID,
                        if en { TAG_H * 2.0 } else { TAG_H },
                    ),
                )
            }
            _ => {
                let rows = ins.len().max(outs.len()).max(1);
                let longest = |names: &[&str], input: bool| {
                    names
                        .iter()
                        .map(|n| {
                            let title = if input {
                                def.def.input(n).map_or(*n, |d| d.title())
                            } else {
                                def.def.output(n).map_or(*n, |d| d.title())
                            };
                            text_w(title, 9.5)
                        })
                        .fold(0.0, f32::max)
                };
                let ports_w = longest(&ins, true) + longest(&outs, false) + 22.0;
                let w = (text_w(&label, 11.0) + 16.0).max(ports_w).max(64.0);
                (
                    Kind::Box,
                    Vec2::new(
                        (w / GRID).ceil() * GRID,
                        HEAD_H + rows as f32 * ROW_H + 10.0,
                    ),
                )
            }
        };
        locals.push((
            &block.id,
            Local {
                pos: Pos2::new(x, y),
                size,
                kind,
                sheet,
                ins,
                outs,
            },
        ));
    }

    // Top and height of each sheet band.
    let mut bands: HashMap<u32, (f32, f32)> = HashMap::new();
    let mut top = 0.0;
    for sheet in &sheets {
        let bottom = locals
            .iter()
            .filter(|(_, l)| l.sheet == *sheet)
            .map(|(_, l)| l.pos.y + l.size.y)
            .fold(0.0, f32::max);
        let height = SHEET_HEADER + (bottom + SHEET_PAD).max(SHEET_MIN_BODY);
        bands.insert(*sheet, (top, height));
        top += height + SHEET_GAP;
    }
    let mut width = MIN_WIDTH;
    let mut placed: HashMap<&str, Placed> = HashMap::new();
    for (id, l) in locals {
        let rect = Rect::from_min_size(
            l.pos + Vec2::new(0.0, bands[&l.sheet].0 + SHEET_HEADER),
            l.size,
        );
        width = width.max(rect.max.x + SHEET_PAD);
        placed.insert(
            id,
            Placed {
                rect,
                kind: l.kind,
                sheet: l.sheet,
                ins: l.ins,
                outs: l.outs,
            },
        );
    }

    egui::ScrollArea::both().show(ui, |ui| {
        let (resp, painter) = ui.allocate_painter(Vec2::new(width, top), Sense::hover());
        let origin = resp.rect.min.to_vec2();
        for sheet in &sheets {
            let (band_top, height) = bands[sheet];
            let r = Rect::from_min_size(Pos2::new(0.0, band_top), Vec2::new(width, height))
                .translate(origin);
            painter.rect(
                r,
                4.0,
                frame,
                Stroke::new(1.0, line),
                egui::StrokeKind::Inside,
            );
            let name = layout
                .sheet(*sheet)
                .map(|s| s.name.as_str())
                .filter(|n| !n.is_empty())
                .map_or_else(|| format!("Лист {sheet}"), str::to_owned);
            painter.text(
                r.left_top() + Vec2::new(14.0, SHEET_HEADER * 0.5),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(14.0),
                fg,
            );
        }
        let pos_of =
            |p: &Placed, i: usize, output: bool| port_pos(p.rect, p.kind, i, output) + origin;
        // Links first, under the blocks.
        for link in &logic.links {
            let (Some(a), Some(b)) = (
                placed.get(link.from.block.as_str()),
                placed.get(link.to.block.as_str()),
            ) else {
                continue;
            };
            let (Some(i), Some(j)) = (
                a.outs.iter().position(|n| *n == link.from.port),
                b.ins.iter().position(|n| *n == link.to.port),
            ) else {
                continue;
            };
            let (p, q) = (pos_of(a, i, true), pos_of(b, j, false));
            let value = sim.and_then(|s| s.output(&link.from.block, &link.from.port));
            let color = match value {
                Some(v) if v > 0.5 => on,
                _ => line,
            };
            let wire = layout.wires.get(&Layout::wire_key(&link.to));
            if a.sheet != b.sheet || wire.is_some_and(|w| w.broken) {
                // Two labelled stubs, as for a link between sheets in the editor.
                let font = egui::FontId::proportional(10.0);
                painter.line_segment([p, p + Vec2::new(16.0, 0.0)], Stroke::new(1.6, color));
                painter.text(
                    p + Vec2::new(19.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    format!("→ {}.{}", link.to.block, link.to.port),
                    font.clone(),
                    color,
                );
                painter.line_segment([q - Vec2::new(16.0, 0.0), q], Stroke::new(1.6, color));
                painter.text(
                    q - Vec2::new(19.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    format!("{}.{} →", link.from.block, link.from.port),
                    font,
                    color,
                );
                continue;
            }
            // A manual bend is in sheet coordinates.
            let bend = wire.and_then(|w| w.bend).map(|v| {
                if q.x - p.x >= 20.0 {
                    v + origin.x
                } else {
                    v + origin.y + bands[&a.sheet].0 + SHEET_HEADER
                }
            });
            painter.add(egui::Shape::line(
                route(p, q, 10.0, bend),
                Stroke::new(1.6, color),
            ));
        }
        for block in &logic.blocks {
            let (Some(p), Some(def)) = (placed.get(block.id.as_str()), lib.resolve(logic, block))
            else {
                continue;
            };
            let rect = p.rect.translate(origin);
            painter.rect(
                rect,
                4.0,
                box_bg,
                Stroke::new(1.2, fg),
                egui::StrokeKind::Inside,
            );
            if matches!(p.kind, Kind::Tag { .. }) {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    block.var_name().unwrap_or(&block.id),
                    egui::FontId::proportional(11.0),
                    fg,
                );
            } else {
                let symbol = def.def.info.symbol.as_deref().unwrap_or(&def.def.info.name);
                painter.text(
                    rect.left_top() + Vec2::new(6.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    symbol,
                    egui::FontId::proportional(12.0),
                    fg,
                );
                painter.text(
                    rect.left_top() + Vec2::new(0.0, -2.0),
                    egui::Align2::LEFT_BOTTOM,
                    &block.id,
                    egui::FontId::proportional(9.0),
                    line,
                );
            }
            for (i, name) in p.ins.iter().enumerate() {
                let at = pos_of(p, i, false);
                painter.circle_filled(at, 3.0, line);
                if p.kind == Kind::Box {
                    let title = def.def.input(name).map_or(*name, |d| d.title());
                    painter.text(
                        at + Vec2::new(6.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        title,
                        egui::FontId::proportional(9.5),
                        fg,
                    );
                }
            }
            for (i, name) in p.outs.iter().enumerate() {
                let at = pos_of(p, i, true);
                let def_out = def.def.output(name);
                let value = sim.and_then(|s| s.output(&block.id, name));
                let high = value.is_some_and(|v| v > 0.5);
                painter.circle_filled(at, 3.5, if high { on } else { line });
                if p.kind != Kind::Box {
                    continue;
                }
                let title = def_out.map_or(*name, |d| d.title());
                let shown = match (value, def_out) {
                    (Some(v), Some(d)) => {
                        format!(
                            "{title} = {}",
                            text_of(v, d.ty == flp_core::SignalType::Bool)
                        )
                    }
                    _ => title.to_owned(),
                };
                painter.text(
                    at + Vec2::new(-6.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    shown,
                    egui::FontId::proportional(9.5),
                    if high { on } else { fg },
                );
            }
        }
    });
}
