//! "Block editor" window: create a library block or change one — header, inputs,
//! outputs, parameters, the ESP32 code template and docs — with a live picture of
//! the block and a trial generation of its code.

use std::fs;
use std::path::PathBuf;

use eframe::egui::{self, Color32, Id, RichText, ScrollArea, TextEdit, Ui};
use flp_core::ident::ident_from_name;
use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{
    BlockDef, Choice, LibraryBlock, ParamDef, ParamKind, ParamShow, ParamValue, PinMode, PortDef,
    SignalType,
};

use crate::app::{EditorApp, LogLevel};
use crate::dialogs::{type_name, value_text};
use crate::theme::Palette;

const PARAM_KINDS: [ParamKind; 6] = [
    ParamKind::Bool,
    ParamKind::Int,
    ParamKind::Float,
    ParamKind::Text,
    ParamKind::Duration,
    ParamKind::Pin,
];

const NEW_TEMPLATE: &str =
    "// @global\n\n// @setup\n\n// @loop\n{{ output.out }} = {{ input.in }};\n";

const SECTIONS_HINT: &str = "Разделы: // @global — переменные блока, // @setup — код в setup(), \
// @loop — код цикла, // @functions — функции. С пометкой once (// @global once) раздел \
попадает в скетч один раз на все блоки. Имена в разделах без once начинайте с {{ id }}.";

fn show_label(show: ParamShow) -> &'static str {
    match show {
        ParamShow::Named => "имя=значение",
        ParamShow::Value => "значение",
        ParamShow::Secret => "пароль (p***1)",
        ParamShow::Hidden => "не показывать",
    }
}

