//! Built-in blocks of the "Строки" group. Texts are bytes like Arduino `String`:
//! positions and lengths count bytes.

use crate::cpp::{CppCtx, CppParts, Lines, Raw};
use crate::{b, Behavior, Ctx};

const KINDS: [&str; 12] = [
    "ip_to_text",
    "text_to_ip",
    "text_const",
    "text_concat",
    "text_length",
    "num_to_text",
    "text_compare",
    "text_to_num",
    "text_substring",
    "text_find",
    "text_char",
    "text_add_char",
];

/// Behaviour of a string block, or `None` for another type.
pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    KINDS
        .contains(&kind)
        .then(|| Box::new(TextBlock(kind.to_owned())) as Box<dyn Behavior>)
}

/// A string block; it has no state, so one type serves all of them.
pub struct TextBlock(pub String);

impl Behavior for TextBlock {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}

    fn step_text(&mut self, c: &Ctx, out: &mut [f64], text: &mut [String]) {
        match self.0.as_str() {
            "text_const" => text[0] = c.param_text(0).to_owned(),
            "text_concat" => {
                let count = (c.param(0).max(1.0) as usize).min(16);
                let mut s = c.text(0).to_owned();
                for i in 1..count {
                    s.push_str(c.param_text(1));
                    s.push_str(c.text(i));
                }
                text[0] = s;
            }
            "text_length" => out[0] = c.text(0).len() as f64,
            "num_to_text" => {
                let decimals = c.param(0).max(0.0) as usize;
                text[0] = format!(
                    "{}{:.decimals$}{}",
                    c.param_text(1),
                    c.input(0),
                    c.param_text(2)
                );
            }
            "text_compare" => {
                let (mut a, mut p) = (c.text(0).to_owned(), c.param_text(0).to_owned());
                if c.param(2) != 0.0 {
                    a.make_ascii_lowercase();
                    p.make_ascii_lowercase();
                }
                out[0] = b(match c.param(1) as i32 {
                    1 => a.contains(&p),
                    2 => a.starts_with(&p),
                    3 => a.ends_with(&p),
                    _ => a == p,
                });
            }
            "text_to_num" => {
                // Spaces around are skipped, a decimal comma is accepted.
                let t = c.text(0).trim().replace(',', ".");
                match t.parse::<f64>() {
                    Ok(v) if !t.is_empty() && v.is_finite() => {
                        out[0] = f64::from(v as f32);
                        out[1] = 1.0;
                    }
                    _ => out[1] = 0.0,
                }
            }
            "text_substring" => {
                let bytes = c.text(0).as_bytes();
                let start = (c.param(0).max(0.0) as usize).min(bytes.len());
                let end = if c.param(1) < 0.0 {
                    bytes.len()
                } else {
                    (start + c.param(1) as usize).min(bytes.len())
                };
                text[0] = String::from_utf8_lossy(&bytes[start..end]).into_owned();
            }
            "text_find" => {
                let (hay, sub) = (c.text(0).as_bytes(), c.text(1).as_bytes());
                let from = c.param(0).max(0.0) as usize;
                let pos = if from > hay.len() {
                    None
                } else if sub.is_empty() {
                    Some(from)
                } else {
                    hay[from..]
                        .windows(sub.len())
                        .position(|w| w == sub)
                        .map(|p| p + from)
                };
                out[0] = pos.map_or(-1.0, |p| p as f64);
                out[1] = b(pos.is_some());
            }
            "text_char" => {
                let bytes = c.text(0).as_bytes();
                match usize::try_from(c.input(1) as i64)
                    .ok()
                    .and_then(|i| bytes.get(i))
                {
                    Some(&byte) => {
                        out[1] = f64::from(byte);
                        text[0] = String::from_utf8_lossy(&[byte]).into_owned();
                    }
                    None => {
                        out[1] = 0.0;
                        text[0].clear();
                    }
                }
            }
            "text_add_char" => {
                let mut s = c.text(0).to_owned();
                s.push(char::from((c.input(1) as i64 & 0xFF) as u8));
                text[0] = s;
            }
            "ip_to_text" => {
                let byte = |i| (c.input(i) as i64 & 0xFF) as u8;
                text[0] = format!("{}.{}.{}.{}", byte(0), byte(1), byte(2), byte(3));
            }
            "text_to_ip" => match parse_ip(c.text(0)) {
                Some(parts) => {
                    out[..4].copy_from_slice(&parts.map(f64::from));
                    out[4] = 1.0;
                }
                None => out[4] = 0.0,
            },
            _ => {}
        }
    }
}

