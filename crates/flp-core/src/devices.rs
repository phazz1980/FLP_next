//! Peripherals of a user board: a `[[device]]` of `board.toml` becomes a block that is
//! a copy of a built-in one (`pcf8574_out`) with the parameters fixed by the board and
//! the channels named after the board's silkscreen (K1..K6). The copy shares the
//! template of its parent, so no files are written for it.

use std::collections::BTreeMap;

use crate::boards::{DeviceDef, UserBoard, BOARD_FILE};
use crate::check::{check_param, Report};
use crate::error::Error;
use crate::library::{Library, LibraryBlock, ESP32_CPP_TEMPLATE};
use crate::value::ParamValue;

/// Block type of a device: `<board id>_<device id>`.
pub fn device_kind(board_id: &str, device_id: &str) -> String {
    format!("{board_id}_{device_id}")
}

/// Blocks of all devices of a board, made from the built-in blocks of `lib`. A device
/// with a problem is skipped and explained, the others stay.
pub fn device_blocks(board: &UserBoard, lib: &Library) -> (Vec<LibraryBlock>, Vec<Error>) {
    let mut blocks = Vec::new();
    let mut errors = Vec::new();
    for dev in &board.def.devices {
        match device_block(board, dev, lib) {
            Ok(block) => blocks.push(block),
            Err(message) => errors.push(Error::InvalidBoard {
                path: board.dir.join(BOARD_FILE),
                message: format!("устройство «{}»: {message}", dev.id),
            }),
        }
    }
    (blocks, errors)
}