fn kind_label(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::Bool => "bool — да/нет",
        ParamKind::Int => "int — целое",
        ParamKind::Float => "float — дробное",
        ParamKind::Text => "string — строка",
        ParamKind::Duration => "duration — время",
        ParamKind::Pin => "pin — вывод",
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PortRow {
    name: String,
    /// Caption in Russian; empty: the name is shown.
    label: String,
    ty: SignalType,
    /// Empty: no default (inputs only).
    default: String,
    when: String,
    description: String,
}

impl PortRow {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            label: String::new(),
            ty: SignalType::Bool,
            default: String::new(),
            when: String::new(),
            description: String::new(),
        }
    }

    fn from_def(p: &PortDef) -> Self {
        Self {
            name: p.name.clone(),
            label: p.label.clone().unwrap_or_default(),
            ty: p.ty,
            default: p.default.as_ref().map(value_text).unwrap_or_default(),
            when: p.when.clone().unwrap_or_default(),
            description: p.description.clone(),
        }
    }

    fn to_def(&self) -> Result<PortDef, String> {
        Ok(PortDef {
            name: self.name.trim().to_owned(),
            label: optional(&self.label),
            ty: self.ty,
            default: parse_signal(self.ty, &self.default)
                .map_err(|e| format!("«{}»: {e}", self.name))?,
            when: optional(&self.when),
            description: self.description.trim().to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ParamRow {
    name: String,
    label: String,
    kind: ParamKind,
    default: String,
    min: String,
    max: String,
    when: String,
    description: String,
    show: ParamShow,
    /// `local` and `[[param.choice]]` from `block.toml`, kept as is (no column for them).
    local: Option<String>,
    choices: Vec<Choice>,
    /// For pins: direction and sharing, edited next to the type.
    mode: PinMode,
    shared: bool,
}

impl ParamRow {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            label: String::new(),
            kind: ParamKind::Int,
            default: String::new(),
            min: String::new(),
            max: String::new(),
            when: String::new(),
            description: String::new(),
            show: ParamShow::Named,
            local: None,
            choices: Vec::new(),
            mode: PinMode::Io,
            shared: false,
        }
    }

    fn from_def(p: &ParamDef) -> Self {
        let num = |v: Option<f64>| v.map(|v| format!("{v}")).unwrap_or_default();
        Self {
            name: p.name.clone(),
            label: p.label.clone().unwrap_or_default(),
            kind: p.kind,
            default: p.default.as_ref().map(value_text).unwrap_or_default(),
            min: num(p.min),
            max: num(p.max),
            when: p.when.clone().unwrap_or_default(),
            description: p.description.clone(),
            show: p.show,
            local: p.local.clone(),
            choices: p.choices.clone(),
            mode: p.mode,
            shared: p.shared,
        }
    }

    fn to_def(&self) -> Result<ParamDef, String> {
        let err = |e: String| format!("«{}»: {e}", self.name);
        Ok(ParamDef {
            name: self.name.trim().to_owned(),
            label: optional(&self.label),
            kind: self.kind,
            default: parse_param(self.kind, &self.default).map_err(err)?,
            min: parse_number(&self.min).map_err(err)?,
            max: parse_number(&self.max).map_err(err)?,
            when: optional(&self.when),
            description: self.description.trim().to_owned(),
            show: self.show,
            local: self.local.clone(),
            mode: if self.kind == ParamKind::Pin {
                self.mode
            } else {
                PinMode::Io
            },
            shared: self.kind == ParamKind::Pin && self.shared,
            choices: self.choices.clone(),
        })
    }
}

/// Everything the window edits; compared with the saved copy to detect changes.
#[derive(Debug, Clone, PartialEq)]
struct Form {
    /// Type of the block: made from the name for a new block, fixed afterwards
    /// (schemas refer to it).
    kind: String,
    name: String,
    symbol: String,
    /// Section and folders in the user library (`Раздел/Папка`), empty in the root.
    group: String,
    description: String,
    memory: bool,
    single: bool,
    /// `[block.uart]`: kept as it is, the form has no field for it yet.
    uart: Option<flp_core::UartUse>,
    /// Comma-separated.
    includes: String,
    libraries: String,
    inputs: Vec<PortRow>,
    outputs: Vec<PortRow>,
    params: Vec<ParamRow>,
    template: String,
    docs: String,
}

impl Form {
    fn new_block(kind: String, group: String) -> Self {
        Self {
            kind,
            name: "Новый блок".into(),
            symbol: String::new(),
            group,
            description: String::new(),
            memory: false,
            single: false,
            uart: None,
            includes: String::new(),
            libraries: String::new(),
            inputs: vec![PortRow::new("in")],
            outputs: vec![PortRow::new("out")],
            params: Vec::new(),
            template: NEW_TEMPLATE.into(),
            docs: String::new(),
        }
    }

    fn from_block(block: &LibraryBlock) -> Self {
        let def = &block.def;
        let info = &def.info;
        let template = block
            .template(ESP32_CPP_TEMPLATE)
            .and_then(Result::ok)
            .unwrap_or_default();
        let docs = block
            .docs_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        Self {
            kind: info.kind.clone(),
            name: info.name.clone(),
            symbol: info.symbol.clone().unwrap_or_default(),
            group: info.group.clone(),
            description: info.description.clone(),
            memory: info.memory,
            single: info.single,
            uart: info.uart.clone(),
            includes: info.includes.join(", "),
            libraries: info.libraries.join(", "),
            inputs: def.inputs.iter().map(PortRow::from_def).collect(),
            outputs: def.outputs.iter().map(PortRow::from_def).collect(),
            params: def.params.iter().map(ParamRow::from_def).collect(),
            template,
            docs,
        }
    }

    /// Block definition from the form, checked the same way the library checks it.
    fn to_def(&self) -> Result<BlockDef, String> {
        let kind = self.kind.trim().to_owned();
        if self.name.trim().is_empty() {
            return Err("у блока должно быть название".into());
        }
        if kind == VAR_GET || kind == VAR_SET {
            return Err(format!("тип «{kind}» занят встроенным блоком переменной"));
        }
        let def = BlockDef {
            info: flp_core::library::BlockInfo {
                kind: kind.clone(),
                name: self.name.trim().to_owned(),
                symbol: optional(&self.symbol),
                group: self.group.trim().to_owned(),
                description: self.description.trim().to_owned(),
                memory: self.memory,
                includes: list(&self.includes),
                libraries: list(&self.libraries),
                single: self.single,
                uart: self.uart.clone(),
            },
            inputs: self
                .inputs
                .iter()
                .map(PortRow::to_def)
                .collect::<Result<_, _>>()?,
            outputs: self
                .outputs
                .iter()
                .map(PortRow::to_def)
                .collect::<Result<_, _>>()?,
            params: self
                .params
                .iter()
                .map(ParamRow::to_def)
                .collect::<Result<_, _>>()?,
        };
        def.validate(&kind)?;
        Ok(def)
    }
}

fn optional(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_bool(t: &str) -> Option<bool> {
    match t {
        "true" | "1" | "да" => Some(true),
        "false" | "0" | "нет" => Some(false),
        _ => None,
    }
}

fn parse_float(t: &str) -> Option<f64> {
    t.replace(',', ".").parse().ok()
}

/// Default of a port; empty means "no default".
fn parse_signal(ty: SignalType, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let value = match ty {
        SignalType::Bool => parse_bool(t).map(ParamValue::Bool),
        SignalType::Int => t.parse().ok().map(ParamValue::Int),
        SignalType::Float => parse_float(t).map(ParamValue::Float),
        SignalType::String => Some(ParamValue::Text(t.to_owned())),
        SignalType::Any => Some(SignalType::guess_value(t)),
    };
    value
        .map(Some)
        .ok_or_else(|| format!("«{t}» не подходит к типу {}", type_name(ty)))
}

/// Default of a parameter; empty means "no default" (the parameter is required).
fn parse_param(kind: ParamKind, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let value = match kind {
        ParamKind::Bool => parse_bool(t).map(ParamValue::Bool),
        ParamKind::Int | ParamKind::Pin => t.parse().ok().map(ParamValue::Int),
        ParamKind::Float => parse_float(t).map(ParamValue::Float),
        // Durations are checked by the definition itself.
        ParamKind::Text | ParamKind::Duration => Some(ParamValue::Text(t.to_owned())),
    };
    value
        .map(Some)
        .ok_or_else(|| format!("«{t}» не подходит к виду «{}»", kind_label(kind)))
}

fn parse_number(text: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    parse_float(t)
        .map(Some)
        .ok_or_else(|| format!("граница «{t}» — не число"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Ports,
    Params,
    Code,
    Docs,
    Sketch,
}

/// Trial generation for one state of the form.
struct Generated {
    def: BlockDef,
    template: String,
    result: Result<String, String>,
}

pub struct BlockEditor {
    /// Type the form was loaded from; `None` for a block not saved yet.
    original: Option<String>,
    form: Form,
    /// The form as last loaded or saved.
    clean: Form,
    tab: Tab,
    generated: Option<Generated>,
    /// Closing with unsaved changes: asking what to do.
    confirm_close: bool,
    /// A built-in block: shown, but cannot be changed.
    read_only: bool,
}

impl BlockEditor {
    /// A new block in a section or folder of the user library (`""` — the root).
    pub fn new_block(app: &EditorApp, group: &str) -> Self {
        let mut form = Form::new_block(String::new(), group.to_owned());
        form.kind = free_kind(app, &ident_from_name(&form.name));
        Self::with(None, form)
    }

    /// Opens a library block: a user block for editing, a built-in one read-only.
    pub fn edit(app: &EditorApp, block: &LibraryBlock) -> Self {
        let kind = block.def.info.kind.clone();
        let read_only = !app.lib.is_user(&kind);
        Self {
            read_only,
            ..Self::with(Some(kind), Form::from_block(block))
        }
    }

    /// A new block based on an existing one.
    /// A copy of a user block lands next to it, of a built-in one in the root.
    pub fn copy_of(app: &EditorApp, block: &LibraryBlock) -> Self {
        let mut form = Form::from_block(block);
        if !app.lib.is_user(&block.def.info.kind) {
            form.group.clear();
        }
        form.name = format!("{} (копия)", form.name);
        form.kind = free_kind(app, &ident_from_name(&form.name));
        Self::with(None, form)
    }

    fn with(original: Option<String>, form: Form) -> Self {
        Self {
            original,
            clean: form.clone(),
            form,
            tab: Tab::Ports,
            generated: None,
            confirm_close: false,
            read_only: false,
        }
    }

    fn dirty(&self) -> bool {
        !self.read_only && self.form != self.clean
    }

    /// Regenerates the trial sketch when the definition or template changed.
    fn update_generated(&mut self, def: &BlockDef) {
        let fresh = self
            .generated
            .as_ref()
            .is_some_and(|g| g.def == *def && g.template == self.form.template);
        if !fresh {
            let result =
                flp_codegen::preview_block(def, &self.form.template).map_err(|e| e.to_string());
            self.generated = Some(Generated {
                def: def.clone(),
                template: self.form.template.clone(),
                result,
            });
        }
    }
}

/// A type not used by any block, made from `base`.
fn free_kind(app: &EditorApp, base: &str) -> String {
    let taken = |k: &str| app.lib.get(k).is_some() || k == VAR_GET || k == VAR_SET;
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|k| !taken(k))
        .expect("some name is free")
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut ed) = app.block_editor.take() else {
        return;
    };
    let title = match &ed.original {
        Some(kind) if ed.read_only => format!("Встроенный блок — {kind} (только просмотр)"),
        Some(kind) => format!("Редактор блока — {kind}"),
        None => "Редактор блока — новый блок".to_owned(),
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new(title)
        .id(Id::unique("block_editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(900.0)
        .default_height(640.0)
        .show(ctx, |ui| close = body(app, ui, &mut ed));
    if !open && ed.dirty() {
        ed.confirm_close = true;
    } else if !open {
        close = true;
    }
    if ed.confirm_close && !close {
        close = confirm_close(app, ctx, &mut ed);
    }
    if !close {
        app.block_editor = Some(ed);
    }
}

/// Modal question on closing with unsaved changes; returns true when the window
/// should close (saved or changes dropped).
fn confirm_close(app: &mut EditorApp, ctx: &egui::Context, ed: &mut BlockEditor) -> bool {
    let def = ed.form.to_def();
    let name = ed.form.name.trim().to_owned();
    let error = app.theme.palette.error;
    let mut answer = None;
    egui::Modal::new(Id::unique("block_editor_close")).show(ctx, |ui| {
        ui.label(RichText::new("Сохранить изменения?").strong());
        ui.label(format!("В блоке «{name}» есть несохранённые изменения."));
        if let Err(e) = &def {
            ui.colored_label(error, format!("Сохранить нельзя: {e}"));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(def.is_ok(), egui::Button::new("Сохранить"))
                .clicked()
            {
                answer = Some(1);
            }
            if ui.button("Выйти без сохранения").clicked() {
                answer = Some(2);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(0);
            }
        });
    });
    match answer {
        Some(1) => {
            let saved = save(app, ed, &def);
            ed.confirm_close = saved;
            saved
        }
        Some(2) => true,
        Some(_) => {
            ed.confirm_close = false;
            false
        }
        None => false,
    }
}

/// Window contents; returns true when the window should close.
fn body(app: &mut EditorApp, ui: &mut Ui, ed: &mut BlockEditor) -> bool {
    let pal = app.theme.palette;
    // Until the first save the type follows the name.
    if ed.original.is_none() {
        let kind = free_kind(app, &ident_from_name(&ed.form.name));
        ed.clean.kind.clone_from(&kind);
        ed.form.kind = kind;
    }
    let def = ed.form.to_def();
    if let Ok(def) = &def {
        ed.update_generated(def);
    }

    // Panels inside the window: the footer stays visible and the tab gets the rest.
    let mut close = false;
    egui::Panel::bottom("block_editor_footer").show(ui, |ui| {
        status(ui, ed, &def, &pal);
        close = footer(app, ui, ed, &def);
    });
    egui::Panel::top("block_editor_header").show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Вид на плате").strong());
                match &def {
                    Ok(def) => crate::canvas::block_preview(ui, def, &pal),
                    Err(_) => {
                        ui.weak("(исправьте ошибки описания)");
                    }
                }
            });
            ui.add_space(16.0);
            let editable = !ed.read_only;
            ui.vertical(|ui| ui.add_enabled_ui(editable, |ui| header(ui, ed)));
        });
        ui.separator();
        ui.horizontal(|ui| {
            let tabs = [
                (Tab::Ports, "Входы и выходы"),
                (Tab::Params, "Параметры"),
                (Tab::Code, "Код ESP32"),
                (Tab::Docs, "Описание"),
                (Tab::Sketch, "Пробная генерация"),
            ];
            for (tab, text) in tabs {
                ui.selectable_value(&mut ed.tab, tab, text);
            }
        });
    });
    let editable = !ed.read_only;
    egui::CentralPanel::default().show(ui, |ui| {
        ScrollArea::both()
            .id_salt("block_editor_tab")
            .auto_shrink([false, false])
            .show(ui, |ui| match ed.tab {
                Tab::Ports => {
                    ui.add_enabled_ui(editable, |ui| ports_tab(ui, &mut ed.form));
                }
                Tab::Params => {
                    ui.add_enabled_ui(editable, |ui| params_tab(ui, &mut ed.form));
                }
                Tab::Code => code_tab(ui, &mut ed.form, editable),
                Tab::Docs => {
                    ui.weak("docs.md: пояснение и пример. По нему ИИ-помощник подбирает блоки.");
                    ui.weak(
                        "Разметка Markdown: # заголовок, **жирный**, *курсив*, `код`, \
                         - список, 1. список, > цитата, ``` блок кода ```, | таблица |.",
                    );
                    ui.columns(2, |cols| {
                        cols[0].add(
                            TextEdit::multiline(&mut ed.form.docs)
                                .interactive(editable)
                                .desired_width(f32::INFINITY)
                                .desired_rows(20),
                        );
                        cols[1].label(RichText::new("Предпросмотр").weak());
                        crate::markdown::show(&mut cols[1], &ed.form.docs);
                    });
                }
                Tab::Sketch => sketch_tab(ui, ed, &pal),
            });
    });
    close
}