/// "a.b.c.d" with every part 0..=255 and digits only.
fn parse_ip(text: &str) -> Option<[u8; 4]> {
    let mut parts = [0u8; 4];
    let mut it = text.split('.');
    for part in &mut parts {
        let s = it.next()?;
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *part = s.parse().ok()?;
    }
    it.next().is_none().then_some(parts)
}

fn looped(code: String) -> CppParts {
    CppParts {
        looped: code,
        ..Default::default()
    }
}

/// Fills `@NAME@` marks of a code fragment; the marks keep braces readable.
fn fill(code: &str, marks: &[(&str, &str)]) -> String {
    marks.iter().fold(code.to_owned(), |code, (name, value)| {
        code.replace(&format!("@{name}@"), value)
    })
}

pub(crate) fn text_const(e: &CppCtx) -> CppParts {
    CppParts {
        setup: format!("{} = {};", e.output("out"), e.param("value")),
        ..Default::default()
    }
}

pub(crate) fn text_concat(e: &CppCtx) -> CppParts {
    const PORTS: [&str; 15] = [
        "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
    ];
    let count = e.int("count", 2).clamp(1, 16) as usize;
    let out = e.output("out");
    let mut l = Lines::default();
    l.line(format!("{out} = {};", e.input("a")));
    for port in &PORTS[..count - 1] {
        l.line(format!("{out} += {};", e.param("separator")))
            .line(format!("{out} += {};", e.input(port)));
    }
    looped(l.finish())
}

pub(crate) fn text_length(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = (int32_t){}.length();",
        e.output("out"),
        e.input("in")
    ))
}

pub(crate) fn num_to_text(e: &CppCtx) -> CppParts {
    let out = e.output("out");
    let mut l = Lines::default();
    l.line(format!("{out} = {};", e.param("prefix")))
        .line(format!(
            "{out} += String({}, {});",
            e.input("in"),
            e.param("decimals")
        ))
        .line(format!("{out} += {};", e.param("suffix")));
    looped(l.finish())
}

pub(crate) fn text_compare(e: &CppCtx) -> CppParts {
    let (out, a, b) = (e.output("out"), e.input("a"), e.param("b"));
    let mode = e.int("mode", 0);
    let mut l = Lines::default();
    if matches!(e.value.get("ignore_case"), Some(Raw::Bool(true))) {
        l.line(format!("{out} = flp_text_match_nocase({a}, {b}, {mode});"));
    } else {
        l.line(match mode {
            1 => format!("{out} = {a}.indexOf({b}) >= 0;"),
            2 => format!("{out} = {a}.startsWith({b});"),
            3 => format!("{out} = {a}.endsWith({b});"),
            _ => format!("{out} = {a} == {b};"),
        });
    }
    let mut parts = looped(l.finish());
    if matches!(e.value.get("ignore_case"), Some(Raw::Bool(true))) {
        parts.once_functions = TEXT_MATCH_NOCASE.to_owned();
    }
    parts
}

/// Case-insensitive (ASCII) comparison without temporary strings: mode 0 equal, 1 contains,
/// 2 starts with, 3 ends with.
const TEXT_MATCH_NOCASE: &str = r#"static inline char flp_lower(char c) {
  return (c >= 'A' && c <= 'Z') ? (char)(c + 32) : c;
}
bool flp_text_match_nocase(const String& a, const String& b, int mode) {
  int la = a.length(), lb = b.length();
  if (lb > la || (mode == 0 && lb != la)) {
    return false;
  }
  int first = mode == 3 ? la - lb : 0;
  int last = (mode == 1 ? la : first + lb) - lb;
  for (int at = first; at <= last; at++) {
    int i = 0;
    while (i < lb && flp_lower(a[at + i]) == flp_lower(b[i])) {
      i++;
    }
    if (i == lb) {
      return true;
    }
  }
  return false;
}"#;

