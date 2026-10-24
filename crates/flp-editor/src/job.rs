//! Background build and upload, so the editor stays responsive.

use std::sync::mpsc::{self, Receiver};
use std::thread;

use flp_build::{ArduinoCli, BuildMode, CompileOptions, OutputLine};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

use crate::app::LogLevel;

pub enum JobKind {
    Build,
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
            JobKind::Precompile => "Фоновая сборка",
            JobKind::Upload { .. } if debug => "Заливка отладочной прошивки",
            JobKind::Upload { .. } => "Заливка",
        };
        let background = matches!(kind, JobKind::Precompile);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let send = |e: JobEvent| {
                let _ = tx.send(e);
                wake();
            };
            let result = run(kind, debug, &project, &lib, &mut |level, line| {
                send(JobEvent::Line(level, line.to_owned()))
            });
            send(JobEvent::Done(result));
        });
        Self {
            title,
            background,
            rx,
        }
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
    out: &mut dyn FnMut(LogLevel, &str),
) -> Result<Done, String> {
    let background = matches!(kind, JobKind::Precompile);
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let code = ArduinoCpp { debug }
        .generate(&program)
        .map_err(|e| e.to_string())?;
    let (path, _) = flp_codegen::write_sketch(&project.root, &code).map_err(|e| e.to_string())?;
    out(LogLevel::Info, &format!("Скетч: {}", path.display()));

    let cli = ArduinoCli::locate().map_err(|e| e.to_string())?;
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
        ..Default::default()
    };
    // Only build stages and diagnostics; compiler command lines stay hidden.
    cli.compile(
        &project.root,
        board,
        &opts,
        &mut |line| match flp_build::classify(line) {
            OutputLine::Stage(s) | OutputLine::Size(s) | OutputLine::Context(s) => {
                out(LogLevel::Info, &s)
            }
            OutputLine::Warning(s) => out(LogLevel::Warning, &s),
            OutputLine::Error(s) => out(LogLevel::Error, &s),
            OutputLine::Detail => {}
        },
    )
    .map_err(|e| e.to_string())?;

    match kind {
        JobKind::Build | JobKind::Precompile => Ok(Done {
            message: "Сборка готова".into(),
            port: None,
            debug,
        }),
        JobKind::Upload { port } => {
            let port = cli
                .choose_port(port.as_deref(), board)
                .map_err(|e| e.to_string())?;
            out(LogLevel::Info, &format!("Заливка на {port}..."));
            cli.upload(&project.root, board, &port, &mut |l| out(LogLevel::Info, l))
                .map_err(|e| e.to_string())?;
            Ok(Done {
                message: format!("Прошивка залита на {port}"),
                port: Some(port),
                debug,
            })
        }
    }
}
