//! "Пользовательские" tab of the block library: the tree «Блоки пользователей» →
//! sections → folders → blocks, as in FLProg. Sections and folders are plain
//! folders inside `user_library/`; a block's place in the tree is its group.

use std::fs;
use std::path::PathBuf;

use eframe::egui::{self, Id, RichText, Ui};
use flp_core::library::is_valid_folder_name;

use crate::app::{EditorApp, LogLevel};
use crate::blockedit::BlockEditor;
use crate::canvas::DragItem;

const ROOT: &str = "Блоки пользователей";

/// Selected tree item: a folder path (`Раздел/Папка`) or a block type.
#[derive(Debug, Clone, PartialEq)]
enum Selected {
    Folder(String),
    Block(String),
}

/// Name asked for a new or renamed section or folder.
#[derive(Debug, Clone, PartialEq)]
struct Prompt {
    /// New folder inside this path (`""` — a section), or rename of this path.
    target: String,
    rename: bool,
    name: String,
}

enum Action {
    Select(Selected),
    Open(String),
    Copy(String),
    Add(String),
    DeleteBlock(String),
    Move(String, String),
    NewBlock(String),
    Ask(Prompt),
    DeleteFolder(String),
    Import,
    ImportProject,
    Export(String),
    Example(String),
}

/// Last part of a folder path.
fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

/// «раздел» for a top-level folder, «папка» deeper.
fn folder_word(path: &str) -> &'static str {
    if path.contains('/') {
        "папка"
    } else {
        "раздел"
    }
}

/// The tab; `needle` is the lower-case search text. Returns a block type to add
/// to the schema (double click).
pub fn show(app: &mut EditorApp, ui: &mut Ui, needle: &str) -> Option<String> {
    let sel_id = Id::unique("userlib_selected");
    let prompt_id = Id::unique("userlib_prompt");
    let selected: Option<Selected> = ui.data_mut(|d| d.get_temp(sel_id)).flatten();
    let mut actions = Vec::new();

    toolbar(app, ui, selected.as_ref(), &mut actions);
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([ui.is_sizing_pass(), false])
        .show(ui, |ui| {
            if needle.is_empty() {
                tree(app, ui, selected.as_ref(), &mut actions);
            } else {
                found(app, ui, needle, selected.as_ref(), &mut actions);
            }
        });

    let mut prompt: Option<Prompt> = ui.data_mut(|d| d.get_temp(prompt_id)).flatten();
    if let Some(p) = &mut prompt {
        match ask_name(app, ui.ctx(), p) {
            Some(true) => {
                if apply_prompt(app, p) {
                    prompt = None;
                }
            }
            Some(false) => prompt = None,
            None => {}
        }
    }

    let mut selected = selected;
    let mut add = None;
    for action in actions {
        match action {
            Action::Select(s) => selected = Some(s),
            Action::Open(kind) => {
                if let Some(b) = app.lib.get(&kind).cloned() {
                    app.block_editor = Some(BlockEditor::edit(app, &b));
                }
            }
            Action::Copy(kind) => {
                if let Some(b) = app.lib.get(&kind).cloned() {
                    app.block_editor = Some(BlockEditor::copy_of(app, &b));
                }
            }
            Action::Add(kind) => add = Some(kind),
            Action::DeleteBlock(kind) => app.block_delete = Some(kind),
            Action::Move(kind, folder) => move_block(app, &kind, &folder),
            Action::NewBlock(folder) => {
                app.block_editor = Some(BlockEditor::new_block(app, &folder));
            }
            Action::Ask(p) => prompt = Some(p),
            Action::Import => crate::blockio::pick_files(app),
            Action::ImportProject => crate::blockio::pick_project(app),
            Action::Export(kind) => crate::blockio::export(app, &kind),
            Action::Example(kind) => crate::blockio::open_example(app, &kind),
            Action::DeleteFolder(path) => {
                if delete_folder(app, &path) {
                    selected = None;
                }
            }
        }
    }
    ui.data_mut(|d| {
        d.insert_temp(sel_id, selected);
        d.insert_temp(prompt_id, prompt);
    });
    add
}

