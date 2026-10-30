//! Small Markdown renderer for block docs (`docs.md`).
//!
//! Covers what the docs use: headings, paragraphs, bullet and numbered lists,
//! quotes, fenced code, tables, and inline `code`, **bold**, *italic* and
//! [links](url), and images on a line of their own (`![alt](images/a.png)`).
//! Anything else is shown as plain text.

use eframe::egui::{self, text::LayoutJob, Color32, FontId, Stroke, TextFormat, TextStyle, Ui};

#[derive(Debug, PartialEq)]
enum Block<'a> {
    Heading(usize, String),
    Paragraph(String),
    /// Marker ("•" or "1.") and item text.
    Item(String, String),
    Quote(String),
    Code(Vec<&'a str>),
    Table(Vec<Vec<String>>),
    /// Alt text and target of a line `![alt](target)`.
    Image(String, String),
    Gap,
}

/// Finds the file behind an image target; `None` hides the image.
pub type ImageResolver<'a> = &'a dyn Fn(&str) -> Option<std::path::PathBuf>;

/// Whether `target` is a plain relative path (no drive, no `..`) that a
/// block's docs may refer to.
pub fn is_local_image(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with(['/', '\\'])
        && !target.contains(':')
        && !target.split(['/', '\\']).any(|part| part == "..")
}

/// Resolver for the pictures in the folder of a block.
pub fn dir_images(dir: &std::path::Path) -> impl Fn(&str) -> Option<std::path::PathBuf> + '_ {
    move |target| {
        let local = is_local_image(target) && target.starts_with("images/");
        local.then(|| dir.join(target))
    }
}

