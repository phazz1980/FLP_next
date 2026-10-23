//! Background build and upload, so the editor stays responsive.

use std::sync::mpsc::{self, Receiver};
use std::thread;

use flp_build::{ArduinoCli, CompileOptions};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

pub enum JobKind {
    Build,
    Upload { port: Option<String> },
}

pub enum JobEvent {
    Line(String),
    Done(Result<String, String>),
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
            JobKind::Upload { .. } => "Заливка",
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
) -> Result<String, String> {
    let program = Program::build(project, lib).map_err(|e| e.to_string())?;
    let code = ArduinoCpp.generate(&program).map_err(|e| e.to_string())?;
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
    cli.compile(&project.root, board, &CompileOptions::default(), log)
        .map_err(|e| e.to_string())?;

    match kind {
        JobKind::Build => Ok("Сборка готова".into()),
        JobKind::Upload { port } => {
            let port = cli
                .choose_port(port.as_deref(), board)
                .map_err(|e| e.to_string())?;
            log(&format!("Заливка на {port}..."));
            cli.upload(&project.root, board, &port, log)
                .map_err(|e| e.to_string())?;
            Ok(format!("Прошивка залита на {port}"))
        }
    }
}
