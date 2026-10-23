//! Block library: folders `library/<type>/` with `block.toml`, code templates and docs.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ident::is_valid_ident;
use crate::toml_io::read_toml;
use crate::value::parse_duration_ms;
use crate::{Error, ParamValue, Result};

pub const BLOCK_FILE: &str = "block.toml";
pub const ESP32_CPP_TEMPLATE: &str = "esp32.cpp.tmpl";
pub const DOCS_FILE: &str = "docs.md";

/// Type of a value carried by a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignalType {
    Bool,
    Int,
    Float,
}

impl SignalType {
    /// Whether an output of type `self` may feed an input of type `to`.
    /// Only `int` → `float` widening is implicit.
    pub fn can_feed(self, to: SignalType) -> bool {
        self == to || (self == SignalType::Int && to == SignalType::Float)
    }

    /// Whether a constant may stand for a signal of this type.
    pub fn accepts(self, value: &ParamValue) -> bool {
        matches!(
            (self, value),
            (SignalType::Bool, ParamValue::Bool(_))
                | (SignalType::Int, ParamValue::Int(_))
                | (SignalType::Float, ParamValue::Float(_) | ParamValue::Int(_))
        )
    }
}

/// Kind of a block parameter; defines how its value is checked and rendered into code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    Bool,
    Int,
    Float,
    #[serde(rename = "string")]
    Text,
    /// Duration like `"5s"`; rendered into code as milliseconds.
    Duration,
    /// Board pin number.
    Pin,
}

impl ParamKind {
    pub fn accepts(self, value: &ParamValue) -> bool {
        match (self, value) {
            (ParamKind::Bool, ParamValue::Bool(_)) => true,
            (ParamKind::Int | ParamKind::Pin, ParamValue::Int(_)) => true,
            (ParamKind::Float, ParamValue::Float(_) | ParamValue::Int(_)) => true,
            (ParamKind::Text, ParamValue::Text(_)) => true,
            (ParamKind::Duration, ParamValue::Text(s)) => parse_duration_ms(s).is_some(),
            _ => false,
        }
    }
}

/// Contents of `block.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDef {
    #[serde(rename = "block")]
    pub info: BlockInfo,
    #[serde(rename = "input", default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<PortDef>,
    #[serde(rename = "output", default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PortDef>,
    #[serde(rename = "param", default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Type name; must match the folder name.
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name in Russian.
    pub name: String,
    /// Library folder in the editor ("Входы и выходы", "Время", ...).
    pub group: String,
    #[serde(default)]
    pub description: String,
    /// The block keeps state between scans (timers, triggers, counters).
    #[serde(default)]
    pub memory: bool,
    /// Headers the generated code needs, e.g. `DallasTemperature.h`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<String>,
    /// Arduino libraries to install before building, as arduino-cli names them
    /// (`DallasTemperature`, optionally with a version: `OneWire@2.3.8`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: SignalType,
    /// Value of an unconnected input. Only inputs may have it; without it
    /// the input must be connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ParamKind,
    /// Without a default the parameter is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default)]
    pub description: String,
}

