//! Building and uploading firmware with arduino-cli.
//!
//! Each project keeps a persistent build folder `<project>/.flp/build`, so
//! repeated builds only recompile the changed sketch.

use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use flp_codegen::{sketch_dir, WORK_DIR};
use flp_core::BoardConfig;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(
        "arduino-cli не найден. Установите его (`winget install ArduinoSA.CLI`) \
         или укажите путь в переменной FLP_ARDUINO_CLI"
    )]
    NotFound,

    #[error("не удалось запустить {}: {}", .exe.display(), .source)]
    Spawn {
        exe: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{what}: arduino-cli завершился с ошибкой ({status})")]
    Failed {
        what: &'static str,
        status: ExitStatus,
    },

    #[error("непонятный ответ arduino-cli: {0}")]
    BadOutput(String),

    #[error(
        "плата не найдена: нет ни одного последовательного порта. \
         Подключите плату по USB или укажите порт через --port"
    )]
    NoPort,

    #[error(
        "найдено несколько портов ({}); укажите нужный через --port \
         или в project.toml ([board] port = \"COM3\")",
        .0.join(", ")
    )]
    AmbiguousPort(Vec<String>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildMode {
    /// Working firmware with normal optimization.
    #[default]
    Release,
    /// Build for online debugging, without heavy optimization.
    Debug,
}

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub mode: BuildMode,
    /// Run the compiler with below-normal priority (background builds).
    pub low_priority: bool,
    /// Parallel compile jobs; `None` lets arduino-cli use all cores.
    pub jobs: Option<u32>,
    /// Extra folders with Arduino libraries (`--libraries`), in addition to
    /// the bundled ones found by [`bundled_libraries`].
    pub libraries: Vec<PathBuf>,
}

/// Serial port found by `arduino-cli board list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPort {
    pub address: String,
    /// Board names recognized by arduino-cli, or the port label.
    pub description: String,
}

/// Folder with Arduino libraries shipped with FLP_next (FLProg_Network, Modbus...).
const BUNDLED_LIBRARIES: &str = "Flprog_Network/Библиотека";

