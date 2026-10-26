//! Pop-up windows with ✓ (apply) and ✗ (cancel): block properties, variable, sheet.
//!
//! Changes are buffered in the dialog and applied as one undo step on ✓.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use eframe::egui::{self, Color32, RichText, Ui};
use flp_core::pins::{pin_uses, Chip, PinUse};
use flp_core::value::parse_duration_ms;
use flp_core::{
    BlockDef, LocalValues, ParamDef, ParamKind, ParamShow, ParamValue, PortDef, SheetInfo,
    SignalType, Var, VarType,
};

use crate::app::{EditorApp, LogLevel, Selection};

pub enum Dialog {
    Block(BlockDialog),
    Var(VarDialog),
    Sheet(SheetDialog),
    Board(crate::boardcfg::BoardDialog),
}

pub struct BlockDialog {
    id: String,
    new_id: String,
    kind: String,
    /// Parameter values; `None` means "use the default". Values of `local`
    /// parameters (passwords) come from [`LocalValues`] and go back there.
    values: BTreeMap<String, Option<ParamValue>>,
    /// Text being typed for string and duration parameters.
    texts: BTreeMap<String, String>,
    /// Secret parameters shown as plain text (the eye button).
    revealed: BTreeSet<String>,
}

pub struct VarDialog {
    /// Name before editing; `None` for a new variable.
    original: Option<String>,
    name: String,
    ty: VarType,
    /// Block the dialog was opened from and its write enable input (per block).
    block: Option<(String, bool)>,
    init: String,
    description: String,
    retain: bool,
}

pub struct SheetDialog {
    number: u32,
    name: String,
    comment: String,
}

enum Outcome {
    Open,
    Apply,
    Cancel,
    Delete,
}

impl Dialog {
    pub fn for_block(app: &EditorApp, id: &str) -> Option<Self> {
        let doc = app.doc.as_ref()?;
        let block = doc.project.logic.block(id)?;
        if let Some(var) = block.var_name() {
            let en = matches!(
                block.params.get(flp_core::vars::WRITE_ENABLE),
                Some(ParamValue::Bool(true))
            );
            let mut dialog = Self::for_var(app, Some(var))?;
            if let Self::Var(d) = &mut dialog {
                d.block = Some((id.to_owned(), en));
            }
            return Some(dialog);
        }
        let def = &app.lib.resolve(&doc.project.logic, block)?.def;
        let mut block = block.clone();
        app.local.fill_block(def, &mut block);
        let values = def
            .params
            .iter()
            .map(|p| (p.name.clone(), block.params.get(&p.name).cloned()))
            .collect();
        let texts = def
            .params
            .iter()
            .filter(|p| matches!(p.kind, ParamKind::Text | ParamKind::Duration))
            .map(|p| {
                let v = block.params.get(&p.name).or(p.default.as_ref());
                let text = match v {
                    Some(ParamValue::Text(s)) => s.clone(),
                    _ => String::new(),
                };
                (p.name.clone(), text)
            })
            .collect();
        Some(Self::Block(BlockDialog {
            id: id.to_owned(),
            new_id: id.to_owned(),
            kind: block.kind.clone(),
            values,
            texts,
            revealed: BTreeSet::new(),
        }))
    }

    pub fn for_var(app: &EditorApp, name: Option<&str>) -> Option<Self> {
        let var = match name {
            Some(n) => app.doc.as_ref()?.project.logic.var(n)?.clone(),
            None => Var {
                name: String::new(),
                ty: VarType::Float,
                init: None,
                description: String::new(),
                retain: false,
                service: false,
            },
        };
        Some(Self::Var(VarDialog {
            original: name.map(str::to_owned),
            init: var.init.as_ref().map(value_text).unwrap_or_default(),
            name: var.name,
            ty: var.ty,
            block: None,
            description: var.description,
            retain: var.retain,
        }))
    }

    pub fn for_sheet(app: &EditorApp, number: u32) -> Self {
        let info = app
            .doc
            .as_ref()
            .and_then(|d| d.project.layout.sheet(number))
            .cloned()
            .unwrap_or_default();
        Self::Sheet(SheetDialog {
            number,
            name: info.name,
            comment: info.comment,
        })
    }
}

