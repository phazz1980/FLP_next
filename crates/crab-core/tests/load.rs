use std::path::PathBuf;

use crab_core::library::ESP32_CPP_TEMPLATE;
use crab_core::{Library, Logic, ParamValue, PortRef, Project};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn load_pump_example() {
    let p = Project::load(repo_path("examples/pump")).unwrap();
    assert_eq!(p.config.project.name, "pump");
    assert_eq!(p.config.board.fqbn, "esp32:esp32:esp32");
    assert_eq!(p.logic.blocks.len(), 5);
    assert_eq!(p.logic.links.len(), 4);
    assert_eq!(
        p.logic.block("low").unwrap().params["b"],
        ParamValue::Float(2.0)
    );
    assert_eq!(p.logic.links[0].from, PortRef::new("sensor", "out"));
    assert_eq!(p.layout.blocks["pump"].sheet, 1);
}

#[test]
fn logic_roundtrips_through_toml_and_json() {
    let logic = Project::load(repo_path("examples/pump")).unwrap().logic;

    let toml_text = toml::to_string_pretty(&logic).unwrap();
    assert_eq!(toml::from_str::<Logic>(&toml_text).unwrap(), logic);

    let json = serde_json::to_string(&logic).unwrap();
    assert_eq!(serde_json::from_str::<Logic>(&json).unwrap(), logic);
}

#[test]
fn load_library() {
    let (lib, errors) = Library::load(repo_path("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    for kind in ["digital_in", "digital_out", "analog_in", "pwm_out"] {
        let block = lib.get(kind).unwrap_or_else(|| panic!("нет блока {kind}"));
        assert!(block.template_path(ESP32_CPP_TEMPLATE).is_none(), "{kind}");
        assert!(block.docs_path().is_some(), "{kind}");
    }
    // Built-in Rust blocks keep only the description and docs in the library.
    let block = lib.get("bit_and").unwrap();
    assert!(block.template_path(ESP32_CPP_TEMPLATE).is_none());
    assert!(block.docs_path().is_some());
    let block = lib.get("web_ui").unwrap();
    assert!(block.template_path(ESP32_CPP_TEMPLATE).is_none());
}

#[test]
fn pump_uses_only_known_blocks_and_ports() {
    let (lib, _) = Library::load(repo_path("library")).unwrap();
    let logic = Project::load(repo_path("examples/pump")).unwrap().logic;
    for b in &logic.blocks {
        let def = &lib.get(&b.kind).unwrap().def;
        for name in b.params.keys() {
            assert!(def.param(name).is_some(), "{}.{name}", b.id);
        }
    }
    for l in &logic.links {
        let def = |r: &PortRef| &lib.get(&logic.block(&r.block).unwrap().kind).unwrap().def;
        assert!(def(&l.from).output(&l.from.port).is_some(), "{}", l.from);
        assert!(def(&l.to).input(&l.to.port).is_some(), "{}", l.to);
    }
}

#[test]
fn web_page_blocks_are_stored_in_web_toml() {
    let mut p = Project::load(repo_path("examples/pump")).unwrap();
    let dir = std::env::temp_dir().join(format!("crab-webtoml-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    p.path = dir.join("page.crab");
    p.logic.blocks.push(crab_core::Block {
        id: "ui".into(),
        kind: "web_ui".into(),
        ..Default::default()
    });
    p.logic.blocks.push(crab_core::Block {
        id: "lamp".into(),
        kind: "web_led".into(),
        ..Default::default()
    });
    p.save().unwrap();

    let mut zip = zip::ZipArchive::new(std::fs::File::open(&p.path).unwrap()).unwrap();
    let mut read = |name: &str| {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut text).unwrap();
        text
    };
    let web = read("web.toml");
    assert!(web.contains("web_ui") && web.contains("web_led"));
    assert!(!read("logic.toml").contains("web_"));

    let back = Project::load(&p.path).unwrap();
    assert!(back.logic.block("ui").is_some() && back.logic.block("lamp").is_some());
    assert_eq!(back.logic.blocks.len(), p.logic.blocks.len());
    std::fs::remove_dir_all(&dir).unwrap();
}
