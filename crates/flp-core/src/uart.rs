//! Hardware UARTs of the board: settings in `project.toml` (`[[uart]]`), the
//! blocks that use them (`[block.uart]` in `block.toml`) and their check.
//!
//! UART0 is the USB port (upload, debugging, the port monitor): it has no
//! settings, runs at the debug link speed and is only written to by blocks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::check::{Report, Severity};
use crate::library::PinMode;
use crate::pins::{Chip, PinUse};
use crate::{Library, Logic, ParamValue};

/// Speed of UART0; equals the debug link speed of `flp-debug`.
pub const UART0_SPEED: u32 = 115_200;

/// One `[[uart]]` of `project.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UartConfig {
    /// 1 or 2 (ESP32-C3 has only UART1).
    pub number: u8,
    /// Baud rate.
    pub speed: u32,
    /// Data bits, parity (N, E, O) and stop bits: `8N1`, `8E1`, `7E2`...
    pub format: String,
    /// RX pin; -1 — the default pin of the Arduino core.
    pub rx: i64,
    /// TX pin; -1 — the default pin of the Arduino core.
    pub tx: i64,
    /// RS-485 direction pin (DE/RE); -1 — not used.
    pub de: i64,
    /// Receive buffer in bytes.
    pub rx_buffer: u32,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            number: 2,
            speed: 9600,
            format: "8N1".into(),
            rx: 16,
            tx: 17,
            de: -1,
            rx_buffer: 256,
        }
    }
}

impl UartConfig {
    /// Default settings of a UART: UART2 on GPIO16/17, others on the core's pins.
    pub fn new(number: u8) -> Self {
        let (rx, tx) = if number == 2 { (16, 17) } else { (-1, -1) };
        Self {
            number,
            rx,
            tx,
            ..Default::default()
        }
    }

    /// Parsed `format`: (data bits, parity `N`/`E`/`O`, stop bits).
    pub fn frame(&self) -> Option<(u8, char, u8)> {
        let mut c = self.format.trim().chars();
        let (Some(d), Some(p), Some(s), None) = (c.next(), c.next(), c.next(), c.next()) else {
            return None;
        };
        let data = d.to_digit(10).filter(|d| (5..=8).contains(d))? as u8;
        let parity = p.to_ascii_uppercase();
        let stop = s.to_digit(10).filter(|s| (1..=2).contains(s))? as u8;
        "NEO".contains(parity).then_some((data, parity, stop))
    }

    /// Parity as Modbus RTU blocks take it: 0 — none, 1 — even, 2 — odd.
    pub fn parity_code(&self) -> u8 {
        match self.frame() {
            Some((_, 'E', _)) => 1,
            Some((_, 'O', _)) => 2,
            _ => 0,
        }
    }

    /// Arduino `HardwareSerial` name.
    pub fn serial(&self) -> String {
        serial_name(self.number)
    }

    /// Arduino frame constant, e.g. `SERIAL_8N1`.
    pub fn arduino_config(&self) -> String {
        format!("SERIAL_{}", self.format.trim().to_ascii_uppercase())
    }

    /// Pins taken by the UART, as the check counts them.
    pub(crate) fn pin_uses(&self) -> Vec<PinUse> {
        [
            ("rx", self.rx, PinMode::In),
            ("tx", self.tx, PinMode::Out),
            ("de", self.de, PinMode::Out),
        ]
        .into_iter()
        .filter(|&(_, gpio, _)| gpio >= 0)
        .map(|(param, gpio, mode)| PinUse {
            block: format!("UART{}", self.number),
            kind: "uart".into(),
            param: param.into(),
            gpio,
            mode,
            shared: false,
        })
        .collect()
    }
}

/// `Serial`, `Serial1`, `Serial2`.
pub fn serial_name(number: u8) -> String {
    match number {
        0 => "Serial".into(),
        n => format!("Serial{n}"),
    }
}

/// How a block uses its UART (`[block.uart]` in `block.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UartUse {
    /// `int` parameter with the UART number.
    pub param: String,
    pub access: UartAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UartAccess {
    /// The block's library sets the port up itself (Modbus RTU): nobody else may use it.
    Exclusive,
    /// Sends only: any number of such blocks may share the port.
    Write,
    /// Receives: one reader per port, the bytes it takes are gone for others.
    Read,
}

