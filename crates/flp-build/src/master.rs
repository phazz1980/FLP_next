//! Master firmware of the WASM-on-board scheme (`specs/wasm-on-mcu`): the sketch is part
//! of the program, so the board can always be given the master that matches it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{board_args, build_dir, check_status, plain_path, ArduinoCli, BuildError};
use flp_core::BoardConfig;

/// The master sketch (wasm3, LittleFS, serial protocol).
const SKETCH: &str = include_str!("../../../specs/wasm-on-mcu/proto/master/master.ino");
/// Identifies the embedded master sketch: changes when the firmware (protocol, ABI) does.
pub fn master_sketch_id() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    SKETCH.hash(&mut hasher);
    hasher.finish()
}

/// Arduino libraries the master needs: wasm3 and the sensor drivers.
const LIBRARIES: [&str; 5] = [
    "Wasm3",
    "OneWire",
    "DallasTemperature",
    "DHT sensor library",
    "Adafruit Unified Sensor",
];

/// Compiled master firmware, ready to be flashed.
pub struct MasterBuild {
    sketch_dir: PathBuf,
    out: PathBuf,
}

impl ArduinoCli {
    /// Builds the master firmware in `<work_dir>/master` without touching the board.
    /// `low_priority` keeps a background build from slowing the editor down.
    pub fn compile_master(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        low_priority: bool,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<MasterBuild, BuildError> {
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
        let installed = self.ensure_libraries(&LIBRARIES, on_line)?;
        if !installed.is_empty() {
            on_line(&format!("Установлены библиотеки: {}", installed.join(", ")));
        }
        let out = build_dir(&root);

        let mut args: Vec<OsString> = vec!["compile".into()];
        args.extend(board_args(board));
        args.push("--build-path".into());
        args.push(out.clone().into());
        args.push(sketch_dir.clone().into());
        let status = self.run(&args, low_priority, on_line)?;
        check_status("сборка мастера", status)?;
        Ok(MasterBuild { sketch_dir, out })
    }

    /// Flashes a compiled master to `port`. This replaces whatever firmware the board runs.
    pub fn flash_master(
        &self,
        build: &MasterBuild,
        board: &BoardConfig,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let mut args: Vec<OsString> = vec!["upload".into()];
        args.extend(board_args(board));
        args.push("--port".into());
        args.push(port.into());
        args.push("--input-dir".into());
        args.push(build.out.clone().into());
        args.push(build.sketch_dir.clone().into());
        let status = self.run(&args, false, on_line)?;
        check_status("заливка мастера", status)
    }

    /// Builds the master firmware in `<work_dir>/master` and uploads it to `port`.
    pub fn upload_master(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let build = self.compile_master(work_dir, board, false, on_line)?;
        self.flash_master(&build, board, port, on_line)
    }
}
