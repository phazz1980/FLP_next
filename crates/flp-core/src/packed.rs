//! User blocks inside a project file: the archive carries a copy of every user block
//! the schema uses (`user_blocks/<group>/<type>/<file>`), so a project opens on another
//! computer without copying `user_library/` by hand.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

use crate::library::Library;
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

/// Archive folder with the user board of the project.
pub const PACKED_BOARDS: &str = "user_boards";

/// Archive entries (name, contents) for the user board the project is made for: its
/// `board.toml`, text and pictures. Empty for a built-in board or no board.
pub fn pack_user_board(model: Option<&str>, lib: &Library) -> Vec<(String, Vec<u8>)> {
    let Some(board) = model.and_then(|id| lib.boards().get(id)) else {
        return Vec::new();
    };
    let Ok(files) = crate::blockfile::read_files(&board.dir) else {
        return Vec::new();
    };
    let id = &board.def.info.id;
    files
        .into_iter()
        .map(|(name, data)| (format!("{PACKED_BOARDS}/{id}/{name}"), data))
        .collect()
}

/// Reads the packed boards of the project file `project`: id → files. A project folder
/// (the old format, used by `examples/`) keeps them in `user_boards/<id>/` as they are.
fn read_packed_boards(project: &Path) -> BTreeMap<String, Vec<PackedFile>> {
    let mut boards = if project.is_dir() {
        read_folder_boards(project)
    } else {
        read_archive_boards(project)
    };
    boards.retain(|id, files| {
        crate::boards::valid_id(id) && files.iter().any(|(n, _)| n == crate::boards::BOARD_FILE)
    });
    boards
}

fn read_folder_boards(project: &Path) -> BTreeMap<String, Vec<PackedFile>> {
    let mut boards = BTreeMap::new();
    let entries = fs::read_dir(project.join(PACKED_BOARDS));
    for entry in entries.into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Ok(files) = crate::blockfile::read_files(&entry.path()) {
            boards.insert(name, files);
        }
    }
    boards
}

fn read_archive_boards(project: &Path) -> BTreeMap<String, Vec<PackedFile>> {
    let mut boards: BTreeMap<String, Vec<PackedFile>> = BTreeMap::new();
    let Ok(file) = fs::File::open(project) else {
        return boards;
    };
    let Ok(mut zip) = ZipArchive::new(std::io::BufReader::new(file)) else {
        return boards;
    };
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let name = entry.name().replace('\\', "/");
        let Some(rest) = name.strip_prefix(&format!("{PACKED_BOARDS}/")) else {
            continue;
        };
        let parts: Vec<&str> = rest.split('/').collect();
        if entry.is_dir() || parts.len() < 2 || !parts.iter().all(|p| safe_part(p)) {
            continue;
        }
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_ok() {
            boards
                .entry(parts[0].to_owned())
                .or_default()
                .push((parts[1..].join("/"), data));
        }
    }
    boards
}

