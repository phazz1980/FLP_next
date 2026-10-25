//! Picture of all sheets of a project as PDF, one page per sheet.
//!
//! The drawing follows the editor canvas (block boxes, variable tags, orthogonal
//! wires) in a fixed light palette, so the schema can be viewed and printed
//! without the editor. It is stored in the project archive on every save.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

use flp_core::project::BOARDS_PDF;
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{Block, BlockDef, Library, ParamValue, PortDef, Project, SignalType};
use printpdf::path::PaintMode;
use printpdf::{
    Color, CustomPdfConformance, IndirectFontRef, Line, Mm, PdfConformance, PdfDocument,
    PdfLayerReference, Point, Polygon, Rect, Rgb,
};

// Geometry in canvas units, the same as in the editor.
const HEAD_H: f32 = 16.0;
const ROW_H: f32 = 14.0;
const TAG_H: f32 = 18.0;
const PORT_R: f32 = 3.0;
const GRID: f32 = 20.0;
const SHEET_HEADER: f32 = 40.0;
const SHEET_MIN_BODY: f32 = 300.0;
const SHEET_MIN_W: f32 = 1200.0;
const SHEET_PAD: f32 = 60.0;

/// Millimetres per canvas unit: a minimal sheet fits A4 landscape.
const MM: f32 = 0.25;
/// Points per canvas unit, for font sizes.
const PT: f32 = MM * 72.0 / 25.4;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("не найден шрифт с кириллицей для PDF (искали {0})")]
    NoFont(String),
    #[error("не удалось построить PDF: {0}")]
    Pdf(#[from] printpdf::Error),
}

/// Renders every sheet of the project; blocks without coordinates are skipped.
pub fn render(project: &Project, lib: &Library) -> Result<Vec<u8>, PdfError> {
    let font_data = load_font()?;
    let title = &project.config.project.name;
    let logic = &project.logic;
    let layout = &project.layout;

    let mut sheets: BTreeSet<u32> = layout.blocks.values().map(|p| p.sheet).collect();
    sheets.extend(layout.sheets.iter().map(|s| s.number));
    sheets.insert(1);

    let items: Vec<Item> = logic
        .blocks
        .iter()
        .filter_map(|b| {
            let pos = layout.blocks.get(&b.id)?;
            let def = lib.resolve(logic, b).map(|l| &l.def);
            let (w, h) = size(b, def);
            Some(Item {
                block: b,
                def,
                sheet: pos.sheet,
                rect: Bounds {
                    x: pos.x,
                    y: pos.y,
                    w,
                    h,
                },
            })
        })
        .collect();
    let by_id: HashMap<&str, &Item> = items.iter().map(|i| (i.block.id.as_str(), i)).collect();

    // No PDF/X conformance: it would embed a 650 KB colour profile.
    let doc = PdfDocument::empty(title.as_str())
        .with_conformance(PdfConformance::Custom(CustomPdfConformance::default()));
    let font = doc.add_external_font(font_data.as_slice())?;
    for &sheet in &sheets {
        let on_sheet: Vec<&Item> = items.iter().filter(|i| i.sheet == sheet).collect();
        let right = on_sheet
            .iter()
            .map(|i| i.rect.x + i.rect.w)
            .fold(0.0, f32::max);
        let bottom = on_sheet
            .iter()
            .map(|i| i.rect.y + i.rect.h)
            .fold(0.0, f32::max);
        let w = (right + SHEET_PAD * 2.0).max(SHEET_MIN_W);
        let h = SHEET_HEADER + (bottom + SHEET_PAD).max(SHEET_MIN_BODY);
        let (page, layer) = doc.add_page(Mm(w * MM), Mm(h * MM), format!("Плата {sheet}"));
        let canvas = Canvas {
            layer: doc.get_page(page).get_layer(layer),
            font: &font,
            height: h,
        };

        let info = layout.sheet(sheet);
        let mut head = format!("Плата {sheet}");
        if let Some(name) = info.map(|i| i.name.as_str()).filter(|n| !n.is_empty()) {
            head = format!("{head}. {name}");
        }
        canvas.text(16.0, 16.0, &head, 12.0, Align::Left, INK);
        canvas.text(w - 16.0, 16.0, title, 9.0, Align::Right, WEAK);
        if let Some(comment) = info.map(|i| i.comment.as_str()).filter(|c| !c.is_empty()) {
            canvas.text(16.0, 30.0, comment, 9.0, Align::Left, WEAK);
        }
        canvas.line(&[(0.0, SHEET_HEADER), (w, SHEET_HEADER)], 0.6, GRID_LINE);

        let origin = |b: Bounds| Bounds {
            y: b.y + SHEET_HEADER,
            ..b
        };
        // Wires first, so blocks cover their ends.
        for link in &logic.links {
            let (Some(from), Some(to)) = (
                by_id.get(link.from.block.as_str()),
                by_id.get(link.to.block.as_str()),
            ) else {
                continue;
            };
            if from.sheet != sheet || to.sheet != sheet {
                continue;
            }
            let (Some(a), Some(b)) = (
                port_index(from, &link.from.port, true)
                    .map(|i| port_pos(origin(from.rect), kind_of(from), i, true)),
                port_index(to, &link.to.port, false)
                    .map(|i| port_pos(origin(to.rect), kind_of(to), i, false)),
            ) else {
                continue;
            };
            let ty = from
                .def
                .and_then(|d| d.outputs.iter().find(|p| p.name == link.from.port))
                .map_or(SignalType::Bool, |p| p.ty);
            canvas.line(&route(a, b, GRID * 0.5), 1.0, signal_color(ty));
        }
        for item in &on_sheet {
            let fed = |port: &str| {
                logic
                    .links
                    .iter()
                    .any(|l| l.to.block == item.block.id && l.to.port == port)
            };
            draw_block(&canvas, item, origin(item.rect), fed);
        }
    }
    Ok(doc.save_to_bytes()?)
}

