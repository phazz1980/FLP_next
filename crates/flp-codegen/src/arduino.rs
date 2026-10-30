//! Arduino C++ backend: renders block templates `esp32.cpp.tmpl` into a sketch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use flp_core::ident::c_ident;
use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::uart::{UartAccess, UartConfig};
use flp_core::value::parse_duration_ms;
use flp_core::vars::VAR_SET;
use flp_core::{ParamKind, ParamValue, PortRef, SignalType};
use minijinja::{context, AutoEscape, Environment, UndefinedBehavior, Value};

use crate::tasks::Tasks;
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
        let tasks = Tasks::new(program)?;
        let mut rendered = Vec::with_capacity(program.blocks.len());
        for inst in &program.blocks {
            let sections = match inst.block.tag.as_deref() {
                Some(tag) if !inst.outputs.is_empty() => input_copy(program, &tasks, inst, tag),
                // A copy of an output writes the pin in its turn; the tag sets it up.
                Some(_) => Sections {
                    looped: render_block(&env, program, &tasks, inst, self.debug)?.looped,
                    ..Default::default()
                },
                None => render_block(&env, program, &tasks, inst, self.debug)?,
            };
            rendered.push((inst, sections));
        }
        Ok(assemble(program, &tasks, &rendered, self.debug))
    }
}

const NL: char = char::from_u32(10).unwrap();

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
    /// `// @defines`: macros placed before the includes (library settings).
    defines: String,
    global: String,
    setup: String,
    looped: String,
    functions: String,
    /// `// @global once`, `// @setup once`, `// @functions once`: shared by all
    /// instances of the block type (and by other types with the same text); the
    /// same text goes into the sketch once.
    once_global: String,
    once_setup: String,
    once_functions: String,
    /// `#include` lines written by hand in the global and function sections: they are
    /// moved up to the other includes, before any declaration.
    includes: Vec<String>,
}

const SECTION_LIST: &str = "// @defines, // @global, // @setup, // @loop, // @functions";

/// A copy of an input reads nothing itself: it takes the values of its tag, which
/// runs before it.
fn input_copy(program: &Program, tasks: &Tasks, inst: &Instance, tag: &str) -> Sections {
    let source = program.blocks.iter().find(|b| b.block.id == tag);
    let mut looped = String::new();
    for (port, &s) in &inst.outputs {
        let Some(&from) = source.and_then(|b| b.outputs.get(port)) else {
            continue;
        };
        let _ = writeln!(
            looped,
            "{} = {};",
            program.signals[s].name,
            tasks.read_name(program, inst, from)
        );
    }
    Sections {
        looped,
        ..Default::default()
    }
}

