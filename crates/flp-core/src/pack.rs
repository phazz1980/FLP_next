//! A pack of user blocks: a folder of `user_library/` with `pack.toml` (name, version,
//! author) and block folders inside. The version belongs to the pack as a whole: editing
//! any block of the pack raises it. A pack travels as one `<name>.flpblocks` file, a ZIP
//! of that folder.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::blockfile::{read_files, safe_part, safe_path, write_files};
use crate::library::{BlockDef, Library};

/// Extension of a pack file.
pub const EXTENSION: &str = "flpblocks";
/// Description file in the root of a pack folder.
pub const PACK_FILE: &str = "pack.toml";
/// Pack that receives blocks created without choosing one.
pub const DEFAULT_PACK: &str = "Мои блоки";
/// Upper bound of the unpacked size, against archives that blow up.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Contents of `pack.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackInfo {
    /// Name of the pack; also the name of its folder.
    pub name: String,
    #[serde(default = "first_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// E-mail addresses, links, social networks: see [`contact_url`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
}

fn first_version() -> u32 {
    1
}

impl PackInfo {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            version: 1,
            description: String::new(),
            authors: Vec::new(),
            contacts: Vec::new(),
            license: String::new(),
        }
    }

    /// Fills the author and contacts if the pack has none yet.
    pub fn with_author(mut self, name: &str, contacts: &[String]) -> Self {
        if self.authors.is_empty() && !name.trim().is_empty() {
            self.authors.push(name.trim().to_owned());
        }
        if self.contacts.is_empty() {
            self.contacts = contacts
                .iter()
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty())
                .collect();
        }
        self
    }

    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(PACK_FILE);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let info: Self = toml::from_str(text).map_err(|e| format!("{PACK_FILE}: {e}"))?;
        if !safe_part(&info.name) {
            return Err(format!("недопустимое имя набора «{}»", info.name));
        }
        Ok(info)
    }

    pub fn write(&self, dir: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(PACK_FILE);
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Link for a contact: an address becomes `mailto:`, a link without scheme gets `https://`.
pub fn contact_url(contact: &str) -> String {
    let c = contact.trim();
    if c.contains("://") || c.starts_with("mailto:") {
        c.to_owned()
    } else if c.contains('@') && !c.starts_with('@') && !c.contains('/') {
        format!("mailto:{c}")
    } else {
        format!("https://{}", c.trim_start_matches('/'))
    }
}

/// Packs of the user library: top-level folders with a valid `pack.toml`, by folder name.
pub fn list(user_dir: &Path) -> Vec<(String, PackInfo)> {
    let Ok(entries) = fs::read_dir(user_dir) else {
        return Vec::new();
    };
    let mut packs: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let folder = e.file_name().to_string_lossy().into_owned();
            Some((folder, PackInfo::read(&e.path()).ok()?))
        })
        .collect();
    packs.sort_by(|a, b| a.0.cmp(&b.0));
    packs
}

/// Creates the pack folder `user_dir/<info.name>` with its `pack.toml`.
pub fn create(user_dir: &Path, info: &PackInfo) -> Result<PathBuf, String> {
    if !safe_part(&info.name) {
        return Err(format!("недопустимое имя набора «{}»", info.name));
    }
    let dir = user_dir.join(&info.name);
    if dir.join(PACK_FILE).exists() {
        return Err(format!("набор «{}» уже есть", info.name));
    }
    info.write(&dir)?;
    Ok(dir)
}

/// The pack whose folder holds `path` (a block folder or the folder it stands in).
pub fn find(path: &Path) -> Option<(PathBuf, PackInfo)> {
    path.ancestors()
        .find(|d| d.join(PACK_FILE).is_file())
        .and_then(|d| Some((d.to_owned(), PackInfo::read(d).ok()?)))
}

/// Raises the version of the pack that holds `path`; `None` when it is in no pack.
pub fn bump(path: &Path) -> Option<u32> {
    let (dir, mut info) = find(path)?;
    info.version += 1;
    info.write(&dir).ok()?;
    Some(info.version)
}

