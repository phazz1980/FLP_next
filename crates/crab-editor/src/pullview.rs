//! "Open project from board": reads the project copy kept in the board's flash.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crab_codegen::Program;
use crab_core::{protect, Project};
use eframe::egui;

use crate::app::{EditorApp, LogLevel};

/// What the reader thread reports.
enum Report {
    Progress(usize, usize),
    Done(Result<Outcome, String>),
}

struct Outcome {
    path: PathBuf,
    /// `Some(true)` when the board runs firmware built for exactly this schema.
    same_schema: Option<bool>,
}

pub struct PullWindow {
    port: String,
    password: String,
    rx: Option<Receiver<Report>>,
    status: String,
}

impl PullWindow {
    pub fn new(port: String) -> Self {
        Self {
            port,
            password: String::new(),
            rx: None,
            status: String::new(),
        }
    }

    /// Reads the board in a thread and saves the project to `path`.
    fn start(&mut self, path: PathBuf, lib: crab_core::Library, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let (port, password) = (self.port.trim().to_owned(), self.password.clone());
        let ctx = ctx.clone();
        thread::spawn(move || {
            let send = |r: Report| {
                let _ = tx.send(r);
                ctx.request_repaint();
            };
            let result = read(&port, &password, path, &lib, |got, total| {
                send(Report::Progress(got, total))
            });
            send(Report::Done(result));
        });
        self.status = crate::i18n::tr("Чтение проекта с платы...").to_owned();
        self.rx = Some(rx);
    }
}

fn read(
    port: &str,
    password: &str,
    path: PathBuf,
    lib: &crab_core::Library,
    progress: impl FnMut(usize, usize),
) -> Result<Outcome, String> {
    let mut transport = crab_debug::transport::serial(port)()
        .map_err(|e| format!("не удалось открыть порт {port}: {e}"))?;
    let pulled = crab_debug::read_project(transport.as_mut(), password, progress)
        .map_err(|e| e.to_string())?;
    let archive = protect::open(&pulled.data, password).map_err(|e| e.to_string())?;
    let project = Project::from_archive_bytes(&archive, &path).map_err(|e| e.to_string())?;
    project.save().map_err(|e| e.to_string())?;
    let same_schema = pulled.fingerprint.and_then(|fp| {
        let enabled = project.without_disabled();
        let program = Program::build(&enabled, lib).ok()?;
        Some(crab_codegen::debug::debug_table(&program).fingerprint() == fp)
    });
    Ok(Outcome { path, same_schema })
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut win) = app.pull.take() else {
        return;
    };
    let mut keep = true;
    let mut opened = None;
    if let Some(rx) = &win.rx {
        while let Ok(report) = rx.try_recv() {
            match report {
                Report::Progress(got, total) => {
                    win.status = crate::i18n::fmt("Прочитано {0} из {1} байт", &[&got, &total]);
                }
                Report::Done(Ok(outcome)) => {
                    match outcome.same_schema {
                        Some(false) => app.info(
                            LogLevel::Warning,
                            crate::i18n::tr(
                                "Отпечаток схемы не совпадает с прошивкой: проект на плате старше или новее работающей программы",
                            ),
                        ),
                        _ => app.info(
                            LogLevel::Ok,
                            crate::i18n::tr("Проект прочитан с платы"),
                        ),
                    }
                    opened = Some(outcome.path);
                    keep = false;
                }
                Report::Done(Err(e)) => {
                    win.status = e;
                    win.rx = None;
                    break;
                }
            }
        }
    }
    let busy = win.rx.is_some();
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Открыть проект с платы"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr(
                "Нужна плата с прошивкой, собранной с отладкой (Отладка → Залить с отладкой).",
            ));
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Порт:"));
                egui::ComboBox::from_id_salt("pull_port")
                    .selected_text(win.port.clone())
                    .show_ui(ui, |ui| {
                        for p in &app.ports {
                            ui.selectable_value(&mut win.port, p.address.clone(), &p.address);
                        }
                    });
                ui.add_enabled(
                    !busy,
                    egui::TextEdit::singleline(&mut win.port).desired_width(80.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Пароль проекта:"));
                ui.add_enabled(
                    !busy,
                    egui::TextEdit::singleline(&mut win.password).password(true),
                );
            });
            if !win.status.is_empty() {
                ui.label(&win.status);
            }
            let ready = !busy && !win.port.trim().is_empty();
            if ui
                .add_enabled(ready, egui::Button::new(crate::i18n::tr("Прочитать...")))
                .on_hover_text(crate::i18n::tr("Спросит, куда сохранить проект"))
                .clicked()
            {
                let picked = rfd::FileDialog::new()
                    .set_title(crate::i18n::tr("Сохранить проект с платы"))
                    .add_filter(
                        crate::i18n::tr("Проект CrabLogic"),
                        &[crab_core::project::PROJECT_EXT],
                    )
                    .set_file_name(format!("С платы.{}", crab_core::project::PROJECT_EXT))
                    .save_file();
                if let Some(mut path) = picked {
                    if !crab_core::project::is_project_file(&path) {
                        path.set_extension(crab_core::project::PROJECT_EXT);
                    }
                    win.start(path, app.lib.clone(), ctx);
                }
            }
        });
    if !open && !busy {
        keep = false;
    }
    if keep {
        app.pull = Some(win);
    }
    if let Some(path) = opened {
        app.request_open(path);
    }
}
