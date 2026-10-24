//! Arduino C++ backend: renders block templates `esp32.cpp.tmpl` into a sketch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::value::parse_duration_ms;
use flp_core::vars::VAR_SET;
use flp_core::{ParamKind, ParamValue, PortRef, SignalType};
use minijinja::{context, AutoEscape, Environment, UndefinedBehavior, Value};

use crate::{Backend, GenError, Instance, Program, Source};

/// Arduino C++ for ESP32, built by arduino-cli.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArduinoCpp {
    /// Embed the online debugging module.
    pub debug: bool,
}

impl ArduinoCpp {
    pub const RELEASE: Self = Self { debug: false };
    pub const DEBUG: Self = Self { debug: true };
}

impl Backend for ArduinoCpp {
    fn template_file(&self) -> &'static str {
        ESP32_CPP_TEMPLATE
    }

    fn generate(&self, program: &Program) -> Result<String, GenError> {
        let env = environment();
        let mut rendered = Vec::with_capacity(program.blocks.len());
        for inst in &program.blocks {
            rendered.push((inst, render_block(&env, program, inst)?));
        }
        Ok(assemble(program, &rendered, self.debug))
    }
}

fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_trim_blocks(true);
    env
}

/// Code of one block split into sketch sections.
#[derive(Debug, Default, PartialEq)]
struct Sections {
    global: String,
    setup: String,
    looped: String,
    functions: String,
    /// `// @global once`, `// @setup once`, `// @functions once`: shared by all
    /// instances of the block type; the same text goes into the sketch once.
    once_global: String,
    once_setup: String,
    once_functions: String,
}

const SECTION_LIST: &str = "// @global, // @setup, // @loop, // @functions";

fn render_block(
    env: &Environment,
    program: &Program,
    inst: &Instance,
) -> Result<Sections, GenError> {
    let kind = &inst.block.kind;
    // Built-in blocks have no folder; errors then point at the type name.
    let path = inst
        .lib
        .template_path(ESP32_CPP_TEMPLATE)
        .unwrap_or_else(|| PathBuf::from(format!("{kind}/{ESP32_CPP_TEMPLATE}")));
    let source = inst
        .lib
        .template(ESP32_CPP_TEMPLATE)
        .ok_or_else(|| GenError::MissingTemplate {
            kind: kind.clone(),
            file: ESP32_CPP_TEMPLATE,
        })?
        .map_err(|source| GenError::Io {
            path: path.clone(),
            source,
        })?;

    let input: BTreeMap<&str, String> = inst
        .inputs
        .iter()
        .map(|(&port, i)| {
            let expr = match &i.source {
                Source::Signal(s) => {
                    let signal = &program.signals[*s];
                    if signal.ty == i.ty {
                        signal.name.clone()
                    } else {
                        format!("({}){}", cpp_type(i.ty), signal.name)
                    }
                }
                Source::Default(value) => constant_literal(i.ty, value),
            };
            (port, expr)
        })
        .collect();
    let output: BTreeMap<&str, &str> = inst
        .outputs
        .iter()
        .map(|(&port, &s)| (port, program.signals[s].name.as_str()))
        .collect();
    let mut param = BTreeMap::new();
    let mut value = BTreeMap::new();
    for (&name, v) in &inst.params {
        let kind = inst.lib.def.param(name).expect("param from def").kind;
        param.insert(name, param_value(kind, v));
        value.insert(name, raw_value(kind, v));
    }

    let name = format!("{kind}/{ESP32_CPP_TEMPLATE}");
    let ctx = context! { id => inst.block.id, input, output, param, value };
    let text = env
        .render_named_str(&name, &source, ctx)
        .map_err(|source| GenError::Render {
            block: inst.block.id.clone(),
            source,
        })?;
    split_sections(&text).map_err(|message| GenError::BadTemplate { path, message })
}

/// Parameter as seen by templates: bools stay bools for `{% if %}`,
/// everything else is a ready C++ literal.
fn param_value(kind: ParamKind, value: &ParamValue) -> Value {
    match (kind, value) {
        (_, ParamValue::Bool(b)) => Value::from(*b),
        (ParamKind::Float, ParamValue::Int(v)) => Value::from(float_literal(*v as f64)),
        (_, ParamValue::Int(v)) => Value::from(v.to_string()),
        (_, ParamValue::Float(v)) => Value::from(float_literal(*v)),
        (ParamKind::Duration, ParamValue::Text(s)) => {
            let ms = parse_duration_ms(s).expect("duration checked");
            Value::from(ms.to_string())
        }
        (_, ParamValue::Text(s)) => Value::from(string_literal(s)),
    }
}

/// Parameter as a plain value for comparisons in templates (`{% if value.on > value.off %}`);
/// durations are milliseconds.
fn raw_value(kind: ParamKind, value: &ParamValue) -> Value {
    match value {
        ParamValue::Bool(b) => Value::from(*b),
        ParamValue::Int(v) => Value::from(*v),
        ParamValue::Float(v) => Value::from(*v),
        ParamValue::Text(s) if kind == ParamKind::Duration => {
            Value::from(parse_duration_ms(s).expect("duration checked"))
        }
        ParamValue::Text(s) => Value::from(s.as_str()),
    }
}

