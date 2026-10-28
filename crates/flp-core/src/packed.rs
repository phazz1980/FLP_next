//! User blocks inside a project file: the archive carries a copy of every user block
//! the schema uses (`user_blocks/<group>/<type>/<file>`), so a project opens on another
//! computer without copying `user_library/` by hand.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

use crate::library::{BlockDef, Library};
use crate::logic::Logic;

/// File name and contents of a packed block file.
type PackedFile = (String, Vec<u8>);

/// Archive folder with the packed user blocks.
pub const PACKED_DIR: &str = "user_blocks";

/// Archive entries (name, contents) for the user blocks that `logic` uses.
pub fn pack_user_blocks(logic: &Logic, lib: &Library) -> Vec<(String, Vec<u8>)> {
    let mut kinds: Vec<&str> = logic.blocks.iter().map(|b| b.kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let mut out = Vec::new();
    for kind in kinds.into_iter().filter(|k| lib.is_user(k)) {
        let Some(block) = lib.get(kind) else { continue };
        let group = block.def.info.group.trim_matches('/');
        let prefix = if group.is_empty() {
            format!("{PACKED_DIR}/{kind}")
        } else {
            format!("{PACKED_DIR}/{group}/{kind}")
        };
        // Attached libraries and the example travel with the block.
        let Ok(files) = crate::blockfile::read_files(&block.dir) else {
            continue;
        };
        for (name, data) in files {
            out.push((format!("{prefix}/{name}"), data));
        }
    }
    out
}

/// Packed copies of user blocks: (group folder, type) → files.
type PackedBlocks = BTreeMap<(String, String), Vec<PackedFile>>;

/// Reads every packed block of the project file `project`.
fn read_packed(project: &Path) -> PackedBlocks {
    let mut blocks = PackedBlocks::new();
    let Ok(file) = fs::File::open(project) else {
        return blocks;
    };
    let Ok(mut zip) = ZipArchive::new(std::io::BufReader::new(file)) else {
        return blocks;
    };
    let mut entries: Vec<(Vec<String>, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let name = entry.name().replace('\\', "/");
        let Some(rest) = name.strip_prefix(&format!("{PACKED_DIR}/")) else {
            continue;
        };
        let parts: Vec<String> = rest.split('/').map(str::to_owned).collect();
        if entry.is_dir() || parts.len() < 2 || !parts.iter().all(|p| safe_part(p)) {
            continue;
        }
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_ok() {
            entries.push((parts, data));
        }
    }
    // A block folder is where its `block.toml` is; deeper files (`lib/...`) belong to it.
    let dirs: Vec<Vec<String>> = entries
        .iter()
        .filter(|(p, _)| p.last().is_some_and(|n| n == "block.toml") && p.len() >= 2)
        .map(|(p, _)| p[..p.len() - 1].to_vec())
        .collect();
    for (parts, data) in entries {
        let Some(dir) = dirs
            .iter()
            .filter(|d| parts.len() > d.len() && parts.starts_with(d))
            .max_by_key(|d| d.len())
        else {
            continue;
        };
        let (kind, group) = dir.split_last().expect("block dir is not empty");
        blocks
            .entry((group.join("/"), kind.clone()))
            .or_default()
            .push((parts[dir.len()..].join("/"), data));
    }
    blocks
}

/// Writes a packed block into `user_dir`, replacing the files of an older copy.
fn write_block(user_dir: &Path, group: &str, kind: &str, files: &[PackedFile]) -> bool {
    let dir = user_dir.join(group).join(kind);
    let _ = fs::remove_dir_all(&dir);
    crate::blockfile::write_files(&dir, files).is_ok()
}

/// Restores the packed user blocks of the project file `project` that are missing in
/// `lib` into `user_dir` (the `user_library/` folder). Blocks that already exist stay as
/// they are. Returns the types written; the caller reloads the library when it is not empty.
pub fn unpack_missing_blocks(project: &Path, user_dir: &Path, lib: &Library) -> Vec<String> {
    read_packed(project)
        .into_iter()
        .filter(|((_, kind), _)| lib.get(kind).is_none())
        .filter(|((group, kind), files)| write_block(user_dir, group, kind, files))
        .map(|((_, kind), _)| kind)
        .collect()
}

/// A user block whose copy in the project and in the program differ in revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDiff {
    pub kind: String,
    /// Revision packed in the project.
    pub project: u32,
    /// Revision in the program's `user_library/`.
    pub program: u32,
}

impl VersionDiff {
    /// Same revision but different contents: the block was edited on two computers.
    pub fn diverged(&self) -> bool {
        self.project == self.program
    }
}

/// Whether the packed files equal the files in `dir` (line endings are ignored).
fn same_files(files: &[PackedFile], dir: &Path) -> bool {
    let norm = |d: &[u8]| {
        d.iter()
            .copied()
            .filter(|b| *b != b'\r')
            .collect::<Vec<u8>>()
    };
    let Ok(read) = crate::blockfile::read_files(dir) else {
        return false;
    };
    let local: BTreeMap<String, Vec<u8>> = read.into_iter().map(|(n, d)| (n, norm(&d))).collect();
    let packed: BTreeMap<String, Vec<u8>> =
        files.iter().map(|(n, d)| (n.clone(), norm(d))).collect();
    local == packed
}