/// Bundled Arduino library folders: `FLP_ARDUINO_LIBS` (separated like `PATH`),
/// otherwise `Flprog_Network/Библиотека` next to the project (or its parents)
/// or next to the executable (or its parents).
pub fn bundled_libraries(project_root: &Path) -> Vec<PathBuf> {
    if let Some(dirs) = env::var_os("FLP_ARDUINO_LIBS") {
        return env::split_paths(&dirs).filter(|d| d.is_dir()).collect();
    }
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    // Not `canonicalize`: arduino-cli does not accept `\\?\` paths.
    let project = std::path::absolute(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    [Some(project), exe_dir]
        .into_iter()
        .flatten()
        .find_map(|start| {
            start
                .ancestors()
                .map(|d| d.join(BUNDLED_LIBRARIES))
                .find(|d| d.is_dir())
        })
        .into_iter()
        .collect()
}

/// Persistent build folder of a project.
pub fn build_dir(project_root: &Path) -> PathBuf {
    project_root.join(WORK_DIR).join("build")
}

/// Handle to the arduino-cli executable.
#[derive(Debug, Clone)]
pub struct ArduinoCli {
    exe: PathBuf,
}

impl ArduinoCli {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into() }
    }

    /// Finds arduino-cli: `FLP_ARDUINO_CLI`, then `PATH`, then the default
    /// install folder (a terminal opened before installation has an old `PATH`).
    pub fn locate() -> Result<Self, BuildError> {
        if let Some(exe) = env::var_os("FLP_ARDUINO_CLI") {
            return Ok(Self::new(exe));
        }
        let name = if cfg!(windows) {
            "arduino-cli.exe"
        } else {
            "arduino-cli"
        };
        let mut candidates: Vec<PathBuf> = env::var_os("PATH")
            .map(|paths| env::split_paths(&paths).map(|d| d.join(name)).collect())
            .unwrap_or_default();
        if let Some(pf) = env::var_os("ProgramFiles") {
            candidates.push(Path::new(&pf).join("Arduino CLI").join(name));
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .map(Self::new)
            .ok_or(BuildError::NotFound)
    }

    pub fn exe(&self) -> &Path {
        &self.exe
    }

    /// Compiles the generated sketch of the project. Output lines go to `on_line`.
    pub fn compile(
        &self,
        project_root: &Path,
        board: &BoardConfig,
        opts: &CompileOptions,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let mut opts = opts.clone();
        for dir in bundled_libraries(project_root) {
            if !opts.libraries.contains(&dir) {
                opts.libraries.push(dir);
            }
        }
        let args = compile_args(project_root, board, &opts);
        let status = self.run(&args, opts.low_priority, on_line)?;
        check_status("сборка", status)
    }

    /// Uploads the last build to the board on `port`.
    pub fn upload(
        &self,
        project_root: &Path,
        board: &BoardConfig,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let args = upload_args(project_root, board, port);
        let status = self.run(&args, false, on_line)?;
        check_status("заливка", status)
    }

    /// Installs the Arduino libraries from `required` that are not installed yet.
    /// Returns the libraries that were installed.
    pub fn ensure_libraries(
        &self,
        required: &[&str],
        on_line: &mut dyn FnMut(&str),
    ) -> Result<Vec<String>, BuildError> {
        if required.is_empty() {
            return Ok(Vec::new());
        }
        let output = Command::new(&self.exe)
            .args(["lib", "list", "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("список библиотек", output.status)?;
        let installed = parse_lib_list(&String::from_utf8_lossy(&output.stdout))?;
        let missing: Vec<String> = required
            .iter()
            .filter(|spec| !is_installed(spec, &installed))
            .map(|spec| spec.to_string())
            .collect();
        if missing.is_empty() {
            return Ok(missing);
        }
        let mut args: Vec<OsString> = vec!["lib".into(), "install".into()];
        args.extend(missing.iter().map(OsString::from));
        let status = self.run(&args, false, on_line)?;
        check_status("установка библиотек", status)?;
        Ok(missing)
    }

    /// Lists serial ports with boards that may be connected to them.
    pub fn serial_ports(&self) -> Result<Vec<SerialPort>, BuildError> {
        let output = Command::new(&self.exe)
            .args(["board", "list", "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("поиск плат", output.status)?;
        parse_board_list(&String::from_utf8_lossy(&output.stdout))
    }

    /// Board menu options (CPU frequency, flash size, partitions...) for `fqbn`.
    pub fn board_menus(&self, fqbn: &str) -> Result<Vec<BoardMenu>, BuildError> {
        let output = Command::new(&self.exe)
            .args(["board", "details", "--fqbn", fqbn, "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("описание платы", output.status)?;
        parse_board_details(&String::from_utf8_lossy(&output.stdout))
    }

    /// Port for upload: explicit, from `project.toml`, or the only serial port found.
    pub fn choose_port(
        &self,
        explicit: Option<&str>,
        board: &BoardConfig,
    ) -> Result<String, BuildError> {
        if let Some(port) = explicit.or(board.port.as_deref()) {
            return Ok(port.to_owned());
        }
        let mut ports = self.serial_ports()?;
        match ports.len() {
            0 => Err(BuildError::NoPort),
            1 => Ok(ports.remove(0).address),
            _ => Err(BuildError::AmbiguousPort(
                ports.into_iter().map(|p| p.address).collect(),
            )),
        }
    }

    fn spawn_error(&self, source: std::io::Error) -> BuildError {
        BuildError::Spawn {
            exe: self.exe.clone(),
            source,
        }
    }

    /// Runs arduino-cli, passing stdout and stderr lines to `on_line` as they come.
    fn run(
        &self,
        args: &[OsString],
        low_priority: bool,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<ExitStatus, BuildError> {
        let mut cmd = Command::new(&self.exe);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        set_low_priority(&mut cmd, low_priority);
        let mut child = cmd.spawn().map_err(|source| self.spawn_error(source))?;

        let (tx, rx) = mpsc::channel::<String>();
        let streams: [Option<Box<dyn Read + Send>>; 2] = [
            child.stdout.take().map(|s| Box::new(s) as _),
            child.stderr.take().map(|s| Box::new(s) as _),
        ];
        for stream in streams.into_iter().flatten() {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream);
                let mut buf = Vec::new();
                // Compiler output is not guaranteed to be valid UTF-8.
                while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
                    let line = String::from_utf8_lossy(&buf).trim_end().to_owned();
                    if tx.send(line).is_err() {
                        break;
                    }
                    buf.clear();
                }
            });
        }
        drop(tx);
        for line in rx {
            on_line(&line);
        }
        child.wait().map_err(|source| self.spawn_error(source))
    }
}

#[cfg(windows)]
fn set_low_priority(cmd: &mut Command, low: bool) {
    use std::os::windows::process::CommandExt;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    if low {
        cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn set_low_priority(_cmd: &mut Command, _low: bool) {}

fn check_status(what: &'static str, status: ExitStatus) -> Result<(), BuildError> {
    if status.success() {
        Ok(())
    } else {
        Err(BuildError::Failed { what, status })
    }
}

fn board_args(board: &BoardConfig) -> Vec<OsString> {
    let mut args = vec!["--fqbn".into(), board.fqbn.clone().into()];
    if !board.options.is_empty() {
        let opts: Vec<String> = board
            .options
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        args.push("--board-options".into());
        args.push(opts.join(",").into());
    }
    args
}

fn compile_args(project_root: &Path, board: &BoardConfig, opts: &CompileOptions) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["compile".into()];
    args.extend(board_args(board));
    args.push("--build-path".into());
    args.push(build_dir(project_root).into());
    if opts.mode == BuildMode::Debug {
        args.push("--optimize-for-debug".into());
    }
    for dir in &opts.libraries {
        args.push("--libraries".into());
        args.push(dir.into());
    }
    args.push("--jobs".into());
    args.push(opts.jobs.unwrap_or(0).to_string().into());
    args.push(sketch_dir(project_root).into());
    args
}

fn upload_args(project_root: &Path, board: &BoardConfig, port: &str) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["upload".into()];
    args.extend(board_args(board));
    args.push("--port".into());
    args.push(port.into());
    args.push("--input-dir".into());
    args.push(build_dir(project_root).into());
    args.push(sketch_dir(project_root).into());
    args
}

/// One board menu from `arduino-cli board details`, e.g. `FlashSize`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BoardMenu {
    /// Key for `--board-options`, e.g. `FlashSize`.
    #[serde(rename = "option")]
    pub key: String,
    #[serde(rename = "option_label", default)]
    pub label: String,
    pub values: Vec<BoardMenuValue>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BoardMenuValue {
    pub value: String,
    #[serde(rename = "value_label", default)]
    pub label: String,
    /// Default value of the menu.
    #[serde(default)]
    pub selected: bool,
}

impl BoardMenu {
    pub fn default_value(&self) -> Option<&str> {
        self.values
            .iter()
            .find(|v| v.selected)
            .or(self.values.first())
            .map(|v| v.value.as_str())
    }
}

fn parse_board_details(json: &str) -> Result<Vec<BoardMenu>, BuildError> {
    #[derive(Deserialize)]
    struct Details {
        #[serde(default)]
        config_options: Vec<BoardMenu>,
    }
    let d: Details =
        serde_json::from_str(json).map_err(|e| BuildError::BadOutput(e.to_string()))?;
    Ok(d.config_options)
}

/// Installed library: name and version.
type InstalledLib = (String, String);

fn parse_lib_list(json: &str) -> Result<Vec<InstalledLib>, BuildError> {
    #[derive(Deserialize)]
    struct LibList {
        #[serde(default)]
        installed_libraries: Vec<Installed>,
    }
    #[derive(Deserialize)]
    struct Installed {
        library: Lib,
    }
    #[derive(Deserialize)]
    struct Lib {
        name: String,
        #[serde(default)]
        version: String,
    }

    let list: LibList =
        serde_json::from_str(json).map_err(|e| BuildError::BadOutput(e.to_string()))?;
    Ok(list
        .installed_libraries
        .into_iter()
        .map(|i| (i.library.name, i.library.version))
        .collect())
}

/// `spec` is `Name` or `Name@version`.
fn is_installed(spec: &str, installed: &[InstalledLib]) -> bool {
    let (name, version) = match spec.split_once('@') {
        Some((n, v)) => (n, Some(v)),
        None => (spec, None),
    };
    installed
        .iter()
        .any(|(n, v)| n == name && version.is_none_or(|want| want == v))
}

fn parse_board_list(json: &str) -> Result<Vec<SerialPort>, BuildError> {
    #[derive(Deserialize)]
    struct BoardList {
        #[serde(default)]
        detected_ports: Vec<DetectedPort>,
    }
    #[derive(Deserialize)]
    struct DetectedPort {
        port: PortInfo,
        #[serde(default)]
        matching_boards: Vec<MatchingBoard>,
    }
    #[derive(Deserialize)]
    struct PortInfo {
        address: String,
        protocol: String,
        #[serde(default)]
        label: String,
    }
    #[derive(Deserialize)]
    struct MatchingBoard {
        name: String,
    }

    let list: BoardList =
        serde_json::from_str(json).map_err(|e| BuildError::BadOutput(e.to_string()))?;
    Ok(list
        .detected_ports
        .into_iter()
        .filter(|d| d.port.protocol == "serial")
        .map(|d| {
            let names: Vec<String> = d.matching_boards.into_iter().map(|b| b.name).collect();
            SerialPort {
                description: if names.is_empty() {
                    d.port.label
                } else {
                    names.join(", ")
                },
                address: d.port.address,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BoardConfig {
        BoardConfig {
            fqbn: "esp32:esp32:esp32".into(),
            port: None,
            options: [("CPUFreq".to_owned(), "240".to_owned())].into(),
        }
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn compile_passes_library_folders() {
        let opts = CompileOptions {
            libraries: vec![PathBuf::from("libs")],
            ..Default::default()
        };
        let args = strings(&compile_args(Path::new("proj"), &board(), &opts));
        let i = args.iter().position(|a| a == "--libraries").unwrap();
        assert_eq!(args[i + 1], "libs");
    }

    #[test]
    fn compile_command_line() {
        let root = Path::new("proj");
        let opts = CompileOptions {
            mode: BuildMode::Debug,
            ..Default::default()
        };
        let args = strings(&compile_args(root, &board(), &opts));
        let build = build_dir(root).to_string_lossy().into_owned();
        let sketch = sketch_dir(root).to_string_lossy().into_owned();
        assert_eq!(
            args,
            [
                "compile",
                "--fqbn",
                "esp32:esp32:esp32",
                "--board-options",
                "CPUFreq=240",
                "--build-path",
                &build,
                "--optimize-for-debug",
                "--jobs",
                "0",
                &sketch,
            ]
        );
    }

    #[test]
    fn board_details_menus() {
        let json = r#"{"fqbn": "esp32:esp32:esp32", "config_options": [
            {"option": "FlashSize", "option_label": "Flash Size", "values": [
                {"value": "4M", "value_label": "4MB (32Mb)", "selected": true},
                {"value": "8M", "value_label": "8MB (64Mb)"}]}]}"#;
        let menus = parse_board_details(json).unwrap();
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].key, "FlashSize");
        assert_eq!(menus[0].values[1].label, "8MB (64Mb)");
        assert_eq!(menus[0].default_value(), Some("4M"));
    }

    #[test]
    fn upload_command_line() {
        let mut board = board();
        board.options.clear();
        let args = strings(&upload_args(Path::new("proj"), &board, "COM5"));
        assert_eq!(
            args[..6],
            [
                "upload",
                "--fqbn",
                "esp32:esp32:esp32",
                "--port",
                "COM5",
                "--input-dir"
            ]
        );
    }

    #[test]
    fn board_list_json() {
        let json = r#"{
          "detected_ports": [
            {"port": {"address": "COM1", "label": "COM1", "protocol": "serial",
                      "protocol_label": "Serial Port"}},
            {"port": {"address": "COM5", "label": "COM5", "protocol": "serial",
                      "protocol_label": "Serial Port (USB)",
                      "properties": {"pid": "0xEA60", "vid": "0x10C4"}},
             "matching_boards": [{"name": "ESP32 Dev Module", "fqbn": "esp32:esp32:esp32"}]},
            {"port": {"address": "192.168.1.7", "label": "esp", "protocol": "network"}}
          ]
        }"#;
        let ports = parse_board_list(json).unwrap();
        assert_eq!(
            ports,
            [
                SerialPort {
                    address: "COM1".into(),
                    description: "COM1".into()
                },
                SerialPort {
                    address: "COM5".into(),
                    description: "ESP32 Dev Module".into()
                },
            ]
        );
        assert_eq!(parse_board_list("{}").unwrap(), []);
        assert!(parse_board_list("not json").is_err());
    }

    #[test]
    fn lib_list_json() {
        let json = r#"{"installed_libraries": [
            {"library": {"name": "OneWire", "version": "2.3.8", "location": "user"},
             "release": {"version": "2.3.8"}},
            {"library": {"name": "DallasTemperature", "version": "4.0.4"}}
        ]}"#;
        let installed = parse_lib_list(json).unwrap();
        assert!(is_installed("OneWire", &installed));
        assert!(is_installed("OneWire@2.3.8", &installed));
        assert!(!is_installed("OneWire@2.3.7", &installed));
        assert!(!is_installed("DHT sensor library", &installed));
        assert_eq!(parse_lib_list("{}").unwrap(), []);
    }

    #[test]
    fn explicit_port_wins() {
        // A missing executable proves no board scan is attempted.
        let cli = ArduinoCli::new("no-such-arduino-cli");
        let mut b = board();
        assert_eq!(cli.choose_port(Some("COM7"), &b).unwrap(), "COM7");
        b.port = Some("COM3".into());
        assert_eq!(cli.choose_port(None, &b).unwrap(), "COM3");
        b.port = None;
        assert!(matches!(
            cli.choose_port(None, &b),
            Err(BuildError::Spawn { .. })
        ));
    }
}
