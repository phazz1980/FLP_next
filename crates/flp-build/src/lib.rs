//! Building and uploading firmware with arduino-cli.
//!
//! Each project keeps a persistent build folder `<dir>/.flp/<project>/build`, so
//! repeated builds only recompile the changed sketch.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use flp_codegen::sketch_dir;
use flp_core::BoardConfig;
use serde::Deserialize;

mod direct;
mod output;
pub use output::{
    classify, explain_compile, explain_upload, locate_block, parse_diagnostic, OutputLine,
};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("отменено")]
    Cancelled,

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
         или в файле проекта ([board] port = \"COM3\")",
        .0.join(", ")
    )]
    AmbiguousPort(Vec<String>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildMode {
    /// Working firmware with normal optimization.
    #[default]
    Release,
    /// Firmware with the online debugging module (same compiler flags as release).
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
    /// Verbose output (`-v`) with compiler warnings; see [`classify`].
    pub verbose: bool,
    /// Run arduino-cli even if the last build is up to date.
    pub force: bool,
}

/// Outcome of a successful [`ArduinoCli::compile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compiled {
    /// arduino-cli compiled the sketch.
    Built,
    /// Only the sketch body changed: it was compiled and linked with the commands of
    /// the last full build, without arduino-cli.
    Direct,
    /// The build folder already held firmware for this sketch and settings.
    UpToDate,
}

/// Serial port found by `arduino-cli board list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPort {
    pub address: String,
    /// Board names recognized by arduino-cli, or the port label.
    pub description: String,
    /// arduino-cli recognized a board on this port.
    pub has_board: bool,
    /// USB serial adapter (has VID/PID), unlike built-in COM ports.
    pub usb: bool,
    /// Boards recognized on the port (`vendor:arch:board`).
    pub fqbns: Vec<String>,
}

/// Picks the port for automatic upload: the only port with a recognized board,
/// otherwise the only USB port, otherwise the only port at all.
pub fn auto_port(ports: &[SerialPort]) -> Result<String, BuildError> {
    if ports.is_empty() {
        return Err(BuildError::NoPort);
    }
    let tiers: [&dyn Fn(&SerialPort) -> bool; 3] = [&|p| p.has_board, &|p| p.usb, &|_| true];
    for tier in tiers {
        let found: Vec<&SerialPort> = ports.iter().filter(|p| tier(p)).collect();
        match found.len() {
            0 => continue,
            1 => return Ok(found[0].address.clone()),
            _ => {
                return Err(BuildError::AmbiguousPort(
                    found.iter().map(|p| p.address.clone()).collect(),
                ))
            }
        }
    }
    unreachable!("the last tier matches every port")
}

/// Orders port names naturally: `COM3` < `COM10`.
fn port_order(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(s: &str) -> (&str, Option<u64>) {
        let digits = s.len() - s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (head, tail) = s.split_at(s.len() - digits);
        (head, tail.parse().ok())
    }
    split(a).cmp(&split(b)).then_with(|| a.cmp(b))
}

/// Folder with third-party Arduino libraries shipped with CrabLogic (FLProg_Network,
/// Modbus, ...); every subfolder is one library.
const BUNDLED_LIBRARIES: &str = "libs";

/// Where the libraries lived before `libs/`; still searched so old installs keep building.
const LEGACY_LIBRARIES: &str = "Flprog_Network/Библиотека";

/// Bundled Arduino library folders: `FLP_ARDUINO_LIBS` (separated like `PATH`),
/// otherwise `libs/` next to the project (or its parents) or next to the executable
/// (or its parents).
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
                .flat_map(|d| [d.join(BUNDLED_LIBRARIES), d.join(LEGACY_LIBRARIES)])
                .find(|d| d.is_dir())
        })
        .into_iter()
        .collect()
}