/// A block on a UART, in schema order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockUart {
    pub block: String,
    pub number: u8,
    pub access: UartAccess,
}

/// Blocks that use UARTs. Unknown types and repeated ids are skipped.
pub fn block_uarts(logic: &Logic, lib: &Library) -> Vec<BlockUart> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for block in &logic.blocks {
        if !seen.insert(block.id.as_str()) {
            continue;
        }
        let Some(def) = lib.resolve(logic, block).map(|l| &l.def) else {
            continue;
        };
        let Some(uart) = &def.info.uart else { continue };
        let value = block
            .params
            .get(&uart.param)
            .or_else(|| def.param(&uart.param).and_then(|p| p.default.as_ref()));
        let Some(&ParamValue::Int(number)) = value else {
            continue;
        };
        out.push(BlockUart {
            block: block.id.clone(),
            number: u8::try_from(number).unwrap_or(u8::MAX),
            access: uart.access,
        });
    }
    out
}

/// UART numbers a chip has besides UART0.
pub fn chip_uarts(chip: Chip) -> &'static [u8] {
    match chip {
        Chip::Esp32 | Chip::Esp32S3 => &[1, 2],
        Chip::Esp32C3 | Chip::Esp32C3SuperMini => &[1],
    }
}

/// Settings of the UARTs, uses by blocks and pins against the blocks' pins.
/// `chip` is `None` for boards without a pin table.
pub(crate) fn check_uarts(
    uarts: &[UartConfig],
    blocks: &[BlockUart],
    block_pins: &[PinUse],
    chip: Option<Chip>,
    report: &mut Report,
) {
    let available = chip.map_or(&[1, 2][..], chip_uarts);
    let board = chip.map_or("платы", Chip::name);
    let mut configured = BTreeMap::new();
    for u in uarts {
        let name = format!("UART{}", u.number);
        let mut error = |text: String| report.push(Severity::Error, None, None, text);
        if !available.contains(&u.number) {
            error(format!(
                "{name}: у {board} такого UART нет (есть {})",
                list(available)
            ));
        }
        if configured.insert(u.number, u).is_some() {
            error(format!("{name} настроен дважды"));
        }
        if u.frame().is_none() {
            error(format!(
                "{name}: формат «{}» не понят — нужно вроде 8N1: биты данных 5…8, чётность N/E/O, стоп-биты 1 или 2",
                u.format
            ));
        }
        if u.speed == 0 {
            error(format!("{name}: скорость должна быть больше нуля"));
        }
        if u.rx_buffer < 64 {
            error(format!("{name}: буфер приёма меньше 64 байт"));
        }
    }

    // UART pins against block pins and each other.
    let mut taken: Vec<PinUse> = block_pins.to_vec();
    for u in uarts {
        for p in u.pin_uses() {
            if let Some(other) = taken.iter().find(|t| t.gpio == p.gpio) {
                let owner = if other.kind == "uart" {
                    format!("{} ({})", other.block, other.param)
                } else {
                    format!("блоком «{}»", other.block)
                };
                report.push(
                    Severity::Error,
                    None,
                    None,
                    format!(
                        "{}: вывод {} GPIO{} уже занят {owner}",
                        p.block,
                        p.param.to_uppercase(),
                        p.gpio
                    ),
                );
            }
            taken.push(p);
        }
    }

    for (i, b) in blocks.iter().enumerate() {
        let id = b.block.as_str();
        let name = format!("UART{}", b.number);
        // UART0 (USB) needs no settings; while debugging, readers get the console input.
        if b.number == 0 && b.access == UartAccess::Exclusive {
            report.push(
                Severity::Error,
                Some(id),
                None,
                format!(
                    "блок «{id}»: UART0 занят USB: заливка, отладка и монитор порта — выберите UART1 или UART2"
                ),
            );
            continue;
        }
        if b.number != 0 && !configured.contains_key(&b.number) {
            report.push(
                Severity::Error,
                Some(id),
                None,
                format!("блок «{id}»: {name} не настроен — добавьте его в настройках платы"),
            );
            continue;
        }
        let clash = blocks[..i].iter().find(|o| {
            o.number == b.number
                && (o.access == UartAccess::Exclusive
                    || b.access == UartAccess::Exclusive
                    || (o.access == UartAccess::Read && b.access == UartAccess::Read))
        });
        if let Some(other) = clash {
            let why = if b.access == UartAccess::Read && other.access == UartAccess::Read {
                "читать один UART может только один блок"
            } else {
                "этот UART целиком занят"
            };
            report.push(
                Severity::Error,
                Some(id),
                None,
                format!(
                    "блок «{id}»: {name} уже использует блок «{}» — {why}",
                    other.block
                ),
            );
        }
    }
}