/// Saves the project archive together with [`BOARDS_PDF`]. If the picture cannot
/// be drawn, the project is still saved without it and the reason is returned
/// as a warning.
pub fn save_project(project: &Project, lib: &Library) -> flp_core::Result<Option<String>> {
    match render(project, lib) {
        Ok(pdf) => project.save_with(&[(BOARDS_PDF, &pdf)]).map(|()| None),
        Err(e) => project
            .save()
            .map(|()| Some(format!("{BOARDS_PDF} не обновлён: {e}"))),
    }
}

/// A TrueType font with Cyrillic: `FLP_PDF_FONT`, otherwise a standard Windows
/// font. Tahoma and Segoe UI come first: their subsets are small, while Arial
/// is embedded whole (about 1 MB).
fn load_font() -> Result<Vec<u8>, PdfError> {
    let fonts_dir = std::env::var_os("WINDIR")
        .map(|w| PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\Fonts"));
    let candidates: Vec<PathBuf> = std::env::var_os("FLP_PDF_FONT")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            ["tahoma.ttf", "segoeui.ttf", "arial.ttf", "verdana.ttf"]
                .iter()
                .map(|f| fonts_dir.join(f)),
        )
        .chain([PathBuf::from(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        )])
        .collect();
    candidates
        .iter()
        .find_map(|p| fs::read(p).ok())
        .ok_or_else(|| PdfError::NoFont(fonts_dir.join("tahoma.ttf").display().to_string()))
}

struct Item<'a> {
    block: &'a Block,
    def: Option<&'a BlockDef>,
    sheet: u32,
    /// Position relative to the sheet body, as in `layout.toml`.
    rect: Bounds,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Box,
    Tag { write: bool, en: bool },
}

fn kind_of(item: &Item) -> Kind {
    match item.block.kind.as_str() {
        VAR_GET => Kind::Tag {
            write: false,
            en: false,
        },
        VAR_SET => Kind::Tag {
            write: true,
            en: item.def.is_some_and(|d| d.inputs.len() > 1),
        },
        _ => Kind::Box,
    }
}

/// Approximate text width, as in the editor.
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