const TEXT_TO_FLOAT: &str = r#"// Number from text: spaces around are skipped, a decimal comma is accepted.
bool flp_text_to_float(const String& text, float& out) {
  String t = text;
  t.trim();
  t.replace(',', '.');
  if (t.length() == 0) {
    return false;
  }
  char* end = nullptr;
  double v = strtod(t.c_str(), &end);
  if (end == t.c_str() || *end != '\0') {
    return false;
  }
  out = (float)v;
  return true;
}"#;

pub(crate) fn text_to_num(e: &CppCtx) -> CppParts {
    let (out, ok) = (e.output("out"), e.output("ok"));
    let mut l = Lines::default();
    l.line("{")
        .line("  float v;")
        .line(format!("  {ok} = flp_text_to_float({}, v);", e.input("in")))
        .line(format!("  if ({ok}) {{"))
        .line(format!("    {out} = v;"))
        .line("  }")
        .line("}");
    CppParts {
        once_functions: TEXT_TO_FLOAT.to_owned(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn text_substring(e: &CppCtx) -> CppParts {
    let (out, input, start) = (e.output("out"), e.input("in"), e.param("start"));
    looped(if e.int("len", -1) < 0 {
        format!("{out} = {input}.substring({start});")
    } else {
        format!(
            "{out} = {input}.substring({start}, {start} + {});",
            e.param("len")
        )
    })
}

pub(crate) fn text_find(e: &CppCtx) -> CppParts {
    let (pos, found) = (e.output("pos"), e.output("found"));
    let mut l = Lines::default();
    l.line(format!(
        "{pos} = {}.indexOf({}, {});",
        e.input("text"),
        e.input("sub"),
        e.param("from")
    ))
    .line(format!("{found} = {pos} >= 0;"));
    looped(l.finish())
}

pub(crate) fn text_char(e: &CppCtx) -> CppParts {
    let (text, index) = (e.input("text"), e.input("index"));
    let (code, ch) = (e.output("code"), e.output("char"));
    let mut l = Lines::default();
    l.line(format!(
        "if ({index} >= 0 && {index} < (int){text}.length()) {{"
    ))
    .line(format!("  {code} = (uint8_t){text}[{index}];"))
    .line(format!("  {ch} = {text}.substring({index}, {index} + 1);"))
    .line("} else {")
    .line(format!("  {code} = 0;"))
    .line(format!("  {ch} = \"\";"))
    .line("}");
    looped(l.finish())
}

pub(crate) fn text_add_char(e: &CppCtx) -> CppParts {
    let out = e.output("out");
    let mut l = Lines::default();
    l.line(format!("{out} = {};", e.input("text")))
        .line(format!("{out} += (char)({} & 0xFF);", e.input("code")));
    looped(l.finish())
}

pub(crate) fn ip_to_text(e: &CppCtx) -> CppParts {
    let byte = |port| format!("String({} & 0xFF)", e.input(port));
    looped(format!(
        "{} = {} + \".\" + {} + \".\" + {} + \".\" + {};",
        e.output("out"),
        byte("a"),
        byte("b"),
        byte("c"),
        byte("d")
    ))
}

const TEXT_TO_IP: &str = r#"{
  int v[4] = {0, 0, 0, 0};
  int part = 0;
  bool digits = false;
  bool good = true;
  for (unsigned int i = 0; i < @IN@.length() && good; i++) {
    char ch = @IN@[i];
    if (ch >= '0' && ch <= '9') {
      v[part] = v[part] * 10 + (ch - '0');
      digits = true;
      if (v[part] > 255) good = false;
    } else if (ch == '.' && digits && part < 3) {
      part++;
      digits = false;
    } else {
      good = false;
    }
  }
  good = good && digits && part == 3;
  if (good) {
    @A@ = v[0];
    @B@ = v[1];
    @C@ = v[2];
    @D@ = v[3];
  }
  @OK@ = good;
}"#;

pub(crate) fn text_to_ip(e: &CppCtx) -> CppParts {
    looped(fill(
        TEXT_TO_IP,
        &[
            ("IN", e.input("in")),
            ("A", e.output("a")),
            ("B", e.output("b")),
            ("C", e.output("c")),
            ("D", e.output("d")),
            ("OK", e.output("ok")),
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        kind: &str,
        inputs: &[f64],
        texts: &[&str],
        params: &[f64],
        ptexts: &[&str],
    ) -> (Vec<f64>, Vec<String>) {
        let texts: Vec<String> = texts.iter().map(|s| (*s).to_owned()).collect();
        let ptexts: Vec<String> = ptexts.iter().map(|s| (*s).to_owned()).collect();
        let active = vec![true; inputs.len().max(texts.len())];
        let ctx = Ctx {
            now_ms: 0,
            inputs,
            active: &active,
            params,
            texts: &texts,
            param_texts: &ptexts,
            bench: &crate::NO_BENCH,
        };
        let mut out = vec![0.0; 5];
        let mut text = vec![String::new(); 1];
        TextBlock(kind.to_owned()).step_text(&ctx, &mut out, &mut text);
        (out, text)
    }

    #[test]
    fn concat_joins_with_separator() {
        let (_, t) = run(
            "text_concat",
            &[0.0; 3],
            &["a", "b", "c"],
            &[3.0, 0.0],
            &["", "-"],
        );
        assert_eq!(t[0], "a-b-c");
    }

    #[test]
    fn number_to_text_has_prefix_and_suffix() {
        let (_, t) = run(
            "num_to_text",
            &[21.456],
            &["", "", ""],
            &[1.0, 0.0, 0.0],
            &["", "T=", " C"],
        );
        assert_eq!(t[0], "T=21.5 C");
    }

    #[test]
    fn compare_modes() {
        let c = |a: &str, mode: f64, ic: f64| {
            run("text_compare", &[0.0], &[a], &[0.0, mode, ic], &["on"]).0[0]
        };
        assert_eq!(c("ON", 0.0, 1.0), 1.0);
        assert_eq!(c("ON", 0.0, 0.0), 0.0);
        assert_eq!(c("xonx", 1.0, 0.0), 1.0);
        assert_eq!(c("onx", 2.0, 0.0), 1.0);
        assert_eq!(c("xon", 3.0, 0.0), 1.0);
    }

    #[test]
    fn find_substring_and_char() {
        let (o, _) = run("text_find", &[0.0, 0.0], &["hello", "ll"], &[0.0], &[]);
        assert_eq!((o[0], o[1]), (2.0, 1.0));
        let (o, _) = run("text_find", &[0.0, 0.0], &["hello", "z"], &[0.0], &[]);
        assert_eq!((o[0], o[1]), (-1.0, 0.0));
        let (o, t) = run("text_char", &[0.0, 1.0], &["hello", ""], &[], &[]);
        assert_eq!((o[1], t[0].as_str()), (101.0, "e"));
    }

    #[test]
    fn text_to_number_keeps_last_value_on_error() {
        let (o, _) = run("text_to_num", &[0.0], &[" 3,5 "], &[], &[]);
        assert_eq!((o[0], o[1]), (3.5, 1.0));
        let (o, _) = run("text_to_num", &[0.0], &["abc"], &[], &[]);
        assert_eq!((o[0], o[1]), (0.0, 0.0));
    }

    #[test]
    fn ip_blocks_round_trip() {
        let (_, t) = run("ip_to_text", &[192.0, 168.0, 1.0, 300.0], &[], &[], &[]);
        assert_eq!(t[0], "192.168.1.44");
        let (o, _) = run("text_to_ip", &[0.0], &["10.0.0.7"], &[], &[]);
        assert_eq!(o, [10.0, 0.0, 0.0, 7.0, 1.0]);
        let (o, _) = run("text_to_ip", &[0.0], &["10.0.0"], &[], &[]);
        assert_eq!(o[4], 0.0);
        let (o, _) = run("text_to_ip", &[0.0], &["1.2.3.256"], &[], &[]);
        assert_eq!(o[4], 0.0);
    }

    #[test]
    fn substring_clamps() {
        let (_, t) = run("text_substring", &[0.0], &["hello"], &[1.0, 3.0], &[]);
        assert_eq!(t[0], "ell");
        let (_, t) = run("text_substring", &[0.0], &["hello"], &[9.0, -1.0], &[]);
        assert_eq!(t[0], "");
    }
}
