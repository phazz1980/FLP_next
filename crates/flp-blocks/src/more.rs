//! Numbers, bits and words: stateless blocks and the few with a little state.
//!
//! Integer ports are 32-bit as in the firmware; floats are computed in 32 bits, as `sinf` and
//! friends do on the board.

use crate::cpp::{self, CppCtx, CppParts, Lines, Raw};
use crate::{b, Behavior, Ctx, Plain};

/// Deterministic generator shared by the simulation and the PC test of the C++: the same
/// sequence comes out of both (the module prelude of `random` uses the same constants).
pub const RANDOM_SEED: u32 = 2_463_534_242;

pub fn next_random(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn int(v: f64) -> i32 {
    v as i32
}

/// The angle in radians; parameter 0 is the unit (0 — degrees).
fn radians(c: &Ctx) -> f32 {
    let x = c.input(0) as f32;
    if c.param(0) == 0.0 {
        x * 0.017_453_292
    } else {
        x
    }
}

pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    let plain = |f: fn(&Ctx, &mut [f64])| -> Option<Box<dyn Behavior>> { Some(Box::new(Plain(f))) };
    match kind {
        "abs" => plain(|c, o| o[0] = c.input(0).abs()),
        "sq" => plain(|c, o| o[0] = c.input(0) * c.input(0)),
        "sqrt" => plain(|c, o| {
            let x = c.input(0) as f32;
            o[0] = if x > 0.0 { f64::from(x.sqrt()) } else { 0.0 };
        }),
        "pow" => plain(|c, o| o[0] = f64::from((c.input(0) as f32).powf(c.input(1) as f32))),
        "sin" => plain(|c, o| o[0] = f64::from(radians(c).sin())),
        "cos" => plain(|c, o| o[0] = f64::from(radians(c).cos())),
        "tan" => plain(|c, o| o[0] = f64::from(radians(c).tan())),
        "float_to_int" => plain(|c, o| {
            let x = c.input(0);
            o[0] = f64::from(match c.param(0) as i64 {
                0 => x.round() as i32,
                1 => x.floor() as i32,
                2 => x.ceil() as i32,
                _ => x as i32,
            });
        }),
        "int_to_float" => plain(|c, o| o[0] = c.input(0)),
        "random" => Some(Box::new(Random {
            prev: false,
            state: RANDOM_SEED,
        })),
        "bit_and" => plain(|c, o| o[0] = f64::from(int(c.input(0)) & int(c.input(1)))),
        "bit_or" => plain(|c, o| o[0] = f64::from(int(c.input(0)) | int(c.input(1)))),
        "bit_xor" => plain(|c, o| o[0] = f64::from(int(c.input(0)) ^ int(c.input(1)))),
        "bit_not" => plain(|c, o| o[0] = f64::from(!int(c.input(0)))),
        "bit_read" => plain(|c, o| o[0] = b(bit(c))),
        "bit_write" => plain(|c, o| {
            let (x, mask) = (int(c.input(0)), 1_i32.wrapping_shl(c.param(0) as u32));
            o[0] = f64::from(if c.flag(1) { x | mask } else { x & !mask });
        }),
        "bit_change" => Some(Box::<BitChange>::default()),
        "bits_to_byte" => plain(|c, o| {
            o[0] = f64::from(
                (0..8)
                    .map(|i| if c.flag(i) { 1 << i } else { 0 })
                    .sum::<i32>(),
            );
        }),
        "byte_to_bits" => plain(|c, o| {
            for (i, v) in o.iter_mut().enumerate().take(8) {
                *v = b((int(c.input(0)) >> i) & 1 != 0);
            }
        }),
        "bytes_to_word" => plain(|c, o| {
            o[0] = f64::from(((int(c.input(1)) & 0xFF) << 8) | (int(c.input(0)) & 0xFF));
        }),
        "word_to_bytes" => plain(|c, o| {
            o[0] = f64::from(int(c.input(0)) & 0xFF);
            o[1] = f64::from((int(c.input(0)) >> 8) & 0xFF);
        }),
        "bytes_to_dword" => plain(|c, o| {
            let v = (0..4).fold(0_u32, |acc, i| {
                acc | (((int(c.input(i)) & 0xFF) as u32) << (8 * i))
            });
            o[0] = f64::from(v as i32);
        }),
        "dword_to_bytes" => plain(|c, o| {
            for (i, v) in o.iter_mut().enumerate().take(4) {
                *v = f64::from(((int(c.input(0)) as u32) >> (8 * i)) & 0xFF);
            }
        }),
        "number_change" => Some(Box::<NumberChange>::default()),
        "string_change" => Some(Box::<StringChange>::default()),
        "pid" => Some(Box::<Pid>::default()),
        "scale_table" => Some(Box::<ScaleTable>::default()),
        _ => None,
    }
}

