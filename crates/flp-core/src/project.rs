//! Project file `<name>.flpn`: a ZIP archive (opens in any archiver) with
//! `project.toml` (board, settings), `logic.toml` (blocks and links),
//! `layout.toml` (coordinates, editor only) and `boards.pdf` (picture of all
//! sheets). Old projects were folders of the three TOML files; they are still
//! read and converted on save.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::toml_io::{parse_toml, read_toml, to_toml};
use crate::{Error, Layout, Logic, Result};

/// Extension of project files.
pub const PROJECT_EXT: &str = "flpn";
/// Work folder next to the project file: generated sketches and build caches.
pub const WORK_DIR: &str = ".flp";

// Entries of the project archive; an old-format project is a folder of the same files.
pub const PROJECT_FILE: &str = "project.toml";
pub const LOGIC_FILE: &str = "logic.toml";
pub const LAYOUT_FILE: &str = "layout.toml";
/// Picture of all sheets, for viewing the schema without the editor.
pub const BOARDS_PDF: &str = "boards.pdf";

/// Project settings: `[project]`, `[board]`, `[diagnostics]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub board: BoardConfig,
    #[serde(default, skip_serializing_if = "Diagnostics::is_default")]
    pub diagnostics: Diagnostics,
    /// Hardware UARTs used by blocks (`[[uart]]`), set up in the board settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uart: Vec<crate::uart::UartConfig>,
}

/// `[diagnostics]`: cycle time measurement in normal (non-debug) firmware.
/// Debug builds always measure and send the numbers to the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagnostics {
    /// Measure cycle time and print it to the serial port as text.
    pub enabled: bool,
    /// How often the text report is printed, in milliseconds.
    pub interval_ms: u32,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 5000,
        }
    }
}

impl Diagnostics {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
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
    /// Exact board on top of the FQBN when it differs in brought-out pins, e.g.
    /// `esp32c3_supermini` (see [`crate::pins::BOARD_MODELS`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Serial port for upload and debugging, e.g. `COM3`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    /// Board menu options passed to arduino-cli as `--board-options`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// Loaded project: one `.flpn` archive next to its work folder `.flp/<name>/`.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// The project file (it may not exist yet for a folder in the old format).
    pub path: PathBuf,
    pub config: ProjectConfig,
    pub logic: Logic,
    /// Empty if there is no `layout.toml` (e.g. the schema was written by hand or by AI).
    pub layout: Layout,
}

