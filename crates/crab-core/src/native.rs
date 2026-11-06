//! The native part of a block: `native.cpp`, C++ built into the master firmware
//! (`specs/drivers`). The file describes itself with annotation comments and two entry
//! points, so there is no separate description file:
//!
//! ```cpp
//! // crab:native libs="Adafruit BMP280" i2c=0x76
//! // crab:sim temperature=21.5
//! CRAB_INIT(State& s, Params p)        { ... }
//! CRAB_STEP(State& s, In in, Out& out) { ... }
//! ```

use std::collections::BTreeMap;
use std::fmt;

use crate::library::{BlockDef, ParamKind, SignalType};
use crate::ParamValue;

/// File name of the native part inside a block folder.
pub const NATIVE_FILE: &str = "native.cpp";

/// What `native.cpp` says about itself.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NativePart {
    /// Arduino libraries to install (`libs="A, B@1.2"`).
    pub libs: Vec<String>,
    /// Other `crab:native` keys: the resources of the block (`i2c`, `spi`, `pin`, ...).
    pub resources: BTreeMap<String, String>,
    /// Output values for the simulation (`// crab:sim name=value`).
    pub sim: BTreeMap<String, ParamValue>,
    /// The file has `CRAB_INIT`.
    pub has_init: bool,
    /// The file defines its own `struct State` (otherwise the master gives an empty one).
    pub has_state: bool,
}

/// A problem found in `native.cpp`; `line` counts from 1, 0 is the file as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "строка {}: {}", self.line, self.message)
        }
    }
}

/// Joins errors into one message, one per line.
pub fn join_errors(errors: &[NativeError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn err(line: usize, message: impl Into<String>) -> NativeError {
    NativeError {
        line,
        message: message.into(),
    }
}

/// Parses `native.cpp` and checks it against the interface of the block `def`.
pub fn parse_native(source: &str, def: &BlockDef) -> Result<NativePart, Vec<NativeError>> {
    let mut part = NativePart::default();
    let mut errors = Vec::new();
    let mut seen_native = false;
    let mut sim_lines: Vec<(usize, String, String)> = Vec::new();
    let mut step_line = 0usize;
    let mut init_line = 0usize;
    let mut in_block_comment = false;

    for (i, raw) in source.lines().enumerate() {
        let n = i + 1;
        let trimmed = raw.trim_start();
        if !in_block_comment {
            if let Some(rest) = trimmed.strip_prefix("//") {
                if let Some(rest) = rest.trim_start().strip_prefix("crab:") {
                    annotation(
                        n,
                        rest,
                        &mut part,
                        &mut seen_native,
                        &mut sim_lines,
                        &mut errors,
                    );
                }
                continue;
            }
        }
        let code = strip_comments(raw, &mut in_block_comment);
        if defines_state(&code) {
            part.has_state = true;
        }
        if has_call(&code, "CRAB_STEP") {
            if step_line != 0 {
                errors.push(err(n, "CRAB_STEP объявлена второй раз"));
            }
            step_line = n;
        }
        if has_call(&code, "CRAB_INIT") {
            if init_line != 0 {
                errors.push(err(n, "CRAB_INIT объявлена второй раз"));
            }
            init_line = n;
        }
    }
    if step_line == 0 {
        errors.push(err(
            0,
            "нет функции CRAB_STEP(State& s, In in, Out& out): без неё блок ничего не считает",
        ));
    }
    part.has_init = init_line != 0;
    check_interface(def, &mut errors);
    check_sim(def, sim_lines, &mut part, &mut errors);
    if errors.is_empty() {
        Ok(part)
    } else {
        Err(errors)
    }
}

/// One `// crab:<name> key=value ...` line.
fn annotation(
    n: usize,
    rest: &str,
    part: &mut NativePart,
    seen_native: &mut bool,
    sim_lines: &mut Vec<(usize, String, String)>,
    errors: &mut Vec<NativeError>,
) {
    let (name, args) = rest
        .split_once(char::is_whitespace)
        .map_or((rest.trim(), ""), |(a, b)| (a, b));
    if name == "native" && std::mem::replace(seen_native, true) {
        errors.push(err(n, "заголовок // crab:native повторён"));
    }
    let pairs = match parse_pairs(args) {
        Ok(p) => p,
        Err(e) => {
            errors.push(err(n, format!("// crab:{name}: {e}")));
            return;
        }
    };
    match name {
        "native" => {
            for (key, value) in pairs {
                if key == "libs" {
                    part.libs = value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                } else {
                    part.resources.insert(key, value);
                }
            }
        }
        "sim" => {
            for (key, value) in pairs {
                sim_lines.push((n, key, value));
            }
        }
        other => errors.push(err(
            n,
            format!("неизвестная аннотация // crab:{other} (бывают native и sim)"),
        )),
    }
}

/// `key=value key="value with spaces"`, in order.
fn parse_pairs(args: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs = Vec::new();
    let mut rest = args.trim_start();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else {
            return Err(format!("ожидалось имя=значение, а не «{rest}»"));
        };
        let key = rest[..eq].trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(format!("неверное имя «{key}» перед «=»"));
        }
        let after = &rest[eq + 1..];
        let (value, tail) = if let Some(quoted) = after.strip_prefix('"') {
            let Some(end) = quoted.find('"') else {
                return Err(format!("не закрыты кавычки у «{key}»"));
            };
            (quoted[..end].to_owned(), &quoted[end + 1..])
        } else {
            let end = after.find(char::is_whitespace).unwrap_or(after.len());
            (after[..end].to_owned(), &after[end..])
        };
        pairs.push((key.to_owned(), value));
        rest = tail.trim_start();
    }
    Ok(pairs)
}