/// Shows the open dialog, if any, and applies or drops it.
pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.dialog.take() else {
        return;
    };
    let title = match &dialog {
        Dialog::Block(d) => format!("Свойства блока «{}»", d.id),
        Dialog::Var(d) => match &d.original {
            Some(n) => format!("Переменная «{n}»"),
            None => "Новая переменная".to_owned(),
        },
        Dialog::Sheet(d) => format!("Плата {}", d.number),
        Dialog::Board(_) => "Настройки МК".to_owned(),
    };
    let mut open = true;
    let mut outcome = Outcome::Open;
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(380.0)
        .show(ctx, |ui| {
            outcome = buttons(
                ui,
                matches!(&dialog, Dialog::Var(d) if d.original.is_some()),
            );
            ui.separator();
            match &mut dialog {
                Dialog::Block(d) => block_body(app, ui, d),
                Dialog::Var(d) => var_body(ui, d),
                Dialog::Sheet(d) => sheet_body(ui, d),
                Dialog::Board(d) => d.body(ui, &app.ports),
            }
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    let keep = match outcome {
        Outcome::Open => true,
        Outcome::Cancel => false,
        Outcome::Apply => match apply(app, &dialog) {
            Ok(()) => false,
            Err(e) => {
                app.info(LogLevel::Error, e);
                true
            }
        },
        Outcome::Delete => {
            if let Dialog::Var(VarDialog {
                original: Some(name),
                ..
            }) = &dialog
            {
                let lib = app.lib.clone();
                if let Some(doc) = &mut app.doc {
                    if let Err(e) = doc.edit(&lib, None, |p| p.remove_var(name)) {
                        app.info(LogLevel::Error, e.to_string());
                    }
                }
                app.selection = None;
            }
            false
        }
    };
    if keep {
        app.dialog = Some(dialog);
    }
}

/// ✓ / ✗ row at the top of the window, as in FLProg.
fn buttons(ui: &mut Ui, deletable: bool) -> Outcome {
    let mut outcome = Outcome::Open;
    ui.horizontal(|ui| {
        let ok =
            egui::Button::new(RichText::new("✔ Применить").color(Color32::from_rgb(72, 136, 88)));
        if ui.add(ok).clicked() {
            outcome = Outcome::Apply;
        }
        let cancel =
            egui::Button::new(RichText::new("✖ Отмена").color(Color32::from_rgb(186, 74, 66)));
        if ui.add(cancel).clicked() {
            outcome = Outcome::Cancel;
        }
        if deletable && ui.button("Удалить").clicked() {
            outcome = Outcome::Delete;
        }
    });
    outcome
}

fn block_body(app: &EditorApp, ui: &mut Ui, d: &mut BlockDialog) {
    let Some(doc) = &app.doc else { return };
    let Some(block) = doc.project.logic.block(&d.id) else {
        ui.weak("Блок удалён");
        return;
    };
    let Some(lib_block) = app.lib.resolve(&doc.project.logic, block) else {
        ui.colored_label(
            Color32::from_rgb(186, 74, 66),
            format!("Неизвестный тип «{}»", block.kind),
        );
        return;
    };
    let docs = lib_block
        .docs_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .filter(|text| !text.trim().is_empty());
    let Some(docs) = docs else {
        block_settings(app, ui, d, doc, lib_block);
        return;
    };
    // Settings on the left, the block's docs on the right.
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(SETTINGS_WIDTH);
            block_settings(app, ui, d, doc, lib_block);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_min_width(DOCS_MIN_WIDTH);
            egui::ScrollArea::vertical()
                .id_salt("block_docs")
                .max_height(ui.ctx().content_rect().height() * 0.7)
                .auto_shrink([false, true])
                .show(ui, |ui| crate::markdown::show(ui, &docs));
        });
    });
}

/// Width of the settings column when the docs are shown beside it.
const SETTINGS_WIDTH: f32 = 360.0;
const DOCS_MIN_WIDTH: f32 = 320.0;

