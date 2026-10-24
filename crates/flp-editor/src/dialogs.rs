//! Pop-up windows with ✓ (apply) and ✗ (cancel): block properties, variable, sheet.
//!
//! Changes are buffered in the dialog and applied as one undo step on ✓.

use std::collections::BTreeMap;
use std::fs;

use eframe::egui::{self, Color32, RichText, Ui};
use flp_core::value::parse_duration_ms;
use flp_core::{ParamDef, ParamKind, ParamValue, PortDef, SheetInfo, SignalType, Var};

use crate::app::{EditorApp, LogLevel, Selection};

pub enum Dialog {
    Block(BlockDialog),
    Var(VarDialog),
    Sheet(SheetDialog),
}

pub struct BlockDialog {
    id: String,
    new_id: String,
    /// Parameter values; `None` means "use the default".
    values: BTreeMap<String, Option<ParamValue>>,
    /// Text being typed for string and duration parameters.
    texts: BTreeMap<String, String>,
}

pub struct VarDialog {
    /// Name before editing; `None` for a new variable.
    original: Option<String>,
    name: String,
    ty: SignalType,
    init: String,
    description: String,
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
            return Self::for_var(app, Some(var));
        }
        let def = &app.lib.resolve(&doc.project.logic, block)?.def;
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
            values,
            texts,
        }))
    }

    pub fn for_var(app: &EditorApp, name: Option<&str>) -> Option<Self> {
        let var = match name {
            Some(n) => app.doc.as_ref()?.project.logic.var(n)?.clone(),
            None => Var {
                name: String::new(),
                ty: SignalType::Float,
                init: None,
                description: String::new(),
            },
        };
        Some(Self::Var(VarDialog {
            original: name.map(str::to_owned),
            init: var.init.as_ref().map(value_text).unwrap_or_default(),
            name: var.name,
            ty: var.ty,
            description: var.description,
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
    let def = &lib_block.def;
    ui.label(RichText::new(&def.info.name).strong());
    ui.weak(&def.info.description);
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
    egui::Grid::new("block_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Надпись (id)");
            ui.text_edit_singleline(&mut d.new_id);
            ui.end_row();
            for &p in &params {
                ui.label(&p.name).on_hover_text(&p.description);
                ui.horizontal(|ui| {
                    param_editor(ui, p, d);
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
    if let Some(path) = lib_block.docs_path() {
        egui::CollapsingHeader::new("Описание").show(ui, |ui| {
            ui.label(fs::read_to_string(path).unwrap_or_default());
        });
    }
}

/// Edits one buffered parameter of the block dialog.
fn param_editor(ui: &mut Ui, p: &ParamDef, d: &mut BlockDialog) {
    let current = d
        .values
        .get(&p.name)
        .cloned()
        .flatten()
        .or_else(|| p.default.clone());
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
            let text = d.texts.entry(p.name.clone()).or_default();
            let valid = p.kind != ParamKind::Duration || parse_duration_ms(text).is_some();
            let mut edit = egui::TextEdit::singleline(text).desired_width(110.0);
            if p.kind == ParamKind::Duration {
                edit = edit.hint_text("5s, 500ms, 2m");
            }
            if !valid {
                edit = edit.text_color(Color32::from_rgb(186, 74, 66));
            }
            if ui.add(edit).changed() {
                let text = text.clone();
                let ok = p.kind != ParamKind::Duration || parse_duration_ms(&text).is_some();
                if ok {
                    d.values
                        .insert(p.name.clone(), Some(ParamValue::Text(text)));
                }
            }
        }
    }
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
            ui.horizontal(|ui| {
                for ty in [SignalType::Bool, SignalType::Int, SignalType::Float] {
                    ui.selectable_value(&mut d.ty, ty, type_name(ty));
                }
            });
            ui.end_row();
            ui.label("Начальное значение");
            let hint = match d.ty {
                SignalType::Bool => "false",
                SignalType::Int => "0",
                SignalType::Float => "0.0",
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
        Dialog::Block(d) => {
            let original = doc
                .project
                .logic
                .block(&d.id)
                .ok_or("блок удалён")?
                .params
                .clone();
            let new_id = d.new_id.trim().to_owned();
            doc.edit(&lib, None, |p| {
                for (name, value) in &d.values {
                    if original.get(name) != value.as_ref() {
                        p.set_param(&lib, &d.id, name, value.clone())?;
                    }
                }
                p.rename_block(&d.id, &new_id)
            })
            .map_err(|e| e.to_string())?;
            app.select(Some(Selection::Block(new_id)));
        }
        Dialog::Var(d) => {
            let var = Var {
                name: d.name.trim().to_owned(),
                ty: d.ty,
                init: parse_init(d.ty, &d.init)?,
                description: d.description.trim().to_owned(),
            };
            match &d.original {
                Some(name) => doc.edit(&lib, None, |p| p.update_var(name, var)),
                None => doc.edit(&lib, None, |p| p.add_var(var)),
            }
            .map_err(|e| e.to_string())?;
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
fn parse_init(ty: SignalType, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let bad = || format!("«{t}» не подходит к типу {}", type_name(ty));
    let value = match ty {
        SignalType::Bool => match t {
            "true" | "1" | "да" => ParamValue::Bool(true),
            "false" | "0" | "нет" => ParamValue::Bool(false),
            _ => return Err(bad()),
        },
        SignalType::Int => ParamValue::Int(t.parse().map_err(|_| bad())?),
        SignalType::Float => ParamValue::Float(t.replace(',', ".").parse().map_err(|_| bad())?),
    };
    Ok(Some(value))
}

pub fn type_name(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int",
        SignalType::Float => "float",
    }
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
            parse_init(SignalType::Float, "22,5"),
            Ok(Some(ParamValue::Float(22.5)))
        );
        assert_eq!(
            parse_init(SignalType::Bool, "да"),
            Ok(Some(ParamValue::Bool(true)))
        );
        assert_eq!(parse_init(SignalType::Int, ""), Ok(None));
        assert!(parse_init(SignalType::Int, "1.5").is_err());
    }
}
