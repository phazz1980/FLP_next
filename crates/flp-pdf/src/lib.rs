//! Picture of all sheets of a project as PDF, one page per sheet.
//!
//! The drawing follows the editor canvas (block boxes, variable tags, orthogonal
//! wires) in a fixed light palette, so the schema can be viewed and printed
//! without the editor. After the drawing of every sheet goes a page with the
//! parameters of its blocks; a click on a block opens its row. The page is written as Typst markup and compiled
//! by Typst in the `crablogic-pdf` helper (`flp-pdf-render`); it is stored in the project archive on every save.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flp_core::project::BOARDS_PDF;
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{
    Block, BlockDef, Library, LocalValues, ParamShow, ParamValue, PortDef, Project, SignalType,
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

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("не найден {HELPER} рядом с программой")]
    NoHelper,
    #[error("не удалось запустить {HELPER}: {0}")]
    Spawn(String),
    #[error("Typst не смог построить PDF: {0}")]
    Typst(String),
}

/// Typst lives only in this executable (it is large); everything else starts it.
const HELPER: &str = "crablogic-pdf.exe";

/// Typst helpers: one canvas unit is a quarter of a millimetre.
const PRELUDE: &str = r##"#let u = 0.25mm
#set text(font: FONTS, lang: "ru")
#let T(x, y, w, al, sz, c, s) = place(top + left, dx: x * u, dy: (y - sz) * u,
  box(width: w * u, height: 2 * sz * u, align(al + horizon, text(size: sz * u, fill: rgb(c), s))))
#let W(c, wd, ..p) = {
  let p = p.pos()
  for i in range(p.len() - 1) {
    place(top + left, line(start: (p.at(i).at(0) * u, p.at(i).at(1) * u),
      end: (p.at(i + 1).at(0) * u, p.at(i + 1).at(1) * u),
      stroke: stroke(paint: rgb(c), thickness: wd * u, cap: "round")))
  }
}
#let R(x, y, w, h, f, b) = place(top + left, dx: x * u, dy: y * u,
  rect(width: w * u, height: h * u, fill: rgb(f), stroke: 0.8 * u + rgb(b)))
#let D(x, y, r, c) = place(top + left, dx: (x - r) * u, dy: (y - r) * u,
  circle(radius: r * u, fill: rgb(c), stroke: none))
"##;

/// Renders every sheet of the project; blocks without coordinates are skipped.
pub fn render(project: &Project, lib: &Library) -> Result<Vec<u8>, PdfError> {
    run_helper(&helper_path()?, source(project, lib))
}

/// The helper lies next to the running program (in tests, one level up from `deps`).
fn helper_path() -> Result<PathBuf, PdfError> {
    if let Some(p) = std::env::var_os("CRABLOGIC_PDF") {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| PdfError::Spawn(e.to_string()))?;
    exe.ancestors()
        .skip(1)
        .take(2)
        .map(|d| d.join(HELPER))
        .find(|p| p.is_file())
        .ok_or(PdfError::NoHelper)
}

/// Feeds the markup to the helper and takes the PDF back from its stdout.
fn run_helper(helper: &Path, source: String) -> Result<Vec<u8>, PdfError> {
    let mut cmd = Command::new(helper);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up when started from the editor.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| PdfError::Spawn(e.to_string()))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // Written from a thread: the helper may fill its stdout pipe before it has read everything.
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| PdfError::Spawn(e.to_string()))?;
    let _ = writer.join();
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(PdfError::Typst(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ))
    }
}