fn device_block(
    board: &UserBoard,
    dev: &DeviceDef,
    lib: &Library,
) -> std::result::Result<LibraryBlock, String> {
    let info = &board.def.info;
    let kind = device_kind(&info.id, &dev.id);
    if lib.get(&kind).is_some() {
        return Err(format!("тип блока «{kind}» уже занят"));
    }
    let parent = lib
        .get(&dev.kind)
        .filter(|b| lib.is_builtin(&dev.kind) && b.def.info.board.is_none())
        .ok_or_else(|| format!("нет встроенного блока «{}»", dev.kind))?;
    let template = parent
        .template(ESP32_CPP_TEMPLATE)
        .ok_or_else(|| format!("у блока «{}» нет шаблона кода", dev.kind))?
        .map_err(|e| format!("шаблон блока «{}» не прочитан: {e}", dev.kind))?;

    let mut def = parent.def.clone();
    let mut fixed: BTreeMap<String, ParamValue> = dev.params.clone();
    // As many ports as channels, when the block can hide the rest.
    if !dev.channels.is_empty() && def.param("count").is_some() {
        fixed
            .entry("count".to_owned())
            .or_insert(ParamValue::Int(dev.channels.len() as i64));
    }
    for (name, value) in &fixed {
        let p = def
            .params
            .iter_mut()
            .find(|p| &p.name == name)
            .ok_or_else(|| format!("у блока «{}» нет параметра «{name}»", dev.kind))?;
        let mut report = Report::default();
        check_param(&dev.id, p, value, &mut report);
        if let Some(e) = report.errors().next() {
            return Err(e.message.clone());
        }
        p.default = Some(value.clone());
        p.fixed = true;
    }
    for (i, channel) in dev.channels.iter().enumerate() {
        let port = format!("p{i}");
        let found = def
            .inputs
            .iter_mut()
            .chain(def.outputs.iter_mut())
            .find(|p| p.name == port);
        match found {
            Some(p) => p.label = Some(channel.clone()),
            None => {
                return Err(format!(
                    "канал «{channel}»: у блока «{}» нет порта {port}",
                    dev.kind
                ))
            }
        }
    }

    def.info.kind = kind.clone();
    def.info.name = format!("{}: {}", info.name, dev.name);
    def.info.symbol = Some(dev.name.clone());
    def.info.group = info.name.clone();
    if !dev.description.trim().is_empty() {
        def.info.description = dev.description.trim().to_owned();
    }
    // One chip on the board: a second block would write the same outputs.
    def.info.single = true;
    def.info.board = Some(info.id.clone());
    def.validate(&kind)?;
    Ok(LibraryBlock {
        def,
        dir: board.dir.clone(),
        inline_templates: [(ESP32_CPP_TEMPLATE, template)].into(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::boards::{BoardDef, BoardInfo, Boards};
    use crate::library::SignalType;

    fn builtin_library() -> Library {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
        let (lib, errors) = Library::load(dir).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        lib
    }

    fn relays() -> DeviceDef {
        DeviceDef {
            id: "relays".into(),
            kind: "pcf8574_out".into(),
            name: "Реле".into(),
            description: String::new(),
            channels: ["K1", "K2", "K3", "K4", "K5", "K6"]
                .map(String::from)
                .into(),
            params: BTreeMap::from([
                ("address".to_owned(), ParamValue::Int(0x24)),
                ("sda".to_owned(), ParamValue::Int(4)),
                ("scl".to_owned(), ParamValue::Int(15)),
                ("inverted".to_owned(), ParamValue::Bool(true)),
            ]),
        }
    }

    fn board(devices: Vec<DeviceDef>) -> UserBoard {
        UserBoard {
            def: BoardDef {
                info: BoardInfo {
                    id: "kc868".into(),
                    name: "KC868".into(),
                    chip: "esp32".into(),
                    fqbn: "esp32:esp32:esp32".into(),
                    version: 1,
                    options: BTreeMap::new(),
                },
                pins: Vec::new(),
                expander_pins: Vec::new(),
                devices,
            },
            dir: PathBuf::from("boards/kc868"),
        }
    }

    #[test]
    fn a_device_is_a_configured_copy_of_the_builtin_block() {
        let lib = builtin_library();
        let (blocks, errors) = device_blocks(&board(vec![relays()]), &lib);
        assert!(errors.is_empty(), "{errors:?}");
        let block = &blocks[0];
        let def = &block.def;
        assert_eq!(def.info.kind, "kc868_relays");
        assert_eq!(def.info.name, "KC868: Реле");
        assert_eq!(def.info.board.as_deref(), Some("kc868"));
        assert!(def.info.fits_board(Some("kc868")));
        assert!(!def.info.fits_board(Some("other")));
        assert!(!def.info.fits_board(None));
        assert!(def.info.single);
        for (name, value) in [
            ("address", ParamValue::Int(0x24)),
            ("sda", ParamValue::Int(4)),
            ("count", ParamValue::Int(6)),
            ("inverted", ParamValue::Bool(true)),
        ] {
            let p = def.param(name).unwrap();
            assert!(p.fixed, "{name}");
            assert_eq!(p.default.as_ref(), Some(&value), "{name}");
        }
        let ports: Vec<_> = def
            .active_inputs(&BTreeMap::new())
            .map(|p| (p.label.clone().unwrap_or_else(|| p.name.clone()), p.ty))
            .collect();
        assert_eq!(ports.len(), 6);
        assert_eq!(ports[0], ("K1".to_owned(), SignalType::Bool));
        assert_eq!(ports[5].0, "K6");
        assert!(block.template(ESP32_CPP_TEMPLATE).is_some());
    }

    #[test]
    fn a_device_with_a_problem_is_skipped_and_explained() {
        let lib = builtin_library();
        let mut unknown_block = relays();
        unknown_block.id = "a".into();
        unknown_block.kind = "no_such_block".into();
        let mut unknown_param = relays();
        unknown_param.id = "b".into();
        unknown_param
            .params
            .insert("speed".into(), ParamValue::Int(1));
        let mut bad_address = relays();
        bad_address.id = "c".into();
        bad_address
            .params
            .insert("address".into(), ParamValue::Int(0x50));
        let mut too_many = relays();
        too_many.id = "d".into();
        too_many
            .channels
            .extend(["K7", "K8", "K9"].map(String::from));

        let (blocks, errors) = device_blocks(
            &board(vec![
                unknown_block,
                unknown_param,
                bad_address,
                too_many,
                relays(),
            ]),
            &lib,
        );
        assert_eq!(blocks.len(), 1);
        let texts: Vec<_> = errors.iter().map(ToString::to_string).collect();
        assert_eq!(texts.len(), 4, "{texts:?}");
        assert!(texts[0].contains("нет встроенного блока"), "{}", texts[0]);
        assert!(texts[1].contains("нет параметра «speed»"), "{}", texts[1]);
        assert!(texts[2].contains("допустимо только"), "{}", texts[2]);
        assert!(texts[3].contains("больше допустимого"), "{}", texts[3]);
    }

    #[test]
    fn the_library_gets_and_loses_device_blocks_with_the_boards() {
        let mut lib = builtin_library();
        let builtin_count = lib.len();
        let mut boards = Boards::default();
        boards.insert(board(vec![relays()]));
        assert!(lib.set_boards(boards).is_empty());
        assert_eq!(lib.len(), builtin_count + 1);
        assert!(lib.get("kc868_relays").is_some());
        // A device block is not a user block: it cannot be edited as one.
        assert!(!lib.is_user("kc868_relays"));
        assert!(lib.set_boards(Boards::default()).is_empty());
        assert_eq!(lib.len(), builtin_count);
    }
}
