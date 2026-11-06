//! Project file `<name>.crab`: a ZIP archive (opens in any archiver) with
//! `project.toml` (board, settings), `logic.toml` (blocks and links),
//! `web.toml` (the web page: its settings and widgets), `layout.toml`
//! (coordinates, editor only) and `boards.pdf` (picture of all sheets). Old projects were folders of the three TOML files; they are still
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
pub const PROJECT_EXT: &str = "crab";
/// Extension of projects saved before the rename; still opened.
pub const LEGACY_PROJECT_EXT: &str = "flpn";
/// Work folder next to the project file: generated sketches and build caches.
pub const WORK_DIR: &str = ".crab";

// Entries of the project archive; an old-format project is a folder of the same files.
pub const PROJECT_FILE: &str = "project.toml";
pub const LOGIC_FILE: &str = "logic.toml";
pub const LAYOUT_FILE: &str = "layout.toml";
/// The web page: the `web_ui` block and the widgets, in the format of `logic.toml`.
/// In memory they are blocks of [`Logic`]; on disk they live apart from the logic.
pub const WEB_FILE: &str = "web.toml";
/// Picture of all sheets, for viewing the schema without the editor.
pub const BOARDS_PDF: &str = "boards.pdf";

/// Blocks of the web page (the `web_ui` host and the widgets) are stored in `web.toml`.
pub fn is_web_kind(kind: &str) -> bool {
    kind.starts_with("web_")
}

/// Project settings: `[project]`, `[board]`, `[[uart]]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub board: BoardConfig,
    /// Hardware UARTs used by blocks (`[[uart]]`), set up in the board settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uart: Vec<crate::uart::UartConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Language of the comments in the generated sketch (`ru`, `en`); empty means `ru`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment_lang: String,
    /// Password of the project copy kept in the board's flash; empty means no protection.
    /// The `.crab` file itself stays readable.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    /// Keep the project copy in the firmware (readable back from the board). On by default.
    #[serde(default = "embed_default", skip_serializing_if = "is_true")]
    pub embed: bool,
}

fn embed_default() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
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