fn header(ui: &mut Ui, ed: &mut BlockEditor) {
    let read_only = ed.read_only;
    let f = &mut ed.form;
    egui::Grid::new("block_header")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label("Название");
            ui.add_sized([220.0, 20.0], TextEdit::singleline(&mut f.name))
                .on_hover_text(format!(
                    "Тип блока в схеме: {}. У нового блока он берётся из названия, \
                     после сохранения не меняется.",
                    f.kind
                ));
            ui.label("Обозначение");
            ui.add_sized(
                [160.0, 20.0],
                TextEdit::singleline(&mut f.symbol).hint_text("как название"),
            )
            .on_hover_text("Короткая надпись на блоке: TON, a + b, &");
            ui.end_row();

            ui.label("Где лежит");
            let place = if read_only {
                format!("Встроенные / {}", f.group)
            } else if f.group.is_empty() {
                "Блоки пользователей".to_owned()
            } else {
                format!("Блоки пользователей / {}", f.group.replace('/', " / "))
            };
            ui.label(place).on_hover_text(
                "Раздел и папка в библиотеке. Перенести блок — правая кнопка на нём \
                 во вкладке «Пользовательские».",
            );
            ui.end_row();

            ui.label("Описание");
            ui.add_sized([160.0, 20.0], TextEdit::singleline(&mut f.description));
            ui.label("");
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.memory, "Хранит состояние")
                    .on_hover_text(
                        "Блок помнит значения между циклами (таймеры, триггеры, счётчики); \
                         через такой блок допустимы обратные связи",
                    );
                ui.checkbox(&mut f.single, "Один на проект")
                    .on_hover_text("В проекте может быть только один такой блок (Wi-Fi, NTP)");
            });
            ui.end_row();

            ui.label("Заголовки");
            ui.add_sized(
                [160.0, 20.0],
                TextEdit::singleline(&mut f.includes).hint_text("DHT.h, Wire.h"),
            )
            .on_hover_text("Файлы для #include через запятую");
            ui.label("Библиотеки");
            ui.add_sized(
                [220.0, 20.0],
                TextEdit::singleline(&mut f.libraries).hint_text("DHT sensor library"),
            )
            .on_hover_text(
                "Библиотеки arduino-cli через запятую, можно с версией: OneWire@2.3.8. \
                 Сборка ставит недостающие сама.",
            );
            ui.end_row();
        });
}

