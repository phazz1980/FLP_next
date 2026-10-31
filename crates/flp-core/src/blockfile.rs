//! A user block as a set of files: the block folder (`block.toml`, templates, `docs.md`),
//! the Arduino libraries it uses (`lib/<Name>/…`) and an example project (`example.flpn`).
//! Blocks travel inside a pack (see `pack`) or inside a project (see `packed`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::library::{BlockDef, Library};
use crate::pack::{self, PackInfo};

/// Example project inside the block folder.
pub const EXAMPLE_FILE: &str = "example.flpn";
/// Folder with the Arduino libraries of the block; every subfolder is one library.
pub const LIB_DIR: &str = "lib";
/// Folder with the pictures of `docs.md`; the docs refer to them as `images/<name>`.
pub const IMAGES_DIR: &str = "images";

/// A block read from a file or from a project, not yet in the library.
#[derive(Debug, Clone)]
pub struct Incoming {
    pub def: BlockDef,
    /// Files of the block folder: relative path with `/` → contents.
    pub files: Vec<(String, Vec<u8>)>,
}

/// What stands in the way of installing an [`Incoming`] block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    None,
    /// A built-in block has this type: it cannot be replaced.
    Builtin,
    /// A user block with this type exists.
    User,
}

impl Incoming {
    /// Builds a block from folder files; `block.toml` must be among them and valid.
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, String> {
        if !files.iter().all(|(n, _)| safe_path(n)) {
            return Err("в файле небезопасные пути".into());
        }
        let (_, toml) = files
            .iter()
            .find(|(n, _)| n == "block.toml")
            .ok_or("в файле нет block.toml")?;
        let text = std::str::from_utf8(toml).map_err(|_| "block.toml не в UTF-8")?;
        let def: BlockDef = toml::from_str(text).map_err(|e| format!("block.toml: {e}"))?;
        let kind = def.info.kind.clone();
        if !safe_part(&kind) {
            return Err(format!("недопустимый тип блока «{kind}»"));
        }
        def.validate(&kind)?;
        if def
            .info
            .group
            .split('/')
            .any(|p| !p.is_empty() && !safe_part(p))
        {
            return Err("недопустимая папка блока".into());
        }
        Ok(Self { def, files })
    }

    pub fn kind(&self) -> &str {
        &self.def.info.kind
    }

    /// Folder of the block inside the user library: its group.
    pub fn group(&self) -> String {
        self.def.info.group.trim_matches('/').to_owned()
    }

    /// Names of the attached Arduino libraries.
    pub fn libraries(&self) -> Vec<String> {
        let prefix = format!("{LIB_DIR}/");
        let mut names: Vec<String> = self
            .files
            .iter()
            .filter_map(|(n, _)| {
                n.strip_prefix(&prefix)?
                    .split_once('/')
                    .map(|(lib, _)| lib.to_owned())
            })
            .collect();
        names.dedup();
        names
    }

    pub fn has_example(&self) -> bool {
        self.files.iter().any(|(n, _)| n == EXAMPLE_FILE)
    }

    pub fn conflict(&self, lib: &Library) -> Conflict {
        match lib.get(self.kind()) {
            None => Conflict::None,
            Some(_) if lib.is_user(self.kind()) => Conflict::User,
            Some(_) => Conflict::Builtin,
        }
    }

    /// Writes the block into `user_dir` (`user_library/`), replacing an older copy.
    /// A built-in type is refused; an existing user block is replaced, so ask first
    /// (see [`Incoming::conflict`]).
    pub fn install(&self, user_dir: &Path, lib: &Library) -> Result<PathBuf, String> {
        if self.conflict(lib) == Conflict::Builtin {
            return Err(format!(
                "тип «{}» занят встроенным блоком, его заменить нельзя",
                self.kind()
            ));
        }
        // An older copy may sit in another folder.
        if let Some(old) = lib.get(self.kind()).filter(|_| lib.is_user(self.kind())) {
            let _ = fs::remove_dir_all(&old.dir);
        }
        let group = self.group();
        let pack_name = group.split('/').next().filter(|p| !p.is_empty());
        let pack_name = pack_name.unwrap_or(pack::DEFAULT_PACK);
        // A block always lives in a pack: make one if the folder is not a pack yet.
        let pack_dir = user_dir.join(pack_name);
        if !pack_dir.join(pack::PACK_FILE).is_file() {
            PackInfo::new(pack_name).write(&pack_dir)?;
        }
        let dir = if group.is_empty() {
            pack_dir.join(self.kind())
        } else {
            user_dir.join(&group).join(self.kind())
        };
        let _ = fs::remove_dir_all(&dir);
        write_files(&dir, &self.files)?;
        Ok(dir)
    }
}

