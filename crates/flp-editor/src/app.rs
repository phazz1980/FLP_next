//! Main window: menu, toolbar, panels and global shortcuts.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use flp_build::{ArduinoCli, SerialPort};
use flp_core::{Library, PortRef, Severity};

use crate::canvas::{self, CanvasState};
use crate::debugview::{self, DebugState};
use crate::dialogs::{self, Dialog};
use crate::doc::Doc;
use crate::job::{Job, JobEvent, JobKind};
use crate::panels;
use crate::theme::{self, palette, ThemeKind};

/// What is selected on the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Block(String),
    /// A link, identified by the input it feeds.
    Link(PortRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Library,
    Ai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Ok,
    Warning,
    Error,
}

pub struct LogLine {
    pub level: LogLevel,
    pub text: String,
    /// Block to select when the line is clicked.
    pub block: Option<String>,
}

/// Modal question asked before discarding unsaved changes.
#[derive(Debug, Clone)]
enum Pending {
    Open(PathBuf),
    Close,
}

pub struct EditorApp {
    pub lib: Library,
    lib_dir: Option<PathBuf>,
    lib_stamp: Option<SystemTime>,
    lib_checked: Instant,
    pub doc: Option<Doc>,
    pub selection: Option<Selection>,
    /// Active sheet: where blocks added from the library go.
    pub sheet: u32,
    /// Sheets added by the user that have no blocks yet.
    pub extra_sheets: BTreeSet<u32>,
    pub canvas: CanvasState,
    pub right_tab: RightTab,
    pub log: Vec<LogLine>,
    /// Open pop-up window (block properties, variable, sheet).
    pub dialog: Option<Dialog>,
    pub theme: ThemeKind,
    /// Selected upload port; empty means automatic.
    pub port: String,
    pub ports: Vec<SerialPort>,
    port_scan: Option<Receiver<Result<Vec<SerialPort>, String>>>,
    /// Online debugging session, when running.
    pub debug: Option<DebugState>,
    job: Option<Job>,
    pending: Option<Pending>,
    allow_close: bool,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, project: Option<PathBuf>) -> Self {
        let mut app = Self {
            lib: Library::default(),
            lib_dir: None,
            lib_stamp: None,
            lib_checked: Instant::now(),
            doc: None,
            selection: None,
            sheet: 1,
            extra_sheets: BTreeSet::new(),
            canvas: CanvasState::default(),
            right_tab: RightTab::Library,
            log: Vec::new(),
            dialog: None,
            theme: ThemeKind::Light,
            port: String::new(),
            ports: Vec::new(),
            port_scan: None,
            debug: None,
            job: None,
            pending: None,
            allow_close: false,
        };
        theme::apply(&cc.egui_ctx, app.theme);
        app.scan_ports(&cc.egui_ctx);
        app.lib_dir = find_library(project.as_deref());
        app.reload_library();
        if let Some(path) = project {
            app.open(&path);
        }
        app
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, theme: ThemeKind) {
        self.theme = theme;
        theme::apply(ctx, theme);
    }

    /// Looks for serial ports with arduino-cli on a worker thread.
    pub fn scan_ports(&mut self, ctx: &egui::Context) {
        if self.port_scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let result = ArduinoCli::locate()
                .and_then(|cli| cli.serial_ports())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.port_scan = Some(rx);
    }

    pub fn scanning_ports(&self) -> bool {
        self.port_scan.is_some()
    }

    fn poll_ports(&mut self) {
        let Some(rx) = &self.port_scan else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.port_scan = None;
        match result {
            Ok(ports) => self.ports = ports,
            Err(e) => self.info(LogLevel::Warning, format!("Порты не найдены: {e}")),
        }
    }

    pub fn info(&mut self, level: LogLevel, text: impl Into<String>) {
        self.log.push(LogLine {
            level,
            text: text.into(),
            block: None,
        });
    }

    fn reload_library(&mut self) {
        let Some(dir) = self.lib_dir.clone() else {
            self.info(
                LogLevel::Error,
                "Не найдена папка библиотеки блоков library/ (задайте FLP_LIBRARY)",
            );
            return;
        };
        match Library::load(&dir) {
            Ok((lib, errors)) => {
                let count = lib.len();
                self.lib = lib;
                for e in errors {
                    self.info(LogLevel::Warning, format!("Блок пропущен: {e}"));
                }
                self.info(
                    LogLevel::Info,
                    format!("Библиотека: {count} блоков из {}", dir.display()),
                );
            }
            Err(e) => self.info(LogLevel::Error, e.to_string()),
        }
        self.lib_stamp = newest_mtime(&dir);
        if let Some(doc) = &mut self.doc {
            doc.recheck(&self.lib);
        }
    }

    /// Hot reload: new or changed block folders are picked up without restart.
    fn watch_library(&mut self) {
        if self.lib_checked.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.lib_checked = Instant::now();
        let Some(dir) = &self.lib_dir else { return };
        let stamp = newest_mtime(dir);
        if stamp != self.lib_stamp {
            self.reload_library();
        }
    }

    fn request_open(&mut self, path: PathBuf) {
        if self.doc.as_ref().is_some_and(|d| d.dirty) {
            self.pending = Some(Pending::Open(path));
        } else {
            self.open(&path);
        }
    }

    pub fn open(&mut self, path: &Path) {
        if self.lib_dir.is_none() {
            self.lib_dir = find_library(Some(path));
            self.reload_library();
        }
        // Debugging belongs to the previous project's firmware.
        self.stop_debug();
        match Doc::open(path, &self.lib) {
            Ok(doc) => {
                self.info(LogLevel::Info, format!("Открыт проект {}", path.display()));
                self.port = doc.project.config.board.port.clone().unwrap_or_default();
                self.doc = Some(doc);
                self.selection = None;
                self.sheet = 1;
                self.extra_sheets.clear();
                self.canvas = CanvasState::default();
            }
            Err(e) => self.info(LogLevel::Error, e.to_string()),
        }
    }

    fn new_project(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Папка нового проекта")
            .pick_folder()
        else {
            return;
        };
        if dir.join(flp_core::project::PROJECT_FILE).exists() {
            self.request_open(dir);
            return;
        }
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_owned();
        let config = format!(
            "[project]\nname = \"{name}\"\n\n[board]\nfqbn = \"esp32:esp32:esp32\"   # ESP32 DevKit\n"
        );
        let files = [
            (flp_core::project::PROJECT_FILE, config.as_str()),
            (flp_core::project::LOGIC_FILE, "# Блоки и связи схемы.\n"),
        ];
        for (file, text) in files {
            if let Err(e) = fs::write(dir.join(file), text) {
                self.info(
                    LogLevel::Error,
                    format!("{}: {e}", dir.join(file).display()),
                );
                return;
            }
        }
        self.open(&dir);
    }

    pub fn save(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        match doc.save() {
            Ok(()) => {
                let root = doc.project.root.display().to_string();
                self.info(LogLevel::Ok, format!("Сохранено: {root}"));
            }
            Err(e) => self.info(LogLevel::Error, e.to_string()),
        }
    }

    pub fn undo(&mut self) {
        if let Some(doc) = &mut self.doc {
            doc.undo(&self.lib);
            self.selection = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(doc) = &mut self.doc {
            doc.redo(&self.lib);
            self.selection = None;
        }
    }

    pub fn delete_selection(&mut self) {
        let (Some(doc), Some(sel)) = (&mut self.doc, self.selection.take()) else {
            return;
        };
        let result = match &sel {
            Selection::Block(id) => doc
                .edit(&self.lib, None, |p| p.remove_block(id))
                .map_err(|e| e.to_string()),
            Selection::Link(to) => doc.edit(&self.lib, None, |p| {
                if p.disconnect(to) {
                    Ok(())
                } else {
                    Err("связь не найдена".to_owned())
                }
            }),
        };
        if let Err(e) = result {
            self.info(LogLevel::Error, e);
        }
    }

    /// Runs the schema check and prints the result to the log.
    pub fn run_check(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        doc.recheck(&self.lib);
        let mut lines = Vec::new();
        for d in &doc.report.diagnostics {
            let level = match d.severity {
                Severity::Error => LogLevel::Error,
                Severity::Warning => LogLevel::Warning,
            };
            let block = d.block.clone().or_else(|| {
                d.link
                    .and_then(|i| doc.project.logic.links.get(i))
                    .map(|l| l.to.block.clone())
            });
            lines.push(LogLine {
                level,
                text: d.to_string(),
                block,
            });
        }
        let errors = doc.report.errors().count();
        self.log.extend(lines);
        if errors == 0 {
            self.info(LogLevel::Ok, "Проверка: ошибок нет");
        } else {
            self.info(LogLevel::Error, format!("Проверка: ошибок — {errors}"));
        }
    }

    pub fn start_job(&mut self, kind: JobKind, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        // The upload needs the serial port that debugging holds.
        if matches!(kind, JobKind::Upload { .. }) {
            self.stop_debug();
        }
        let Some(doc) = &self.doc else { return };
        if doc.report.has_errors() {
            self.run_check();
            self.info(LogLevel::Error, "Сначала исправьте ошибки схемы");
            return;
        }
        let ctx = ctx.clone();
        let job = Job::start(kind, doc.project.clone(), self.lib.clone(), move || {
            ctx.request_repaint()
        });
        self.info(LogLevel::Info, format!("{}...", job.title));
        self.job = Some(job);
    }

    pub fn job_running(&self) -> bool {
        self.job.is_some()
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let mut done = false;
        let mut connect = None;
        for event in job.poll() {
            match event {
                JobEvent::Line(text) => self.info(LogLevel::Info, text),
                JobEvent::Done(Ok(result)) => {
                    self.info(LogLevel::Ok, result.message);
                    if let (true, Some(port)) = (result.debug, result.port) {
                        connect = Some(port);
                    }
                    done = true;
                }
                JobEvent::Done(Err(text)) => {
                    self.info(LogLevel::Error, format!("ошибка: {text}"));
                    done = true;
                }
            }
        }
        if done {
            self.job = None;
        }
        if let Some(port) = connect {
            self.connect_debug(Some(port), ctx);
        }
    }

    /// Builds and uploads firmware with the debugging module, then connects.
    pub fn start_debug(&mut self, ctx: &egui::Context) {
        let port = self.port_choice();
        self.start_job(JobKind::Upload { port, debug: true }, ctx);
    }

    /// Connects to a board that already runs debugging firmware.
    pub fn connect_debug(&mut self, port: Option<String>, ctx: &egui::Context) {
        self.stop_debug();
        let Some(doc) = &self.doc else { return };
        let port = port
            .or_else(|| self.port_choice())
            .or_else(|| doc.project.config.board.port.clone())
            .or_else(|| (self.ports.len() == 1).then(|| self.ports[0].address.clone()));
        let Some(port) = port else {
            self.info(
                LogLevel::Error,
                "Выберите порт платы на панели инструментов",
            );
            return;
        };
        match DebugState::start(&doc.project, &self.lib, &port, ctx) {
            Ok(state) => {
                self.info(LogLevel::Info, format!("Отладка: подключение к {port}"));
                self.debug = Some(state);
            }
            Err(e) => self.info(LogLevel::Error, e),
        }
    }

    pub fn stop_debug(&mut self) {
        if let Some(d) = self.debug.take() {
            d.stop();
            self.info(LogLevel::Info, "Отладка остановлена");
        }
    }

    fn poll_debug(&mut self) {
        let Some(d) = &mut self.debug else { return };
        d.refresh();
        let fresh: Vec<String> = d
            .live
            .console
            .iter()
            .skip(d.console_seen)
            .cloned()
            .collect();
        d.console_seen = d.live.console.len();
        for line in fresh {
            self.info(LogLevel::Info, format!("> {line}"));
        }
    }

    /// Sheets to show: with blocks, named, added by the user, and the active one.
    pub fn sheets(&self) -> Vec<u32> {
        let mut sheets: BTreeSet<u32> = self.extra_sheets.clone();
        sheets.insert(1);
        sheets.insert(self.sheet);
        if let Some(doc) = &self.doc {
            let layout = &doc.project.layout;
            sheets.extend(layout.blocks.values().map(|p| p.sheet));
            sheets.extend(layout.sheets.iter().map(|s| s.number));
        }
        sheets.into_iter().collect()
    }

    /// Selects a block and scrolls it into view.
    pub fn focus_block(&mut self, id: &str) {
        self.canvas.focus = Some(id.to_owned());
        if let Some(pos) = self
            .doc
            .as_ref()
            .and_then(|d| d.project.layout.blocks.get(id))
        {
            self.sheet = pos.sheet;
        }
        self.select(Some(Selection::Block(id.to_owned())));
    }

    pub fn select(&mut self, selection: Option<Selection>) {
        self.selection = selection;
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if pressed(shortcut(Modifiers::COMMAND, Key::S)) {
            self.save();
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::Z)) {
            self.undo();
        }
        if pressed(shortcut(Modifiers::COMMAND, Key::Y))
            || pressed(shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
        {
            self.redo();
        }
        if pressed(shortcut(Modifiers::NONE, Key::F7)) {
            self.run_check();
        }
        if pressed(shortcut(Modifiers::NONE, Key::F5)) {
            self.start_job(JobKind::Build, ctx);
        }
        // Delete only when no text field or window has focus.
        if self.dialog.is_none()
            && !ctx.egui_wants_keyboard_input()
            && pressed(shortcut(Modifiers::NONE, Key::Delete))
        {
            self.delete_selection();
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Файл", |ui| {
                if ui.button("Новый проект...").clicked() {
                    self.new_project();
                }
                if ui.button("Открыть...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Папка проекта")
                        .pick_folder()
                    {
                        self.request_open(dir);
                    }
                }
                let has_doc = self.doc.is_some();
                if ui
                    .add_enabled(
                        has_doc,
                        egui::Button::new("Сохранить").shortcut_text("Ctrl+S"),
                    )
                    .clicked()
                {
                    self.save();
                }
                ui.separator();
                if ui.button("Выход").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Вид", |ui| panels::theme_menu(self, ui));
            ui.menu_button("Правка", |ui| {
                let (can_undo, can_redo) = self
                    .doc
                    .as_ref()
                    .map_or((false, false), |d| (d.can_undo(), d.can_redo()));
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new("Отменить").shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new("Повторить").shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.selection.is_some(),
                        egui::Button::new("Удалить").shortcut_text("Del"),
                    )
                    .clicked()
                {
                    self.delete_selection();
                }
            });
            ui.menu_button("Проект", |ui| {
                let has_doc = self.doc.is_some();
                let idle = !self.job_running();
                if ui
                    .add_enabled(has_doc, egui::Button::new("Проверить").shortcut_text("F7"))
                    .clicked()
                {
                    self.run_check();
                }
                if ui
                    .add_enabled(
                        has_doc && idle,
                        egui::Button::new("Собрать").shortcut_text("F5"),
                    )
                    .clicked()
                {
                    self.start_job(JobKind::Build, ui.ctx());
                }
                if ui
                    .add_enabled(has_doc && idle, egui::Button::new("Залить на плату"))
                    .clicked()
                {
                    let port = self.port_choice();
                    self.start_job(JobKind::Upload { port, debug: false }, ui.ctx());
                }
            });
            ui.menu_button("Библиотека", |ui| {
                if ui.button("Перечитать блоки").clicked() {
                    self.reload_library();
                }
            });
        });
    }

    /// Port typed in the toolbar, or `None` to use project.toml / autodetect.
    pub fn port_choice(&self) -> Option<String> {
        let port = self.port.trim();
        (!port.is_empty()).then(|| port.to_owned())
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| match &self.doc {
            Some(doc) => {
                let p = &doc.project;
                ui.label(format!("Проект: {}", p.config.project.name));
                ui.separator();
                ui.label(format!("Плата: {}", p.config.board.fqbn));
                ui.separator();
                ui.label(format!("Блоков: {}", p.logic.blocks.len()));
                ui.separator();
                let errors = doc.report.errors().count();
                if errors == 0 {
                    ui.label(RichText::new("Схема без ошибок").color(palette(self.theme).ok));
                } else {
                    ui.label(
                        RichText::new(format!("Ошибок в схеме: {errors}"))
                            .color(palette(self.theme).error),
                    );
                }
                if doc.dirty {
                    ui.separator();
                    ui.label("Есть несохранённые изменения");
                }
                if let Some(job) = &self.job {
                    ui.separator();
                    ui.spinner();
                    ui.label(job.title);
                }
                if let Some(d) = &self.debug {
                    ui.separator();
                    let color = if d.online() {
                        palette(self.theme).ok
                    } else {
                        palette(self.theme).warning
                    };
                    ui.label(
                        RichText::new(format!("Отладка {}: {}", d.port, d.live.status))
                            .color(color),
                    );
                }
            }
            None => {
                ui.label("Проект не открыт");
            }
        });
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        let mut decision = None;
        egui::Modal::new(egui::Id::unique("unsaved")).show(ctx, |ui| {
            ui.heading("Несохранённые изменения");
            ui.label("Сохранить изменения в проекте?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Сохранить").clicked() {
                    decision = Some(Some(true));
                }
                if ui.button("Не сохранять").clicked() {
                    decision = Some(Some(false));
                }
                if ui.button("Отмена").clicked() {
                    decision = Some(None);
                }
            });
        });
        let Some(decision) = decision else { return };
        self.pending = None;
        let Some(save) = decision else { return };
        if save {
            self.save();
            if self.doc.as_ref().is_some_and(|d| d.dirty) {
                return; // saving failed, keep the project open
            }
        }
        match pending {
            Pending::Open(path) => self.open(&path),
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        let close = ctx.input(|i| i.viewport().close_requested());
        if close && !self.allow_close && self.doc.as_ref().is_some_and(|d| d.dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match &self.doc {
            Some(doc) => format!(
                "{}{} — FLP_next",
                doc.project.config.project.name,
                if doc.dirty { " *" } else { "" }
            ),
            None => "FLP_next".to_owned(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_job(&ctx);
        self.poll_debug();
        self.poll_ports();
        self.watch_library();
        self.shortcuts(&ctx);
        self.handle_close(&ctx);
        self.update_title(&ctx);
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        } else {
            // Keep the library watcher ticking even without input.
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            panels::toolbar(self, ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("log")
            .resizable(true)
            .default_size(150.0)
            .show(ui, |ui| panels::log(self, ui));
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| panels::left(self, ui));
        egui::Panel::right("library")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| panels::right(self, ui));
        egui::CentralPanel::default_margins().show(ui, |ui| canvas::show(self, ui));

        dialogs::show(self, &ctx);
        let pal = palette(self.theme);
        if let Some(d) = &mut self.debug {
            debugview::chart_window(d, &pal, &ctx);
        }
        self.unsaved_dialog(&ctx);
    }
}

/// `library/` next to the project (or its parents), next to the executable, or `FLP_LIBRARY`.
fn find_library(project: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FLP_LIBRARY") {
        return Some(PathBuf::from(dir));
    }
    let starts = [
        project.and_then(|p| p.canonicalize().ok()),
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
    ];
    starts
        .into_iter()
        .flatten()
        .flat_map(|s| s.ancestors().map(|a| a.join("library")).collect::<Vec<_>>())
        .find(|d| d.is_dir())
}

/// Latest modification time of any file in the library (two levels deep).
fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    let mut newest = fs::metadata(dir).and_then(|m| m.modified()).ok();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let mut consider = |path: &Path| {
            if let Ok(t) = fs::metadata(path).and_then(|m| m.modified()) {
                newest = newest.max(Some(t));
            }
        };
        consider(&entry.path());
        if let Ok(files) = fs::read_dir(entry.path()) {
            for f in files.flatten() {
                consider(&f.path());
            }
        }
    }
    newest
}
