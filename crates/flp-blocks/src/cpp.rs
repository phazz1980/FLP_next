//! Firmware side of a built-in block: the Arduino C++ it adds to the sketch.

use std::collections::{BTreeMap, BTreeSet};

/// Parameter as a plain value (durations are milliseconds), for decisions taken
/// while generating.
#[derive(Debug, Clone, PartialEq)]
pub enum Raw {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

/// What a block sees while its code is generated: the same maps a template gets.
#[derive(Debug, Default, Clone)]
pub struct CppCtx {
    /// Block id as a C identifier, the prefix of its variables.
    pub id: String,
    /// C++ expression read by each input port.
    pub input: BTreeMap<String, String>,
    /// Name of the variable each output port writes.
    pub output: BTreeMap<String, String>,
    /// C++ literal (or expression, when the parameter is bound to a variable or an input).
    pub param: BTreeMap<String, String>,
    /// Parameters that are evaluated at run time: an integer literal suffix would be wrong.
    pub param_bound: BTreeSet<String>,
    pub value: BTreeMap<String, Raw>,
    /// Real type name (`bool`, `int`, `float`, `string`) of what feeds each input.
    pub input_type: BTreeMap<String, String>,
}

impl CppCtx {
    pub fn input(&self, port: &str) -> &str {
        self.input.get(port).map_or("", String::as_str)
    }

    pub fn output(&self, port: &str) -> &str {
        self.output.get(port).map_or("", String::as_str)
    }

    pub fn param(&self, name: &str) -> &str {
        self.param.get(name).map_or("", String::as_str)
    }

    /// A duration or unsigned parameter with the `UL` suffix when it is a literal.
    pub fn param_ul(&self, name: &str) -> String {
        let p = self.param(name);
        if self.param_bound.contains(name) {
            p.to_owned()
        } else {
            format!("{p}UL")
        }
    }

    /// A boolean parameter: a literal `true` / `false`, or an expression evaluated at run
    /// time (a variable, an input), which counts as set while generating.
    pub fn truthy(&self, name: &str) -> bool {
        !matches!(self.param(name), "" | "false")
    }

    /// Real type of the source of an input.
    pub fn input_type(&self, port: &str) -> &str {
        self.input_type.get(port).map_or("", String::as_str)
    }

    /// A numeric parameter as the plain value taken while generating.
    pub fn num(&self, name: &str) -> f64 {
        match self.value.get(name) {
            Some(Raw::Int(v)) => *v as f64,
            Some(Raw::Float(v)) => *v,
            _ => 0.0,
        }
    }

    pub fn int(&self, name: &str, default: i64) -> i64 {
        match self.value.get(name) {
            Some(Raw::Int(v)) => *v,
            _ => default,
        }
    }
}

/// Code of one block split into sketch sections (see the template sections `// @global` etc.).
/// Fragments carry no leading or trailing blank lines.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CppParts {
    pub defines: String,
    pub global: String,
    pub setup: String,
    pub looped: String,
    pub functions: String,
    pub once_global: String,
    pub once_setup: String,
    pub once_functions: String,
    pub includes: Vec<String>,
}

/// Generates C++ lines: each `line` ends with a newline, `finish` trims the tail.
#[derive(Default)]
pub(crate) struct Lines(String);

impl Lines {
    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str(text.as_ref());
        self.0.push('\n');
        self
    }

    pub fn finish(self) -> String {
        self.0.trim_matches('\n').to_owned()
    }
}

/// `fn(&CppCtx)` of the blocks that are implemented here.
pub(crate) type Emit = fn(&CppCtx) -> CppParts;

pub(crate) fn add(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = {} + {};",
        e.output("out"),
        e.input("a"),
        e.input("b")
    ))
}

pub(crate) fn not(e: &CppCtx) -> CppParts {
    looped(format!("{} = !{};", e.output("out"), e.input("in")))
}

fn looped(code: String) -> CppParts {
    CppParts {
        looped: code,
        ..Default::default()
    }
}