/// Move, delete buttons of a table row; returns the action chosen.
enum RowOp {
    Up(usize),
    Down(usize),
    Remove(usize),
}

fn row_ops(ui: &mut Ui, i: usize, len: usize, op: &mut Option<RowOp>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(i > 0, egui::Button::new("↑").small())
            .clicked()
        {
            *op = Some(RowOp::Up(i));
        }
        if ui
            .add_enabled(i + 1 < len, egui::Button::new("↓").small())
            .clicked()
        {
            *op = Some(RowOp::Down(i));
        }
        if ui.small_button("✕").on_hover_text("Удалить").clicked() {
            *op = Some(RowOp::Remove(i));
        }
    });
}

fn apply_op<T>(rows: &mut Vec<T>, op: Option<RowOp>) {
    match op {
        Some(RowOp::Up(i)) => rows.swap(i, i - 1),
        Some(RowOp::Down(i)) => rows.swap(i, i + 1),
        Some(RowOp::Remove(i)) => {
            rows.remove(i);
        }
        None => {}
    }
}

fn type_combo(ui: &mut Ui, id: (&str, usize), ty: &mut SignalType) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(type_name(*ty))
        .width(70.0)
        .show_ui(ui, |ui| {
            // `any` fits inputs only; the block check says so for an output.
            for t in SignalType::ALL.into_iter().chain([SignalType::Any]) {
                ui.selectable_value(ty, t, type_name(t));
            }
        });
}

/// A name not used by any port or parameter of the form.
fn free_name(f: &Form, base: &str) -> String {
    let taken = |n: &str| {
        f.inputs.iter().chain(&f.outputs).any(|p| p.name == n)
            || f.params.iter().any(|p| p.name == n)
    };
    (1..)
        .map(|i| format!("{base}{i}"))
        .find(|n| !taken(n))
        .expect("some name is free")
}

