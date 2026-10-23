//! Arduino C++ backend: renders block templates `esp32.cpp.tmpl` into a sketch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::value::parse_duration_ms;
use flp_core::{ParamKind, ParamValue, SignalType};
use minijinja::{context, AutoEscape, Environment, UndefinedBehavior, Value};

use crate::{Backend, GenError, Instance, Program, Source};

/// Arduino C++ for ESP32, built by arduino-cli.
pub struct ArduinoCpp;

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
        Ok(assemble(program, &rendered))
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
}

fn render_block(
    env: &Environment,
    program: &Program,
    inst: &Instance,
) -> Result<Sections, GenError> {
    let kind = &inst.block.kind;
    let path =
        inst.lib
            .template_path(ESP32_CPP_TEMPLATE)
            .ok_or_else(|| GenError::MissingTemplate {
                kind: kind.clone(),
                file: ESP32_CPP_TEMPLATE,
            })?;
    let source = fs::read_to_string(&path).map_err(|source| GenError::Io {
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

/// Splits rendered template text by `// @global`, `// @setup`, `// @loop` markers.
fn split_sections(text: &str) -> Result<Sections, String> {
    let mut sections = Sections::default();
    let mut current: Option<&mut String> = None;
    for line in text.lines() {
        let marker = line.trim();
        if let Some(name) = marker.strip_prefix("// @") {
            current = Some(match name {
                "global" => &mut sections.global,
                "setup" => &mut sections.setup,
                "loop" => &mut sections.looped,
                _ => return Err(format!("неизвестный раздел «{marker}»")),
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
                    "код до первого раздела (// @global, // @setup, // @loop): «{marker}»"
                ))
            }
        }
    }
    for s in [
        &mut sections.global,
        &mut sections.setup,
        &mut sections.looped,
    ] {
        let trimmed = s.trim_matches('\n').to_owned();
        *s = trimmed;
    }
    Ok(sections)
}

fn assemble(program: &Program, rendered: &[(&Instance, Sections)]) -> String {
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

    for (inst, sec) in rendered {
        if !sec.global.is_empty() {
            let _ = writeln!(out, "\n{}\n{}", block_comment(inst), sec.global);
        }
    }

    out.push_str("\nvoid setup() {\n");
    push_function_body(&mut out, rendered, |s| &s.setup);
    out.push_str("}\n\nvoid loop() {\n");
    push_function_body(&mut out, rendered, |s| &s.looped);
    out.push_str("}\n");
    out
}

fn push_function_body(
    out: &mut String,
    rendered: &[(&Instance, Sections)],
    section: fn(&Sections) -> &String,
) {
    let mut first = true;
    for (inst, sec) in rendered {
        let code = section(sec);
        if code.is_empty() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        let _ = writeln!(out, "  {}", block_comment(inst));
        for line in code.lines() {
            if line.is_empty() {
                out.push('\n');
            } else {
                let _ = writeln!(out, "  {line}");
            }
        }
    }
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
    fn literals() {
        assert_eq!(float_literal(2.0), "2.0f");
        assert_eq!(float_literal(0.1), "0.1f");
        assert_eq!(float_literal(1e20), "1e20f");
        assert_eq!(float_literal(f64::NEG_INFINITY), "-INFINITY");
        assert_eq!(string_literal("a\"b\\c\n"), r#""a\"b\\c\n""#);
    }
}