/// Literal for the default value of an unconnected input.
fn constant_literal(ty: SignalType, value: &ParamValue) -> String {
    match (ty, value) {
        (_, ParamValue::Bool(b)) => b.to_string(),
        (SignalType::Float, ParamValue::Int(v)) => float_literal(*v as f64),
        (_, ParamValue::Int(v)) => v.to_string(),
        (_, ParamValue::Float(v)) => float_literal(*v),
        (_, ParamValue::Text(_)) => {
            unreachable!("input defaults are checked against the port type")
        }
    }
}

fn cpp_type(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int32_t",
        SignalType::Float => "float",
    }
}

fn zero(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "false",
        SignalType::Int => "0",
        SignalType::Float => "0.0f",
    }
}

/// `float` literal; `f` suffix avoids slow double math on ESP32.
fn float_literal(v: f64) -> String {
    if v.is_nan() {
        "NAN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "INFINITY" } else { "-INFINITY" }.into()
    } else {
        // Debug formatting always has `.` or an exponent: `2.0`, `1e20`.
        format!("{v:?}f")
    }
}

fn string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Splits rendered template text by section markers: `// @global`, `// @setup`,
/// `// @loop`, `// @functions`, and `once` variants of all but `loop`.
fn split_sections(text: &str) -> Result<Sections, String> {
    let mut sections = Sections::default();
    let mut current: Option<&mut String> = None;
    for line in text.lines() {
        let marker = line.trim();
        if let Some(name) = marker.strip_prefix("// @") {
            let words: Vec<&str> = name.split_whitespace().collect();
            current = Some(match words.as_slice() {
                ["global"] => &mut sections.global,
                ["setup"] => &mut sections.setup,
                ["loop"] => &mut sections.looped,
                ["functions"] => &mut sections.functions,
                ["global", "once"] => &mut sections.once_global,
                ["setup", "once"] => &mut sections.once_setup,
                ["functions", "once"] => &mut sections.once_functions,
                ["loop", "once"] => {
                    return Err("раздела «// @loop once» нет: код цикла выполняется \
                                для каждого блока в порядке схемы"
                        .to_owned())
                }
                _ => {
                    return Err(format!(
                        "неизвестный раздел «{marker}»: допустимы {SECTION_LIST}, \
                         к любому, кроме loop, можно добавить «once»"
                    ))
                }
            });
            continue;
        }
        match current.as_deref_mut() {
            Some(buf) => {
                buf.push_str(line.trim_end());
                buf.push('\n');
            }
            None if marker.is_empty() => {}
            None => {
                return Err(format!(
                    "код до первого раздела ({SECTION_LIST}): «{marker}»"
                ))
            }
        }
    }
    for s in [
        &mut sections.global,
        &mut sections.setup,
        &mut sections.looped,
        &mut sections.functions,
        &mut sections.once_global,
        &mut sections.once_setup,
        &mut sections.once_functions,
    ] {
        let trimmed = s.trim_matches('\n').to_owned();
        *s = trimmed;
    }
    Ok(sections)
}