fn ports_table(ui: &mut Ui, id: &str, rows: &mut Vec<PortRow>, inputs: bool) {
    if rows.is_empty() {
        ui.weak(if inputs {
            "Входов нет"
        } else {
            "Выходов нет"
        });
        return;
    }
    let mut op = None;
    let len = rows.len();
    egui::Grid::new(id)
        .num_columns(7)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Имя").strong());
            ui.label(RichText::new("Подпись").strong());
            ui.label(RichText::new("Тип").strong());
            ui.label(
                RichText::new(if inputs {
                    "Если не подключён"
                } else {
                    ""
                })
                .strong(),
            );
            ui.label(RichText::new("Условие when").strong());
            ui.label(RichText::new("Описание").strong());
            ui.label("");
            ui.end_row();
            for (i, row) in rows.iter_mut().enumerate() {
                ui.add_sized([90.0, 20.0], TextEdit::singleline(&mut row.name));
                label_edit(ui, &mut row.label, &row.name);
                type_combo(ui, (id, i), &mut row.ty);
                if inputs {
                    ui.add_sized(
                        [90.0, 20.0],
                        TextEdit::singleline(&mut row.default).hint_text("обязателен"),
                    )
                    .on_hover_text("Значение неподключённого входа; пусто — вход надо подключить");
                } else {
                    ui.label("");
                }
                ui.add_sized(
                    [130.0, 20.0],
                    TextEdit::singleline(&mut row.when).hint_text("всегда"),
                )
                .on_hover_text(
                    "Выражение по параметрам, например mode == \"input\": \
                     порт есть, только пока оно выполняется",
                );
                ui.add_sized([220.0, 20.0], TextEdit::singleline(&mut row.description));
                row_ops(ui, i, len, &mut op);
                ui.end_row();
            }
        });
    apply_op(rows, op);
}

/// Caption field of a port or parameter; the name shows through while it is empty.
fn label_edit(ui: &mut Ui, label: &mut String, name: &str) {
    ui.add_sized([120.0, 20.0], TextEdit::singleline(label).hint_text(name))
        .on_hover_text(
            "Как вход, выход или параметр называется на блоке и в свойствах;              можно по-русски. Пусто — показывается имя.",
        );
}

fn ports_tab(ui: &mut Ui, f: &mut Form) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Входы").strong());
        if ui.small_button("+ Добавить вход").clicked() {
            let name = free_name(f, "in");
            f.inputs.push(PortRow::new(&name));
        }
    });
    ports_table(ui, "block_inputs", &mut f.inputs, true);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Выходы").strong());
        if ui.small_button("+ Добавить выход").clicked() {
            let name = free_name(f, "out");
            f.outputs.push(PortRow::new(&name));
        }
    });
    ports_table(ui, "block_outputs", &mut f.outputs, false);
    ui.add_space(8.0);
    ui.weak(
        "Имена — буквы (можно русские), цифры и «_», без пробелов: по ним блок связан в схеме \
         и в коде; название с пробелами пишите в «Подпись». Входы, выходы и параметры не могут называться \
         одинаково, имена id и type заняты. Неявно приводится только int → float.",
    );
}

fn params_tab(ui: &mut Ui, f: &mut Form) {
    if ui.small_button("+ Добавить параметр").clicked() {
        let name = free_name(f, "p");
        f.params.push(ParamRow::new(&name));
    }
    if f.params.is_empty() {
        ui.weak("Параметров нет");
        return;
    }
    let mut op = None;
    let len = f.params.len();
    egui::Grid::new("block_params")
        .num_columns(10)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in [
                "Имя",
                "Подпись",
                "Вид",
                "По умолчанию",
                "Мин.",
                "Макс.",
                "Условие when",
                "Под блоком",
                "Описание",
            ] {
                ui.label(RichText::new(h).strong());
            }
            ui.label("");
            ui.end_row();
            for (i, row) in f.params.iter_mut().enumerate() {
                ui.add_sized([90.0, 20.0], TextEdit::singleline(&mut row.name));
                label_edit(ui, &mut row.label, &row.name);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("param_kind", i))
                        .selected_text(kind_label(row.kind))
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for k in PARAM_KINDS {
                                ui.selectable_value(&mut row.kind, k, kind_label(k));
                            }
                        });
                    if row.kind == ParamKind::Pin {
                        pin_mode_edit(ui, i, row);
                    }
                });
                let hint = match row.kind {
                    ParamKind::Duration => "обязателен (1s, 500ms)",
                    _ => "обязателен",
                };
                ui.add_sized(
                    [110.0, 20.0],
                    TextEdit::singleline(&mut row.default).hint_text(hint),
                )
                .on_hover_text("Пусто — параметр надо задать в свойствах блока");
                let numeric = matches!(
                    row.kind,
                    ParamKind::Int | ParamKind::Float | ParamKind::Pin | ParamKind::Duration
                );
                for bound in [&mut row.min, &mut row.max] {
                    ui.add_enabled_ui(numeric, |ui| {
                        ui.add_sized([50.0, 20.0], TextEdit::singleline(bound))
                    })
                    .inner
                    .on_hover_text("У duration — в миллисекундах");
                }
                ui.add_sized(
                    [130.0, 20.0],
                    TextEdit::singleline(&mut row.when).hint_text("всегда"),
                );
                egui::ComboBox::from_id_salt(("param_show", i))
                    .selected_text(show_label(row.show))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for s in ParamShow::ALL {
                            ui.selectable_value(&mut row.show, s, show_label(s));
                        }
                    })
                    .response
                    .on_hover_text("Как значение подписывается под блоком на плате");
                ui.add_sized([200.0, 20.0], TextEdit::singleline(&mut row.description));
                row_ops(ui, i, len, &mut op);
                ui.end_row();
            }
        });
    apply_op(&mut f.params, op);
}

