//! Import and export of user boards: a board is one `.crabboard` file with its folder
//! (`board.toml`, the description and the pictures).

use std::path::{Path, PathBuf};

use eframe::egui::{self, Id, RichText};
use flp_core::boardfile::{self, BoardConflict, IncomingBoard};

use crate::app::{EditorApp, LogLevel};

/// A board waiting in the import window.
struct Entry {
    board: IncomingBoard,
    /// File it came from, for the list.
    source: String,
    install: bool,
    /// Replace the installed board of the same id.
    replace: bool,
}

/// Boards offered for import (`EditorApp::board_import`).
#[derive(Default)]
pub struct Import {
    entries: Vec<Entry>,
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Whether `path` is a board file by its extension.
pub fn is_board_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case(boardfile::EXTENSION)
            || e.eq_ignore_ascii_case(boardfile::LEGACY_EXTENSION)
    })
}

/// Reads `.crabboard` files and shows the boards in the import window.
pub fn add_files(app: &mut EditorApp, paths: &[PathBuf]) {
    for path in paths {
        match boardfile::read_board_file(path) {
            Ok(board) => {
                // The same board again: the later file wins.
                app.board_import
                    .entries
                    .retain(|e| e.board.id() != board.id());
                app.board_import.entries.push(Entry {
                    board,
                    source: name_of(path),
                    install: true,
                    replace: false,
                });
            }
            Err(e) => app.info(
                LogLevel::Error,
                crate::i18n::fmt("Плата не прочитана: {0}", &[&e]),
            ),
        }
    }
}

/// «Импорт платы…»: choose board files.
pub fn pick_files(app: &mut EditorApp) {
    let paths = rfd::FileDialog::new()
        .set_title(crate::i18n::tr("Импорт платы"))
        .add_filter(crate::i18n::tr("Плата FLP"), &[boardfile::EXTENSION])
        .pick_files();
    if let Some(paths) = paths {
        add_files(app, &paths);
    }
}

/// Files dropped onto the window: `.crabboard` go to the import window.
pub fn handle_drops(app: &mut EditorApp, ctx: &egui::Context) {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| is_board_file(p))
            .collect()
    });
    if !dropped.is_empty() {
        add_files(app, &dropped);
    }
}

/// «Экспорт платы…» for the saved user board `id`.
pub fn export(app: &mut EditorApp, id: &str) {
    let Some(dest) = rfd::FileDialog::new()
        .set_title(crate::i18n::tr("Экспорт платы"))
        .set_file_name(format!("{id}.{}", boardfile::EXTENSION))
        .add_filter(crate::i18n::tr("Плата FLP"), &[boardfile::EXTENSION])
        .save_file()
    else {
        return;
    };
    match boardfile::export_board(&app.lib, id, &dest) {
        Ok(()) => app.info(
            LogLevel::Ok,
            crate::i18n::fmt("Плата «{0}» сохранена в {1}", &[&id, &dest.display()]),
        ),
        Err(e) => app.info(
            LogLevel::Error,
            crate::i18n::fmt("Плата не экспортирована: {0}", &[&e]),
        ),
    }
}

/// The import window; installs the checked boards.
pub fn window(app: &mut EditorApp, ctx: &egui::Context) {
    if app.board_import.entries.is_empty() {
        return;
    }
    let warn = app.theme.palette.warning;
    let mut entries = std::mem::take(&mut app.board_import.entries);
    let conflicts: Vec<BoardConflict> =
        entries.iter().map(|e| e.board.conflict(&app.lib)).collect();
    let mut answer = None;
    egui::Modal::new(Id::unique("board_import")).show(ctx, |ui| {
        ui.label(RichText::new(crate::i18n::tr("Импорт плат")).strong());
        ui.add_space(4.0);
        for (e, conflict) in entries.iter_mut().zip(&conflicts) {
            ui.horizontal(|ui| {
                ui.checkbox(&mut e.install, "");
                ui.label(format!("{} ({})", e.board.name(), e.board.id()));
                ui.weak(format!("← {}", e.source));
            });
            ui.indent(("board_import", e.board.id()), |ui| {
                let def = &e.board.def;
                let mut parts = vec![crate::i18n::fmt("чип {0}", &[&def.info.chip])];
                if !def.pins.is_empty() {
                    parts.push(format!("выводов в таблице: {}", def.pins.len()));
                }
                if !def.devices.is_empty() {
                    parts.push(format!("устройств: {}", def.devices.len()));
                }
                if e.board.files.iter().any(|(n, _)| n.starts_with("images/")) {
                    parts.push("есть картинки".to_owned());
                }
                ui.weak(parts.join("; "));
                if let BoardConflict::User(have) = conflict {
                    ui.colored_label(
                        warn,
                        crate::i18n::fmt(
                            "Такая плата уже есть (версия {0}, в файле {1}).",
                            &[&have, &def.info.version],
                        ),
                    );
                    ui.checkbox(&mut e.replace, crate::i18n::tr("Заменить существующую"));
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
        None => app.board_import.entries = entries,
        Some(false) => {}
        Some(true) => install(app, &entries, &conflicts),
    }
}

fn install(app: &mut EditorApp, entries: &[Entry], conflicts: &[BoardConflict]) {
    let Some(user_dir) = app.user_lib_dir() else {
        app.info(
            LogLevel::Error,
            crate::i18n::tr("Нет папки пользовательской библиотеки"),
        );
        return;
    };
    let mut done = false;
    for (e, conflict) in entries.iter().zip(conflicts) {
        if !e.install {
            continue;
        }
        let name = e.board.name();
        if matches!(conflict, BoardConflict::User(_)) && !e.replace {
            app.info(
                LogLevel::Warning,
                crate::i18n::fmt(
                    "Плата «{0}» оставлена как была (замена не выбрана)",
                    &[&name],
                ),
            );
            continue;
        }
        match e.board.install(&user_dir) {
            Ok(dir) => {
                done = true;
                app.info(
                    LogLevel::Ok,
                    crate::i18n::fmt("Плата «{0}» установлена в {1}", &[&name, &dir.display()]),
                );
            }
            Err(err) => app.info(
                LogLevel::Error,
                crate::i18n::fmt("Плата «{0}» не установлена: {1}", &[&name, &err]),
            ),
        }
    }
    if done {
        app.reload_library();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_files_are_told_by_extension() {
        assert!(is_board_file(Path::new("kc868_a6.crabboard")));
        assert!(is_board_file(Path::new("D:/x/KC868.FLPBOARD")));
        assert!(!is_board_file(Path::new("pack.crabblocks")));
        assert!(!is_board_file(Path::new("flpboard")));
    }
}