fn render_block(
    env: &Environment,
    program: &Program,
    tasks: &Tasks,
    inst: &Instance,
    debug: bool,
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
                Source::Signal { index, invert } => {
                    let signal = &program.signals[*index];
                    let name = tasks.read_name(program, inst, *index);
                    if *invert {
                        format!("!{name}")
                    } else if signal.ty == i.ty {
                        name
                    } else {
                        format!("({}){name}", cpp_type(i.ty))
                    }
                }
                Source::Default(value) => constant_literal(i.ty, value),
                Source::Var(name) => flp_core::vars::storage_name(name),
            };
            let expr = if inst.block.inverted_inputs.contains(port) {
                format!("!({expr})")
            } else {
                expr
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
        let literal = match inst.param_vars.get(name) {
            // A text parameter is used as `const char *` in the templates.
            Some(var) if kind == ParamKind::Text => {
                Value::from(format!("{}.c_str()", flp_core::vars::storage_name(var)))
            }
            Some(var) => Value::from(flp_core::vars::storage_name(var)),
            None => param_value(kind, v),
        };
        param.insert(name, literal);
        value.insert(name, raw_value(kind, v));
    }

    let name = format!("{kind}/{ESP32_CPP_TEMPLATE}");
    // Storage of the variable a `var_get` / `var_set` block accesses.
    let var = inst.block.var_name().map(flp_core::vars::storage_name);
    let id = c_ident(&inst.block.id);
    let uart = uart_of(program, inst).map(|(u, _)| uart_value(&u, debug));
    // Real types of the inputs: for `any` ports, the type of what feeds them.
    let input_type: BTreeMap<&str, &str> = inst
        .inputs
        .iter()
        .map(|(&port, i)| (port, i.ty.name()))
        .collect();
    // Parameters taken from variables: templates build their code in and switch it at run time.
    let bound: BTreeMap<&str, bool> = inst
        .lib
        .def
        .params
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                inst.param_vars.contains_key(p.name.as_str()),
            )
        })
        .collect();
    // Boolean parameters as C++ expressions: a variable or `true` / `false`.
    let flag: BTreeMap<&str, String> = inst
        .params
        .iter()
        .filter_map(|(&n, v)| {
            let ParamValue::Bool(b) = v else { return None };
            let text = match inst.param_vars.get(n) {
                Some(var) => flp_core::vars::storage_name(var),
                None => b.to_string(),
            };
            Some((n, text))
        })
        .collect();
    // With a network settings widget on the page, Wi-Fi and Ethernet take their address
    // settings from it (saved in flash) instead of the block's own parameters.
    let mut param = param;
    let mut bound = bound;
    let mut flag = flag;
    let mut value = value;
    let mut input = input;
    if kind == "web_net" {
        // The defaults of the form are the settings made in the editor on the Wi-Fi and
        // Ethernet blocks: the page starts from them after every flash.
        const INHERIT: [(&str, &str); 8] = [
            ("init_ssid", "ssid"),
            ("init_password", "password"),
            ("init_dhcp", "dhcp"),
            ("init_ip", "ip"),
            ("init_gateway", "gateway"),
            ("init_subnet", "subnet"),
            ("init_ap_ssid", "ap_ssid"),
            ("init_ap_password", "ap_password"),
        ];
        for (init, from) in INHERIT {
            let found = ["wifi", "ethernet"].iter().find_map(|k| {
                program
                    .blocks
                    .iter()
                    .filter(|b| b.block.kind == *k && !b.param_vars.contains_key(from))
                    .find_map(|b| Some((b, b.params.get(from)?, b.lib.def.param(from)?.kind)))
            });
            let Some((_, v, pk)) = found else { continue };
            if matches!(v, ParamValue::Text(s) if s.is_empty()) {
                continue;
            }
            param.insert(init, param_value(pk, v));
            value.insert(init, raw_value(pk, v));
        }
        let mode = program
            .blocks
            .iter()
            .filter(|b| b.block.kind == "wifi")
            .find_map(|b| match b.params.get("wifi_mode") {
                Some(ParamValue::Int(m)) if (0..=2).contains(m) => Some(*m),
                _ => None,
            });
        if let Some(m) = mode {
            value.insert("init_mode", Value::from(m));
            param.insert("init_mode", Value::from(m.to_string()));
        }
    }
    if matches!(kind.as_str(), "wifi" | "ethernet")
        && program.blocks.iter().any(|b| b.block.kind == "web_net")
    {
        const FIELDS: [(&str, usize); 8] = [
            ("ssid", 0),
            ("password", 1),
            ("dhcp", 2),
            ("ip", 3),
            ("gateway", 4),
            ("subnet", 5),
            ("ap_ssid", 6),
            ("ap_password", 7),
        ];
        for (name, i) in FIELDS {
            let Some(key) = bound.keys().find(|k| **k == name).copied() else {
                continue;
            };
            bound.insert(key, true);
            if name == "dhcp" {
                flag.insert(key, format!("(flpNet.f[{i}] != \"0\")"));
            } else {
                param.insert(key, Value::from(format!("flpNet.f[{i}].c_str()")));
            }
        }
        if kind == "wifi" {
            // The mode is chosen on the page, so the block is built as "set by the mode input".
            value.insert("wifi_mode", Value::from(3));
            input.insert("mode", "flpNet.eff".to_owned());
        }
    }
    // Pin word names of the expander the block stands on, `none` otherwise.
    let expander = match inst.block.expander.as_deref() {
        Some(name) => {
            let (input, output) = crate::expander::state_names(name);
            context! { input, output }
        }
        None => Value::from(()),
    };
    let ctx =
        context! { id, input, input_type, output, param, value, bound, flag, var, uart, expander };
    let text = env
        .render_named_str(&name, &source, ctx)
        .map_err(|source| GenError::Render {
            block: inst.block.id.clone(),
            source,
        })?;
    let mut sections =
        split_sections(&text).map_err(|message| GenError::BadTemplate { path, message })?;
    // Outputs bound to variables are copied right after the block.
    for (port, var) in &inst.output_vars {
        if let Some(name) = output.get(port) {
            if !sections.looped.is_empty() && !sections.looped.ends_with(NL) {
                sections.looped.push(NL);
            }
            sections.looped.push_str(&format!(
                "{} = {name};{NL}",
                flp_core::vars::storage_name(var)
            ));
        }
    }
    Ok(sections)
}

