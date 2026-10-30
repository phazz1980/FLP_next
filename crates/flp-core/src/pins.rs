//! Board pins: which GPIOs a chip has, what each of them can do, and which
//! blocks of a schema use them. The check and the editor's pin list share it.

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::boards::{Boards, UserBoard};
use crate::expander::Expander;
use crate::library::{ParamKind, PinMode};
use crate::{BoardConfig, Library, Logic, ParamValue};

/// Pin layout of the board: the chip family, or an exact board when it brings out
/// only part of the chip's pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    Esp32,
    Esp32C3,
    Esp32S3,
    /// ESP32-C3 Super Mini: GPIO0–10, 20, 21; LED on GPIO8, BOOT button on GPIO9.
    Esp32C3SuperMini,
}

/// A board with its own pin layout on top of a generic FQBN.
pub struct BoardModel {
    /// Value of `board.model` in `project.toml`.
    pub id: &'static str,
    pub fqbn: &'static str,
    pub name: &'static str,
    pub chip: Chip,
}

pub const BOARD_MODELS: &[BoardModel] = &[BoardModel {
    id: "esp32c3_supermini",
    fqbn: "esp32:esp32:esp32c3",
    name: "ESP32-C3 Super Mini",
    chip: Chip::Esp32C3SuperMini,
}];

impl BoardModel {
    pub fn find(id: &str) -> Option<&'static Self> {
        BOARD_MODELS.iter().find(|m| m.id == id)
    }
}

/// What a pin can be used for (`functions` of a `[[pin]]` in `board.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinFunction {
    /// Digital input.
    Din,
    /// Digital output.
    Dout,
    /// PWM output.
    Pwm,
    /// Analog input.
    Adc,
    /// Analog output.
    Dac,
}

impl PinFunction {
    pub const ALL: [Self; 5] = [Self::Din, Self::Dout, Self::Pwm, Self::Adc, Self::Dac];

    /// Name in Russian for the board editor and messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Din => "цифровой вход",
            Self::Dout => "цифровой выход",
            Self::Pwm => "ШИМ",
            Self::Adc => "АЦП",
            Self::Dac => "ЦАП",
        }
    }
}

/// One GPIO of a chip or of a board's pin table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinInfo {
    pub gpio: i64,
    /// What the board has on this pin ("A1", "RS485 RX"); empty for plain chips.
    pub label: String,
    /// ADC unit (1 or 2), 0 without an ADC.
    pub adc: u8,
    /// The pin has a DAC.
    pub dac: bool,
    /// Functions the board allows on the pin: a subset of what the chip can do.
    /// `None` — everything the chip can.
    pub only: Option<BTreeSet<PinFunction>>,
    pub input_only: bool,
    /// Wired to the flash chip: using it hangs the board.
    pub flash: bool,
    /// Sampled at reset to choose the boot mode; an external pull may stop booting.
    pub strapping: bool,
    /// Usable, but busy with something else on most boards (reason in Russian).
    pub warning: Option<String>,
}

impl PinInfo {
    fn new(gpio: i64) -> Self {
        Self {
            gpio,
            label: String::new(),
            adc: 0,
            dac: false,
            only: None,
            input_only: false,
            flash: false,
            strapping: false,
            warning: None,
        }
    }

    /// What the chip can do on this pin, whatever the board says.
    pub fn chip_can(&self, function: PinFunction) -> bool {
        if self.flash {
            return false;
        }
        match function {
            PinFunction::Din => true,
            PinFunction::Dout | PinFunction::Pwm => !self.input_only,
            PinFunction::Adc => self.adc != 0,
            PinFunction::Dac => self.dac,
        }
    }

    /// The pin can be used for this function: the chip can and the board allows.
    pub fn has(&self, function: PinFunction) -> bool {
        self.chip_can(function) && self.only.as_ref().is_none_or(|o| o.contains(&function))
    }

    /// Functions of the pin in the order of [`PinFunction::ALL`].
    pub fn functions(&self) -> Vec<PinFunction> {
        PinFunction::ALL
            .into_iter()
            .filter(|f| self.has(*f))
            .collect()
    }

    /// The pin can serve a parameter of this mode.
    pub fn fits(&self, mode: PinMode) -> bool {
        match mode {
            PinMode::In => self.has(PinFunction::Din),
            PinMode::Out => self.has(PinFunction::Dout),
            PinMode::Io => self.has(PinFunction::Din) && self.has(PinFunction::Dout),
            PinMode::Pwm => self.has(PinFunction::Pwm),
            PinMode::Adc => self.has(PinFunction::Adc),
            PinMode::Dac => self.has(PinFunction::Dac),
        }
    }