/// Direction of a pin parameter and whether blocks of the type may share the pin.
fn pin_mode_edit(ui: &mut Ui, i: usize, row: &mut ParamRow) {
    let label = |m: PinMode| match m {
        PinMode::Io => "вход-выход",
        PinMode::In => "вход",
        PinMode::Out => "выход",
        PinMode::Adc => "АЦП",
    };
    egui::ComboBox::from_id_salt(("pin_mode", i))
        .selected_text(label(row.mode))
        .width(90.0)
        .show_ui(ui, |ui| {
            for m in [PinMode::Io, PinMode::In, PinMode::Out, PinMode::Adc] {
                ui.selectable_value(&mut row.mode, m, label(m));
            }
        })
        .response
        .on_hover_text("Какие выводы платы подходят: выход не ставится на вывод «только вход», АЦП — только на выводы с АЦП");
    ui.checkbox(&mut row.shared, "общий")
        .on_hover_text("Несколько блоков этого типа могут сидеть на одном выводе (шина 1-Wire)");
}

/// Text inserted into the template from the substitution menu or buttons.
struct Snippet {
    label: String,
    text: String,
    hint: String,
}

impl Snippet {
    fn new(label: impl Into<String>, text: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            hint: hint.into(),
        }
    }
}

/// Name with the caption, for menus: `in (Вход)`.
fn row_title(name: &str, label: &str) -> String {
    match label.trim() {
        "" => name.to_owned(),
        l => format!("{name} ({l})"),
    }
}

/// Port substitutions: `{{ input.x }}` or `{{ output.x }}`.
fn port_snippets(rows: &[PortRow], kind: &str) -> Vec<Snippet> {
    rows.iter()
        .map(|p| {
            let hint = if p.description.is_empty() {
                type_name(p.ty).to_owned()
            } else {
                format!("{}, {}", type_name(p.ty), p.description)
            };
            Snippet::new(
                row_title(&p.name, &p.label),
                format!("{{{{ {kind}.{} }}}}", p.name),
                hint,
            )
        })
        .collect()
}

/// Submenu of snippets; the chosen text goes to `insert`.
fn snippet_menu(ui: &mut Ui, title: &str, items: &[Snippet], insert: &mut Option<String>) {
    ui.add_enabled_ui(!items.is_empty(), |ui| {
        ui.menu_button(title, |ui| {
            for s in items {
                let text = RichText::new(format!("{}   {}", s.label, s.text.trim_end()));
                if ui.button(text).on_hover_text(&s.hint).clicked() {
                    *insert = Some(s.text.clone());
                    ui.close();
                }
            }
        });
    });
}

/// Inserts `text` at the cursor of the text edit `id` (replacing the selection,
/// or at the end without a cursor) and puts the cursor after it.
fn insert_at_cursor(ctx: &egui::Context, id: Id, template: &mut String, text: &str) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let chars = template.chars().count();
    let (start, end) = state
        .cursor
        .char_range()
        .map(|r| {
            let (a, b) = (r.primary.index.0, r.secondary.index.0);
            (a.min(b).min(chars), a.max(b).min(chars))
        })
        .unwrap_or((chars, chars));
    let byte = |i: usize| {
        template
            .char_indices()
            .nth(i)
            .map_or(template.len(), |(b, _)| b)
    };
    let (from, to) = (byte(start), byte(end));
    template.replace_range(from..to, text);
    let cursor = CCursor::new(start + text.chars().count());
    state.cursor.set_char_range(Some(CCursorRange::one(cursor)));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

fn code_tab(ui: &mut Ui, f: &mut Form, editable: bool) {
    ui.weak(SECTIONS_HINT);
    let inputs = port_snippets(&f.inputs, "input");
    let outputs = port_snippets(&f.outputs, "output");
    let literal: Vec<Snippet> = f
        .params
        .iter()
        .map(|p| {
            let hint = format!("{} — значение как литерал C++", kind_label(p.kind));
            Snippet::new(
                row_title(&p.name, &p.label),
                format!("{{{{ param.{} }}}}", p.name),
                hint,
            )
        })
        .collect();
    let conditions: Vec<Snippet> = f
        .params
        .iter()
        .map(|p| {
            let hint = "Код внутри попадёт в скетч, только если условие выполнено";
            let text = match p.kind {
                ParamKind::Bool => format!(
                    "{{% if value.{} %}}

{{% endif %}}
",
                    p.name
                ),
                ParamKind::Text => {
                    format!(
                        "{{% if value.{} == \"\" %}}

{{% endif %}}
",
                        p.name
                    )
                }
                _ => format!(
                    "{{% if value.{} > 0 %}}

{{% endif %}}
",
                    p.name
                ),
            };
            Snippet::new(row_title(&p.name, &p.label), text, hint)
        })
        .collect();
    let sections: Vec<Snippet> = [
        ("// @global", "Переменные блока, по одной на экземпляр"),
        ("// @setup", "Код в setup()"),
        ("// @loop", "Код цикла, в порядке схемы"),
        ("// @functions", "Функции блока, перед setup()"),
        ("// @global once", "Общие переменные, один раз на проект"),
        ("// @setup once", "Общий код в setup(), один раз"),
        ("// @functions once", "Общие функции, один раз на проект"),
    ]
    .into_iter()
    .map(|(text, hint)| {
        Snippet::new(
            "",
            format!(
                "{text}
"
            ),
            hint,
        )
    })
    .collect();

    if !editable {
        let mut text = f.template.as_str();
        ui.add(
            TextEdit::multiline(&mut text)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
        return;
    }
    let id = Id::unique("block_template");
    let mut insert = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Вставить:");
        let buttons = inputs.iter().chain(&outputs).chain(&literal);
        for s in std::iter::once(&Snippet::new("", "{{ id }}", "")).chain(buttons) {
            if ui
                .small_button(RichText::new(&s.text).monospace())
                .clicked()
            {
                insert = Some(s.text.clone());
            }
        }
    });
    ui.weak("Правая кнопка в поле кода — меню подстановок; вставка идёт в место курсора.");
    let output = TextEdit::multiline(&mut f.template)
        .id(id)
        .code_editor()
        .desired_width(f32::INFINITY)
        .desired_rows(24)
        .show(ui);
    output.response.context_menu(|ui| {
        if ui
            .button("{{ id }} — префикс переменных блока")
            .on_hover_text("Имена в разделах без once начинайте с него: {{ id }}_start")
            .clicked()
        {
            insert = Some("{{ id }}".into());
            ui.close();
        }
        ui.separator();
        snippet_menu(ui, "Вход", &inputs, &mut insert);
        snippet_menu(ui, "Выход", &outputs, &mut insert);
        snippet_menu(ui, "Параметр", &literal, &mut insert);
        snippet_menu(ui, "Условие по параметру", &conditions, &mut insert);
        ui.separator();
        snippet_menu(ui, "Раздел", &sections, &mut insert);
    });
    if let Some(text) = insert {
        insert_at_cursor(ui.ctx(), id, &mut f.template, &text);
    }
}

