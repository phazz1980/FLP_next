//! Master firmware of the WASM-on-board scheme (`specs/wasm-on-mcu`): the sketch is part
//! of the program, so the board can always be given the master that matches it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{board_args, build_dir, check_status, plain_path, ArduinoCli, BuildError};
use flp_core::BoardConfig;

/// The master sketch (wasm3, LittleFS, serial protocol).
const SKETCH: &str = include_str!("../../../specs/wasm-on-mcu/proto/master/master.ino");
/// Hardware drivers the master carries. Each one costs flash and RAM, so the master is built
/// with only those the project's blocks need (`#define FLP_<NAME>` in front of the sketch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MasterFeatures {
    /// DS18B20 and DHT22 (`sensor_read`).
    pub sensors: bool,
    /// PCF8574 expanders on I²C (`pcf_read`, `pcf_write`).
    pub pcf: bool,
    /// Modbus RTU master and slave (`modbus`); the libraries come from `libs/`.
    pub modbus: bool,
    /// UART bytes (`uart`): `uart_read`, `uart_send`, `uart_config`.
    pub uart: bool,
}

impl MasterFeatures {
    /// The drivers the given block types (`ds18b20`, `pcf8574_in`...) need.
    pub fn of_blocks<'a>(kinds: impl IntoIterator<Item = &'a str>) -> Self {
        let mut f = Self::default();
        for kind in kinds {
            match kind {
                "ds18b20" | "dht22" => f.sensors = true,
                "pcf8574_in" | "pcf8574_out" => f.pcf = true,
                "modbus_rtu_master" | "modbus_rtu_slave" => f.modbus = true,
                "uart_read" | "uart_send" | "uart_config" => f.uart = true,
                _ => {}
            }
        }
        f
    }

    fn defines(&self) -> String {
        let mut text = String::new();
        for (on, name) in [
            (self.sensors, "FLP_SENSORS"),
            (self.pcf, "FLP_PCF"),
            (self.modbus, "FLP_MODBUS"),
            (self.uart, "FLP_UART"),
        ] {
            if on {
                text.push_str(&format!(
                    "#define {name} 1
"
                ));
            }
        }
        text
    }

    fn libraries(&self) -> Vec<&'static str> {
        let mut libs = vec!["Wasm3"];
        if self.sensors {
            libs.extend([
                "OneWire",
                "DallasTemperature",
                "DHT sensor library",
                "Adafruit Unified Sensor",
            ]);
        }
        libs
    }
}

/// The master sketch with the defines of its drivers in front.
fn sketch_text(features: &MasterFeatures) -> String {
    format!("{}{SKETCH}", features.defines())
}

/// Identifies the master sketch with these drivers: changes when the firmware (protocol,
/// ABI, drivers) does.
pub fn master_sketch_id(features: &MasterFeatures) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::sync::OnceLock;
    static BASE: OnceLock<u64> = OnceLock::new();
    let base = *BASE.get_or_init(|| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        SKETCH.hash(&mut hasher);
        hasher.finish()
    });
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (base, features).hash(&mut hasher);
    hasher.finish()
}

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
        features: &MasterFeatures,
        low_priority: bool,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<MasterBuild, BuildError> {
        let root = plain_path(work_dir).join("master");
        let sketch_dir = root.join("master");
        let sketch = sketch_dir.join("master.ino");
        std::fs::create_dir_all(&sketch_dir).map_err(|e| BuildError::BadOutput(e.to_string()))?;
        // Untouched sources keep arduino-cli's cache valid.
        let text = sketch_text(features);
        if std::fs::read_to_string(&sketch).ok().as_deref() != Some(text.as_str()) {
            std::fs::write(&sketch, &text).map_err(|e| BuildError::BadOutput(e.to_string()))?;
        }
        if let Some(core) = self.ensure_core(&board.fqbn, on_line)? {
            on_line(&format!("Установлено ядро {core}"));
        }
        let installed = self.ensure_libraries(&features.libraries(), on_line)?;
        if !installed.is_empty() {
            on_line(&format!("Установлены библиотеки: {}", installed.join(", ")));
        }
        let out = build_dir(&root);

        let mut args: Vec<OsString> = vec!["compile".into()];
        args.extend(board_args(board));
        args.push("--build-path".into());
        args.push(out.clone().into());
        if features.modbus {
            for dir in crate::bundled_libraries(work_dir) {
                args.push("--libraries".into());
                args.push(dir.into());
            }
        }
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
        features: &MasterFeatures,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let build = self.compile_master(work_dir, board, features, false, on_line)?;
        self.flash_master(&build, board, port, on_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drivers_follow_the_blocks() {
        let none = MasterFeatures::of_blocks(["timer_on", "digital_in"]);
        assert_eq!(none, MasterFeatures::default());
        assert_eq!(none.defines(), "");
        assert_eq!(none.libraries(), ["Wasm3"]);
        let both = MasterFeatures::of_blocks(["dht22", "pcf8574_out"]);
        assert_eq!(both.defines(), "#define FLP_SENSORS 1\n#define FLP_PCF 1\n");
        assert!(both.libraries().contains(&"DHT sensor library"));
        assert_ne!(master_sketch_id(&none), master_sketch_id(&both));
        let modbus = MasterFeatures::of_blocks(["modbus_rtu_slave"]);
        assert_eq!(
            modbus.defines(),
            "#define FLP_MODBUS 1
"
        );
        assert_ne!(master_sketch_id(&none), master_sketch_id(&modbus));
        let uart = MasterFeatures::of_blocks(["uart_send"]);
        assert_eq!(uart.defines(), "#define FLP_UART 1\n");
        assert_ne!(master_sketch_id(&none), master_sketch_id(&uart));
        assert!(sketch_text(&both).ends_with(SKETCH));
    }
}
