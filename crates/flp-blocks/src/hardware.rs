//! Firmware code of the simple built-in blocks with hardware: pins, sensors, flash and the
//! PCF8574 expanders. Those that are plain logic (`dms`, `decoder`, `encoder`, `flash_store`)
//! are simulated; the rest have no simulation.

use crate::cpp::{CppCtx, CppParts, Lines};
use crate::{b, Behavior, Ctx};

/// Behaviour of a block of this file, or `None` when it has no simulation.
pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    match kind {
        "dms" => Some(Box::new(Dms)),
        "decoder" => Some(Box::new(Decoder)),
        "encoder" => Some(Box::new(Encoder)),
        "flash_store" => Some(Box::<FlashStore>::default()),
        "cycle_report" => Some(Box::new(CycleReport)),
        _ => None,
    }
}

/// Demultiplexer: `in` goes to the output chosen by `sel`, the others are zero (empty).
struct Dms;

impl Behavior for Dms {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let idx = c.input(0) as i64;
        let count = (c.param(0) as usize).clamp(0, 16).min(out.len());
        for (i, o) in out.iter_mut().take(count).enumerate() {
            *o = if idx == i as i64 { c.input(1) } else { 0.0 };
        }
    }

    fn step_text(&mut self, c: &Ctx, _: &mut [f64], text: &mut [String]) {
        let idx = c.input(0) as i64;
        let count = (c.param(0) as usize).clamp(0, 16).min(text.len());
        for (i, t) in text.iter_mut().take(count).enumerate() {
            *t = if idx == i as i64 {
                c.text(1).to_owned()
            } else {
                String::new()
            };
        }
    }
}

/// Decoder: output `o<n>` is set while `in` equals `n`.
struct Decoder;

impl Behavior for Decoder {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let count = (c.param(0) as usize).clamp(0, 16).min(out.len());
        for (i, o) in out.iter_mut().take(count).enumerate() {
            *o = b(c.input(0) == i as f64);
        }
    }
}

/// Encoder: the lowest set input gives `out`, `any` tells that some input is set.
struct Encoder;

impl Behavior for Encoder {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let count = (c.param(0) as usize).clamp(0, 16);
        let first = (0..count).find(|i| c.flag(*i));
        out[0] = first.map_or(0.0, |i| i as f64);
        out[1] = b(first.is_some());
    }
}

/// Value kept in flash: loaded at start, saved on the rising edge of `save`.
#[derive(Default)]
struct FlashStore {
    started: bool,
    prev: bool,
}

impl Behavior for FlashStore {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        if !self.started {
            self.started = true;
            out[0] = c.param(0);
        }
        if c.flag(1) && !self.prev && c.input(0) != out[0] {
            out[0] = c.input(0);
        }
        self.prev = c.flag(1);
    }
}

/// Only reports to the debugger; nothing to compute.
struct CycleReport;

impl Behavior for CycleReport {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}
}

fn looped(code: String) -> CppParts {
    CppParts {
        looped: code,
        ..Default::default()
    }
}

/// `!` when the boolean parameter is set.
fn not_if(e: &CppCtx, name: &str) -> &'static str {
    if e.truthy(name) {
        "!"
    } else {
        ""
    }
}

/// `if (<en>) ` before a write that waits for the enable input.
fn guard(e: &CppCtx) -> String {
    if e.truthy("write_enable") {
        format!("if ({}) ", e.input("en"))
    } else {
        String::new()
    }
}

pub(crate) fn digital_in(e: &CppCtx) -> CppParts {
    let (out, pin, inv) = (e.output("out"), e.param("pin"), not_if(e, "inverted"));
    if let Some((input, _)) = &e.expander {
        return looped(format!("{out} = {inv}(({input} >> {pin}) & 1);"));
    }
    let mode = if e.truthy("pullup") {
        "INPUT_PULLUP"
    } else {
        "INPUT"
    };
    CppParts {
        setup: format!("pinMode({pin}, {mode});"),
        looped: format!("{out} = {inv}digitalRead({pin});"),
        ..Default::default()
    }
}

pub(crate) fn digital_out(e: &CppCtx) -> CppParts {
    let (pin, input, inv) = (e.param("pin"), e.input("in"), not_if(e, "inverted"));
    let guard = guard(e);
    if let Some((_, output)) = &e.expander {
        return looped(format!("{guard}bitWrite({output}, {pin}, {inv}({input}));"));
    }
    CppParts {
        setup: format!("pinMode({pin}, OUTPUT);"),
        looped: format!("{guard}digitalWrite({pin}, {inv}({input}) ? HIGH : LOW);"),
        ..Default::default()
    }
}

pub(crate) fn analog_in(e: &CppCtx) -> CppParts {
    looped(format!(
        "{} = analogRead({});",
        e.output("out"),
        e.param("pin")
    ))
}

pub(crate) fn pwm_out(e: &CppCtx) -> CppParts {
    looped(format!(
        "{}analogWrite({}, (int)(constrain({}, 0.0f, 100.0f) * 2.55f + 0.5f));",
        guard(e),
        e.param("pin"),
        e.input("in")
    ))
}