/// Revision of a packed `block.toml`; `None` when it cannot be read.
fn packed_version(files: &[PackedFile]) -> Option<u32> {
    let (_, data) = files.iter().find(|(n, _)| n == "block.toml")?;
    let def: BlockDef = toml::from_str(std::str::from_utf8(data).ok()?).ok()?;
    Some(def.info.version)
}

/// User blocks that exist both in the project file and in `lib` with different
/// revisions or, at the same revision, different contents. A larger `project` revision means the project carries a newer block;
/// a larger `program` revision means the program's block is newer.
pub fn version_diffs(project: &Path, lib: &Library) -> Vec<VersionDiff> {
    read_packed(project)
        .into_iter()
        .filter_map(|((_, kind), files)| {
            let local = lib.get(&kind).filter(|_| lib.is_user(&kind))?;
            let packed = packed_version(&files)?;
            let program = local.def.info.version;
            (packed != program || !same_files(&files, &local.dir)).then_some(VersionDiff {
                kind,
                project: packed,
                program,
            })
        })
        .collect()
}

/// User blocks packed in the project file `project`, to offer them for import.
pub fn project_blocks(project: &Path) -> Vec<crate::blockfile::Incoming> {
    read_packed(project)
        .into_values()
        .filter_map(|files| crate::blockfile::Incoming::from_files(files).ok())
        .collect()
}

/// Replaces the program's user blocks `kinds` with the copies packed in the project
/// (each at its place in `user_dir`). Returns the types written.
pub fn adopt_project_blocks(project: &Path, user_dir: &Path, kinds: &[String]) -> Vec<String> {
    read_packed(project)
        .into_iter()
        .filter(|((_, kind), _)| kinds.contains(kind))
        .filter(|((group, kind), files)| write_block(user_dir, group, kind, files))
        .map(|((_, kind), _)| kind)
        .collect()
}

fn safe_part(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && !part.contains([':', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{save_block, user_library_dir, BlockDef};
    use crate::logic::Block;
    use crate::project::Project;

    #[test]
    fn safe_parts() {
        assert!(safe_part("Насосы"));
        assert!(!safe_part(".."));
        assert!(!safe_part(""));
        assert!(!safe_part("C:"));
    }

    #[test]
    fn user_blocks_travel_in_the_project() {
        let root = std::env::temp_dir().join(format!("flp_packed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        let user = user_library_dir(&builtin);
        let def: BlockDef =
            toml::from_str("[block]\ntype = \"mine\"\nname = \"N\"\ngroup = \"Раздел/Папка\"\n")
                .unwrap();
        let bdir = save_block(&user.join("Раздел").join("Папка"), &def, "// code", "docs").unwrap();
        fs::create_dir_all(bdir.join("lib/Fancy")).unwrap();
        fs::write(bdir.join("lib/Fancy/Fancy.h"), "// lib").unwrap();
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert!(lib.is_user("mine"));

        let mut logic = Logic::default();
        logic.blocks.push(Block {
            id: "a".into(),
            kind: "mine".into(),
            ..Default::default()
        });
        let entries = pack_user_blocks(&logic, &lib);
        assert!(entries
            .iter()
            .any(|(n, _)| n == "user_blocks/Раздел/Папка/mine/block.toml"));

        let project = Project {
            path: root.join("p.flpn"),
            config: toml::from_str(
                "[project]
name = \"t\"
[board]
fqbn = \"x:y:z\"
",
            )
            .unwrap(),
            logic,
            layout: Default::default(),
        };
        let extra: Vec<(&str, &[u8])> = entries
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        project.save_with(&extra).unwrap();

        let offered = project_blocks(&project.path);
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].libraries(), ["Fancy"]);

        // Another computer: no user library at all.
        fs::remove_dir_all(&user).unwrap();
        let (empty, _) = Library::load_with_user(&builtin).unwrap();
        let written = unpack_missing_blocks(&project.path, &user, &empty);
        assert_eq!(written, ["mine"]);
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert!(lib.is_user("mine"));
        assert_eq!(lib.get("mine").unwrap().def.info.group, "Раздел/Папка");
        // Already present: nothing is overwritten.
        assert!(unpack_missing_blocks(&project.path, &user, &lib).is_empty());
        assert!(version_diffs(&project.path, &lib).is_empty());

        // The program's block is edited: its revision rises above the packed one.
        let mut def = lib.get("mine").unwrap().def.clone();
        def.info.description = "changed".into();
        let dir = lib.get("mine").unwrap().dir.parent().unwrap().to_owned();
        save_block(&dir, &def, "// code 2", "docs").unwrap();
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert_eq!(
            version_diffs(&project.path, &lib),
            [VersionDiff {
                kind: "mine".into(),
                project: 1,
                program: 2
            }]
        );
        // Taking the project's copy brings the old revision back.
        assert_eq!(
            adopt_project_blocks(&project.path, &user, &["mine".to_owned()]),
            ["mine"]
        );
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert!(version_diffs(&project.path, &lib).is_empty());
        // Same revision, edited by hand elsewhere: reported as diverged.
        fs::write(
            lib.get("mine").unwrap().dir.join("esp32.cpp.tmpl"),
            "// other",
        )
        .unwrap();
        let diffs = version_diffs(&project.path, &lib);
        assert!(diffs.len() == 1 && diffs[0].diverged());
        fs::remove_dir_all(&root).unwrap();
    }
}
