//! Small Markdown renderer for block docs (`docs.md`).
//!
//! Covers what the docs use: headings, paragraphs, bullet and numbered lists,
//! quotes, fenced code, tables, and inline `code`, **bold**, *italic* and
//! [links](url). Anything else is shown as plain text.

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
    Gap,
}

#[derive(Debug, PartialEq)]
enum Span<'a> {
    Text(&'a str, Emphasis),
    Code(&'a str),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Emphasis {
    bold: bool,
    italic: bool,
    link: bool,
}

/// Draws `text` as formatted Markdown, wrapping at the available width.
pub fn show(ui: &mut Ui, text: &str) {
    for block in parse(text) {
        match block {
            Block::Heading(level, text) => {
                ui.add_space(if level == 1 { 2.0 } else { 6.0 });
                let size = match level {
                    1 => 20.0,
                    2 => 17.0,
                    _ => 15.0,
                };
                let color = ui.visuals().strong_text_color();
                rich(ui, &text, FontId::proportional(size), color);
                ui.add_space(2.0);
            }
            Block::Paragraph(text) => body(ui, &text),
            Block::Item(marker, text) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0);
                    ui.label(marker);
                    ui.vertical(|ui| body(ui, &text));
                });
            }
            Block::Quote(text) => {
                let bar = ui.visuals().widgets.noninteractive.bg_stroke.color;
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 8,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        let rect = ui.max_rect();
                        body(ui, &text);
                        let x = rect.left() - 5.0;
                        ui.painter().vline(
                            x,
                            rect.top()..=ui.min_rect().bottom(),
                            Stroke::new(3.0, bar),
                        );
                    });
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
            Block::Table(rows) => table(ui, &rows),
            Block::Gap => ui.add_space(6.0),
        }
    }
}

fn body(ui: &mut Ui, text: &str) {
    let font = TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    rich(ui, text, font, color);
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
                    color: if e.link {
                        visuals.hyperlink_color
                    } else if e.bold {
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
        }
    }
    job
}

fn table(ui: &mut Ui, rows: &[Vec<String>]) {
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
                    for (i, row) in rows.iter().enumerate() {
                        for cell in row {
                            let font = TextStyle::Body.resolve(ui.style());
                            let color = if i == 0 {
                                ui.visuals().strong_text_color()
                            } else {
                                ui.visuals().text_color()
                            };
                            let job = layout(ui, cell, font, color);
                            ui.label(job);
                        }
                        ui.end_row();
                    }
                });
        });
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
                // [text](url) shows the text only.
                if let Some(close) = text[i..].find("](") {
                    if let Some(end) = text[i + close..].find(')') {
                        flush(&mut spans, start, i, emphasis);
                        let label = &text[i + 1..i + close];
                        spans.push(Span::Text(
                            label,
                            Emphasis {
                                link: true,
                                ..emphasis
                            },
                        ));
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
        let link = Emphasis {
            link: true,
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
                Span::Text("c", link),
                Span::Text(" ", plain),
            ]
        );
    }

    #[test]
    fn hash_without_space_is_text() {
        assert_eq!(
            parse("#define X"),
            vec![Block::Paragraph("#define X".into())]
        );
    }
}