/// `file://` URI of a local file, in the form the egui file loader reads on every system:
/// a Windows path `D:/a.png` needs three slashes (`file:///D:/a.png`), otherwise the
/// loader takes the drive for a network host.
fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// Image targets that `text` refers to.
pub fn image_targets(text: &str) -> Vec<String> {
    parse(text)
        .into_iter()
        .filter_map(|block| match block {
            Block::Image(_, target) => Some(target),
            _ => None,
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum Span<'a> {
    Text(&'a str, Emphasis),
    Code(&'a str),
    /// Label and target of `[label](target)`.
    Link(&'a str, &'a str),
}

/// Anchor of a heading: lower case, words joined by `-`, punctuation dropped.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if (c.is_whitespace() || c == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// Draws `text` as Markdown. A click on a `help:` link returns its target (`page#anchor`);
/// other links open in the browser. `images` finds the files of `![..](..)` lines.
pub fn show_with_images(
    ui: &mut Ui,
    text: &str,
    anchor: Option<&str>,
    images: Option<ImageResolver>,
) -> Option<String> {
    let url = show_inner(ui, text, anchor, images)?;
    match url.strip_prefix("help:") {
        Some(target) => Some(target.to_owned()),
        None => {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            None
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Emphasis {
    bold: bool,
    italic: bool,
}

fn show_inner(
    ui: &mut Ui,
    text: &str,
    anchor: Option<&str>,
    images: Option<ImageResolver>,
) -> Option<String> {
    let mut clicked = None;
    for block in parse(text) {
        match block {
            Block::Heading(level, text) => {
                ui.add_space(if level == 1 { 2.0 } else { 6.0 });
                if anchor.is_some_and(|a| a == slug(&text)) {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                }
                let size = match level {
                    1 => 20.0,
                    2 => 17.0,
                    _ => 15.0,
                };
                let color = ui.visuals().strong_text_color();
                rich(ui, &text, FontId::proportional(size), color);
                ui.add_space(2.0);
            }
            Block::Paragraph(text) => clicked = body(ui, &text).or(clicked),
            Block::Item(marker, text) => {
                let url = ui
                    .horizontal_top(|ui| {
                        ui.add_space(8.0);
                        ui.label(marker);
                        ui.vertical(|ui| body(ui, &text)).inner
                    })
                    .inner;
                clicked = url.or(clicked);
            }
            Block::Quote(text) => {
                let bar = ui.visuals().widgets.noninteractive.bg_stroke.color;
                let quote = egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 8,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        let rect = ui.max_rect();
                        let url = body(ui, &text);
                        let x = rect.left() - 5.0;
                        ui.painter().vline(
                            x,
                            rect.top()..=ui.min_rect().bottom(),
                            Stroke::new(3.0, bar),
                        );
                        url
                    });
                clicked = quote.inner.or(clicked);
            }
            Block::Code(lines) => {
                egui::Frame::new()
                    .fill(ui.visuals().code_bg_color)
                    .corner_radius(3.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let text = lines.join("\n");
                        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap());
                    });
            }
            Block::Table(rows) => clicked = table(ui, &rows).or(clicked),
            Block::Image(alt, target) => {
                match images
                    .and_then(|find| find(&target))
                    .filter(|p| p.is_file())
                {
                    Some(path) => {
                        let uri = file_uri(&path);
                        ui.add(
                            egui::Image::new(uri)
                                .fit_to_original_size(1.0)
                                .max_width(ui.available_width())
                                .alt_text(alt),
                        );
                    }
                    None => {
                        ui.weak(crate::i18n::fmt(
                            "[изображение не найдено: {0}]",
                            &[&target],
                        ));
                    }
                }
            }
            Block::Gap => ui.add_space(6.0),
        }
    }
    clicked
}

/// Draws a paragraph; returns the target of a link clicked in it.
fn body(ui: &mut Ui, text: &str) -> Option<String> {
    let font = TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    let spans = inline(text);
    if !spans.iter().any(|s| matches!(s, Span::Link(..))) {
        rich(ui, text, font, color);
        return None;
    }
    // Links need their own widgets to be clickable, so the paragraph is a row of them.
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in spans {
            match span {
                Span::Text(s, e) => {
                    let mut t = egui::RichText::new(s).font(font.clone());
                    if e.bold {
                        t = t.color(ui.visuals().strong_text_color());
                    }
                    if e.italic {
                        t = t.italics();
                    }
                    ui.label(t);
                }
                Span::Code(s) => {
                    ui.label(
                        egui::RichText::new(s)
                            .monospace()
                            .background_color(ui.visuals().code_bg_color),
                    );
                }
                Span::Link(label, url) => {
                    if ui.link(label).clicked() {
                        clicked = Some(url.to_owned());
                    }
                }
            }
        }
    });
    clicked
}

fn rich(ui: &mut Ui, text: &str, font: FontId, color: Color32) {
    let job = layout(ui, text, font, color);
    ui.add(egui::Label::new(job).wrap());
}

fn layout(ui: &Ui, text: &str, font: FontId, color: Color32) -> LayoutJob {
    let visuals = ui.visuals();
    let mono = FontId::monospace(font.size * 0.9);
    let mut job = LayoutJob::default();
    for span in inline(text) {
        match span {
            Span::Text(s, e) => {
                // The default fonts have no bold face: bold is the strong text colour.
                let format = TextFormat {
                    font_id: font.clone(),
                    color: if e.bold {
                        visuals.strong_text_color()
                    } else {
                        color
                    },
                    italics: e.italic,
                    ..Default::default()
                };
                job.append(s, 0.0, format);
            }
            Span::Code(s) => job.append(
                s,
                0.0,
                TextFormat {
                    font_id: mono.clone(),
                    color,
                    background: visuals.code_bg_color,
                    ..Default::default()
                },
            ),
            // Not clickable here (headings): only the label.
            Span::Link(label, _) => job.append(
                label,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: visuals.hyperlink_color,
                    ..Default::default()
                },
            ),
        }
    }
    job
}