    /// Name for lists: "GPIO36", or "A1 (GPIO36)" for a pin the board has a name for.
    pub fn title(&self) -> String {
        if self.label.is_empty() {
            format!("GPIO{}", self.gpio)
        } else {
            format!("{} (GPIO{})", self.label, self.gpio)
        }
    }

    /// Short notes for the pin list: "АЦП1, только вход".
    pub fn notes(&self) -> String {
        let mut notes = Vec::new();
        match self.adc {
            0 => {}
            2 => notes.push("АЦП2, не при Wi-Fi".to_owned()),
            n => notes.push(format!("АЦП{n}")),
        }
        if self.input_only {
            notes.push("только вход".to_owned());
        }
        if self.strapping {
            notes.push("загрузочный".to_owned());
        }
        if let Some(w) = &self.warning {
            notes.push(w.clone());
        }
        notes.join(", ")
    }
}

const UART0: &str = "UART0: заливка и отладка по USB";
const USB: &str = "встроенный USB: заливка и отладка";

impl Chip {
    /// Pin layout of the project's board: its model if known, otherwise by FQBN.
    pub fn of_board(board: &BoardConfig) -> Option<Self> {
        board
            .model
            .as_deref()
            .and_then(BoardModel::find)
            .filter(|m| m.fqbn == board.fqbn)
            .map(|m| m.chip)
            .or_else(|| Self::from_fqbn(&board.fqbn))
    }

    /// Chip of an arduino-cli board name; `None` for boards without a pin table.
    pub fn from_fqbn(fqbn: &str) -> Option<Self> {
        let mut parts = fqbn.split(':');
        let (Some(vendor), Some(arch), Some(board)) = (parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        if vendor != "esp32" || arch != "esp32" {
            return None;
        }
        let board = board.to_lowercase();
        if board.contains("c3") {
            Some(Self::Esp32C3)
        } else if board.contains("s3") {
            Some(Self::Esp32S3)
        } else if ["s2", "c2", "c5", "c6", "h2", "p4"]
            .iter()
            .any(|other| board.contains(other))
        {
            None
        } else {
            Some(Self::Esp32)
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Esp32 => "ESP32",
            Self::Esp32C3 => "ESP32-C3",
            Self::Esp32S3 => "ESP32-S3",
            Self::Esp32C3SuperMini => "ESP32-C3 Super Mini",
        }
    }

    /// GPIOs brought out on boards with this chip, in ascending order.
    pub fn pins(self) -> Vec<PinInfo> {
        match self {
            Self::Esp32 => esp32(),
            Self::Esp32C3 => esp32c3(),
            Self::Esp32S3 => esp32s3(),
            Self::Esp32C3SuperMini => esp32c3_supermini(),
        }
    }

    pub fn pin(self, gpio: i64) -> Option<PinInfo> {
        self.pins().into_iter().find(|p| p.gpio == gpio)
    }

    /// Id of the chip family in `board.toml`; `None` for exact boards.
    pub fn id(self) -> Option<&'static str> {
        match self {
            Self::Esp32 => Some("esp32"),
            Self::Esp32C3 => Some("esp32c3"),
            Self::Esp32S3 => Some("esp32s3"),
            Self::Esp32C3SuperMini => None,
        }
    }

    /// Chip family by its id in `board.toml`.
    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Esp32, Self::Esp32C3, Self::Esp32S3]
            .into_iter()
            .find(|c| c.id() == Some(id))
    }
}

/// Pins of the project's board: the chip's table, or the one a user board brings out
/// with names of what is wired to each pin.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardLayout {
    chip: Chip,
    name: String,
    pins: Vec<PinInfo>,
    expanders: Vec<BoardExpander>,
}

/// A pin of an expander soldered on the board, as the board's author named it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardExpanderPin {
    /// Pin of the chip (`P0` is 0).
    pub pin: i64,
    /// What is printed on the board ("K1"); may be empty.
    pub label: String,
    /// `Din` and/or `Dout`: what the board lets blocks do with the pin.
    pub functions: BTreeSet<PinFunction>,
}

