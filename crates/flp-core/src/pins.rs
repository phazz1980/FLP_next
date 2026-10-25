//! Board pins: which GPIOs a chip has, what each of them can do, and which
//! blocks of a schema use them. The check and the editor's pin list share it.

use std::collections::HashSet;

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

/// One GPIO of a chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinInfo {
    pub gpio: i64,
    /// ADC unit (1 or 2), 0 without an ADC.
    pub adc: u8,
    pub input_only: bool,
    /// Wired to the flash chip: using it hangs the board.
    pub flash: bool,
    /// Sampled at reset to choose the boot mode; an external pull may stop booting.
    pub strapping: bool,
    /// Usable, but busy with something else on most boards (reason in Russian).
    pub warning: Option<&'static str>,
}

impl PinInfo {
    const fn new(gpio: i64) -> Self {
        Self {
            gpio,
            adc: 0,
            input_only: false,
            flash: false,
            strapping: false,
            warning: None,
        }
    }

    /// The pin can serve a parameter of this mode.
    pub fn fits(&self, mode: PinMode) -> bool {
        !self.flash
            && match mode {
                PinMode::In => true,
                PinMode::Io | PinMode::Out => !self.input_only,
                PinMode::Adc => self.adc != 0,
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
        if let Some(w) = self.warning {
            notes.push(w.to_owned());
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
            p.flash = (6..=11).contains(&gpio);
            p.strapping = matches!(gpio, 0 | 2 | 5 | 12 | 15);
            if matches!(gpio, 1 | 3) {
                p.warning = Some(UART0);
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
                18 | 19 => Some(USB),
                20 | 21 => Some(UART0),
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
                8 => Some("встроенный светодиод (горит при 0)"),
                9 => Some("кнопка BOOT"),
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
                19 | 20 => Some(USB),
                33..=37 => Some("занят PSRAM на модулях с октальной памятью"),
                43 | 44 => Some(UART0),
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
}

impl PinUse {
    /// The two uses may stay on one pin: the same shared parameter of one block type.
    pub fn can_share(&self, other: &PinUse) -> bool {
        self.shared
            && other.shared
            && self.kind == other.kind
            && self.param == other.param
            && self.block != other.block
    }
}

/// Pins used by the blocks, in schema order. Parameters switched off by `when`
/// and negative values ("not used") are skipped; unknown block types and repeated
/// ids too (the check reports them on their own).
pub fn pin_uses(logic: &Logic, lib: &Library) -> Vec<PinUse> {
    let mut uses = Vec::new();
    let mut seen = HashSet::new();
    for block in &logic.blocks {
        if !seen.insert(block.id.as_str()) {
            continue;
        }
        let Some(lib_block) = lib.resolve(logic, block) else {
            continue;
        };
        let def = &lib_block.def;
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
}