/// Draws a table; returns the target of a link clicked in a cell.
fn table(ui: &mut Ui, rows: &[Vec<String>]) -> Option<String> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let id = ui.next_auto_id();
    egui::ScrollArea::horizontal()
        .id_salt(id)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new(id)
                .num_columns(columns)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    let mut clicked = None;
                    for (i, row) in rows.iter().enumerate() {
                        for cell in row {
                            let font = TextStyle::Body.resolve(ui.style());
                            let color = if i == 0 {
                                ui.visuals().strong_text_color()
                            } else {
                                ui.visuals().text_color()
                            };
                            if inline(cell).iter().any(|s| matches!(s, Span::Link(..))) {
                                clicked = body(ui, cell).or(clicked);
                            } else {
                                ui.label(layout(ui, cell, font, color));
                            }
                        }
                        ui.end_row();
                    }
                    clicked
                })
                .inner
        })
        .inner
}

fn parse(text: &str) -> Vec<Block<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !matches!(blocks.last(), None | Some(Block::Gap)) {
                blocks.push(Block::Gap);
            }
            i += 1;
        } else if trimmed.starts_with("```") {
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code(code));
            i += 1;
        } else if let Some((level, title)) = heading(trimmed) {
            blocks.push(Block::Heading(level, title.to_owned()));
            i += 1;
        } else if let Some((alt, target)) = image_line(trimmed) {
            blocks.push(Block::Image(alt.to_owned(), target.to_owned()));
            i += 1;
        } else if trimmed.starts_with('|') {
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let cells = cells(lines[i]);
                if !cells.iter().all(|c| is_rule(c)) {
                    rows.push(cells);
                }
                i += 1;
            }
            blocks.push(Block::Table(rows));
        } else if let Some(quote) = trimmed.strip_prefix('>') {
            let mut text = quote.trim().to_owned();
            i += 1;
            while i < lines.len() {
                let Some(more) = lines[i].trim().strip_prefix('>') else {
                    break;
                };
                join(&mut text, more);
                i += 1;
            }
            blocks.push(Block::Quote(text));
        } else if let Some((marker, item)) = list_item(trimmed) {
            let mut text = item.to_owned();
            i += 1;
            // Indented lines continue the item.
            while i < lines.len() && lines[i].starts_with("  ") && !starts_block(lines[i]) {
                join(&mut text, lines[i]);
                i += 1;
            }
            blocks.push(Block::Item(marker, text));
        } else {
            let mut text = trimmed.to_owned();
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(lines[i]) {
                join(&mut text, lines[i]);
                i += 1;
            }
            blocks.push(Block::Paragraph(text));
        }
    }
    if blocks.last() == Some(&Block::Gap) {
        blocks.pop();
    }
    blocks
}

fn join(text: &mut String, line: &str) {
    text.push(' ');
    text.push_str(line.trim());
}

fn starts_block(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("```")
        || t.starts_with('|')
        || t.starts_with('>')
        || heading(t).is_some()
        || list_item(t).is_some()
        || image_line(t).is_some()
}

/// `![alt](target)` taking the whole line.
fn image_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let target = rest.strip_suffix(')')?;
    (!target.is_empty() && !target.contains([')', ' '])).then_some((alt, target))
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    let rest = line[level..].strip_prefix(' ')?;
    (1..=6).contains(&level).then(|| (level, rest.trim()))
}

fn list_item(line: &str) -> Option<(String, &str)> {
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return Some(("•".to_owned(), rest.trim()));
        }
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let rest = line[digits..].strip_prefix(". ")?;
    (digits > 0).then(|| (format!("{}.", &line[..digits]), rest.trim()))
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_rule(cell: &str) -> bool {
    let c = cell.trim_matches(':');
    !c.is_empty() && c.bytes().all(|b| b == b'-')
}