/// Bit number `param 0` of input 0.
fn bit(c: &Ctx) -> bool {
    int(c.input(0)).wrapping_shr(c.param(0) as u32) & 1 != 0
}

struct Random {
    prev: bool,
    state: u32,
}

impl Behavior for Random {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) && !self.prev {
            let r = (next_random(&mut self.state) % 10_001) as f32 / 10_000.0;
            let (lo, hi) = (ctx.param(0) as f32, ctx.param(1) as f32);
            out[0] = f64::from(lo + (hi - lo) * r);
        }
        self.prev = ctx.flag(0);
    }
}

#[derive(Default)]
struct BitChange {
    prev: bool,
    init: bool,
}

impl Behavior for BitChange {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = bit(ctx);
        out[0] = b(self.init && now != self.prev);
        self.prev = now;
        self.init = true;
    }
}

fn looped(code: String) -> CppParts {
    CppParts {
        looped: code,
        ..Default::default()
    }
}

/// `out = <expr of in>;`
fn unary(e: &CppCtx, f: impl Fn(&str) -> String) -> CppParts {
    looped(format!("{} = {};", e.output("out"), f(e.input("in"))))
}

fn binary(e: &CppCtx, op: &str) -> CppParts {
    looped(format!(
        "{} = {} {op} {};",
        e.output("out"),
        e.input("a"),
        e.input("b")
    ))
}

pub(crate) fn abs(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("fabs({x})"))
}

pub(crate) fn sq(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("{x} * {x}"))
}

pub(crate) fn sqrt(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("{x} > 0.0f ? sqrtf({x}) : 0.0f"))
}

pub(crate) fn pow(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = powf({}, {});",
        e.output("out"),
        e.input("a"),
        e.input("b")
    ))
}

/// `sinf(x)` with the degrees factor when the unit is degrees.
fn trig(e: &CppCtx, func: &str) -> CppParts {
    let factor = if matches!(e.value.get("unit"), Some(Raw::Int(0))) {
        " * 0.017453292f"
    } else {
        ""
    };
    unary(e, |x| format!("{func}({x}{factor})"))
}

pub(crate) fn sin(e: &CppCtx) -> CppParts {
    trig(e, "sinf")
}

pub(crate) fn cos(e: &CppCtx) -> CppParts {
    trig(e, "cosf")
}

pub(crate) fn tan(e: &CppCtx) -> CppParts {
    trig(e, "tanf")
}

pub(crate) fn float_to_int(e: &CppCtx) -> CppParts {
    let round = match e.value.get("mode") {
        Some(Raw::Int(0)) => "roundf",
        Some(Raw::Int(1)) => "floorf",
        Some(Raw::Int(2)) => "ceilf",
        _ => "",
    };
    if matches!(e.input_type("in"), "int" | "bool") {
        return unary(e, |x| x.to_owned());
    }
    let parts = unary(e, |x| {
        if round.is_empty() {
            format!("flp_f2i({x})")
        } else {
            format!("flp_f2i({round}({x}))")
        }
    });
    cpp::with_functions(parts, cpp::F2I)
}

pub(crate) fn int_to_float(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("(float){x}"))
}

