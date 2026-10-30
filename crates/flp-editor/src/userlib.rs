//! "Пользовательские" tab of the block library: the tree «Блоки пользователей» →
//! packs → folders → blocks. A pack is a top-level folder of `user_library/` with a
//! `pack.toml` (one version for all its blocks); a block's place in the tree is its group.

use std::fs;
use std::path::PathBuf;

use eframe::egui::{self, Id, RichText, Ui};
use flp_core::library::is_valid_folder_name;
use flp_core::pack::{self, PackInfo};

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

/// Dialog of a new pack, or of the properties of an existing one.
#[derive(Debug, Clone, Default)]
struct PackForm {
    /// Folder of the pack being edited; `None` — a new pack.
    edit: Option<String>,
    name: String,
    description: String,
    authors: String,
    /// One contact per line: e-mail, link, social network.
    contacts: String,
    license: String,
    version: u32,
    error: String,
}

impl PackForm {
    fn new() -> Self {
        let author = crate::theme::author();
        Self {
            authors: author.name,
            contacts: author.contacts.join("\n"),
            version: 1,
            ..Self::default()
        }
    }

    fn of(folder: &str, info: &PackInfo) -> Self {
        Self {
            edit: Some(folder.to_owned()),
            name: info.name.clone(),
            description: info.description.clone(),
            authors: info.authors.join(", "),
            contacts: info.contacts.join("\n"),
            license: info.license.clone(),
            version: info.version,
            error: String::new(),
        }
    }

    fn contacts(&self) -> Vec<String> {
        self.contacts
            .lines()
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty())
            .collect()
    }

    fn authors(&self) -> Vec<String> {
        self.authors
            .split(',')
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty())
            .collect()
    }
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
    NewPack,
    PackProps(String),
    ExportPack(String),
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
        crate::i18n::tr("папка")
    } else {
        crate::i18n::tr("раздел")
    }
}