/// Folder a new item goes into: the selected folder, or the folder of the
/// selected block, or the root.
fn current_folder(app: &EditorApp, selected: Option<&Selected>) -> String {
    match selected {
        Some(Selected::Folder(path)) => path.clone(),
        Some(Selected::Block(kind)) => app
            .lib
            .get(kind)
            .map(|b| b.def.info.group.clone())
            .unwrap_or_default(),
        None => String::new(),
    }
}

fn toolbar(app: &EditorApp, ui: &mut Ui, selected: Option<&Selected>, actions: &mut Vec<Action>) {
    let folder = current_folder(app, selected);
    ui.horizontal_wrapped(|ui| {
        if ui
            .small_button("+ Раздел")
            .on_hover_text("Новый раздел верхнего уровня")
            .clicked()
        {
            actions.push(Action::Ask(Prompt {
                target: String::new(),
                rename: false,
                name: String::new(),
            }));
        }
        let in_folder = !folder.is_empty();
        if ui
            .add_enabled(in_folder, egui::Button::new("+ Папка").small())
            .on_hover_text("Новая папка в выбранном разделе или папке")
            .on_disabled_hover_text("Сначала выберите раздел или папку")
            .clicked()
        {
            actions.push(Action::Ask(Prompt {
                target: folder.clone(),
                rename: false,
                name: String::new(),
            }));
        }
        if ui
            .small_button("+ Блок")
            .on_hover_text("Новый блок в выбранном разделе или папке")
            .clicked()
        {
            actions.push(Action::NewBlock(folder.clone()));
        }
        ui.menu_button("Импорт", |ui| {
            if ui.button("Из файла .flpblock...").clicked() {
                actions.push(Action::Import);
                ui.close();
            }
            if ui.button("Из проекта .flpn...").clicked() {
                actions.push(Action::ImportProject);
                ui.close();
            }
        });
        let (edit_hint, delete_hint) = match selected {
            Some(Selected::Block(_)) => ("Открыть блок в редакторе", "Удалить блок"),
            Some(Selected::Folder(_)) => ("Переименовать", "Удалить (только пустой)"),
            None => ("", ""),
        };
        if ui
            .add_enabled(selected.is_some(), egui::Button::new("Изменить").small())
            .on_hover_text(edit_hint)
            .clicked()
        {
            match selected {
                Some(Selected::Block(kind)) => actions.push(Action::Open(kind.clone())),
                Some(Selected::Folder(path)) => actions.push(Action::Ask(Prompt {
                    target: path.clone(),
                    rename: true,
                    name: leaf(path).to_owned(),
                })),
                None => {}
            }
        }
        if ui
            .add_enabled(selected.is_some(), egui::Button::new("Удалить").small())
            .on_hover_text(delete_hint)
            .clicked()
        {
            match selected {
                Some(Selected::Block(kind)) => actions.push(Action::DeleteBlock(kind.clone())),
                Some(Selected::Folder(path)) => actions.push(Action::DeleteFolder(path.clone())),
                None => {}
            }
        }
    });
}

fn tree(app: &EditorApp, ui: &mut Ui, selected: Option<&Selected>, actions: &mut Vec<Action>) {
    let folders: Vec<&str> = app.lib.user_folders().collect();
    egui::CollapsingHeader::new(RichText::new(ROOT).strong())
        .id_salt("userlib_root")
        .default_open(true)
        .show(ui, |ui| {
            folder_contents(app, ui, "", &folders, selected, actions);
            if folders.is_empty() && app.lib.iter().all(|b| !app.lib.is_user(&b.def.info.kind)) {
                ui.weak(
                    "Пусто. Создайте раздел или блок кнопками выше, либо блок на основе \
                     встроенного: правая кнопка на нём во вкладке «Встроенные».",
                );
            }
        });
}

