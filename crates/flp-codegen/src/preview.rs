//! Trial generation of a single block, for the block editor: the block being edited
//! is placed alone in an in-memory project and rendered into a full sketch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::{
    Block, BlockDef, BoardConfig, Diagnostics, Layout, Library, LibraryBlock, Logic, ParamKind,
    ParamValue, Project, ProjectConfig, ProjectInfo, SignalType,
};

use crate::{ArduinoCpp, GenError};

/// Id of the sample instance in the preview sketch.
pub fn preview_id(def: &BlockDef) -> String {
    format!("{}_1", def.info.kind)
}

/// Generates a sketch with one instance of the block (id from [`preview_id`]).
///
/// Inputs that must be connected get a zero constant and required parameters a
/// sample value (zero, the minimum, `1s`, an empty string), so the template is
/// rendered the way it would be in a real schema.
pub fn preview_block(def: &BlockDef, template: &str) -> Result<String, GenError> {
    let mut def = def.clone();
    for input in def.inputs.iter_mut().filter(|p| p.default.is_none()) {
        input.default = Some(match input.ty {
            SignalType::Bool => ParamValue::Bool(false),
            SignalType::Int => ParamValue::Int(0),
            SignalType::Float => ParamValue::Float(0.0),
        });
    }
    let mut params = BTreeMap::new();
    for p in def.params.iter().filter(|p| p.default.is_none()) {
        let min = p.min.unwrap_or(0.0).max(0.0);
        let value = match p.kind {
            ParamKind::Bool => ParamValue::Bool(false),
            ParamKind::Int | ParamKind::Pin => ParamValue::Int(min.ceil() as i64),
            ParamKind::Float => ParamValue::Float(min),
            ParamKind::Text => ParamValue::Text(String::new()),
            ParamKind::Duration => ParamValue::Text("1s".into()),
        };
        params.insert(p.name.clone(), value);
    }

    let mut lib = Library::default();
    lib.insert(LibraryBlock {
        def: def.clone(),
        dir: PathBuf::new(),
        inline_templates: [(ESP32_CPP_TEMPLATE, template.to_owned())].into(),
    });
    let project = Project {
        path: PathBuf::new(),
        config: ProjectConfig {
            project: ProjectInfo {
                name: def.info.name.clone(),
                description: String::new(),
            },
            board: BoardConfig {
                fqbn: "esp32:esp32:esp32".into(),
                port: None,
                options: BTreeMap::new(),
            },
            diagnostics: Diagnostics::default(),
        },
        logic: Logic {
            vars: Vec::new(),
            blocks: vec![Block {
                id: preview_id(&def),
                kind: def.info.kind.clone(),
                params,
            }],
            links: Vec::new(),
        },
        layout: Layout::default(),
    };
    crate::generate(&project, &lib, &ArduinoCpp::RELEASE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(src: &str) -> BlockDef {
        toml::from_str(src).unwrap()
    }

    const BLOCK: &str = r#"
[block]
type = "blink"
name = "Мигалка"
group = "Тест"
memory = true

[[input]]
name = "en"
type = "bool"

[[output]]
name = "q"
type = "bool"

[[param]]
name = "period"
type = "duration"

[[param]]
name = "pin"
type = "pin"
min = 2
"#;

    #[test]
    fn renders_required_ports_and_params() {
        let template = "// @global\nuint32_t {{ id }}_t = 0;\n\
                        // @loop\nif ({{ input.en }} && millis() - {{ id }}_t > {{ param.period }}) {\n  \
                        {{ output.q }} = !{{ output.q }}; // pin {{ param.pin }}\n}\n";
        let code = preview_block(&def(BLOCK), template).unwrap();
        assert!(code.contains("uint32_t blink_1_t = 0;"), "{code}");
        assert!(code.contains("> 1000"), "{code}");
        assert!(code.contains("// pin 2"), "{code}");
    }

    #[test]
    fn reports_template_errors() {
        let err = preview_block(&def(BLOCK), "// @loop\n{{ input.nope }}\n").unwrap_err();
        assert!(!err.to_string().is_empty());
        assert!(preview_block(&def(BLOCK), "// @loop\n{% if %}\n").is_err());
    }
}