fn block_settings(
    app: &EditorApp,
    ui: &mut Ui,
    d: &mut BlockDialog,
    doc: &crate::doc::Doc,
    lib_block: &flp_core::LibraryBlock,
) {
    let def = &lib_block.def;
    ui.label(RichText::new(&def.info.name).strong());
    ui.add(egui::Label::new(RichText::new(&def.info.description).weak()).wrap());
    ui.add_space(6.0);
    // Parameters and ports switched off by `when` follow the values being edited.
    let current: BTreeMap<String, ParamValue> = d
        .values
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), v.clone()?)))
        .collect();
    let params: Vec<&ParamDef> = def.active_params(&current).collect();
    let inputs: Vec<&PortDef> = def.active_inputs(&current).collect();
    let outputs: Vec<&PortDef> = def.active_outputs(&current).collect();
    let pins = Pins {
        chip: Chip::of_board(&doc.project.config.board),
        others: pin_uses(&doc.project.logic, &app.lib)
            .into_iter()
            .filter(|u| u.block != d.id)
            .collect(),
    };
    egui::Grid::new("block_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Надпись (id)");
            ui.text_edit_singleline(&mut d.new_id);
            ui.end_row();
            for &p in &params {
                let hint = match &p.label {
                    Some(_) => format!("{} ({})", p.description, p.name),
                    None => p.description.clone(),
                };
                ui.label(p.title()).on_hover_text(hint.trim());
                ui.horizontal(|ui| {
                    param_editor(ui, p, d, def, &app.local, &pins);
                    let set = d.values.get(&p.name).is_some_and(Option::is_some);
                    if set && p.default.is_some() && ui.small_button("по умолч.").clicked() {
                        d.values.insert(p.name.clone(), None);
                        if let Some(ParamValue::Text(s)) = &p.default {
                            d.texts.insert(p.name.clone(), s.clone());
                        }
                    }
                });
                ui.end_row();
            }
        });
    if !inputs.is_empty() || !outputs.is_empty() {
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Входы и выходы")
            .default_open(true)
            .show(ui, |ui| {
                for input in &inputs {
                    let source = doc
                        .project
                        .logic
                        .links
                        .iter()
                        .find(|l| l.to.block == d.id && l.to.port == input.name)
                        .map(|l| format!("← {}", l.from))
                        .or_else(|| {
                            input
                                .default
                                .as_ref()
                                .map(|v| format!("не подключён, {}", value_text(v)))
                        })
                        .unwrap_or_else(|| "не подключён".to_owned());
                    ui.label(format!(
                        "вход {} · {}: {source}",
                        input.name,
                        type_name(input.ty)
                    ))
                    .on_hover_text(&input.description);
                }
                for output in &outputs {
                    ui.label(format!("выход {} · {}", output.name, type_name(output.ty)))
                        .on_hover_text(&output.description);
                }
            });
    }
}

/// Board pins and the pins other blocks of the schema take, for the pin list.
struct Pins {
    /// `None` for a board without a pin table: the number is typed then.
    chip: Option<Chip>,
    others: Vec<PinUse>,
}