/// Subfolders, then blocks of the folder `path`.
fn folder_contents(
    app: &EditorApp,
    ui: &mut Ui,
    path: &str,
    folders: &[&str],
    selected: Option<&Selected>,
    actions: &mut Vec<Action>,
) {
    for &sub in folders.iter().filter(|f| parent(f) == path) {
        let is_sel = selected == Some(&Selected::Folder(sub.to_owned()));
        let mut text = RichText::new(leaf(sub));
        if is_sel {
            text = text.strong().color(ui.visuals().selection.stroke.color);
        }
        let resp = egui::CollapsingHeader::new(text)
            .id_salt(("userlib_folder", sub))
            .show(ui, |ui| {
                folder_contents(app, ui, sub, folders, selected, actions)
            });
        let header = resp.header_response;
        if header.clicked() || header.secondary_clicked() {
            actions.push(Action::Select(Selected::Folder(sub.to_owned())));
        }
        header.context_menu(|ui| folder_menu(ui, sub, actions));
    }
    let mut blocks: Vec<_> = app
        .lib
        .iter()
        .filter(|b| app.lib.is_user(&b.def.info.kind) && b.def.info.group == path)
        .collect();
    blocks.sort_by(|a, b| a.def.info.name.cmp(&b.def.info.name));
    let folders_all: Vec<String> = folders.iter().map(|f| (*f).to_owned()).collect();
    for b in blocks {
        block_row(ui, b, &b.def.info.name, selected, &folders_all, actions);
    }
}

/// Search results: matching user blocks with their place in the tree.
fn found(
    app: &EditorApp,
    ui: &mut Ui,
    needle: &str,
    selected: Option<&Selected>,
    actions: &mut Vec<Action>,
) {
    let folders: Vec<String> = app.lib.user_folders().map(str::to_owned).collect();
    let mut any = false;
    for b in app.lib.iter().filter(|b| app.lib.is_user(&b.def.info.kind)) {
        let info = &b.def.info;
        let hay = format!("{} {} {}", info.kind, info.name, info.description).to_lowercase();
        if !hay.contains(needle) {
            continue;
        }
        any = true;
        let text = if info.group.is_empty() {
            info.name.clone()
        } else {
            format!("{}  — {}", info.name, info.group.replace('/', " / "))
        };
        block_row(ui, b, &text, selected, &folders, actions);
    }
    if !any {
        ui.weak("Ничего не найдено");
    }
}

fn block_row(
    ui: &mut Ui,
    b: &flp_core::LibraryBlock,
    text: &str,
    selected: Option<&Selected>,
    folders: &[String],
    actions: &mut Vec<Action>,
) {
    let kind = &b.def.info.kind;
    let is_sel = selected == Some(&Selected::Block(kind.clone()));
    let resp = ui
        .dnd_drag_source(
            Id::unique(("lib", kind.as_str())),
            DragItem::Block(kind.clone()),
            |ui| ui.selectable_label(is_sel, text),
        )
        .inner;
    let resp = resp.on_hover_text(&b.def.info.description);
    if resp.clicked() || resp.secondary_clicked() {
        actions.push(Action::Select(Selected::Block(kind.clone())));
    }
    if resp.double_clicked() {
        actions.push(Action::Add(kind.clone()));
    }
    resp.context_menu(|ui| {
        if ui.button("Изменить блок...").clicked() {
            actions.push(Action::Open(kind.clone()));
        }
        if ui.button("Новый блок на основе этого...").clicked() {
            actions.push(Action::Copy(kind.clone()));
        }
        ui.menu_button("Перенести в", |ui| {
            let group = &b.def.info.group;
            let targets = std::iter::once(String::new()).chain(folders.iter().cloned());
            for target in targets {
                let label = if target.is_empty() {
                    ROOT.to_owned()
                } else {
                    target.replace('/', " / ")
                };
                if ui
                    .add_enabled(&target != group, egui::Button::new(label))
                    .clicked()
                {
                    actions.push(Action::Move(kind.clone(), target));
                    ui.close();
                }
            }
        });
        if ui.button("Экспорт в файл...").clicked() {
            actions.push(Action::Export(kind.clone()));
        }
        let example = b.dir.join(flp_core::blockfile::EXAMPLE_FILE).is_file();
        if ui
            .add_enabled(example, egui::Button::new("Открыть пример"))
            .clicked()
        {
            actions.push(Action::Example(kind.clone()));
        }
        ui.separator();
        if ui.button("Удалить блок...").clicked() {
            actions.push(Action::DeleteBlock(kind.clone()));
        }
    });
}

fn folder_menu(ui: &mut Ui, path: &str, actions: &mut Vec<Action>) {
    if ui.button("Новый блок...").clicked() {
        actions.push(Action::NewBlock(path.to_owned()));
    }
    if ui.button("Новая папка...").clicked() {
        actions.push(Action::Ask(Prompt {
            target: path.to_owned(),
            rename: false,
            name: String::new(),
        }));
    }
    ui.separator();
    if ui.button("Переименовать...").clicked() {
        actions.push(Action::Ask(Prompt {
            target: path.to_owned(),
            rename: true,
            name: leaf(path).to_owned(),
        }));
    }
    if ui.button("Удалить").clicked() {
        actions.push(Action::DeleteFolder(path.to_owned()));
    }
}