/// UART problems alone, for the code generator: it needs the settings of every
/// UART a block uses. The full check ([`crate::check_project`]) adds pin conflicts.
pub fn check_project_uarts(project: &crate::Project, lib: &Library) -> Report {
    let mut report = Report::default();
    check_uarts(
        &project.config.uart,
        &block_uarts(&project.logic, lib),
        &[],
        Chip::of_board(&project.config.board),
        &mut report,
    );
    report
}

/// Settings of a UART for templates and setup code; UART0 gets the USB settings.
pub fn config_of(uarts: &[UartConfig], number: u8) -> Option<UartConfig> {
    if number == 0 {
        return Some(UartConfig {
            number: 0,
            speed: UART0_SPEED,
            rx: -1,
            tx: -1,
            ..Default::default()
        });
    }
    uarts.iter().find(|u| u.number == number).cloned()
}

fn list(numbers: &[u8]) -> String {
    numbers
        .iter()
        .map(|n| format!("UART{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Block types that kept their UART settings as their own parameters before
/// `[[uart]]` appeared, with the defaults they had.
const LEGACY_BLOCKS: [&str; 2] = ["modbus_rtu_master", "modbus_rtu_slave"];

/// Moves the old UART parameters of Modbus RTU blocks (`speed`, `parity`, `rx`,
/// `tx`, `de`) into `[[uart]]`. A UART that is already configured keeps its
/// settings. Returns the numbers of the UARTs created.
pub fn migrate_legacy(uarts: &mut Vec<UartConfig>, logic: &mut Logic) -> Vec<u8> {
    const KEYS: [&str; 5] = ["speed", "parity", "rx", "tx", "de"];
    let mut created = Vec::new();
    for block in &mut logic.blocks {
        if !LEGACY_BLOCKS.contains(&block.kind.as_str())
            || !KEYS.iter().any(|k| block.params.contains_key(*k))
        {
            continue;
        }
        let number = match block.params.get("uart") {
            Some(ParamValue::Int(n)) => u8::try_from(*n).unwrap_or(2),
            _ => 2,
        };
        let int = |key: &str, default: i64| match block.params.get(key) {
            Some(ParamValue::Int(v)) => *v,
            _ => default,
        };
        let parity = match int("parity", 0) {
            1 => "8E1",
            2 => "8O1",
            _ => "8N1",
        };
        let config = UartConfig {
            number,
            speed: u32::try_from(int("speed", 9600)).unwrap_or(9600),
            format: parity.into(),
            rx: int("rx", 16),
            tx: int("tx", 17),
            de: int("de", -1),
            rx_buffer: 256,
        };
        for key in KEYS {
            block.params.remove(key);
        }
        if uarts.iter().all(|u| u.number != number) {
            uarts.push(config);
            created.push(number);
        }
    }
    uarts.sort_by_key(|u| u.number);
    created
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let mut u = UartConfig::new(1);
        assert_eq!(u.frame(), Some((8, 'N', 1)));
        assert_eq!(u.arduino_config(), "SERIAL_8N1");
        u.format = "7e2".into();
        assert_eq!(u.frame(), Some((7, 'E', 2)));
        assert_eq!(u.arduino_config(), "SERIAL_7E2");
        assert_eq!(u.parity_code(), 1);
        for bad in ["8X1", "9N1", "8N3", "8N", "8N11", ""] {
            u.format = bad.into();
            assert_eq!(u.frame(), None, "{bad}");
        }
        assert_eq!(UartConfig::new(2).serial(), "Serial2");
        assert_eq!(serial_name(0), "Serial");
    }

    fn messages(
        uarts: &[UartConfig],
        blocks: &[(&str, u8, UartAccess)],
        chip: Chip,
    ) -> Vec<String> {
        let blocks: Vec<BlockUart> = blocks
            .iter()
            .map(|&(block, number, access)| BlockUart {
                block: block.into(),
                number,
                access,
            })
            .collect();
        let pins = [PinUse {
            block: "led".into(),
            kind: "digital_out".into(),
            param: "pin".into(),
            gpio: 4,
            mode: PinMode::Out,
            shared: false,
        }];
        let mut report = Report::default();
        check_uarts(uarts, &blocks, &pins, Some(chip), &mut report);
        report.diagnostics.into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn check_finds_uart_problems() {
        use UartAccess::*;
        let two = UartConfig::new(2);
        assert!(messages(
            std::slice::from_ref(&two),
            &[("tx", 2, Write), ("rx", 2, Read)],
            Chip::Esp32
        )
        .is_empty());
        // Writers share a port; USB is write-only.
        assert!(messages(
            std::slice::from_ref(&two),
            &[("a", 2, Write), ("b", 2, Write), ("c", 0, Write)],
            Chip::Esp32
        )
        .is_empty());

        let m = messages(&[], &[("mb", 2, Exclusive)], Chip::Esp32);
        assert!(m[0].contains("UART2 не настроен"), "{m:?}");
        let m = messages(
            std::slice::from_ref(&two),
            &[("mb", 2, Exclusive), ("tx", 2, Write)],
            Chip::Esp32,
        );
        assert!(m[0].contains("уже использует блок «mb»"), "{m:?}");
        let m = messages(
            std::slice::from_ref(&two),
            &[("r1", 2, Read), ("r2", 2, Read)],
            Chip::Esp32,
        );
        assert!(m[0].contains("только один блок"), "{m:?}");
        let m = messages(&[], &[("mb", 0, Exclusive)], Chip::Esp32);
        assert!(m[0].contains("UART0 занят USB"), "{m:?}");
        // USB: no settings needed, one reader besides writers.
        assert!(messages(&[], &[("rx", 0, Read), ("tx", 0, Write)], Chip::Esp32).is_empty());
        let m = messages(&[], &[("r1", 0, Read), ("r2", 0, Read)], Chip::Esp32);
        assert!(m[0].contains("только один блок"), "{m:?}");

        // ESP32-C3 has no UART2; a pin taken by a block; a bad format; twice.
        let m = messages(std::slice::from_ref(&two), &[], Chip::Esp32C3);
        assert!(m[0].contains("такого UART нет (есть UART1)"), "{m:?}");
        let mut one = UartConfig::new(1);
        one.tx = 4;
        one.format = "8X1".into();
        let m = messages(&[one.clone(), one], &[], Chip::Esp32);
        assert!(m.iter().any(|m| m.contains("настроен дважды")), "{m:?}");
        assert!(m.iter().any(|m| m.contains("формат «8X1»")), "{m:?}");
        assert!(
            m.iter()
                .any(|m| m.contains("TX GPIO4 уже занят блоком «led»")),
            "{m:?}"
        );
        let mut one = UartConfig::new(1);
        one.rx = 16;
        let m = messages(&[one, two], &[], Chip::Esp32);
        assert_eq!(m, ["UART2: вывод RX GPIO16 уже занят UART1 (rx)"]);
    }

    #[test]
    fn legacy_modbus_params_move_to_uart() {
        let mut logic: Logic = toml::from_str(
            r#"
[[block]]
id = "master"
type = "modbus_rtu_master"
uart = 2
rx = 16
tx = 17
de = 4

[[block]]
id = "slave"
type = "modbus_rtu_slave"
uart = 1
speed = 19200
parity = 2
rx = 25
tx = 27

[[block]]
id = "other"
type = "digital_out"
pin = 5
"#,
        )
        .unwrap();
        let mut uarts = Vec::new();
        assert_eq!(migrate_legacy(&mut uarts, &mut logic), [2, 1]);
        assert_eq!(uarts.len(), 2);
        assert_eq!(
            uarts[0],
            UartConfig {
                number: 1,
                speed: 19200,
                format: "8O1".into(),
                rx: 25,
                tx: 27,
                de: -1,
                rx_buffer: 256,
            }
        );
        assert_eq!((uarts[1].number, uarts[1].speed, uarts[1].de), (2, 9600, 4));
        assert!(!logic.blocks[0].params.contains_key("rx"));
        assert!(logic.blocks[0].params.contains_key("uart"));
        assert!(logic.blocks[2].params.contains_key("pin"));
        // Second run: nothing left to move.
        assert!(migrate_legacy(&mut uarts, &mut logic).is_empty());
    }
}
