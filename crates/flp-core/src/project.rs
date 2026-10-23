//! Project folder: `project.toml`, `logic.toml`, `layout.toml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::toml_io::{read_toml, write_toml};
use crate::{Layout, Logic, Result};

pub const PROJECT_FILE: &str = "project.toml";
pub const LOGIC_FILE: &str = "logic.toml";
pub const LAYOUT_FILE: &str = "layout.toml";

/// Contents of `project.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub board: BoardConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardConfig {
    /// Fully qualified board name for arduino-cli, e.g. `esp32:esp32:esp32`.
    pub fqbn: String,
    /// Serial port for upload and debugging, e.g. `COM3`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    /// Board menu options passed to arduino-cli as `--board-options`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// Loaded project folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub logic: Logic,
    /// Empty if `layout.toml` is missing (e.g. the schema was written by hand or by AI).
    pub layout: Layout,
}

impl Project {
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let config = read_toml(&root.join(PROJECT_FILE))?;
        let logic = read_toml(&root.join(LOGIC_FILE))?;
        let layout_path = root.join(LAYOUT_FILE);
        let layout = if layout_path.exists() {
            read_toml(&layout_path)?
        } else {
            Layout::default()
        };
        Ok(Self {
            root,
            config,
            logic,
            layout,
        })
    }

    /// Writes all three files.
    ///
    /// Comments in the files are not preserved yet; comment-preserving edits
    /// (via `toml_edit`) will come with the editor.
    pub fn save(&self) -> Result<()> {
        write_toml(&self.root.join(PROJECT_FILE), &self.config)?;
        write_toml(&self.root.join(LOGIC_FILE), &self.logic)?;
        write_toml(&self.root.join(LAYOUT_FILE), &self.layout)
    }
}
