//! C++ syntax highlighting for the code fields: a small hand-written scanner (keywords,
//! types, numbers, strings, comments, `#` directives) that feeds a `TextEdit` layouter.
//! With `jinja` it also marks the template syntax of block templates: `{{ .. }}`,
//! `{% .. %}`, `{# .. #}` and the `// @section` lines.

use std::sync::Arc;

use eframe::egui::{self, text::LayoutJob, Color32, FontId, Galley, TextFormat, TextStyle, Ui};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Plain,
    Keyword,
    Type,
    Number,
    Str,
    Comment,
    Preproc,
    Jinja,
    Marker,
}

const KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "goto",
    "const",
    "static",
    "volatile",
    "extern",
    "inline",
    "constexpr",
    "struct",
    "class",
    "enum",
    "union",
    "namespace",
    "using",
    "typedef",
    "template",
    "typename",
    "public",
    "private",
    "protected",
    "virtual",
    "override",
    "new",
    "delete",
    "sizeof",
    "true",
    "false",
    "nullptr",
    "this",
    "operator",
    "auto",
    "unsigned",
    "signed",
    "try",
    "catch",
    "throw",
];

const TYPES: &[&str] = &[
    "void", "bool", "char", "short", "int", "long", "float", "double", "size_t", "uint8_t",
    "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t", "byte", "word",
    "boolean", "String",
];

/// Arduino constants, coloured like numbers.
const CONSTANTS: &[&str] = &[
    "HIGH",
    "LOW",
    "INPUT",
    "OUTPUT",
    "INPUT_PULLUP",
    "LED_BUILTIN",
];

fn colour(kind: Kind, dark: bool, plain: Color32) -> Color32 {
    let (d, l) = match kind {
        Kind::Plain => return plain,
        Kind::Keyword => ((86, 156, 214), (0, 92, 197)),
        Kind::Type => ((78, 201, 176), (0, 118, 128)),
        Kind::Number => ((181, 206, 168), (9, 120, 80)),
        Kind::Str => ((206, 145, 120), (163, 21, 21)),
        Kind::Comment => ((106, 153, 85), (90, 130, 70)),
        Kind::Preproc => ((197, 134, 192), (150, 40, 180)),
        Kind::Jinja => ((255, 205, 90), (176, 100, 0)),
        Kind::Marker => ((255, 150, 80), (200, 70, 0)),
    };
    let (r, g, b) = if dark { d } else { l };
    Color32::from_rgb(r, g, b)
}