/// The Typst markup of the whole document; see [`render`].
pub fn source(project: &Project, lib: &Library) -> String {
    let title = &project.config.project.name;
    let logic = &project.logic;
    let layout = &project.layout;

    let mut sheets: BTreeSet<u32> = layout.blocks.values().map(|p| p.sheet).collect();
    sheets.extend(layout.sheets.iter().map(|s| s.number));
    sheets.insert(1);

    let mut items: Vec<Item> = logic
        .blocks
        .iter()
        .filter_map(|b| {
            let pos = layout.blocks.get(&b.id)?;
            let def = lib.resolve(logic, b).map(|l| &l.def);
            let (w, h) = size(b, def);
            Some(Item {
                num: 0,
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
    // Boxes are numbered across the project (sheet, top to bottom, left to right).
    let mut order: Vec<usize> = (0..items.len())
        .filter(|&i| kind_of(&items[i]) == Kind::Box)
        .collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&items[a], &items[b]);
        a.sheet
            .cmp(&b.sheet)
            .then(a.rect.y.total_cmp(&b.rect.y))
            .then(a.rect.x.total_cmp(&b.rect.x))
    });
    for (n, i) in order.into_iter().enumerate() {
        items[i].num = n + 1;
    }
    let by_id: HashMap<&str, &Item> = items.iter().map(|i| (i.block.id.as_str(), i)).collect();

    // Empty sheets are left out; a project without any block keeps its sheets.
    let used: Vec<u32> = sheets
        .iter()
        .copied()
        .filter(|s| items.iter().any(|i| i.sheet == *s))
        .collect();
    let sheets: Vec<u32> = if used.is_empty() {
        sheets.into_iter().collect()
    } else {
        used
    };
    let mut out = PRELUDE.to_owned();
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
        let mut drawing = String::new();
        let mut c = Canvas { out: &mut drawing };

        let info = layout.sheet(sheet);
        let mut head = format!("Плата {sheet}");
        if let Some(name) = info.map(|i| i.name.as_str()).filter(|n| !n.is_empty()) {
            head = format!("{head}. {name}");
        }
        // Properties of the sheet go to its title.
        if let Some(info) = info {
            let mut props = Vec::new();
            if info.background {
                props.push("фоновая задача, ядро 0".to_owned());
                if info.period > 0 {
                    props.push(format!("период {} мс", info.period));
                }
            }
            if info.disabled {
                props.push("плата отключена".to_owned());
            }
            if !props.is_empty() {
                head = format!("{head} ({})", props.join(", "));
            }
        }
        c.text(16.0, 16.0, &head, 12.0, Align::Left, INK);
        c.text(w - 16.0, 16.0, title, 9.0, Align::Right, WEAK);
        if let Some(comment) = info.map(|i| i.comment.as_str()).filter(|c| !c.is_empty()) {
            c.text(16.0, 30.0, comment, 9.0, Align::Left, WEAK);
        }
        c.line(&[(0.0, SHEET_HEADER), (w, SHEET_HEADER)], 0.6, GRID_LINE);

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
            c.line(&route(a, b, GRID * 0.5), 1.0, signal_color(ty));
        }
        for item in &on_sheet {
            let fed = |port: &str| {
                logic
                    .links
                    .iter()
                    .any(|l| l.to.block == item.block.id && l.to.port == port)
            };
            draw_block(&mut c, item, origin(item.rect), fed);
        }
        // Links go over the blocks, on top of the drawing: a click opens the parameters.
        for item in &on_sheet {
            c.out.push_str(&block_link(item, origin(item.rect)));
        }
        let _ = write!(
            out,
            "#set page(width: {w:.1} * u, height: auto, margin: 0pt)
             #box(width: {w:.1} * u, height: {h:.1} * u, {{
{drawing}}})
"
        );
    }
    out.push_str(&param_page(&items));
    out
}

/// Parameters of a block with their values, passwords hidden. Empty when it has none.
fn block_params<'a>(item: &Item<'a>) -> Vec<(&'a str, String)> {
    let (Some(def), true) = (item.def, kind_of(item) == Kind::Box) else {
        return Vec::new();
    };
    def.active_params(&item.block.params)
        .filter(|p| !p.fixed && p.show != ParamShow::Hidden)
        .map(|p| {
            let value = item.block.params.get(&p.name).or(p.default.as_ref());
            let text = match value {
                // The PDF is easy to pass around: keep passwords out of it entirely.
                Some(_) if p.show == ParamShow::Secret || p.name.contains("password") => {
                    "***".to_owned()
                }
                Some(v) => p.value_label(v),
                None => String::new(),
            };
            (
                p.title(),
                if text.is_empty() {
                    "—".to_owned()
                } else {
                    text
                },
            )
        })
        .collect()
}

/// A viewer puts the target of a link at the top of the screen; the anchors lie this
/// much (canvas units) above their block so that the block ends up nearer the middle.
const ANCHOR_LEAD: f32 = 300.0;

