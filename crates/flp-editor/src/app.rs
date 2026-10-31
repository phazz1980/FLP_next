//! Main window: menu, toolbar, panels and global shortcuts.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

/// How often the editor looks for a new release while it stays open.
const UPDATE_RECHECK: Duration = Duration::from_secs(6 * 3600);

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use flp_build::{ArduinoCli, SerialPort};
use flp_core::project::PROJECT_EXT;
use flp_core::{
    BlockPos, BoardConfig, Fragment, Library, LocalValues, PortRef, Project, ProjectConfig,
    ProjectInfo, Severity,
};

use crate::canvas::{self, CanvasState};
use crate::debugview::{self, DebugState};
use crate::dialogs::{self, Dialog};
use crate::doc::Doc;
use crate::job::{Job, JobEvent, JobKind};
use crate::panels;
use crate::selfupdate;
use crate::theme::{self, Theme};
use crate::update::{self, Check, Release};

/// What a build was made from: project folder, schema revision, debug firmware.
type BuildKey = (PathBuf, u64, bool);

/// What is selected on the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Block(String),
    /// Several blocks (two or more), selected with Ctrl or a frame.
    Blocks(Vec<String>),
    /// A link, identified by the input it feeds.
    Link(PortRef),
}

impl Selection {
    /// Selection of the given blocks: none, one or a group.
    pub fn of_blocks(mut ids: Vec<String>) -> Option<Self> {
        ids.dedup();
        match ids.len() {
            0 => None,
            1 => ids.pop().map(Selection::Block),
            _ => Some(Selection::Blocks(ids)),
        }
    }

    pub fn blocks(&self) -> &[String] {
        match self {
            Selection::Block(id) => std::slice::from_ref(id),
            Selection::Blocks(ids) => ids,
            Selection::Link(_) => &[],
        }
    }

