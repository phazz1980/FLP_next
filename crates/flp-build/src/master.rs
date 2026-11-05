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
    /// Text operations (`text`): the `text_*` blocks, UART lines and text sending.
    pub text: bool,
    /// Network (`net`): the `wifi`, `ethernet` and `ntp` blocks; the library comes from `libs/`.
    pub net: bool,
    /// The Wi-Fi interface (`wifi`).
    pub wifi: bool,
    /// The W5500 Ethernet interface (`ethernet`).
    pub eth: bool,
    /// The MQTT client (`mqtt`).
    pub mqtt: bool,
    /// The web page (`web_ui` and widgets): the master serves the page and the module fills it.
    pub web: bool,
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
                // The address and the time are texts: they come through `text` too.
                "wifi" => {
                    f.net = true;
                    f.wifi = true;
                    f.text = true;
                }
                "ethernet" => {
                    f.net = true;
                    f.eth = true;
                    f.text = true;
                }
                "ntp" => {
                    f.net = true;
                    f.text = true;
                }
                "mqtt" | "mqtt_publish" | "mqtt_subscribe" => {
                    f.net = true;
                    f.mqtt = true;
                    f.text = true;
                }
                "web_ui" | "web_net" | "web_button" | "web_switch" | "web_label" | "web_clock"
                | "web_led" | "web_text" | "web_icon" | "web_image" | "web_shape" | "web_gauge"
                | "web_tank" | "web_value" | "web_slider" | "web_text_input" | "web_input"
                | "web_chart" => {
                    f.net = true;
                    f.web = true;
                    f.text = true;
                }
                "ip_to_text" | "text_to_ip" | "text_const" | "text_concat" | "text_length"
                | "num_to_text" | "text_compare" | "text_to_num" | "text_substring"
                | "text_find" | "text_char" | "text_add_char" => f.text = true,
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
            (self.text, "FLP_TEXT"),
            (self.net, "FLP_NET"),
            (self.wifi, "FLP_WIFI"),
            (self.eth, "FLP_ETH"),
            (self.mqtt, "FLP_MQTT"),
            (self.web, "FLP_WEB"),
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

/// The board as the master is built for. The master is flashed over USB and never updates itself
/// over the air, so it takes the "No OTA" partitions (2 MB app, 2 MB for modules) instead of the
/// default 1.2 MB app, in which the network stack alone does not fit. A scheme or flash size set
/// in the project wins.
fn master_board(board: &BoardConfig) -> BoardConfig {
    let mut board = board.clone();
    let small_flash = board
        .options
        .get("FlashSize")
        .is_some_and(|v| matches!(v.as_str(), "1M" | "2M"));
    if board.fqbn.starts_with("esp32:esp32:") && !small_flash {
        board
            .options
            .entry("PartitionScheme".into())
            .or_insert_with(|| MASTER_PARTITIONS.into());
    }
    board
}

/// Partition scheme of the master (see `master_board`).
const MASTER_PARTITIONS: &str = "no_ota";

/// The master sketch with the defines of its drivers in front.
fn sketch_text(features: &MasterFeatures) -> String {
    format!("{}{}{SKETCH}", features.defines(), page_text(features))
}

/// The web page as a flash constant: the same page a C++ sketch serves, so it is not copied into
/// the sketch file.
fn page_text(features: &MasterFeatures) -> String {
    if features.web {
        format!(
            "const char flp_html[] PROGMEM = R\"FLPUI({})FLPUI\";
",
            flp_blocks::device_page()
        )
    } else {
        String::new()
    }
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
    // The page is part of the firmware: a new page needs a new master.
    page_text(features).hash(&mut hasher);
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
        let board = &master_board(board);
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
        if features.net {
            // The master never updates over the air: leave ArduinoOTA and Update out of
            // the network library (they also pull in AES/bignum from mbedcrypto).
            args.push("--build-property".into());
            args.push("compiler.cpp.extra_flags=-DFLPROG_NETWORK_COMPILE_OTA=0".into());
        }
        if features.modbus || features.net {
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
        let board = &master_board(board);
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
        let text = MasterFeatures::of_blocks(["text_concat", "uart_read"]);
        assert_eq!(
            text.defines(),
            "#define FLP_UART 1
#define FLP_TEXT 1
"
        );
        let net = MasterFeatures::of_blocks(["ntp", "wifi"]);
        assert_eq!(
            net.defines(),
            "#define FLP_TEXT 1
#define FLP_NET 1
#define FLP_WIFI 1
"
        );
        let eth = MasterFeatures::of_blocks(["ethernet", "ntp"]);
        assert_eq!(
            eth.defines(),
            "#define FLP_TEXT 1
#define FLP_NET 1
#define FLP_ETH 1
"
        );
        let wifi_eth = MasterFeatures::of_blocks(["wifi", "ethernet"]);
        assert!(wifi_eth.defines().contains("FLP_WIFI") && wifi_eth.defines().contains("FLP_ETH"));
        let mqtt = MasterFeatures::of_blocks(["mqtt", "mqtt_publish", "wifi"]);
        assert!(mqtt.defines().contains("FLP_MQTT") && mqtt.defines().contains("FLP_NET"));
        assert_ne!(master_sketch_id(&net), master_sketch_id(&mqtt));
        let web = MasterFeatures::of_blocks(["web_ui", "web_slider", "wifi"]);
        assert!(web.defines().contains("FLP_WEB") && web.defines().contains("FLP_NET"));
        assert!(sketch_text(&web).contains("const char flp_html[] PROGMEM"));
        assert!(!sketch_text(&mqtt).contains("const char flp_html[] PROGMEM"));
        assert_ne!(master_sketch_id(&net), master_sketch_id(&web));
        assert_ne!(master_sketch_id(&none), master_sketch_id(&net));
        assert!(sketch_text(&both).ends_with(SKETCH));
    }

    #[test]
    fn master_takes_the_no_ota_partitions() {
        let mut board = BoardConfig {
            fqbn: "esp32:esp32:esp32c3".into(),
            model: None,
            port: None,
            options: Default::default(),
        };
        assert_eq!(master_board(&board).options["PartitionScheme"], "no_ota");
        board
            .options
            .insert("PartitionScheme".into(), "huge_app".into());
        assert_eq!(master_board(&board).options["PartitionScheme"], "huge_app");
        board.options.clear();
        board.options.insert("FlashSize".into(), "2M".into());
        assert!(!master_board(&board).options.contains_key("PartitionScheme"));
    }
}
