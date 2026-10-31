//! Code of the port expanders (PCF8574 / PCF8574A / PCF8575) that digital inputs and
//! outputs stand on.
//!
//! Each expander in use gets two words of state: the pins as last read and the output
//! levels the blocks want. The inputs are read once at the start of a scan and the
//! outputs are written once at the end, so a chip costs at most two bus transactions
//! per scan whatever the number of blocks on it.

use std::fmt::Write;

use flp_core::expander::pin_param;
use flp_core::ident::c_ident;
use flp_core::{ExpanderChip, ParamValue, PinMode};

use crate::Program;

/// Retry period while the chip does not answer.
const RETRY_MS: u32 = 500;

/// An expander with at least one block on it.
struct Used<'a> {
    id: &'a str,
    /// C++ name part, `flp_x_<name>_…`.
    name: String,
    chip: ExpanderChip,
    address: u8,
    bus: Option<(i64, i64)>,
    period_ms: u64,
    /// Pins read by blocks: written as 1 so the chip lets them float high.
    in_mask: u32,
}

impl Used<'_> {
    fn bytes(&self) -> u8 {
        self.chip.pins() / 8
    }

    fn all_pins(&self) -> u32 {
        (1u32 << self.chip.pins()) - 1
    }
}

fn used<'a>(program: &'a Program) -> Vec<Used<'a>> {
    let logic = &program.project.logic;
    let mut out: Vec<Used> = Vec::new();
    for inst in &program.blocks {
        let Some(name) = inst.block.expander.as_deref() else {
            continue;
        };
        if inst.block.tag.is_some() {
            continue;
        }
        let Some(e) = logic.expander(name) else {
            continue;
        };
        let Some(p) = pin_param(&inst.lib.def) else {
            continue;
        };
        let Some(ParamValue::Int(pin)) = inst.params.get(p.name.as_str()) else {
            continue;
        };
        let index = match out.iter().position(|u| u.id == name) {
            Some(i) => i,
            None => {
                out.push(Used {
                    id: &e.id,
                    name: c_ident(&e.id),
                    chip: e.chip,
                    address: e.address,
                    bus: (e.sda >= 0).then_some((e.sda, e.scl)),
                    period_ms: e.period_ms().unwrap_or(20),
                    in_mask: 0,
                });
                out.len() - 1
            }
        };
        if p.mode == PinMode::In && (0..16).contains(pin) {
            out[index].in_mask |= 1 << pin;
        }
    }
    out
}

/// Names the templates of `digital_in` / `digital_out` use: the word with the pins as
/// read and the word with the wanted output levels.
pub fn state_names(expander: &str) -> (String, String) {
    let n = c_ident(expander);
    (format!("flp_x_{n}_in"), format!("flp_x_{n}_out"))
}

/// Header that the code needs, when any expander is in use.
pub fn header(program: &Program) -> Option<&'static str> {
    (!used(program).is_empty()).then_some("Wire.h")
}