/// Name dialog; Some(true) — OK, Some(false) — cancelled.
fn ask_name(app: &EditorApp, ctx: &egui::Context, p: &mut Prompt) -> Option<bool> {
    let title = match (p.rename, p.target.is_empty()) {
        (true, _) => format!("Переименовать {}", folder_word(&p.target)),
        (false, true) => "Новый раздел".to_owned(),
        (false, false) => format!("Новая папка в «{}»", leaf(&p.target)),
    };
    let valid = is_valid_folder_name(p.name.trim());
    let error = app.theme.palette.error;
    let mut answer = None;
    egui::Modal::new(Id::unique("userlib_prompt_modal")).show(ctx, |ui| {
        ui.label(RichText::new(title).strong());
        let edit = ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(260.0));
        edit.request_focus();
        if !p.name.trim().is_empty() && !valid {
            ui.colored_label(error, "Нельзя использовать символы < > : \" / \\ | ? *");
        }
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            if ui.add_enabled(valid, egui::Button::new("ОК")).clicked() || (enter && valid) {
                answer = Some(true);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    answer
}

fn folder_path(app: &EditorApp, path: &str) -> Option<PathBuf> {
    let user = app.user_lib_dir()?;
    Some(
        path.split('/')
            .filter(|p| !p.is_empty())
            .fold(user, |d, p| d.join(p)),
    )
}

/// Creates or renames a folder; returns whether the dialog may close.
fn apply_prompt(app: &mut EditorApp, p: &Prompt) -> bool {
    let name = p.name.trim();
    let Some(target) = folder_path(app, &p.target) else {
        return false;
    };
    let result = if p.rename {
        let new = target.with_file_name(name);
        if new == target {
            return true;
        }
        if new.exists() {
            Err(format!("«{name}» здесь уже есть"))
        } else {
            fs::rename(&target, &new).map_err(|e| e.to_string())
        }
    } else {
        let new = target.join(name);
        if new.exists() {
            Err(format!("«{name}» здесь уже есть"))
        } else {
            fs::create_dir_all(&new).map_err(|e| e.to_string())
        }
    };
    match result {
        Ok(()) => {
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(LogLevel::Error, format!("Папка не изменена: {e}"));
            false
        }
    }
}

/// Deletes an empty section or folder; returns whether it was deleted.
fn delete_folder(app: &mut EditorApp, path: &str) -> bool {
    let Some(dir) = folder_path(app, path) else {
        return false;
    };
    let word = folder_word(path);
    let empty = fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none());
    if !empty {
        app.info(
            LogLevel::Error,
            format!(
                "{} «{}» не пуст(а): сначала удалите или перенесите блоки и папки из него",
                if word == "раздел" {
                    "Раздел"
                } else {
                    "Папка"
                },
                leaf(path)
            ),
        );
        return false;
    }
    match fs::remove_dir(&dir) {
        Ok(()) => {
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(LogLevel::Error, format!("Не удалось удалить {word}: {e}"));
            false
        }
    }
}

/// Moves a user block into another section or folder (`""` — the root).
fn move_block(app: &mut EditorApp, kind: &str, folder: &str) {
    let Some(block) = app.lib.get(kind).filter(|_| app.lib.is_user(kind)).cloned() else {
        return;
    };
    let Some(target) = folder_path(app, folder) else {
        return;
    };
    let dest = target.join(kind);
    if dest.exists() {
        app.info(
            LogLevel::Error,
            format!("Блок не перенесён: {} уже есть", dest.display()),
        );
        return;
    }
    let result = fs::create_dir_all(&target).and_then(|()| fs::rename(&block.dir, &dest));
    match result {
        Ok(()) => app.reload_library(),
        Err(e) => app.info(LogLevel::Error, format!("Блок не перенесён: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(leaf("A/B/C"), "C");
        assert_eq!(parent("A/B/C"), "A/B");
        assert_eq!(parent("A"), "");
        assert_eq!(folder_word("A"), "раздел");
        assert_eq!(folder_word("A/B"), "папка");
    }
}
