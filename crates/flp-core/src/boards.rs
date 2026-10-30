//! User boards: `user_library/boards/<id>/board.toml` describes a board on top of an
//! ESP32 chip family: the FQBN, the pins it brings out and what is wired to each of
//! them. A board is read on every library load, so new ones need no recompilation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::expander::{Expander, ExpanderChip};
use crate::pins::{BoardExpander, BoardExpanderPin, BoardLayout, Chip, PinFunction, BOARD_MODELS};
use crate::toml_io::{read_toml, to_toml};
use crate::value::ParamValue;

/// Folder of the boards inside the user library.
pub const BOARDS_DIR: &str = "boards";
/// Description file in a board folder.
pub const BOARD_FILE: &str = "board.toml";
/// Text of the board (Markdown, pictures from `images/`).
pub const DOCS_FILE: &str = "docs.md";
/// Pictures and schemes of the board.
pub const IMAGES_DIR: &str = "images";

/// Contents of `board.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardDef {
    #[serde(rename = "board")]
    pub info: BoardInfo,
    /// Pins the board brings out; empty means all pins of the chip.
    #[serde(rename = "pin", default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<BoardPin>,
    /// Pins of the expanders soldered on the board (`device` names a `[[device]]`).
    #[serde(
        rename = "expander_pin",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub expander_pins: Vec<BoardExpanderPinDef>,
    /// Peripherals soldered on the board: ready-made blocks, see [`crate::devices`].
    #[serde(rename = "device", default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DeviceDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardInfo {
    /// Value of `board.model` in `project.toml`; must match the folder name.
    pub id: String,
    /// Display name in Russian.
    pub name: String,
    /// Chip family: `esp32`, `esp32c3` or `esp32s3`. Its UART and ADC rules apply.
    pub chip: String,
    /// Board name for arduino-cli, e.g. `esp32:esp32:esp32`.
    pub fqbn: String,
    #[serde(default = "first_version", skip_serializing_if = "is_first_version")]
    pub version: u32,
    /// Board menu options of arduino-cli used by default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// One pin of the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardPin {
    pub gpio: i64,
    /// What is wired to the pin ("A1", "RS485 RX"): shown next to the GPIO number.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Longer remark for the board editor ("analog input 0–5 V").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The pin is taken on the board; the reason shows up as a warning when a block uses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy: Option<String>,
    /// What the board lets blocks do with the pin; never more than the chip can.
    /// `None` — everything the chip can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<PinFunction>>,
}

/// One pin of an expander chip that the board brings out: pin `pin` of the chip behind
/// the `[[device]]` named `device`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardExpanderPinDef {
    pub device: String,
    pub pin: i64,
    /// What is printed on the board ("K1").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// `din` and/or `dout`; `None` — both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<PinFunction>>,
}

/// A peripheral soldered on the board (relays, optoisolated inputs): a copy of a
/// built-in block whose parameters the board has fixed, so a project picks "Реле"
/// instead of setting an I²C address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceDef {
    /// Short id inside the board; the block type is `<board id>_<id>`.
    pub id: String,
    /// Built-in block the device is made from (`pcf8574_out`).
    pub kind: String,
    /// Display name of the block ("Реле"); the library shows it as "<board>: <name>".
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Names of the channels in order: channel `i` captions port `p<i>`, and the block
    /// gets as many ports as there are names (`count`, if the block has it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<String>,
    /// Fixed parameters of the block (`address`, `sda`, `scl`, `inverted`, ...).
    #[serde(flatten)]
    pub params: BTreeMap<String, ParamValue>,
}

impl DeviceDef {
    /// The expander chip behind the device: the built-in PCF8574 blocks carry its
    /// address and bus pins as fixed parameters.
    pub fn expander(&self) -> Option<Expander> {
        let chip = match self.kind.as_str() {
            "pcf8574_out" | "pcf8574_in" => ExpanderChip::Pcf8574,
            _ => return None,
        };
        let int = |name: &str| match self.params.get(name) {
            Some(ParamValue::Int(i)) => Some(*i),
            _ => None,
        };
        let address = u8::try_from(int("address")?).ok()?;
        let mut e = Expander::new(self.id.clone(), chip, address);
        e.sda = int("sda").unwrap_or(-1);
        e.scl = int("scl").unwrap_or(-1);
        Some(e)
    }
}

fn first_version() -> u32 {
    1
}

fn is_first_version(v: &u32) -> bool {
    *v == 1
}