fn sketch_tab(ui: &mut Ui, ed: &BlockEditor, pal: &Palette) {
    match &ed.generated {
        None => {
            ui.weak("Исправьте ошибки описания блока — тогда здесь появится код.");
        }
        Some(g) => match &g.result {
            Ok(code) => {
                ui.weak(format!(
                    "Скетч с одним блоком «{}»: неподключённые входы — нули, обязательные \
                     параметры — пробные значения.",
                    flp_codegen::preview_id(&g.def)
                ));
                let mut text = code.as_str();
                ui.add(
                    TextEdit::multiline(&mut text)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            }
            Err(e) => {
                ui.colored_label(pal.error, e);
            }
        },
    }
}

fn status(ui: &mut Ui, ed: &BlockEditor, def: &Result<BlockDef, String>, pal: &Palette) {
    let (color, text): (Color32, String) = match (def, &ed.generated) {
        (Err(e), _) => (pal.error, format!("Описание: {e}")),
        (Ok(_), Some(Generated { result: Err(e), .. })) => {
            let first = e.lines().next().unwrap_or_default();
            (pal.error, format!("Код: {first}"))
        }
        _ => (pal.ok, "Описание и код в порядке".to_owned()),
    };
    ui.colored_label(color, text);
}

/// Save and close buttons; returns true when the window should close.
fn footer(
    app: &mut EditorApp,
    ui: &mut Ui,
    ed: &mut BlockEditor,
    def: &Result<BlockDef, String>,
) -> bool {
    let mut close = false;
    if ed.read_only {
        ui.horizontal(|ui| {
            ui.colored_label(
                app.theme.palette.warning,
                "Встроенный блок изменять нельзя — создайте на его основе свой.",
            );
            if ui.button("Создать копию").clicked() {
                let block = ed.original.as_ref().and_then(|k| app.lib.get(k)).cloned();
                if let Some(block) = block {
                    *ed = BlockEditor::copy_of(app, &block);
                }
            }
            close = ui.button("Закрыть").clicked();
        });
        return close;
    }
    ui.horizontal(|ui| {
        // A new block can be saved untouched; an opened one only after changes.
        let savable = ed.dirty() || ed.original.is_none();
        if ui
            .add_enabled(def.is_ok() && savable, egui::Button::new("Сохранить"))
            .clicked()
        {
            save(app, ed, def);
        }
        if ui
            .add_enabled(ed.form != ed.clean, egui::Button::new("Отменить правки"))
            .clicked()
        {
            ed.form = ed.clean.clone();
        }
        if ui.button("Закрыть").clicked() {
            if ed.dirty() {
                ed.confirm_close = true;
            } else {
                close = true;
            }
        }
        if let Some(dir) = target_dir(app, ed) {
            ui.weak(format!("Папка: {}", dir.join(&ed.form.kind).display()));
        }
    });
    close
}

/// Folder the block is saved into: where it already lies, or for a new block
/// its section and folders under `user_library/`.
fn target_dir(app: &EditorApp, ed: &BlockEditor) -> Option<PathBuf> {
    let existing = ed.original.as_ref().and_then(|k| app.lib.get(k));
    if let Some(parent) = existing.and_then(|b| b.dir.parent()) {
        return Some(parent.to_owned());
    }
    let user = app.user_lib_dir()?;
    Some(
        ed.form
            .group
            .split('/')
            .filter(|p| !p.is_empty())
            .fold(user, |d, p| d.join(p)),
    )
}

/// Writes the block into the library and reloads it; returns whether it worked.
fn save(app: &mut EditorApp, ed: &mut BlockEditor, def: &Result<BlockDef, String>) -> bool {
    let def = match def {
        Ok(def) => def,
        Err(e) => {
            app.info(LogLevel::Error, format!("Блок не сохранён: {e}"));
            return false;
        }
    };
    let Some(lib_dir) = target_dir(app, ed) else {
        app.info(LogLevel::Error, "Блок не сохранён: нет папки библиотеки");
        return false;
    };
    let kind = &def.info.kind;
    if ed.read_only || app.lib.is_builtin(kind) {
        app.info(
            LogLevel::Error,
            format!("Блок не сохранён: тип «{kind}» занят встроенным блоком, его менять нельзя"),
        );
        return false;
    }
    let new = ed.original.as_deref() != Some(kind.as_str());
    if new && (app.lib.get(kind).is_some() || lib_dir.join(kind).exists()) {
        app.info(
            LogLevel::Error,
            format!("Блок не сохранён: тип «{kind}» уже есть, выберите другое имя"),
        );
        return false;
    }
    match flp_core::save_block(&lib_dir, def, &ed.form.template, &ed.form.docs) {
        Ok(dir) => {
            app.info(
                LogLevel::Ok,
                format!("Блок «{}» сохранён в {}", def.info.name, dir.display()),
            );
            ed.original = Some(kind.clone());
            ed.clean = ed.form.clone();
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(LogLevel::Error, format!("Блок не сохранён: {e}"));
            false
        }
    }
}

/// Asks before deleting a user block (`app.block_delete`) and deletes its folder.
pub fn delete_dialog(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(kind) = app.block_delete.clone() else {
        return;
    };
    let (Some(block), Some(user_dir)) = (app.lib.get(&kind).cloned(), app.user_lib_dir()) else {
        app.block_delete = None;
        return;
    };
    if !app.lib.is_user(&kind) {
        app.block_delete = None;
        app.info(LogLevel::Error, "Встроенный блок удалить нельзя");
        return;
    }
    let used = app.doc.as_ref().map_or(0, |d| {
        d.project
            .logic
            .blocks
            .iter()
            .filter(|b| b.kind == kind)
            .count()
    });
    let warning = app.theme.palette.warning;
    let mut answer = None;
    egui::Modal::new(Id::unique("block_delete")).show(ctx, |ui| {
        ui.label(RichText::new("Удалить блок").strong());
        ui.label(format!(
            "Удалить пользовательский блок «{}» ({kind})? Папка {} будет удалена.",
            block.def.info.name,
            block.dir.display()
        ));
        if used > 0 {
            ui.colored_label(
                warning,
                format!(
                    "В открытом проекте таких блоков: {used}. Схема перестанет проходить проверку."
                ),
            );
        }
        ui.horizontal(|ui| {
            if ui.button("Удалить").clicked() {
                answer = Some(true);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    match answer {
        Some(true) => {
            app.block_delete = None;
            // Only a folder inside user_library/ is ever removed.
            if !block.dir.starts_with(&user_dir) || block.dir == user_dir {
                app.info(
                    LogLevel::Error,
                    "Блок не удалён: он не в папке пользовательских блоков",
                );
                return;
            }
            match fs::remove_dir_all(&block.dir) {
                Ok(()) => {
                    app.info(LogLevel::Ok, format!("Блок «{kind}» удалён"));
                    if app
                        .block_editor
                        .as_ref()
                        .is_some_and(|e| e.original.as_deref() == Some(kind.as_str()))
                    {
                        app.block_editor = None;
                    }
                    app.reload_library();
                }
                Err(e) => app.info(LogLevel::Error, format!("Блок не удалён: {e}")),
            }
        }
        Some(false) => app.block_delete = None,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_round_trip() {
        let src = r#"
[block]
type = "x"
name = "X"
group = "Тест"
includes = ["A.h", "B.h"]

[[input]]
name = "k"
label = "Коэффициент"
type = "float"
default = 2.5

[[output]]
name = "q"
type = "bool"

[[param]]
name = "t"
type = "duration"
default = "1s"
min = 0
"#;
        let def: BlockDef = toml::from_str(src).unwrap();
        let block = LibraryBlock {
            def: def.clone(),
            dir: Default::default(),
            inline_templates: [(ESP32_CPP_TEMPLATE, "// @loop\n".to_owned())].into(),
        };
        let form = Form::from_block(&block);
        assert_eq!(form.includes, "A.h, B.h");
        assert_eq!(form.inputs[0].label, "Коэффициент");
        assert_eq!(form.to_def(), Ok(def));
        assert_eq!(form.template, "// @loop\n");
    }

    #[test]
    fn form_errors() {
        let mut form = Form::new_block("ok".into(), String::new());
        assert!(form.to_def().is_ok());
        form.inputs[0].default = "maybe".into();
        assert!(form.to_def().unwrap_err().contains("maybe"));
        form.inputs[0].default = "да".into();
        assert_eq!(
            form.to_def().unwrap().inputs[0].default,
            Some(ParamValue::Bool(true))
        );
        form.kind = VAR_GET.into();
        assert!(form.to_def().is_err());
        form.kind = "ok".into();
        form.outputs[0].name = "in".into();
        assert!(form.to_def().is_err());
    }

    #[test]
    fn insert_at_cursor_replaces_selection() {
        use egui::text::{CCursor, CCursorRange};
        let ctx = egui::Context::default();
        let id = Id::unique("test_template");
        let mut text = "аб".to_owned();
        insert_at_cursor(&ctx, id, &mut text, "{{ id }}");
        assert_eq!(text, "аб{{ id }}");

        let mut state = egui::text_edit::TextEditState::default();
        let range = CCursorRange::two(CCursor::new(1), CCursor::new(2));
        state.cursor.set_char_range(Some(range));
        state.store(&ctx, id);
        let mut text = "абв".to_owned();
        insert_at_cursor(&ctx, id, &mut text, "X");
        assert_eq!(text, "аXв");
        // The cursor moved after the inserted text.
        insert_at_cursor(&ctx, id, &mut text, "Y");
        assert_eq!(text, "аXYв");
    }

    #[test]
    fn new_block_generates() {
        let form = Form::new_block("demo".into(), String::new());
        let def = form.to_def().unwrap();
        let code = flp_codegen::preview_block(&def, &form.template).unwrap();
        assert!(
            code.contains("demo_1_out = false;") || code.contains("demo_1_out"),
            "{code}"
        );
    }
}