/// An expander chip of the board with the pins the board brings out. A project that
/// uses one of its pins declares the same chip in `[[expander]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardExpander {
    /// Name of the device it comes from ("Реле").
    pub title: String,
    pub expander: Expander,
    /// Pins in ascending order.
    pub pins: Vec<BoardExpanderPin>,
}

impl BoardExpander {
    /// The project's expander is this very chip: same type, address and bus pins.
    pub fn is(&self, e: &Expander) -> bool {
        self.expander.chip == e.chip
            && self.expander.address == e.address
            && self.expander.sda == e.sda
            && self.expander.scl == e.scl
    }

    pub fn pin(&self, pin: i64) -> Option<&BoardExpanderPin> {
        self.pins.iter().find(|p| p.pin == pin)
    }

    /// Whether the pin can take a block that works in `mode`.
    pub fn fits(&self, pin: i64, mode: PinMode) -> bool {
        let Some(p) = self.pin(pin) else {
            return false;
        };
        let has = |f| p.functions.contains(&f);
        match mode {
            PinMode::In => has(PinFunction::Din),
            PinMode::Out => has(PinFunction::Dout),
            PinMode::Io => has(PinFunction::Din) && has(PinFunction::Dout),
            PinMode::Pwm | PinMode::Adc | PinMode::Dac => false,
        }
    }
}

impl BoardLayout {
    pub fn new(chip: Chip, name: String, pins: Vec<PinInfo>) -> Self {
        Self {
            chip,
            name,
            pins,
            expanders: Vec::new(),
        }
    }

    /// The same board with the expanders it carries.
    pub fn with_expanders(mut self, expanders: Vec<BoardExpander>) -> Self {
        self.expanders = expanders;
        self
    }

    /// Expanders soldered on the board whose pins the author listed.
    pub fn expanders(&self) -> &[BoardExpander] {
        &self.expanders
    }

    /// The board's description of the project's expander, if it is the same chip.
    pub fn expander_for(&self, e: &Expander) -> Option<&BoardExpander> {
        self.expanders.iter().find(|b| b.is(e))
    }

    pub fn of_chip(chip: Chip) -> Self {
        Self::new(chip, chip.name().to_owned(), chip.pins())
    }

    /// Layout of the project's board: a user board by `board.model`, otherwise the
    /// chip by model or FQBN. `None` for boards without a pin table.
    pub fn of_board(board: &BoardConfig, boards: &Boards) -> Option<Self> {
        board
            .model
            .as_deref()
            .and_then(|id| boards.get(id))
            .filter(|b| b.def.info.fqbn == board.fqbn)
            .and_then(UserBoard::layout)
            .or_else(|| Chip::of_board(board).map(Self::of_chip))
    }