/// The id is the folder name and the value in project files: plain ASCII.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl BoardDef {
    /// Problems of the description (in Russian), `Ok` if the board is usable.
    /// `folder` is the name of the folder it lives in, if it has one.
    pub fn validate(&self, folder: Option<&str>) -> std::result::Result<(), String> {
        let info = &self.info;
        if !valid_id(&info.id) {
            return Err(format!(
                "id «{}» должен состоять из латинских букв a–z, цифр и «_»",
                info.id
            ));
        }
        if folder.is_some_and(|f| f != info.id) {
            return Err(format!(
                "id «{}» не совпадает с именем папки «{}»",
                info.id,
                folder.unwrap_or_default()
            ));
        }
        if BOARD_MODELS.iter().any(|m| m.id == info.id) {
            return Err(format!("id «{}» занят встроенной платой", info.id));
        }
        if info.name.trim().is_empty() {
            return Err("у платы нет названия".to_owned());
        }
        let Some(chip) = Chip::from_id(&info.chip) else {
            return Err(format!(
                "неизвестный чип «{}» (есть esp32, esp32c3, esp32s3)",
                info.chip
            ));
        };
        if info.fqbn.trim().is_empty() {
            return Err("не указан FQBN платы".to_owned());
        }
        if Chip::from_fqbn(&info.fqbn).is_some_and(|c| c != chip) {
            return Err(format!("FQBN «{}» не от чипа {}", info.fqbn, chip.name()));
        }
        let mut devices = BTreeSet::new();
        for dev in &self.devices {
            if !valid_id(&dev.id) {
                return Err(format!(
                    "id устройства «{}» должен состоять из латинских букв a–z, цифр и «_»",
                    dev.id
                ));
            }
            if !devices.insert(dev.id.as_str()) {
                return Err(format!("устройство «{}» описано дважды", dev.id));
            }
            if dev.kind.trim().is_empty() || dev.name.trim().is_empty() {
                return Err(format!(
                    "у устройства «{}» нужны тип блока и название",
                    dev.id
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for pin in &self.pins {
            let Some(info) = chip.pin(pin.gpio) else {
                return Err(format!("у {} нет вывода GPIO{}", chip.name(), pin.gpio));
            };
            if let Some(f) = pin.functions.iter().flatten().find(|f| !info.chip_can(**f)) {
                return Err(format!(
                    "у GPIO{} нет функции «{}» у {}",
                    pin.gpio,
                    f.label(),
                    chip.name()
                ));
            }
            if !seen.insert(pin.gpio) {
                return Err(format!("вывод GPIO{} описан дважды", pin.gpio));
            }
        }
        let mut seen_expander = BTreeSet::new();
        for pin in &self.expander_pins {
            let Some(e) = self
                .devices
                .iter()
                .find(|d| d.id == pin.device)
                .map(|d| d.expander())
            else {
                return Err(format!(
                    "вывод расширителя P{}: нет устройства «{}»",
                    pin.pin, pin.device
                ));
            };
            let Some(e) = e else {
                return Err(format!(
                    "устройство «{}» не расширитель: нужен блок PCF8574 с адресом",
                    pin.device
                ));
            };
            if let Some(problem) = e.problem() {
                return Err(format!("устройство «{}»: {problem}", pin.device));
            }
            if pin.pin < 0 || pin.pin >= i64::from(e.chip.pins()) {
                return Err(format!(
                    "у {} устройства «{}» нет вывода P{}",
                    e.chip.name(),
                    pin.device,
                    pin.pin
                ));
            }
            if let Some(f) = pin
                .functions
                .iter()
                .flatten()
                .find(|f| !matches!(f, PinFunction::Din | PinFunction::Dout))
            {
                return Err(format!(
                    "вывод P{} устройства «{}»: расширитель не умеет «{}», только вход и выход",
                    pin.pin,
                    pin.device,
                    f.label()
                ));
            }
            if !seen_expander.insert((pin.device.as_str(), pin.pin)) {
                return Err(format!(
                    "вывод P{} устройства «{}» описан дважды",
                    pin.pin, pin.device
                ));
            }
        }
        Ok(())
    }

    /// Expanders of the board whose pins the author listed, in the order of the devices.
    pub fn expanders(&self) -> Vec<BoardExpander> {
        self.devices
            .iter()
            .filter_map(|d| {
                let expander = d.expander()?;
                let mut pins: Vec<BoardExpanderPin> = self
                    .expander_pins
                    .iter()
                    .filter(|p| p.device == d.id)
                    .map(|p| BoardExpanderPin {
                        pin: p.pin,
                        label: p.name.trim().to_owned(),
                        functions: p.functions.as_deref().map_or_else(
                            || [PinFunction::Din, PinFunction::Dout].into(),
                            |f| f.iter().copied().collect(),
                        ),
                    })
                    .collect();
                if pins.is_empty() {
                    return None;
                }
                pins.sort_by_key(|p| p.pin);
                Some(BoardExpander {
                    title: d.name.clone(),
                    expander,
                    pins,
                })
            })
            .collect()
    }

    /// Chip family of the board.
    pub fn chip(&self) -> Option<Chip> {
        Chip::from_id(&self.info.chip)
    }

    /// Pin table: the board's pins with the chip's capabilities, or all pins of the chip.
    pub fn layout(&self) -> Option<BoardLayout> {
        let chip = self.chip()?;
        let pins = if self.pins.is_empty() {
            chip.pins()
        } else {
            let mut pins: Vec<_> = self
                .pins
                .iter()
                .filter_map(|p| {
                    let busy = p.busy.as_deref().filter(|b| !b.trim().is_empty());
                    Some(
                        chip.pin(p.gpio)?
                            .on_board(p.name.trim(), busy, p.functions.as_deref()),
                    )
                })
                .collect();
            pins.sort_by_key(|p| p.gpio);
            pins
        };
        Some(BoardLayout::new(chip, self.info.name.clone(), pins).with_expanders(self.expanders()))
    }
}

/// A board loaded from a folder.
#[derive(Debug, Clone, PartialEq)]
pub struct UserBoard {
    pub def: BoardDef,
    pub dir: PathBuf,
}

impl UserBoard {
    /// Reads and checks `<dir>/board.toml`.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(BOARD_FILE);
        let def: BoardDef = read_toml(&path)?;
        let folder = dir.file_name().and_then(|n| n.to_str());
        def.validate(folder)
            .map_err(|message| Error::InvalidBoard {
                path: path.clone(),
                message,
            })?;
        Ok(Self {
            def,
            dir: dir.to_owned(),
        })
    }

    pub fn layout(&self) -> Option<BoardLayout> {
        self.def.layout()
    }

    /// Text of the board, if it has one.
    pub fn docs_path(&self) -> Option<PathBuf> {
        let path = self.dir.join(DOCS_FILE);
        path.is_file().then_some(path)
    }
}

/// Boards of the user library by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Boards {
    boards: BTreeMap<String, UserBoard>,
}

impl Boards {
    /// Loads every board folder of `dir` (`user_library/boards`). A broken board is
    /// skipped and its error returned alongside, so the others keep working.
    pub fn load(dir: &Path) -> (Self, Vec<Error>) {
        let mut boards = Self::default();
        let mut errors = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return (boards, errors);
        };
        let mut folders: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join(BOARD_FILE).is_file())
            .collect();
        folders.sort();
        for folder in folders {
            match UserBoard::load(&folder) {
                Ok(board) => {
                    boards.boards.insert(board.def.info.id.clone(), board);
                }
                Err(e) => errors.push(e),
            }
        }
        (boards, errors)
    }

    pub fn get(&self, id: &str) -> Option<&UserBoard> {
        self.boards.get(id)
    }

    /// Adds a board that is not on disk (or replaces the one with the same id).
    pub fn insert(&mut self, board: UserBoard) {
        self.boards.insert(board.def.info.id.clone(), board);
    }

    pub fn iter(&self) -> impl Iterator<Item = &UserBoard> {
        self.boards.values()
    }

    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }
}

