//! Import and export of user blocks: a pack of blocks is one `.crabblocks` file with the
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

/// Reads `.crabblocks` files and shows the pack in the import window.
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
            Err(e) => app.info(
                LogLevel::Error,
                crate::i18n::fmt("Набор не прочитан: {0}", &[&e]),
            ),
        }
    }
}

/// «Набор из файла…»: choose a pack file.
pub fn pick_files(app: &mut EditorApp) {
    let paths = rfd::FileDialog::new()
        .set_title(crate::i18n::tr("Импорт набора блоков"))
        .add_filter(
            crate::i18n::tr("Набор блоков FLP"),
            &[pack::EXTENSION, pack::LEGACY_EXTENSION],
        )
        .pick_files();
    if let Some(paths) = paths {
        add_files(app, &paths);
    }
}

/// «Из проекта…»: offer the user blocks packed in a project file.
pub fn pick_project(app: &mut EditorApp) {
    let Some(path) = rfd::FileDialog::new()
        .set_title(crate::i18n::tr("Блоки из проекта"))
        .add_filter(
            crate::i18n::tr("Проект FLP"),
            &[
                flp_core::project::PROJECT_EXT,
                flp_core::project::LEGACY_PROJECT_EXT,
            ],
        )
        .pick_file()
    else {
        return;
    };
    let blocks = flp_core::packed::project_blocks(&path);
    if blocks.is_empty() {
        app.info(
            LogLevel::Warning,
            crate::i18n::fmt(
                "В проекте {0} нет пользовательских блоков",
                &[&name_of(&path)],
            ),
        );
    }
    for block in blocks {
        app.import.add(block, name_of(&path));
    }
}

/// Files dropped onto the window: `.crabblocks` go to the import window.
pub fn handle_drops(app: &mut EditorApp, ctx: &egui::Context) {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| {
                p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case(pack::EXTENSION)
                        || e.eq_ignore_ascii_case(pack::LEGACY_EXTENSION)
                })
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
        .set_title(crate::i18n::tr("Экспорт набора блоков"))
        .set_file_name(format!("{folder}.{}", pack::EXTENSION))
        .add_filter(crate::i18n::tr("Набор блоков FLP"), &[pack::EXTENSION])
        .save_file()
    else {
        return;
    };
    match pack::export_pack(&user_dir, folder, &dest) {
        Ok(()) => app.info(
            LogLevel::Ok,
            crate::i18n::fmt("Набор «{0}» сохранён в {1}", &[&folder, &dest.display()]),
        ),
        Err(e) => app.info(
            LogLevel::Error,
            crate::i18n::fmt("Набор не экспортирован: {0}", &[&e]),
        ),
    }
}