pub(crate) fn timer_on(e: &CppCtx) -> CppParts {
    let (id, out) = (&e.id, e.output("out"));
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_running = false;"))
        .line(format!("unsigned long {id}_start = 0;"));
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("in")))
        .line(format!("  if (!{id}_running) {{"))
        .line(format!("    {id}_running = true;"))
        .line(format!("    {id}_start = millis();"))
        .line("  }")
        .line(format!(
            "  {out} = millis() - {id}_start >= {};",
            e.param_ul("time")
        ))
        .line("} else {")
        .line(format!("  {id}_running = false;"))
        .line(format!("  {out} = false;"))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn counter(e: &CppCtx) -> CppParts {
    let (id, count) = (&e.id, e.output("count"));
    let (up, down) = (e.input("up"), e.input("down"));
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_up_prev = false;"))
        .line(format!("bool {id}_down_prev = false;"));
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("reset")))
        .line(format!("  {count} = 0;"))
        .line("} else {")
        .line(format!("  if ({up} && !{id}_up_prev) {{"))
        .line(format!("    {count}++;"))
        .line("  }")
        .line(format!("  if ({down} && !{id}_down_prev) {{"))
        .line(format!("    {count}--;"))
        .line("  }")
        .line("}")
        .line(format!("{id}_up_prev = {up};"))
        .line(format!("{id}_down_prev = {down};"));
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

/// `a <op> b <op> c ...` over the first `count` inputs `a`..`p`.
fn chain(e: &CppCtx, op: &str) -> CppParts {
    const PORTS: [&str; 16] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
    ];
    let count = e.int("count", 2).clamp(0, 16) as usize;
    let terms: Vec<&str> = PORTS[..count].iter().map(|p| e.input(p)).collect();
    looped(format!("{} = {};", e.output("out"), terms.join(op)))
}

pub(crate) fn and(e: &CppCtx) -> CppParts {
    chain(e, " && ")
}

pub(crate) fn or(e: &CppCtx) -> CppParts {
    chain(e, " || ")
}

pub(crate) fn xor(e: &CppCtx) -> CppParts {
    chain(e, " != ")
}

pub(crate) fn rs_trigger(e: &CppCtx) -> CppParts {
    let q = e.output("q");
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("r")))
        .line(format!("  {q} = false;"))
        .line(format!("}} else if ({}) {{", e.input("s")))
        .line(format!("  {q} = true;"))
        .line("}");
    looped(l.finish())
}

pub(crate) fn sr_trigger(e: &CppCtx) -> CppParts {
    let out = e.output("out");
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("set")))
        .line(format!("  {out} = true;"))
        .line(format!("}} else if ({}) {{", e.input("reset")))
        .line(format!("  {out} = false;"))
        .line("}");
    looped(l.finish())
}

/// A block whose only state is the previous value of its input.
fn with_prev(e: &CppCtx, looped: String) -> CppParts {
    CppParts {
        global: format!("bool {}_prev = false;", e.id),
        looped,
        ..Default::default()
    }
}

pub(crate) fn t_trigger(e: &CppCtx) -> CppParts {
    let (id, out, input) = (&e.id, e.output("out"), e.input("in"));
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("reset")))
        .line(format!("  {out} = false;"))
        .line(format!("}} else if ({input} && !{id}_prev) {{"))
        .line(format!("  {out} = !{out};"))
        .line("}")
        .line(format!("{id}_prev = {input};"));
    with_prev(e, l.finish())
}

pub(crate) fn rising_edge(e: &CppCtx) -> CppParts {
    let (id, input) = (&e.id, e.input("in"));
    let mut l = Lines::default();
    l.line(format!("{} = {input} && !{id}_prev;", e.output("out")))
        .line(format!("{id}_prev = {input};"));
    with_prev(e, l.finish())
}

pub(crate) fn falling_edge(e: &CppCtx) -> CppParts {
    let (id, input) = (&e.id, e.input("in"));
    let mut l = Lines::default();
    l.line(format!("{} = !{input} && {id}_prev;", e.output("out")))
        .line(format!("{id}_prev = {input};"));
    with_prev(e, l.finish())
}

