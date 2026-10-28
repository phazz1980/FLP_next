//! Import and export of user blocks: one block is one `.flpblock` file with its
//! Arduino libraries and an example project.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Id, RichText};
use flp_core::blockfile::{self, Conflict, Incoming};

use crate::app::{EditorApp, LogLevel};

/// A block waiting in the import window.
struct Entry {
    block: Incoming,
    /// Where it came from, for the list.
    source: String,
    install: bool,
    replace: bool,
}

/// Blocks offered for import (`EditorApp::import`).
#[derive(Default)]
pub struct Import {
    entries: Vec<Entry>,
}

impl Import {
    fn is_open(&self) -> bool {
        !self.entries.is_empty()
    }

    fn add(&mut self, block: Incoming, source: String) {
        // The same type again: the later file wins.
        self.entries.retain(|e| e.block.kind() != block.kind());
        self.entries.push(Entry {
            block,
            source,
            install: true,
            replace: false,
        });
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Reads `.flpblock` files and queues the blocks in the import window.
pub fn add_files(app: &mut EditorApp, paths: &[PathBuf]) {
    for path in paths {
        match blockfile::read_block_file(path) {
            Ok(block) => app.import.add(block, name_of(path)),
            Err(e) => app.info(LogLevel::Error, format!("Блок не прочитан: {e}")),
        }
    }
}

/// «Импорт…»: choose block files.
pub fn pick_files(app: &mut EditorApp) {
    let paths = rfd::FileDialog::new()
        .set_title("Импорт блоков")
        .add_filter("Блок FLP", &[blockfile::EXTENSION])
        .pick_files();
    if let Some(paths) = paths {
        add_files(app, &paths);
    }
}

/// «Из проекта…»: offer the user blocks packed in a project file.
pub fn pick_project(app: &mut EditorApp) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Блоки из проекта")
        .add_filter("Проект FLP", &["flpn"])
        .pick_file()
    else {
        return;
    };
    let blocks = flp_core::packed::project_blocks(&path);
    if blocks.is_empty() {
        app.info(
            LogLevel::Warning,
            format!("В проекте {} нет пользовательских блоков", name_of(&path)),
        );
    }
    for block in blocks {
        app.import.add(block, name_of(&path));
    }
}

/// Files dropped onto the window: `.flpblock` go to the import window.
pub fn handle_drops(app: &mut EditorApp, ctx: &egui::Context) {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(blockfile::EXTENSION))
            })
            .collect()
    });
    if !dropped.is_empty() {
        add_files(app, &dropped);
    }
}

/// «Экспорт в файл…» for the user block `kind`.
pub fn export(app: &mut EditorApp, kind: &str) {
    let Some(dest) = rfd::FileDialog::new()
        .set_title("Экспорт блока")
        .set_file_name(format!("{kind}.{}", blockfile::EXTENSION))
        .add_filter("Блок FLP", &[blockfile::EXTENSION])
        .save_file()
    else {
        return;
    };
    match blockfile::export_block(&app.lib, kind, &dest) {
        Ok(()) => app.info(
            LogLevel::Ok,
            format!("Блок «{kind}» сохранён в {}", dest.display()),
        ),
        Err(e) => app.info(LogLevel::Error, format!("Блок не экспортирован: {e}")),
    }
}

/// Opens the example project of the block (from a copy, so the block stays intact).
pub fn open_example(app: &mut EditorApp, kind: &str) {
    let Some(block) = app.lib.get(kind) else {
        return;
    };
    let src = block.dir.join(blockfile::EXAMPLE_FILE);
    if !src.is_file() {
        app.info(LogLevel::Warning, format!("У блока «{kind}» нет примера"));
        return;
    }
    let dir = std::env::temp_dir().join("flp_block_examples");
    let dest = dir.join(format!("{kind}_пример.flpn"));
    let copied = std::fs::create_dir_all(&dir).and_then(|()| std::fs::copy(&src, &dest));
    match copied {
        Ok(_) => app.request_open(dest),
        Err(e) => app.info(LogLevel::Error, format!("Пример не открыт: {e}")),
    }
}

/// The import window; installs the checked blocks.
pub fn window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.import.is_open() {
        return;
    }
    let warn = app.theme.palette.warning;
    let mut entries = std::mem::take(&mut app.import.entries);
    let conflicts: Vec<Conflict> = entries.iter().map(|e| e.block.conflict(&app.lib)).collect();
    let mut answer = None;
    egui::Modal::new(Id::unique("block_import")).show(ctx, |ui| {
        ui.label(RichText::new("Импорт блоков").strong());
        ui.add_space(4.0);
        for (e, conflict) in entries.iter_mut().zip(&conflicts) {
            ui.horizontal(|ui| {
                let blocked = *conflict == Conflict::Builtin;
                ui.add_enabled(!blocked, egui::Checkbox::without_text(&mut e.install));
                ui.label(format!("{} ({})", e.block.def.info.name, e.block.kind()));
                ui.weak(format!("← {}", e.source));
            });
            ui.indent(("import", e.block.kind()), |ui| {
                let libs = e.block.libraries();
                let mut parts = Vec::new();
                if !libs.is_empty() {
                    parts.push(format!("библиотеки: {}", libs.join(", ")));
                }
                if e.block.has_example() {
                    parts.push("есть пример".to_owned());
                }
                if !parts.is_empty() {
                    ui.weak(parts.join("; "));
                }
                match conflict {
                    Conflict::None => {}
                    Conflict::Builtin => {
                        ui.colored_label(warn, "Тип занят встроенным блоком — импорт невозможен");
                    }
                    Conflict::User(have) => {
                        ui.colored_label(
                            warn,
                            format!(
                                "Такой блок уже есть (версия {have}, в файле {}).",
                                e.block.def.info.version
                            ),
                        );
                        ui.checkbox(&mut e.replace, "Заменить существующий");
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Импортировать").clicked() {
                answer = Some(true);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    match answer {
        None => app.import.entries = entries,
        Some(false) => {}
        Some(true) => install(app, &entries, &conflicts),
    }
}

fn install(app: &mut EditorApp, entries: &[Entry], conflicts: &[Conflict]) {
    let Some(user_dir) = app.user_lib_dir() else {
        app.info(LogLevel::Error, "Нет папки библиотеки блоков");
        return;
    };
    let mut done = false;
    for (e, conflict) in entries.iter().zip(conflicts) {
        if !e.install {
            continue;
        }
        let name = &e.block.def.info.name;
        if matches!(conflict, Conflict::User(_)) && !e.replace {
            app.info(
                LogLevel::Warning,
                format!("Блок «{name}» оставлен как был (замена не выбрана)"),
            );
            continue;
        }
        match e.block.install(&user_dir, &app.lib) {
            Ok(dir) => {
                done = true;
                app.info(
                    LogLevel::Ok,
                    format!("Блок «{name}» установлен в {}", dir.display()),
                );
            }
            Err(err) => app.info(
                LogLevel::Error,
                format!("Блок «{name}» не установлен: {err}"),
            ),
        }
    }
    if done {
        app.reload_library();
    }
}
