//! Background build and upload, so the editor stays responsive.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use flp_build::{
    ArduinoCli, BuildControl, BuildMode, CliReason, CompileOptions, Compiled, OutputLine,
};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

use crate::app::LogLevel;

pub enum JobKind {
    Build,
    /// Build even if the last build is up to date (library or core changed).
    Rebuild,
    /// Quiet low-priority build after edits, so the user's build finds the firmware
    /// ready (or at least the core and libraries compiled).
    Precompile,
    Upload {
        port: Option<String>,
    },
    /// Sheets as WASM modules into the master firmware already on the board.
    UploadModules {
        port: Option<String>,
    },
}

pub struct Done {
    pub message: String,
    /// Port the firmware was uploaded to.
    pub port: Option<String>,
    pub debug: bool,
}

pub enum JobEvent {
    Line(LogLevel, String),
    /// Shown even for a background job.
    Note(LogLevel, String),
    Done(Result<Done, String>),
}

pub struct Job {
    pub title: &'static str,
    /// Precompilation: no log output, buttons stay enabled.
    pub background: bool,
    /// Upload: the serial port must stay free.
    pub uses_port: bool,
    control: Arc<BuildControl>,
    /// The build goes without arduino-cli (or is up to date).
    direct: Arc<AtomicBool>,
    rx: Receiver<JobEvent>,
}

impl Job {
    /// Starts generation + build (+ upload) on a worker thread. `wake` is called after
    /// every event so the UI can repaint.
    /// `debug`: firmware with the online debugging module; after an upload the editor
    /// connects to it.
    pub fn start(
        kind: JobKind,
        debug: bool,
        project: Project,
        source: Project,
        lib: Library,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let title = match kind {
            JobKind::Build => crate::i18n::tr("Сборка"),
            JobKind::Rebuild => crate::i18n::tr("Пересборка"),
            JobKind::Precompile => crate::i18n::tr("Фоновая сборка"),
            JobKind::Upload { .. } if debug => crate::i18n::tr("Заливка отладочной прошивки"),
            JobKind::Upload { .. } => crate::i18n::tr("Заливка"),
            JobKind::UploadModules { .. } => crate::i18n::tr("Заливка модулей"),
        };
        let background = matches!(kind, JobKind::Precompile);
        let uses_port = matches!(kind, JobKind::Upload { .. } | JobKind::UploadModules { .. });
        let control = Arc::new(BuildControl::default());
        let worker = control.clone();
        let direct = Arc::new(AtomicBool::new(false));
        let direct_flag = direct.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let send = |e: JobEvent| {
                let _ = tx.send(e);
                wake();
            };
            let report = Report {
                out: &mut |level, line| send(JobEvent::Line(level, line.to_owned())),
                note: &mut |level, line| send(JobEvent::Note(level, line.to_owned())),
                direct: &direct_flag,
            };
            let result = run(kind, debug, &project, &source, &lib, worker, report);
            send(JobEvent::Done(result));
        });
        Self {
            title,
            background,
            uses_port,
            control,
            direct,
            rx,
        }
    }

    /// Stops the job; it ends with an error soon after.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Compiles without arduino-cli: cheap to cancel and start again.
    pub fn is_direct(&self) -> bool {
        self.direct.load(Ordering::Relaxed)
    }

    pub fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }

    /// Runs a background build at normal priority (the user waits for it now).
    pub fn raise_priority(&self) {
        self.control.raise_priority();
    }

    /// Events received since the last call.
    pub fn poll(&self) -> Vec<JobEvent> {
        self.rx.try_iter().collect()
    }
}

/// Where a job reports: the log (hidden for a background job), lines shown anyway,
/// and whether the build goes without arduino-cli.
struct Report<'a> {
    out: &'a mut dyn FnMut(LogLevel, &str),
    note: &'a mut dyn FnMut(LogLevel, &str),
    direct: &'a AtomicBool,
}