/// UART of a block with `[block.uart]`, with its settings (checked to exist).
pub(crate) fn uart_of(program: &Program, inst: &Instance) -> Option<(UartConfig, UartAccess)> {
    let uart = inst.lib.def.info.uart.as_ref()?;
    let Some(ParamValue::Int(n)) = inst.params.get(uart.param.as_str()) else {
        return None;
    };
    let config = flp_core::uart::config_of(&program.uarts, flp_core::uart::port_number(*n))?;
    Some((config, uart.access))
}

/// `{{ uart.* }}` in templates: ready C++ values of the block's UART.
/// `input` is what readers take bytes from: the port itself, or in debug
/// firmware for USB the console input of the debugging module.
fn uart_value(u: &UartConfig, debug: bool) -> Value {
    let input = if u.number == flp_core::uart::USB && debug {
        "flp_dbg_input".to_owned()
    } else {
        u.serial()
    };
    context! {
        number => u.number,
        serial => u.serial(),
        input,
        speed => u.speed,
        config => u.arduino_config(),
        parity => u.parity_code(),
        rx => u.rx,
        tx => u.tx,
        de => u.de,
        rx_buffer => u.rx_buffer,
    }
}

/// Setup of the UARTs that blocks share (all but `exclusive`, whose library
/// sets the port up itself). USB is started here only if nothing else does.
fn uart_setup(program: &Program, serial_started: bool) -> Vec<String> {
    let uarts: Vec<(UartConfig, UartAccess)> = program
        .blocks
        .iter()
        .filter_map(|inst| uart_of(program, inst))
        .collect();
    // A port owned by a library (Modbus RTU) is opened by that library.
    let owned = |n: u8| {
        uarts
            .iter()
            .any(|(u, a)| u.number == n && *a == UartAccess::Exclusive)
    };
    let mut used: Vec<UartConfig> = Vec::new();
    for (u, _) in &uarts {
        if !owned(u.number) && used.iter().all(|o| o.number != u.number) {
            used.push(u.clone());
        }
    }
    // USB first, then the UARTs in order.
    used.sort_by_key(|u| (u.number != flp_core::uart::USB, u.number));
    let mut lines = Vec::new();
    for u in used {
        if u.number == flp_core::uart::USB {
            if !serial_started {
                lines.push(format!("// USB: {} бод", u.speed));
                lines.push(format!("Serial.begin({});", u.speed));
            }
            continue;
        }
        let serial = u.serial();
        let pin = |p: i64| {
            if p >= 0 {
                format!("GPIO{p}")
            } else {
                "по умолчанию".into()
            }
        };
        lines.push(format!(
            "// UART{}: {} бод, {}, RX {}, TX {}",
            u.number,
            u.speed,
            u.format.trim().to_ascii_uppercase(),
            pin(u.rx),
            pin(u.tx)
        ));
        lines.push(format!("{serial}.setRxBufferSize({});", u.rx_buffer));
        lines.push(format!(
            "{serial}.begin({}, {}, {}, {});",
            u.speed,
            u.arduino_config(),
            u.rx,
            u.tx
        ));
        if u.de >= 0 {
            lines.push(format!(
                "{serial}.setPins(-1, -1, -1, {});  // DE/RE RS-485",
                u.de
            ));
            lines.push(format!("{serial}.setMode(UART_MODE_RS485_HALF_DUPLEX);"));
        }
    }
    lines
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
        (_, ParamValue::Text(s)) => format!("String({})", string_literal(s)),
    }
}