/// The code of a line without `//` and `/* */` comments and string literals.
fn strip_comments(line: &str, in_block: &mut bool) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if *in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                *in_block = false;
            }
            continue;
        }
        if in_str {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '/' if chars.peek() == Some(&'/') => break,
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                *in_block = true;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Whether `code` has `name(` as a whole word (an entry point, not `MY_CRAB_STEP(`).
fn has_call(code: &str, name: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = code[from..].find(name) {
        let start = from + pos;
        let end = start + name.len();
        let before_ok = code[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let after_ok = code[end..].trim_start().starts_with('(');
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

/// Whether `code` starts `struct State` or `class State` (a whole word).
fn defines_state(code: &str) -> bool {
    ["struct", "class"].iter().any(|kw| {
        code.match_indices(kw).any(|(at, _)| {
            let before_ok = code[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            let rest = code[at + kw.len()..].trim_start();
            let named = rest.strip_prefix("State").is_some_and(|tail| {
                tail.chars()
                    .next()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
            });
            before_ok && code[at + kw.len()..].starts_with(char::is_whitespace) && named
        })
    })
}

/// A name that is a C++ identifier: ports and parameters become struct fields.
fn is_cpp_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Types that cross into native code: `bool`, `int`, `float` (a pin or a duration is an `int`).
fn check_interface(def: &BlockDef, errors: &mut Vec<NativeError>) {
    let ports = def
        .inputs
        .iter()
        .chain(&def.outputs)
        .map(|p| (&p.name, p.ty));
    for (name, kind) in ports {
        if !is_cpp_ident(name) {
            errors.push(err(
                0,
                format!("имя порта «{name}» нельзя использовать в нативной части: в C++ оно станет полем структуры, нужны латинские буквы, цифры и _"),
            ));
        }
        if !matches!(kind, SignalType::Bool | SignalType::Int | SignalType::Float) {
            errors.push(err(
                0,
                format!(
                    "порт «{name}» типа {} нельзя использовать в нативной части (допустимы bool, int, float)",
                    kind.name()
                ),
            ));
        }
    }
    for p in &def.params {
        if !is_cpp_ident(&p.name) {
            errors.push(err(
                0,
                format!(
                    "имя параметра «{}» нельзя использовать в нативной части: нужны латинские буквы, цифры и _",
                    p.name
                ),
            ));
        }
        if p.kind == ParamKind::Text {
            errors.push(err(
                0,
                format!(
                    "параметр «{}» текстовый: в нативной части допустимы bool, int, float, время и пин",
                    p.name
                ),
            ));
        }
    }
}

/// `// crab:sim name=value`: the name is an output of the block and the value fits its type.
fn check_sim(
    def: &BlockDef,
    lines: Vec<(usize, String, String)>,
    part: &mut NativePart,
    errors: &mut Vec<NativeError>,
) {
    for (n, key, value) in lines {
        let Some(port) = def.outputs.iter().find(|o| o.name == key) else {
            errors.push(err(n, format!("// crab:sim: у блока нет выхода «{key}»")));
            continue;
        };
        let parsed = SignalType::guess_value(&value);
        if !port.ty.accepts(&parsed) {
            errors.push(err(
                n,
                format!(
                    "// crab:sim: значение «{value}» не подходит выходу «{key}» типа {}",
                    port.ty.name()
                ),
            ));
            continue;
        }
        let stored = match (port.ty, parsed) {
            (SignalType::Float, ParamValue::Int(v)) => ParamValue::Float(v as f64),
            (_, v) => v,
        };
        if part.sim.insert(key.clone(), stored).is_some() {
            errors.push(err(n, format!("// crab:sim: выход «{key}» задан дважды")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(extra: &str) -> BlockDef {
        toml::from_str(&format!(
            r#"
[block]
type = "bmp"
name = "BMP"
group = "Датчики"

[[output]]
name = "temperature"
type = "float"
[[output]]
name = "ok"
type = "bool"
{extra}
"#
        ))
        .unwrap()
    }

    const GOOD: &str = r#"// crab:native libs="Adafruit BMP280, Wire" i2c=0x76
// crab:sim temperature=21 ok=true
#include <Adafruit_BMP280.h>
struct State { int x; };
CRAB_INIT(State& s, Params p) { /* CRAB_STEP( in a comment */ }
CRAB_STEP(State& s, In in, Out& out) { out.ok = true; }
"#;

    #[test]
    fn parses_a_good_file() {
        let part = parse_native(GOOD, &def("")).unwrap();
        assert_eq!(part.libs, ["Adafruit BMP280", "Wire"]);
        assert_eq!(part.resources["i2c"], "0x76");
        assert!(part.has_init);
        assert_eq!(part.sim["temperature"], ParamValue::Float(21.0));
        assert_eq!(part.sim["ok"], ParamValue::Bool(true));
    }

    #[test]
    fn init_is_optional_and_step_is_required() {
        let only_step = "CRAB_STEP(State& s, In in, Out& out) {}\n";
        assert!(!parse_native(only_step, &def("")).unwrap().has_init);
        let e = parse_native("CRAB_INIT(State& s, Params p) {}\n", &def("")).unwrap_err();
        assert!(e[0].message.contains("нет функции CRAB_STEP"), "{e:?}");
    }

    #[test]
    fn entry_points_must_be_whole_words_outside_comments() {
        let src = "// CRAB_STEP(x)\nvoid MY_CRAB_STEP(int a) {}\nconst char* s = \"CRAB_STEP(\";\n";
        assert!(parse_native(src, &def("")).is_err());
    }

    #[test]
    fn duplicates_are_reported_with_lines() {
        let src = "CRAB_STEP(a) {}\nCRAB_STEP(b) {}\n";
        let e = parse_native(src, &def("")).unwrap_err();
        assert_eq!(e[0].to_string(), "строка 2: CRAB_STEP объявлена второй раз");
    }

    #[test]
    fn bad_annotations_are_explained_in_russian() {
        let src = "// crab:driver name=x\n// crab:native libs\n// crab:native libs=\"a\n// crab:native a=1\nCRAB_STEP(a) {}\n";
        let e = parse_native(src, &def("")).unwrap_err();
        let text = join_errors(&e);
        assert!(
            text.contains("строка 1: неизвестная аннотация // crab:driver"),
            "{text}"
        );
        assert!(
            text.contains("строка 2:") && text.contains("ожидалось имя=значение"),
            "{text}"
        );
        assert!(
            text.contains("строка 3:") && text.contains("не закрыты кавычки"),
            "{text}"
        );
        assert!(
            text.contains("строка 4: заголовок // crab:native повторён"),
            "{text}"
        );
    }

    #[test]
    fn sim_is_checked_against_outputs() {
        let src = "// crab:sim nope=1 temperature=abc ok=1\nCRAB_STEP(a) {}\n";
        let e = parse_native(src, &def("")).unwrap_err();
        let text = join_errors(&e);
        assert!(text.contains("у блока нет выхода «nope»"), "{text}");
        assert!(
            text.contains("значение «abc» не подходит выходу «temperature» типа float"),
            "{text}"
        );
        assert!(
            text.contains("значение «1» не подходит выходу «ok» типа bool"),
            "{text}"
        );
    }

    #[test]
    fn state_definition_is_detected() {
        let with = "struct State { int x; };
CRAB_STEP(State& s, In in, Out& out) {}
";
        assert!(parse_native(with, &def("")).unwrap().has_state);
        let without = "struct StateX {};
// struct State
CRAB_STEP(a) {}
";
        assert!(!parse_native(without, &def("")).unwrap().has_state);
    }

    #[test]
    fn port_names_must_be_cpp_identifiers() {
        let d = def("[[input]]
name = \"темп\"
type = \"int\"
");
        let e = parse_native(
            "CRAB_STEP(a) {}
",
            &d,
        )
        .unwrap_err();
        assert!(join_errors(&e).contains("имя порта «темп»"), "{e:?}");
    }

    #[test]
    fn unsupported_types_are_rejected() {
        let d = def("[[input]]\nname = \"msg\"\ntype = \"string\"\n[[param]]\nname = \"ssid\"\ntype = \"string\"\ndefault = \"x\"\n");
        let e = parse_native("CRAB_STEP(a) {}\n", &d).unwrap_err();
        let text = join_errors(&e);
        assert!(text.contains("порт «msg» типа string нельзя"), "{text}");
        assert!(text.contains("параметр «ssid» текстовый"), "{text}");
    }
}