/// The tab; `needle` is the lower-case search text. Returns a block type to add
/// to the schema (double click).
pub fn show(app: &mut EditorApp, ui: &mut Ui, needle: &str) -> Option<String> {
    let sel_id = Id::unique("userlib_selected");
    let prompt_id = Id::unique("userlib_prompt");
    let form_id = Id::unique("userlib_pack_form");
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

    let mut form: Option<PackForm> = ui.data_mut(|d| d.get_temp(form_id)).flatten();
    if let Some(f) = &mut form {
        match pack_form(app, ui.ctx(), f) {
            Some(true) => {
                if apply_pack_form(app, f) {
                    form = None;
                }
            }
            Some(false) => form = None,
            None => {}
        }
    }

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
            Action::NewPack => form = Some(PackForm::new()),
            Action::PackProps(folder) => {
                let info = app
                    .user_lib_dir()
                    .and_then(|d| PackInfo::read(&d.join(&folder)).ok());
                if let Some(info) = info {
                    form = Some(PackForm::of(&folder, &info));
                }
            }
            Action::ExportPack(folder) => crate::blockio::export_pack(app, &folder),
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
        d.insert_temp(form_id, form);
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
            .small_button(crate::i18n::tr("+ Набор"))
            .on_hover_text(crate::i18n::tr(
                "Новый набор блоков: у всех его блоков одна версия",
            ))
            .clicked()
        {
            actions.push(Action::NewPack);
        }
        let in_folder = !folder.is_empty();
        if ui
            .add_enabled(
                in_folder,
                egui::Button::new(crate::i18n::tr("+ Папка")).small(),
            )
            .on_hover_text(crate::i18n::tr("Новая папка в выбранном наборе или папке"))
            .on_disabled_hover_text(crate::i18n::tr("Сначала выберите набор или папку"))
            .clicked()
        {
            actions.push(Action::Ask(Prompt {
                target: folder.clone(),
                rename: false,
                name: String::new(),
            }));
        }
        if ui
            .small_button(crate::i18n::tr("+ Блок"))
            .on_hover_text(crate::i18n::tr("Новый блок в выбранном наборе или папке"))
            .clicked()
        {
            actions.push(Action::NewBlock(folder.clone()));
        }
        ui.menu_button(crate::i18n::tr("Импорт"), |ui| {
            if ui
                .button(crate::i18n::tr("Набор из файла .crabblocks..."))
                .clicked()
            {
                actions.push(Action::Import);
                ui.close();
            }
            if ui.button(crate::i18n::tr("Из проекта .flpn...")).clicked() {
                actions.push(Action::ImportProject);
                ui.close();
            }
        });
        let (edit_hint, delete_hint) = match selected {
            Some(Selected::Block(_)) => (
                crate::i18n::tr("Открыть блок в редакторе"),
                crate::i18n::tr("Удалить блок"),
            ),
            Some(Selected::Folder(_)) => (
                crate::i18n::tr("Переименовать"),
                crate::i18n::tr("Удалить (только пустой)"),
            ),
            None => ("", ""),
        };
        if ui
            .add_enabled(
                selected.is_some(),
                egui::Button::new(crate::i18n::tr("Изменить")).small(),
            )
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
            .add_enabled(
                selected.is_some(),
                egui::Button::new(crate::i18n::tr("Удалить")).small(),
            )
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
                ui.weak(crate::i18n::tr(
                    "Пусто. Создайте раздел или блок кнопками выше, либо блок на основе \
                     встроенного: правая кнопка на нём во вкладке «Встроенные».",
                ));
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
        let pack = if path.is_empty() {
            app.user_lib_dir()
                .and_then(|d| PackInfo::read(&d.join(sub)).ok())
        } else {
            None
        };
        let label = match &pack {
            Some(info) => format!("{} · v{}", leaf(sub), info.version),
            None => leaf(sub).to_owned(),
        };
        let mut text = RichText::new(label);
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
        if let Some(info) = &pack {
            header
                .clone()
                .on_hover_ui(|ui| crate::blockio::pack_credits(ui, info));
        }
        header.context_menu(|ui| folder_menu(ui, sub, pack.is_some(), actions));
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
        ui.weak(crate::i18n::tr("Ничего не найдено"));
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
    let resp = crate::canvas::drag_source(
        ui,
        Id::unique(("lib", kind.as_str())),
        DragItem::Block(kind.clone()),
        |ui| ui.selectable_label(is_sel, text),
    );
    let resp = resp.on_hover_text(&b.def.info.description);
    if resp.clicked() || resp.secondary_clicked() {
        actions.push(Action::Select(Selected::Block(kind.clone())));
    }
    if resp.double_clicked() {
        actions.push(Action::Add(kind.clone()));
    }
    resp.context_menu(|ui| {
        if ui.button(crate::i18n::tr("Изменить блок...")).clicked() {
            actions.push(Action::Open(kind.clone()));
        }
        if ui
            .button(crate::i18n::tr("Новый блок на основе этого..."))
            .clicked()
        {
            actions.push(Action::Copy(kind.clone()));
        }
        ui.menu_button(crate::i18n::tr("Перенести в"), |ui| {
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
        let example = b.dir.join(flp_core::blockfile::EXAMPLE_FILE).is_file();
        if ui
            .add_enabled(
                example,
                egui::Button::new(crate::i18n::tr("Открыть пример")),
            )
            .clicked()
        {
            actions.push(Action::Example(kind.clone()));
        }
        ui.separator();
        if ui.button(crate::i18n::tr("Удалить блок...")).clicked() {
            actions.push(Action::DeleteBlock(kind.clone()));
        }
    });
}

fn folder_menu(ui: &mut Ui, path: &str, is_pack: bool, actions: &mut Vec<Action>) {
    if is_pack {
        if ui.button(crate::i18n::tr("Свойства набора...")).clicked() {
            actions.push(Action::PackProps(path.to_owned()));
            ui.close();
        }
        if ui.button(crate::i18n::tr("Экспорт набора...")).clicked() {
            actions.push(Action::ExportPack(path.to_owned()));
            ui.close();
        }
        ui.separator();
    }
    if ui.button(crate::i18n::tr("Новый блок...")).clicked() {
        actions.push(Action::NewBlock(path.to_owned()));
    }
    if ui.button(crate::i18n::tr("Новая папка...")).clicked() {
        actions.push(Action::Ask(Prompt {
            target: path.to_owned(),
            rename: false,
            name: String::new(),
        }));
    }
    ui.separator();
    if ui.button(crate::i18n::tr("Переименовать...")).clicked() {
        actions.push(Action::Ask(Prompt {
            target: path.to_owned(),
            rename: true,
            name: leaf(path).to_owned(),
        }));
    }
    if ui.button(crate::i18n::tr("Удалить")).clicked() {
        actions.push(Action::DeleteFolder(path.to_owned()));
    }
}

/// Name dialog; Some(true) — OK, Some(false) — cancelled.
fn ask_name(app: &EditorApp, ctx: &egui::Context, p: &mut Prompt) -> Option<bool> {
    let title = match (p.rename, p.target.is_empty()) {
        (true, _) => crate::i18n::fmt("Переименовать {0}", &[&folder_word(&p.target)]),
        (false, true) => crate::i18n::tr("Новый раздел").to_owned(),
        (false, false) => crate::i18n::fmt("Новая папка в «{0}»", &[&leaf(&p.target)]),
    };
    let valid = is_valid_folder_name(p.name.trim());
    let error = app.theme.palette.error;
    let mut answer = None;
    egui::Modal::new(Id::unique("userlib_prompt_modal")).show(ctx, |ui| {
        ui.label(RichText::new(title).strong());
        let edit = ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(260.0));
        edit.request_focus();
        if !p.name.trim().is_empty() && !valid {
            ui.colored_label(
                error,
                crate::i18n::tr("Нельзя использовать символы < > : \" / \\ | ? *"),
            );
        }
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(valid, egui::Button::new(crate::i18n::tr("ОК")))
                .clicked()
                || (enter && valid)
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
    answer
}

/// Pack dialog; Some(true) — OK, Some(false) — cancelled.
fn pack_form(app: &EditorApp, ctx: &egui::Context, f: &mut PackForm) -> Option<bool> {
    let editing = f.edit.is_some();
    let valid = editing || is_valid_folder_name(f.name.trim());
    let error = app.theme.palette.error;
    let mut answer = None;
    // The dialog takes a quarter of the screen: half the width and half the height.
    let screen = ctx.content_rect().size();
    let field_width = (screen.x / 2.0 - 140.0).max(200.0);
    egui::Modal::new(Id::unique("userlib_pack_modal")).show(ctx, |ui| {
        let title = if editing {
            crate::i18n::tr("Свойства набора")
        } else {
            crate::i18n::tr("Новый набор блоков")
        };
        ui.set_width(screen.x / 2.0 - 24.0);
        ui.set_min_height(screen.y / 2.0 - 24.0);
        ui.label(RichText::new(title).strong());
        egui::Grid::new("pack_form").num_columns(2).show(ui, |ui| {
            ui.label(crate::i18n::tr("Название"));
            if editing {
                ui.label(crate::i18n::fmt("{0} · версия {1}", &[&f.name, &f.version]));
            } else {
                ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(field_width));
            }
            ui.end_row();
            ui.label(crate::i18n::tr("Описание"));
            ui.add(egui::TextEdit::singleline(&mut f.description).desired_width(field_width));
            ui.end_row();
            ui.label(crate::i18n::tr("Автор"));
            ui.add(egui::TextEdit::singleline(&mut f.authors).desired_width(field_width));
            ui.end_row();
            ui.label(crate::i18n::tr("Контакты"));
            ui.add(
                egui::TextEdit::multiline(&mut f.contacts)
                    .desired_rows(5)
                    .desired_width(field_width)
                    .hint_text(crate::i18n::tr(
                        "почта, ссылка или соцсеть — по одной в строке",
                    )),
            );
            ui.end_row();
            ui.label(crate::i18n::tr("Лицензия"));
            ui.add(egui::TextEdit::singleline(&mut f.license).desired_width(field_width));
            ui.end_row();
        });
        if !valid && !f.name.trim().is_empty() {
            ui.colored_label(
                error,
                crate::i18n::tr("Нельзя использовать символы < > : \" / \\ | ? *"),
            );
        }
        if !f.error.is_empty() {
            ui.colored_label(error, &f.error);
        }
        let ok = valid && !f.name.trim().is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(ok, egui::Button::new(crate::i18n::tr("ОК")))
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
    answer
}

/// Creates the pack or saves its properties; returns whether the dialog may close.
/// The entered author is remembered for the next packs.
fn apply_pack_form(app: &mut EditorApp, f: &mut PackForm) -> bool {
    let Some(user) = app.user_lib_dir() else {
        f.error = crate::i18n::tr("Нет папки библиотеки блоков").to_owned();
        return false;
    };
    let result = match &f.edit {
        Some(folder) => {
            let dir = user.join(folder);
            PackInfo::read(&dir).and_then(|mut info| {
                info.description = f.description.trim().to_owned();
                info.authors = f.authors();
                info.contacts = f.contacts();
                info.license = f.license.trim().to_owned();
                info.write(&dir)
            })
        }
        None => {
            let mut info = PackInfo::new(f.name.trim());
            info.description = f.description.trim().to_owned();
            info.authors = f.authors();
            info.contacts = f.contacts();
            info.license = f.license.trim().to_owned();
            pack::create(&user, &info).map(|_| ())
        }
    };
    match result {
        Ok(()) => {
            crate::theme::save_author(&crate::theme::Author {
                name: f.authors.trim().to_owned(),
                contacts: f.contacts(),
            });
            app.reload_library();
            true
        }
        Err(e) => {
            f.error = e;
            false
        }
    }
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
            Err(crate::i18n::fmt("«{0}» здесь уже есть", &[&name]))
        } else {
            fs::rename(&target, &new).map_err(|e| e.to_string())
        }
    } else {
        let new = target.join(name);
        if new.exists() {
            Err(crate::i18n::fmt("«{0}» здесь уже есть", &[&name]))
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
            app.info(
                LogLevel::Error,
                crate::i18n::fmt("Папка не изменена: {0}", &[&e]),
            );
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
            crate::i18n::fmt(
                "{0} «{1}» не пуст(а): сначала удалите или перенесите блоки и папки из него",
                &[
                    &if word == "раздел" {
                        "Раздел"
                    } else {
                        "Папка"
                    },
                    &leaf(path),
                ],
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
            app.info(
                LogLevel::Error,
                crate::i18n::fmt("Не удалось удалить {0}: {1}", &[&word, &e]),
            );
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
            crate::i18n::fmt("Блок не перенесён: {0} уже есть", &[&dest.display()]),
        );
        return;
    }
    let result = fs::create_dir_all(&target).and_then(|()| fs::rename(&block.dir, &dest));
    match result {
        Ok(()) => app.reload_library(),
        Err(e) => app.info(
            LogLevel::Error,
            crate::i18n::fmt("Блок не перенесён: {0}", &[&e]),
        ),
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
