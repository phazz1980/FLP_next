//! Port expanders of a project: PCF8574 / PCF8574A / PCF8575 on the I²C bus.
//!
//! An expander is declared once in `logic.toml` (`[[expander]]`). A digital input or
//! output then stands on one of its pins instead of a GPIO: the block carries
//! `expander = "<id>"` and its `pin` parameter is the pin number of the chip.

use std::collections::{BTreeSet, HashSet};
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::ident::is_valid_ident;
use crate::library::{ParamKind, PinMode};
use crate::value::parse_duration_ms;
use crate::{Library, Logic, ParamValue};

/// Chip of an expander.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpanderChip {
    /// 8 pins, addresses 0x20–0x27.
    #[default]
    Pcf8574,
    /// 8 pins, addresses 0x38–0x3F.
    Pcf8574a,
    /// 16 pins, addresses 0x20–0x27.
    Pcf8575,
}

impl ExpanderChip {
    pub const ALL: [ExpanderChip; 3] = [Self::Pcf8574, Self::Pcf8574a, Self::Pcf8575];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pcf8574 => "PCF8574",
            Self::Pcf8574a => "PCF8574A",
            Self::Pcf8575 => "PCF8575",
        }
    }

    /// Number of pins (`P0`…).
    pub fn pins(self) -> u8 {
        match self {
            Self::Pcf8574 | Self::Pcf8574a => 8,
            Self::Pcf8575 => 16,
        }
    }

    /// I²C addresses set by the jumpers A0–A2.
    pub fn addresses(self) -> RangeInclusive<u8> {
        match self {
            Self::Pcf8574 | Self::Pcf8575 => 0x20..=0x27,
            Self::Pcf8574a => 0x38..=0x3f,
        }
    }
}

fn default_period() -> String {
    "20ms".to_owned()
}

fn is_default_period(s: &String) -> bool {
    *s == default_period()
}

fn no_pin() -> i64 {
    -1
}

fn is_no_pin(v: &i64) -> bool {
    *v == -1
}

/// One expander chip of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expander {
    /// Name that blocks refer to (`relays`); the same alphabet as block ids.
    pub id: String,
    pub chip: ExpanderChip,
    /// I²C address.
    pub address: u8,
    /// Bus pins; `-1` for both means the default pins of the board.
    #[serde(default = "no_pin", skip_serializing_if = "is_no_pin")]
    pub sda: i64,
    #[serde(default = "no_pin", skip_serializing_if = "is_no_pin")]
    pub scl: i64,
    /// How often the inputs are read (`20ms`).
    #[serde(default = "default_period", skip_serializing_if = "is_default_period")]
    pub period: String,
}

impl Expander {
    /// A new expander on the default bus pins.
    pub fn new(id: impl Into<String>, chip: ExpanderChip, address: u8) -> Self {
        Self {
            id: id.into(),
            chip,
            address,
            sda: -1,
            scl: -1,
            period: default_period(),
        }
    }

    /// First address of the chip, for a fresh expander.
    pub fn default_address(chip: ExpanderChip) -> u8 {
        *chip.addresses().start()
    }

    /// Period of reading the inputs in milliseconds.
    pub fn period_ms(&self) -> Option<u64> {
        parse_duration_ms(&self.period).filter(|ms| *ms >= 1)
    }

    /// What is wrong with the declaration itself, in Russian.
    pub fn problem(&self) -> Option<String> {
        let id = &self.id;
        if !is_valid_ident(id) {
            return Some(format!(
                "расширитель «{id}»: имя может содержать буквы, цифры, «_» и «-»"
            ));
        }
        if !self.chip.addresses().contains(&self.address) {
            let r = self.chip.addresses();
            return Some(format!(
                "расширитель «{id}»: адрес 0x{:02X} не подходит для {}, допустимы 0x{:02X}–0x{:02X}",
                self.address,
                self.chip.name(),
                r.start(),
                r.end()
            ));
        }
        if (self.sda < 0) != (self.scl < 0) {
            return Some(format!(
                "расширитель «{id}»: выводы SDA и SCL задаются вместе или оба -1 (по умолчанию)"
            ));
        }
        if self.sda >= 0 && self.sda == self.scl {
            return Some(format!(
                "расширитель «{id}»: SDA и SCL не могут быть одним выводом"
            ));
        }
        if self.period_ms().is_none() {
            return Some(format!(
                "расширитель «{id}»: период опроса «{}» не разобран, пример: \"20ms\"",
                self.period
            ));
        }
        None
    }
}

impl Logic {
    pub fn expander(&self, id: &str) -> Option<&Expander> {
        self.expanders.iter().find(|e| e.id == id)
    }
}