fn inline<'a>(text: &'a str) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut emphasis = Emphasis::default();
    let mut start = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    let flush = |spans: &mut Vec<Span<'a>>, from: usize, to: usize, e: Emphasis| {
        if to > from {
            spans.push(Span::Text(&text[from..to], e));
        }
    };
    while i < bytes.len() {
        match bytes[i] {
            b'`' => {
                if let Some(len) = text[i + 1..].find('`') {
                    flush(&mut spans, start, i, emphasis);
                    spans.push(Span::Code(&text[i + 1..i + 1 + len]));
                    i += len + 2;
                    start = i;
                    continue;
                }
            }
            b'*' if bytes.get(i + 1) == Some(&b'*') => {
                flush(&mut spans, start, i, emphasis);
                emphasis.bold = !emphasis.bold;
                i += 2;
                start = i;
                continue;
            }
            b'*' => {
                flush(&mut spans, start, i, emphasis);
                emphasis.italic = !emphasis.italic;
                i += 1;
                start = i;
                continue;
            }
            b'[' => {
                if let Some(close) = text[i..].find("](") {
                    if let Some(end) = text[i + close..].find(')') {
                        flush(&mut spans, start, i, emphasis);
                        let label = &text[i + 1..i + close];
                        let url = &text[i + close + 2..i + close + end];
                        spans.push(Span::Link(label, url));
                        i += close + end + 1;
                        start = i;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    flush(&mut spans, start, bytes.len(), emphasis);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks() {
        let md = "# Заголовок\n\nПервая строка\nвторая.\n\n- раз\n  дальше\n2. два\n\n\
                  ```cpp\nint x;\n```\n| a | b |\n|---|:-:|\n| 1 | 2 |\n> цитата\n";
        assert_eq!(
            parse(md),
            vec![
                Block::Heading(1, "Заголовок".into()),
                Block::Gap,
                Block::Paragraph("Первая строка вторая.".into()),
                Block::Gap,
                Block::Item("•".into(), "раз дальше".into()),
                Block::Item("2.".into(), "два".into()),
                Block::Gap,
                Block::Code(vec!["int x;"]),
                Block::Table(vec![
                    vec!["a".into(), "b".into()],
                    vec!["1".into(), "2".into()]
                ]),
                Block::Quote("цитата".into()),
            ]
        );
    }

    #[test]
    fn parses_inline() {
        let plain = Emphasis::default();
        let bold = Emphasis {
            bold: true,
            ..plain
        };
        assert_eq!(
            inline("a `x_y` **b** [c](http://d) *"),
            vec![
                Span::Text("a ", plain),
                Span::Code("x_y"),
                Span::Text(" ", plain),
                Span::Text("b", bold),
                Span::Text(" ", plain),
                Span::Link("c", "http://d"),
                Span::Text(" ", plain),
            ]
        );
    }

    #[test]
    fn file_uris_have_an_empty_host() {
        assert_eq!(
            file_uri(std::path::Path::new(r"D:\FLP\images\a b.png")),
            "file:///D:/FLP/images/a b.png"
        );
        assert_eq!(
            file_uri(std::path::Path::new("/home/u/a.png")),
            "file:///home/u/a.png"
        );
    }

    #[test]
    fn parses_image_lines() {
        let md = "Текст
![схема](images/a.png)
ещё";
        assert_eq!(
            parse(md),
            vec![
                Block::Paragraph("Текст".into()),
                Block::Image("схема".into(), "images/a.png".into()),
                Block::Paragraph("ещё".into()),
            ]
        );
        assert_eq!(image_targets(md), vec!["images/a.png".to_owned()]);
        assert!(is_local_image("images/a.png"));
        assert!(!is_local_image("../a.png"));
        assert!(!is_local_image("C:/a.png"));
        assert!(!is_local_image("/etc/a.png"));
    }

    #[test]
    fn slug_of_heading() {
        assert_eq!(slug("Что такое «Цикл»?"), "что-такое-цикл");
        assert_eq!(slug("  Два  слова - тут "), "два-слова-тут");
    }

    #[test]
    fn hash_without_space_is_text() {
        assert_eq!(
            parse("#define X"),
            vec![Block::Paragraph("#define X".into())]
        );
    }
}