pub(crate) fn random(e: &CppCtx) -> CppParts {
    let (id, en) = (&e.id, e.input("en"));
    let (lo, hi) = (e.param("min"), e.param("max"));
    let mut l = Lines::default();
    l.line(format!("if ({en} && !{id}_prev) {{"))
        .line(format!(
            "  {} = {lo} + ({hi} - {lo}) * (random(0, 10001) / 10000.0f);",
            e.output("out")
        ))
        .line("}")
        .line(format!("{id}_prev = {en};"));
    CppParts {
        global: format!("bool {id}_prev = false;"),
        once_setup: "randomSeed(esp_random());".to_owned(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn bit_and(e: &CppCtx) -> CppParts {
    binary(e, "&")
}

pub(crate) fn bit_or(e: &CppCtx) -> CppParts {
    binary(e, "|")
}

pub(crate) fn bit_xor(e: &CppCtx) -> CppParts {
    binary(e, "^")
}

pub(crate) fn bit_not(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("~{x}"))
}

pub(crate) fn bit_read(e: &CppCtx) -> CppParts {
    unary(e, |x| format!("(({x} >> {}) & 1) != 0", e.param("bit")))
}

pub(crate) fn bit_write(e: &CppCtx) -> CppParts {
    let (x, bit) = (e.input("in"), e.param("bit"));
    looped(format!(
        "{} = {} ? ({x} | (1 << {bit})) : ({x} & ~(1 << {bit}));",
        e.output("out"),
        e.input("value")
    ))
}

pub(crate) fn bit_change(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let mut global = Lines::default();
    global
        .line(format!("bool {id}_prev = false;"))
        .line(format!("bool {id}_init = false;"));
    let mut l = Lines::default();
    l.line("{")
        .line(format!(
            "  bool now = (({} >> {}) & 1) != 0;",
            e.input("in"),
            e.param("bit")
        ))
        .line(format!(
            "  {} = {id}_init && now != {id}_prev;",
            e.output("out")
        ))
        .line(format!("  {id}_prev = now;"))
        .line(format!("  {id}_init = true;"))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn bits_to_byte(e: &CppCtx) -> CppParts {
    let terms: Vec<String> = (0..8)
        .map(|i| format!("({} ? {} : 0)", e.input(&format!("b{i}")), 1 << i))
        .collect();
    looped(format!("{} = {};", e.output("out"), terms.join(" | ")))
}

pub(crate) fn byte_to_bits(e: &CppCtx) -> CppParts {
    let mut l = Lines::default();
    for i in 0..8 {
        l.line(format!(
            "{} = (({} >> {i}) & 1) != 0;",
            e.output(&format!("b{i}")),
            e.input("in")
        ));
    }
    looped(l.finish())
}

pub(crate) fn bytes_to_word(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = (({} & 0xFF) << 8) | ({} & 0xFF);",
        e.output("out"),
        e.input("hi"),
        e.input("lo")
    ))
}

pub(crate) fn word_to_bytes(e: &CppCtx) -> CppParts {
    let x = e.input("in");
    let mut l = Lines::default();
    l.line(format!("{} = {x} & 0xFF;", e.output("lo")))
        .line(format!("{} = ({x} >> 8) & 0xFF;", e.output("hi")));
    looped(l.finish())
}

pub(crate) fn bytes_to_dword(e: &CppCtx) -> CppParts {
    let term = |i: u32| {
        let part = format!("(uint32_t)({} & 0xFF)", e.input(&format!("b{i}")));
        match i {
            0 => part,
            _ => format!("({part} << {})", 8 * i),
        }
    };
    let terms: Vec<String> = (0..4).rev().map(term).collect();
    looped(format!(
        "{} = (int32_t)({});",
        e.output("out"),
        terms.join(" | ")
    ))
}

pub(crate) fn dword_to_bytes(e: &CppCtx) -> CppParts {
    let mut l = Lines::default();
    for i in 0..4 {
        l.line(format!(
            "{} = ((uint32_t){} >> {}) & 0xFF;",
            e.output(&format!("b{i}")),
            e.input("in"),
            8 * i
        ));
    }
    looped(l.finish())
}

#[derive(Default)]
struct NumberChange {
    prev: f64,
    init: bool,
}

impl Behavior for NumberChange {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let x = ctx.input(0);
        if self.init {
            out[0] = b((x - self.prev).abs() > ctx.param(0));
            if out[0] != 0.0 {
                self.prev = x;
            }
        } else {
            self.init = true;
            self.prev = x;
            out[0] = 0.0;
        }
    }
}

#[derive(Default)]
struct StringChange {
    prev: String,
    init: bool,
}

impl Behavior for StringChange {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}

    fn step_text(&mut self, ctx: &Ctx, out: &mut [f64], _: &mut [String]) {
        out[0] = b(self.init && ctx.text(0) != self.prev);
        self.prev = ctx.text(0).to_owned();
        self.init = true;
    }
}

