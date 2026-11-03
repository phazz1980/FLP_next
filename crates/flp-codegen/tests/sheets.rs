//! Sheets as modules: the border between sheets (tags) and the errors.

use std::path::PathBuf;

use flp_codegen::{Program, SheetError, TagKind};
use flp_core::{Library, ParamValue, Project};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn library() -> Library {
    let (lib, errors) = Library::load(repo_path("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    lib
}

fn background() -> Project {
    Project::load(repo_path("examples/background")).unwrap()
}

#[test]
fn border_of_two_sheets() {
    let lib = library();
    let mut project = background();
    project.logic.vars[0].retain = false;
    let program = Program::build(&project, &lib).unwrap();
    assert_eq!(program.sheets(), vec![1, 2]);

    // The variable (used on both sheets, written on 2) comes first, then press.out.
    let table = program.tag_table();
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].kind, TagKind::Var("Нажатий всего".into()));
    assert_eq!(table[0].owner, 2);
    assert_eq!(table[1].kind, TagKind::Signal("press.out".into()));
    assert_eq!(table[1].owner, 2);

    let back = program.sheet_module(2).unwrap();
    assert!(back.name.len() <= 15 && !back.name.is_empty());
    assert_eq!(back.blocks.len(), 6);
    assert!(back.locals.is_empty());
    assert_eq!(back.tags.len(), 2);
    assert!(back.tags[0].reads && back.tags[0].writes);
    assert_eq!(back.tags[0].init, Some(ParamValue::Float(0.0)));
    assert!(!back.tags[1].reads && back.tags[1].writes);

    let main = program.sheet_module(1).unwrap();
    assert_eq!(main.name, "sheet1");
    assert_eq!(main.blocks.len(), 4);
    assert_eq!(main.tags.len(), 2);
    assert!(main
        .tags
        .iter()
        .all(|t| t.reads && !t.writes && t.init.is_none()));

    // No block belongs to both sheets.
    assert!(main.blocks.iter().all(|b| !back.blocks.contains(b)));
}

#[test]
fn variable_on_one_sheet_stays_local() {
    let lib = library();
    let mut project = background();
    project.logic.vars[0].retain = false;
    // The readers move to the sheet of the writer: the variable no longer crosses the border.
    for id in ["shown", "reached", "led"] {
        project.layout.blocks.get_mut(id).unwrap().sheet = 2;
    }
    let program = Program::build(&project, &lib).unwrap();
    let table = program.tag_table();
    assert_eq!(table.len(), 1);
    assert_eq!(table[0].kind, TagKind::Signal("press.out".into()));
    let back = program.sheet_module(2).unwrap();
    assert_eq!(back.locals.len(), 1);
    assert_eq!(back.locals[0].name, "Нажатий всего");
}

#[test]
fn retained_variable_is_refused() {
    let lib = library();
    let project = background();
    let program = Program::build(&project, &lib).unwrap();
    let err = program.sheet_module(2).unwrap_err();
    assert!(matches!(err, SheetError::Retain { sheet: 2, .. }), "{err}");
    assert!(err.to_string().contains("флеш"));
}

#[test]
fn empty_sheet_is_refused() {
    let lib = library();
    let project = background();
    let program = Program::build(&project, &lib).unwrap();
    assert_eq!(program.sheet_module(7).unwrap_err(), SheetError::Empty(7));
}

#[test]
fn pins_become_imports_and_rights() {
    let lib = library();
    let mut project = background();
    project.logic.vars[0].retain = false;
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    assert!(
        built.manifest.contains("allow = [\"gpio_write\"]"),
        "{}",
        built.manifest
    );
}

#[test]
fn unsupported_block_is_refused_with_its_name() {
    let lib = library();
    let mut project = background();
    project.logic.vars[0].retain = false;
    project
        .logic
        .blocks
        .iter_mut()
        .find(|b| b.id == "blink_led")
        .unwrap()
        .expander = Some("pcf".into());
    project.logic.expanders.push(flp_core::expander::Expander {
        id: "pcf".into(),
        chip: Default::default(),
        address: 0x20,
        sda: -1,
        scl: -1,
        period: "20ms".into(),
    });
    let program = Program::build(&project, &lib).unwrap();
    let err = program.wasm_module(1).unwrap_err().to_string();
    assert!(
        err.contains("blink_led") && err.contains("не поддержан"),
        "{err}"
    );
}