fn assemble(program: &Program, rendered: &[(&Instance, Sections)], debug: bool) -> String {
    let mut out = String::new();
    let order: Vec<&str> = program.blocks.iter().map(|b| b.block.id.as_str()).collect();
    let _ = writeln!(
        out,
        "// Сгенерировано FLP_next из проекта «{}». Ручные правки будут перезаписаны.",
        program.project.config.project.name
    );
    let _ = writeln!(out, "// Порядок выполнения: {}", order.join(" → "));
    out.push_str("\n#include <Arduino.h>\n");
    for header in program.includes() {
        let _ = writeln!(out, "#include <{header}>");
    }

    if !program.signals.is_empty() {
        out.push_str("\n// Выходы блоков\n");
        for s in &program.signals {
            let _ = writeln!(out, "{} {} = {};", cpp_type(s.ty), s.name, zero(s.ty));
        }
    }

    let vars = &program.project.logic.vars;
    if !vars.is_empty() {
        out.push_str("\n// Переменные\n");
        for v in vars {
            let init = v.init.as_ref().map_or_else(
                || zero(v.ty.signal()).to_owned(),
                |i| constant_literal(v.ty.signal(), i),
            );
            let _ = writeln!(
                out,
                "{} {} = {};",
                v.ty.cpp(),
                flp_core::vars::storage_name(&v.name),
                init
            );
        }
    }

    for (kind, code) in shared(rendered, |s| &s.once_global) {
        let _ = writeln!(out, "\n{}\n{code}", shared_comment(kind));
    }
    for (inst, sec) in rendered {
        if !sec.global.is_empty() {
            let _ = writeln!(out, "\n{}\n{}", block_comment(inst), sec.global);
        }
    }

    // Functions go before setup() and loop(), so no prototypes are needed.
    for (kind, code) in shared(rendered, |s| &s.once_functions) {
        let _ = writeln!(out, "\n{}\n{code}", shared_comment(kind));
    }
    for (inst, sec) in rendered {
        if !sec.functions.is_empty() {
            let _ = writeln!(out, "\n{}\n{}", block_comment(inst), sec.functions);
        }
    }

    let table = debug.then(|| crate::debug::debug_table(program));
    if let Some(table) = &table {
        out.push_str(&crate::debug::runtime(table));
    }
    // Forcing: after each block, its forced outputs (and a written variable) win.
    let forces = |inst: &Instance| -> Vec<String> {
        let Some(table) = &table else {
            return Vec::new();
        };
        let mut lines: Vec<String> = inst
            .outputs
            .keys()
            .filter_map(|&port| table.output_index(&PortRef::new(&inst.block.id, port)))
            .map(|i| format!("flp_dbg_force({i});"))
            .collect();
        if inst.block.kind == VAR_SET {
            if let Some(i) = inst.block.var_name().and_then(|v| table.var_index(v)) {
                lines.push(format!("flp_dbg_force({i});"));
            }
        }
        lines
    };

    out.push_str("\nvoid setup() {\n");
    if debug {
        out.push_str("  flp_dbg_setup();\n\n");
    }
    let once_setup = shared(rendered, |s| &s.once_setup);
    for (i, (kind, code)) in once_setup.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "  {}", shared_comment(kind));
        push_indented(&mut out, code.lines().map(str::to_owned));
    }
    let after_shared = !once_setup.is_empty();
    push_function_body(&mut out, rendered, after_shared, |s| &s.setup, &|_| {
        Vec::new()
    });
    out.push_str("}\n\nvoid loop() {\n");
    if let Some(table) = &table {
        let vars: Vec<String> = (0..table.entries.len())
            .filter(|&i| matches!(table.entries[i].source, flp_debug::Source::Var(_)))
            .map(|i| format!("flp_dbg_force({i});"))
            .collect();
        if !vars.is_empty() {
            let _ = writeln!(out, "  // Forced variables\n  {}\n", vars.join(" "));
        }
    }
    push_function_body(&mut out, rendered, false, |s| &s.looped, &forces);
    if debug {
        out.push_str("\n  flp_dbg_loop();\n");
    }
    out.push_str("}\n");
    out
}

/// Writes the code of each block into a function body; `after` means the body
/// already has code, so the first block is separated by a blank line too.
fn push_function_body(
    out: &mut String,
    rendered: &[(&Instance, Sections)],
    after: bool,
    section: fn(&Sections) -> &String,
    extra: &dyn Fn(&Instance) -> Vec<String>,
) {
    let mut first = !after;
    for (inst, sec) in rendered {
        let code = section(sec);
        let extra = extra(inst);
        if code.is_empty() && extra.is_empty() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        let _ = writeln!(out, "  {}", block_comment(inst));
        push_indented(out, code.lines().map(str::to_owned).chain(extra));
    }
}

fn push_indented(out: &mut String, lines: impl Iterator<Item = String>) {
    for line in lines {
        if line.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "  {line}");
        }
    }
}

/// Non-empty `once` sections, without repeats of the same block type and text,
/// in order of first use.
fn shared<'a>(
    rendered: &'a [(&Instance, Sections)],
    section: fn(&Sections) -> &String,
) -> Vec<(&'a str, &'a str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for (inst, sec) in rendered {
        let item = (inst.block.kind.as_str(), section(sec).as_str());
        if !item.1.is_empty() && !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn shared_comment(kind: &str) -> String {
    format!("// {kind}: общий код")
}

fn block_comment(inst: &Instance) -> String {
    format!("// {}: {}", inst.block.id, inst.block.kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections() {
        let s = split_sections("\n// @global\nint x;\n\n// @loop\nx++;\n  y();  \n").unwrap();
        assert_eq!(s.global, "int x;");
        assert_eq!(s.setup, "");
        assert_eq!(s.looped, "x++;\n  y();");
        assert!(split_sections("x++;\n// @loop\n").is_err());
        assert!(split_sections("// @init\n").is_err());
    }

    #[test]
    fn functions_and_once_sections() {
        let s = split_sections(
            "// @global once\nOneWire bus(4);\n// @functions\nint f() { return 1; }\n\
             // @setup  once\nbus.begin();\n// @functions once\nvoid g() {}\n",
        )
        .unwrap();
        assert_eq!(s.once_global, "OneWire bus(4);");
        assert_eq!(s.functions, "int f() { return 1; }");
        assert_eq!(s.once_setup, "bus.begin();");
        assert_eq!(s.once_functions, "void g() {}");
        assert!(split_sections("// @loop once\nx++;\n").is_err());
        assert!(split_sections("// @global twice\n").is_err());
    }

    #[test]
    fn literals() {
        assert_eq!(float_literal(2.0), "2.0f");
        assert_eq!(float_literal(0.1), "0.1f");
        assert_eq!(float_literal(1e20), "1e20f");
        assert_eq!(float_literal(f64::NEG_INFINITY), "-INFINITY");
        assert_eq!(string_literal("a\"b\\c\n"), r#""a\"b\\c\n""#);
    }
}