/// Like Arduino `constrain`: no panic when the limits are swapped.
fn constrain(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[derive(Default)]
struct Pid {
    integral: f64,
    prev_err: f64,
    last_ms: u64,
    running: bool,
}

impl Behavior for Pid {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let (kp, ki, kd) = (ctx.param(0), ctx.param(1), ctx.param(2));
        let (lo, hi) = (ctx.param(3), ctx.param(4));
        if !ctx.flag(2) {
            self.running = false;
            self.integral = 0.0;
            out[0] = lo;
            return;
        }
        let err = ctx.input(0) - ctx.input(1);
        if self.running {
            let dt = (ctx.now_ms - self.last_ms) as f64 / 1000.0;
            if dt > 0.0 {
                self.integral = constrain(self.integral + err * dt * ki, lo, hi);
                let d = (err - self.prev_err) / dt;
                out[0] = constrain(kp * err + self.integral + kd * d, lo, hi);
                self.prev_err = err;
                self.last_ms = ctx.now_ms;
            }
        } else {
            self.running = true;
            self.last_ms = ctx.now_ms;
            self.prev_err = err;
            out[0] = constrain(kp * err, lo, hi);
        }
    }
}

/// `strtof` of the C library, without `inf`, `nan` and hex: the value (0 when there is no
/// number) and how many bytes were used.
fn parse_float(s: &str) -> (f32, usize) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    let start = i;
    if matches!(bytes.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let digits = |mut i: usize| {
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let int_end = digits(i);
    let mut end = int_end;
    let mut has_digits = int_end > i;
    if bytes.get(end) == Some(&b'.') {
        let frac_end = digits(end + 1);
        has_digits |= frac_end > end + 1;
        if has_digits {
            end = frac_end;
        }
    }
    if !has_digits {
        return (0.0, 0);
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exp = end + 1;
        if matches!(bytes.get(exp), Some(b'+' | b'-')) {
            exp += 1;
        }
        let exp_end = digits(exp);
        if exp_end > exp {
            end = exp_end;
        }
    }
    (s[start..end].parse().unwrap_or(0.0), end)
}

/// Points of `"x:y;x:y"`, parsed the way the firmware does it (up to 32 of them).
fn parse_points(text: &str) -> Vec<(f32, f32)> {
    let mut points = Vec::new();
    let mut rest = text;
    while !rest.is_empty() && points.len() < 32 {
        let (x, used) = parse_float(rest);
        if rest.as_bytes().get(used) != Some(&b':') {
            break;
        }
        rest = &rest[used + 1..];
        let (y, used) = parse_float(rest);
        points.push((x, y));
        rest = match rest.as_bytes().get(used) {
            Some(b';') => &rest[used + 1..],
            _ => &rest[used..],
        };
    }
    if points.is_empty() {
        points.push((0.0, 0.0));
    }
    points
}

#[derive(Default)]
struct ScaleTable {
    source: String,
    points: Vec<(f32, f32)>,
}

impl Behavior for ScaleTable {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if self.points.is_empty() || self.source != ctx.param_text(0) {
            self.source = ctx.param_text(0).to_owned();
            self.points = parse_points(&self.source);
        }
        let p = &self.points;
        let v = ctx.input(0) as f32;
        let y = if v <= p[0].0 {
            p[0].1
        } else if v >= p[p.len() - 1].0 {
            p[p.len() - 1].1
        } else {
            let i = p
                .iter()
                .position(|q| v <= q.0)
                .unwrap_or(p.len() - 1)
                .max(1);
            p[i - 1].1 + (p[i].1 - p[i - 1].1) * (v - p[i - 1].0) / (p[i].0 - p[i - 1].0)
        };
        out[0] = f64::from(y);
    }
}