impl Project {
    /// Loads a project file, or a folder holding one. A folder in the old
    /// three-file format is read too; saving converts it to `<folder>/<folder>.flpn`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Self::load_file(path);
        }
        if let Some(file) = find_project_file(path) {
            return Self::load_file(&file);
        }
        if path.join(PROJECT_FILE).is_file() {
            return Self::load_legacy(path);
        }
        Err(Error::NoProject {
            path: path.to_owned(),
        })
    }

    fn load_file(path: &Path) -> Result<Self> {
        let read_err = |source| Error::Read {
            path: path.to_owned(),
            source,
        };
        let file = File::open(path).map_err(read_err)?;
        let mut zip = ZipArchive::new(BufReader::new(file)).map_err(|e| zip_err(path, e))?;
        let mut entry = |name: &str| -> Result<Option<String>> {
            let mut file = match zip.by_name(name) {
                Ok(f) => f,
                Err(ZipError::FileNotFound) => return Ok(None),
                Err(e) => return Err(zip_err(path, e)),
            };
            let mut text = String::new();
            file.read_to_string(&mut text).map_err(read_err)?;
            Ok(Some(text))
        };
        let inner = |name: &str| path.join(name);
        let config = entry(PROJECT_FILE)?.ok_or_else(|| Error::MissingEntry {
            path: path.to_owned(),
            entry: PROJECT_FILE,
        })?;
        let config = parse_toml(&config, &inner(PROJECT_FILE))?;
        let logic = match entry(LOGIC_FILE)? {
            Some(text) => parse_toml(&text, &inner(LOGIC_FILE))?,
            None => Logic::default(),
        };
        let layout = match entry(LAYOUT_FILE)? {
            Some(text) => parse_toml(&text, &inner(LAYOUT_FILE))?,
            None => Layout::default(),
        };
        Ok(Self {
            path: path.to_owned(),
            config,
            logic,
            layout,
        }
        .migrated())
    }

    fn load_legacy(dir: &Path) -> Result<Self> {
        let config = read_toml(&dir.join(PROJECT_FILE))?;
        let logic = read_toml(&dir.join(LOGIC_FILE))?;
        let layout_path = dir.join(LAYOUT_FILE);
        let layout = if layout_path.exists() {
            read_toml(&layout_path)?
        } else {
            Layout::default()
        };
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");
        Ok(Self {
            path: dir.join(format!("{name}.{PROJECT_EXT}")),
            config,
            logic,
            layout,
        }
        .migrated())
    }

    /// Brings a project written by an older version to the current format.
    fn migrated(mut self) -> Self {
        crate::uart::migrate_legacy(&mut self.config.uart, &mut self.logic);
        self
    }

    /// Writes the project archive with just the schema files.
    pub fn save(&self) -> Result<()> {
        self.save_with(&[])
    }

    /// Writes the project archive: the schema files plus `extra` entries
    /// (name, contents), e.g. [`BOARDS_PDF`].
    ///
    /// The archive is written next to the project and then moved over it, so a
    /// failed save never leaves a broken project. Comments in the TOML files
    /// are not preserved.
    pub fn save_with(&self, extra: &[(&str, &[u8])]) -> Result<()> {
        let inner = |name: &str| self.path.join(name);
        let texts = [
            (PROJECT_FILE, to_toml(&self.config, &inner(PROJECT_FILE))?),
            (LOGIC_FILE, to_toml(&self.logic, &inner(LOGIC_FILE))?),
            (LAYOUT_FILE, to_toml(&self.layout, &inner(LAYOUT_FILE))?),
        ];
        let tmp = self.path.with_extension(format!("{PROJECT_EXT}.tmp"));
        let write_err = |source| Error::Write {
            path: self.path.clone(),
            source,
        };
        let file = File::create(&tmp).map_err(write_err)?;
        let mut zip = ZipWriter::new(BufWriter::new(file));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let entries = texts
            .iter()
            .map(|(name, text)| (*name, text.as_bytes()))
            .chain(extra.iter().copied());
        for (name, data) in entries {
            zip.start_file(name, options)
                .map_err(|e| zip_err(&self.path, e))?;
            zip.write_all(data).map_err(write_err)?;
        }
        zip.finish()
            .map_err(|e| zip_err(&self.path, e))?
            .flush()
            .map_err(write_err)?;
        std::fs::rename(&tmp, &self.path).map_err(write_err)
    }

    /// Folder the project file lives in.
    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new(""))
    }

    /// Project name taken from the file name.
    pub fn file_stem(&self) -> &str {
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
    }

    /// Generated sketch and build cache: `<dir>/.flp/<file name>/`, so several
    /// projects can share a folder.
    pub fn work_dir(&self) -> PathBuf {
        self.dir().join(WORK_DIR).join(self.file_stem())
    }
}

/// The only `.flpn` file in `dir`, if there is exactly one.
pub fn find_project_file(dir: &Path) -> Option<PathBuf> {
    let mut files = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_project_file(p));
    let first = files.next()?;
    files.next().is_none().then_some(first)
}

/// Whether `path` has the project file extension.
pub fn is_project_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(PROJECT_EXT))
}

/// Whether `path` is something [`Project::load`] can open.
pub fn is_project(path: &Path) -> bool {
    if path.is_dir() {
        find_project_file(path).is_some() || path.join(PROJECT_FILE).is_file()
    } else {
        is_project_file(path) && path.is_file()
    }
}

fn zip_err(path: &Path, source: ZipError) -> Error {
    Error::Archive {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_roundtrip_keeps_extra_entries() {
        let dir = std::env::temp_dir().join(format!("flp-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config: ProjectConfig =
            toml::from_str("[project]\nname = \"t\"\n[board]\nfqbn = \"x:y:z\"\n").unwrap();
        let project = Project {
            path: dir.join("t.flpn"),
            config,
            logic: Logic::default(),
            layout: Layout::default(),
        };
        project.save_with(&[(BOARDS_PDF, b"%PDF")]).unwrap();
        assert_eq!(Project::load(&dir).unwrap(), project);
        let mut zip = ZipArchive::new(File::open(&project.path).unwrap()).unwrap();
        assert!(zip.by_name(BOARDS_PDF).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