pub(crate) fn timer_off(e: &CppCtx) -> CppParts {
    let (id, out) = (&e.id, e.output("out"));
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_on = false;"))
        .line(format!("unsigned long {id}_off_at = 0;"));
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("in")))
        .line(format!("  {id}_on = true;"))
        .line(format!("  {out} = true;"))
        .line("} else {")
        .line(format!("  if ({id}_on) {{"))
        .line(format!("    {id}_on = false;"))
        .line(format!("    {id}_off_at = millis();"))
        .line("  }")
        .line(format!(
            "  {out} = {out} && millis() - {id}_off_at < {};",
            e.param_ul("time")
        ))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn pulse(e: &CppCtx) -> CppParts {
    let (id, input) = (&e.id, e.input("in"));
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_prev = false;"))
        .line(format!("bool {id}_active = false;"))
        .line(format!("unsigned long {id}_start = 0;"));
    let mut l = Lines::default();
    l.line(format!("if ({input} && !{id}_prev && !{id}_active) {{"))
        .line(format!("  {id}_active = true;"))
        .line(format!("  {id}_start = millis();"))
        .line("}")
        .line(format!("{id}_prev = {input};"))
        .line(format!(
            "if ({id}_active && millis() - {id}_start >= {}) {{",
            e.param_ul("time")
        ))
        .line(format!("  {id}_active = false;"))
        .line("}")
        .line(format!("{} = {id}_active;", e.output("out")));
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn generator(e: &CppCtx) -> CppParts {
    let (id, out) = (&e.id, e.output("out"));
    let period = e.int("time_on", 0) + e.int("time_off", 0);
    let mut l = Lines::default();
    l.line(format!("if ({}) {{", e.input("en")))
        .line(format!(
            "  unsigned long phase = (millis() - {id}_start) % ({period}UL);"
        ))
        .line(format!("  {out} = phase < {};", e.param_ul("time_on")))
        .line("} else {")
        .line(format!("  {id}_start = millis();"))
        .line(format!("  {out} = false;"))
        .line("}");
    CppParts {
        global: format!("unsigned long {id}_start = 0;"),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn debounce(e: &CppCtx) -> CppParts {
    let (id, input) = (&e.id, e.input("in"));
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_last = false;"))
        .line(format!("unsigned long {id}_changed = 0;"));
    let mut l = Lines::default();
    l.line(format!("if ({input} != {id}_last) {{"))
        .line(format!("  {id}_last = {input};"))
        .line(format!("  {id}_changed = millis();"))
        .line("}")
        .line(format!(
            "if (millis() - {id}_changed >= {}) {{",
            e.param_ul("time")
        ))
        .line(format!("  {} = {id}_last;", e.output("out")))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn hysteresis(e: &CppCtx) -> CppParts {
    let (input, out) = (e.input("in"), e.output("out"));
    let (on, off) = (e.param("on"), e.param("off"));
    // Which side switches on depends on the order of the thresholds.
    let (on_op, off_op) = if e.num("on") >= e.num("off") {
        (">=", "<=")
    } else {
        ("<=", ">=")
    };
    let mut l = Lines::default();
    l.line(format!("if ({input} {on_op} {on}) {{"))
        .line(format!("  {out} = true;"))
        .line(format!("}} else if ({input} {off_op} {off}) {{"))
        .line(format!("  {out} = false;"))
        .line("}");
    looped(l.finish())
}

pub(crate) fn compare(e: &CppCtx) -> CppParts {
    const OPS: [&str; 6] = ["==", "!=", "<", "<=", ">", ">="];
    let op = OPS[usize::try_from(e.int("op", 0))
        .unwrap_or(0)
        .min(OPS.len() - 1)];
    binary(e, op)
}

/// `out = a > <param b>;`
fn against_param(e: &CppCtx, op: &str) -> CppParts {
    looped(format!(
        "{} = {} {op} {};",
        e.output("out"),
        e.input("a"),
        e.param("b")
    ))
}

pub(crate) fn greater(e: &CppCtx) -> CppParts {
    against_param(e, ">")
}

pub(crate) fn less(e: &CppCtx) -> CppParts {
    against_param(e, "<")
}

/// `out = a <op> b;`
fn binary(e: &CppCtx, op: &str) -> CppParts {
    looped(format!(
        "{} = {} {op} {};",
        e.output("out"),
        e.input("a"),
        e.input("b")
    ))
}

pub(crate) fn sub(e: &CppCtx) -> CppParts {
    binary(e, "-")
}

pub(crate) fn mul(e: &CppCtx) -> CppParts {
    binary(e, "*")
}

pub(crate) fn div(e: &CppCtx) -> CppParts {
    let (a, b) = (e.input("a"), e.input("b"));
    looped(format!(
        "{} = {b} != 0.0f ? {a} / {b} : 0.0f;",
        e.output("out")
    ))
}

pub(crate) fn modulo(e: &CppCtx) -> CppParts {
    let (a, b, out) = (e.input("a"), e.input("b"), e.output("out"));
    if e.input_type("a") == "int" && e.input_type("b") == "int" {
        looped(format!("{out} = {b} != 0 ? {a} % {b} : 0;"))
    } else {
        looped(format!("{out} = {b} != 0.0f ? fmodf({a}, {b}) : 0.0f;"))
    }
}

pub(crate) fn min(e: &CppCtx) -> CppParts {
    let (a, b) = (e.input("a"), e.input("b"));
    looped(format!("{} = {a} < {b} ? {a} : {b};", e.output("out")))
}

pub(crate) fn max(e: &CppCtx) -> CppParts {
    let (a, b) = (e.input("a"), e.input("b"));
    looped(format!("{} = {a} > {b} ? {a} : {b};", e.output("out")))
}

pub(crate) fn limit(e: &CppCtx) -> CppParts {
    let (x, lo, hi) = (e.input("in"), e.param("lo"), e.param("hi"));
    looped(format!(
        "{} = {x} < {lo} ? {lo} : ({x} > {hi} ? {hi} : {x});",
        e.output("out")
    ))
}

pub(crate) fn scale(e: &CppCtx) -> CppParts {
    let out = e.output("out");
    let (out_min, out_max) = (e.param("out_min"), e.param("out_max"));
    let (in_min, in_max) = (e.param("in_min"), e.param("in_max"));
    let mut l = Lines::default();
    l.line(format!(
        "{out} = {out_min} + ({} - {in_min}) * ({out_max} - {out_min}) / ({in_max} - {in_min});",
        e.input("in")
    ));
    if e.truthy("clamp") {
        if e.num("out_min") <= e.num("out_max") {
            l.line(format!("{out} = constrain({out}, {out_min}, {out_max});"));
        } else {
            l.line(format!("{out} = constrain({out}, {out_max}, {out_min});"));
        }
    }
    looped(l.finish())
}

pub(crate) fn switch(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = {} ? {} : {};",
        e.output("out"),
        e.input("sel"),
        e.input("b"),
        e.input("a")
    ))
}

pub(crate) fn mux(e: &CppCtx) -> CppParts {
    const PORTS: [&str; 16] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
    ];
    let count = e.int("count", 2).clamp(0, 16) as usize;
    let out = e.output("out");
    let mut l = Lines::default();
    l.line("{")
        .line(format!("  int idx = (int){};", e.input("sel")));
    for (i, port) in PORTS[..count].iter().enumerate() {
        if i + 1 == count {
            l.line(format!("  {out} = {};", e.input(port)));
        } else {
            l.line(format!("  if (idx <= {i}) {{"))
                .line(format!("    {out} = {};", e.input(port)))
                .line("  } else");
        }
    }
    l.line("}");
    looped(l.finish())
}

pub(crate) fn constant(e: &CppCtx) -> CppParts {
    CppParts {
        setup: format!("{} = {};", e.output("out"), e.param("value")),
        ..Default::default()
    }
}