/// Folder with the boards inside the user library `user_dir`.
pub fn boards_dir(user_dir: &Path) -> PathBuf {
    user_dir.join(BOARDS_DIR)
}

/// Writes `board.toml` of a board into `<user_dir>/boards/<id>/` after checking it.
/// Other files of the folder (text, pictures) stay as they are.
pub fn save_board(user_dir: &Path, def: &BoardDef) -> Result<PathBuf> {
    let dir = boards_dir(user_dir).join(&def.info.id);
    let path = dir.join(BOARD_FILE);
    def.validate(None).map_err(|message| Error::InvalidBoard {
        path: path.clone(),
        message,
    })?;
    let write_err = |path: &Path| {
        let path = path.to_owned();
        move |source| Error::Write { path, source }
    };
    fs::create_dir_all(&dir).map_err(write_err(&dir))?;
    fs::write(&path, to_toml(def, &path)?).map_err(write_err(&path))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> BoardDef {
        BoardDef {
            info: BoardInfo {
                id: "kc868_a6".into(),
                name: "Kincony KC868-A6".into(),
                chip: "esp32".into(),
                fqbn: "esp32:esp32:esp32".into(),
                version: 1,
                options: BTreeMap::new(),
            },
            pins: vec![
                BoardPin {
                    gpio: 39,
                    name: "A2".into(),
                    note: String::new(),
                    busy: None,
                    functions: None,
                },
                BoardPin {
                    gpio: 15,
                    name: "SCL".into(),
                    note: "I2C".into(),
                    busy: Some("шина I2C расширителей".into()),
                    functions: None,
                },
            ],
            expander_pins: Vec::new(),
            devices: Vec::new(),
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flp_boards_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn layout_names_the_pins_and_keeps_chip_flags() {
        let layout = def().layout().unwrap();
        assert_eq!(layout.name(), "Kincony KC868-A6");
        assert_eq!(layout.chip(), Chip::Esp32);
        let gpios: Vec<_> = layout.pins().iter().map(|p| p.gpio).collect();
        assert_eq!(gpios, [15, 39]);
        let a2 = layout.pin(39).unwrap();
        assert_eq!(a2.title(), "A2 (GPIO39)");
        assert!(a2.input_only);
        assert_eq!(a2.adc, 1);
        let scl = layout.pin(15).unwrap();
        assert!(scl.strapping);
        assert_eq!(scl.warning.as_deref(), Some("шина I2C расширителей"));
        assert!(layout.pin(4).is_none());
    }

    #[test]
    fn a_board_without_pins_has_all_pins_of_its_chip() {
        let mut d = def();
        d.pins.clear();
        assert_eq!(d.layout().unwrap().pins().len(), Chip::Esp32.pins().len());
    }

    #[test]
    fn functions_are_written_when_given_and_read_back() {
        let mut d = def();
        d.pins[0].functions = Some(vec![PinFunction::Din, PinFunction::Adc]);
        let text = toml::to_string(&d).unwrap();
        assert!(text.contains("functions = [\"din\", \"adc\"]"), "{text}");
        assert_eq!(text.matches("functions").count(), 1);
        assert_eq!(toml::from_str::<BoardDef>(&text).unwrap(), d);
        // a file from before the field is read as before
        let old = text.replace("functions = [\"din\", \"adc\"]\n", "");
        let back: BoardDef = toml::from_str(&old).unwrap();
        assert_eq!(back.pins[0].functions, None);
        assert!(back
            .layout()
            .unwrap()
            .pin(39)
            .unwrap()
            .has(PinFunction::Din));
    }

    #[test]
    fn expander_pins_round_trip_and_reach_the_layout() {
        use crate::value::ParamValue;
        let mut d = def();
        d.devices.push(DeviceDef {
            id: "relays".into(),
            kind: "pcf8574_out".into(),
            name: "Реле".into(),
            description: String::new(),
            channels: Vec::new(),
            params: [
                ("address".to_owned(), ParamValue::Int(0x24)),
                ("sda".to_owned(), ParamValue::Int(4)),
                ("scl".to_owned(), ParamValue::Int(15)),
            ]
            .into(),
        });
        d.expander_pins = vec![
            BoardExpanderPinDef {
                device: "relays".into(),
                pin: 1,
                name: "K2".into(),
                note: String::new(),
                functions: Some(vec![PinFunction::Dout]),
            },
            BoardExpanderPinDef {
                device: "relays".into(),
                pin: 0,
                name: "K1".into(),
                note: String::new(),
                functions: None,
            },
        ];
        assert_eq!(d.validate(None), Ok(()));
        let text = toml::to_string(&d).unwrap();
        assert!(text.contains("[[expander_pin]]"), "{text}");
        assert_eq!(toml::from_str::<BoardDef>(&text).unwrap(), d);
        let layout = d.layout().unwrap();
        let [board] = layout.expanders() else {
            panic!("one expander expected");
        };
        assert_eq!(board.title, "Реле");
        assert_eq!(
            (
                board.expander.address,
                board.expander.sda,
                board.expander.scl
            ),
            (0x24, 4, 15)
        );
        let pins: Vec<_> = board
            .pins
            .iter()
            .map(|p| (p.pin, p.label.as_str()))
            .collect();
        assert_eq!(pins, [(0, "K1"), (1, "K2")]);
        assert!(board.fits(0, crate::PinMode::In) && board.fits(0, crate::PinMode::Out));
        assert!(board.fits(1, crate::PinMode::Out) && !board.fits(1, crate::PinMode::In));
        assert!(!board.fits(2, crate::PinMode::Out));
        // The project's expander on the same chip is the board's one, another address is not.
        let mut same = board.expander.clone();
        same.id = "mine".into();
        assert!(layout.expander_for(&same).is_some());
        same.address = 0x25;
        assert!(layout.expander_for(&same).is_none());
        // A file from before the table is read as before and offers no expanders.
        let old = toml::to_string(&def()).unwrap();
        assert!(!old.contains("expander_pin"));
        assert!(toml::from_str::<BoardDef>(&old)
            .unwrap()
            .layout()
            .unwrap()
            .expanders()
            .is_empty());
    }

    #[test]
    fn expander_pins_are_checked_in_russian() {
        use crate::value::ParamValue;
        let mut d = def();
        let pin = |device: &str, pin, functions| BoardExpanderPinDef {
            device: device.into(),
            pin,
            name: String::new(),
            note: String::new(),
            functions,
        };
        d.expander_pins = vec![pin("relays", 0, None)];
        assert!(d
            .validate(None)
            .unwrap_err()
            .contains("нет устройства «relays»"));
        d.devices.push(DeviceDef {
            id: "relays".into(),
            kind: "pcf8574_out".into(),
            name: "Реле".into(),
            description: String::new(),
            channels: Vec::new(),
            params: [("address".to_owned(), ParamValue::Int(0x24))].into(),
        });
        d.expander_pins = vec![pin("relays", 8, None)];
        assert!(d.validate(None).unwrap_err().contains("нет вывода P8"));
        d.expander_pins = vec![pin("relays", 0, Some(vec![PinFunction::Pwm]))];
        assert!(d
            .validate(None)
            .unwrap_err()
            .contains("только вход и выход"));
        d.expander_pins = vec![pin("relays", 0, None), pin("relays", 0, None)];
        assert!(d.validate(None).unwrap_err().contains("описан дважды"));
        // A device that is not an expander cannot have pins.
        d.devices[0].kind = "something_else".into();
        d.expander_pins = vec![pin("relays", 0, None)];
        assert!(d.validate(None).unwrap_err().contains("не расширитель"));
    }

    #[test]
    fn the_board_narrows_functions_but_never_widens_them() {
        let mut d = def();
        d.pins[0].functions = Some(vec![PinFunction::Adc]);
        let layout = d.layout().unwrap();
        let a2 = layout.pin(39).unwrap();
        assert_eq!(a2.functions(), [PinFunction::Adc]);
        assert!(a2.fits(crate::PinMode::Adc) && !a2.fits(crate::PinMode::In));
        d.pins[0].functions = Some(vec![PinFunction::Din, PinFunction::Dout]);
        let err = d.validate(None).unwrap_err();
        assert!(
            err.contains("GPIO39") && err.contains("цифровой выход"),
            "{err}"
        );
    }

    #[test]
    fn broken_descriptions_are_explained() {
        let mut d = def();
        d.info.id = "Плата".into();
        assert!(d.validate(None).unwrap_err().contains("латинских"));
        let mut d = def();
        assert!(d.validate(Some("other")).unwrap_err().contains("папки"));
        d.info.id = "esp32c3_supermini".into();
        assert!(d.validate(None).unwrap_err().contains("встроенной"));
        let mut d = def();
        d.info.chip = "avr".into();
        assert!(d.validate(None).unwrap_err().contains("чип"));
        let mut d = def();
        d.info.fqbn = "esp32:esp32:esp32c3".into();
        assert!(d.validate(None).unwrap_err().contains("FQBN"));
        let mut d = def();
        d.pins[0].gpio = 107;
        assert!(d.validate(None).unwrap_err().contains("нет вывода GPIO107"));
        let mut d = def();
        d.pins[1].gpio = 39;
        assert!(d.validate(None).unwrap_err().contains("дважды"));
    }

    #[test]
    fn saved_boards_are_loaded_and_broken_ones_do_not_stop_the_rest() {
        let user = temp("load");
        save_board(&user, &def()).unwrap();
        let bad = boards_dir(&user).join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join(BOARD_FILE), "[board]\nid = \"bad\"\n").unwrap();
        let (boards, errors) = Boards::load(&boards_dir(&user));
        assert_eq!(boards.iter().count(), 1);
        let board = boards.get("kc868_a6").unwrap();
        assert_eq!(board.def, def());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].to_string().contains("bad"));
        fs::remove_dir_all(&user).unwrap();
    }

    #[test]
    fn saving_refuses_a_broken_board() {
        let user = temp("refuse");
        let mut d = def();
        d.info.name.clear();
        assert!(save_board(&user, &d).is_err());
        assert!(!boards_dir(&user).exists());
        fs::remove_dir_all(&user).unwrap();
    }
}