/// Lays out `text` with highlighting, wrapping at `wrap_width`.
pub fn layout(ui: &Ui, text: &str, wrap_width: f32, jinja: bool) -> Arc<Galley> {
    let dark = ui.visuals().dark_mode;
    let plain = ui.visuals().text_color();
    let font: FontId = TextStyle::Monospace.resolve(ui.style());
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    // Keep the trailing whitespace: hiding it while typing feels wrong.
    job.keep_trailing_whitespace = true;
    for (from, to, kind) in tokens(text, jinja) {
        let format = TextFormat::simple(font.clone(), colour(kind, dark, plain));
        job.append(&text[from..to], 0.0, format);
    }
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Same as [`layout`] for a `TextEdit` layouter.
pub fn layouter(jinja: bool) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> Arc<Galley> {
    move |ui, text, wrap_width| layout(ui, text.as_str(), wrap_width, jinja)
}

/// Consecutive pieces `(from, to, kind)` that cover the whole `src`.
fn tokens(src: &str, jinja: bool) -> Vec<(usize, usize, Kind)> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<(usize, usize, Kind)> = Vec::new();
    let mut push = |from: usize, to: usize, kind: Kind| {
        if from >= to {
            return;
        }
        match out.last_mut() {
            Some(last) if last.2 == kind && last.1 == from => last.1 = to,
            _ => out.push((from, to, kind)),
        }
    };
    let next_char = |i: usize| i + src[i..].chars().next().map_or(1, char::len_utf8);
    let line_end = |i: usize| b[i..].iter().position(|&c| c == b'\n').map_or(n, |p| i + p);
    // End of the template tag opening at `i`, if one does.
    let tag_end = |i: usize| -> Option<usize> {
        let close = match b.get(i..i + 2)? {
            b"{{" => "}}",
            b"{%" => "%}",
            b"{#" => "#}",
            _ => return None,
        };
        Some(src[i + 2..].find(close).map_or(n, |p| i + 2 + p + 2))
    };

    let mut i = 0;
    let mut line_start = true;
    while i < n {
        let c = b[i];
        if c == b'\n' {
            push(i, i + 1, Kind::Plain);
            i += 1;
            line_start = true;
            continue;
        }
        if jinja {
            if let Some(end) = tag_end(i) {
                push(i, end, Kind::Jinja);
                i = end;
                line_start = false;
                continue;
            }
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            push(i, i + 1, Kind::Plain);
            i += 1;
            continue;
        }
        let at_line_start = line_start;
        line_start = false;
        if c == b'#' && at_line_start {
            let end = line_end(i);
            push(i, end, Kind::Preproc);
            i = end;
        } else if src[i..].starts_with("//") {
            let end = line_end(i);
            let marker = jinja && src[i + 2..end].trim_start().starts_with('@');
            push(i, end, if marker { Kind::Marker } else { Kind::Comment });
            i = end;
        } else if src[i..].starts_with("/*") {
            let end = src[i + 2..].find("*/").map_or(n, |p| i + 2 + p + 2);
            push(i, end, Kind::Comment);
            i = end;
        } else if c == b'"' || c == b'\'' {
            // A literal ends at its closing quote or at the end of the line; template
            // tags inside it keep their own colour.
            let mut j = i + 1;
            let mut from = i;
            while j < n && b[j] != b'\n' {
                if let Some(end) = jinja.then(|| tag_end(j)).flatten() {
                    push(from, j, Kind::Str);
                    push(j, end, Kind::Jinja);
                    j = end;
                    from = j;
                } else if b[j] == b'\\' && j + 1 < n && b[j + 1] != b'\n' {
                    j = next_char(j + 1);
                } else if b[j] == c {
                    j += 1;
                    break;
                } else {
                    j = next_char(j);
                }
            }
            push(from, j.min(n), Kind::Str);
            i = j.min(n);
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            push(i, j, Kind::Number);
            i = j;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            let word = &src[i..j];
            let kind = if KEYWORDS.contains(&word) {
                Kind::Keyword
            } else if TYPES.contains(&word) {
                Kind::Type
            } else if CONSTANTS.contains(&word) {
                Kind::Number
            } else {
                Kind::Plain
            };
            push(i, j, kind);
            i = j;
        } else {
            let end = next_char(i);
            push(i, end, Kind::Plain);
            i = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str, jinja: bool) -> Vec<(&str, Kind)> {
        tokens(src, jinja)
            .into_iter()
            .map(|(a, b, k)| (&src[a..b], k))
            .filter(|(s, _)| !s.trim().is_empty())
            .collect()
    }

    #[test]
    fn highlights_cpp() {
        let got = kinds(
            "#include <DHT.h>\nint x = 0x1F; // тест\nif (a) \"s\\\"t\";",
            false,
        );
        assert_eq!(
            got,
            vec![
                ("#include <DHT.h>", Kind::Preproc),
                ("int", Kind::Type),
                (" x = ", Kind::Plain),
                ("0x1F", Kind::Number),
                ("; ", Kind::Plain),
                ("// тест", Kind::Comment),
                ("if", Kind::Keyword),
                (" (a) ", Kind::Plain),
                ("\"s\\\"t\"", Kind::Str),
                (";", Kind::Plain),
            ]
        );
    }

    #[test]
    fn covers_all_text() {
        let src = "/* а\nб */ int ж = 1; \"незакрытая\nx";
        let total: usize = tokens(src, true).iter().map(|(a, b, _)| b - a).sum();
        assert_eq!(total, src.len());
    }

    #[test]
    fn marks_template_syntax() {
        let got = kinds(
            "// @setup\nauto s = \"{{ param.ssid }}!\"; {% if value.x %}{{ id }}{% endif %}",
            true,
        );
        assert_eq!(got[0], ("// @setup", Kind::Marker));
        assert!(got.contains(&("{{ param.ssid }}", Kind::Jinja)));
        assert!(got.contains(&("\"", Kind::Str)));
        assert!(got.contains(&("!\"", Kind::Str)));
        assert!(got.contains(&("{% if value.x %}{{ id }}{% endif %}", Kind::Jinja)));
        // Without the template mode the tags are plain C++.
        assert_eq!(kinds("{{ id }}", false), vec![("{{ id }}", Kind::Plain)]);
    }
}
