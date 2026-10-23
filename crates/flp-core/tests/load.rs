use std::path::PathBuf;

use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::{Library, Logic, ParamValue, PortRef, Project};

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
    assert_eq!(p.logic.blocks.len(), 4);
    assert_eq!(p.logic.links.len(), 3);
    assert_eq!(
        p.logic.block("low").unwrap().params["b"],
        ParamValue::Float(2.0)
    );
    assert_eq!(p.logic.links[0].from, PortRef::new("pressure", "out"));
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
    for kind in ["digital_in", "digital_out", "analog_in", "less", "timer_on"] {
        let block = lib.get(kind).unwrap_or_else(|| panic!("нет блока {kind}"));
        assert!(block.template_path(ESP32_CPP_TEMPLATE).is_some(), "{kind}");
        assert!(block.docs_path().is_some(), "{kind}");
    }
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
