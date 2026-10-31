//! C++ source of a block's simulation module, made from its `esp32.cpp.tmpl`.
//!
//! The template is rendered with run-time variables in place of the ports and
//! parameters, wrapped in a state struct (so `init()` can reset it) and given
//! the `flp` imports of a simulation module (see `flp-sim`). Blocks that touch
//! hardware or libraries are refused with the list of offending lines.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use flp_core::{BlockDef, ParamKind, SignalType};
use minijinja::{context, Value};

use flp_core::ident::c_ident;

use super::{environment, raw_value, split_sections, strip_literal_suffix, Sections};

/// Names that make a template impossible to run on a computer.
const UNSUPPORTED: &[&str] = &[
    "pinMode",
    "digitalRead",
    "digitalWrite",
    "analogRead",
    "analogWrite",
    "analogReadResolution",
    "ledcSetup",
    "ledcAttachPin",
    "ledcWrite",
    "dacWrite",
    "touchRead",
    "attachInterrupt",
    "detachInterrupt",
    "pulseIn",
    "shiftIn",
    "shiftOut",
    "tone",
    "noTone",
    "delay",
    "delayMicroseconds",
    "yield",
    "Serial",
    "Serial1",
    "Serial2",
    "Wire",
    "SPI",
    "WiFi",
    "WiFiClient",
    "WebServer",
    "HTTPClient",
    "EEPROM",
    "Preferences",
    "ESP",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "pow",
    "exp",
    "log",
    "log10",
    "random",
    "randomSeed",
];

/// Why a block cannot be simulated from its template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimSourceError {
    /// Hardware, libraries or unsupported features: (template line or 0, what).
    Unsupported(Vec<(usize, String)>),
    /// The template does not render or is not a valid template.
    Template(String),
}

impl std::fmt::Display for SimSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(list) => {
                f.write_str("не симулируется: ")?;
                for (i, (line, what)) in list.iter().enumerate() {
                    if i > 0 {
                        f.write_str("; ")?;
                    }
                    if *line == 0 {
                        f.write_str(what)?;
                    } else {
                        write!(f, "строка {line}: {what}")?;
                    }
                }
                Ok(())
            }
            Self::Template(message) => f.write_str(message),
        }
    }
}

/// Word-like tokens of `line` outside string literals and `//` comments.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q && prev != '\\' {
                quote = None;
            }
            prev = c;
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            break;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
        }
        if quote.is_none() && (c.is_alphanumeric() || c == '_') {
            word.push(c);
        } else if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        prev = c;
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// What in the template cannot run on a computer, with line numbers.
fn unsupported(def: &BlockDef, template: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    if def.info.uart.is_some() {
        found.push((0, "блок работает с UART".to_owned()));
    }
    for (n, line) in template.lines().enumerate() {
        let line_no = n + 1;
        if line.trim_start().starts_with("#include") {
            found.push((line_no, format!("подключение библиотеки: {}", line.trim())));
            continue;
        }
        for token in tokens(line) {
            if UNSUPPORTED.contains(&token.as_str()) {
                found.push((line_no, format!("вызов «{token}»")));
            }
        }
    }
    found
}

/// C++ type of a port in the simulation module.
fn port_type(ty: SignalType) -> Option<&'static str> {
    match ty {
        SignalType::Bool => Some("bool"),
        SignalType::Int => Some("int32_t"),
        SignalType::Float | SignalType::Any | SignalType::Number | SignalType::Same => {
            Some("float")
        }
        SignalType::String => Some("String"),
    }
}

fn param_type(kind: ParamKind) -> Option<&'static str> {
    match kind {
        ParamKind::Bool => Some("bool"),
        ParamKind::Int => Some("int32_t"),
        ParamKind::Float => Some("float"),
        ParamKind::Duration => Some("uint32_t"),
        ParamKind::Text => Some("String"),
        ParamKind::Pin => None,
    }
}

/// Arduino `String` for modules (fixed capacity, no libc).
const STRING_CLASS: &str = include_str!("sim_string.h");

const PRELUDE: &str = r#"#include <stdint.h>
#define FLP(name) __attribute__((import_module("flp"), import_name(#name)))
#define EXPORT(name) __attribute__((export_name(#name)))

extern "C" {
FLP(input) double input(int index);
FLP(param) double param(int index);
FLP(output) void output(int index, double value);
}

// @STRING
// Arduino helpers the templates use.
typedef uint8_t byte;
static unsigned long flp_now_ms = 0;
static unsigned long millis() { return flp_now_ms; }
static unsigned long micros() { return flp_now_ms * 1000UL; }
#define constrain(x, a, b) ((x) < (a) ? (a) : ((x) > (b) ? (b) : (x)))
#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))
#define abs(x) ((x) > 0 ? (x) : -(x))
#define sqrt(x) __builtin_sqrt(x)
#define PI 3.1415926535897932384626433832795
"#;