/// Opens the example project of the block (from a copy, so the block stays intact).
pub fn open_example(app: &mut EditorApp, kind: &str) {
    let Some(block) = app.lib.get(kind) else {
        return;
    };
    let src = block.dir.join(blockfile::EXAMPLE_FILE);
    if !src.is_file() {
        app.info(
            LogLevel::Warning,
            crate::i18n::fmt("У блока «{0}» нет примера", &[&kind]),
        );
        return;
    }
    let dir = std::env::temp_dir().join("flp_block_examples");
    let dest = dir.join(format!("{kind}_пример.crab"));
    let copied = std::fs::create_dir_all(&dir).and_then(|()| std::fs::copy(&src, &dest));
    match copied {
        Ok(_) => app.request_open(dest),
        Err(e) => app.info(
            LogLevel::Error,
            crate::i18n::fmt("Пример не открыт: {0}", &[&e]),
        ),
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
        ui.label(RichText::new(crate::i18n::tr("Импорт блоков")).strong());
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
                        ui.colored_label(
                            warn,
                            crate::i18n::tr("Тип занят встроенным блоком — импорт невозможен"),
                        );
                    }
                    Conflict::User => {
                        ui.colored_label(warn, crate::i18n::tr("Такой блок уже есть."));
                        ui.checkbox(&mut e.replace, crate::i18n::tr("Заменить существующий"));
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("Импортировать")).clicked() {
                answer = Some(true);
            }
            if ui.button(crate::i18n::tr("Отмена")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
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
        app.info(
            LogLevel::Error,
            crate::i18n::tr("Нет папки библиотеки блоков"),
        );
        return;
    };
    let mut done = false;
    for (e, conflict) in entries.iter().zip(conflicts) {
        if !e.install {
            continue;
        }
        let name = &e.block.def.info.name;
        if matches!(conflict, Conflict::User) && !e.replace {
            app.info(
                LogLevel::Warning,
                crate::i18n::fmt("Блок «{0}» оставлен как был (замена не выбрана)", &[&name]),
            );
            continue;
        }
        match e.block.install(&user_dir, &app.lib) {
            Ok(dir) => {
                done = true;
                app.info(
                    LogLevel::Ok,
                    crate::i18n::fmt("Блок «{0}» установлен в {1}", &[&name, &dir.display()]),
                );
            }
            Err(err) => app.info(
                LogLevel::Error,
                crate::i18n::fmt("Блок «{0}» не установлен: {1}", &[&name, &err]),
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
        ui.label(RichText::new(crate::i18n::fmt("Набор «{0}»", &[&info.name])).strong());
        ui.weak(crate::i18n::fmt(
            "версия {0} · файл {1}",
            &[&info.version, &entry.source],
        ));
        if !info.description.is_empty() {
            ui.label(&info.description);
        }
        pack_credits(ui, &info);
        ui.label(crate::i18n::fmt(
            "Блоков: {0} ({1})",
            &[&kinds.len(), &kinds.join(", ")],
        ));
        match status {
            PackImport::New => {}
            PackImport::Update(have) => {
                ui.colored_label(
                    warn,
                    crate::i18n::fmt(
                        "Набор уже есть (версия {0}): он будет заменён целиком, \
                         блоки, которых нет в файле, пропадут.",
                        &[&have],
                    ),
                );
            }
            PackImport::NotNewer(have) => {
                ui.colored_label(
                    warn,
                    crate::i18n::fmt(
                        "Установлен набор той же или более новой версии ({0}, в файле {1}).",
                        &[&have, &info.version],
                    ),
                );
                ui.checkbox(
                    &mut entry.replace,
                    crate::i18n::tr("Всё равно заменить целиком"),
                );
            }
        }
        ui.add_space(6.0);
        let allowed = !matches!(status, PackImport::NotNewer(_)) || entry.replace;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(allowed, egui::Button::new(crate::i18n::tr("Установить")))
                .clicked()
            {
                answer = Some(true);
            }
            if ui.button(crate::i18n::tr("Отмена")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
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
                app.info(
                    LogLevel::Error,
                    crate::i18n::tr("Нет папки библиотеки блоков"),
                );
                return;
            };
            match entry.pack.install(&user_dir, &app.lib) {
                Ok(_) => {
                    app.info(
                        LogLevel::Ok,
                        crate::i18n::fmt("Набор «{0}» установлен", &[&info.name]),
                    );
                    app.reload_library();
                }
                Err(e) => app.info(
                    LogLevel::Error,
                    crate::i18n::fmt("Набор «{0}» не установлен: {1}", &[&info.name, &e]),
                ),
            }
        }
    }
}

/// Authors, contacts (as links) and license of a pack.
pub fn pack_credits(ui: &mut egui::Ui, info: &pack::PackInfo) {
    if !info.authors.is_empty() {
        ui.label(crate::i18n::fmt("Автор: {0}", &[&info.authors.join(", ")]));
    }
    if !info.contacts.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::i18n::tr("Контакты:"));
            for c in &info.contacts {
                ui.hyperlink_to(c, pack::contact_url(c));
            }
        });
    }
    if !info.license.is_empty() {
        ui.label(crate::i18n::fmt("Лицензия: {0}", &[&info.license]));
    }
}