fn run(
    kind: JobKind,
    debug: bool,
    project: &Project,
    source: &Project,
    lib: &Library,
    control: Arc<BuildControl>,
    report: Report,
) -> Result<Done, String> {
    let Report { out, note, direct } = report;
    let background = matches!(kind, JobKind::Precompile);
    if let JobKind::UploadModules { port } = &kind {
        return upload_modules(port.as_deref(), project, lib, debug, out);
    }
    // The plain upload puts the master on the board; the schema follows as modules.
    // Debugging still needs the schema firmware with the debugging module.
    if let (JobKind::Upload { port }, false) = (&kind, debug) {
        return upload_master(port.as_deref(), project, control, out);
    }
    if let Some(w2c2) = crate::theme::w2c2() {
        for problem in flp_build::wasm_c::translate_library(&w2c2, lib) {
            out(LogLevel::Warning, &problem);
        }
    }
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let code = ArduinoCpp { debug }
        .generate(&program)
        .map_err(|e| e.to_string())?;
    // The project goes into the firmware as saved, not as prepared for generation.
    let code = flp_codegen::embed_project(&code, source).map_err(|e| e.to_string())?;
    let (path, _) =
        flp_codegen::write_sketch(&project.work_dir(), &code).map_err(|e| e.to_string())?;
    out(
        LogLevel::Info,
        &crate::i18n::fmt("Скетч: {0}", &[&path.display()]),
    );

    let cli = ArduinoCli::locate()
        .map_err(|e| e.to_string())?
        .with_control(control);
    let board = &project.config.board;
    let opts = CompileOptions {
        mode: if debug {
            BuildMode::Debug
        } else {
            BuildMode::Release
        },
        verbose: !background,
        low_priority: background,
        force: matches!(kind, JobKind::Rebuild),
        libraries: program.attached_libraries(),
        ..Default::default()
    };
    match cli.cli_reason(&project.work_dir(), board, &opts) {
        Some(reason) => note(
            LogLevel::Info,
            &crate::i18n::fmt("Сборка через arduino-cli: {0}", &[&reason_text(reason)]),
        ),
        None => direct.store(true, Ordering::Relaxed),
    }
    let installed = cli
        .ensure_libraries(&program.libraries(), &mut |l| out(LogLevel::Info, l))
        .map_err(|e| e.to_string())?;
    if !installed.is_empty() {
        out(
            LogLevel::Info,
            &crate::i18n::fmt("Установлены библиотеки: {0}", &[&installed.join(", ")]),
        );
    }
    out(
        LogLevel::Info,
        &crate::i18n::fmt(
            "Сборка для {0}...",
            &[&crate::boardcfg::board_name(board, lib.boards())],
        ),
    );
    // Only build stages and diagnostics; compiler command lines stay hidden.
    let sketch = std::fs::read_to_string(&path).unwrap_or_default();
    let blocks: Vec<(String, String)> = project
        .logic
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b.kind.clone()))
        .collect();
    let mut errors = 0;
    let mut first_error: Option<String> = None;
    let compiled = cli
        .compile(
            &project.work_dir(),
            board,
            &opts,
            &mut |line| match flp_build::classify(line) {
                OutputLine::Stage(s) | OutputLine::Size(s) | OutputLine::Context(s) => {
                    out(LogLevel::Info, &s)
                }
                OutputLine::Warning(s) => out(LogLevel::Warning, &s),
                OutputLine::Error(s) => {
                    out(LogLevel::Error, &s);
                    errors += 1;
                    let note = explain_error(&s, &sketch, &blocks);
                    for n in &note {
                        out(LogLevel::Error, &format!("    ↳ {n}"));
                    }
                    if first_error.is_none() {
                        first_error = Some(note.join("; "));
                    }
                }
                OutputLine::Detail => {}
            },
        )
        .map_err(|e| match (&e, errors) {
            (flp_build::BuildError::Failed { .. }, 1..) => {
                let first = first_error.as_deref().filter(|f| !f.is_empty());
                crate::i18n::fmt(
                    "Ошибка компиляции ({0}): подробности в журнале{1}",
                    &[
                        &errors,
                        &first.map(|f| format!(". Первая: {f}")).unwrap_or_default(),
                    ],
                )
            }
            _ => e.to_string(),
        })?;
    match compiled {
        Compiled::UpToDate => out(
            LogLevel::Info,
            crate::i18n::tr("Прошивка уже собрана, схема не менялась"),
        ),
        Compiled::Direct => out(
            LogLevel::Info,
            crate::i18n::tr("Прямая сборка: изменилась только схема, arduino-cli не запускался"),
        ),
        Compiled::Built => {}
    }

    match kind {
        JobKind::Build | JobKind::Rebuild | JobKind::Precompile => Ok(Done {
            message: crate::i18n::tr("Сборка готова").into(),
            port: None,
            debug,
        }),
        JobKind::UploadModules { .. } => unreachable!("handled before the sketch is built"),
        JobKind::Upload { port } => {
            let port = cli
                .choose_port(port.as_deref(), board)
                .map_err(|e| e.to_string())?;
            warn_if_other_board(&cli, &port, &board.fqbn, out);
            out(
                LogLevel::Info,
                &crate::i18n::fmt("Заливка на {0}...", &[&port]),
            );
            let mut explained = None;
            let result = cli.upload(&project.work_dir(), board, &port, &mut |l| {
                match flp_build::explain_upload(l, &board.fqbn) {
                    Some(text) => {
                        out(LogLevel::Info, l);
                        out(LogLevel::Error, &text);
                        explained.get_or_insert(text);
                    }
                    None => out(LogLevel::Info, l),
                }
            });
            result.map_err(|e| explained.unwrap_or_else(|| e.to_string()))?;
            Ok(Done {
                message: crate::i18n::fmt("Прошивка залита на {0}", &[&port]),
                port: Some(port),
                debug,
            })
        }
    }
}