/// Folder with the attached Arduino libraries of the block, to pass to arduino-cli as
/// `--libraries` (each subfolder is one library); `None` when the block has none.
pub fn library_dirs(block_dir: &Path) -> Option<PathBuf> {
    let dir = block_dir.join(LIB_DIR);
    dir.is_dir().then_some(dir)
}

/// Every file under `dir` as (relative path with `/`, contents), sorted.
pub fn read_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if let Ok(rel) = path.strip_prefix(root) {
                let name = rel.to_string_lossy().replace('\\', "/");
                out.push((name, fs::read(&path).map_err(|e| e.to_string())?));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

/// Writes relative files under `dir`, creating subfolders.
pub fn write_files(dir: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    for (name, data) in files {
        if !safe_path(name) {
            return Err(format!("небезопасный путь «{name}»"));
        }
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn safe_part(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && !part.contains([':', '\\', '/'])
}

pub(crate) fn safe_path(path: &str) -> bool {
    !path.starts_with('/') && path.split('/').all(safe_part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{save_block, user_library_dir};

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("flp_blockfile_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        (root, builtin)
    }

    #[test]
    fn install_keeps_library_and_example_and_makes_a_pack() {
        let (root, builtin) = setup("roundtrip");
        let user = user_library_dir(&builtin);
        let def: BlockDef = toml::from_str(
            "[block]
type = \"mine\"
name = \"N\"
group = \"Раздел\"
",
        )
        .unwrap();
        let dir = save_block(&user.join("Раздел"), &def, "// code", "docs").unwrap();
        fs::create_dir_all(dir.join("lib/Fancy/src")).unwrap();
        fs::write(dir.join("lib/Fancy/src/Fancy.h"), "// lib").unwrap();
        fs::write(dir.join(EXAMPLE_FILE), "zip").unwrap();
        let inc = Incoming::from_files(read_files(&dir).unwrap()).unwrap();
        assert_eq!(inc.libraries(), ["Fancy"]);
        assert!(inc.has_example());

        // A clean library elsewhere.
        fs::remove_dir_all(&user).unwrap();
        let (empty, _) = Library::load_with_user(&builtin).unwrap();
        assert_eq!(inc.conflict(&empty), Conflict::None);
        inc.install(&user, &empty).unwrap();
        assert!(user.join("Раздел").join(pack::PACK_FILE).is_file());
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert!(lib.is_user("mine"));
        let dir = &lib.get("mine").unwrap().dir;
        assert_eq!(
            fs::read_to_string(dir.join("lib/Fancy/src/Fancy.h")).unwrap(),
            "// lib"
        );
        assert_eq!(library_dirs(dir), Some(dir.join(LIB_DIR)));
        assert_eq!(inc.conflict(&lib), Conflict::User);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_files_are_refused() {
        let toml = b"[block]\ntype = \"mine\"\nname = \"N\"\ngroup = \"G\"\n".to_vec();
        Incoming::from_files(vec![("block.toml".into(), toml.clone())]).unwrap();
        let evil = vec![
            ("block.toml".into(), toml.clone()),
            ("../evil".into(), vec![]),
        ];
        assert!(Incoming::from_files(evil).is_err());
        assert!(Incoming::from_files(vec![("docs.md".into(), vec![])]).is_err());
        let bad_type = b"[block]\ntype = \"..\"\nname = \"N\"\ngroup = \"G\"\n".to_vec();
        assert!(Incoming::from_files(vec![("block.toml".into(), bad_type)]).is_err());
    }
}