/// Edits one buffered parameter of the block dialog.
fn param_editor(
    ui: &mut Ui,
    p: &ParamDef,
    d: &mut BlockDialog,
    def: &BlockDef,
    local: &LocalValues,
    pins: &Pins,
) {
    let current = d
        .values
        .get(&p.name)
        .cloned()
        .flatten()
        .or_else(|| p.default.clone());
    if p.kind == ParamKind::Pin && pins.chip.is_some() {
        let current = match current {
            Some(ParamValue::Int(i)) => Some(i),
            _ => None,
        };
        if let Some(gpio) = pin_combo(ui, p, d, def, pins, current) {
            d.values.insert(p.name.clone(), Some(ParamValue::Int(gpio)));
        }
        if current.is_none() {
            ui.colored_label(Color32::from_rgb(186, 74, 66), "не задан");
        }
        return;
    }
    if !p.choices.is_empty() {
        let selected = current
            .as_ref()
            .map_or_else(|| "не задан".to_owned(), |v| p.value_label(v));
        egui::ComboBox::from_id_salt(("param_choice", &p.name))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for c in &p.choices {
                    let on = current.as_ref() == Some(&c.value);
                    if ui.selectable_label(on, &c.label).clicked() {
                        if let ParamValue::Text(s) = &c.value {
                            d.texts.insert(p.name.clone(), s.clone());
                        }
                        d.values.insert(p.name.clone(), Some(c.value.clone()));
                    }
                }
            });
        return;
    }
    let mut set = |v: ParamValue| {
        d.values.insert(p.name.clone(), Some(v));
    };
    match p.kind {
        ParamKind::Bool => {
            let mut v = matches!(current, Some(ParamValue::Bool(true)));
            if ui.checkbox(&mut v, "").changed() {
                set(ParamValue::Bool(v));
            }
        }
        ParamKind::Int | ParamKind::Pin => {
            let missing = current.is_none();
            let mut v = match current {
                Some(ParamValue::Int(i)) => i,
                _ => 0,
            };
            let mut drag = egui::DragValue::new(&mut v);
            if p.kind == ParamKind::Pin {
                drag = drag.range(0..=48).prefix("GPIO");
            }
            if let (Some(min), Some(max)) = (p.min, p.max) {
                drag = drag.range(min as i64..=max as i64);
            }
            if ui.add(drag).changed() || (missing && ui.small_button("задать").clicked()) {
                set(ParamValue::Int(v));
            }
            if missing {
                ui.colored_label(Color32::from_rgb(186, 74, 66), "не задан");
            }
        }
        ParamKind::Float => {
            let mut v = match current {
                Some(ParamValue::Float(f)) => f,
                Some(ParamValue::Int(i)) => i as f64,
                _ => 0.0,
            };
            let mut drag = egui::DragValue::new(&mut v).speed(0.1);
            if let (Some(min), Some(max)) = (p.min, p.max) {
                drag = drag.range(min..=max);
            }
            if ui.add(drag).changed() {
                set(ParamValue::Float(v));
            }
        }
        ParamKind::Text | ParamKind::Duration => {
            // Parameters kept by the program under this one's value (SSID -> password).
            let dependents: Vec<&ParamDef> = def
                .params
                .iter()
                .filter(|q| q.local.as_deref() == Some(p.name.as_str()))
                .collect();
            let secret = p.show == ParamShow::Secret && !d.revealed.contains(&p.name);
            let text = d.texts.entry(p.name.clone()).or_default();
            let valid = p.kind != ParamKind::Duration || parse_duration_ms(text).is_some();
            let mut edit = egui::TextEdit::singleline(text)
                .desired_width(110.0)
                .password(secret);
            if p.kind == ParamKind::Duration {
                edit = edit.hint_text("5s, 500ms, 2m");
            }
            if !valid {
                edit = edit.text_color(Color32::from_rgb(186, 74, 66));
            }
            let changed = if dependents.is_empty() {
                ui.add(edit).changed()
            } else {
                // Remembered keys (entered SSIDs) are suggested while typing.
                let known: BTreeSet<&str> = dependents
                    .iter()
                    .flat_map(|q| local.keys(&d.kind, &q.name))
                    .collect();
                let id = ui.make_persistent_id(("suggest", &p.name));
                let out = egui::CompletionPopup::new(id)
                    .word_boundary(|_| false)
                    .show(ui, edit, |query| {
                        let typed = query.text.to_lowercase();
                        known
                            .iter()
                            .filter(|k| **k != query.text && k.to_lowercase().contains(&typed))
                            .map(|k| egui::Suggestion::new(*k))
                            .collect()
                    });
                out.accepted.is_some() || out.text_edit.response.changed()
            };
            if p.show == ParamShow::Secret {
                let shown = d.revealed.contains(&p.name);
                if ui
                    .selectable_label(shown, "\u{1f441}")
                    .on_hover_text("Показать или скрыть")
                    .clicked()
                {
                    if shown {
                        d.revealed.remove(&p.name);
                    } else {
                        d.revealed.insert(p.name.clone());
                    }
                }
            }
            if changed {
                let text = d.texts.get(&p.name).cloned().unwrap_or_default();
                let ok = p.kind != ParamKind::Duration || parse_duration_ms(&text).is_some();
                if ok {
                    // A new key brings its stored values (the password of that SSID).
                    for q in &dependents {
                        let value = local.get(&d.kind, &q.name, &text).unwrap_or_default();
                        d.texts.insert(q.name.clone(), value.to_owned());
                        d.values
                            .insert(q.name.clone(), Some(ParamValue::Text(value.to_owned())));
                    }
                    d.values
                        .insert(p.name.clone(), Some(ParamValue::Text(text)));
                }
            }
        }
    }
}