/// Builds the master firmware and uploads it: the schema itself goes to the board as
/// WASM modules (`upload_modules`).
fn upload_master(
    port: Option<&str>,
    project: &Project,
    control: Arc<BuildControl>,
    out: &mut dyn FnMut(LogLevel, &str),
) -> Result<Done, String> {
    let cli = ArduinoCli::locate()
        .map_err(|e| e.to_string())?
        .with_control(control);
    let board = &project.config.board;
    let port = cli.choose_port(port, board).map_err(|e| e.to_string())?;
    warn_if_other_board(&cli, &port, &board.fqbn, out);
    out(
        LogLevel::Info,
        &crate::i18n::fmt("Сборка и заливка мастер-прошивки на {0}...", &[&port]),
    );
    let mut explained = None;
    let result = cli.upload_master(&project.work_dir(), board, &port, &mut |l| {
        match flp_build::explain_upload(l, &board.fqbn) {
            Some(text) => {
                out(LogLevel::Info, l);
                out(LogLevel::Error, &text);
                explained.get_or_insert(text);
            }
            None => match flp_build::classify(l) {
                OutputLine::Stage(s) | OutputLine::Size(s) => out(LogLevel::Info, &s),
                OutputLine::Warning(s) => out(LogLevel::Warning, &s),
                OutputLine::Error(s) => out(LogLevel::Error, &s),
                _ => {}
            },
        }
    });
    result.map_err(|e| explained.unwrap_or_else(|| e.to_string()))?;
    Ok(Done {
        message: crate::i18n::fmt("Мастер-прошивка залита на {0}", &[&port]),
        port: Some(port),
        debug: false,
    })
}

