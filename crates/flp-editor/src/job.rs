//! Background build and upload, so the editor stays responsive.

use std::sync::mpsc::{self, Receiver};
use std::thread;

use flp_build::{ArduinoCli, BuildMode, CompileOptions};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

pub enum JobKind {
    Build,
    Upload {
        port: Option<String>,
        /// Firmware with the online debugging module; the editor connects afterwards.
        debug: bool,
    },
}

pub struct Done {
    pub message: String,
    /// Port the firmware was uploaded to.
    pub port: Option<String>,
    pub debug: bool,
}

pub enum JobEvent {
    Line(String),
    Done(Result<Done, String>),
}

pub struct Job {
    pub title: &'static str,
    rx: Receiver<JobEvent>,
}

impl Job {
    /// Starts generation + build (+ upload) on a worker thread. `wake` is called after
    /// every event so the UI can repaint.
    pub fn start(
        kind: JobKind,
        project: Project,
        lib: Library,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let title = match kind {
            JobKind::Build => "Сборка",
            JobKind::Upload { debug: false, .. } => "Заливка",
            JobKind::Upload { debug: true, .. } => "Заливка отладочной прошивки",
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let send = |e: JobEvent| {
                let _ = tx.send(e);
                wake();
            };
            let result = run(kind, &project, &lib, &mut |line| {
                send(JobEvent::Line(line.to_owned()))
            });
            send(JobEvent::Done(result));
        });
        Self { title, rx }
    }

    /// Events received since the last call.
    pub fn poll(&self) -> Vec<JobEvent> {
        self.rx.try_iter().collect()
    }
}

fn run(
    kind: JobKind,
    project: &Project,
    lib: &Library,
    log: &mut dyn FnMut(&str),
) -> Result<Done, String> {
    let debug = matches!(kind, JobKind::Upload { debug: true, .. });
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let code = ArduinoCpp { debug }
        .generate(&program)
        .map_err(|e| e.to_string())?;
    let (path, _) = flp_codegen::write_sketch(&project.root, &code).map_err(|e| e.to_string())?;
    log(&format!("Скетч: {}", path.display()));

    let cli = ArduinoCli::locate().map_err(|e| e.to_string())?;
    let board = &project.config.board;
    let installed = cli
        .ensure_libraries(&program.libraries(), log)
        .map_err(|e| e.to_string())?;
    if !installed.is_empty() {
        log(&format!("Установлены библиотеки: {}", installed.join(", ")));
    }
    log(&format!("Сборка для {}...", board.fqbn));
    let opts = CompileOptions {
        mode: if debug {
            BuildMode::Debug
        } else {
            BuildMode::Release
        },
        ..Default::default()
    };
    cli.compile(&project.root, board, &opts, log)
        .map_err(|e| e.to_string())?;

    match kind {
        JobKind::Build => Ok(Done {
            message: "Сборка готова".into(),
            port: None,
            debug,
        }),
        JobKind::Upload { port, .. } => {
            let port = cli
                .choose_port(port.as_deref(), board)
                .map_err(|e| e.to_string())?;
            log(&format!("Заливка на {port}..."));
            cli.upload(&project.root, board, &port, log)
                .map_err(|e| e.to_string())?;
            Ok(Done {
                message: format!("Прошивка залита на {port}"),
                port: Some(port),
                debug,
            })
        }
    }
}