/// Loaded project: one `.crab` archive next to its work folder `.crab/<name>/`.
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
    /// Ids of the blocks standing on switched-off sheets.
    pub fn disabled_blocks(&self) -> std::collections::HashSet<&str> {
        self.layout
            .blocks
            .iter()
            .filter(|(_, p)| self.layout.is_disabled(p.sheet))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Indices (in [`Logic::links`]) of the links that survive [`Project::without_disabled`].
    pub fn enabled_links(&self) -> Vec<usize> {
        let off = self.disabled_blocks();
        (0..self.logic.links.len())
            .filter(|&i| {
                let l = &self.logic.links[i];
                !off.contains(l.from.block.as_str()) && !off.contains(l.to.block.as_str())
            })
            .collect()
    }

    /// A copy without the blocks of switched-off sheets and the links touching them:
    /// what is checked and built.
    pub fn without_disabled(&self) -> Project {
        let mut out = self.clone();
        let mut off: Vec<String> = self
            .disabled_blocks()
            .into_iter()
            .map(String::from)
            .collect();
        // A switched-off web page builds nothing: its widgets stay only in the project.
        let web_off = self.logic.blocks.iter().any(|b| {
            b.kind == "web_ui"
                && matches!(
                    b.params.get("enabled"),
                    Some(crate::ParamValue::Bool(false))
                )
        });
        if web_off {
            off.extend(
                self.logic
                    .blocks
                    .iter()
                    .filter(|b| is_web_kind(&b.kind))
                    .map(|b| b.id.clone()),
            );
        }
        if off.is_empty() {
            return out;
        }
        out.logic.links = self
            .enabled_links()
            .into_iter()
            .map(|i| self.logic.links[i].clone())
            .filter(|l| !off.contains(&l.from.block) && !off.contains(&l.to.block))
            .collect();
        out.logic.blocks.retain(|b| !off.contains(&b.id));
        out.layout.blocks.retain(|id, _| !off.contains(id));
        out
    }

    /// Loads a project file, or a folder holding one. A folder in the old
    /// three-file format is read too; saving converts it to `<folder>/<folder>.crab`.
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
        Self::read_archive(BufReader::new(file), path)
    }

    /// Reads a project from archive bytes (e.g. read back from a board); `path` is the
    /// file the project will be saved to.
    pub fn from_archive_bytes(bytes: &[u8], path: impl AsRef<Path>) -> Result<Self> {
        Self::read_archive(std::io::Cursor::new(bytes), path.as_ref())
    }

    fn read_archive<R: Read + std::io::Seek>(reader: R, path: &Path) -> Result<Self> {
        let read_err = |source| Error::Read {
            path: path.to_owned(),
            source,
        };
        let mut zip = ZipArchive::new(reader).map_err(|e| zip_err(path, e))?;
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
        let mut logic: Logic = match entry(LOGIC_FILE)? {
            Some(text) => parse_toml(&text, &inner(LOGIC_FILE))?,
            None => Logic::default(),
        };
        let layout = match entry(LAYOUT_FILE)? {
            Some(text) => parse_toml(&text, &inner(LAYOUT_FILE))?,
            None => Layout::default(),
        };
        if let Some(text) = entry(WEB_FILE)? {
            let web: Logic = parse_toml(&text, &inner(WEB_FILE))?;
            logic.blocks.extend(web.blocks);
        }
        Ok(Self {
            path: path.to_owned(),
            config,
            logic,
            layout,
        }
        .migrated())
    }

    /// Builds a project from the texts of its three files kept in memory (the browser
    /// demo has no folders); `name` stands in for the project path.
    pub fn from_sources(name: &str, project: &str, logic: &str, layout: &str) -> Result<Self> {
        let at = |file: &str| Path::new(name).join(file);
        Ok(Self {
            path: at(&format!("{name}.{PROJECT_EXT}")),
            config: parse_toml(project, &at(PROJECT_FILE))?,
            logic: parse_toml(logic, &at(LOGIC_FILE))?,
            layout: parse_toml(layout, &at(LAYOUT_FILE))?,
        }
        .migrated())
    }

    fn load_legacy(dir: &Path) -> Result<Self> {
        let config = read_toml(&dir.join(PROJECT_FILE))?;
        let mut logic: Logic = read_toml(&dir.join(LOGIC_FILE))?;
        let web_path = dir.join(WEB_FILE);
        if web_path.exists() {
            let web: Logic = read_toml(&web_path)?;
            logic.blocks.extend(web.blocks);
        }
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
        migrate_wifi_mode(&mut self.logic);
        crate::uart::migrate_uart_send(&mut self.logic);
        self.logic.sync_tags();
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
        let texts = self.files(true)?;
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

    /// The schema files packed into an archive in memory: the copy that goes into the
    /// firmware. Equal projects give equal bytes (no timestamps), so an unchanged
    /// project does not change the sketch.
    pub fn archive_bytes(&self) -> Result<Vec<u8>> {
        let texts = self.files(true)?;
        let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, text) in &texts {
            zip.start_file(*name, options)
                .map_err(|e| zip_err(&self.path, e))?;
            zip.write_all(text.as_bytes())
                .map_err(|source| Error::Write {
                    path: self.path.clone(),
                    source,
                })?;
        }
        Ok(zip
            .finish()
            .map_err(|e| zip_err(&self.path, e))?
            .into_inner())
    }

    /// The text of the schema files as they are stored: the web page blocks go to
    /// `web.toml` (written only when there are any), the rest of the logic stays in
    /// `logic.toml`. `with_layout` adds `layout.toml` at the end.
    fn files(&self, with_layout: bool) -> Result<Vec<(&'static str, String)>> {
        let inner = |name: &str| self.path.join(name);
        let (web, rest): (Vec<_>, Vec<_>) = self
            .logic
            .blocks
            .iter()
            .cloned()
            .partition(|b| is_web_kind(&b.kind));
        let logic = Logic {
            blocks: rest,
            ..self.logic.clone()
        };
        let mut files = vec![
            (PROJECT_FILE, to_toml(&self.config, &inner(PROJECT_FILE))?),
            (LOGIC_FILE, to_toml(&logic, &inner(LOGIC_FILE))?),
        ];
        if !web.is_empty() {
            let page = Logic {
                blocks: web,
                ..Logic::default()
            };
            files.push((WEB_FILE, to_toml(&page, &inner(WEB_FILE))?));
        }
        if with_layout {
            files.push((LAYOUT_FILE, to_toml(&self.layout, &inner(LAYOUT_FILE))?));
        }
        Ok(files)
    }

    /// Short identity of the project's logic and settings (`project.toml` and
    /// `logic.toml`; coordinates in `layout.toml` do not count). The firmware carries
    /// the same value, so the editor can tell whether the board runs this project.
    pub fn content_hash(&self) -> Result<[u8; 8]> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for (name, text) in self.files(false)? {
            hasher.update((text.len() as u64).to_le_bytes());
            hasher.update(name.as_bytes());
            hasher.update(text.as_bytes());
        }
        let mut hash = [0u8; 8];
        hash.copy_from_slice(&hasher.finalize()[..8]);
        Ok(hash)
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

    /// Generated sketch and build cache: `<dir>/.crab/<file name>/`, so several
    /// projects can share a folder.
    pub fn work_dir(&self) -> PathBuf {
        self.dir().join(WORK_DIR).join(self.file_stem())
    }
}