impl BlockDef {
    pub fn input(&self, name: &str) -> Option<&PortDef> {
        self.inputs.iter().find(|p| p.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&PortDef> {
        self.outputs.iter().find(|p| p.name == name)
    }

    pub fn param(&self, name: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Checks internal consistency; `folder` is the name of the block folder.
    pub fn validate(&self, folder: &str) -> Result<(), String> {
        if self.info.kind != folder {
            return Err(format!(
                "тип «{}» не совпадает с именем папки «{folder}»",
                self.info.kind
            ));
        }
        if !is_valid_ident(&self.info.kind) {
            return Err(format!("недопустимое имя типа «{}»", self.info.kind));
        }
        // Ports share one namespace: a link `block.port` does not say the direction.
        let mut names = HashSet::new();
        let ports = self.inputs.iter().chain(&self.outputs).map(|p| &p.name);
        for name in ports.chain(self.params.iter().map(|p| &p.name)) {
            if !is_valid_ident(name) {
                return Err(format!("недопустимое имя «{name}»"));
            }
            if name == "id" || name == "type" {
                return Err(format!("имя «{name}» зарезервировано"));
            }
            if !names.insert(name.as_str()) {
                return Err(format!("имя «{name}» встречается дважды"));
            }
        }
        for p in &self.inputs {
            if p.default.as_ref().is_some_and(|d| !p.ty.accepts(d)) {
                return Err(format!(
                    "значение по умолчанию входа «{}» не подходит к его типу",
                    p.name
                ));
            }
        }
        if let Some(p) = self.outputs.iter().find(|p| p.default.is_some()) {
            return Err(format!(
                "у выхода «{}» не может быть значения по умолчанию",
                p.name
            ));
        }
        for p in &self.params {
            if let Some(default) = &p.default {
                if !p.kind.accepts(default) {
                    return Err(format!(
                        "значение по умолчанию параметра «{}» не подходит к его типу",
                        p.name
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Block loaded from a library folder.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryBlock {
    pub def: BlockDef,
    pub dir: PathBuf,
    /// Templates of built-in blocks, which have no folder: file name → source.
    pub inline_templates: BTreeMap<&'static str, &'static str>,
}

impl LibraryBlock {
    /// Template source: inline for built-in blocks, otherwise read from the folder.
    pub fn template(&self, file: &str) -> Option<std::io::Result<String>> {
        if let Some(src) = self.inline_templates.get(file) {
            return Some(Ok((*src).to_owned()));
        }
        self.template_path(file).map(fs::read_to_string)
    }

    /// Path to a template (e.g. [`ESP32_CPP_TEMPLATE`]) if the block has one.
    pub fn template_path(&self, file: &str) -> Option<PathBuf> {
        let path = self.dir.join(file);
        path.is_file().then_some(path)
    }

    pub fn docs_path(&self) -> Option<PathBuf> {
        self.template_path(DOCS_FILE)
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(BLOCK_FILE);
        let def: BlockDef = read_toml(&path)?;
        let folder = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        def.validate(folder)
            .map_err(|message| Error::InvalidBlockDef { path, message })?;
        Ok(Self {
            def,
            dir: dir.to_owned(),
            inline_templates: BTreeMap::new(),
        })
    }
}

/// All known block types.
#[derive(Debug, Clone)]
pub struct Library {
    blocks: BTreeMap<String, LibraryBlock>,
    /// Variable access blocks, one per access kind and signal type (`var_get_float`).
    builtins: BTreeMap<String, LibraryBlock>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            blocks: BTreeMap::new(),
            builtins: crate::vars::builtin_blocks(),
        }
    }
}

impl Library {
    /// Loads blocks from `dir`. A broken block does not stop loading: it is skipped
    /// and its error is returned alongside the library.
    pub fn load(dir: impl AsRef<Path>) -> Result<(Self, Vec<Error>)> {
        let mut lib = Self::default();
        let errors = lib.add_dir(dir)?;
        Ok((lib, errors))
    }

    /// Adds blocks from one more library folder; blocks with the same type are replaced,
    /// so a user folder added later overrides built-in blocks.
    pub fn add_dir(&mut self, dir: impl AsRef<Path>) -> Result<Vec<Error>> {
        let dir = dir.as_ref();
        let read_err = |source| Error::Read {
            path: dir.to_owned(),
            source,
        };
        let mut errors = Vec::new();
        for entry in fs::read_dir(dir).map_err(read_err)? {
            let path = entry.map_err(read_err)?.path();
            if !path.join(BLOCK_FILE).is_file() {
                continue;
            }
            match LibraryBlock::load(&path) {
                Ok(block) => {
                    self.blocks.insert(block.def.info.kind.clone(), block);
                }
                Err(e) => errors.push(e),
            }
        }
        Ok(errors)
    }

    /// Adds or replaces a block (e.g. one created by AI before it is written to disk).
    pub fn insert(&mut self, block: LibraryBlock) {
        self.blocks.insert(block.def.info.kind.clone(), block);
    }

    /// Library block by type. Variable blocks (`var_get`, `var_set`) depend on the
    /// variable they access; use [`Library::resolve`] for blocks of a schema.
    pub fn get(&self, kind: &str) -> Option<&LibraryBlock> {
        self.blocks.get(kind)
    }

    /// Definition of a schema block, including variable blocks, whose port type comes
    /// from the variable (float while the variable is unknown).
    pub fn resolve(&self, logic: &crate::Logic, block: &crate::Block) -> Option<&LibraryBlock> {
        match crate::vars::builtin_kind(logic, block) {
            Some(kind) => self.builtins.get(&kind),
            None => self.get(&block.kind),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LibraryBlock> {
        self.blocks.values()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(src: &str) -> BlockDef {
        toml::from_str(src).unwrap()
    }

    const HEADER: &str = "[block]\ntype = \"x\"\nname = \"X\"\ngroup = \"Тест\"\n";

    #[test]
    fn type_widening() {
        assert!(SignalType::Int.can_feed(SignalType::Float));
        assert!(!SignalType::Float.can_feed(SignalType::Int));
        assert!(!SignalType::Bool.can_feed(SignalType::Int));
    }

    #[test]
    fn validate_ok() {
        let d = def(&format!(
            "{HEADER}[[input]]\nname = \"in\"\ntype = \"bool\"\n\
             [[param]]\nname = \"time\"\ntype = \"duration\"\ndefault = \"1s\"\n"
        ));
        assert_eq!(d.validate("x"), Ok(()));
        assert_eq!(d.param("time").unwrap().kind, ParamKind::Duration);
    }

    #[test]
    fn validate_errors() {
        assert!(def(HEADER).validate("y").is_err());
        let dup = def(&format!(
            "{HEADER}[[input]]\nname = \"a\"\ntype = \"bool\"\n\
             [[output]]\nname = \"a\"\ntype = \"bool\"\n"
        ));
        assert!(dup.validate("x").is_err());
        let bad_default = def(&format!(
            "{HEADER}[[param]]\nname = \"t\"\ntype = \"duration\"\ndefault = \"soon\"\n"
        ));
        assert!(bad_default.validate("x").is_err());
        let reserved = def(&format!(
            "{HEADER}[[param]]\nname = \"id\"\ntype = \"int\"\n"
        ));
        assert!(reserved.validate("x").is_err());
    }

    #[test]
    fn port_defaults() {
        let ok = def(&format!(
            "{HEADER}[[input]]\nname = \"en\"\ntype = \"bool\"\ndefault = true\n\
             [[input]]\nname = \"k\"\ntype = \"float\"\ndefault = 2\n"
        ));
        assert_eq!(ok.validate("x"), Ok(()));
        let wrong_type = def(&format!(
            "{HEADER}[[input]]\nname = \"en\"\ntype = \"bool\"\ndefault = 1\n"
        ));
        assert!(wrong_type.validate("x").is_err());
        let on_output = def(&format!(
            "{HEADER}[[output]]\nname = \"q\"\ntype = \"bool\"\ndefault = false\n"
        ));
        assert!(on_output.validate("x").is_err());
    }
}