/// Builds every sheet as a WASM module and sends it to the master firmware on the board.
/// With `watch` the modules report every block output, so the debugger can show them.
fn upload_modules(
    port: Option<&str>,
    project: &Project,
    lib: &Library,
    watch: bool,
    out: &mut dyn FnMut(LogLevel, &str),
) -> Result<Done, String> {
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let mut modules = Vec::new();
    for sheet in program.sheets() {
        let module = program
            .wasm_module_with(sheet, watch)
            .map_err(|e| crate::i18n::fmt("лист {0}: {1}", &[&sheet, &e]))?;
        modules.push(module);
    }
    let cli = ArduinoCli::locate().map_err(|e| e.to_string())?;
    let port = cli
        .choose_port(port, &project.config.board)
        .map_err(|e| e.to_string())?;
    let mut serial = serialport::new(&port, 115_200)
        .timeout(std::time::Duration::from_millis(200))
        .open()
        .map_err(|e| crate::i18n::fmt("порт {0} не открыт: {1}", &[&port, &e]))?;
    // Release DTR/RTS so opening the port does not reset the board.
    let _ = serial.write_data_terminal_ready(false);
    let _ = serial.write_request_to_send(false);
    flp_build::mcu_module::wait_for_master(&mut serial, std::time::Duration::from_secs(8))
        .map_err(|e| e.to_string())?;
    for module in &modules {
        let allow = flp_sim::mcu::Manifest::parse(&module.manifest)
            .map(|m| m.allow)
            .unwrap_or_default();
        out(
            LogLevel::Info,
            &crate::i18n::fmt(
                "Модуль «{0}» ({1} байт) -> {2}...",
                &[&module.name, &module.wasm.len(), &port],
            ),
        );
        flp_build::mcu_module::upload_module(&mut serial, &module.name, &module.wasm, &allow)
            .map_err(|e| e.to_string())?;
    }
    Ok(Done {
        message: crate::i18n::fmt("Модули залиты на {0}", &[&port]),
        port: Some(port),
        debug: watch,
    })
}

fn reason_text(reason: CliReason) -> &'static str {
    crate::i18n::tr(match reason {
        CliReason::Forced => "выбрано «Пересобрать полностью»",
        CliReason::NoFullBuild => "нет прошлой полной сборки",
        CliReason::SketchFiles => "в папке скетча есть другие файлы",
        CliReason::Tool => "обновился arduino-cli",
        CliReason::Board => "изменились плата или её настройки",
        CliReason::Libraries => "изменился список библиотек",
        CliReason::Includes => "изменились #include скетча",
        CliReason::LibraryFiles => "изменились файлы ядра или библиотек",
    })
}

/// What a compiler error means and which block's code it is in.
fn explain_error(line: &str, sketch: &str, blocks: &[(String, String)]) -> Vec<String> {
    let mut notes = Vec::new();
    let Some((number, message)) = flp_build::parse_diagnostic(line) else {
        if let Some(hint) = flp_build::explain_compile(line) {
            notes.push(format!("Пояснение: {hint}"));
        }
        return notes;
    };
    if let Some((id, kind)) = flp_build::locate_block(sketch, number, blocks) {
        notes.push(format!(
            "Ошибка в коде блока «{id}» ({kind}), строка {number} скетча"
        ));
    } else {
        notes.push(format!("Строка {number} скетча (вне кода блоков)"));
    }
    if let Some(hint) = flp_build::explain_compile(&message) {
        notes.push(format!("Пояснение: {hint}"));
    }
    notes
}

/// Warns before upload when arduino-cli recognized another board on the port.
fn warn_if_other_board(
    cli: &ArduinoCli,
    port: &str,
    fqbn: &str,
    out: &mut dyn FnMut(LogLevel, &str),
) {
    let short = |f: &str| f.split(':').take(3).collect::<Vec<_>>().join(":");
    let Ok(ports) = cli.serial_ports() else {
        return;
    };
    let Some(found) = ports.iter().find(|p| p.address == port && p.has_board) else {
        return;
    };
    if !found.fqbns.iter().any(|f| short(f) == short(fqbn)) {
        out(
            LogLevel::Warning,
            &crate::i18n::fmt(
                "Возможно, выбрана неверная плата: на {0} опознано «{1}», а в проекте — {2}",
                &[
                    &port,
                    &found.description,
                    &fqbn.rsplit(':').next().unwrap_or(fqbn),
                ],
            ),
        );
    }
}