/// Source of the simulation module of `def` made from `template`.
///
/// Inputs, outputs and parameters are numbered in the order of `[[input]]`,
/// `[[output]]` and `[[param]]`, as the `flp` imports expect.
pub fn sim_source(def: &BlockDef, template: &str) -> Result<String, SimSourceError> {
    let mut found = unsupported(def, template);
    for p in def.params.iter().filter(|p| param_type(p.kind).is_none()) {
        found.push((0, format!("параметр «{}» типа {:?}", p.name, p.kind)));
    }
    if !found.is_empty() {
        return Err(SimSourceError::Unsupported(found));
    }

    let var = |prefix: &str, name: &str| format!("flp_{prefix}_{}", c_ident(name));
    let input: BTreeMap<&str, String> = def
        .inputs
        .iter()
        .map(|p| (p.name.as_str(), var("in", &p.name)))
        .collect();
    let input_type: BTreeMap<&str, &str> = def
        .inputs
        .iter()
        .map(|p| (p.name.as_str(), p.ty.name()))
        .collect();
    let output: BTreeMap<&str, String> = def
        .outputs
        .iter()
        .map(|p| (p.name.as_str(), var("out", &p.name)))
        .collect();
    let param: BTreeMap<&str, String> = def
        .params
        .iter()
        .map(|p| (p.name.as_str(), var("param", &p.name)))
        .collect();
    // Decisions made while rendering see the default values only.
    let value: BTreeMap<&str, Value> = def
        .params
        .iter()
        .filter_map(|p| Some((p.name.as_str(), raw_value(p.kind, p.default.as_ref()?))))
        .collect();
    let bound: BTreeMap<&str, bool> = def
        .params
        .iter()
        .map(|p| (p.name.as_str(), false))
        .collect();
    let flag: BTreeMap<&str, String> = def
        .params
        .iter()
        .filter(|p| p.kind == ParamKind::Bool)
        .map(|p| (p.name.as_str(), var("param", &p.name)))
        .collect();

    let mut source = template.to_owned();
    for p in &def.params {
        source = strip_literal_suffix(&source, &p.name);
    }
    let ctx = context! {
        id => "blk",
        input, input_type, output, param, value, bound, flag,
        var => Value::from(()), uart => Value::from(()), expander => Value::from(()),
    };
    let text = environment()
        .render_named_str("esp32.cpp.tmpl", &source, ctx)
        .map_err(|e| SimSourceError::Template(e.to_string()))?;
    let sections = split_sections(&text).map_err(SimSourceError::Template)?;
    Ok(assemble(def, &sections))
}

/// Indents every non-empty line by two spaces.
fn indent(code: &str) -> String {
    code.lines().fold(String::new(), |mut out, l| {
        if !l.is_empty() {
            out.push_str("  ");
            out.push_str(l);
        }
        out.push('\n');
        out
    })
}