/// Drop-down list of the board pins that fit the parameter; pins taken by other
/// blocks (or other parameters of this one) are shown but cannot be chosen.
/// Returns the chosen pin.
fn pin_combo(
    ui: &mut Ui,
    p: &ParamDef,
    d: &BlockDialog,
    def: &BlockDef,
    pins: &Pins,
    current: Option<i64>,
) -> Option<i64> {
    let chip = pins.chip?;
    let me = PinUse {
        block: d.id.clone(),
        kind: d.kind.clone(),
        param: p.name.clone(),
        gpio: 0,
        mode: p.mode,
        shared: p.shared,
    };
    // Other pin parameters of this block, with the values being edited.
    let own: Vec<(i64, String)> = def
        .params
        .iter()
        .filter(|q| q.kind == ParamKind::Pin && q.name != p.name)
        .filter_map(|q| {
            let v = d
                .values
                .get(&q.name)
                .cloned()
                .flatten()
                .or(q.default.clone());
            match v {
                Some(ParamValue::Int(g)) if g >= 0 => {
                    Some((g, format!("«{}» этого блока", q.title())))
                }
                _ => None,
            }
        })
        .collect();
    let mut entries: Vec<(i64, String, bool)> = Vec::new();
    if p.min.is_some_and(|min| min < 0.0) {
        entries.push((-1, "не используется".to_owned(), true));
    }
    for pin in chip.pins().into_iter().filter(|pin| pin.fits(p.mode)) {
        let mut text = format!("GPIO{}", pin.gpio);
        let notes = pin.notes();
        if !notes.is_empty() {
            text += &format!(" · {notes}");
        }
        let taken = pins
            .others
            .iter()
            .find(|u| u.gpio == pin.gpio && !u.can_share(&me))
            .map(|u| format!("«{}»", u.block))
            .or_else(|| {
                own.iter()
                    .find(|(g, _)| *g == pin.gpio)
                    .map(|(_, who)| who.clone())
            });
        let free = taken.is_none();
        if let Some(who) = taken {
            text += &format!(" — занят: {who}");
        } else if let Some(u) = pins.others.iter().find(|u| u.gpio == pin.gpio) {
            text += &format!(" — общий с «{}»", u.block);
        }
        entries.push((pin.gpio, text, free));
    }
    let selected = match current {
        None => "выбрать…".to_owned(),
        Some(v) if v < 0 => "не используется".to_owned(),
        Some(v) if entries.iter().any(|(g, _, _)| *g == v) => format!("GPIO{v}"),
        Some(v) => format!("GPIO{v} (не подходит)"),
    };
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("pin", &p.name))
        .selected_text(selected)
        .height(400.0)
        .show_ui(ui, |ui| {
            for (gpio, text, free) in entries {
                let button = egui::Button::selectable(current == Some(gpio), text);
                if ui.add_enabled(free, button).clicked() {
                    chosen = Some(gpio);
                }
            }
        })
        .response
        .on_hover_text(format!("Выводы {}", chip.name()));
    chosen
}

fn var_body(ui: &mut Ui, d: &mut VarDialog) {
    egui::Grid::new("var_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Имя");
            ui.text_edit_singleline(&mut d.name);
            ui.end_row();
            ui.label("Тип");
            egui::ComboBox::from_id_salt("var_type")
                .selected_text(d.ty.name())
                .height(400.0)
                .show_ui(ui, |ui| {
                    for ty in VarType::ALL {
                        ui.selectable_value(&mut d.ty, ty, ty.name());
                    }
                });
            ui.end_row();
            ui.label("Начальное значение");
            let hint = match d.ty.signal() {
                SignalType::Bool => "false",
                SignalType::Int => "0",
                SignalType::Float => "0.0",
                SignalType::String | SignalType::Any => "текст",
            };
            let valid = parse_init(d.ty, &d.init).is_ok();
            let mut edit = egui::TextEdit::singleline(&mut d.init).hint_text(hint);
            if !valid {
                edit = edit.text_color(Color32::from_rgb(186, 74, 66));
            }
            ui.add(edit);
            ui.end_row();
            ui.label("Описание");
            ui.text_edit_singleline(&mut d.description);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.retain, "Сохранять в EEPROM").on_hover_text(
                "Значение хранится во флеш-памяти (NVS) и восстанавливается после перезагрузки.                  Запись — только когда значение изменилось, не чаще раза в секунду.",
            );
            ui.end_row();
            if let Some((_, en)) = &mut d.block {
                ui.label("");
                ui.checkbox(en, "Вход разрешения записи (En)").on_hover_text(
                    "Только для этого блока: у него появляется логический вход En,                      значение записывается в переменную, только пока En = true.",
                );
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    ui.weak("Переменную можно читать на любой плате сколько угодно раз, а записывать — в одном месте. Если её никто не записывает, она работает как уставка.");
}

fn sheet_body(ui: &mut Ui, d: &mut SheetDialog) {
    egui::Grid::new("sheet_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Наименование");
            ui.text_edit_singleline(&mut d.name);
            ui.end_row();
            ui.label("Комментарии");
            ui.add(egui::TextEdit::multiline(&mut d.comment).desired_rows(3));
            ui.end_row();
        });
}

