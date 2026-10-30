//! Hardware UARTs of the board: settings in `project.toml` (`[[uart]]`) or in a
//! settings block on a sheet (`uart_config`), the blocks that use them
//! (`[block.uart]` in `block.toml`) and their check.
//!
//! USB (`Serial`: upload, debugging, the port monitor) is a port of its own
//! for blocks (`-1` in the project): it has no settings and runs at the debug
//! link speed. On ESP32 it is UART0 through the USB bridge, so UART0 is not
//! offered there; ESP32-C3/S3 have native USB and a free UART0 (`Serial0`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::check::{DiagKind, Report, Severity};
use crate::library::PinMode;
use crate::pins::{BoardLayout, Chip, PinUse};
use crate::{Library, Logic, ParamValue};

/// Speed of the USB port; equals the debug link speed of `flp-debug`.
pub const USB_SPEED: u32 = 115_200;

/// Number of the USB port inside the program; blocks write `-1` for it.
pub const USB: u8 = u8::MAX;
/// A number no UART has (a typo in the project), so that the check reports it.
const INVALID: u8 = u8::MAX - 1;

/// Port of a block parameter: `-1` is USB, `0`… a UART.
pub fn port_number(value: i64) -> u8 {
    match value {
        -1 => USB,
        n => u8::try_from(n)
            .ok()
            .filter(|&n| n < INVALID)
            .unwrap_or(INVALID),
    }
}

/// `USB`, `UART0`, `UART1`...
pub fn port_name(number: u8) -> String {
    match number {
        USB => "USB".into(),
        n => format!("UART{n}"),
    }
}

/// One `[[uart]]` of `project.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UartConfig {
    /// 0, 1 or 2 — which of them the board has, see [`chip_uarts`].
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
            block: port_name(self.number),
            kind: "uart".into(),
            param: param.into(),
            gpio,
            mode,
            shared: false,
            bus: String::new(),
        })
        .collect()
    }
}

/// `Serial` for USB, `Serial0`, `Serial1`, `Serial2` for the UARTs.
pub fn serial_name(number: u8) -> String {
    match number {
        USB => "Serial".into(),
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
    /// Sets the port up instead of `[[uart]]`: its parameters `speed`, `format`,
    /// `rx`, `tx`, `de`, `rx_buffer` are the UART settings. Not an owner of the port.
    Config,
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
            number: port_number(number),
            access: uart.access,
        });
    }
    out
}

/// UARTs set up by settings blocks (`access = "config"`): (block id, settings).
pub fn block_configs(logic: &Logic, lib: &Library) -> Vec<(String, UartConfig)> {
    let mut out = Vec::new();
    for b in block_uarts(logic, lib) {
        if b.access != UartAccess::Config {
            continue;
        }
        let Some(block) = logic.block(&b.block) else {
            continue;
        };
        let Some(def) = lib.resolve(logic, block).map(|l| &l.def) else {
            continue;
        };
        let value = |name: &str| {
            block
                .params
                .get(name)
                .or_else(|| def.param(name).and_then(|p| p.default.as_ref()))
        };
        let defaults = UartConfig::new(b.number);
        let int = |name: &str, default: i64| match value(name) {
            Some(ParamValue::Int(v)) => *v,
            _ => default,
        };
        let format = match value("format") {
            Some(ParamValue::Text(s)) => s.clone(),
            _ => defaults.format.clone(),
        };
        out.push((
            b.block.clone(),
            UartConfig {
                number: b.number,
                speed: u32::try_from(int("speed", defaults.speed.into())).unwrap_or(0),
                format,
                rx: int("rx", defaults.rx),
                tx: int("tx", defaults.tx),
                de: int("de", defaults.de),
                rx_buffer: u32::try_from(int("rx_buffer", defaults.rx_buffer.into())).unwrap_or(0),
            },
        ));
    }
    out
}

/// Settings of all UARTs: `[[uart]]` of the board plus the settings blocks.
/// A UART set in both places, or by two blocks, keeps the first settings;
/// with `report` the clash is reported.
fn merged_uarts(
    project: &crate::Project,
    lib: &Library,
    report: Option<&mut Report>,
) -> Vec<UartConfig> {
    let mut uarts = project.config.uart.clone();
    let mut problems = Vec::new();
    let mut by_block: Vec<(u8, String)> = Vec::new();
    for (id, config) in block_configs(&project.logic, lib) {
        let name = port_name(config.number);
        if config.number == USB {
            problems.push((id, "USB не настраивается — выберите UART".into()));
        } else if let Some((_, first)) = by_block.iter().find(|(n, _)| *n == config.number) {
            problems.push((id, format!("{name} уже настроен блоком «{first}»")));
        } else if uarts.iter().any(|u| u.number == config.number) {
            problems.push((
                id,
                format!("{name} уже настроен в настройках платы — уберите его там или удалите этот блок"),
            ));
        } else {
            by_block.push((config.number, id));
            uarts.push(config);
        }
    }
    if let Some(report) = report {
        for (id, text) in problems {
            let message = format!("блок «{id}»: {text}");
            report.push(DiagKind::Uart, Severity::Error, Some(&id), None, message);
        }
    }
    uarts.sort_by_key(|u| u.number);
    uarts
}