/// Block size in canvas units; must match the editor so coordinates line up.
fn size(block: &Block, def: Option<&BlockDef>) -> (f32, f32) {
    let text = label(block, def);
    match block.kind.as_str() {
        VAR_GET | VAR_SET => {
            let en = block.kind == VAR_SET && def.is_some_and(|d| d.inputs.len() > 1);
            (
                text_w(&text, 11.0) + 18.0,
                if en { TAG_H * 2.0 } else { TAG_H },
            )
        }
        _ => {
            let (ins, outs): (Vec<&PortDef>, Vec<&PortDef>) =
                def.map_or_else(Default::default, |d| {
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
            ((w / 4.0).ceil() * 4.0, HEAD_H + rows as f32 * ROW_H + 4.0)
        }
    }
}

/// Index of a port among the drawn ones of its side.
fn port_index(item: &Item, port: &str, output: bool) -> Option<usize> {
    if let Kind::Tag { .. } = kind_of(item) {
        let inputs = item.def.map(|d| d.inputs.as_slice()).unwrap_or_default();
        return Some(if output {
            0
        } else {
            inputs.iter().position(|p| p.name == port).unwrap_or(0)
        });
    }
    let def = item.def?;
    let params = &item.block.params;
    if output {
        def.active_outputs(params).position(|p| p.name == port)
    } else {
        def.active_inputs(params).position(|p| p.name == port)
    }
}

fn port_pos(r: Bounds, kind: Kind, index: usize, output: bool) -> (f32, f32) {
    let x = if output { r.x + r.w } else { r.x };
    match kind {
        Kind::Tag { en: false, .. } => (x, r.y + r.h * 0.5),
        Kind::Tag { en: true, .. } => {
            let row = if output { 0.0 } else { index as f32 };
            (x, r.y + r.h * (0.25 + 0.5 * row))
        }
        Kind::Box => (x, r.y + HEAD_H + ROW_H * (index as f32 + 0.5)),
    }
}

/// Orthogonal route from an output to an input, as in the editor.
fn route(a: (f32, f32), b: (f32, f32), step: f32) -> Vec<(f32, f32)> {
    if b.0 - a.0 >= 2.0 * step {
        let mx = (a.0 + b.0) * 0.5;
        vec![a, (mx, a.1), (mx, b.1), b]
    } else {
        let my = if (b.1 - a.1).abs() > 3.0 * step {
            (a.1 + b.1) * 0.5
        } else {
            a.1.max(b.1) + 2.5 * step
        };
        vec![
            a,
            (a.0 + step, a.1),
            (a.0 + step, my),
            (b.0 - step, my),
            (b.0 - step, b.1),
            b,
        ]
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

fn draw_block(c: &Canvas, item: &Item, r: Bounds, fed: impl Fn(&str) -> bool) {
    let (block, def) = (item.block, item.def);
    let kind = kind_of(item);
    match kind {
        Kind::Tag { write, en } => {
            c.rect(r, if write { HEADER } else { TAG_FILL }, BORDER);
            let cy = if en {
                r.y + r.h * 0.25
            } else {
                r.y + r.h * 0.5
            };
            c.text(
                r.x + r.w * 0.5,
                cy,
                &label(block, def),
                11.0,
                Align::Center,
                INK,
            );
            if en {
                let p = port_pos(r, kind, 1, false);
                c.text(p.0 + 6.0, p.1, "En", 9.5, Align::Left, INK);
                c.dot(p, signal_color(SignalType::Bool));
            }
            if let Some(port) = def.and_then(|d| d.inputs.first().or(d.outputs.first())) {
                c.dot(port_pos(r, kind, 0, !write), signal_color(port.ty));
            }
        }
        Kind::Box => {
            c.rect(r, FILL, BORDER);
            c.rect(Bounds { h: HEAD_H, ..r }, HEADER, BORDER);
            let head = label(block, def);
            c.text(
                r.x + r.w * 0.5,
                r.y + HEAD_H * 0.5,
                &head,
                10.5,
                Align::Center,
                INK,
            );
            c.text(r.x, r.y - 6.0, &block.id, 9.5, Align::Left, WEAK);
            let Some(def) = def else {
                c.text(
                    r.x + r.w * 0.5,
                    r.y + r.h * 0.6,
                    "?",
                    12.0,
                    Align::Center,
                    ERROR,
                );
                return;
            };
            for (i, port) in def.active_inputs(&block.params).enumerate() {
                let p = port_pos(r, kind, i, false);
                c.dot(p, signal_color(port.ty));
                c.text(p.0 + 5.0, p.1, port.title(), 9.0, Align::Left, WEAK);
                if !fed(&port.name) {
                    if let Some(d) = &port.default {
                        c.text(p.0 - 5.0, p.1, &value_text(d), 9.0, Align::Right, WEAK);
                    }
                }
            }
            for (i, port) in def.active_outputs(&block.params).enumerate() {
                let p = port_pos(r, kind, i, true);
                c.dot(p, signal_color(port.ty));
                c.text(p.0 - 5.0, p.1, port.title(), 9.0, Align::Right, WEAK);
            }
            let summary: Vec<String> = def
                .params
                .iter()
                .filter_map(|p| {
                    block.params.get(&p.name).map(|v| {
                        // The PDF is easy to pass around: keep passwords out of it.
                        let text = if p.name.contains("password") {
                            "***".to_owned()
                        } else {
                            value_text(v)
                        };
                        format!("{}={text}", p.title())
                    })
                })
                .collect();
            if !summary.is_empty() {
                c.text(
                    r.x,
                    r.y + r.h + 8.0,
                    &summary.join(" "),
                    9.0,
                    Align::Left,
                    WEAK,
                );
            }
        }
    }
}

type Rgb8 = (u8, u8, u8);

const INK: Rgb8 = (0x20, 0x20, 0x20);
const WEAK: Rgb8 = (0x70, 0x70, 0x70);
const ERROR: Rgb8 = (0xc0, 0x30, 0x30);
const BORDER: Rgb8 = (0x50, 0x50, 0x50);
const FILL: Rgb8 = (0xff, 0xff, 0xff);
const HEADER: Rgb8 = (0xdd, 0xe6, 0xf0);
const TAG_FILL: Rgb8 = (0xee, 0xf4, 0xe4);
const GRID_LINE: Rgb8 = (0xb0, 0xb0, 0xb0);

fn signal_color(ty: SignalType) -> Rgb8 {
    match ty {
        SignalType::Bool => (0x2e, 0x8b, 0x3a),
        SignalType::Int => (0x2a, 0x5d, 0xb0),
        SignalType::Float => (0xc0, 0x6a, 0x10),
    }
}

fn color((r, g, b): Rgb8) -> Color {
    Color::Rgb(Rgb::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        None,
    ))
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

/// One page; takes canvas coordinates (y down) and converts them to PDF (y up, mm).
struct Canvas<'a> {
    layer: PdfLayerReference,
    font: &'a IndirectFontRef,
    height: f32,
}

impl Canvas<'_> {
    fn pt(&self, (x, y): (f32, f32)) -> Point {
        Point::new(Mm(x * MM), Mm((self.height - y) * MM))
    }

    fn line(&self, points: &[(f32, f32)], width: f32, rgb: Rgb8) {
        self.layer.set_outline_color(color(rgb));
        self.layer.set_outline_thickness(width * PT);
        self.layer.add_line(Line {
            points: points.iter().map(|&p| (self.pt(p), false)).collect(),
            is_closed: false,
        });
    }

    fn rect(&self, r: Bounds, fill: Rgb8, border: Rgb8) {
        self.layer.set_fill_color(color(fill));
        self.layer.set_outline_color(color(border));
        self.layer.set_outline_thickness(0.8 * PT);
        self.layer.add_rect(
            Rect::new(
                Mm(r.x * MM),
                Mm((self.height - r.y - r.h) * MM),
                Mm((r.x + r.w) * MM),
                Mm((self.height - r.y) * MM),
            )
            .with_mode(PaintMode::FillStroke),
        );
    }

    fn dot(&self, (x, y): (f32, f32), rgb: Rgb8) {
        // An octagon is round enough at this size.
        let ring = (0..8)
            .map(|k| {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                (self.pt((x + PORT_R * a.cos(), y + PORT_R * a.sin())), false)
            })
            .collect();
        self.layer.set_fill_color(color(rgb));
        self.layer.add_polygon(Polygon {
            rings: vec![ring],
            mode: PaintMode::Fill,
            winding_order: Default::default(),
        });
    }

    /// Text vertically centred on `y`.
    fn text(&self, x: f32, y: f32, text: &str, size: f32, align: Align, rgb: Rgb8) {
        let w = text_w(text, size);
        let x = match align {
            Align::Left => x,
            Align::Center => x - w * 0.5,
            Align::Right => x - w,
        };
        self.layer.set_fill_color(color(rgb));
        let baseline = y + size * 0.35;
        self.layer.use_text(
            text,
            size * PT,
            Mm(x * MM),
            Mm((self.height - baseline) * MM),
            self.font,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pump_example() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (lib, _) = Library::load(root.join("library")).unwrap();
        let project = Project::load(root.join("examples/pump")).unwrap();
        match render(&project, &lib) {
            Ok(pdf) => {
                assert!(pdf.starts_with(b"%PDF"));
                if let Some(out) = std::env::var_os("FLP_PDF_OUT") {
                    fs::write(out, &pdf).unwrap();
                }
            }
            // No system font on this machine: nothing to check.
            Err(PdfError::NoFont(_)) => {}
            Err(e) => panic!("{e}"),
        }
    }
}