fn apply(app: &mut EditorApp, dialog: &Dialog) -> Result<(), String> {
    let lib = app.lib.clone();
    let doc = app.doc.as_mut().ok_or("проект не открыт")?;
    match dialog {
        Dialog::Board(d) => {
            let board = d.config()?;
            let uarts = d.uarts();
            if doc.project.config.board != board || doc.project.config.uart != uarts {
                app.port = board.port.clone().unwrap_or_default();
                doc.project.config.board = board;
                doc.project.config.uart = uarts;
                doc.dirty = true;
                doc.revision += 1;
                // Pins and UARTs are checked against the board.
                doc.recheck(&lib);
            }
        }
        Dialog::Block(d) => {
            let original = doc
                .project
                .logic
                .block(&d.id)
                .ok_or("блок удалён")?
                .params
                .clone();
            let new_id = d.new_id.trim().to_owned();
            let block = doc.project.logic.block(&d.id).ok_or("блок удалён")?;
            let def = lib
                .resolve(&doc.project.logic, block)
                .ok_or("неизвестный тип блока")?
                .def
                .clone();
            let is_local = |name: &str| def.param(name).is_some_and(|p| p.local.is_some());
            doc.edit(&lib, None, |p| {
                for (name, value) in &d.values {
                    if !is_local(name) && original.get(name) != value.as_ref() {
                        p.set_param(&lib, &d.id, name, value.clone())?;
                    }
                }
                p.rename_block(&d.id, &new_id)
            })
            .map_err(|e| e.to_string())?;
            // Passwords stay in the program, under the SSID set above.
            let mut block = doc.project.logic.block(&new_id).cloned();
            if let Some(block) = &mut block {
                for q in def.params.iter().filter(|q| q.local.is_some()) {
                    let value = d.values.get(&q.name).cloned().flatten();
                    let value = value.or_else(|| q.default.clone());
                    if let Some(value) = value {
                        block.params.insert(q.name.clone(), value);
                    }
                }
                app.local.take_block(&def, block);
                app.save_local();
            }
            app.select(Some(Selection::Block(new_id)));
        }
        Dialog::Var(d) => {
            let var = Var {
                name: d.name.trim().to_owned(),
                ty: d.ty,
                init: parse_init(d.ty, &d.init)?,
                description: d.description.trim().to_owned(),
                retain: d.retain,
                service: false,
            };
            match &d.original {
                Some(name) => doc.edit(&lib, None, |p| p.update_var(name, var)),
                None => doc.edit(&lib, None, |p| p.add_var(var)),
            }
            .map_err(|e| e.to_string())?;
            if let Some((id, en)) = &d.block {
                doc.edit(&lib, None, |p| p.set_var_write_enable(&lib, id, *en))
                    .map_err(|e| e.to_string())?;
            }
        }
        Dialog::Sheet(d) => {
            let info = SheetInfo {
                number: d.number,
                name: d.name.trim().to_owned(),
                comment: d.comment.trim().to_owned(),
            };
            doc.edit(&lib, None, |p| {
                p.set_sheet_info(info);
                Ok::<_, String>(())
            })?;
        }
    }
    Ok(())
}

/// Parses the initial value field; empty means "no initial value".
fn parse_init(ty: VarType, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let bad = || format!("«{t}» не подходит к типу {}", ty.name());
    let value = match ty.signal() {
        SignalType::Bool => match t {
            "true" | "1" | "да" => ParamValue::Bool(true),
            "false" | "0" | "нет" => ParamValue::Bool(false),
            _ => return Err(bad()),
        },
        SignalType::Int => ParamValue::Int(t.parse().map_err(|_| bad())?),
        SignalType::Float => ParamValue::Float(t.replace(',', ".").parse().map_err(|_| bad())?),
        // Text keeps its spaces.
        SignalType::String | SignalType::Any => ParamValue::Text(text.to_owned()),
    };
    if !ty.accepts(&value) {
        return Err(bad());
    }
    Ok(Some(value))
}

pub fn type_name(ty: SignalType) -> &'static str {
    ty.name()
}

pub fn value_text(v: &ParamValue) -> String {
    match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Float(f) => format!("{f}"),
        ParamValue::Text(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_values() {
        assert_eq!(
            parse_init(VarType::Float, "22,5"),
            Ok(Some(ParamValue::Float(22.5)))
        );
        assert_eq!(
            parse_init(VarType::Bool, "да"),
            Ok(Some(ParamValue::Bool(true)))
        );
        assert_eq!(parse_init(VarType::Int, ""), Ok(None));
        assert!(parse_init(VarType::Int, "1.5").is_err());
    }
}