/// Settings of all UARTs of the project, for the code generator and the editor.
pub fn project_uarts(project: &crate::Project, lib: &Library) -> Vec<UartConfig> {
    merged_uarts(project, lib, None)
}

/// UARTs a chip has for blocks besides USB. UART0 of ESP32 is the USB bridge.
pub fn chip_uarts(chip: Chip) -> &'static [u8] {
    match chip {
        Chip::Esp32 => &[1, 2],
        Chip::Esp32S3 => &[0, 1, 2],
        Chip::Esp32C3 | Chip::Esp32C3SuperMini => &[0, 1],
    }
}

/// Why a UART is missing: UART0 of ESP32 is taken by the USB bridge.
fn missing(number: u8, chip: Option<Chip>, available: &[u8]) -> String {
    let board = chip.map_or("платы", Chip::name);
    if number == 0 && chip == Some(Chip::Esp32) {
        "у ESP32 UART0 — это USB-мост: выберите «USB» или UART1/UART2".into()
    } else {
        format!(
            "у {board} нет {} (есть {})",
            port_name(number),
            list(available)
        )
    }
}

/// Settings of the UARTs, uses by blocks and pins against the blocks' pins.
/// `chip` is `None` for boards without a pin table.
/// [`check_uarts`] on the merged settings, for [`crate::check_project`].
pub(crate) fn check_project_part(
    project: &crate::Project,
    lib: &Library,
    block_pins: &[PinUse],
    chip: Option<Chip>,
    report: &mut Report,
) -> Vec<UartConfig> {
    let uarts = merged_uarts(project, lib, Some(report));
    check_uarts(
        &uarts,
        &block_uarts(&project.logic, lib),
        block_pins,
        chip,
        report,
    );
    uarts
}

