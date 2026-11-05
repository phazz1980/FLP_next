//! Read-only drawing of a sheet: blocks, ports, links; live values when simulating.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use flp_core::{Library, Project};
use flp_sim::Simulation;

const HEAD_H: f32 = 22.0;
const ROW_H: f32 = 18.0;
const WIDTH: f32 = 150.0;

struct Placed<'a> {
    rect: Rect,
    ins: Vec<&'a str>,
    outs: Vec<&'a str>,
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

pub fn show(ui: &mut egui::Ui, project: &Project, lib: &Library, sim: Option<&Simulation>) {
    let logic = &project.logic;
    let dark = ui.visuals().dark_mode;
    let (fg, box_bg, line) = if dark {
        (
            Color32::from_gray(225),
            Color32::from_gray(38),
            Color32::from_gray(120),
        )
    } else {
        (
            Color32::from_gray(30),
            Color32::from_gray(250),
            Color32::from_gray(110),
        )
    };
    let on = Color32::from_rgb(46, 160, 80);

    let mut placed: HashMap<&str, Placed> = HashMap::new();
    let mut extent = Vec2::ZERO;
    for (n, block) in logic.blocks.iter().enumerate() {
        let Some(def) = lib.resolve(logic, block) else {
            continue;
        };
        let pos = project.layout.blocks.get(&block.id);
        if pos.is_some_and(|p| project.layout.is_disabled(p.sheet)) {
            continue;
        }
        let (x, y) = pos.map_or(
            (40.0 + 180.0 * (n % 5) as f32, 40.0 + 90.0 * (n / 5) as f32),
            |p| (p.x, p.y + 1000.0 * (p.sheet.saturating_sub(1)) as f32),
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
        let rows = ins.len().max(outs.len()).max(1);
        let rect = Rect::from_min_size(
            Pos2::new(x, y),
            Vec2::new(WIDTH, HEAD_H + rows as f32 * ROW_H + 6.0),
        );
        extent = extent.max(rect.max.to_vec2());
        placed.insert(&block.id, Placed { rect, ins, outs });
    }

    egui::ScrollArea::both().show(ui, |ui| {
        let (resp, painter) = ui.allocate_painter(extent + Vec2::splat(60.0), Sense::hover());
        let origin = resp.rect.min.to_vec2();
        let port_pos = |p: &Placed, i: usize, output: bool| {
            let x = if output {
                p.rect.right()
            } else {
                p.rect.left()
            };
            Pos2::new(x, p.rect.top() + HEAD_H + (i as f32 + 0.5) * ROW_H) + origin
        };
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
            let (p, q) = (port_pos(a, i, true), port_pos(b, j, false));
            let value = sim.and_then(|s| s.output(&link.from.block, &link.from.port));
            let color = match value {
                Some(v) if v > 0.5 => on,
                _ => line,
            };
            let mid = if q.x > p.x + 20.0 {
                (p.x + q.x) / 2.0
            } else {
                p.x + 14.0
            };
            let pts = if q.x > p.x + 20.0 {
                vec![p, Pos2::new(mid, p.y), Pos2::new(mid, q.y), q]
            } else {
                let ym = a.rect.bottom().max(b.rect.bottom()) + origin.y + 12.0;
                vec![
                    p,
                    Pos2::new(mid, p.y),
                    Pos2::new(mid, ym),
                    Pos2::new(q.x - 14.0, ym),
                    Pos2::new(q.x - 14.0, q.y),
                    q,
                ]
            };
            painter.add(egui::Shape::line(pts, Stroke::new(1.6, color)));
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
            let symbol = def.def.info.symbol.as_deref().unwrap_or(&def.def.info.name);
            painter.text(
                rect.left_top() + Vec2::new(6.0, 3.0),
                egui::Align2::LEFT_TOP,
                symbol,
                egui::FontId::proportional(12.0),
                fg,
            );
            painter.text(
                rect.right_top() + Vec2::new(-6.0, 4.0),
                egui::Align2::RIGHT_TOP,
                &block.id,
                egui::FontId::proportional(10.0),
                line,
            );
            for (i, name) in p.ins.iter().enumerate() {
                let at = port_pos(p, i, false);
                painter.circle_filled(at, 3.0, line);
                let title = def.def.input(name).map_or(*name, |d| d.title());
                painter.text(
                    at + Vec2::new(6.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(10.0),
                    fg,
                );
            }
            for (i, name) in p.outs.iter().enumerate() {
                let at = port_pos(p, i, true);
                let def_out = def.def.output(name);
                let value = sim.and_then(|s| s.output(&block.id, name));
                let high = value.is_some_and(|v| v > 0.5);
                painter.circle_filled(at, 3.5, if high { on } else { line });
                let title = def_out.map_or(*name, |d| d.title());
                let text = match (value, def_out) {
                    (Some(v), Some(d)) => format!(
                        "{title} = {}",
                        text_of(v, d.ty == flp_core::SignalType::Bool)
                    ),
                    _ => title.to_owned(),
                };
                painter.text(
                    at + Vec2::new(-6.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    text,
                    egui::FontId::proportional(10.0),
                    if high { on } else { fg },
                );
            }
        }
    });
}