    /// Chip family: its UART and ADC rules apply to the board.
    pub fn chip(&self) -> Chip {
        self.chip
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// GPIOs the board brings out, in ascending order.
    pub fn pins(&self) -> &[PinInfo] {
        &self.pins
    }

    pub fn pin(&self, gpio: i64) -> Option<&PinInfo> {
        self.pins.iter().find(|p| p.gpio == gpio)
    }
}

impl PinInfo {
    /// The pin as a board brings it out: named, and busy if the board says so.
    pub(crate) fn on_board(
        mut self,
        label: &str,
        busy: Option<&str>,
        functions: Option<&[PinFunction]>,
    ) -> Self {
        self.label = label.to_owned();
        self.only = functions.map(|f| f.iter().copied().collect());
        if let Some(reason) = busy {
            self.warning = Some(reason.to_owned());
        }
        self
    }
}

fn esp32() -> Vec<PinInfo> {
    let gpios = (0..=27).chain(32..=36).chain(39..=39);
    gpios
        .filter(|g| *g != 20 && *g != 24)
        .map(|gpio| {
            let mut p = PinInfo::new(gpio);
            p.adc = match gpio {
                32..=39 => 1,
                0 | 2 | 4 | 12..=15 | 25..=27 => 2,
                _ => 0,
            };
            p.input_only = gpio >= 34;
            p.dac = matches!(gpio, 25 | 26);
            p.flash = (6..=11).contains(&gpio);
            p.strapping = matches!(gpio, 0 | 2 | 5 | 12 | 15);
            if matches!(gpio, 1 | 3) {
                p.warning = Some(UART0.to_owned());
            }
            p
        })
        .collect()
}

fn esp32c3() -> Vec<PinInfo> {
    (0..=21)
        .map(|gpio| {
            let mut p = PinInfo::new(gpio);
            p.adc = match gpio {
                0..=4 => 1,
                5 => 2,
                _ => 0,
            };
            p.flash = (11..=17).contains(&gpio);
            p.strapping = matches!(gpio, 2 | 8 | 9);
            p.warning = match gpio {
                18 | 19 => Some(USB.to_owned()),
                20 | 21 => Some(UART0.to_owned()),
                _ => None,
            };
            p
        })
        .collect()
}

fn esp32c3_supermini() -> Vec<PinInfo> {
    esp32c3()
        .into_iter()
        .filter(|p| matches!(p.gpio, 0..=10 | 20 | 21))
        .map(|mut p| {
            p.warning = match p.gpio {
                8 => Some("встроенный светодиод (горит при 0)".to_owned()),
                9 => Some("кнопка BOOT".to_owned()),
                _ => p.warning,
            };
            p
        })
        .collect()
}

fn esp32s3() -> Vec<PinInfo> {
    (0..=21)
        .chain(26..=48)
        .map(|gpio| {
            let mut p = PinInfo::new(gpio);
            p.adc = match gpio {
                1..=10 => 1,
                11..=20 => 2,
                _ => 0,
            };
            p.flash = (26..=32).contains(&gpio);
            p.strapping = matches!(gpio, 0 | 3 | 45 | 46);
            p.warning = match gpio {
                19 | 20 => Some(USB.to_owned()),
                33..=37 => Some("занят PSRAM на модулях с октальной памятью".to_owned()),
                43 | 44 => Some(UART0.to_owned()),
                _ => None,
            };
            p
        })
        .collect()
}

/// A pin taken by a parameter of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinUse {
    pub block: String,
    pub kind: String,
    pub param: String,
    pub gpio: i64,
    pub mode: PinMode,
    pub shared: bool,
    /// Bus name of the parameter, see [`crate::ParamDef::bus`].
    pub bus: String,
}

impl PinUse {
    /// The two uses may stay on one pin: the same shared parameter of one block type,
    /// or parameters of any blocks on the same bus.
    pub fn can_share(&self, other: &PinUse) -> bool {
        if self.block == other.block {
            return false;
        }
        let same_bus = !self.bus.is_empty() && self.bus == other.bus;
        let same_param =
            self.shared && other.shared && self.kind == other.kind && self.param == other.param;
        same_bus || same_param
    }
}