/// State and functions of the expanders, for the global part of the sketch.
pub fn globals(program: &Program) -> String {
    let lang = program.lang();
    let mut out = String::new();
    for u in used(program) {
        let (input, output) = state_names(u.id);
        let n = &u.name;
        let all = u.all_pins();
        let _ = writeln!(
            out,
            "{}",
            crate::locf!(
                lang,
                "\n// Расширитель {}: {}, адрес 0x{:02X}",
                "\n// Expander {}: {}, address 0x{:02X}",
                u.id,
                u.chip.name(),
                u.address
            )
        );
        let _ = writeln!(out, "uint16_t {input} = 0x{all:04X};");
        let _ = writeln!(out, "uint16_t {output} = 0x{all:04X};");
        let _ = writeln!(out, "uint16_t flp_x_{n}_sent = 0x{all:04X};");
        let _ = writeln!(out, "bool flp_x_{n}_ok = false;");
        let _ = writeln!(out, "unsigned long flp_x_{n}_try = 0;");
        if u.in_mask != 0 {
            let _ = writeln!(out, "unsigned long flp_x_{n}_read = 0;");
            let bytes = u.bytes();
            let _ = writeln!(
                out,
                "{}",
                crate::locf!(
                    lang,
                    "\n// Читает выводы: не чаще одного раза в {} мс; при ошибке остаются прежние значения.",
                    "\n// Reads the pins: at most once per {} ms; on an error the previous values are kept.",
                    u.period_ms
                )
            );
            let _ = writeln!(out, "void flp_x_{n}_poll() {{");
            let _ = writeln!(
                out,
                "  if (millis() - flp_x_{n}_read < {}UL) {{\n    return;\n  }}",
                u.period_ms
            );
            let _ = writeln!(out, "  flp_x_{n}_read = millis();");
            let _ = writeln!(
                out,
                "  if (Wire.requestFrom((uint8_t)0x{:02X}, (uint8_t){bytes}) == {bytes}) {{",
                u.address
            );
            let _ = writeln!(out, "    uint16_t word = Wire.read();");
            if bytes > 1 {
                let _ = writeln!(out, "    word |= (uint16_t)Wire.read() << 8;");
            }
            let _ = writeln!(out, "    {input} = word;");
            let _ = writeln!(out, "  }} else {{");
            let _ = writeln!(
                out,
                "    // {}",
                lang.pick(
                    "Чип мог потерять питание: выводы будут записаны заново.",
                    "The chip may have lost power: the pins will be written again."
                )
            );
            let _ = writeln!(out, "    flp_x_{n}_ok = false;\n  }}\n}}");
        }
        let _ = writeln!(
            out,
            "\n// {}",
            lang.pick(
                "Записывает выводы при изменении; пока чип молчит, повторяет дважды в секунду.",
                "Writes the pins when they change; while the chip is silent, retries twice a second."
            )
        );
        let _ = writeln!(out, "void flp_x_{n}_flush() {{");
        let _ = writeln!(
            out,
            "  uint16_t want = ({output} | 0x{:04X}) & 0x{all:04X};",
            u.in_mask
        );
        let _ = writeln!(
            out,
            "  if (flp_x_{n}_ok ? want == flp_x_{n}_sent : millis() - flp_x_{n}_try < {RETRY_MS}UL) {{\n    return;\n  }}"
        );
        let _ = writeln!(out, "  Wire.beginTransmission(0x{:02X});", u.address);
        let _ = writeln!(out, "  Wire.write(want & 0xFF);");
        if u.bytes() > 1 {
            let _ = writeln!(out, "  Wire.write(want >> 8);");
        }
        let _ = writeln!(out, "  flp_x_{n}_ok = Wire.endTransmission() == 0;");
        let _ = writeln!(out, "  flp_x_{n}_try = millis();");
        let _ = writeln!(
            out,
            "  if (flp_x_{n}_ok) {{\n    flp_x_{n}_sent = want;\n  }}\n}}"
        );
    }
    out
}

/// Start of `setup()`: the bus and the first write (inputs go high at once).
pub fn setup(program: &Program) -> Vec<String> {
    let all = used(program);
    let mut lines = Vec::new();
    let mut buses: Vec<Option<(i64, i64)>> = Vec::new();
    for u in &all {
        if !buses.contains(&u.bus) {
            buses.push(u.bus);
            lines.push(match u.bus {
                Some((sda, scl)) => format!("Wire.begin({sda}, {scl});"),
                None => "Wire.begin();".to_owned(),
            });
        }
    }
    for u in &all {
        let n = &u.name;
        lines.push(format!("flp_x_{n}_try = millis() - {RETRY_MS}UL;"));
        lines.push(format!("flp_x_{n}_flush();"));
    }
    lines
}

/// Start of every scan: read the inputs.
pub fn poll(program: &Program) -> Vec<String> {
    used(program)
        .iter()
        .filter(|u| u.in_mask != 0)
        .map(|u| {
            crate::locf!(
                program.lang(),
                "flp_x_{}_poll();  // расширитель {}",
                "flp_x_{}_poll();  // expander {}",
                u.name,
                u.id
            )
        })
        .collect()
}

/// End of every scan: write the outputs.
pub fn flush(program: &Program) -> Vec<String> {
    used(program)
        .iter()
        .map(|u| {
            crate::locf!(
                program.lang(),
                "flp_x_{}_flush();  // расширитель {}",
                "flp_x_{}_flush();  // expander {}",
                u.name,
                u.id
            )
        })
        .collect()
}