/// Saves the pack `folder` of the user library as one file.
pub fn export_pack(user_dir: &Path, folder: &str, dest: &Path) -> Result<(), String> {
    if !safe_part(folder) {
        return Err(format!("недопустимое имя набора «{folder}»"));
    }
    let files = read_files(&user_dir.join(folder))?;
    if !files.iter().any(|(n, _)| n == PACK_FILE) {
        return Err(format!("«{folder}» — не набор блоков"));
    }
    let out = fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in files {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// A pack read from a file, not yet in the library.
#[derive(Debug, Clone)]
pub struct IncomingPack {
    pub info: PackInfo,
    /// Files of the pack folder: relative path with `/` → contents.
    pub files: Vec<(String, Vec<u8>)>,
}

/// What an import does to the library, to ask the user when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackImport {
    /// No such pack yet.
    New,
    /// The installed pack is older: it is replaced as a whole.
    Update(u32),
    /// The installed pack has the same or a newer version: ask first.
    NotNewer(u32),
}

impl IncomingPack {
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, String> {
        if !files.iter().all(|(n, _)| safe_path(n)) {
            return Err("в файле небезопасные пути".into());
        }
        let (_, text) = files
            .iter()
            .find(|(n, _)| n == PACK_FILE)
            .ok_or("в файле нет pack.toml")?;
        let info = PackInfo::parse(std::str::from_utf8(text).map_err(|_| "pack.toml не в UTF-8")?)?;
        let pack = Self { info, files };
        for (_, def) in pack.blocks()? {
            def.validate(&def.info.kind)?;
        }
        Ok(pack)
    }

    /// Definitions of the blocks in the pack, with the folder of each.
    pub fn blocks(&self) -> Result<Vec<(String, BlockDef)>, String> {
        let mut out = Vec::new();
        for (name, data) in &self.files {
            let Some(dir) = name.strip_suffix("block.toml") else {
                continue;
            };
            if !dir.is_empty() && !dir.ends_with('/') {
                continue;
            }
            let text = std::str::from_utf8(data).map_err(|_| format!("{name} не в UTF-8"))?;
            let def: BlockDef = toml::from_str(text).map_err(|e| format!("{name}: {e}"))?;
            out.push((dir.trim_end_matches('/').to_owned(), def));
        }
        Ok(out)
    }

    pub fn kinds(&self) -> Vec<String> {
        self.blocks()
            .map(|b| b.into_iter().map(|(_, d)| d.info.kind).collect())
            .unwrap_or_default()
    }

    pub fn status(&self, user_dir: &Path) -> PackImport {
        match PackInfo::read(&user_dir.join(&self.info.name)) {
            Err(_) => PackImport::New,
            Ok(old) if old.version < self.info.version => PackImport::Update(old.version),
            Ok(old) => PackImport::NotNewer(old.version),
        }
    }

    /// Replaces the pack folder with the contents of the file: blocks that are not in
    /// the file disappear. A type taken by a built-in block or by a block of another
    /// pack is refused before anything is written.
    pub fn install(&self, user_dir: &Path, lib: &Library) -> Result<PathBuf, String> {
        let dir = user_dir.join(&self.info.name);
        for kind in self.kinds() {
            let Some(existing) = lib.get(&kind) else {
                continue;
            };
            if !lib.is_user(&kind) {
                return Err(format!("тип «{kind}» занят встроенным блоком"));
            }
            if !existing.dir.starts_with(&dir) {
                return Err(format!(
                    "тип «{kind}» уже есть в другом наборе или вне наборов"
                ));
            }
        }
        let _ = fs::remove_dir_all(&dir);
        write_files(&dir, &self.files)?;
        Ok(dir)
    }
}

/// Reads a `.flpblocks` file.
pub fn read_pack_file(path: &Path) -> Result<IncomingPack, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("{}: не архив набора ({e})", path.display()))?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        total += entry.size();
        if total > MAX_BYTES {
            return Err("файл набора слишком велик".into());
        }
        let name = entry.name().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        files.push((name, data));
    }
    IncomingPack::from_files(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{save_block, user_library_dir};

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("flp_pack_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        (root, builtin)
    }

    fn block(user: &Path, pack: &str, kind: &str, name: &str) {
        let def: BlockDef = toml::from_str(&format!(
            "[block]\ntype = \"{kind}\"\nname = \"{name}\"\ngroup = \"{pack}\"\n"
        ))
        .unwrap();
        save_block(&user.join(pack), &def, "// code", "").unwrap();
    }

    fn version(user: &Path, pack: &str) -> u32 {
        PackInfo::read(&user.join(pack)).unwrap().version
    }

    #[test]
    fn author_fills_only_empty_fields() {
        let info = PackInfo::new("P").with_author(" me ", &["a@b.ru".into(), " ".into()]);
        assert_eq!(info.authors, ["me"]);
        assert_eq!(info.contacts, ["a@b.ru"]);
        let kept = info.clone().with_author("other", &["x".into()]);
        assert_eq!(kept, info);
    }

    #[test]
    fn contacts_become_links() {
        assert_eq!(contact_url("me@mail.ru"), "mailto:me@mail.ru");
        assert_eq!(contact_url("t.me/name"), "https://t.me/name");
        assert_eq!(contact_url("https://vk.com/x"), "https://vk.com/x");
    }

    #[test]
    fn editing_a_block_raises_the_pack_version() {
        let (root, builtin) = setup("bump");
        let user = user_library_dir(&builtin);
        create(&user, &PackInfo::new("Мои")).unwrap();
        block(&user, "Мои", "a", "N");
        let v = version(&user, "Мои");
        // Saving an unchanged block keeps the version.
        block(&user, "Мои", "a", "N");
        assert_eq!(version(&user, "Мои"), v);
        block(&user, "Мои", "a", "Other");
        assert_eq!(version(&user, "Мои"), v + 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_import_replaces_the_pack_as_a_whole() {
        let (root, builtin) = setup("roundtrip");
        let user = user_library_dir(&builtin);
        let mut info = PackInfo::new("Мои");
        info.authors = vec!["me".into()];
        info.contacts = vec!["me@mail.ru".into()];
        create(&user, &info).unwrap();
        block(&user, "Мои", "a", "N");
        block(&user, "Мои", "b", "N");
        let file = root.join("my.flpblocks");
        export_pack(&user, "Мои", &file).unwrap();
        let pack = read_pack_file(&file).unwrap();
        assert_eq!(pack.info.contacts, ["me@mail.ru"]);
        let mut kinds = pack.kinds();
        kinds.sort();
        assert_eq!(kinds, ["a", "b"]);
        assert!(matches!(pack.status(&user), PackImport::NotNewer(_)));

        // A newer file without "b" replaces the pack as a whole.
        let mut newer = pack.clone();
        newer.info.version += 1;
        for f in &mut newer.files {
            if f.0 == PACK_FILE {
                f.1 = toml::to_string(&newer.info).unwrap().into_bytes();
            }
        }
        newer.files.retain(|(n, _)| !n.starts_with("b/"));
        assert!(matches!(newer.status(&user), PackImport::Update(_)));
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        newer.install(&user, &lib).unwrap();
        assert!(user.join("Мои/a/block.toml").is_file());
        assert!(!user.join("Мои/b").exists());
        assert_eq!(version(&user, "Мои"), newer.info.version);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn type_of_another_pack_is_refused() {
        let (root, builtin) = setup("conflict");
        let user = user_library_dir(&builtin);
        create(&user, &PackInfo::new("Первый")).unwrap();
        block(&user, "Первый", "a", "N");
        let file = root.join("p.flpblocks");
        export_pack(&user, "Первый", &file).unwrap();
        let mut pack = read_pack_file(&file).unwrap();
        pack.info.name = "Второй".into();
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        let err = pack.install(&user, &lib).unwrap_err();
        assert!(err.contains("«a»"), "{err}");
        let _ = fs::remove_dir_all(root);
    }
}
