//! Values kept by the program on this computer instead of the project file:
//! parameters with `local = "<key parameter>"` in `block.toml`, e.g. a Wi-Fi
//! password kept for each SSID. The project file stays safe to pass around;
//! the values are put back only for generating the sketch.
//!
//! The file is `%APPDATA%\CrabLogic\local.toml` (`~/.config/CrabLogic` elsewhere),
//! a table per block type and parameter, keyed by the value of the key parameter.
//! A key with an empty value is just remembered (the list of entered SSIDs):
//!
//! ```toml
//! ["wifi.password"]
//! "Дача" = "secret"
//! "Гости" = ""
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::toml_io::{read_toml, to_toml};
use crate::{Block, BlockDef, Error, Library, ParamValue, Project, Result};

pub const LOCAL_FILE: &str = "local.toml";

/// Settings folder of the program: `%APPDATA%\CrabLogic` (or `~/.config/CrabLogic`).
/// A folder left by the former name (`FLP_next`) is moved there on first use.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    let dir = base.join("CrabLogic");
    let old = base.join("FLP_next");
    if !dir.exists() && old.is_dir() {
        let _ = std::fs::rename(&old, &dir);
    }
    Some(dir)
}

/// A running editor window, as told by its heartbeat file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorInfo {
    pub pid: u32,
    /// Project open in that window; `None` while it has none.
    pub project: Option<PathBuf>,
}

/// An editor whose heartbeat is older than this is considered closed.
pub const EDITOR_ALIVE: std::time::Duration = std::time::Duration::from_secs(8);
/// How often an editor refreshes its heartbeat.
pub const EDITOR_BEAT: std::time::Duration = std::time::Duration::from_secs(2);

fn editors_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("editors"))
}

/// Called by the editor every frame: refreshes `editors/<pid>.toml` with the open
/// project, at most once per [`EDITOR_BEAT`] unless the project changed.
pub fn editor_beat(project: Option<&Path>) {
    use std::sync::Mutex;
    use std::time::Instant;
    static LAST: Mutex<Option<(Instant, Option<PathBuf>)>> = Mutex::new(None);
    let Ok(mut last) = LAST.lock() else { return };
    let project = project.map(Path::to_path_buf);
    if let Some((at, p)) = last.as_ref() {
        if *p == project && at.elapsed() < EDITOR_BEAT {
            return;
        }
    }
    let Some(dir) = editors_dir() else { return };
    let _ = fs::create_dir_all(&dir);
    let mut table = toml::Table::new();
    if let Some(p) = &project {
        let path = std::path::absolute(p).unwrap_or_else(|_| p.clone());
        table.insert("project".into(), path.display().to_string().into());
    }
    let file = dir.join(format!("{}.toml", std::process::id()));
    if fs::write(file, table.to_string()).is_ok() {
        *last = Some((Instant::now(), project));
    }
}

/// Editors running now (fresh heartbeat), oldest pid first. Stale files are removed.
pub fn live_editors() -> Vec<EditorInfo> {
    let Some(dir) = editors_dir() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let Some(pid) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let age = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok());
        if age.is_none_or(|a| a > EDITOR_ALIVE) {
            let _ = fs::remove_file(&path);
            continue;
        }
        let project = fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| t.get("project")?.as_str().map(PathBuf::from));
        found.push(EditorInfo { pid, project });
    }
    found.sort_by_key(|e| e.pid);
    found
}

/// Project the editor opened last (its `editor.toml`), for tools that follow the
/// editor, e.g. the MCP server. `None` if the editor never ran or saved none.
pub fn editor_last_project() -> Option<PathBuf> {
    let text = fs::read_to_string(config_dir()?.join("editor.toml")).ok()?;
    let value: toml::Table = text.parse().ok()?;
    let path = value.get("last_project")?.as_str()?;
    // Older settings keep the Windows verbatim prefix.
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    Some(PathBuf::from(path))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalValues {
    /// Where [`LocalValues::save`] writes; `None` keeps the values in memory only.
    path: Option<PathBuf>,
    tables: BTreeMap<String, BTreeMap<String, String>>,
}

fn table_name(kind: &str, param: &str) -> String {
    format!("{kind}.{param}")
}

impl LocalValues {
    /// Reads the file; a missing file gives an empty store that will be written there.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let tables = if path.exists() {
            read_toml(&path)?
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            path: Some(path),
            tables,
        })
    }

    /// The store of this computer, see [`config_dir`].
    pub fn open_default() -> Result<Self> {
        match config_dir() {
            Some(dir) => Self::open(dir.join(LOCAL_FILE)),
            None => Ok(Self::default()),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let write_err = |source| Error::Write {
            path: path.clone(),
            source,
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(write_err)?;
        }
        fs::write(path, to_toml(&self.tables, path)?).map_err(write_err)
    }

    pub fn get(&self, kind: &str, param: &str, key: &str) -> Option<&str> {
        self.tables
            .get(&table_name(kind, param))?
            .get(key)
            .map(String::as_str)
    }

    /// Stores the value for `key`; an empty key is not stored.
    pub fn set(&mut self, kind: &str, param: &str, key: &str, value: &str) {
        if key.is_empty() {
            return;
        }
        self.tables
            .entry(table_name(kind, param))
            .or_default()
            .insert(key.to_owned(), value.to_owned());
    }

    /// Remembers `key` without touching a value stored for it.
    pub fn remember(&mut self, kind: &str, param: &str, key: &str) {
        if self.get(kind, param, key).is_none() {
            self.set(kind, param, key, "");
        }
    }

    pub fn forget(&mut self, kind: &str, param: &str, key: &str) {
        if let Some(table) = self.tables.get_mut(&table_name(kind, param)) {
            table.remove(key);
        }
    }

    /// Remembered keys in sorted order (entered SSIDs).
    pub fn keys(&self, kind: &str, param: &str) -> impl Iterator<Item = &str> {
        self.tables
            .get(&table_name(kind, param))
            .into_iter()
            .flat_map(|t| t.keys().map(String::as_str))
    }
}

