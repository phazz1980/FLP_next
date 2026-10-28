//! Background build and upload, so the editor stays responsive.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use flp_build::{ArduinoCli, BuildControl, BuildMode, CompileOptions, Compiled, OutputLine};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

use crate::app::LogLevel;

pub enum JobKind {
    Build,
    /// Build even if the last build is up to date (library or core changed).
    Rebuild,
    /// Quiet low-priority build that warms the build cache after edits.
    Precompile,
    Upload {
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
    Done(Result<Done, String>),
}

pub struct Job {
    pub title: &'static str,
    /// Precompilation: no log output, buttons stay enabled.
    pub background: bool,
    /// Upload: the serial port must stay free.
    pub uses_port: bool,
    control: Arc<BuildControl>,
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
        lib: Library,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let title = match kind {
            JobKind::Build => "Сборка",
            JobKind::Rebuild => "Пересборка",
            JobKind::Precompile => "Фоновая сборка",
            JobKind::Upload { .. } if debug => "Заливка отладочной прошивки",
            JobKind::Upload { .. } => "Заливка",
        };
        let background = matches!(kind, JobKind::Precompile);
        let uses_port = matches!(kind, JobKind::Upload { .. });
        let control = Arc::new(BuildControl::default());
        let worker = control.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let send = |e: JobEvent| {
                let _ = tx.send(e);
                wake();
            };
            let result = run(kind, debug, &project, &lib, worker, &mut |level, line| {
                send(JobEvent::Line(level, line.to_owned()))
            });
            send(JobEvent::Done(result));
        });
        Self {
            title,
            background,
            uses_port,
            control,
            rx,
        }
    }

    /// Stops the job; it ends with an error soon after.
    pub fn cancel(&self) {
        self.control.cancel();
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

fn run(
    kind: JobKind,
    debug: bool,
    project: &Project,
    lib: &Library,
    control: Arc<BuildControl>,
    out: &mut dyn FnMut(LogLevel, &str),
) -> Result<Done, String> {
    let background = matches!(kind, JobKind::Precompile);
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let code = ArduinoCpp { debug }
        .generate(&program)
        .map_err(|e| e.to_string())?;
    let (path, _) =
        flp_codegen::write_sketch(&project.work_dir(), &code).map_err(|e| e.to_string())?;
    out(LogLevel::Info, &format!("Скетч: {}", path.display()));

    let cli = ArduinoCli::locate()
        .map_err(|e| e.to_string())?
        .with_control(control);
    let board = &project.config.board;
    let installed = cli
        .ensure_libraries(&program.libraries(), &mut |l| out(LogLevel::Info, l))
        .map_err(|e| e.to_string())?;
    if !installed.is_empty() {
        out(
            LogLevel::Info,
            &format!("Установлены библиотеки: {}", installed.join(", ")),
        );
    }
    out(LogLevel::Info, &format!("Сборка для {}...", board.fqbn));
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
                format!(
                    "Ошибка компиляции ({errors}): подробности в журнале{}",
                    first.map(|f| format!(". Первая: {f}")).unwrap_or_default()
                )
            }
            _ => e.to_string(),
        })?;
    if compiled == Compiled::UpToDate {
        out(LogLevel::Info, "Прошивка уже собрана, схема не менялась");
    }

    match kind {
        JobKind::Build | JobKind::Rebuild | JobKind::Precompile => Ok(Done {
            message: "Сборка готова".into(),
            port: None,
            debug,
        }),
        JobKind::Upload { port } => {
            let port = cli
                .choose_port(port.as_deref(), board)
                .map_err(|e| e.to_string())?;
            warn_if_other_board(&cli, &port, &board.fqbn, out);
            out(LogLevel::Info, &format!("Заливка на {port}..."));
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
                message: format!("Прошивка залита на {port}"),
                port: Some(port),
                debug,
            })
        }
    }
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
            &format!(
                "Возможно, выбрана неверная плата: на {port} опознано «{}», а в проекте — {fqbn}",
                found.description
            ),
        );
    }
}