/// Restores the user board packed into the project file `project` into
/// `<user_dir>/boards/` if `lib` does not know it. A folder that already exists is not
/// touched, even a broken one, and a packed board that does not pass the check is
/// not written. Returns the ids written; the caller reloads the library when it is not
/// empty.
pub fn unpack_missing_boards(project: &Path, user_dir: &Path, lib: &Library) -> Vec<String> {
    let boards_dir = crate::boards::boards_dir(user_dir);
    let mut written = Vec::new();
    for (id, files) in read_packed_boards(project) {
        let dir = boards_dir.join(&id);
        if lib.boards().get(&id).is_some() || dir.exists() {
            continue;
        }
        let ok = crate::blockfile::write_files(&dir, &files).is_ok()
            && crate::boards::UserBoard::load(&dir).is_ok();
        if ok {
            written.push(id);
        } else {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    written
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

/// A user block whose copy in the project differs from the one in the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDiff {
    pub kind: String,
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

/// User blocks that exist both in the project file and in `lib` with different
/// contents. Blocks have no versions of their own, so which copy is newer is for the
/// user to say.
pub fn version_diffs(project: &Path, lib: &Library) -> Vec<VersionDiff> {
    read_packed(project)
        .into_iter()
        .filter_map(|((_, kind), files)| {
            let local = lib.get(&kind).filter(|_| lib.is_user(&kind))?;
            (!same_files(&files, &local.dir)).then_some(VersionDiff { kind })
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

        // The program's block is edited: its contents differ from the packed copy.
        let mut def = lib.get("mine").unwrap().def.clone();
        def.info.description = "changed".into();
        let dir = lib.get("mine").unwrap().dir.parent().unwrap().to_owned();
        save_block(&dir, &def, "// code 2", "docs").unwrap();
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert_eq!(
            version_diffs(&project.path, &lib),
            [VersionDiff {
                kind: "mine".into()
            }]
        );
        // Taking the project's copy brings the old contents back.
        assert_eq!(
            adopt_project_blocks(&project.path, &user, &["mine".to_owned()]),
            ["mine"]
        );
        let (lib, _) = Library::load_with_user(&builtin).unwrap();
        assert!(version_diffs(&project.path, &lib).is_empty());
        // Edited by hand elsewhere: reported again.
        fs::write(
            lib.get("mine").unwrap().dir.join("esp32.cpp.tmpl"),
            "// other",
        )
        .unwrap();
        let diffs = version_diffs(&project.path, &lib);
        assert_eq!(diffs.len(), 1);
        fs::remove_dir_all(&root).unwrap();
    }

    const BOARD_TOML: &str = "[board]
id = \"mine\"
name = \"Моя плата\"
chip = \"esp32\"
fqbn = \"esp32:esp32:esp32\"

[[pin]]
gpio = 4
name = \"OUT1\"
";

    #[test]
    fn the_user_board_travels_in_the_project() {
        let root = std::env::temp_dir().join(format!("flp_packed_board_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        let user = user_library_dir(&builtin);
        let board_dir = crate::boards::boards_dir(&user).join("mine");
        fs::create_dir_all(board_dir.join("images")).unwrap();
        fs::write(board_dir.join("board.toml"), BOARD_TOML).unwrap();
        fs::write(board_dir.join("docs.md"), "Описание").unwrap();
        fs::write(board_dir.join("images/scheme.png"), [1u8, 2, 3]).unwrap();
        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        let project = Project {
            path: root.join("p.flpn"),
            config: toml::from_str(
                "[project]
name = \"t\"
[board]
fqbn = \"esp32:esp32:esp32\"
model = \"mine\"
",
            )
            .unwrap(),
            logic: Logic::default(),
            layout: Default::default(),
        };
        let entries = pack_user_board(project.config.board.model.as_deref(), &lib);
        let names: Vec<_> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "user_boards/mine/board.toml",
                "user_boards/mine/docs.md",
                "user_boards/mine/images/scheme.png"
            ]
        );
        // A built-in board or no board packs nothing.
        assert!(pack_user_board(Some("esp32"), &lib).is_empty());
        assert!(pack_user_board(None, &lib).is_empty());
        let extra: Vec<(&str, &[u8])> = entries
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        project.save_with(&extra).unwrap();

        // Another computer: a clean library knows no board.
        let clean_user = root.join("other").join("user_library");
        let none = Library::default();
        let written = unpack_missing_boards(&project.path, &clean_user, &none);
        assert_eq!(written, ["mine"]);
        let restored = crate::boards::boards_dir(&clean_user).join("mine");
        assert_eq!(
            fs::read(restored.join("images/scheme.png")).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(
            fs::read_to_string(restored.join("docs.md")).unwrap(),
            "Описание"
        );
        // Known, or already in a folder: left alone.
        assert!(unpack_missing_boards(&project.path, &clean_user, &none).is_empty());
        assert!(unpack_missing_boards(&project.path, &user, &lib).is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_project_folder_carries_its_board_in_user_boards() {
        let root = std::env::temp_dir().join(format!("flp_folder_board_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let project = root.join("project");
        let board = project.join("user_boards/mine");
        fs::create_dir_all(board.join("images")).unwrap();
        fs::write(board.join("board.toml"), BOARD_TOML).unwrap();
        fs::write(board.join("images/scheme.png"), [1u8, 2, 3]).unwrap();
        // A folder without `board.toml` and one with a bad id are not boards.
        fs::create_dir_all(project.join("user_boards/empty")).unwrap();
        fs::create_dir_all(project.join("user_boards/Bad Id")).unwrap();
        fs::write(project.join("user_boards/Bad Id/board.toml"), BOARD_TOML).unwrap();

        let user = root.join("user_library");
        let written = unpack_missing_boards(&project, &user, &Library::default());
        assert_eq!(written, ["mine"]);
        let restored = crate::boards::boards_dir(&user).join("mine");
        assert_eq!(
            fs::read(restored.join("images/scheme.png")).unwrap(),
            [1, 2, 3]
        );
        assert!(unpack_missing_boards(&project, &user, &Library::default()).is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_broken_packed_board_is_not_written() {
        let root =
            std::env::temp_dir().join(format!("flp_packed_bad_board_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            path: root.join("p.flpn"),
            config: toml::from_str("[project]\nname = \"t\"\n[board]\nfqbn = \"x:y:z\"\n").unwrap(),
            logic: Logic::default(),
            layout: Default::default(),
        };
        let bad = BOARD_TOML.replace("chip = \"esp32\"", "chip = \"avr\"");
        let extra: Vec<(&str, &[u8])> = vec![
            ("user_boards/mine/board.toml", bad.as_bytes()),
            ("user_boards/mine/../escape.txt", b"x"),
            ("user_boards/Bad Id/board.toml", BOARD_TOML.as_bytes()),
        ];
        project.save_with(&extra).unwrap();
        let user = root.join("user_library");
        let written = unpack_missing_boards(&project.path, &user, &Library::default());
        assert!(written.is_empty(), "{written:?}");
        assert!(!crate::boards::boards_dir(&user).join("mine").exists());
        assert!(!root.join("escape.txt").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