/// `local` parameters of a block: (parameter, value of its key parameter).
fn locals(def: &BlockDef, block: &Block) -> Vec<(String, String)> {
    def.params
        .iter()
        .filter_map(|p| {
            let key_param = p.local.as_ref()?;
            let key = block
                .params
                .get(key_param)
                .or(def.param(key_param)?.default.as_ref());
            let key = match key {
                Some(ParamValue::Text(s)) => s.clone(),
                _ => String::new(),
            };
            Some((p.name.clone(), key))
        })
        .collect()
}

impl LocalValues {
    /// Moves values of `local` parameters out of the block into the store and
    /// remembers their keys; returns whether the block changed.
    pub fn take_block(&mut self, def: &BlockDef, block: &mut Block) -> bool {
        let mut changed = false;
        for (param, key) in locals(def, block) {
            match block.params.remove(&param) {
                Some(value) => {
                    changed = true;
                    self.set(&block.kind, &param, &key, &value.to_string());
                }
                None => self.remember(&block.kind, &param, &key),
            }
        }
        changed
    }

    /// Puts the stored values of `local` parameters into the block.
    /// A value missing from the store keeps the default.
    pub fn fill_block(&self, def: &BlockDef, block: &mut Block) {
        for (param, key) in locals(def, block) {
            if let Some(value) = self.get(&block.kind, &param, &key) {
                block
                    .params
                    .insert(param, ParamValue::Text(value.to_owned()));
            }
        }
    }
}

impl Project {
    /// Moves values of `local` parameters out of the blocks into `store` and
    /// remembers their keys; returns whether a block changed.
    pub fn take_local(&mut self, lib: &Library, store: &mut LocalValues) -> bool {
        let mut changed = false;
        for i in 0..self.logic.blocks.len() {
            let Some(lb) = lib.resolve(&self.logic, &self.logic.blocks[i]) else {
                continue;
            };
            let def = lb.def.clone();
            changed |= store.take_block(&def, &mut self.logic.blocks[i]);
        }
        changed
    }

    /// A copy with the values of `local` parameters taken from `store`, for
    /// generating the sketch.
    pub fn with_local(&self, lib: &Library, store: &LocalValues) -> Project {
        let mut out = self.without_disabled();
        for i in 0..out.logic.blocks.len() {
            let Some(lb) = lib.resolve(&out.logic, &out.logic.blocks[i]) else {
                continue;
            };
            store.fill_block(&lb.def, &mut out.logic.blocks[i]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LibraryBlock;

    fn library() -> Library {
        let def: BlockDef = toml::from_str(
            r#"
[block]
type = "wifi"
name = "Wi-Fi"
group = "Сеть"

[[param]]
name = "ssid"
type = "string"

[[param]]
name = "password"
type = "string"
default = ""
local = "ssid"
"#,
        )
        .unwrap();
        def.validate("wifi").unwrap();
        let mut lib = Library::default();
        lib.insert(LibraryBlock {
            def,
            dir: Default::default(),
            inline_templates: Default::default(),
        });
        lib
    }

    fn project() -> Project {
        let logic = toml::from_str(
            "[[block]]\nid = \"net\"\ntype = \"wifi\"\nssid = \"Дача\"\npassword = \"secret\"\n",
        )
        .unwrap();
        Project {
            path: PathBuf::from("p.flpn"),
            config: toml::from_str(
                "[project]\nname = \"p\"\n[board]\nfqbn = \"esp32:esp32:esp32\"\n",
            )
            .unwrap(),
            logic,
            layout: Default::default(),
        }
    }

    #[test]
    fn password_moves_to_the_store_and_back() {
        let lib = library();
        let mut store = LocalValues::default();
        let mut p = project();
        assert!(p.take_local(&lib, &mut store));
        assert!(!p.logic.blocks[0].params.contains_key("password"));
        assert_eq!(store.get("wifi", "password", "Дача"), Some("secret"));
        assert!(!p.take_local(&lib, &mut store));

        let filled = p.with_local(&lib, &store);
        assert_eq!(
            filled.logic.blocks[0].params.get("password"),
            Some(&ParamValue::Text("secret".into()))
        );
        assert!(!p.logic.blocks[0].params.contains_key("password"));
    }

    #[test]
    fn keys_are_remembered_without_overwriting() {
        let lib = library();
        let mut store = LocalValues::default();
        store.set("wifi", "password", "Дача", "old");
        let mut p = project();
        p.logic.blocks[0].params.remove("password");
        p.take_local(&lib, &mut store);
        assert_eq!(store.get("wifi", "password", "Дача"), Some("old"));
        store.remember("wifi", "password", "Гости");
        let keys: Vec<&str> = store.keys("wifi", "password").collect();
        assert_eq!(keys, ["Гости", "Дача"]);
    }

    #[test]
    fn store_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("flp_local_{}", std::process::id()));
        let path = dir.join(LOCAL_FILE);
        let mut store = LocalValues::open(&path).unwrap();
        store.set("wifi", "password", "Дача", "secret \"1\"");
        store.save().unwrap();
        let again = LocalValues::open(&path).unwrap();
        assert_eq!(again.get("wifi", "password", "Дача"), Some("secret \"1\""));
        fs::remove_dir_all(dir).unwrap();
    }
}