    pub fn has_block(&self, id: &str) -> bool {
        self.blocks().iter().any(|b| b == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Log,
    Console,
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
    /// Help target (`page#anchor`) that explains the problem, for the "Что это?" link.
    pub help: Option<String>,
}

/// Modal question asked before discarding unsaved changes.
#[derive(Debug, Clone)]
enum Pending {
    Open(PathBuf),
    Close,
}

pub struct EditorApp {
    pub lib: Library,
    pub lib_dir: Option<PathBuf>,
    /// `crablogic-web` started from the "Конструктор веб-интерфейса" menu item.
    web_editor: Option<std::process::Child>,
    lib_stamp: Option<SystemTime>,
    lib_checked: Instant,
    /// Modification time of the project file as the editor last read or wrote it; a
    /// different time means another program (MCP server, web designer) changed it.
    disk_stamp: Option<SystemTime>,
    disk_checked: Instant,
    /// The changed-on-disk warning for unsaved work was already shown for this stamp.
    disk_warned: bool,
    pub doc: Option<Doc>,
    pub selection: Option<Selection>,
    /// Active sheet: where blocks added from the library go.
    pub sheet: u32,
    /// Sheets added by the user that have no blocks yet.
    pub extra_sheets: BTreeSet<u32>,
    /// Last tag or variable clicked in the tree and the index of the image shown:
    /// the next click on it shows the next image.
    pub tree_cycle: Option<(String, usize)>,
    pub canvas: CanvasState,
    pub right_tab: RightTab,
    /// Panel sizes and which side panels are expanded; saved in the editor settings.
    pub panels: theme::PanelSizes,
    pub log: Vec<LogLine>,
    pub bottom_tab: BottomTab,
    /// Port monitor, or the console of the debugging session.
    pub console: crate::console::PortConsole,
    /// Open pop-up window (block properties, variable, sheet).
    pub dialog: Option<Dialog>,
    pub theme: Theme,
    /// Built-in and user themes.
    pub themes: Vec<Theme>,
    pub theme_editor: Option<crate::themeedit::ThemeEditor>,
    /// Open "Block editor" window.
    pub block_editor: Option<crate::blockedit::BlockEditor>,
    pub board_editor: Option<crate::boardedit::BoardEditor>,
    /// Open "Project code" window.
    pub code_view: Option<crate::codeview::CodeView>,
    pub help: Option<crate::help::HelpWindow>,
    /// User block waiting for the delete confirmation.
    pub block_delete: Option<String>,
    /// Sheet waiting for the delete confirmation.
    pub sheet_delete: Option<u32>,
    /// Full screen was requested while maximized: wait until the window is restored.
    fs_pending: bool,
    /// Attempts to resize a full-screen window that does not cover the monitor.
    fs_fixes: u8,
    /// Selected upload port; empty means automatic.
    pub port: String,
    pub ports: Vec<SerialPort>,
    port_scan: Option<Receiver<Result<Vec<SerialPort>, String>>>,
    /// Online debugging session, when running.
    pub debug: Option<DebugState>,
    /// Build firmware with the debugging module (all builds, precompilation too).
    pub debug_firmware: bool,
    /// Chat with the AI assistant.
    pub ai: crate::ai::AiChat,
    /// Live values published for the AI while debugging.
    ai_live: crate::ai::SharedLive,
    job: Option<Job>,
    /// Project and revision the running job was started for.
    job_rev: Option<BuildKey>,
    /// Build requested while precompilation runs; starts right after it.
    queued: Option<JobKind>,
    /// Project and revision the build folder holds a release build of.
    precompiled: Option<BuildKey>,
    /// Last seen revision and when it changed: precompile after a pause in edits.
    seen_rev: Option<(u64, Instant)>,
    pending: Option<Pending>,
    allow_close: bool,
    /// Autosave period in minutes; 0 means off.
    pub autosave_minutes: u32,
    last_autosave: Instant,
    /// Values kept by the program instead of the project file: Wi-Fi passwords
    /// for each SSID and the list of entered SSIDs.
    pub local: LocalValues,
    /// Look for a newer release at startup.
    update_check: bool,
    /// Download a found release in the background; installing stays the user's call.
    auto_update: bool,
    /// Offer beta versions (GitHub pre-releases) as updates.
    update_beta: bool,
    /// Running release check; the flag tells whether the user asked for it.
    update_rx: Option<Receiver<(bool, Result<Check, String>)>>,
    /// When the last update check was started (periodic re-check while the editor stays open).
    update_checked: Instant,
    /// Newer release found on GitHub.
    new_release: Option<Release>,
    /// The window with the changes of the newer releases is shown.
    update_window: bool,
    changes_window: bool,
    pub mcp: crate::mcp_host::McpHost,
    pub ai_conf: crate::aiconf::AiConf,
    report: crate::report::Form,
    pub import: crate::blockio::Import,
    pub board_import: crate::boardio::Import,
    /// User blocks whose revision differs between the project and the program.
    block_diffs: Vec<flp_core::packed::VersionDiff>,
    /// arduino-cli is missing: the window offers to install it.
    cli_prompt: CliPrompt,
    /// Update download in progress, or the error of the last attempt.
    self_update: SelfUpdate,
}

enum SelfUpdate {
    Idle(Option<String>),
    Downloading {
        done: Arc<AtomicU64>,
        total: Arc<AtomicU64>,
        rx: Receiver<Result<PathBuf, String>>,
        /// Started by the program, not the button: stop after downloading.
        auto: bool,
    },
    /// Downloaded and unpacked, waiting for the user to restart.
    Ready(PathBuf),
}

/// State of the "arduino-cli not found" offer.
enum CliPrompt {
    Hidden,
    Ask,
    /// winget runs on a worker thread and sends its lines and the final result.
    Installing(Receiver<Result<String, String>>),
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, project: Option<PathBuf>) -> Self {
        let mut app = Self {
            lib: Library::default(),
            lib_dir: None,
            web_editor: None,
            lib_stamp: None,
            lib_checked: Instant::now(),
            disk_stamp: None,
            disk_checked: Instant::now(),
            disk_warned: false,
            doc: None,
            selection: None,
            sheet: 1,
            extra_sheets: BTreeSet::new(),
            tree_cycle: None,
            canvas: CanvasState::default(),
            right_tab: RightTab::Library,
            panels: theme::saved_panels(),
            log: Vec::new(),
            bottom_tab: BottomTab::Log,
            console: Default::default(),
            dialog: None,
            theme: theme::light(),
            themes: theme::load_all(),
            theme_editor: None,
            block_editor: None,
            board_editor: None,
            code_view: None,
            help: None,
            block_delete: None,
            sheet_delete: None,
            fs_pending: false,
            fs_fixes: 0,
            port: String::new(),
            ports: Vec::new(),
            port_scan: None,
            debug: None,
            debug_firmware: false,
            ai: Default::default(),
            ai_live: Default::default(),
            job: None,
            pending: None,
            allow_close: false,
            autosave_minutes: theme::autosave_minutes(),
            last_autosave: Instant::now(),
            job_rev: None,
            queued: None,
            precompiled: None,
            seen_rev: None,
            local: LocalValues::default(),
            update_check: theme::update_check(),
            update_beta: theme::update_beta(),
            auto_update: theme::auto_update(),
            update_rx: None,
            update_checked: Instant::now(),
            new_release: None,
            update_window: false,
            changes_window: false,
            mcp: Default::default(),
            ai_conf: Default::default(),
            report: Default::default(),
            import: Default::default(),
            board_import: Default::default(),
            block_diffs: Vec::new(),
            self_update: SelfUpdate::Idle(None),
            cli_prompt: if ArduinoCli::locate().is_ok() {
                CliPrompt::Hidden
            } else {
                CliPrompt::Ask
            },
        };
        match LocalValues::open_default() {
            Ok(local) => app.local = local,
            // Kept in memory only, so the broken file is not overwritten.
            Err(e) => app.info(
                LogLevel::Error,
                crate::i18n::fmt("Сохранённые пароли не прочитаны: {0}", &[&e]),
            ),
        }
        if let Some(t) =
            theme::saved_choice().and_then(|n| app.themes.iter().find(|t| t.name == n).cloned())
        {
            app.theme = t;
        }
        if let Some(lang) = theme::saved_language().and_then(|c| crate::i18n::Lang::from_code(&c)) {
            crate::i18n::set(lang);
        }
        crate::i18n::install_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, &app.theme);
        app.scan_ports(&cc.egui_ctx);
        if let Some(text) = selfupdate::take_last_result() {
            let level = if text == "ok" {
                LogLevel::Info
            } else {
                LogLevel::Error
            };
            let text = if text == "ok" {
                crate::i18n::tr("Программа обновлена").to_owned()
            } else {
                crate::i18n::fmt(
                    "Обновление не удалось, возвращена прежняя версия ({0})",
                    &[&text],
                )
            };
            app.info(level, text);
        }
        if app.update_check {
            app.start_update_check(&cc.egui_ctx, false);
        }
        app.lib_dir = find_library(project.as_deref());
        crate::aiconf::apply(&theme::ai_settings());
        app.reload_library();
        // A block or board file given on the command line goes to the import window.
        let (project, block_file) = match project {
            Some(p) if is_block_file(&p) || crate::boardio::is_board_file(&p) => (None, Some(p)),
            other => (other, None),
        };
        // Start with a project: the given one, the last one or a fresh one.
        let last = theme::last_project().filter(|p| flp_core::project::is_project(p));
        match project.or(last) {
            Some(path) => app.open(&path),
            None => app.create_default_project(),
        }
        if let Some(path) = block_file {
            if crate::boardio::is_board_file(&path) {
                crate::boardio::add_files(&mut app, &[path]);
            } else {
                crate::blockio::add_files(&mut app, &[path]);
            }
        }
        app
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        theme::save_choice(&theme.name);
        theme::apply(ctx, &theme);
        self.theme = theme;
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
            Err(e) => self.info(
                LogLevel::Warning,
                crate::i18n::fmt("Порты не найдены: {0}", &[&e]),
            ),
        }
    }

    /// Asks GitHub for the latest release on a worker thread. A manual check
    /// reports every outcome in the log, an automatic one only news and failures.
    fn start_update_check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.update_rx.is_some() {
            return;
        }
        self.update_checked = Instant::now();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let repo = theme::update_repo();
        let beta = self.update_beta;
        thread::spawn(move || {
            let _ = tx.send((manual, update::check(&repo, beta)));
            ctx.request_repaint();
        });
        self.update_rx = Some(rx);
    }

    /// Offer to install arduino-cli (winget) when it is not found; shows the progress.
    fn cli_window(&mut self, ctx: &egui::Context) {
        match &self.cli_prompt {
            CliPrompt::Hidden => {}
            CliPrompt::Ask => {
                let mut install = false;
                let mut later = false;
                egui::Window::new(crate::i18n::tr("arduino-cli не найден"))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(crate::i18n::tr("Для сборки и заливки прошивки нужна программа arduino-cli."));
                        ui.label(crate::i18n::tr("Установить её сейчас через winget? Ядро ESP32 докачается при первой сборке."));
                        ui.horizontal(|ui| {
                            install = ui.button(crate::i18n::tr("Установить")).clicked();
                            later = ui.button(crate::i18n::tr("Позже")).clicked();
                        });
                    });
                if install {
                    let (tx, rx) = mpsc::channel();
                    let ctx = ctx.clone();
                    thread::spawn(move || {
                        let mut lines = Vec::new();
                        let result = flp_build::install_cli(&mut |l| lines.push(l.to_owned()));
                        let text = lines.join("\n");
                        let _ = tx.send(match result {
                            Ok(()) => Ok(text),
                            Err(e) => Err(format!("{e}\n{text}")),
                        });
                        ctx.request_repaint();
                    });
                    self.info(
                        LogLevel::Info,
                        crate::i18n::tr("Устанавливаю arduino-cli через winget..."),
                    );
                    self.cli_prompt = CliPrompt::Installing(rx);
                } else if later {
                    self.cli_prompt = CliPrompt::Hidden;
                }
            }
            CliPrompt::Installing(rx) => {
                let done = rx.try_recv().ok();
                egui::Window::new(crate::i18n::tr("Установка arduino-cli"))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            panels::busy(ui);
                            ui.label(crate::i18n::tr(
                                "Идёт установка, это может занять минуту...",
                            ));
                        });
                    });
                ctx.request_repaint_after(Duration::from_millis(300));
                if let Some(result) = done {
                    self.cli_prompt = CliPrompt::Hidden;
                    match result {
                        Ok(_) => {
                            self.info(LogLevel::Info, crate::i18n::tr("arduino-cli установлен"));
                            self.scan_ports(ctx);
                        }
                        Err(e) => self.info(
                            LogLevel::Error,
                            crate::i18n::fmt("Не удалось установить arduino-cli: {0}", &[&e]),
                        ),
                    }
                }
            }
        }
    }

    fn start_self_update(&mut self, url: String, auto: bool) {
        let (done, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
        let (tx, rx) = mpsc::channel();
        let (d, t) = (done.clone(), total.clone());
        thread::spawn(move || {
            let _ = tx.send(selfupdate::download_and_stage(&url, &d, &t));
        });
        self.self_update = SelfUpdate::Downloading {
            done,
            total,
            rx,
            auto,
        };
    }

    /// Starts the background download of the release found earlier, if it can be installed.
    fn start_known_release_download(&mut self) {
        let Some(release) = &self.new_release else {
            return;
        };
        if selfupdate::install_dir().is_some() && matches!(self.self_update, SelfUpdate::Idle(_)) {
            let url = release.url.clone();
            self.start_self_update(url, true);
        }
    }

    fn poll_self_update(&mut self, ctx: &egui::Context) {
        let SelfUpdate::Downloading { rx, auto, .. } = &self.self_update else {
            return;
        };
        let auto = *auto;
        ctx.request_repaint_after(Duration::from_millis(200));
        let Ok(result) = rx.try_recv() else { return };
        self.self_update = SelfUpdate::Idle(None);
        let staged = match result {
            Ok(staged) => staged,
            Err(e) => {
                if auto {
                    self.info(
                        LogLevel::Warning,
                        crate::i18n::fmt("Автоскачивание обновления: {0}", &[&e]),
                    );
                }
                self.self_update = SelfUpdate::Idle(Some(e));
                return;
            }
        };
        if auto {
            let version = self.new_release.as_ref().map_or("", |r| r.version.as_str());
            let text = crate::i18n::fmt(
                "Обновление {0} скачано: Справка → «Перезапустить и обновить»",
                &[&version],
            );
            self.info(LogLevel::Info, text);
            self.self_update = SelfUpdate::Ready(staged);
            return;
        }
        self.apply_update(ctx, &staged);
    }

    /// Hands the unpacked release to the helper and closes the editor.
    fn apply_update(&mut self, ctx: &egui::Context, staged: &Path) {
        // The new version opens the project again from its file: save it first.
        if self.doc.as_ref().is_some_and(|d| d.dirty) && !self.save() {
            self.self_update = SelfUpdate::Idle(Some(
                crate::i18n::tr("проект не сохранён, обновление отменено").into(),
            ));
            return;
        }
        let project = self.doc.as_ref().map(|d| d.project.path.clone());
        match selfupdate::launch_helper(staged, project.as_deref()) {
            Ok(()) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => self.self_update = SelfUpdate::Idle(Some(e)),
        }
    }

    fn poll_update(&mut self) {
        let Some(rx) = &self.update_rx else { return };
        let Ok((manual, result)) = rx.try_recv() else {
            return;
        };
        self.update_rx = None;
        match result {
            // A periodic check that finds the version already announced stays silent.
            Ok(Check::Newer(release))
                if !manual
                    && self.new_release.as_ref().map(|r| &r.version) == Some(&release.version) => {}
            Ok(Check::Newer(release)) => {
                // A still newer release than the one downloaded earlier: fetch that one instead.
                if matches!(self.self_update, SelfUpdate::Ready(_)) {
                    self.self_update = SelfUpdate::Idle(None);
                }
                self.info(
                    LogLevel::Info,
                    crate::i18n::fmt(
                        "Доступна версия {0} (установлена {1}): {2}",
                        &[&release.version, &update::CURRENT, &release.url],
                    ),
                );
                self.new_release = Some(release);
                // The window opens by itself only when the user asked for the check.
                self.update_window = manual;
                if self.auto_update {
                    self.start_known_release_download();
                }
            }
            Ok(Check::UpToDate) if manual => self.info(
                LogLevel::Info,
                crate::i18n::fmt("Установлена последняя версия ({0})", &[&update::CURRENT]),
            ),
            Ok(Check::NoReleases) if manual => self.info(
                LogLevel::Info,
                crate::i18n::tr(
                    "Выпущенных версий не найдено (или у токена нет доступа к репозиторию)",
                ),
            ),
            Ok(_) => {}
            Err(e) => self.info(
                LogLevel::Warning,
                crate::i18n::fmt("Проверка обновлений: {0}", &[&e]),
            ),
        }
    }

    /// Writes [`EditorApp::local`] to its file, reporting a failure in the log.
    pub fn save_local(&mut self) {
        if let Err(e) = self.local.save() {
            self.info(
                LogLevel::Error,
                crate::i18n::fmt("Пароли не сохранены: {0}", &[&e]),
            );
        }
    }

    /// Moves Wi-Fi passwords (and other `local` values) out of the open project
    /// into the program; the next save writes the project file without them.
    fn take_local(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        if doc.project.take_local(&self.lib, &mut self.local) {
            doc.dirty = true;
            self.info(
                LogLevel::Info,
                crate::i18n::tr("Пароль Wi-Fi перенесён из файла проекта в настройки программы"),
            );
        }
        self.save_local();
    }

    pub fn info(&mut self, level: LogLevel, text: impl Into<String>) {
        self.log.push(LogLine {
            level,
            text: text.into(),
            block: None,
            help: None,
        });
    }

    /// Folder of user blocks: `user_library/` next to the built-in library.
    pub fn user_lib_dir(&self) -> Option<PathBuf> {
        self.lib_dir.as_deref().map(flp_core::user_library_dir)
    }

    pub fn reload_library(&mut self) {
        let Some(dir) = self.lib_dir.clone() else {
            self.info(
                LogLevel::Error,
                crate::i18n::tr(
                    "Не найдена папка библиотеки блоков library/ (задайте FLP_LIBRARY)",
                ),
            );
            return;
        };
        match Library::load_with_user(&dir) {
            Ok((lib, errors)) => {
                let user = lib.iter().filter(|b| lib.is_user(&b.def.info.kind)).count();
                let builtin = lib.len() - user;
                self.lib = lib;
                if let Some(help) = &mut self.help {
                    help.blocks_dirty = true;
                }
                for e in errors {
                    self.info(LogLevel::Warning, crate::i18n::fmt("Пропущено: {0}", &[&e]));
                }
                self.info(
                    LogLevel::Info,
                    crate::i18n::fmt(
                        "Библиотека: {0} встроенных блоков из {1}, {2} пользовательских",
                        &[&builtin, &dir.display(), &user],
                    ),
                );
            }
            Err(e) => self.info(LogLevel::Error, e.to_string()),
        }
        self.lib_stamp = library_stamp(&dir);
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
        let stamp = library_stamp(dir);
        if stamp != self.lib_stamp {
            self.reload_library();
        }
    }

    fn project_stamp(&self) -> Option<SystemTime> {
        let doc = self.doc.as_ref()?;
        fs::metadata(&doc.project.path)
            .and_then(|m| m.modified())
            .ok()
    }

    /// Picks up changes made to the open project file by other programs. Unsaved work in
    /// the editor is kept (only a warning is logged); otherwise the project is reloaded.
    fn watch_project(&mut self) {
        if self.disk_checked.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.disk_checked = Instant::now();
        let Some(doc) = &self.doc else { return };
        let stamp = self.project_stamp();
        if stamp.is_none() || stamp == self.disk_stamp {
            return;
        }
        if doc.dirty {
            if !self.disk_warned {
                self.disk_warned = true;
                self.info(
                    LogLevel::Warning,
                    crate::i18n::tr("Файл проекта изменён другой программой, а в редакторе есть несохранённые правки:                      сохранение перезапишет внешние изменения"),
                );
            }
            return;
        }
        let path = doc.project.path.clone();
        match Doc::open(&path, &self.lib) {
            Ok(new) => {
                self.doc = Some(new);
                self.disk_stamp = stamp;
                self.selection = None;
                self.info(
                    LogLevel::Info,
                    crate::i18n::tr("Проект обновлён: файл изменён снаружи"),
                );
            }
            Err(e) => {
                self.disk_stamp = stamp;
                self.info(LogLevel::Error, e.to_string());
            }
        }
    }

    /// User blocks packed into the project file but missing on this computer are put into
    /// `user_library/` before the project is checked.
    fn restore_packed_blocks(&mut self, path: &Path) {
        let Some(dir) = self.user_lib_dir() else {
            return;
        };
        if !flp_core::project::is_project_file(path) && !path.is_dir() {
            return;
        }
        // The board goes first: the library has to know it for the blocks of its devices.
        let boards = flp_core::packed::unpack_missing_boards(path, &dir, &self.lib);
        if !boards.is_empty() {
            self.info(
                LogLevel::Info,
                crate::i18n::fmt("Из проекта добавлена плата: {0}", &[&boards.join(", ")]),
            );
            self.reload_library();
        }
        let written = flp_core::packed::unpack_missing_blocks(path, &dir, &self.lib);
        if !written.is_empty() {
            self.info(
                LogLevel::Info,
                crate::i18n::fmt(
                    "Из проекта добавлены пользовательские блоки: {0}",
                    &[&written.join(", ")],
                ),
            );
            self.reload_library();
        }
    }

    /// Offers to sync user blocks whose revision differs between the project and the
    /// program: take the project's newer copy, or save the project with the program's.
    fn block_versions_window(&mut self, ctx: &egui::Context) {
        if self.block_diffs.is_empty() {
            return;
        }
        let mut take: Vec<String> = Vec::new();
        let mut repack = false;
        let mut close = false;
        egui::Window::new(crate::i18n::tr("Версии блоков"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr(
                    "Пользовательские блоки в проекте и в программе отличаются версией.",
                ));
                ui.add_space(6.0);
                let (newer_project, newer_program): (Vec<_>, Vec<_>) = self
                    .block_diffs
                    .iter()
                    .filter(|d| !d.diverged())
                    .partition(|d| d.project > d.program);
                let diverged: Vec<_> = self.block_diffs.iter().filter(|d| d.diverged()).collect();
                if !diverged.is_empty() {
                    ui.strong(crate::i18n::tr(
                        "Версия одна, а содержимое разное (правили на разных компьютерах)",
                    ));
                    for d in &diverged {
                        ui.label(format!("{} (v{})", d.kind, d.project));
                    }
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::tr("Взять из проекта")).clicked() {
                            take.extend(diverged.iter().map(|d| d.kind.clone()));
                        }
                        if ui
                            .button(crate::i18n::tr(
                                "Оставить блоки программы и сохранить проект",
                            ))
                            .clicked()
                        {
                            repack = true;
                        }
                    });
                    ui.add_space(6.0);
                }
                if !newer_project.is_empty() {
                    ui.strong(crate::i18n::tr("В проекте блоки новее, чем в программе"));
                    for d in &newer_project {
                        ui.label(crate::i18n::fmt(
                            "{0}: в проекте v{1}, в программе v{2}",
                            &[&d.kind, &d.project, &d.program],
                        ));
                    }
                    if ui
                        .button(crate::i18n::tr("Обновить блоки программы из проекта"))
                        .clicked()
                    {
                        take = newer_project.iter().map(|d| d.kind.clone()).collect();
                    }
                    ui.add_space(6.0);
                }
                if !newer_program.is_empty() {
                    ui.strong(crate::i18n::tr("В программе блоки новее, чем в проекте"));
                    for d in &newer_program {
                        ui.label(crate::i18n::fmt(
                            "{0}: в проекте v{1}, в программе v{2}",
                            &[&d.kind, &d.project, &d.program],
                        ));
                    }
                    if ui
                        .button(crate::i18n::tr("Обновить блоки проекта из программы"))
                        .clicked()
                    {
                        repack = true;
                    }
                    ui.add_space(6.0);
                }
                if ui.button(crate::i18n::tr("Оставить как есть")).clicked() {
                    close = true;
                }
            });
        if !take.is_empty() {
            if let (Some(dir), Some(doc)) = (self.user_lib_dir(), &self.doc) {
                let written =
                    flp_core::packed::adopt_project_blocks(&doc.project.path, &dir, &take);
                self.info(
                    LogLevel::Ok,
                    crate::i18n::fmt(
                        "Блоки программы обновлены из проекта: {0}",
                        &[&written.join(", ")],
                    ),
                );
                self.reload_library();
            }
            self.block_diffs.retain(|d| !take.contains(&d.kind));
        }
        if repack {
            // Saving packs the program's copies of the blocks the schema uses.
            if self.save() {
                self.info(
                    LogLevel::Ok,
                    crate::i18n::tr("Блоки проекта обновлены из программы"),
                );
            }
            self.block_diffs
                .retain(|d| d.program <= d.project && !d.diverged());
        }
        if close {
            self.block_diffs.clear();
        }
    }

    pub(crate) fn request_open(&mut self, path: PathBuf) {
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
        self.restore_packed_blocks(path);
        self.block_diffs = if flp_core::project::is_project_file(path) {
            flp_core::packed::version_diffs(path, &self.lib)
        } else {
            Vec::new()
        };
        match Doc::open(path, &self.lib) {
            Ok(doc) => {
                self.info(
                    LogLevel::Info,
                    crate::i18n::fmt("Открыт проект {0}", &[&path.display()]),
                );
                theme::save_last_project(path);
                self.port = doc.project.config.board.port.clone().unwrap_or_default();
                self.doc = Some(doc);
                self.disk_stamp = self.project_stamp();
                self.disk_warned = false;
                self.selection = None;
                self.sheet = 1;
                self.extra_sheets.clear();
                self.canvas = CanvasState::default();
                self.take_local();
            }
            Err(e) => self.info(LogLevel::Error, e.to_string()),
        }
    }

    /// Opens an example from a copy in the temp folder, so edits and saves never
    /// touch the files in `examples/` (they live in git).
    fn open_example(&mut self, name: &str, src: &Path) {
        let dir = std::env::temp_dir().join("flp_examples");
        let dest = dir.join(format!("{name}.crab"));
        let copied = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                if flp_core::project::is_project_file(src) {
                    std::fs::copy(src, &dest)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                } else {
                    let mut project =
                        flp_core::project::Project::load(src).map_err(|e| e.to_string())?;
                    project.path = dest.clone();
                    project.save().map_err(|e| e.to_string())
                }
            });
        match copied {
            Ok(()) => self.request_open(dest),
            Err(e) => self.info(
                LogLevel::Error,
                crate::i18n::fmt("Пример не открыт: {0}", &[&e]),
            ),
        }
    }

    /// Projects from `examples/` next to the library: `(name, path)`, a `.crab` wins over a folder of the same name.
    fn example_projects(&self) -> Vec<(String, PathBuf)> {
        let Some(dir) = self
            .lib_dir
            .as_deref()
            .and_then(Path::parent)
            .map(|p| p.join("examples"))
        else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut found: std::collections::BTreeMap<String, PathBuf> = Default::default();
        for path in entries.flatten().map(|e| e.path()) {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else {
                continue;
            };
            if flp_core::project::is_project_file(&path) {
                found.insert(name, path);
            } else if path.join("project.toml").is_file() {
                found.entry(name).or_insert(path);
            }
        }
        found.into_iter().collect()
    }

    fn new_project(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title(crate::i18n::tr("Новый проект"))
            .add_filter(crate::i18n::tr("Проект CrabLogic"), &[PROJECT_EXT])
            .set_file_name(format!("Новый проект.{PROJECT_EXT}"));
        if let Some(dir) = theme::default_projects_dir().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else {
            return;
        };
        if !flp_core::project::is_project_file(&path) {
            path.set_extension(PROJECT_EXT);
        }
        if path.exists() {
            self.request_open(path);
            return;
        }
        self.create_project(&path);
    }

    /// Asks for a project file; an old-format project is opened by its `project.toml`.
    fn pick_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(crate::i18n::tr("Открыть проект"))
            .add_filter(
                crate::i18n::tr("Проект CrabLogic"),
                &[PROJECT_EXT, flp_core::project::LEGACY_PROJECT_EXT],
            )
            .add_filter(
                crate::i18n::tr("Проект старого формата (project.toml)"),
                &["toml"],
            )
            .pick_file()
        else {
            return;
        };
        let path = match path.parent() {
            Some(dir) if !flp_core::project::is_project_file(&path) => dir.to_path_buf(),
            _ => path,
        };
        self.request_open(path);
    }

    /// Creates "Новый проект N.crab" in the default projects folder and opens it.
    fn create_default_project(&mut self) {
        let Some(base) = theme::default_projects_dir() else {
            return;
        };
        if let Err(e) = fs::create_dir_all(&base) {
            self.info(LogLevel::Error, format!("{}: {e}", base.display()));
            return;
        }
        let path = (1..)
            .map(|n| base.join(format!("Новый проект {n}.{PROJECT_EXT}")))
            .find(|p| !p.exists())
            .expect("unbounded range");
        self.create_project(&path);
    }

    /// Writes an empty ESP32 project to `path` and opens it.
    fn create_project(&mut self, path: &Path) {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_owned();
        let project = Project {
            path: path.to_owned(),
            config: ProjectConfig {
                project: ProjectInfo {
                    name,
                    description: String::new(),
                },
                board: BoardConfig {
                    fqbn: "esp32:esp32:esp32".into(),
                    model: None,
                    port: None,
                    options: Default::default(),
                },
                diagnostics: Default::default(),
                uart: Vec::new(),
            },
            logic: Default::default(),
            layout: Default::default(),
        };
        if let Err(e) = flp_pdf::save_project(&project, &self.lib) {
            self.info(LogLevel::Error, e.to_string());
            return;
        }
        self.open(path);
    }

    /// Starts the web interface constructor (`crablogic-web`) for the open project and shows it
    /// in the browser. The project is saved first: the constructor edits the file itself.
    pub fn open_web_designer(&mut self) {
        let Some(doc) = &self.doc else { return };
        let path = doc.project.path.clone();
        if doc.dirty {
            self.save();
        }
        let running = self
            .web_editor
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if !running {
            let sibling = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("crablogic-web.exe")))
                .filter(|p| p.is_file());
            let mut cmd =
                std::process::Command::new(sibling.unwrap_or_else(|| "crablogic-web".into()));
            cmd.arg(&path);
            if let Some(dir) = &self.lib_dir {
                cmd.arg("--library").arg(dir);
            }
            hide_console(&mut cmd);
            match cmd.spawn() {
                Ok(child) => self.web_editor = Some(child),
                Err(e) => {
                    self.info(
                        LogLevel::Error,
                        crate::i18n::fmt(
                            "Не удалось запустить crablogic-web (соберите: cargo build -p flp-web): {0}",
                            &[&e],
                        ),
                    );
                    return;
                }
            }
        }
        // The server needs a moment to start listening; the browser retries on its own.
        std::thread::sleep(Duration::from_millis(500));
        let mut open = std::process::Command::new("cmd");
        open.args(["/C", "start", "", "http://127.0.0.1:8080/designer"]);
        hide_console(&mut open);
        match open.spawn() {
            Ok(_) => self.info(
                LogLevel::Info,
                crate::i18n::tr("Конструктор веб-интерфейса открыт в браузере."),
            ),
            Err(e) => self.info(
                LogLevel::Error,
                crate::i18n::fmt("Не удалось открыть браузер: {0}", &[&e]),
            ),
        }
    }

    /// Saves the open project; `false` if it could not be saved.
    pub fn save(&mut self) -> bool {
        let Some(doc) = &mut self.doc else {
            return false;
        };
        match doc.save(&self.lib) {
            Ok(warning) => {
                let path = doc.project.path.display().to_string();
                self.info(LogLevel::Ok, crate::i18n::fmt("Сохранено: {0}", &[&path]));
                if let Some(w) = warning {
                    self.info(LogLevel::Warning, w);
                }
                self.disk_stamp = self.project_stamp();
                self.disk_warned = false;
                true
            }
            Err(e) => {
                self.info(LogLevel::Error, e.to_string());
                false
            }
        }
    }

    /// Asks for a new file and saves the project there; the editor then works with
    /// the new file. Also converts an old-format project folder into a `.crab`.
    pub fn save_as(&mut self) {
        let Some(doc) = &self.doc else { return };
        let old = doc.project.path.clone();
        let name = doc.project.config.project.name.clone();
        let mut dialog = rfd::FileDialog::new()
            .set_title(crate::i18n::tr("Сохранить проект как"))
            .add_filter(crate::i18n::tr("Проект CrabLogic"), &[PROJECT_EXT]);
        dialog = match old.parent().filter(|d| d.is_dir()) {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        };
        let file = old
            .file_stem()
            .and_then(|n| n.to_str())
            .map_or(name, str::to_owned);
        let Some(mut path) = dialog
            .set_file_name(format!("{file}.{PROJECT_EXT}"))
            .save_file()
        else {
            return;
        };
        if !flp_core::project::is_project_file(&path) {
            path.set_extension(PROJECT_EXT);
        }
        if let Some(doc) = &mut self.doc {
            doc.project.path = path.clone();
        }
        if self.save() {
            theme::save_last_project(&path);
        } else if let Some(doc) = &mut self.doc {
            // Saving failed: stay with the previous file.
            doc.project.path = old;
        }
    }

    /// Asks for a file and writes the picture of all sheets of the project there.
    fn export_pdf(&mut self) {
        let Some(doc) = &self.doc else { return };
        let path = &doc.project.path;
        let stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("boards")
            .to_owned();
        let mut dialog = rfd::FileDialog::new()
            .set_title(crate::i18n::tr("Сохранить PDF"))
            .add_filter("PDF", &["pdf"])
            .set_file_name(format!("{stem}.pdf"));
        if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut dest) = dialog.save_file() else {
            return;
        };
        if dest.extension().is_none() {
            dest.set_extension("pdf");
        }
        let result = flp_pdf::render(&doc.project, &self.lib)
            .map_err(|e| e.to_string())
            .and_then(|pdf| std::fs::write(&dest, pdf).map_err(|e| e.to_string()));
        match result {
            Ok(()) => self.info(
                LogLevel::Ok,
                crate::i18n::fmt("PDF сохранён: {0}", &[&dest.display()]),
            ),
            Err(e) => self.info(
                LogLevel::Error,
                crate::i18n::fmt("Не удалось сохранить PDF: {0}", &[&e]),
            ),
        }
    }

    /// Saves a changed project once the autosave period has passed.
    fn autosave(&mut self, ctx: &egui::Context) {
        if self.autosave_minutes == 0 {
            return;
        }
        let period = Duration::from_secs(u64::from(self.autosave_minutes) * 60);
        let elapsed = self.last_autosave.elapsed();
        if elapsed < period {
            ctx.request_repaint_after(period - elapsed);
            return;
        }
        self.last_autosave = Instant::now();
        ctx.request_repaint_after(period);
        if self.doc.as_ref().is_some_and(|d| d.dirty) {
            self.save();
        }
    }

    pub fn set_autosave(&mut self, minutes: u32) {
        self.autosave_minutes = minutes;
        self.last_autosave = Instant::now();
        theme::save_autosave_minutes(minutes);
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
            Selection::Block(_) | Selection::Blocks(_) => doc
                .edit(&self.lib, None, |p| {
                    // Copies go first: removing a tag folds its first copy into it.
                    let mut ids = sel.blocks().to_vec();
                    ids.sort_by_key(|id| p.logic.block(id).is_none_or(|b| b.tag.is_none()));
                    // An input or output leaves the sheet but stays in the project tree.
                    ids.iter().try_for_each(|id| {
                        if p.is_tag(&self.lib, p.logic.tag_of(id)) {
                            p.take_off_board(id)
                        } else {
                            p.remove_block(id)
                        }
                    })?;
                    p.release_unused_services(&self.lib)
                })
                .map_err(|e| e.to_string()),
            Selection::Link(to) => doc.edit(&self.lib, None, |p| {
                if p.disconnect(to) {
                    Ok(())
                } else {
                    Err(crate::i18n::tr("связь не найдена").to_owned())
                }
            }),
        };
        if let Err(e) = result {
            self.info(LogLevel::Error, e);
        }
    }

    /// Puts the selected blocks on the system clipboard as JSON, so they can be
    /// pasted in another editor window or project.
    pub fn copy_selection(&mut self, ctx: &egui::Context) {
        let (Some(doc), Some(sel)) = (&self.doc, &self.selection) else {
            return;
        };
        if sel.blocks().is_empty() {
            return;
        }
        let frag = doc.project.copy_fragment(sel.blocks());
        let n = frag.blocks.len();
        ctx.copy_text(frag.to_clipboard());
        self.info(
            LogLevel::Info,
            crate::i18n::fmt("Скопировано блоков: {0}", &[&n]),
        );
    }

    /// Copies the selected blocks to the clipboard and removes them.
    pub fn cut_selection(&mut self, ctx: &egui::Context) {
        if self
            .selection
            .as_ref()
            .is_none_or(|s| s.blocks().is_empty())
        {
            return;
        }
        self.copy_selection(ctx);
        self.delete_selection();
    }

    /// Inserts blocks from clipboard text at `at` (or a free spot on the active sheet).
    pub fn paste(&mut self, text: &str, at: Option<BlockPos>) {
        let Some(frag) = Fragment::from_clipboard(text) else {
            return;
        };
        let Some(doc) = &mut self.doc else { return };
        let at = at.unwrap_or_else(|| doc.project.free_position(self.sheet));
        let lib = self.lib.clone();
        match doc.edit(&lib, None, |p| p.paste_fragment(&lib, &frag, at)) {
            Ok(ids) => {
                self.sheet = at.sheet;
                self.selection = Selection::of_blocks(ids);
            }
            Err(e) => self.info(LogLevel::Error, crate::i18n::fmt("Вставка: {0}", &[&e])),
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
                help: Some(format!(
                    "{}#{}",
                    crate::help::ERRORS,
                    crate::help::error_anchor(d.kind)
                )),
            });
        }
        let errors = doc.report.errors().count();
        self.log.extend(lines);
        if errors == 0 {
            self.info(LogLevel::Ok, crate::i18n::tr("Проверка: ошибок нет"));
        } else {
            self.info(
                LogLevel::Error,
                crate::i18n::fmt("Проверка: ошибок — {0}", &[&errors]),
            );
        }
    }

    pub fn start_job(&mut self, kind: JobKind, ctx: &egui::Context) {
        if let Some(job) = &self.job {
            // The user's build waits for precompilation, which shares the build folder.
            // It is raised to normal priority: the user waits for it now.
            if job.background && !matches!(kind, JobKind::Precompile) && self.queued.is_none() {
                job.raise_priority();
                self.info(
                    LogLevel::Info,
                    crate::i18n::tr("Ждём окончания фоновой сборки..."),
                );
                self.queued = Some(kind);
            }
            return;
        }
        // The upload needs the serial port that debugging or the monitor holds.
        if matches!(kind, JobKind::Upload { .. }) {
            self.stop_debug();
            self.console.release();
        }
        let Some(doc) = &self.doc else { return };
        if doc.report.has_errors() {
            self.run_check();
            self.info(
                LogLevel::Error,
                crate::i18n::tr("Сначала исправьте ошибки схемы"),
            );
            return;
        }
        self.job_rev = Some((doc.project.path.clone(), doc.revision, self.debug_firmware));
        let ctx = ctx.clone();
        let job = Job::start(
            kind,
            self.debug_firmware,
            doc.project.with_local(&self.lib, &self.local),
            self.lib.clone(),
            move || ctx.request_repaint(),
        );
        if !job.background {
            self.info(LogLevel::Info, format!("{}...", job.title));
        }
        self.job = Some(job);
    }

    /// Cancels the running job together with the queued one.
    pub fn cancel_job(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
            self.queued = None;
        }
    }

    /// Any job runs, background precompilation included.
    pub fn has_job(&self) -> bool {
        self.job.is_some()
    }

    /// A foreground job runs; precompilation does not block build buttons.
    pub fn job_running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.background) || self.queued.is_some()
    }

    /// Starts precompilation once the schema has been unchanged for a while.
    fn precompile(&mut self, ctx: &egui::Context) {
        const PAUSE: Duration = Duration::from_secs(3);
        let Some(doc) = &self.doc else { return };
        let rev = doc.revision;
        let now = Instant::now();
        match self.seen_rev {
            Some((seen, _)) if seen == rev => {}
            _ => self.seen_rev = Some((rev, now)),
        }
        let current = Some((doc.project.path.clone(), rev, self.debug_firmware));
        let quiet = self.seen_rev.is_some_and(|(_, t)| now - t >= PAUSE);
        if self.job.is_some()
            || self.debug.is_some()
            || !quiet
            || doc.report.has_errors()
            || self.precompiled == current
        {
            return;
        }
        // Mark it tried even if the build fails, so a broken sketch is not rebuilt in a loop.
        self.precompiled = current;
        self.start_job(JobKind::Precompile, ctx);
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let background = job.background;
        let cancelled = job.is_cancelled();
        let title = job.title;
        let mut done = false;
        let mut connect = None;
        for event in job.poll() {
            match event {
                JobEvent::Line(..) if background || cancelled => {}
                JobEvent::Done(Err(_)) if cancelled => {
                    self.info(
                        LogLevel::Warning,
                        crate::i18n::fmt("{0}: отменено", &[&title]),
                    );
                    done = true;
                }
                JobEvent::Line(level, text) => self.info(level, text),
                JobEvent::Done(Ok(_)) if background => done = true,
                JobEvent::Done(Err(_)) if background => {
                    self.info(
                        LogLevel::Warning,
                        crate::i18n::tr("Фоновая сборка не удалась; подробности покажет «Собрать»"),
                    );
                    done = true;
                }
                JobEvent::Done(Ok(result)) => {
                    self.precompiled = self.job_rev.clone();
                    self.info(LogLevel::Ok, result.message);
                    if let (true, Some(port)) = (result.debug, result.port) {
                        connect = Some(port);
                    }
                    done = true;
                }
                JobEvent::Done(Err(text)) => {
                    self.info(LogLevel::Error, crate::i18n::fmt("ошибка: {0}", &[&text]));
                    done = true;
                }
            }
        }
        if done {
            self.job = None;
            if let Some(kind) = self.queued.take() {
                self.start_job(kind, ctx);
            }
        }
        if let Some(port) = connect {
            self.connect_debug(Some(port), ctx);
        }
    }

    /// Builds and uploads firmware with the debugging module, then connects.
    pub fn start_debug(&mut self, ctx: &egui::Context) {
        self.debug_firmware = true;
        let port = self.port_choice();
        self.start_job(JobKind::Upload { port }, ctx);
    }

    /// Connects to a board that already runs debugging firmware.
    pub fn connect_debug(&mut self, port: Option<String>, ctx: &egui::Context) {
        self.stop_debug();
        self.console.release();
        let Some(doc) = &self.doc else { return };
        let Some(port) = port.or_else(|| self.board_port()) else {
            self.info(
                LogLevel::Error,
                crate::i18n::tr("Выберите порт платы на панели инструментов"),
            );
            return;
        };
        match DebugState::start(&doc.project, &self.lib, &port, ctx) {
            Ok(state) => {
                self.info(
                    LogLevel::Info,
                    crate::i18n::fmt("Отладка: подключение к {0}", &[&port]),
                );
                self.debug = Some(state);
            }
            Err(e) => self.info(LogLevel::Error, e),
        }
    }

    /// Port of the board: chosen in the toolbar, set in the project, or detected.
    fn board_port(&self) -> Option<String> {
        self.port_choice()
            .or_else(|| self.doc.as_ref()?.project.config.board.port.clone())
            .or_else(|| flp_build::auto_port(&self.ports).ok())
    }

    /// Keeps the port monitor in step with debugging and uploads.
    fn poll_console(&mut self, ctx: &egui::Context) {
        if self.console.hold && !self.ai.busy() {
            self.console.hold = false;
        }
        let busy = self.console.hold || self.job.as_ref().is_some_and(|j| j.uses_port);
        let port = self.board_port();
        self.console.debug_reader = self.doc.as_ref().is_some_and(|doc| {
            flp_core::uart::block_uarts(&doc.project.logic, &self.lib)
                .iter()
                .any(|u| u.number == flp_core::uart::USB && u.access == flp_core::UartAccess::Read)
        });
        let live = self.debug.as_ref().map(|d| &d.live);
        self.console.sync(live, port, busy, ctx);
        if let Some(d) = &self.debug {
            for bytes in self.console.take_debug_input() {
                d.send(flp_debug::Command::TextIn(bytes));
            }
        }
        if self.bottom_tab == BottomTab::Console {
            self.console.unread = 0;
        }
    }

    pub fn stop_debug(&mut self) {
        if let Some(d) = self.debug.take() {
            d.stop();
            self.info(LogLevel::Info, crate::i18n::tr("Отладка остановлена"));
        }
    }

    pub fn send_ai(&mut self, ctx: &egui::Context) {
        let Some(doc) = &self.doc else { return };
        // MCP is on: the request goes to the external client, not to the API.
        if let Some(endpoint) = self.mcp.endpoint() {
            let env = crate::aiconf::claude_code_env(&theme::ai_settings());
            self.ai.send_external(endpoint, env, ctx);
            return;
        }
        self.ai.send(
            &doc.project,
            &self.lib,
            self.lib_dir.clone(),
            self.ai_live.clone(),
            ctx,
        );
    }

    fn poll_ai(&mut self) {
        for request in self.ai.poll() {
            match request {
                crate::ai::Request::Apply(logic, layout) => {
                    let lib = self.lib.clone();
                    if let Some(doc) = &mut self.doc {
                        let _ = doc.edit(&lib, None, |p| {
                            p.logic = logic;
                            p.layout = layout;
                            Ok::<_, String>(())
                        });
                    }
                    // A password set by AI goes to the program, not into the file.
                    self.take_local();
                }
                crate::ai::Request::StopDebug => {
                    // The AI is about to upload: free the port until it finishes.
                    self.stop_debug();
                    self.console.release();
                    self.console.hold = true;
                }
                crate::ai::Request::ReloadLibrary => self.reload_library(),
            }
        }
    }

    fn poll_debug(&mut self) {
        let Some(d) = &mut self.debug else {
            *self.ai_live.lock().expect("live values") = None;
            return;
        };
        d.refresh();
        // Publish values for the AI assistant.
        let published = d.online().then(|| {
            (0..d.table.entries.len())
                .map(|i| {
                    let v = d.live.values[i].map_or("—".to_owned(), |v| v.to_string());
                    (d.name(i), v, d.live.forced[i].is_some())
                })
                .collect()
        });
        *self.ai_live.lock().expect("live values") = published;
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

    /// Opens the help window on `target` (`page` or `page#anchor`).
    pub fn open_help(&mut self, target: &str) {
        match &mut self.help {
            Some(help) => help.goto(target),
            None => self.help = Some(crate::help::HelpWindow::new(&self.lib, target)),
        }
    }

    /// Help page of the selected block, or the start page without one.
    fn help_target(&self) -> String {
        let kind = match (&self.selection, &self.doc) {
            (Some(Selection::Block(id)), Some(doc)) => doc
                .project
                .logic
                .blocks
                .iter()
                .find(|b| b.id == *id)
                .map(|b| b.kind.as_str()),
            _ => None,
        };
        match kind.and_then(|k| self.lib.get(k)) {
            Some(block) if !block.def.info.hidden => {
                format!("{}/{}", crate::help::BLOCKS, block.def.info.kind)
            }
            _ => crate::help::START.to_owned(),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        // The shortcut with Shift first: `COMMAND` alone would also match it.
        if pressed(shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
            self.save_as();
        }
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
        if pressed(shortcut(Modifiers::NONE, Key::F1)) {
            let target = self.help_target();
            self.open_help(&target);
        }
        if pressed(shortcut(Modifiers::NONE, Key::F11)) {
            self.toggle_fullscreen(ctx);
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
        if self.dialog.is_none() && !ctx.egui_wants_keyboard_input() {
            let (copy, cut, paste) = ctx.input(|i| {
                let mut copy = false;
                let mut cut = false;
                let mut paste = None;
                for e in &i.events {
                    match e {
                        egui::Event::Copy => copy = true,
                        egui::Event::Cut => cut = true,
                        egui::Event::Paste(t) => paste = Some(t.clone()),
                        _ => {}
                    }
                }
                (copy, cut, paste)
            });
            if cut {
                self.cut_selection(ctx);
            } else if copy {
                self.copy_selection(ctx);
            }
            if let Some(text) = paste {
                let at = self.canvas.paste_pos();
                self.paste(&text, at);
            }
        }
    }

    /// Custom window title bar: icon, burger with the editor menus, draggable title and
    /// window buttons (the OS decorations are off).
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (maximized, fullscreen, inner, monitor) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.maximized.unwrap_or(false),
                v.fullscreen.unwrap_or(false),
                v.inner_rect.map(|r| r.size()),
                v.monitor_size,
            )
        });
        self.fix_fullscreen(&ctx, maximized, fullscreen, inner, monitor);
        // Loaded outside `data_mut`: `load_texture` takes the context lock too.
        let icon_id = egui::Id::unique("app_icon");
        let icon = match ctx.data(|d| d.get_temp::<egui::TextureHandle>(icon_id)) {
            Some(icon) => icon,
            None => {
                let icon = ctx.load_texture(
                    "app_icon",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [64, 64],
                        include_bytes!("../assets/icon64.rgba"),
                    ),
                    egui::TextureOptions::LINEAR,
                );
                ctx.data_mut(|d| d.insert_temp(icon_id, icon.clone()));
                icon
            }
        };
        ui.add_space(6.0);
        // The row is as tall as the port selector from the start, so every item is
        // centred on the same line (a taller item would only lower those after it).
        let row_height = ui.spacing().interact_size.y;
        ui.spacing_mut().interact_size.y = 34.0;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(24.0, 24.0)));
            // No `MenuBar` here: it takes the full row width and pushes the rest off screen.
            // The burger is drawn with lines, so it does not depend on font glyphs.
            let burger = ui
                .allocate_response(egui::vec2(36.0, 30.0), egui::Sense::click())
                .on_hover_text(crate::i18n::tr("Меню"));
            let color = if burger.hovered() || burger.is_pointer_button_down_on() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color()
            };
            if burger.hovered() {
                ui.painter().rect_filled(
                    burger.rect,
                    3.0,
                    ui.visuals().widgets.hovered.weak_bg_fill,
                );
            }
            for dy in [-6.0, 0.0, 6.0] {
                let y = burger.rect.center().y + dy;
                ui.painter().line_segment(
                    [
                        egui::pos2(burger.rect.center().x - 9.0, y),
                        egui::pos2(burger.rect.center().x + 9.0, y),
                    ],
                    egui::Stroke::new(2.0, color),
                );
            }
            egui::Popup::menu(&burger).show(|ui| self.menus(ui));
            panels::toolbar(self, ui);
            // The rest of the row drags the window; double click maximizes.
            // The program name sits at its right end, next to the window buttons.
            let width = (ui.available_width() - 3.0 * 44.0 - 14.0).max(0.0);
            let (rect, drag) = ui.allocate_exact_size(
                egui::vec2(width, ui.spacing().interact_size.y.max(30.0)),
                egui::Sense::click_and_drag(),
            );
            let font = egui::TextStyle::Body.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(
                "CrabLogic".to_owned(),
                font,
                ui.visuals().weak_text_color(),
            );
            if galley.size().x + 12.0 <= rect.width() {
                ui.painter().galley(
                    egui::pos2(
                        rect.right() - 8.0 - galley.size().x,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    ui.visuals().weak_text_color(),
                );
            }
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() && !fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if !fullscreen {
                if panels::window_button(ui, panels::WinIcon::Minimize, crate::i18n::tr("Свернуть"))
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                let (icon, tip) = if maximized {
                    (panels::WinIcon::Restore, crate::i18n::tr("Восстановить"))
                } else {
                    (panels::WinIcon::Maximize, crate::i18n::tr("Развернуть"))
                };
                if panels::window_button(ui, icon, tip) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
            } else if panels::window_button(
                ui,
                panels::WinIcon::ExitFullscreen,
                crate::i18n::tr("Выйти из полного экрана (F11)"),
            ) {
                self.toggle_fullscreen(&ctx);
            }
            if panels::window_button(ui, panels::WinIcon::Close, crate::i18n::tr("Закрыть"))
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.spacing_mut().interact_size.y = row_height;
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        let (on, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (v.fullscreen.unwrap_or(false), v.maximized.unwrap_or(false))
        });
        if on {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        } else if maximized {
            // A maximized undecorated window keeps the work area (the taskbar stays
            // visible) when made full screen: leave the maximized state first.
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            self.fs_pending = true;
            ctx.request_repaint();
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
    }

    /// Finishes entering full screen after the window left the maximized state; if the
    /// window still does not cover the monitor, resizes it to the monitor.
    fn fix_fullscreen(
        &mut self,
        ctx: &egui::Context,
        maximized: bool,
        fullscreen: bool,
        inner: Option<egui::Vec2>,
        monitor: Option<egui::Vec2>,
    ) {
        if self.fs_pending && !maximized {
            self.fs_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        if !fullscreen {
            self.fs_fixes = 0;
            return;
        }
        if let (Some(inner), Some(monitor)) = (inner, monitor) {
            let gap = (inner.x - monitor.x).abs() > 2.0 || (inner.y - monitor.y).abs() > 2.0;
            if gap && self.fs_fixes < 2 {
                self.fs_fixes += 1;
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(monitor));
            }
        }
    }

    /// The editor menus; shown inside the burger button of the title bar.
    fn menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(crate::i18n::tr("Файл"), |ui| {
            if ui.button(crate::i18n::tr("Новый проект...")).clicked() {
                self.new_project();
            }
            if ui.button(crate::i18n::tr("Открыть...")).clicked() {
                self.pick_project();
            }
            let has_doc = self.doc.is_some();
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(crate::i18n::tr("Сохранить")).shortcut_text("Ctrl+S"),
                )
                .clicked()
            {
                self.save();
            }
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(crate::i18n::tr("Сохранить как...")).shortcut_text("Ctrl+Shift+S"),
                )
                .clicked()
            {
                self.save_as();
            }
            if ui
                .add_enabled(has_doc, egui::Button::new(crate::i18n::tr("Сохранить PDF...")))
                .on_hover_text(crate::i18n::tr("Схема всех плат и таблицы параметров блоков"))
                .clicked()
            {
                self.export_pdf();
            }
            if ui
                .button(crate::i18n::tr("Открывать .crab двойным щелчком"))
                .on_hover_text(crate::i18n::tr("Связать файлы проектов, наборов блоков (.crabblocks) и плат (.crabboard) с этим редактором в Проводнике"))
                .clicked()
            {
                match crate::assoc::register() {
                    Ok(()) => self.info(
                        LogLevel::Ok,
                        crate::i18n::tr("Файлы .crab теперь открываются этим редактором"),
                    ),
                    Err(e) => {
                        self.info(LogLevel::Error, crate::i18n::fmt("Не удалось связать .crab: {0}", &[&e]))
                    }
                }
            }
            ui.menu_button(crate::i18n::tr("Автосохранение"), |ui| {
                let mut minutes = self.autosave_minutes;
                ui.radio_value(&mut minutes, 0, crate::i18n::tr("Выключено"));
                for m in [1, 2, 5, 10, 15, 30] {
                    ui.radio_value(&mut minutes, m, crate::i18n::fmt("Каждые {0} мин", &[&m]));
                }
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Период:"));
                    ui.add(
                        egui::DragValue::new(&mut minutes)
                            .range(0..=240)
                            .suffix(crate::i18n::tr(" мин")),
                    );
                });
                if minutes != self.autosave_minutes {
                    self.set_autosave(minutes);
                }
            });
            ui.separator();
            if ui.button(crate::i18n::tr("Выход")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button(crate::i18n::tr("Примеры"), |ui| {
            let examples = self.example_projects();
            if examples.is_empty() {
                ui.label(crate::i18n::tr("Папка examples не найдена"));
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for (name, path) in examples {
                        if ui.button(&name).clicked() {
                            self.open_example(&name, &path);
                            ui.close();
                        }
                    }
                });
        });
        ui.menu_button(crate::i18n::tr("Вид"), |ui| {
            let on = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
            if ui
                .add(
                    egui::Button::new(crate::i18n::tr("На весь экран"))
                        .selected(on)
                        .shortcut_text("F11"),
                )
                .clicked()
            {
                self.toggle_fullscreen(ui.ctx());
                ui.close();
            }
            ui.separator();
            panels::theme_menu(self, ui);
            ui.separator();
            ui.menu_button("Language / Язык", |ui| {
                for lang in crate::i18n::Lang::ALL {
                    let on = crate::i18n::current() == lang;
                    if ui.radio(on, lang.native_name()).clicked() {
                        crate::i18n::set(lang);
                        theme::save_language(lang.code());
                        ui.close();
                    }
                }
            });
        });
        ui.menu_button(crate::i18n::tr("Правка"), |ui| {
            let (can_undo, can_redo) = self
                .doc
                .as_ref()
                .map_or((false, false), |d| (d.can_undo(), d.can_redo()));
            if ui
                .add_enabled(
                    can_undo,
                    egui::Button::new(crate::i18n::tr("Отменить")).shortcut_text("Ctrl+Z"),
                )
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(
                    can_redo,
                    egui::Button::new(crate::i18n::tr("Повторить")).shortcut_text("Ctrl+Y"),
                )
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            if ui
                .add_enabled(
                    self.selection.is_some(),
                    egui::Button::new(crate::i18n::tr("Удалить")).shortcut_text("Del"),
                )
                .clicked()
            {
                self.delete_selection();
            }
            let has_blocks = self
                .selection
                .as_ref()
                .is_some_and(|s| !s.blocks().is_empty());
            if ui
                .add_enabled(
                    has_blocks,
                    egui::Button::new(crate::i18n::tr("Вырезать")).shortcut_text("Ctrl+X"),
                )
                .clicked()
            {
                self.cut_selection(ui.ctx());
            }
            if ui
                .add_enabled(
                    has_blocks,
                    egui::Button::new(crate::i18n::tr("Копировать")).shortcut_text("Ctrl+C"),
                )
                .clicked()
            {
                self.copy_selection(ui.ctx());
            }
            if ui
                .add_enabled(
                    self.doc.is_some(),
                    egui::Button::new(crate::i18n::tr("Вставить")).shortcut_text("Ctrl+V"),
                )
                .clicked()
            {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
        });
        ui.menu_button(crate::i18n::tr("Проект"), |ui| {
            let has_doc = self.doc.is_some();
            let idle = !self.job_running();
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(crate::i18n::tr("Настройки МК...")),
                )
                .clicked()
            {
                self.open_board_settings(ui.ctx());
            }
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(crate::i18n::tr("Конструктор веб-интерфейса...")),
                )
                .on_hover_text(crate::i18n::tr(
                    "Страница управления на ESP32 (главная страница): откроется в браузере",
                ))
                .clicked()
            {
                self.open_web_designer();
            }
            if let Some(doc) = self.doc.as_mut() {
                let mut diag = doc.project.config.diagnostics.clone();
                ui.checkbox(&mut diag.enabled, crate::i18n::tr("Время цикла в порт"))
                    .on_hover_text(crate::i18n::tr(
                        "Прошивка раз в несколько секунд печатает в последовательный порт \
                         время цикла (всего и по листам). Режим отладки не нужен",
                    ));
                if diag.enabled {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Период отчёта:"));
                        ui.add(
                            egui::DragValue::new(&mut diag.interval_ms)
                                .range(500..=600_000)
                                .speed(100)
                                .suffix(crate::i18n::tr(" мс")),
                        );
                    });
                }
                if diag != doc.project.config.diagnostics {
                    doc.project.config.diagnostics = diag;
                    doc.dirty = true;
                    doc.revision += 1;
                }
            }
            ui.separator();
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(crate::i18n::tr("Проверить")).shortcut_text("F7"),
                )
                .clicked()
            {
                self.run_check();
            }
            if ui
                .add_enabled(has_doc, egui::Button::new(crate::i18n::tr("Код C++")))
                .clicked()
            {
                self.code_view = Some(crate::codeview::CodeView::new());
            }
            if ui
                .add_enabled(
                    has_doc && idle,
                    egui::Button::new(crate::i18n::tr("Собрать")).shortcut_text("F5"),
                )
                .clicked()
            {
                self.start_job(JobKind::Build, ui.ctx());
            }
            if ui
                .add_enabled(
                    has_doc && idle,
                    egui::Button::new(crate::i18n::tr("Пересобрать полностью")),
                )
                .on_hover_text(crate::i18n::tr(
                    "Собрать заново, даже если схема не менялась: \
                     после обновления библиотек или ядра ESP32",
                ))
                .clicked()
            {
                self.start_job(JobKind::Rebuild, ui.ctx());
            }
            if ui
                .add_enabled(
                    has_doc && idle,
                    egui::Button::new(crate::i18n::tr("Залить на плату")),
                )
                .clicked()
            {
                let port = self.port_choice();
                self.start_job(JobKind::Upload { port }, ui.ctx());
            }
            if ui
                .add_enabled(
                    has_doc && idle,
                    egui::Button::new(crate::i18n::tr("Отладка: залить и подключиться")),
                )
                .on_hover_text(crate::i18n::tr(
                    "Залить прошивку с модулем отладки и подключиться к плате",
                ))
                .clicked()
            {
                self.start_debug(ui.ctx());
            }
        });
        ui.menu_button(crate::i18n::tr("Библиотека"), |ui| {
            if ui.button(crate::i18n::tr("Новый блок...")).clicked() {
                self.block_editor = Some(crate::blockedit::BlockEditor::new_block(self, ""));
            }
            ui.weak(crate::i18n::tr(
                "Изменить блок — правой кнопкой в библиотеке",
            ));
            ui.separator();
            if ui.button(crate::i18n::tr("Новая плата...")).clicked() {
                self.board_editor = Some(crate::boardedit::BoardEditor::new_board(self));
            }
            let boards: Vec<_> = self.lib.boards().iter().cloned().collect();
            ui.add_enabled_ui(!boards.is_empty(), |ui| {
                ui.menu_button(crate::i18n::tr("Изменить плату"), |ui| {
                    for b in &boards {
                        if ui.button(&b.def.info.name).clicked() {
                            self.board_editor = Some(crate::boardedit::BoardEditor::edit(b));
                            ui.close();
                        }
                    }
                });
                ui.menu_button(crate::i18n::tr("Экспорт платы"), |ui| {
                    for b in &boards {
                        if ui.button(&b.def.info.name).clicked() {
                            crate::boardio::export(self, &b.def.info.id);
                            ui.close();
                        }
                    }
                });
            });
            if ui
                .button(crate::i18n::tr("Импорт платы из файла..."))
                .clicked()
            {
                crate::boardio::pick_files(self);
                ui.close();
            }
            ui.separator();
            if ui.button(crate::i18n::tr("Перечитать блоки")).clicked() {
                self.reload_library();
            }
        });
        ui.menu_button(crate::i18n::tr("Справка"), |ui| {
            if ui
                .add(egui::Button::new(crate::i18n::tr("Содержание")).shortcut_text("F1"))
                .clicked()
            {
                self.open_help(crate::help::START);
                ui.close();
            }
            if ui.button(crate::i18n::tr("Справочник блоков")).clicked() {
                self.open_help(crate::help::BLOCKS);
                ui.close();
            }
            if ui.button(crate::i18n::tr("Частые ошибки")).clicked() {
                self.open_help(crate::help::ERRORS);
                ui.close();
            }
            ui.separator();
            ui.weak(format!("CrabLogic {}", update::CURRENT));
            ui.separator();
            if ui
                .add_enabled(
                    self.update_rx.is_none(),
                    egui::Button::new(crate::i18n::tr("Проверить обновления")),
                )
                .clicked()
            {
                self.start_update_check(ui.ctx(), true);
                ui.close();
            }
            if ui
                .checkbox(
                    &mut self.update_check,
                    crate::i18n::tr("Проверять при запуске"),
                )
                .changed()
            {
                theme::save_update_check(self.update_check);
            }
            if ui
                .checkbox(
                    &mut self.update_beta,
                    crate::i18n::tr("Получать бета-версии"),
                )
                .on_hover_text(crate::i18n::tr(
                    "Предлагать и пробные версии (пререлизы на GitHub); они могут быть нестабильны",
                ))
                .changed()
            {
                theme::save_update_beta(self.update_beta);
            }
            if ui
                .checkbox(
                    &mut self.auto_update,
                    crate::i18n::tr("Скачивать обновления автоматически"),
                )
                .on_hover_text(crate::i18n::tr(
                    "Найденная версия скачивается в фоне; устанавливается по вашей команде",
                ))
                .changed()
            {
                theme::save_auto_update(self.auto_update);
                if self.auto_update {
                    self.start_known_release_download();
                }
            }
            if let SelfUpdate::Ready(staged) = &self.self_update {
                let staged = staged.clone();
                if ui
                    .button(crate::i18n::tr("Перезапустить и обновить"))
                    .clicked()
                {
                    self.apply_update(ui.ctx(), &staged);
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .button(crate::i18n::tr("Сообщить об ошибке"))
                .on_hover_text(crate::i18n::tr("Форма: описание уйдёт на GitHub"))
                .clicked()
            {
                self.report.open = true;
                ui.close();
            }
            if ui
                .button(crate::i18n::tr("Что нового в этой версии"))
                .clicked()
            {
                self.changes_window = true;
                ui.close();
            }
            if let Some(release) = &self.new_release {
                ui.separator();
                if ui
                    .button(crate::i18n::fmt("Что нового в {0}", &[&release.version]))
                    .clicked()
                {
                    self.update_window = true;
                    ui.close();
                }
                ui.hyperlink_to(
                    crate::i18n::fmt("Скачать версию {0}", &[&release.version]),
                    &release.url,
                );
            }
        });
    }

    pub fn open_board_settings(&mut self, ctx: &egui::Context) {
        let Some(doc) = &self.doc else { return };
        let users = flp_core::uart::block_uarts(&doc.project.logic, &self.lib)
            .into_iter()
            .map(|u| (u.number, u.block))
            .collect();
        let from_blocks = flp_core::uart::block_configs(&doc.project.logic, &self.lib)
            .into_iter()
            .map(|(id, u)| (u.number, id))
            .collect();
        let dialog = crate::boardcfg::BoardDialog::new(
            &doc.project.config.board,
            &doc.project.config.uart,
            users,
            from_blocks,
            self.lib.boards(),
            ctx,
        );
        self.dialog = Some(Dialog::Board(dialog));
        self.scan_ports(ctx);
    }

    /// Port typed in the toolbar, or `None` to use the project settings / autodetect.
    pub fn port_choice(&self) -> Option<String> {
        let port = self.port.trim();
        (!port.is_empty()).then(|| port.to_owned())
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let mut cancel = false;
        let mut install = None;
        ui.horizontal(|ui| {
            self.status_items(ui, &mut cancel);
            if let Some(release) = &self.new_release {
                ui.separator();
                let link = ui
                    .link(crate::i18n::fmt("Доступна версия {0}", &[&release.version]))
                    .on_hover_text(crate::i18n::tr("Показать, что изменилось"));
                if link.clicked() {
                    self.update_window = true;
                }
                // Downloaded: installing waits for the user's yes.
                if let SelfUpdate::Ready(staged) = &self.self_update {
                    if ui
                        .small_button(crate::i18n::tr("Установить и перезапустить"))
                        .on_hover_text(crate::i18n::tr(
                            "Проект сохранится, редактор перезапустится уже новым",
                        ))
                        .clicked()
                    {
                        install = Some(staged.clone());
                    }
                }
                if let SelfUpdate::Downloading { done, total, .. } = &self.self_update {
                    let (done, total) =
                        (done.load(Ordering::Relaxed), total.load(Ordering::Relaxed));
                    let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.0).replace('.', ",");
                    let (fraction, text) = if total > 0 {
                        (
                            done as f32 / total as f32,
                            crate::i18n::fmt("{0} из {1} Мб", &[&mb(done), &mb(total)]),
                        )
                    } else {
                        (0.0, crate::i18n::fmt("{0} Мб", &[&mb(done)]))
                    };
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(110.0)
                            .desired_height(12.0)
                            .text(egui::RichText::new(text).small()),
                    );
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(format!("v{}", update::CURRENT));
            });
        });
        if let Some(staged) = install {
            self.apply_update(&ui.ctx().clone(), &staged);
        }
        if cancel {
            self.cancel_job();
        }
    }

    fn status_items(&self, ui: &mut egui::Ui, cancel: &mut bool) {
        match &self.doc {
            Some(doc) => {
                let p = &doc.project;
                ui.label(crate::i18n::fmt("Проект: {0}", &[&p.config.project.name]));
                ui.separator();
                let board = &p.config.board;
                ui.label(crate::i18n::fmt(
                    "Плата: {0}",
                    &[&crate::boardcfg::board_name(board, self.lib.boards())],
                ))
                .on_hover_text(&board.fqbn);
                ui.separator();
                ui.label(crate::i18n::fmt("Блоков: {0}", &[&p.logic.blocks.len()]));
                ui.separator();
                let errors = doc.report.errors().count();
                if errors == 0 {
                    ui.label(
                        RichText::new(crate::i18n::tr("Схема без ошибок"))
                            .color(self.theme.palette.ok),
                    );
                } else {
                    ui.label(
                        RichText::new(crate::i18n::fmt("Ошибок в схеме: {0}", &[&errors]))
                            .color(self.theme.palette.error),
                    );
                }
                if doc.dirty {
                    ui.separator();
                    ui.label(crate::i18n::tr("Есть несохранённые изменения"));
                }
                if let Some(job) = &self.job {
                    ui.separator();
                    panels::busy(ui);
                    ui.label(job.title);
                    if ui.small_button(crate::i18n::tr("Отменить")).clicked() {
                        *cancel = true;
                    }
                }
                if let Some(d) = &self.debug {
                    ui.separator();
                    let color = if d.online() {
                        self.theme.palette.ok
                    } else {
                        self.theme.palette.warning
                    };
                    ui.label(
                        RichText::new(crate::i18n::fmt(
                            "Отладка {0}: {1}",
                            &[&d.port, &d.live.status],
                        ))
                        .color(color),
                    );
                    if let Some(s) = d.live.stats.as_ref().filter(|_| d.online()) {
                        ui.separator();
                        ui.label(crate::i18n::fmt(
                            "Цикл {0} мкс, макс {1} мкс",
                            &[&s.last_us, &s.max_us],
                        ));
                        if let Some(bg) = &s.background {
                            ui.label(crate::i18n::fmt(
                                "Ядро 0: {0} мкс, макс {1} мкс",
                                &[&bg.last_us, &bg.max_us],
                            ))
                            .on_hover_text(crate::i18n::tr(
                                "Проход фоновой задачи на ядре 0: сеть и фоновые платы",
                            ));
                        }
                    }
                    if let Some(ip) = d.device_address(&p.logic.blocks) {
                        ui.separator();
                        ui.hyperlink_to(&ip, format!("http://{ip}")).on_hover_text(
                            crate::i18n::tr("Открыть страницу устройства в браузере"),
                        );
                    }
                }
            }
            None => {
                ui.label(crate::i18n::tr("Проект не открыт"));
            }
        }
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        let mut decision = None;
        egui::Modal::new(egui::Id::unique("unsaved")).show(ctx, |ui| {
            ui.heading(crate::i18n::tr("Несохранённые изменения"));
            ui.label(crate::i18n::tr("Сохранить изменения в проекте?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("Сохранить")).clicked() {
                    decision = Some(Some(true));
                }
                if ui.button(crate::i18n::tr("Не сохранять")).clicked() {
                    decision = Some(Some(false));
                }
                if ui.button(crate::i18n::tr("Отмена")).clicked() {
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
        if !close || self.allow_close {
            return;
        }
        // An open block editor with unsaved changes asks first; the project is
        // checked on the next close.
        if let Some(ed) = self.block_editor.as_mut().filter(|e| e.dirty()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ed.ask_to_close();
        } else if let Some(ed) = self.board_editor.as_mut().filter(|e| e.dirty()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ed.ask_to_close();
        } else if self.doc.as_ref().is_some_and(|d| d.dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
    }

    fn window_title(&self) -> String {
        match &self.doc {
            Some(doc) => format!(
                "{}{} — CrabLogic",
                doc.project.config.project.name,
                if doc.dirty { " *" } else { "" }
            ),
            None => "CrabLogic".to_owned(),
        }
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = self.window_title();
        // A viewport command always forces a repaint, so send it only on change;
        // otherwise the window redraws non-stop (~30 fps) and loads the GPU at idle.
        let id = egui::Id::unique("window_title");
        if ctx.data(|d| d.get_temp::<String>(id)).as_deref() != Some(title.as_str()) {
            ctx.data_mut(|d| d.insert_temp(id, title.clone()));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        flp_core::local::editor_beat(self.doc.as_ref().map(|d| d.project.path.as_path()));
        ctx.request_repaint_after(flp_core::local::EDITOR_BEAT);
        crate::textmenu::begin(&ctx);
        self.poll_job(&ctx);
        self.precompile(&ctx);
        self.poll_debug();
        self.poll_ai();
        self.poll_console(&ctx);
        self.poll_ports();
        self.poll_update();
        if self.update_check
            && self.update_rx.is_none()
            && self.update_checked.elapsed() >= UPDATE_RECHECK
        {
            self.start_update_check(&ctx, false);
        }
        self.cli_window(&ctx);
        self.mcp
            .set_project(self.doc.as_ref().map(|d| d.project.path.as_path()));
        self.mcp.window(&ctx);
        self.ai_conf.window(&ctx);
        self.poll_self_update(&ctx);
        update::changes_window(&ctx, &mut self.changes_window);
        crate::blockio::handle_drops(self, &ctx);
        crate::blockio::window(self, &ctx);
        crate::boardio::handle_drops(self, &ctx);
        crate::boardio::window(self, &ctx);
        if self.report.open {
            let repo = theme::update_repo();
            let board = self.doc.as_ref().map(|d| {
                let b = &d.project.config.board;
                (
                    crate::boardcfg::board_name(b, self.lib.boards()),
                    b.fqbn.clone(),
                )
            });
            let facts = crate::report::Context {
                repo: &repo,
                version: update::CURRENT,
                board: board.as_ref().map(|(n, f)| (*n, f.as_str())),
                log: self.log.iter().map(|l| l.text.clone()).collect(),
            };
            if let Some(url) = self.report.show(&ctx, &facts) {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
        }
        if let Some(release) = &self.new_release {
            let install = match &self.self_update {
                SelfUpdate::Idle(e) => update::Install::Idle(e.as_deref()),
                SelfUpdate::Downloading { done, total, .. } => update::Install::Busy(
                    done.load(Ordering::Relaxed),
                    total.load(Ordering::Relaxed),
                ),
                SelfUpdate::Ready(_) => update::Install::Ready,
            };
            let can = selfupdate::install_dir().is_some();
            if update::window(&ctx, release, &mut self.update_window, install, can) {
                if let SelfUpdate::Ready(staged) = &self.self_update {
                    let staged = staged.clone();
                    self.apply_update(&ctx, &staged);
                } else {
                    let url = release.url.clone();
                    self.start_self_update(url, false);
                }
            }
        }
        self.watch_library();
        self.watch_project();
        self.block_versions_window(&ctx);
        ctx.request_repaint_after(Duration::from_secs(1));
        self.autosave(&ctx);
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
            self.title_bar(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        let mut sizes = self.panels;
        if sizes.log_open {
            sizes.log_height = egui::Panel::bottom("log")
                .resizable(true)
                .default_size(sizes.log_height)
                .show(ui, |ui| panels::bottom(self, ui, &mut sizes.log_open))
                .response
                .rect
                .height();
        } else {
            // One row; a separate id keeps the open height in the panel memory.
            egui::Panel::bottom("log_collapsed")
                .resizable(false)
                .show(ui, |ui| panels::bottom(self, ui, &mut sizes.log_open));
        }
        panels::side_panel(
            ui,
            "tree",
            true,
            &mut sizes.left_open,
            &mut sizes.left_width,
            |ui| panels::left(self, ui),
        );
        panels::side_panel(
            ui,
            "library",
            false,
            &mut sizes.right_open,
            &mut sizes.right_width,
            |ui| panels::right(self, ui),
        );
        // Save once a resize drag is over, not on every frame of it.
        if sizes != self.panels && !ui.input(|i| i.pointer.any_down()) {
            self.panels = sizes;
            theme::save_panels(sizes);
        }
        egui::CentralPanel::default_margins().show(ui, |ui| canvas::show(self, ui));

        dialogs::show(self, &ctx);
        crate::themeedit::show(self, &ctx);
        crate::blockedit::show(self, &ctx);
        crate::boardedit::show(self, &ctx);
        crate::codeview::show(self, &ctx);
        crate::help::show(self, &ctx);
        crate::blockedit::delete_dialog(self, &ctx);
        crate::canvas::sheet_delete_dialog(self, &ctx);
        let pal = self.theme.palette;
        if let Some(d) = &mut self.debug {
            debugview::values_window(d, &pal, &ctx);
            debugview::chart_window(d, &pal, &ctx);
            let layout = self.doc.as_ref().map(|doc| &doc.project.layout);
            debugview::diagnostics_window(d, &pal, layout, &ctx);
        }
        self.unsaved_dialog(&ctx);
        crate::textmenu::show(&ctx);
    }
}

/// Keeps helper processes from flashing a console window.
fn hide_console(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

impl Drop for EditorApp {
    fn drop(&mut self) {
        if let Some(child) = &mut self.web_editor {
            let _ = child.kill();
        }
    }
}

/// `library/` next to the project (or its parents), next to the executable, or `FLP_LIBRARY`.
fn find_library(project: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FLP_LIBRARY") {
        return Some(PathBuf::from(dir));
    }
    let starts = [
        project.and_then(|p| std::path::absolute(p).ok()),
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

/// Change stamp of the built-in and user block folders.
fn library_stamp(dir: &Path) -> Option<SystemTime> {
    newest_mtime(dir).max(newest_mtime_deep(&flp_core::user_library_dir(dir)))
}

/// Latest modification time in a whole tree: user blocks lie in sections and folders.
fn newest_mtime_deep(dir: &Path) -> Option<SystemTime> {
    let mut newest = fs::metadata(dir).and_then(|m| m.modified()).ok();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let t = if path.is_dir() {
            newest_mtime_deep(&path)
        } else {
            fs::metadata(&path).and_then(|m| m.modified()).ok()
        };
        newest = newest.max(t);
    }
    newest
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

fn is_block_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case(flp_core::pack::EXTENSION)
            || e.eq_ignore_ascii_case(flp_core::pack::LEGACY_EXTENSION)
    })
}