/// Link over a block that opens its row on the parameter page, and the anchor
/// the row links back to. Internal links work in every viewer, unlike PDF notes.
fn block_link(item: &Item, r: Bounds) -> String {
    if block_params(item).is_empty() {
        return String::new();
    }
    let n = item.num;
    format!(
        "place(top + left, dx: {x:.2} * u, dy: {anchor_y:.2} * u, [#metadata(none)#label(\"b{n}\")])
         place(top + left, dx: {x:.2} * u, dy: {y:.2} * u, link(label(\"p{n}\"),          rect(width: {w:.2} * u, height: {h:.2} * u, fill: none, stroke: none)))
",
        x = r.x,
        y = r.y,
        anchor_y = (r.y - ANCHOR_LEAD).max(0.0),
        w = r.w,
        h = r.h
    )
}

/// The common list at the end of the document: every block of the project with its
/// parameters, on portrait A4 pages. Empty when no block has parameters.
fn param_page(items: &[Item]) -> String {
    let mut rows: Vec<&Item> = items
        .iter()
        .filter(|i| !block_params(i).is_empty())
        .collect();
    rows.sort_by_key(|i| i.num);
    if rows.is_empty() {
        return String::new();
    }
    let mut body = String::new();
    let mut stripe = 0;
    for (k, item) in rows.into_iter().enumerate() {
        let params = block_params(item);
        let n = params.len();
        // A double line (two thin lines with a gap) separates the blocks.
        if k > 0 {
            body.push_str(
                "table.cell(colspan: 5, inset: 0pt, fill: white,                  stroke: (y: 0.5 * u + luma(60)), box(height: 2.5 * u)),",
            );
        }
        let _ = write!(
            body,
            "table.cell(rowspan: {n})[#place(top + left, dy: -25mm, [#metadata(none)#label(\"p{num}\")])#link(label(\"b{num}\"))[#underline[{num}]]], \
             table.cell(rowspan: {n}, {id}), table.cell(rowspan: {n}, {name}),",
            num = item.num,
            id = quote(&item.block.id),
            name = quote(item.def.map_or("", |d| d.info.name.as_str())),
        );
        for (title, value) in params {
            // Parameter rows alternate in colour, like stripes of a Markdown table.
            let fill = if stripe % 2 == 1 { "luma(205)" } else { "none" };
            stripe += 1;
            let _ = write!(
                body,
                "table.cell(fill: {fill}, {}), table.cell(fill: {fill}, {}),",
                quote(title),
                quote(&value)
            );
        }
        body.push('\n');
    }
    format!(
        "#set page(paper: \"a4\", margin: 15mm)
         #text(size: 14pt, weight: \"bold\")[Параметры блоков]
         #v(2mm)
         #text(size: 9pt, fill: rgb(\"{WEAK_HEX}\"))[Номер блока — ссылка на него на схеме; нажатие на блок на схеме приводит сюда.]
         #v(4mm)
         #set text(size: 9pt)
         #table(columns: (auto, auto, auto, auto, 1fr), inset: 4pt, stroke: 0.4pt + luma(160),
         fill: (_, y) => if y == 0 {{ rgb(\"{HEADER_HEX}\") }},
         table.header([*№*], [*Блок*], [*Тип*], [*Параметр*], [*Значение*]),
         {body})
",
        WEAK_HEX = hex(WEAK),
        HEADER_HEX = hex(HEADER),
    )
}

/// Typst string literal.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Saves the project archive together with [`BOARDS_PDF`]. If the picture cannot
/// be drawn, the project is still saved without it and the reason is returned
/// as a warning.
///
/// Values of `local` parameters (Wi-Fi passwords) never go into the archive: if
/// the project still holds some, they are moved to [`LocalValues`] of this computer.
pub fn save_project(project: &Project, lib: &Library) -> flp_core::Result<Option<String>> {
    let mut clean = project.clone();
    if clean.take_local(lib, &mut LocalValues::default()) {
        clean = project.clone();
        let mut store = LocalValues::open_default()?;
        clean.take_local(lib, &mut store);
        store.save()?;
    }
    let project = &clean;
    // The user blocks the schema uses and the user board travel inside the project file.
    let mut packed = flp_core::packed::pack_user_blocks(&project.logic, lib);
    packed.extend(flp_core::packed::pack_user_board(
        project.config.board.model.as_deref(),
        lib,
    ));
    let mut extra: Vec<(&str, &[u8])> = packed
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    match render(project, lib) {
        Ok(pdf) => {
            extra.push((BOARDS_PDF, &pdf));
            project.save_with(&extra).map(|()| None)
        }
        Err(e) => project
            .save_with(&extra)
            .map(|()| Some(format!("{BOARDS_PDF} не обновлён: {e}"))),
    }
}

struct Item<'a> {
    /// Number among the boxes of the project, to tell blocks of one type apart.
    num: usize,
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

fn draw_block(c: &mut Canvas, item: &Item, r: Bounds, fed: impl Fn(&str) -> bool) {
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
            let id = format!("{} · {}", item.num, block.id);
            c.text(r.x, r.y - 6.0, &id, 9.5, Align::Left, WEAK);
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
                    let caption = p.caption(block.params.get(&p.name)?)?;
                    // The PDF is easy to pass around: keep passwords out of it entirely.
                    let secret = p.show == ParamShow::Secret || p.name.contains("password");
                    Some(match p.show {
                        ParamShow::Named if secret => format!("{}=***", p.title()),
                        _ if secret => "***".to_owned(),
                        _ => caption,
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
        SignalType::String => (0x8a, 0x3f, 0xa8),
        SignalType::Any | SignalType::Number | SignalType::Same => (0x60, 0x60, 0x60),
    }
}

fn hex((r, g, b): Rgb8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

/// Typst calls of one sheet; takes canvas coordinates (y down, units of a quarter of a millimetre).
struct Canvas<'a> {
    out: &'a mut String,
}

impl Canvas<'_> {
    fn line(&mut self, points: &[(f32, f32)], width: f32, rgb: Rgb8) {
        let _ = write!(self.out, "W(\"{}\", {width:.2}", hex(rgb));
        for (x, y) in points {
            let _ = write!(self.out, ", ({x:.2}, {y:.2})");
        }
        self.out.push_str(")\n");
    }

    fn rect(&mut self, r: Bounds, fill: Rgb8, border: Rgb8) {
        let _ = writeln!(
            self.out,
            "R({:.2}, {:.2}, {:.2}, {:.2}, \"{}\", \"{}\")",
            r.x,
            r.y,
            r.w,
            r.h,
            hex(fill),
            hex(border)
        );
    }

    fn dot(&mut self, (x, y): (f32, f32), rgb: Rgb8) {
        let _ = writeln!(self.out, "D({x:.2}, {y:.2}, {PORT_R}, \"{}\")", hex(rgb));
    }

    /// Text vertically centred on `y`.
    fn text(&mut self, x: f32, y: f32, text: &str, size: f32, align: Align, rgb: Rgb8) {
        // The box is wide enough that the text never wraps.
        let w = text.chars().count() as f32 * size + 40.0;
        let (x, al) = match align {
            Align::Left => (x, "left"),
            Align::Center => (x - w * 0.5, "center"),
            Align::Right => (x - w, "right"),
        };
        let _ = writeln!(
            self.out,
            "T({x:.2}, {y:.2}, {w:.1}, {al}, {size}, \"{}\", {})",
            hex(rgb),
            quote(text)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump() -> (Project, Library) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (lib, _) = Library::load(root.join("library")).unwrap();
        (Project::load(root.join("examples/pump")).unwrap(), lib)
    }

    #[test]
    fn lists_block_parameters_without_secrets() {
        let (project, lib) = pump();
        let src = source(&project, &lib);
        assert!(src.contains("Параметры блоков"));
        assert!(src.contains("link(label(\"p1\")"));
        assert!(!src.to_lowercase().contains("password"));
    }

    #[test]
    fn parameter_list_is_one_portrait_page_run_at_the_end() {
        let (project, lib) = pump();
        let src = source(&project, &lib);
        assert_eq!(src.matches("Параметры блоков").count(), 1);
        let list = src.find("paper: \"a4\"").expect("portrait page");
        assert!(src.rfind("height: auto").unwrap() < list);
    }

    #[test]
    fn quotes_typst_strings() {
        assert_eq!(quote("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
    }
}