pub(crate) fn check_uarts(
    uarts: &[UartConfig],
    blocks: &[BlockUart],
    block_pins: &[PinUse],
    chip: Option<Chip>,
    report: &mut Report,
) {
    let available = chip.map_or(&[1, 2][..], chip_uarts);
    let mut configured = BTreeMap::new();
    for u in uarts {
        let name = port_name(u.number);
        let mut error =
            |text: String| report.push(DiagKind::Uart, Severity::Error, None, None, text);
        if !available.contains(&u.number) {
            error(format!("{name}: {}", missing(u.number, chip, available)));
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
                    DiagKind::Uart,
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
        let name = port_name(b.number);
        // USB needs no settings; while debugging, readers get the console input.
        if b.number == USB && b.access == UartAccess::Exclusive {
            report.push(
                DiagKind::Uart,
                Severity::Error,
                Some(id),
                None,
                format!(
                    "блок «{id}»: USB занят заливкой, отладкой и монитором порта — выберите UART"
                ),
            );
            continue;
        }
        if b.number != USB && !available.contains(&b.number) {
            let why = missing(b.number, chip, available);
            report.push(
                DiagKind::Uart,
                Severity::Error,
                Some(id),
                None,
                format!("блок «{id}»: {why}"),
            );
            continue;
        }
        if b.number != USB && !configured.contains_key(&b.number) {
            report.push(
                DiagKind::Uart,
                Severity::Error,
                Some(id),
                None,
                format!("блок «{id}»: {name} не настроен — добавьте его в настройках платы"),
            );
            continue;
        }
        if b.access == UartAccess::Config {
            continue;
        }
        let clash = blocks[..i].iter().find(|o| {
            o.number == b.number
                && o.access != UartAccess::Config
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
                DiagKind::Uart,
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
    let uarts = merged_uarts(project, lib, Some(&mut report));
    check_uarts(
        &uarts,
        &block_uarts(&project.logic, lib),
        &[],
        BoardLayout::of_board(&project.config.board, lib.boards()).map(|b| b.chip()),
        &mut report,
    );
    report
}

/// Settings of a port for templates and setup code; USB gets its fixed settings.
pub fn config_of(uarts: &[UartConfig], number: u8) -> Option<UartConfig> {
    if number == USB {
        return Some(UartConfig {
            number: USB,
            speed: USB_SPEED,
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
        .map(|&n| port_name(n))
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

/// `uart_send` and `uart_read` took the port in `uart`, where 0 meant USB;
/// now it is `port` with -1 for USB, and 0 is UART0 (ESP32-C3/S3).
fn migrate_port_param(logic: &mut Logic) {
    for block in logic
        .blocks
        .iter_mut()
        .filter(|b| b.kind == "uart_send" || b.kind == "uart_read")
    {
        if let Some(value) = block.params.remove("uart") {
            let port = match value {
                ParamValue::Int(0) => ParamValue::Int(-1),
                other => other,
            };
            block.params.insert("port".into(), port);
        }
    }
}

/// `uart_send` before its input took any type: `edge` (bool) and `mode`
/// (0 text, 1 byte, 2 int16, 3 float, 4 long) with inputs `text`, `value`
/// (float) and `lvalue` (int). Now `trigger`, `send_as`, `int_size` and the
/// one input `value`. A float sent as a byte or int16 now goes out as a float.
pub fn migrate_uart_send(logic: &mut Logic) {
    migrate_port_param(logic);
    let mut renamed: Vec<String> = Vec::new();
    for block in logic.blocks.iter_mut().filter(|b| b.kind == "uart_send") {
        if let Some(edge) = block.params.remove("edge") {
            if edge == ParamValue::Bool(false) {
                block.params.insert("trigger".into(), ParamValue::Int(1));
            }
        }
        if let Some(ParamValue::Int(mode)) = block.params.remove("mode") {
            if mode != 0 {
                block.params.insert("send_as".into(), ParamValue::Int(1));
            }
            let size = match mode {
                1 => Some(0),
                2 => Some(1),
                4 => Some(2),
                _ => None,
            };
            if let Some(size) = size {
                block
                    .params
                    .insert("int_size".into(), ParamValue::Int(size));
            }
        }
        for old in ["text", "lvalue"] {
            if let Some(value) = block.consts.remove(old) {
                block.consts.insert("value".into(), value);
            }
        }
        renamed.push(block.id.clone());
    }
    for link in &mut logic.links {
        if renamed.contains(&link.to.block) && matches!(link.to.port.as_str(), "text" | "lvalue") {
            link.to.port = "value".into();
        }
    }
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
        assert_eq!(serial_name(USB), "Serial");
        assert_eq!(serial_name(0), "Serial0");
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
            bus: String::new(),
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
            &[("a", 2, Write), ("b", 2, Write), ("c", USB, Write)],
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
        let m = messages(&[], &[("mb", USB, Exclusive)], Chip::Esp32);
        assert!(m[0].contains("USB занят"), "{m:?}");
        // UART0: the USB bridge on ESP32, a free port on ESP32-C3.
        let m = messages(&[], &[("tx", 0, Write)], Chip::Esp32);
        assert!(m[0].contains("UART0 — это USB-мост"), "{m:?}");
        let zero = UartConfig::new(0);
        assert!(messages(
            std::slice::from_ref(&zero),
            &[("tx", 0, Write), ("u", USB, Write)],
            Chip::Esp32C3
        )
        .is_empty());
        // USB: no settings needed, one reader besides writers.
        assert!(messages(&[], &[("rx", USB, Read), ("tx", USB, Write)], Chip::Esp32).is_empty());
        let m = messages(&[], &[("r1", USB, Read), ("r2", USB, Read)], Chip::Esp32);
        assert!(m[0].contains("только один блок"), "{m:?}");

        // ESP32-C3 has no UART2; a pin taken by a block; a bad format; twice.
        let m = messages(std::slice::from_ref(&two), &[], Chip::Esp32C3);
        assert!(m[0].contains("нет UART2 (есть UART0, UART1)"), "{m:?}");
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
    fn old_uart_send_params_move() {
        let mut logic: Logic = toml::from_str(
            r#"
[[block]]
id = "a"
type = "uart_send"
uart = 0
edge = false

[[block]]
id = "r"
type = "uart_read"
uart = 2

[[block]]
id = "b"
type = "uart_send"
mode = 4
edge = true

[[block]]
id = "src"
type = "constant"

[[link]]
from = "src.out"
to = "a.text"

[[link]]
from = "src.out"
to = "b.lvalue"
"#,
        )
        .unwrap();
        migrate_uart_send(&mut logic);
        let a = &logic.blocks[0].params;
        assert_eq!(a.get("trigger"), Some(&ParamValue::Int(1)));
        assert!(!a.contains_key("edge") && !a.contains_key("send_as"));
        let b = &logic.blocks[2].params;
        assert_eq!(b.get("send_as"), Some(&ParamValue::Int(1)));
        assert_eq!(b.get("int_size"), Some(&ParamValue::Int(2)));
        assert!(!b.contains_key("mode") && !b.contains_key("trigger"));
        assert!(logic.links.iter().all(|l| l.to.port == "value"));
        // `uart = 0` meant USB: now `port = -1`; other numbers stay.
        assert_eq!(a.get("port"), Some(&ParamValue::Int(-1)));
        assert!(!a.contains_key("uart"));
        assert_eq!(
            logic.blocks[1].params.get("port"),
            Some(&ParamValue::Int(2))
        );
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