fn assemble(def: &BlockDef, s: &Sections) -> String {
    let mut out = PRELUDE.replace(
        "// @STRING
",
        STRING_CLASS,
    );
    if !s.defines.is_empty() {
        let _ = write!(out, "\n{}\n", s.defines);
    }
    out.push('\n');
    for p in &def.inputs {
        let ty = port_type(p.ty).expect("checked");
        let _ = writeln!(out, "static {ty} flp_in_{} = {{}};", c_ident(&p.name));
    }
    for p in &def.outputs {
        let ty = port_type(p.ty).expect("checked");
        let _ = writeln!(out, "static {ty} flp_out_{} = {{}};", c_ident(&p.name));
    }
    for p in &def.params {
        let ty = param_type(p.kind).expect("checked");
        let _ = writeln!(out, "static {ty} flp_param_{} = {{}};", c_ident(&p.name));
    }
    for part in [&s.once_global, &s.once_functions] {
        if !part.is_empty() {
            let _ = write!(out, "\n{part}\n");
        }
    }
    // The block's own variables are members, so `init()` resets them all at once.
    out.push_str("\nstruct FlpState {\n");
    for part in [&s.global, &s.functions] {
        if !part.is_empty() {
            out.push_str(&indent(part));
        }
    }
    let setup = [s.once_setup.as_str(), s.setup.as_str()].join("\n");
    let _ = write!(
        out,
        "  void setup() {{\n{}  }}\n  void run() {{\n{}  }}\n}};\n",
        indent(&indent(&setup)),
        indent(&indent(&s.looped)),
    );
    out.push_str("\nstatic FlpState flp_state;\n\nextern \"C\" {\n");
    out.push_str("EXPORT(init) void init() {\n  flp_now_ms = 0;\n  flp_state = FlpState();\n");
    for p in &def.outputs {
        let _ = writeln!(out, "  flp_out_{} = {{}};", c_ident(&p.name));
    }
    out.push_str("  flp_state.setup();\n}\n\n");
    out.push_str("EXPORT(step) void step(int dt_ms) {\n  flp_now_ms += dt_ms;\n");
    for (i, p) in def.inputs.iter().enumerate() {
        let ty = port_type(p.ty).expect("checked");
        let name = c_ident(&p.name);
        if p.ty == SignalType::String {
            let _ = writeln!(out, "  flp_in_{name} = flp_input_text({i});");
        } else {
            let _ = writeln!(out, "  flp_in_{name} = ({ty})input({i});");
        }
    }
    for (i, p) in def.params.iter().enumerate() {
        let ty = param_type(p.kind).expect("checked");
        let name = c_ident(&p.name);
        if p.kind == ParamKind::Text {
            let _ = writeln!(out, "  flp_param_{name} = flp_param_text({i});");
        } else {
            let _ = writeln!(out, "  flp_param_{name} = ({ty})param({i});");
        }
    }
    out.push_str("  flp_state.run();\n");
    for (i, p) in def.outputs.iter().enumerate() {
        let name = c_ident(&p.name);
        if p.ty == SignalType::String {
            let _ = writeln!(out, "  flp_output_text({i}, flp_out_{name});");
        } else {
            let _ = writeln!(out, "  output({i}, (double)flp_out_{name});");
        }
    }
    out.push_str("}\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(src: &str) -> BlockDef {
        toml::from_str(src).unwrap()
    }

    const PULSE: &str = r#"
[block]
type = "pulse"
name = "Импульс"
group = "Таймеры"
memory = true

[[input]]
name = "in"
type = "bool"

[[output]]
name = "out"
type = "bool"

[[param]]
name = "time"
type = "duration"
default = "1s"
"#;

    const PULSE_TMPL: &str = "// @global
bool {{ id }}_prev = false;
bool {{ id }}_active = false;
unsigned long {{ id }}_start = 0;
// @loop
if ({{ input.in }} && !{{ id }}_prev && !{{ id }}_active) {
  {{ id }}_active = true;
  {{ id }}_start = millis();
}
{{ id }}_prev = {{ input.in }};
if ({{ id }}_active && millis() - {{ id }}_start >= {{ param.time }}UL) {
  {{ id }}_active = false;
}
{{ output.out }} = {{ id }}_active;
";

    #[test]
    fn pulse_becomes_a_module() {
        let code = sim_source(&def(PULSE), PULSE_TMPL).unwrap();
        assert!(code.contains("static bool flp_in_in = {};"), "{code}");
        assert!(
            code.contains("static uint32_t flp_param_time = {};"),
            "{code}"
        );
        assert!(code.contains("bool blk_active = false;"), "{code}");
        assert!(code.contains(">= flp_param_time) {"), "{code}");
        assert!(code.contains("flp_in_in = (bool)input(0);"), "{code}");
        assert!(code.contains("output(0, (double)flp_out_out);"), "{code}");
        assert!(code.contains("EXPORT(init)"), "{code}");
    }

    #[test]
    fn hardware_calls_are_listed_with_lines() {
        let tmpl = "// @setup\npinMode(2, OUTPUT);\n// @loop\n{{ output.out }} = digitalRead(2); // Serial is fine in a comment\n";
        let Err(SimSourceError::Unsupported(list)) = sim_source(&def(PULSE), tmpl) else {
            panic!("must be refused");
        };
        assert_eq!(
            list,
            vec![
                (2, "вызов «pinMode»".to_owned()),
                (4, "вызов «digitalRead»".to_owned())
            ]
        );
    }

    #[test]
    fn includes_are_refused_and_strings_ignored() {
        let tmpl = "#include <Wire.h>\n// @loop\n{{ output.out }} = true;\n";
        let err = sim_source(&def(PULSE), tmpl).unwrap_err();
        assert!(err.to_string().contains("строка 1"), "{err}");
        let ok = "// @loop\nconst char *s = \"digitalRead\";\n{{ output.out }} = true;\n";
        assert!(sim_source(&def(PULSE), ok).is_ok());
    }

    #[test]
    fn library_pulse_template_is_supported() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library/pulse");
        let def = def(&std::fs::read_to_string(dir.join("block.toml")).unwrap());
        let tmpl = std::fs::read_to_string(dir.join("esp32.cpp.tmpl")).unwrap();
        assert!(sim_source(&def, &tmpl).is_ok());
    }
}