/// Strips the Windows verbatim prefix (`\\?\`), which arduino-cli does not accept.
fn plain_path(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// Persistent build folder inside the project work folder ([`flp_core::Project::work_dir`]).
pub fn build_dir(work_dir: &Path) -> PathBuf {
    work_dir.join("build")
}

/// File in the build folder with the fingerprint of the last successful build.
pub const BUILD_STAMP: &str = "flp-build.stamp";

/// Fingerprint of everything that decides the firmware: arduino-cli, board and its
/// options, library folders and the sketch files. Output settings (`-v`, `--jobs`)
/// are left out. Changes inside library folders or an updated core are not seen;
/// [`CompileOptions::force`] covers that.
fn build_stamp(exe: &Path, work_dir: &Path, board: &BoardConfig, opts: &CompileOptions) -> String {
    let mut h = Fnv64::new();
    h.write(exe.as_os_str().as_encoded_bytes());
    h.write(board.fqbn.as_bytes());
    for (k, v) in board_options(board) {
        h.write(k.as_bytes());
        h.write(v.as_bytes());
    }
    for dir in &opts.libraries {
        h.write(dir.as_os_str().as_encoded_bytes());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(sketch_dir(work_dir))
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.sort();
    for file in files {
        h.write(file.file_name().unwrap_or_default().as_encoded_bytes());
        h.write(&std::fs::read(&file).unwrap_or_default());
    }
    // Flags chosen from the sketch (see `compile_args`) are part of the firmware too.
    if uses_web_ui(work_dir) {
        h.write(WEBUI_RESPONSE_DEFINE.as_bytes());
    }
    format!("{:016x}", h.0)
}

/// The build folder holds a firmware image (`.bin` for ESP32, `.hex` for AVR).
/// Plain work dir and options with the bundled libraries of the project added.
fn with_bundled(work_dir: &Path, opts: &CompileOptions) -> (PathBuf, CompileOptions) {
    let work_dir = plain_path(work_dir);
    let mut opts = opts.clone();
    for dir in bundled_libraries(&work_dir) {
        if !opts.libraries.contains(&dir) {
            opts.libraries.push(dir);
        }
    }
    (work_dir, opts)
}

fn has_firmware(work_dir: &Path) -> bool {
    let dir = build_dir(work_dir);
    ["sketch.ino.bin", "sketch.ino.hex"]
        .iter()
        .any(|f| dir.join(f).is_file())
}

/// FNV-1a: stable across runs and Rust versions, unlike `DefaultHasher`.
struct Fnv64(u64);

impl Fnv64 {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// Hashes `bytes` with a length prefix, so neighbouring parts cannot merge.
    fn write(&mut self, bytes: &[u8]) {
        for b in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Shared control of a running arduino-cli: cancellation and priority, set from
/// another thread (e.g. the editor UI).
#[derive(Debug, Default)]
pub struct BuildControl {
    cancel: AtomicBool,
    raise: AtomicBool,
}

impl BuildControl {
    /// Kills the running command; it ends with [`BuildError::Cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Raises a low-priority build to normal priority.
    pub fn raise_priority(&self) {
        self.raise.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn raised(&self) -> bool {
        self.raise.load(Ordering::Relaxed)
    }
}

/// Package index with the ESP32 core (not known to arduino-cli by default).
pub const ESP32_INDEX_URL: &str =
    "https://espressif.github.io/arduino-esp32/package_esp32_index.json";

/// `vendor:arch` of a board `fqbn` (`esp32:esp32:esp32c3` → `esp32:esp32`).
fn core_id(fqbn: &str) -> Option<String> {
    let mut parts = fqbn.split(':');
    let (vendor, arch) = (parts.next()?, parts.next()?);
    Some(format!("{vendor}:{arch}"))
}

/// Ids of the installed cores from `arduino-cli core list --format json`
/// (a plain array in older versions, `{"platforms": [...]}` in newer ones).
fn parse_core_list(json: &str) -> Result<Vec<String>, BuildError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| BuildError::BadOutput(e.to_string()))?;
    let list = value.get("platforms").unwrap_or(&value);
    Ok(list
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("id")?.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// Installs arduino-cli with winget (Windows). Output lines go to `on_line`.
pub fn install_cli(on_line: &mut dyn FnMut(&str)) -> Result<(), BuildError> {
    let exe = PathBuf::from("winget");
    let output = Command::new(&exe)
        .args([
            "install",
            "--id",
            "ArduinoSA.CLI",
            "-e",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| BuildError::Spawn { exe, source })?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        // Skip the spinner and progress bar frames.
        if line.trim().len() > 2 && !line.contains('█') {
            on_line(line.trim());
        }
    }
    check_status("установка arduino-cli", output.status)
}

/// Handle to the arduino-cli executable.
#[derive(Debug, Clone)]
pub struct ArduinoCli {
    exe: PathBuf,
    control: Option<Arc<BuildControl>>,
}

impl ArduinoCli {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self {
            exe: exe.into(),
            control: None,
        }
    }

    /// Commands run under `control`: they can be cancelled and reprioritized.
    pub fn with_control(mut self, control: Arc<BuildControl>) -> Self {
        self.control = Some(control);
        self
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
    ///
    /// Skips arduino-cli when the build folder already holds firmware for the same
    /// sketch and settings (see [`BUILD_STAMP`]), so it survives editor restarts.
    pub fn compile(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        opts: &CompileOptions,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<Compiled, BuildError> {
        let (work_dir, opts) = &with_bundled(work_dir, opts);
        let stamp_path = build_dir(work_dir).join(BUILD_STAMP);
        let stamp = build_stamp(&self.exe, work_dir, board, opts);
        if !opts.force && self.is_up_to_date(work_dir, board, opts) {
            return Ok(Compiled::UpToDate);
        }
        // A failed or cancelled build leaves no stamp, so the next one runs for real.
        let _ = std::fs::remove_file(&stamp_path);
        let verbose = opts.verbose;
        // The full build always runs verbose (to capture the recipe); without `verbose`
        // only diagnostics and the size report get through.
        let mut show = |line: &str| {
            if verbose || !matches!(classify(line), OutputLine::Detail | OutputLine::Stage(_)) {
                on_line(line);
            }
        };
        if !opts.force && self.compile_direct(work_dir, board, opts, &mut show)? {
            let _ = std::fs::write(&stamp_path, &stamp);
            return Ok(Compiled::Direct);
        }
        direct::Recipe::remove(work_dir);
        // A quiet background build must not start a download of hundreds of megabytes.
        if !opts.low_priority {
            if let Some(core) = self.ensure_core(&board.fqbn, &mut show)? {
                show(&format!("Установлено ядро {core}"));
            }
        }
        let args = compile_args(work_dir, board, opts);
        let mut capture = direct::Capture::default();
        let status = self.run(&args, opts.low_priority, &mut |line| {
            capture.feed(line);
            show(line);
        })?;
        check_status("сборка", status)?;
        // Only a missed shortcut next time if this fails.
        let _ = std::fs::write(&stamp_path, &stamp);
        if let Some(compile) = direct::sketch_compile_command(work_dir) {
            if let Some(key) = direct::recipe_key(&self.exe, work_dir, board, opts, &compile) {
                if !capture.post.is_empty() {
                    let post = capture.post;
                    direct::Recipe { key, compile, post }.save(work_dir);
                }
            }
        }
        Ok(Compiled::Built)
    }

    /// The next [`compile`](Self::compile) of the sketch written to `work_dir` would run
    /// arduino-cli: libraries, includes, board or options changed since the last full
    /// build, or there was none. Otherwise the build is up to date or a direct rebuild
    /// of a few seconds, so a background build can leave it to the user's build.
    pub fn needs_cli(&self, work_dir: &Path, board: &BoardConfig, opts: &CompileOptions) -> bool {
        let (work_dir, opts) = &with_bundled(work_dir, opts);
        opts.force
            || !self.is_up_to_date(work_dir, board, opts)
                && self.fitting_recipe(work_dir, board, opts).is_none()
    }

    fn is_up_to_date(&self, work_dir: &Path, board: &BoardConfig, opts: &CompileOptions) -> bool {
        let stamp = build_stamp(&self.exe, work_dir, board, opts);
        has_firmware(work_dir)
            && std::fs::read_to_string(build_dir(work_dir).join(BUILD_STAMP))
                .is_ok_and(|s| s.trim() == stamp)
    }

    /// The recipe of the last full build, if only the sketch body changed since.
    fn fitting_recipe(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        opts: &CompileOptions,
    ) -> Option<direct::Recipe> {
        let recipe = direct::Recipe::load(work_dir)?;
        let key = direct::recipe_key(&self.exe, work_dir, board, opts, &recipe.compile);
        (key.as_deref() == Some(recipe.key.as_str())
            && build_dir(work_dir).join("sketch.ino.elf").is_file())
        .then_some(recipe)
    }

    /// Rebuilds with the recipe of the last full build when it still fits. `false` means
    /// "build with arduino-cli": no recipe, something besides the sketch body changed,
    /// or a command failed (its output is dropped; the full build shows the errors).
    fn compile_direct(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        opts: &CompileOptions,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<bool, BuildError> {
        let Some(recipe) = self.fitting_recipe(work_dir, board, opts) else {
            return Ok(false);
        };
        let Ok(commands) =
            direct::write_sketch_cpp(work_dir).and_then(|()| recipe.commands(work_dir))
        else {
            return Ok(false);
        };
        let mut lines = Vec::new();
        for mut cmd in commands {
            cmd.stdin(Stdio::null());
            match self.run_command(cmd, opts.low_priority, &mut |l| lines.push(l.to_owned())) {
                Ok(status) if status.success() => {}
                Err(BuildError::Cancelled) => return Err(BuildError::Cancelled),
                _ => return Ok(false),
            }
        }
        for line in &lines {
            on_line(line);
        }
        Ok(true)
    }

    /// Uploads the last build to the board on `port`.
    pub fn upload(
        &self,
        work_dir: &Path,
        board: &BoardConfig,
        port: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<(), BuildError> {
        let args = upload_args(&plain_path(work_dir), board, port);
        let mut chip_mismatch = false;
        let status = self.run(&args, false, &mut |line| {
            chip_mismatch |= line.contains("This chip is");
            on_line(line);
        })?;
        if chip_mismatch && !status.success() {
            on_line(
                "Подсказка: в плату впаян другой чип, чем выбран в проекте —                  выберите подходящую плату в настройках платы.",
            );
        }
        check_status("заливка", status)
    }

    /// Installs the board core (`esp32:esp32` for an `esp32:esp32:...` board) when it is
    /// missing. Returns the core id if it was installed.
    pub fn ensure_core(
        &self,
        fqbn: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<Option<String>, BuildError> {
        let Some(id) = core_id(fqbn) else {
            return Ok(None);
        };
        let output = self
            .command()
            .args(["core", "list", "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("список ядер", output.status)?;
        if parse_core_list(&String::from_utf8_lossy(&output.stdout))?.contains(&id) {
            return Ok(None);
        }
        on_line(&format!(
            "Ядро {id} не установлено, устанавливаю (может занять несколько минут)..."
        ));
        // The ESP32 core is not in the default package index.
        let extra: Vec<OsString> = if id.starts_with("esp32:") {
            vec!["--additional-urls".into(), ESP32_INDEX_URL.into()]
        } else {
            Vec::new()
        };
        let mut update: Vec<OsString> = vec!["core".into(), "update-index".into()];
        update.extend(extra.iter().cloned());
        let status = self.run(&update, false, on_line)?;
        check_status("обновление индекса ядер", status)?;
        let mut install: Vec<OsString> = vec!["core".into(), "install".into(), id.clone().into()];
        install.extend(extra);
        let status = self.run(&install, false, on_line)?;
        check_status("установка ядра", status)?;
        Ok(Some(id))
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
        let output = self
            .command()
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
        let output = self
            .command()
            .args(["board", "list", "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("поиск плат", output.status)?;
        parse_board_list(&String::from_utf8_lossy(&output.stdout))
    }

    /// Board menu options (CPU frequency, flash size, partitions...) for `fqbn`.
    pub fn board_menus(&self, fqbn: &str) -> Result<Vec<BoardMenu>, BuildError> {
        let output = self
            .command()
            .args(["board", "details", "--fqbn", fqbn, "--format", "json"])
            .stdin(Stdio::null())
            .output()
            .map_err(|source| self.spawn_error(source))?;
        check_status("описание платы", output.status)?;
        parse_board_details(&String::from_utf8_lossy(&output.stdout))
    }

    /// Port for upload: explicit, from the project settings, or the only serial port found.
    pub fn choose_port(
        &self,
        explicit: Option<&str>,
        board: &BoardConfig,
    ) -> Result<String, BuildError> {
        if let Some(port) = explicit.or(board.port.as_deref()) {
            return Ok(port.to_owned());
        }
        auto_port(&self.serial_ports()?)
    }

    fn command(&self) -> Command {
        hidden_command(&self.exe)
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
        let mut cmd = self.command();
        cmd.args(args).stdin(Stdio::null());
        if args.iter().any(|a| a == "-v") {
            // `classify` and the recipe capture match the English stage messages.
            cmd.env("ARDUINO_LOCALE", "en");
        }
        self.run_command(cmd, low_priority, on_line)
    }

    /// Runs `cmd` under the build control (cancel, priority), passing stdout and stderr
    /// lines to `on_line` as they come.
    fn run_command(
        &self,
        mut cmd: Command,
        low_priority: bool,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<ExitStatus, BuildError> {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let control = self.control.clone().unwrap_or_default();
        if control.is_cancelled() {
            return Err(BuildError::Cancelled);
        }
        let mut low = low_priority && !control.raised();
        set_low_priority(&mut cmd, low);
        let exe = PathBuf::from(cmd.get_program());
        let spawn_error = |source| BuildError::Spawn {
            exe: exe.clone(),
            source,
        };
        let mut child = cmd.spawn().map_err(spawn_error)?;

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
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => on_line(&line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if control.is_cancelled() {
                kill_tree(&mut child);
                let _ = child.wait();
                return Err(BuildError::Cancelled);
            }
            if low && control.raised() {
                raise_priority(&child);
                low = false;
            }
        }
        let status = child.wait().map_err(spawn_error)?;
        if control.is_cancelled() {
            return Err(BuildError::Cancelled);
        }
        Ok(status)
    }
}

#[cfg(windows)]
fn set_low_priority(cmd: &mut Command, low: bool) {
    use std::os::windows::process::CommandExt;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // The flags are set as a whole: keep the console window hidden either way.
    let priority = if low { BELOW_NORMAL_PRIORITY_CLASS } else { 0 };
    cmd.creation_flags(CREATE_NO_WINDOW | priority);
}

#[cfg(not(windows))]
fn set_low_priority(_cmd: &mut Command, _low: bool) {}

/// A command that does not flash a console window when started from the editor.
fn hidden_command(exe: &std::path::Path) -> Command {
    let mut cmd = Command::new(exe);
    set_low_priority(&mut cmd, false);
    cmd
}

/// Sets normal priority for arduino-cli; compilers it starts afterwards inherit it.
#[cfg(windows)]
fn raise_priority(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    const NORMAL_PRIORITY_CLASS: u32 = 0x0000_0020;
    extern "system" {
        fn SetPriorityClass(process: *mut std::ffi::c_void, class: u32) -> i32;
    }
    // SAFETY: the handle belongs to the live child process.
    unsafe {
        SetPriorityClass(child.as_raw_handle(), NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn raise_priority(_child: &std::process::Child) {}

/// Kills arduino-cli together with the compilers it started.
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let killed = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if killed {
            return;
        }
    }
    let _ = child.kill();
}

fn check_status(what: &'static str, status: ExitStatus) -> Result<(), BuildError> {
    if status.success() {
        Ok(())
    } else {
        Err(BuildError::Failed { what, status })
    }
}

/// Chips whose boards are usually wired through the chip's own USB, without a UART bridge.
const NATIVE_USB_CHIPS: [&str; 3] = [
    "esp32:esp32:esp32c3",
    "esp32:esp32:esp32c6",
    "esp32:esp32:esp32h2",
];

/// Board options with defaults filled in: on native-USB chips `Serial` must go to USB
/// (`CDCOnBoot=cdc`), otherwise the board is silent on its only port. An option set in
/// the project wins.
fn board_options(board: &BoardConfig) -> BTreeMap<String, String> {
    let mut options = board.options.clone();
    if NATIVE_USB_CHIPS.contains(&board.fqbn.as_str()) {
        options
            .entry("CDCOnBoot".into())
            .or_insert_with(|| "cdc".into());
    }
    options
}

fn board_args(board: &BoardConfig) -> Vec<OsString> {
    let mut args = vec!["--fqbn".into(), board.fqbn.clone().into()];
    let options = board_options(board);
    if !options.is_empty() {
        let opts: Vec<String> = options.iter().map(|(k, v)| format!("{k}={v}")).collect();
        args.push("--board-options".into());
        args.push(opts.join(",").into());
    }
    args
}

/// Response buffer size of the library HTTP server for the web interface page.
const WEBUI_RESPONSE_DEFINE: &str = "FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES=6144U";

/// The sketch contains the `web_ui` block (it defines `FLP_WEBUI`).
fn uses_web_ui(work_dir: &Path) -> bool {
    std::fs::read_to_string(sketch_dir(work_dir).join("sketch.ino"))
        .is_ok_and(|s| s.contains("#define FLP_WEBUI"))
}

fn compile_args(work_dir: &Path, board: &BoardConfig, opts: &CompileOptions) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["compile".into()];
    args.extend(board_args(board));
    args.push("--build-path".into());
    args.push(build_dir(work_dir).into());
    // Debug firmware differs only by the debugging module in the sketch. Compiler flags
    // stay the same, so the cached core is shared and switching modes rebuilds just
    // the sketch (`--optimize-for-debug` would recompile the whole ESP32 core).
    for dir in &opts.libraries {
        args.push("--libraries".into());
        args.push(dir.into());
    }
    // The web interface page does not fit into the library's default response buffer
    // (2 KiB, answer 500), so a sketch with the `web_ui` block raises the limit.
    if uses_web_ui(work_dir) {
        args.push("--build-property".into());
        args.push(format!("compiler.cpp.extra_flags=-D{WEBUI_RESPONSE_DEFINE}").into());
    }
    // Always verbose: the log carries the link commands for the direct rebuild
    // (`compile` hides the noise unless `verbose` is set).
    args.push("-v".into());
    if opts.verbose {
        args.push("--warnings".into());
        args.push("default".into());
    }
    // Older arduino-cli versions have no `--jobs`; the default (all cores) is the same.
    if let Some(jobs) = opts.jobs {
        args.push("--jobs".into());
        args.push(jobs.to_string().into());
    }
    args.push(sketch_dir(work_dir).into());
    args
}

fn upload_args(work_dir: &Path, board: &BoardConfig, port: &str) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["upload".into()];
    args.extend(board_args(board));
    args.push("--port".into());
    args.push(port.into());
    args.push("--input-dir".into());
    args.push(build_dir(work_dir).into());
    args.push(sketch_dir(work_dir).into());
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
        #[serde(default)]
        properties: std::collections::HashMap<String, String>,
    }
    #[derive(Deserialize)]
    struct MatchingBoard {
        name: String,
        #[serde(default)]
        fqbn: String,
    }

    // Newer arduino-cli wraps the list in `detected_ports`, older ones print the array itself.
    let list: BoardList = match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::Array(items)) => {
            serde_json::from_value(serde_json::json!({ "detected_ports": items }))
        }
        Ok(value) => serde_json::from_value(value),
        Err(e) => Err(e),
    }
    .map_err(|e| {
        let head: String = json.chars().take(200).collect();
        BuildError::BadOutput(format!("{e}; вывод: {head}"))
    })?;
    let mut ports: Vec<SerialPort> = list
        .detected_ports
        .into_iter()
        .filter(|d| d.port.protocol == "serial")
        .map(|d| {
            let fqbns: Vec<String> = d
                .matching_boards
                .iter()
                .map(|b| b.fqbn.clone())
                .filter(|f| !f.is_empty())
                .collect();
            let names: Vec<String> = d.matching_boards.into_iter().map(|b| b.name).collect();
            SerialPort {
                fqbns,
                has_board: !names.is_empty(),
                usb: d.port.properties.contains_key("vid"),
                description: if names.is_empty() {
                    d.port.label
                } else {
                    names.join(", ")
                },
                address: d.port.address,
            }
        })
        .collect();
    ports.sort_by(|a, b| port_order(&a.address, &b.address));
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BoardConfig {
        BoardConfig {
            fqbn: "esp32:esp32:esp32".into(),
            model: None,
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
    fn verbatim_prefix_is_stripped() {
        assert_eq!(plain_path(Path::new(r"\\?\C:\p")), PathBuf::from(r"C:\p"));
        assert_eq!(
            plain_path(Path::new(r"\\?\UNC\srv\share")),
            PathBuf::from(r"\\srv\share")
        );
        assert_eq!(plain_path(Path::new("rel/p")), PathBuf::from("rel/p"));
    }

    #[test]
    fn build_stamp_follows_sketch_and_board_only() {
        let root = env::temp_dir().join(format!("flp-stamp-{}", std::process::id()));
        let sketch = sketch_dir(&root);
        std::fs::create_dir_all(&sketch).unwrap();
        std::fs::write(sketch.join("sketch.ino"), "void loop() {}").unwrap();
        let exe = Path::new("arduino-cli");
        let plain = CompileOptions::default();
        let stamp = build_stamp(exe, &root, &board(), &plain);

        let output_only = CompileOptions {
            verbose: true,
            jobs: Some(2),
            low_priority: true,
            ..Default::default()
        };
        assert_eq!(build_stamp(exe, &root, &board(), &output_only), stamp);

        let other_board = BoardConfig {
            fqbn: "esp32:esp32:esp32c3".into(),
            ..board()
        };
        assert_ne!(build_stamp(exe, &root, &other_board, &plain), stamp);

        std::fs::write(sketch.join("sketch.ino"), "void loop() { }").unwrap();
        assert_ne!(build_stamp(exe, &root, &board(), &plain), stamp);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn background_build_needs_cli_only_for_new_includes() {
        let root = env::temp_dir().join(format!("flp-needs-cli-{}", std::process::id()));
        let sketch = sketch_dir(&root);
        let build = build_dir(&root);
        std::fs::create_dir_all(&sketch).unwrap();
        std::fs::create_dir_all(&build).unwrap();
        let ino = sketch.join("sketch.ino");
        std::fs::write(&ino, "#include <A.h>\nvoid loop() {}").unwrap();
        let cli = ArduinoCli::new("arduino-cli");
        let opts = CompileOptions::default();
        assert!(cli.needs_cli(&root, &board(), &opts), "no full build yet");

        let compile = vec!["g++".to_owned()];
        let (_, bundled) = with_bundled(&root, &opts);
        let key = direct::recipe_key(&cli.exe, &root, &board(), &bundled, &compile).unwrap();
        let post = Vec::new();
        direct::Recipe { key, compile, post }.save(&root);
        std::fs::write(build.join("sketch.ino.elf"), "").unwrap();
        std::fs::write(&ino, "#include <A.h>\nvoid loop() { int x = 2; }").unwrap();
        assert!(!cli.needs_cli(&root, &board(), &opts), "body edit");
        let forced = CompileOptions {
            force: true,
            ..Default::default()
        };
        assert!(cli.needs_cli(&root, &board(), &forced), "rebuild");

        std::fs::write(&ino, "#include <A.h>\n#include <B.h>\nvoid loop() {}").unwrap();
        assert!(cli.needs_cli(&root, &board(), &opts), "new include");
        let _ = std::fs::remove_dir_all(&root);
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
    fn native_usb_chips_print_to_usb() {
        let mut c3 = BoardConfig {
            fqbn: "esp32:esp32:esp32c3".into(),
            ..board()
        };
        c3.options.clear();
        assert_eq!(board_options(&c3)["CDCOnBoot"], "cdc");
        // The project's own choice wins.
        c3.options.insert("CDCOnBoot".into(), "default".into());
        assert_eq!(board_options(&c3)["CDCOnBoot"], "default");
        // Boards with a UART bridge keep their defaults.
        assert!(!board_options(&board()).contains_key("CDCOnBoot"));
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
                "-v",
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
            {"port": {"address": "COM10", "label": "COM10", "protocol": "serial",
                      "properties": {"pid": "0x7523", "vid": "0x1A86"}}},
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
                    description: "COM1".into(),
                    has_board: false,
                    usb: false,
                    fqbns: vec![],
                },
                SerialPort {
                    address: "COM5".into(),
                    description: "ESP32 Dev Module".into(),
                    has_board: true,
                    usb: true,
                    fqbns: vec!["esp32:esp32:esp32".into()],
                },
                SerialPort {
                    address: "COM10".into(),
                    description: "COM10".into(),
                    has_board: false,
                    usb: true,
                    fqbns: vec![],
                },
            ]
        );
        assert_eq!(auto_port(&ports).unwrap(), "COM5");
        // Without a recognized board the only USB adapter wins over COM1.
        assert_eq!(
            auto_port(&[ports[0].clone(), ports[2].clone()]).unwrap(),
            "COM10"
        );
        assert!(matches!(
            auto_port(&[ports[1].clone(), ports[1].clone()]),
            Err(BuildError::AmbiguousPort(_))
        ));
        assert!(matches!(auto_port(&[]), Err(BuildError::NoPort)));
        assert_eq!(parse_board_list("{}").unwrap(), []);
        assert!(parse_board_list("not json").is_err());
        let old = r#"[{"port": {"address": "COM7", "protocol": "serial", "label": "COM7"}}]"#;
        assert_eq!(parse_board_list(old).unwrap()[0].address, "COM7");
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

    #[test]
    fn core_ids_and_list() {
        assert_eq!(
            core_id("esp32:esp32:esp32c3").as_deref(),
            Some("esp32:esp32")
        );
        assert_eq!(core_id("esp32"), None);
        let old = r#"[{"id": "esp32:esp32", "installed": "3.0.0"}]"#;
        assert_eq!(parse_core_list(old).unwrap(), ["esp32:esp32"]);
        let new = r#"{"platforms": [{"id": "arduino:avr"}]}"#;
        assert_eq!(parse_core_list(new).unwrap(), ["arduino:avr"]);
        assert!(parse_core_list("{}").unwrap().is_empty());
    }
}
