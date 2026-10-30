//! A user board as one file: `<id>.flpboard` is a ZIP with the files of the board folder
//! (`board.toml`, `docs.md`, `images/…`).

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::blockfile::{read_files, safe_path, write_files};
use crate::boards::{boards_dir, BoardDef, BOARD_FILE};
use crate::library::Library;

/// Extension of a board file.
pub const EXTENSION: &str = "flpboard";
/// Upper bound of the unpacked size, against archives that blow up.
const MAX_BYTES: u64 = 128 * 1024 * 1024;

/// A board read from a file, not yet in the library.
#[derive(Debug, Clone)]
pub struct IncomingBoard {
    pub def: BoardDef,
    /// Files of the board folder: relative path with `/` → contents.
    pub files: Vec<(String, Vec<u8>)>,
}

/// What stands in the way of installing an [`IncomingBoard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardConflict {
    None,
    /// A user board with this id exists; its revision.
    User(u32),
}

impl IncomingBoard {
    /// Builds a board from folder files; `board.toml` must be among them and valid.
    pub fn from_files(files: Vec<(String, Vec<u8>)>) -> Result<Self, String> {
        if !files.iter().all(|(n, _)| safe_path(n)) {
            return Err("в файле небезопасные пути".into());
        }
        let (_, toml) = files
            .iter()
            .find(|(n, _)| n == BOARD_FILE)
            .ok_or("в файле нет board.toml")?;
        let text = std::str::from_utf8(toml).map_err(|_| "board.toml не в UTF-8")?;
        let def: BoardDef = toml::from_str(text).map_err(|e| format!("board.toml: {e}"))?;
        def.validate(Some(&def.info.id))?;
        Ok(Self { def, files })
    }

    pub fn id(&self) -> &str {
        &self.def.info.id
    }

    pub fn name(&self) -> &str {
        &self.def.info.name
    }

    pub fn conflict(&self, lib: &Library) -> BoardConflict {
        match lib.boards().get(self.id()) {
            None => BoardConflict::None,
            Some(b) => BoardConflict::User(b.def.info.version),
        }
    }

    /// Writes the board into `user_dir` (`user_library/`), replacing an older copy, so
    /// ask first (see [`IncomingBoard::conflict`]). The caller reloads the library.
    pub fn install(&self, user_dir: &Path) -> Result<PathBuf, String> {
        let dir = boards_dir(user_dir).join(self.id());
        let _ = fs::remove_dir_all(&dir);
        write_files(&dir, &self.files)?;
        Ok(dir)
    }
}

/// Reads a `.flpboard` file.
pub fn read_board_file(path: &Path) -> Result<IncomingBoard, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("{}: не архив платы ({e})", path.display()))?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        total += entry.size();
        if total > MAX_BYTES {
            return Err("файл платы слишком велик".into());
        }
        let name = entry.name().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        files.push((name, data));
    }
    IncomingBoard::from_files(files)
}

/// Saves the user board `id` as one file.
pub fn export_board(lib: &Library, id: &str, dest: &Path) -> Result<(), String> {
    let board = lib
        .boards()
        .get(id)
        .ok_or_else(|| format!("«{id}» — не пользовательская плата"))?;
    let files = read_files(&board.dir)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::user_library_dir;

    const BOARD_TOML: &str = "[board]
id = \"mine\"
name = \"Моя плата\"
chip = \"esp32\"
fqbn = \"esp32:esp32:esp32\"

[[pin]]
gpio = 4
name = \"OUT1\"
";

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("flp_boardfile_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        (root, builtin)
    }

    #[test]
    fn export_import_keeps_every_file() {
        let (root, builtin) = setup("roundtrip");
        let user = user_library_dir(&builtin);
        let dir = boards_dir(&user).join("mine");
        fs::create_dir_all(dir.join("images")).unwrap();
        fs::write(dir.join("board.toml"), BOARD_TOML).unwrap();
        fs::write(dir.join("docs.md"), "Описание ![схема](images/scheme.png)").unwrap();
        fs::write(dir.join("images/scheme.png"), [9u8, 8, 7]).unwrap();
        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        let file = root.join("mine.flpboard");
        export_board(&lib, "mine", &file).unwrap();
        assert!(export_board(&lib, "esp32", &root.join("x.flpboard")).is_err());
        let incoming = read_board_file(&file).unwrap();
        assert_eq!((incoming.id(), incoming.name()), ("mine", "Моя плата"));
        assert_eq!(incoming.conflict(&lib), BoardConflict::User(1));

        // A clean library elsewhere.
        fs::remove_dir_all(&user).unwrap();
        let (empty, _) = Library::load_with_user(&builtin).unwrap();
        assert_eq!(incoming.conflict(&empty), BoardConflict::None);
        incoming.install(&user).unwrap();
        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let board = lib.boards().get("mine").unwrap();
        assert_eq!(board.def.pins.len(), 1);
        assert_eq!(
            fs::read(board.dir.join("images/scheme.png")).unwrap(),
            [9, 8, 7]
        );
        // Installing again replaces the folder: a file that is gone from the board is gone.
        fs::write(board.dir.join("stale.txt"), "x").unwrap();
        incoming.install(&user).unwrap();
        assert!(!board.dir.join("stale.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_files_are_refused() {
        let toml = BOARD_TOML.as_bytes().to_vec();
        IncomingBoard::from_files(vec![("board.toml".into(), toml.clone())]).unwrap();
        let evil = vec![
            ("board.toml".into(), toml.clone()),
            ("../evil".into(), vec![]),
        ];
        assert!(IncomingBoard::from_files(evil).is_err());
        assert!(IncomingBoard::from_files(vec![("docs.md".into(), vec![])]).is_err());
        // The id of a built-in board, a chip that does not exist, a repeated pin.
        for bad in [
            BOARD_TOML.replace("\"mine\"", "\"esp32c3_supermini\""),
            BOARD_TOML.replace("\"esp32\"\nfqbn", "\"avr\"\nfqbn"),
            format!("{BOARD_TOML}\n[[pin]]\ngpio = 4\n"),
            BOARD_TOML.replace("\"mine\"", "\"../mine\""),
        ] {
            let err =
                IncomingBoard::from_files(vec![("board.toml".into(), bad.clone().into_bytes())])
                    .unwrap_err();
            assert!(!err.is_empty(), "{bad}");
        }
    }
}
