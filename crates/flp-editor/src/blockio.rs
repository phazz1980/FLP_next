//! Import and export of user blocks: a pack of blocks is one `.flpblocks` file with the
//! Arduino libraries and examples of its blocks. Single blocks can be taken from a project.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Id, RichText};
use flp_core::blockfile::{self, Conflict, Incoming};
use flp_core::pack::{self, IncomingPack, PackImport};

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
    /// A pack read from a file, waiting for the user's decision.
    pack: Option<PackEntry>,
}

struct PackEntry {
    pack: IncomingPack,
    source: String,
    /// Replace an installed pack of the same or a newer version.
    replace: bool,
}

impl Import {
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

/// Reads `.flpblocks` files and shows the pack in the import window.
pub fn add_files(app: &mut EditorApp, paths: &[PathBuf]) {
    for path in paths {
        match pack::read_pack_file(path) {
            Ok(pack) => {
                app.import.pack = Some(PackEntry {
                    pack,
                    source: name_of(path),
                    replace: false,
                });
            }
            Err(e) => app.info(LogLevel::Error, format!("Набор не прочитан: {e}")),
        }
    }
}

/// «Набор из файла…»: choose a pack file.
pub fn pick_files(app: &mut EditorApp) {
    let paths = rfd::FileDialog::new()
        .set_title("Импорт набора блоков")
        .add_filter("Набор блоков FLP", &[pack::EXTENSION])
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

/// Files dropped onto the window: `.flpblocks` go to the import window.
pub fn handle_drops(app: &mut EditorApp, ctx: &egui::Context) {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(pack::EXTENSION))
            })
            .collect()
    });
    if !dropped.is_empty() {
        add_files(app, &dropped);
    }
}

/// «Экспорт набора…» for the pack in the folder `folder` of the user library.
pub fn export_pack(app: &mut EditorApp, folder: &str) {
    let Some(user_dir) = app.user_lib_dir() else {
        return;
    };
    let Some(dest) = rfd::FileDialog::new()
        .set_title("Экспорт набора блоков")
        .set_file_name(format!("{folder}.{}", pack::EXTENSION))
        .add_filter("Набор блоков FLP", &[pack::EXTENSION])
        .save_file()
    else {
        return;
    };
    match pack::export_pack(&user_dir, folder, &dest) {
        Ok(()) => app.info(
            LogLevel::Ok,
            format!("Набор «{folder}» сохранён в {}", dest.display()),
        ),
        Err(e) => app.info(LogLevel::Error, format!("Набор не экспортирован: {e}")),
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
    pack_window(app, ctx);
    if app.import.entries.is_empty() {
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

/// The window of a pack read from a file: what it holds and what installing does.
fn pack_window(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut entry) = app.import.pack.take() else {
        return;
    };
    let warn = app.theme.palette.warning;
    let status = app
        .user_lib_dir()
        .map_or(PackImport::New, |dir| entry.pack.status(&dir));
    let info = entry.pack.info.clone();
    let kinds = entry.pack.kinds();
    let mut answer = None;
    egui::Modal::new(Id::unique("pack_import")).show(ctx, |ui| {
        ui.label(RichText::new(format!("Набор «{}»", info.name)).strong());
        ui.weak(format!("версия {} · файл {}", info.version, entry.source));
        if !info.description.is_empty() {
            ui.label(&info.description);
        }
        pack_credits(ui, &info);
        ui.label(format!("Блоков: {} ({})", kinds.len(), kinds.join(", ")));
        match status {
            PackImport::New => {}
            PackImport::Update(have) => {
                ui.colored_label(
                    warn,
                    format!(
                        "Набор уже есть (версия {have}): он будет заменён целиком, \
                         блоки, которых нет в файле, пропадут."
                    ),
                );
            }
            PackImport::NotNewer(have) => {
                ui.colored_label(
                    warn,
                    format!(
                        "Установлен набор той же или более новой версии ({have}, в файле {}).",
                        info.version
                    ),
                );
                ui.checkbox(&mut entry.replace, "Всё равно заменить целиком");
            }
        }
        ui.add_space(6.0);
        let allowed = !matches!(status, PackImport::NotNewer(_)) || entry.replace;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(allowed, egui::Button::new("Установить"))
                .clicked()
            {
                answer = Some(true);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    match answer {
        None => app.import.pack = Some(entry),
        Some(false) => {}
        Some(true) => {
            let Some(user_dir) = app.user_lib_dir() else {
                app.info(LogLevel::Error, "Нет папки библиотеки блоков");
                return;
            };
            match entry.pack.install(&user_dir, &app.lib) {
                Ok(_) => {
                    app.info(LogLevel::Ok, format!("Набор «{}» установлен", info.name));
                    app.reload_library();
                }
                Err(e) => app.info(
                    LogLevel::Error,
                    format!("Набор «{}» не установлен: {e}", info.name),
                ),
            }
        }
    }
}

/// Authors, contacts (as links) and license of a pack.
pub fn pack_credits(ui: &mut egui::Ui, info: &pack::PackInfo) {
    if !info.authors.is_empty() {
        ui.label(format!("Автор: {}", info.authors.join(", ")));
    }
    if !info.contacts.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Контакты:");
            for c in &info.contacts {
                ui.hyperlink_to(c, pack::contact_url(c));
            }
        });
    }
    if !info.license.is_empty() {
        ui.label(format!("Лицензия: {}", info.license));
    }
}