/// Pins used by the blocks, in schema order. Parameters switched off by `when`
/// and negative values ("not used") are skipped; unknown block types and repeated
/// ids too (the check reports them on their own).
pub fn pin_uses(logic: &Logic, lib: &Library) -> Vec<PinUse> {
    let mut uses = Vec::new();
    let mut seen = HashSet::new();
    for block in &logic.blocks {
        // A copy of a tag uses the tag's pin.
        if block.tag.is_some() || !seen.insert(block.id.as_str()) {
            continue;
        }
        let Some(lib_block) = lib.resolve(logic, block) else {
            continue;
        };
        let def = &lib_block.def;
        // A block on an expander pin does not take a GPIO.
        if block.expander.is_some() && def.info.expander {
            continue;
        }
        for p in def.active_params(&block.params) {
            if p.kind != ParamKind::Pin {
                continue;
            }
            let Some(ParamValue::Int(gpio)) = block.params.get(&p.name).or(p.default.as_ref())
            else {
                continue;
            };
            if *gpio < 0 {
                continue;
            }
            uses.push(PinUse {
                block: block.id.clone(),
                kind: block.kind.clone(),
                param: p.name.clone(),
                gpio: *gpio,
                mode: p.mode,
                shared: p.shared,
                bus: p.bus.clone(),
            });
        }
    }
    uses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_from_fqbn() {
        assert_eq!(Chip::from_fqbn("esp32:esp32:esp32"), Some(Chip::Esp32));
        assert_eq!(
            Chip::from_fqbn("esp32:esp32:esp32doit-devkit-v1"),
            Some(Chip::Esp32)
        );
        assert_eq!(Chip::from_fqbn("esp32:esp32:esp32c3"), Some(Chip::Esp32C3));
        assert_eq!(
            Chip::from_fqbn("esp32:esp32:XIAO_ESP32S3"),
            Some(Chip::Esp32S3)
        );
        assert_eq!(Chip::from_fqbn("esp32:esp32:esp32c6"), None);
        assert_eq!(Chip::from_fqbn("arduino:avr:uno"), None);
    }

    #[test]
    fn board_model_picks_pins() {
        let mut board = BoardConfig {
            fqbn: "esp32:esp32:esp32c3".into(),
            model: Some("esp32c3_supermini".into()),
            port: None,
            options: Default::default(),
        };
        assert_eq!(Chip::of_board(&board), Some(Chip::Esp32C3SuperMini));
        let pins = Chip::Esp32C3SuperMini.pins();
        assert_eq!(pins.len(), 13);
        assert!(pins.iter().all(|p| !p.flash));
        assert!(Chip::Esp32C3SuperMini.pin(18).is_none());
        assert_eq!(Chip::Esp32C3SuperMini.pin(4).unwrap().adc, 1);
        // A model for another FQBN is ignored.
        board.fqbn = "esp32:esp32:esp32".into();
        assert_eq!(Chip::of_board(&board), Some(Chip::Esp32));
    }

    fn pin_use(block: &str, kind: &str, bus: &str, shared: bool) -> PinUse {
        PinUse {
            block: block.into(),
            kind: kind.into(),
            param: "sda".into(),
            gpio: 4,
            mode: PinMode::Io,
            shared,
            bus: bus.into(),
        }
    }

    #[test]
    fn bus_lets_different_blocks_share_a_pin() {
        let out = pin_use("relays", "pcf8574_out", "i2c-sda", false);
        let input = pin_use("inputs", "pcf8574_in", "i2c-sda", false);
        assert!(out.can_share(&input));
        // The same block never shares a pin with itself.
        assert!(!out.can_share(&pin_use("relays", "pcf8574_out", "i2c-sda", false)));
        // Another bus, or no bus at all.
        assert!(!out.can_share(&pin_use("inputs", "pcf8574_in", "i2c-scl", false)));
        assert!(!pin_use("a", "x", "", false).can_share(&pin_use("b", "y", "", false)));
        // Shared parameters of one block type still work without a bus.
        assert!(pin_use("a", "x", "", true).can_share(&pin_use("b", "x", "", true)));
        assert!(!pin_use("a", "x", "", true).can_share(&pin_use("b", "y", "", true)));
    }

    #[test]
    fn esp32_pins() {
        let chip = Chip::Esp32;
        assert!(chip.pin(6).unwrap().flash);
        assert!(!chip.pin(34).unwrap().fits(PinMode::Out));
        assert!(chip.pin(34).unwrap().fits(PinMode::Adc));
        assert!(!chip.pin(5).unwrap().fits(PinMode::Adc));
        assert!(chip.pin(37).is_none());
        assert!(chip.pin(40).is_none());
    }

    #[test]
    fn functions_follow_the_chip() {
        use PinFunction::*;
        let esp = Chip::Esp32;
        assert_eq!(esp.pin(25).unwrap().functions(), [Din, Dout, Pwm, Adc, Dac]);
        assert_eq!(esp.pin(36).unwrap().functions(), [Din, Adc]);
        assert_eq!(esp.pin(5).unwrap().functions(), [Din, Dout, Pwm]);
        assert!(esp.pin(6).unwrap().functions().is_empty());
        assert!(esp.pin(4).unwrap().fits(PinMode::Pwm));
        assert!(!esp.pin(34).unwrap().fits(PinMode::Pwm));
        assert!(!esp.pin(4).unwrap().fits(PinMode::Dac));
        for chip in [Chip::Esp32C3, Chip::Esp32S3] {
            assert!(chip.pins().iter().all(|p| !p.has(Dac)));
        }
    }

    #[test]
    fn a_board_can_only_narrow_the_chip() {
        use PinFunction::*;
        let pin = Chip::Esp32
            .pin(36)
            .unwrap()
            .on_board("A1", None, Some(&[Adc, Dout, Dac]));
        // Dout and Dac are not the chip's: asking for them gives nothing.
        assert_eq!(pin.functions(), [Adc]);
        assert!(pin.fits(PinMode::Adc));
        assert!(!pin.fits(PinMode::In));
        let pin = Chip::Esp32
            .pin(4)
            .unwrap()
            .on_board("D4", None, Some(&[Dout]));
        assert!(pin.fits(PinMode::Out) && !pin.fits(PinMode::Pwm) && !pin.fits(PinMode::Io));
    }
}