/// A block standing on a pin of an expander.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpanderUse {
    pub block: String,
    pub expander: String,
    /// Pin number on the chip.
    pub pin: i64,
    /// `In` for a block that reads the pin, `Out` for one that drives it.
    pub mode: PinMode,
}

/// Name of the `pin` parameter that moves to the expander: the first pin parameter of
/// the block.
pub fn pin_param(def: &crate::BlockDef) -> Option<&crate::ParamDef> {
    def.params.iter().find(|p| p.kind == ParamKind::Pin)
}

/// Blocks standing on expander pins, in schema order. Blocks whose type cannot stand
/// on an expander, and repeated ids, are skipped (the check reports them itself).
pub fn expander_uses(logic: &Logic, lib: &Library) -> Vec<ExpanderUse> {
    let mut uses = Vec::new();
    let mut seen = HashSet::new();
    for block in &logic.blocks {
        let Some(expander) = &block.expander else {
            continue;
        };
        if block.tag.is_some() || !seen.insert(block.id.as_str()) {
            continue;
        }
        let Some(lib_block) = lib.resolve(logic, block) else {
            continue;
        };
        let def = &lib_block.def;
        if !def.info.expander {
            continue;
        }
        let Some(p) = pin_param(def) else { continue };
        let Some(ParamValue::Int(pin)) = block.params.get(&p.name).or(p.default.as_ref()) else {
            continue;
        };
        uses.push(ExpanderUse {
            block: block.id.clone(),
            expander: expander.clone(),
            pin: *pin,
            mode: p.mode,
        });
    }
    uses
}

/// Pins of an expander taken by other blocks than `except` (a block being edited),
/// with the block that holds each.
pub fn taken_pins(
    logic: &Logic,
    lib: &Library,
    expander: &str,
    except: Option<&str>,
) -> Vec<(i64, String)> {
    expander_uses(logic, lib)
        .into_iter()
        .filter(|u| u.expander == expander && Some(u.block.as_str()) != except)
        .map(|u| (u.pin, u.block))
        .collect()
}

/// An id for a new expander: `pcf1`, `pcf2`… not used yet.
pub fn free_expander_id(logic: &Logic) -> String {
    let used: BTreeSet<&str> = logic.expanders.iter().map(|e| e.id.as_str()).collect();
    (1..)
        .map(|n| format!("pcf{n}"))
        .find(|id| !used.contains(id.as_str()))
        .expect("infinite range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_know_their_pins_and_addresses() {
        assert_eq!(ExpanderChip::Pcf8574.pins(), 8);
        assert_eq!(ExpanderChip::Pcf8575.pins(), 16);
        assert!(ExpanderChip::Pcf8574.addresses().contains(&0x27));
        assert!(!ExpanderChip::Pcf8574.addresses().contains(&0x38));
        assert!(ExpanderChip::Pcf8574a.addresses().contains(&0x3f));
    }

    #[test]
    fn declarations_are_checked() {
        let ok = Expander::new("relays", ExpanderChip::Pcf8574, 0x24);
        assert_eq!(ok.problem(), None);

        let mut bad = ok.clone();
        bad.address = 0x50;
        assert!(bad.problem().unwrap().contains("0x50"));

        let mut bad = ok.clone();
        bad.sda = 4;
        assert!(bad.problem().unwrap().contains("SDA и SCL"));

        let mut bad = ok.clone();
        bad.sda = 4;
        bad.scl = 4;
        assert!(bad.problem().unwrap().contains("одним выводом"));

        let mut bad = ok.clone();
        bad.period = "быстро".into();
        assert!(bad.problem().unwrap().contains("период"));

        let mut bad = ok;
        bad.id = "a b".into();
        assert!(bad.problem().unwrap().contains("имя"));
    }

    #[test]
    fn toml_roundtrip_keeps_defaults_out() {
        let e = Expander::new("relays", ExpanderChip::Pcf8575, 0x21);
        let logic = Logic {
            expanders: vec![e.clone()],
            ..Default::default()
        };
        let text = toml::to_string(&logic).unwrap();
        assert!(text.contains("chip = \"pcf8575\""), "{text}");
        assert!(!text.contains("sda"), "{text}");
        assert!(!text.contains("period"), "{text}");
        let back: Logic = toml::from_str(&text).unwrap();
        assert_eq!(back.expanders, vec![e]);
    }

    #[test]
    fn free_ids_do_not_repeat() {
        let mut logic = Logic::default();
        assert_eq!(free_expander_id(&logic), "pcf1");
        logic
            .expanders
            .push(Expander::new("pcf1", ExpanderChip::Pcf8574, 0x20));
        assert_eq!(free_expander_id(&logic), "pcf2");
    }
}
