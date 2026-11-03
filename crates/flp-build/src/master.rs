//! Master firmware of the WASM-on-board scheme (`specs/wasm-on-mcu`): the sketch is part
//! of the program, so the board can always be given the master that matches it.

use std::ffi::OsString;
use std::path::Path;

use crate::{board_args, build_dir, check_status, plain_path, ArduinoCli, BuildError};
use flp_core::BoardConfig;

/// The master sketch (wasm3, LittleFS, serial protocol).
const SKETCH: &str = include_str!("../../../specs/wasm-on-mcu/proto/master/master.ino");
/// Arduino library the master needs.
const LIBRARY: &str = "Wasm3";

impl ArduinoCli {
    /// Builds the master firmware in `<work_dir>/master` and uploads it to `port`.
    /// This replaces whatever firmware the board runs.
    pub fn upload_master(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let root = plain_path(work_dir).join("master");
        let sketch_dir = root.join("master");
        let sketch = sketch_dir.join("master.ino");
        std::fs::create_dir_all(&sketch_dir).map_err(|e| BuildError::BadOutput(e.to_string()))?;
        // Untouched sources keep arduino-cli's cache valid.
        if std::fs::read_to_string(&sketch).ok().as_deref() != Some(SKETCH) {
            std::fs::write(&sketch, SKETCH).map_err(|e| BuildError::BadOutput(e.to_string()))?;
        }
        if let Some(core) = self.ensure_core(&board.fqbn, on_line)? {
            on_line(&format!("Установлено ядро {core}"));
        }
        let installed = self.ensure_libraries(&[LIBRARY], on_line)?;
        if !installed.is_empty() {
            on_line(&format!("Установлены библиотеки: {}", installed.join(", ")));
        }
        let out = build_dir(&root);

        let mut args: Vec<OsString> = vec!["compile".into()];
        args.extend(board_args(board));
        args.push("--build-path".into());
        args.push(out.clone().into());
        args.push(sketch_dir.clone().into());
        let status = self.run(&args, false, on_line)?;
        check_status("сборка мастера", status)?;

        let mut args: Vec<OsString> = vec!["upload".into()];
        args.extend(board_args(board));
        args.push("--port".into());
        args.push(port.into());
        args.push("--input-dir".into());
        args.push(out.into());
        args.push(sketch_dir.into());
        let status = self.run(&args, false, on_line)?;
        check_status("заливка мастера", status)
    }
}