/// The only `.crab` file in `dir`, if there is exactly one.
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
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case(PROJECT_EXT) || e.eq_ignore_ascii_case(LEGACY_PROJECT_EXT)
    })
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

/// The Wi-Fi block took its mode from the input when the check box `use_mode`
/// was on; now that is the choice `wifi_mode = 3`.
fn migrate_wifi_mode(logic: &mut Logic) {
    for block in logic.blocks.iter_mut().filter(|b| b.kind == "wifi") {
        if let Some(use_mode) = block.params.remove("use_mode") {
            if use_mode == crate::ParamValue::Bool(true) {
                block
                    .params
                    .insert("wifi_mode".into(), crate::ParamValue::Int(3));
            }
        }
    }
}

/// A project hash as 16 hex digits.
pub fn hash_hex(hash: &[u8; 8]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_use_mode_becomes_mode_choice() {
        let mut logic: Logic = toml::from_str(
            r#"
[[block]]
id = "on"
type = "wifi"
ssid = "net"
use_mode = true
wifi_mode = 1

[[block]]
id = "off"
type = "wifi"
ssid = "net"
use_mode = false
wifi_mode = 2
"#,
        )
        .unwrap();
        migrate_wifi_mode(&mut logic);
        let on = &logic.blocks[0].params;
        assert_eq!(on.get("wifi_mode"), Some(&crate::ParamValue::Int(3)));
        assert!(!on.contains_key("use_mode"));
        let off = &logic.blocks[1].params;
        assert_eq!(off.get("wifi_mode"), Some(&crate::ParamValue::Int(2)));
        assert!(!off.contains_key("use_mode"));
    }

    #[test]
    fn archive_roundtrip_keeps_extra_entries() {
        let dir = std::env::temp_dir().join(format!("crab-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config: ProjectConfig =
            toml::from_str("[project]\nname = \"t\"\n[board]\nfqbn = \"x:y:z\"\n").unwrap();
        let project = Project {
            path: dir.join("t.crab"),
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

    #[test]
    fn content_hash_ignores_layout_only() {
        let config: ProjectConfig = toml::from_str(
            "[project]
name = \"t\"
[board]
fqbn = \"x:y:z\"
",
        )
        .unwrap();
        let project = Project {
            path: PathBuf::from("t.crab"),
            config,
            logic: Logic::default(),
            layout: Layout::default(),
        };
        let hash = project.content_hash().unwrap();
        assert_eq!(hash, project.content_hash().unwrap());

        let mut moved = project.clone();
        moved.layout.sheets.push(Default::default());
        assert_eq!(moved.content_hash().unwrap(), hash);

        let mut renamed = project.clone();
        renamed.config.project.name = "other".into();
        assert_ne!(renamed.content_hash().unwrap(), hash);

        let mut edited = project.clone();
        edited.logic.blocks.push(
            toml::from_str(
                "id = \"a\"
type = \"wifi\"
",
            )
            .unwrap(),
        );
        assert_ne!(edited.content_hash().unwrap(), hash);
    }
}