pub(crate) fn number_change(e: &CppCtx) -> CppParts {
    let (id, input, out) = (&e.id, e.input("in"), e.output("out"));
    let mut global = Lines::default();
    global
        .line(format!("float {id}_prev = 0.0f;"))
        .line(format!("bool {id}_init = false;"));
    let mut l = Lines::default();
    l.line(format!("if (!{id}_init) {{"))
        .line(format!("  {id}_init = true;"))
        .line(format!("  {id}_prev = {input};"))
        .line(format!("  {out} = false;"))
        .line("} else {")
        .line(format!("  float diff = {input} - {id}_prev;"))
        .line(format!(
            "  {out} = (diff < 0 ? -diff : diff) > {};",
            e.param("deadband")
        ))
        .line(format!("  if ({out}) {id}_prev = {input};"))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn string_change(e: &CppCtx) -> CppParts {
    let (id, input) = (&e.id, e.input("in"));
    let mut global = Lines::default();
    global
        .line(format!("String {id}_prev;"))
        .line(format!("bool {id}_init = false;"));
    let mut l = Lines::default();
    l.line(format!(
        "{} = {id}_init && {input} != {id}_prev;",
        e.output("out")
    ))
    .line(format!("{id}_prev = {input};"))
    .line(format!("{id}_init = true;"));
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn pid(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let (sp, pv, en) = (e.input("sp"), e.input("pv"), e.input("en"));
    let out = e.output("out");
    let (lo, hi) = (e.param("out_min"), e.param("out_max"));
    let (kp, ki, kd) = (e.param("kp"), e.param("ki"), e.param("kd"));
    let mut global = Lines::default();
    global
        .line(format!("float {id}_i = 0.0f;"))
        .line(format!("float {id}_prev_err = 0.0f;"))
        .line(format!("unsigned long {id}_t = 0;"))
        .line(format!("bool {id}_run = false;"));
    let mut l = Lines::default();
    l.line(format!("if ({en}) {{"))
        .line("  unsigned long now = millis();")
        .line(format!("  float err = {sp} - {pv};"))
        .line(format!("  if ({id}_run) {{"))
        .line(format!("    float dt = (now - {id}_t) / 1000.0f;"))
        .line("    if (dt > 0.0f) {")
        .line(format!("      {id}_i += err * dt * {ki};"))
        .line(format!("      {id}_i = constrain({id}_i, {lo}, {hi});"))
        .line(format!("      float d = (err - {id}_prev_err) / dt;"))
        .line(format!(
            "      {out} = constrain({kp} * err + {id}_i + {kd} * d, {lo}, {hi});"
        ))
        .line(format!("      {id}_prev_err = err;"))
        .line(format!("      {id}_t = now;"))
        .line("    }")
        .line("  } else {")
        .line(format!("    {id}_run = true;"))
        .line(format!("    {id}_t = now;"))
        .line(format!("    {id}_prev_err = err;"))
        .line(format!("    {out} = constrain({kp} * err, {lo}, {hi});"))
        .line("  }")
        .line("} else {")
        .line(format!("  {id}_run = false;"))
        .line(format!("  {id}_i = 0.0f;"))
        .line(format!("  {out} = {lo};"))
        .line("}");
    CppParts {
        global: global.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

const SCALE_TABLE: &str = r#"{
  // points are parsed from the "x:y;x:y" string once
  static float xs[32], ys[32];
  static int n = 0;
  if (n == 0) {
    const char *p = "@POINTS@";
    while (*p && n < 32) {
      char *end;
      xs[n] = strtof(p, &end);
      if (*end != ':') break;
      p = end + 1;
      ys[n] = strtof(p, &end);
      n++;
      p = (*end == ';') ? end + 1 : end;
    }
    if (n == 0) { xs[0] = 0; ys[0] = 0; n = 1; }
  }
  float v = @IN@;
  if (v <= xs[0]) {
    @OUT@ = ys[0];
  } else if (v >= xs[n - 1]) {
    @OUT@ = ys[n - 1];
  } else {
    int i = 1;
    while (v > xs[i]) i++;
    @OUT@ = ys[i - 1] + (ys[i] - ys[i - 1]) * (v - xs[i - 1]) / (xs[i] - xs[i - 1]);
  }
}"#;

pub(crate) fn scale_table(e: &CppCtx) -> CppParts {
    let points = match e.value.get("points") {
        Some(Raw::Text(s)) => s.as_str(),
        _ => "",
    };
    looped(
        SCALE_TABLE
            .replace("@POINTS@", points)
            .replace("@IN@", e.input("in"))
            .replace("@OUT@", e.output("out")),
    )
}