pub(crate) fn dht22(e: &CppCtx) -> CppParts {
    let (id, ok) = (&e.id, e.output("ok"));
    let mut global = Lines::default();
    global
        .line(format!("DHT {id}_dht({}, DHT22);", e.param("pin")))
        .line(format!("unsigned long {id}_last = 0;"));
    let mut l = Lines::default();
    l.line(format!(
        "if (millis() - {id}_last >= {}) {{",
        e.param_ul("period")
    ))
    .line(format!("  {id}_last = millis();"))
    .line(format!("  float t = {id}_dht.readTemperature();"))
    .line(format!("  float h = {id}_dht.readHumidity();"))
    .line(format!("  {ok} = !isnan(t) && !isnan(h);"))
    .line(format!("  if ({ok}) {{"))
    .line(format!("    {} = t;", e.output("temp")))
    .line(format!("    {} = h;", e.output("humidity")))
    .line("  }")
    .line("}");
    CppParts {
        global: global.finish(),
        setup: format!("{id}_dht.begin();"),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn ds18b20(e: &CppCtx) -> CppParts {
    let (id, ok) = (&e.id, e.output("ok"));
    let mut global = Lines::default();
    global
        .line(format!("OneWire {id}_wire({});", e.param("pin")))
        .line(format!("DallasTemperature {id}_bus(&{id}_wire);"))
        .line(format!("unsigned long {id}_requested = 0;"));
    let mut setup = Lines::default();
    setup
        .line(format!("{id}_bus.begin();"))
        .line("// Do not block the scan for 750 ms: request now, read on a later scan.")
        .line(format!("{id}_bus.setWaitForConversion(false);"))
        .line(format!("{id}_bus.requestTemperatures();"))
        .line(format!("{id}_requested = millis();"));
    let mut l = Lines::default();
    l.line(format!(
        "if (millis() - {id}_requested >= {}) {{",
        e.param_ul("period")
    ))
    .line(format!(
        "  float t = {id}_bus.getTempCByIndex({});",
        e.param("index")
    ))
    .line(format!("  {ok} = t != DEVICE_DISCONNECTED_C;"))
    .line(format!("  if ({ok}) {{"))
    .line(format!("    {} = t;", e.output("temp")))
    .line("  }")
    .line(format!("  {id}_bus.requestTemperatures();"))
    .line(format!("  {id}_requested = millis();"))
    .line("}");
    CppParts {
        global: global.finish(),
        setup: setup.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn flash_store(e: &CppCtx) -> CppParts {
    let (id, out) = (&e.id, e.output("out"));
    let (save, input) = (e.input("save"), e.input("in"));
    // Preferences keys are 15 characters at most.
    let key: String = e.id.chars().take(15).collect();
    let mut global = Lines::default();
    global
        .line(format!("Preferences {id}_prefs;"))
        .line(format!("bool {id}_prev = false;"));
    let mut setup = Lines::default();
    setup
        .line(format!("{id}_prefs.begin(\"flp\", false);"))
        .line(format!(
            "{out} = {id}_prefs.getFloat(\"{key}\", {});",
            e.param("initial")
        ));
    let mut l = Lines::default();
    l.line("// Flash wears out: write only a value that differs from the saved one.")
        .line(format!("if ({save} && !{id}_prev && {input} != {out}) {{"))
        .line(format!("  {out} = {input};"))
        .line(format!("  {id}_prefs.putFloat(\"{key}\", {out});"))
        .line("}")
        .line(format!("{id}_prev = {save};"));
    CppParts {
        global: global.finish(),
        setup: setup.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn dms(e: &CppCtx) -> CppParts {
    let zero = match e.input_type("in") {
        "bool" => "false",
        "float" => "0.0f",
        "string" => "String()",
        _ => "0",
    };
    let count = e.int("count", 2).clamp(0, 16);
    let mut l = Lines::default();
    l.line("{")
        .line(format!("  int idx = (int){};", e.input("sel")));
    for i in 0..count {
        l.line(format!(
            "  {} = idx == {i} ? {} : {zero};",
            e.output(&format!("o{}", i + 1)),
            e.input("in")
        ));
    }
    l.line("}");
    looped(l.finish())
}

pub(crate) fn cycle_report(_: &CppCtx) -> CppParts {
    CppParts::default()
}

pub(crate) fn decoder(e: &CppCtx) -> CppParts {
    // One line, as the template had it.
    let code: String = (0..e.int("count", 0).clamp(0, 16))
        .map(|i| format!("{} = {} == {i};", e.output(&format!("o{i}")), e.input("in")))
        .collect();
    looped(code)
}

pub(crate) fn encoder(e: &CppCtx) -> CppParts {
    let (out, any) = (e.output("out"), e.output("any"));
    let mut code = format!(
        "{out} = 0;
{any} = false;
"
    );
    // The conditions go on one line, as the template had them.
    for i in (0..e.int("count", 0).clamp(0, 16)).rev() {
        code += &format!(
            "if ({}) {{ {out} = {i}; {any} = true; }}",
            e.input(&format!("i{i}"))
        );
    }
    looped(code)
}

/// `Wire.begin()` shared by the expanders of one project: the pins when both are set.
fn wire_begin(e: &CppCtx) -> String {
    if e.int("sda", -1) >= 0 && e.int("scl", -1) >= 0 {
        format!("Wire.begin({}, {});", e.param("sda"), e.param("scl"))
    } else {
        "Wire.begin();".to_owned()
    }
}

pub(crate) fn pcf8574_in(e: &CppCtx) -> CppParts {
    let (id, ok, address) = (&e.id, e.output("ok"), e.param("address"));
    let raw = format!("{id}_raw");
    let mut setup = Lines::default();
    setup
        .line("// A pin reads as an input while the chip holds it high.")
        .line(format!("Wire.beginTransmission({address});"))
        .line("Wire.write(0xFF);")
        .line("Wire.endTransmission();");
    let mut l = Lines::default();
    l.line(format!(
        "if (millis() - {id}_polled >= {}) {{",
        e.param_ul("period")
    ))
    .line(format!("  {id}_polled = millis();"))
    .line(format!(
        "  if (Wire.requestFrom((uint8_t){address}, (uint8_t)1) == 1) {{"
    ))
    .line(format!("    uint8_t {raw} = Wire.read();"));
    if e.truthy("inverted") {
        l.line(format!("    {raw} = ~{raw};"));
    }
    for i in 0..e.int("count", 8).clamp(1, 8) {
        l.line(format!(
            "    {} = ({raw} & 0x{:02X}) != 0;",
            e.output(&format!("p{i}")),
            1 << i
        ));
    }
    l.line(format!("    {ok} = true;"))
        .line("  } else {")
        .line(format!("    {ok} = false;"))
        .line("  }")
        .line("}");
    CppParts {
        once_setup: wire_begin(e),
        global: format!("unsigned long {id}_polled = 0;"),
        setup: setup.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn pcf8574_out(e: &CppCtx) -> CppParts {
    let (id, ok, address) = (&e.id, e.output("ok"), e.param("address"));
    let want = format!("{id}_want");
    let state = format!("{id}_state");
    let mut global = Lines::default();
    global
        .line(format!("uint8_t {state} = 0;"))
        .line(format!("unsigned long {id}_try = 0;"));
    let mut l = Lines::default();
    l.line(format!("uint8_t {want} = 0;"));
    for i in 0..e.int("count", 8).clamp(1, 8) {
        l.line(format!(
            "if ({}) {want} |= 0x{:02X};",
            e.input(&format!("p{i}")),
            1 << i
        ));
    }
    if e.truthy("inverted") {
        l.line(format!("{want} = ~{want};"));
    }
    l.line("// Write on change; while the chip does not answer, retry twice a second.")
        .line(format!(
            "if ({ok} ? {want} != {state} : millis() - {id}_try >= 500UL) {{"
        ))
        .line(format!("  Wire.beginTransmission({address});"))
        .line(format!("  Wire.write({want});"))
        .line(format!("  {ok} = Wire.endTransmission() == 0;"))
        .line(format!("  {id}_try = millis();"))
        .line(format!("  if ({ok}) {{"))
        .line(format!("    {state} = {want};"))
        .line("  }")
        .line("}");
    CppParts {
        once_setup: wire_begin(e),
        global: global.finish(),
        setup: format!(
            "// The first write goes out on the first scan.\n{id}_try = millis() - 500UL;"
        ),
        looped: l.finish(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(b: &mut dyn Behavior, inputs: &[f64], initial: f64, out: &mut [f64]) {
        let ctx = Ctx {
            now_ms: 0,
            inputs,
            active: &[true, true],
            params: &[initial],
            texts: &[],
            param_texts: &[],
        };
        b.step(&ctx, out);
    }

    #[test]
    fn flash_store_loads_initial_and_saves_on_the_edge() {
        let mut store = behavior("flash_store").unwrap();
        let mut out = [0.0];
        step(&mut *store, &[5.0, 0.0], 2.5, &mut out);
        assert_eq!(out[0], 2.5);
        step(&mut *store, &[5.0, 1.0], 2.5, &mut out);
        assert_eq!(out[0], 5.0);
        // Held `save` does not write again.
        step(&mut *store, &[7.0, 1.0], 2.5, &mut out);
        assert_eq!(out[0], 5.0);
        step(&mut *store, &[7.0, 0.0], 2.5, &mut out);
        step(&mut *store, &[7.0, 1.0], 2.5, &mut out);
        assert_eq!(out[0], 7.0);
    }

    #[test]
    fn blocks_with_pins_have_no_simulation() {
        for kind in ["digital_in", "dht22", "pcf8574_out"] {
            assert!(behavior(kind).is_none(), "{kind}");
        }
    }
}