/// Bytes reserved for every string signal and variable at startup.
const STRING_RESERVE: usize = 32;

pub(crate) fn cpp_type(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int32_t",
        SignalType::Float => "float",
        SignalType::String => "String",
        SignalType::Any | SignalType::Number => {
            unreachable!("an any input gets the type of its source")
        }
    }
}

pub(crate) fn zero(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "false",
        SignalType::Int => "0",
        SignalType::Float => "0.0f",
        SignalType::String => "String()",
        SignalType::Any | SignalType::Number => {
            unreachable!("an any input gets the type of its source")
        }
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
                ["defines"] => &mut sections.defines,
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
        &mut sections.functions,
        &mut sections.once_global,
        &mut sections.once_functions,
    ] {
        let (includes, rest): (Vec<&str>, Vec<&str>) = s
            .lines()
            .partition(|l| l.trim_start().starts_with("#include"));
        if !includes.is_empty() {
            sections
                .includes
                .extend(includes.iter().map(|l| l.trim().to_owned()));
            *s = rest.join("\n");
        }
    }
    for s in [
        &mut sections.defines,
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

fn assemble(
    program: &Program,
    tasks: &Tasks,
    rendered: &[(&Instance, Sections)],
    debug: bool,
) -> String {
    let mut out = String::new();
    let order: Vec<&str> = program.blocks.iter().map(|b| b.block.id.as_str()).collect();
    let _ = writeln!(
        out,
        "// Сгенерировано FLP_next из проекта «{}». Ручные правки будут перезаписаны.",
        program.project.config.project.name
    );
    let _ = writeln!(out, "// Порядок выполнения: {}", order.join(" → "));
    let defines: Vec<&str> = rendered
        .iter()
        .map(|(_, sec)| sec.defines.as_str())
        .filter(|d| !d.is_empty())
        .collect();
    if !defines.is_empty() {
        out.push('\n');
        for d in defines {
            let _ = writeln!(out, "{d}");
        }
    }
    out.push_str("\n#include <Arduino.h>\n");
    let mut included = vec!["#include <Arduino.h>".to_owned()];
    for header in program.includes() {
        let line = format!("#include <{header}>");
        let _ = writeln!(out, "{line}");
        included.push(line);
    }
    if let Some(header) = crate::expander::header(program) {
        let line = format!("#include <{header}>");
        if !included.contains(&line) {
            let _ = writeln!(out, "{line}");
            included.push(line);
        }
    }
    // Includes typed by hand in a block template go here, ahead of all declarations.
    for (_, sec) in rendered {
        for line in &sec.includes {
            if !included.contains(line) {
                let _ = writeln!(out, "{line}");
                included.push(line.clone());
            }
        }
    }
    if tasks.any() {
        out.push_str("#include <freertos/FreeRTOS.h>\n#include <freertos/semphr.h>\n#include <freertos/task.h>\n");
    }

    if !program.signals.is_empty() {
        out.push_str("\n// Выходы блоков\n");
        for s in &program.signals {
            let _ = writeln!(out, "{} {} = {};", cpp_type(s.ty), s.name, zero(s.ty));
        }
    }

    if tasks.any() {
        out.push_str("\n// Обмен сигналами между основной и фоновой задачами\nSemaphoreHandle_t flp_bg_lock;\n");
        for line in tasks.globals(program) {
            let _ = writeln!(out, "{line}");
        }
    }

    let gate_globals = tasks.gate_globals();
    if !gate_globals.is_empty() {
        out.push_str("\n// Периоды фоновых плат\n");
        for line in gate_globals {
            let _ = writeln!(out, "{line}");
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
    out.push_str(&crate::retain::globals(vars));
    out.push_str(&crate::expander::globals(program));

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

    let diagnostics = &program.project.config.diagnostics;
    let stats = debug || diagnostics.enabled;
    let looped = rendered
        .iter()
        .filter(|(inst, sec)| !sec.looped.is_empty() && !tasks.is_background(inst))
        .map(|(inst, _)| *inst);
    let sheets = crate::stats::measured_sheets(program, looped);
    if stats {
        out.push_str(&crate::stats::runtime(&sheets, tasks.any()));
        if !debug {
            out.push_str(&crate::stats::text_report(
                diagnostics.interval_ms,
                tasks.any(),
            ));
        }
    }
    let table = debug.then(|| crate::debug::debug_table(program));
    // Console input is compiled in only for a block reading USB.
    let text_in = program.blocks.iter().any(
        |inst| matches!(uart_of(program, inst), Some((u, UartAccess::Read)) if u.number == flp_core::uart::USB),
    );
    if let Some(table) = &table {
        out.push_str(&crate::debug::runtime(table, text_in, tasks.any()));
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

    // Cycle time: a mark where execution moves to blocks of another sheet.
    let mut marks: BTreeMap<&str, usize> = BTreeMap::new();
    if stats {
        let mut current = None;
        for (inst, sec) in rendered {
            if tasks.is_background(inst) || (sec.looped.is_empty() && forces(inst).is_empty()) {
                continue;
            }
            let sheet = crate::stats::sheet_of(program, inst);
            let k = sheets.iter().position(|&s| s == sheet);
            if let Some(k) = k.filter(|_| k != current) {
                marks.insert(&inst.block.id, k);
                current = Some(k);
            }
        }
    }
    let mark = |inst: &Instance| -> Vec<String> {
        marks
            .get(inst.block.id.as_str())
            .map(|&k| vec![format!("flp_st_sheet({k});  // лист {}", sheets[k])])
            .unwrap_or_default()
    };

    // Housekeeping that may leave the main loop: reporting and saving touch no
    // strings and no console input, so the background task can own them.
    let text_vars = vars.iter().any(|v| v.ty.signal() == SignalType::String);
    let bg_report = tasks.any() && !text_in && table.as_ref().is_none_or(|t| t.texts.is_empty());
    let bg_save = tasks.any() && !text_vars;
    let save = crate::retain::save(vars);
    let report = if debug {
        Some("flp_dbg_loop();")
    } else if stats {
        Some("flp_st_report();")
    } else {
        None
    };
    // Without a background task the reporting gets a task of its own on core 0, so
    // USB traffic never delays the cycle (same restrictions as for `bg_report`).
    let report_task = report.filter(|_| {
        !tasks.any()
            && tasks.threaded()
            && !text_in
            && table.as_ref().is_none_or(|t| t.texts.is_empty())
    });
    let lock = |lines: Vec<String>| -> Vec<String> {
        if lines.is_empty() {
            return lines;
        }
        let mut wrapped = vec!["xSemaphoreTake(flp_bg_lock, portMAX_DELAY);".to_owned()];
        wrapped.extend(lines);
        wrapped.push("xSemaphoreGive(flp_bg_lock);".to_owned());
        wrapped
    };
    if tasks.any() {
        out.push_str("\n// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»\nvoid flp_bg_task(void *) {\n  for (;;) {\n");
        let mut body = String::new();
        if stats {
            body.push_str(
                "flp_st_bg_begin();

",
            );
        }
        let take = lock(tasks.take(program, true));
        let started = !take.is_empty();
        push_indented(&mut body, take.into_iter());
        push_function_body(
            &mut body,
            rendered,
            started,
            |s| &s.looped,
            &|_| Vec::new(),
            &forces,
            &|inst| tasks.is_background(inst),
            &|_| None,
        );
        let publish = lock(tasks.publish(program, true));
        if !publish.is_empty() {
            body.push('\n');
            push_indented(&mut body, publish.into_iter());
        }
        if bg_save && !save.is_empty() {
            body.push('\n');
            push_indented(&mut body, save.iter().cloned());
        }
        if stats {
            body.push_str(
                "
flp_st_bg_end();
",
            );
        }
        if bg_report {
            if let Some(call) = report {
                let _ = writeln!(body, "\n  {call}");
            }
        }
        body.push_str("\n  vTaskDelay(1);\n");
        for line in body.lines() {
            if line.is_empty() {
                out.push('\n');
            } else {
                let _ = writeln!(out, "  {line}");
            }
        }
        out.push_str("  }\n}\n");
    }

    if let Some(call) = report_task {
        let _ = write!(
            out,
            "\n// Отчёт по USB: ядро 0, отдельно от основного цикла\nvoid flp_report_task(void *) {{\n  for (;;) {{\n    {call}\n    vTaskDelay(1);\n  }}\n}}\n"
        );
    }

    out.push_str("\nvoid setup() {\n");
    if tasks.any() {
        out.push_str("  flp_bg_lock = xSemaphoreCreateMutex();\n\n");
    }
    if debug {
        out.push_str("  flp_dbg_setup();\n\n");
    } else if stats {
        let _ = writeln!(out, "  Serial.begin({});\n", flp_debug::transport::BAUD);
    }
    let uarts = uart_setup(program, debug || stats);
    if !uarts.is_empty() {
        push_indented(&mut out, uarts.into_iter());
        out.push('\n');
    }
    // String storage is reserved up front: fewer heap reallocations in the loop.
    let strings: Vec<String> = program
        .signals
        .iter()
        .filter(|s| s.ty == SignalType::String)
        .map(|s| s.name.clone())
        .chain(
            vars.iter()
                .filter(|v| v.ty.signal() == SignalType::String)
                .map(|v| flp_core::vars::storage_name(&v.name)),
        )
        .collect();
    if !strings.is_empty() {
        out.push_str("  // Память под строки заранее: меньше дробления кучи.\n");
        for name in &strings {
            let _ = writeln!(out, "  {name}.reserve({STRING_RESERVE});");
        }
        out.push('\n');
    }
    // Retained variables are restored before any block uses them.
    let restore = crate::retain::setup(vars);
    let after_restore = !restore.is_empty();
    push_indented(&mut out, restore.into_iter());
    let once_setup = shared(rendered, |s| &s.once_setup);
    for (i, (kind, code)) in once_setup.iter().enumerate() {
        if i > 0 || after_restore {
            out.push('\n');
        }
        let _ = writeln!(out, "  {}", shared_comment(kind));
        push_indented(&mut out, code.lines().map(str::to_owned));
    }
    let expander_setup = crate::expander::setup(program);
    if !expander_setup.is_empty() {
        if after_restore || !once_setup.is_empty() {
            out.push('\n');
        }
        out.push_str("  // Расширители: шина и первая запись\n");
        push_indented(&mut out, expander_setup.iter().cloned());
    }
    let after_shared = after_restore || !once_setup.is_empty() || !expander_setup.is_empty();
    let none = |_: &Instance| Vec::new();
    push_function_body(
        &mut out,
        rendered,
        after_shared,
        |s| &s.setup,
        &none,
        &none,
        &|_| true,
        &|_| None,
    );
    if tasks.any() {
        out.push_str(
            "\n  // Фоновая задача стартует после настройки всех блоков\n  xTaskCreatePinnedToCore(flp_bg_task, \"flp_bg\", 10240, nullptr, 1, nullptr, 0);\n",
        );
    }
    if report_task.is_some() {
        out.push_str(
            "\n  // Отчёт стартует после настройки всех блоков\n  xTaskCreatePinnedToCore(flp_report_task, \"flp_report\", 8192, nullptr, 1, nullptr, 0);\n",
        );
    }
    out.push_str("}\n\nvoid loop() {\n");
    if stats {
        out.push_str("  flp_st_begin();\n\n");
    }
    if let Some(table) = &table {
        let vars: Vec<String> = (0..table.entries.len())
            .filter(|&i| matches!(table.entries[i].source, flp_debug::Source::Var(_)))
            .map(|i| format!("flp_dbg_force({i});"))
            .collect();
        if !vars.is_empty() {
            let _ = writeln!(out, "  // Forced variables\n  {}\n", vars.join(" "));
        }
    }
    let mut take = crate::expander::poll(program);
    let prologue = tasks.gate_prologue();
    if !prologue.is_empty() {
        if !take.is_empty() {
            take.push(String::new());
        }
        take.extend(prologue);
        take.push(String::new());
    }
    take.extend(lock(tasks.take(program, false)));
    let started = !take.is_empty();
    push_indented(&mut out, take.into_iter());
    push_function_body(
        &mut out,
        rendered,
        started,
        |s| &s.looped,
        &mark,
        &forces,
        &|inst| !tasks.is_background(inst),
        &|inst| tasks.gate(program, inst),
    );
    let flush = crate::expander::flush(program);
    if !flush.is_empty() {
        out.push('\n');
        push_indented(&mut out, flush.into_iter());
    }
    let publish = lock(tasks.publish(program, false));
    if !publish.is_empty() {
        out.push('\n');
        push_indented(&mut out, publish.into_iter());
    }
    if !(bg_save || save.is_empty()) {
        out.push('\n');
        push_indented(&mut out, save.into_iter());
    }
    if stats {
        out.push_str("\n  flp_st_end();\n");
    }
    if !bg_report && report_task.is_none() {
        if let Some(call) = report {
            let _ = writeln!(out, "  {call}");
        }
    }
    out.push_str("}\n");
    out
}

/// Writes the code of each block into a function body; `after` means the body
/// already has code, so the first block is separated by a blank line too.
/// `before` and `extra` add lines in front of and after the block code;
/// `only` selects the blocks; `gate` names a flag that the code of a block runs under.
#[allow(clippy::too_many_arguments)]
fn push_function_body(
    out: &mut String,
    rendered: &[(&Instance, Sections)],
    after: bool,
    section: fn(&Sections) -> &String,
    before: &dyn Fn(&Instance) -> Vec<String>,
    extra: &dyn Fn(&Instance) -> Vec<String>,
    only: &dyn Fn(&Instance) -> bool,
    gate: &dyn Fn(&Instance) -> Option<String>,
) {
    let mut first = !after;
    for (inst, sec) in rendered {
        if !only(inst) {
            continue;
        }
        let code = section(sec);
        let extra = extra(inst);
        if code.is_empty() && extra.is_empty() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        for line in before(inst) {
            let _ = writeln!(out, "  {line}");
        }
        let _ = writeln!(out, "  {}", block_comment(inst));
        match gate(inst).filter(|_| !code.is_empty()) {
            Some(run) => {
                let _ = writeln!(out, "  if ({run}) {{");
                for line in code.lines() {
                    if line.is_empty() {
                        out.push('\n');
                    } else {
                        let _ = writeln!(out, "    {line}");
                    }
                }
                out.push_str("  }\n");
                push_indented(out, extra.into_iter());
            }
            None => push_indented(out, code.lines().map(str::to_owned).chain(extra)),
        }
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

/// Non-empty `once` sections, without repeats of the same text (from one block type
/// or from several: chips on one I²C bus start `Wire` with the same call), in order
/// of first use. The block type is the one that came first.
fn shared<'a>(
    rendered: &'a [(&Instance, Sections)],
    section: fn(&Sections) -> &String,
) -> Vec<(&'a str, &'a str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for (inst, sec) in rendered {
        let text = section(sec).as_str();
        if !text.is_empty() && !out.iter().any(|(_, seen)| *seen == text) {
            out.push((inst.block.kind.as_str(), text));
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
    fn hand_written_includes_move_out_of_sections() {
        let s = split_sections(
            "// @global\n#include <Adafruit_ADS1X16.h>\nint x;\n// @functions once\n\
             #include <Wire.h>\nvoid g() {}\n// @loop\ny();\n",
        )
        .unwrap();
        assert_eq!(s.global, "int x;");
        assert_eq!(s.once_functions, "void g() {}");
        assert_eq!(
            s.includes,
            ["#include <Adafruit_ADS1X16.h>", "#include <Wire.h>"]
        );
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
