//! Network services in the left panel: Wi-Fi, Ethernet and the web interface.
//! A service is a hidden block of the schema: the check box adds or removes it,
//! the settings are its parameters, and every port or text/number parameter can
//! be taken out to the board as a variable (a binding stored in the block).

use eframe::egui::{self, Ui};
use flp_core::{bindable_type, ParamKind, ParamValue, SignalType, Var, VarType};

use crate::app::{EditorApp, LogLevel};
use crate::dialogs::{value_text, Dialog};

const NETWORKS: [(&str, &str); 2] = [("wifi", "Wi-Fi"), ("ethernet", "Ethernet")];
const WEB_UI: &str = "web_ui";

pub enum NetAction {
    Toggle {
        kind: &'static str,
        on: bool,
    },
    SetParam {
        id: String,
        name: String,
        value: ParamValue,
    },
    Settings(String),
    /// Takes a port or a parameter from a variable (`None` releases it).
    Bind {
        id: String,
        name: String,
        var: Option<String>,
    },
    /// Creates a variable of the type and binds it.
    BindNew {
        id: String,
        name: String,
        ty: SignalType,
    },
    Designer,
    AddWidget(String),
    Focus(String),
}

pub fn show(app: &EditorApp, ui: &mut Ui) -> Vec<NetAction> {
    let mut actions = Vec::new();
    let Some(doc) = &app.doc else {
        return actions;
    };
    let logic = &doc.project.logic;
    let has_net = NETWORKS
        .iter()
        .any(|(kind, _)| logic.blocks.iter().any(|b| b.kind == *kind));
    for (kind, title) in NETWORKS {
        service(app, ui, kind, title, true, &mut actions);
    }
    // The web interface is served by a network service: its section appears with one.
    if has_net {
        service(app, ui, WEB_UI, "Веб-интерфейс", true, &mut actions);
    }
    actions
}

fn service(
    app: &EditorApp,
    ui: &mut Ui,
    kind: &'static str,
    title: &str,
    available: bool,
    actions: &mut Vec<NetAction>,
) {
    let Some(doc) = &app.doc else { return };
    let logic = &doc.project.logic;
    let block = logic.blocks.iter().find(|b| b.kind == kind);
    egui::CollapsingHeader::new(title)
        .id_salt(("net", kind))
        .default_open(block.is_some())
        .show(ui, |ui| {
            let mut on = block.is_some();
            let check = ui.add_enabled(available || on, egui::Checkbox::new(&mut on, "Включить"));
            if !available {
                check.on_hover_text("Сначала включите Wi-Fi или Ethernet");
            } else if check.changed() {
                actions.push(NetAction::Toggle { kind, on });
            }
            let Some(b) = block else { return };
            let Some(lib) = app.lib.resolve(logic, b) else {
                return;
            };
            let def = &lib.def;
            for p in def.active_params(&b.params) {
                let label = p.label.as_deref().unwrap_or(&p.name);
                let current = b.params.get(&p.name).or(p.default.as_ref());
                ui.horizontal(|ui| {
                    if !p.choices.is_empty() {
                        let selected =
                            current.map_or_else(|| "не задан".to_owned(), |v| p.value_label(v));
                        ui.label(label);
                        egui::ComboBox::from_id_salt(("net_param", &b.id, &p.name))
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for c in &p.choices {
                                    let sel = current == Some(&c.value);
                                    if ui.selectable_label(sel, &c.label).clicked() {
                                        actions.push(NetAction::SetParam {
                                            id: b.id.clone(),
                                            name: p.name.clone(),
                                            value: c.value.clone(),
                                        });
                                    }
                                }
                            });
                    } else if p.kind == ParamKind::Bool {
                        let mut v = matches!(current, Some(ParamValue::Bool(true)));
                        if ui
                            .checkbox(&mut v, label)
                            .on_hover_text(&p.description)
                            .changed()
                        {
                            actions.push(NetAction::SetParam {
                                id: b.id.clone(),
                                name: p.name.clone(),
                                value: ParamValue::Bool(v),
                            });
                        }
                    } else if let Some(var) = b.bound.get(&p.name) {
                        ui.label(format!("{label}: ↔ {var}"))
                            .on_hover_text(&p.description);
                        bind_control(
                            ui,
                            logic,
                            &b.id,
                            &p.name,
                            bindable_type(p.kind, false),
                            Some(var),
                            actions,
                        );
                    } else {
                        ui.label(label).on_hover_text(&p.description);
                        let set = |value| NetAction::SetParam {
                            id: b.id.clone(),
                            name: p.name.clone(),
                            value,
                        };
                        match p.kind {
                            ParamKind::Int => {
                                let mut v = match current {
                                    Some(ParamValue::Int(i)) => *i,
                                    _ => 0,
                                };
                                if ui.add(egui::DragValue::new(&mut v)).changed() {
                                    actions.push(set(ParamValue::Int(v)));
                                }
                            }
                            ParamKind::Float => {
                                let mut v = match current {
                                    Some(ParamValue::Float(f)) => *f,
                                    _ => 0.0,
                                };
                                if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                                    actions.push(set(ParamValue::Float(v)));
                                }
                            }
                            ParamKind::Text => {
                                let mut v = match current {
                                    Some(ParamValue::Text(t)) => t.clone(),
                                    _ => String::new(),
                                };
                                let secret = p.show == flp_core::ParamShow::Secret;
                                let edit = egui::TextEdit::singleline(&mut v)
                                    .password(secret)
                                    .desired_width(110.0);
                                if ui.add(edit).changed() {
                                    actions.push(set(ParamValue::Text(v)));
                                }
                            }
                            _ => {
                                ui.label(current.map_or_else(|| "не задан".to_owned(), value_text));
                            }
                        }
                        bind_control(
                            ui,
                            logic,
                            &b.id,
                            &p.name,
                            bindable_type(p.kind, false),
                            None,
                            actions,
                        );
                    }
                });
            }
            if ui.button("Настройки...").clicked() {
                actions.push(NetAction::Settings(b.id.clone()));
            }
            if kind == WEB_UI {
                let widgets = widget_blocks(logic).count();
                ui.label(format!("Виджетов на странице: {widgets}"));
                widget_list(app, ui, actions);
                if ui.button("Конструктор страницы...").clicked() {
                    actions.push(NetAction::Designer);
                }
            }
            ports(ui, b, def, logic, actions);
        });
}

fn widget_blocks(logic: &flp_core::Logic) -> impl Iterator<Item = &flp_core::Block> {
    logic
        .blocks
        .iter()
        .filter(|w| w.kind.starts_with("web_") && w.kind != WEB_UI)
}

/// Widgets of the page: the caption is the name shown in the editor; a click opens the settings.
fn widget_list(app: &EditorApp, ui: &mut Ui, actions: &mut Vec<NetAction>) {
    let Some(doc) = &app.doc else { return };
    let logic = &doc.project.logic;
    ui.menu_button("+ Добавить виджет", |ui| {
        for b in app.lib.iter() {
            let kind = &b.def.info.kind;
            let is_widget = kind.starts_with("web_") && kind != WEB_UI;
            if is_widget
                && ui
                    .button(&b.def.info.name)
                    .on_hover_text(&b.def.info.description)
                    .clicked()
            {
                actions.push(NetAction::AddWidget(kind.clone()));
                ui.close();
            }
        }
    });
    for w in widget_blocks(logic) {
        let caption = match w.params.get("caption") {
            Some(ParamValue::Text(c)) if !c.is_empty() => c.as_str(),
            _ => "без имени",
        };
        let sel = app.selection.as_ref().is_some_and(|s| s.has_block(&w.id));
        let resp = ui
            .selectable_label(sel, format!("{caption} ({})", w.id))
            .on_hover_text("Щелчок — найти на плате, двойной щелчок — параметры");
        if resp.double_clicked() {
            actions.push(NetAction::Settings(w.id.clone()));
        } else if resp.clicked() {
            actions.push(NetAction::Focus(w.id.clone()));
        }
    }
}

/// Ports of the service; each can be taken out to the sheet as a variable.
fn ports(
    ui: &mut Ui,
    b: &flp_core::Block,
    def: &flp_core::BlockDef,
    logic: &flp_core::Logic,
    actions: &mut Vec<NetAction>,
) {
    let inputs: Vec<_> = def.active_inputs(&b.params).collect();
    let outputs: Vec<_> = def.active_outputs(&b.params).collect();
    if inputs.is_empty() && outputs.is_empty() {
        return;
    }
    ui.separator();
    for (input, name, label, ty, description) in inputs
        .iter()
        .map(|p| (true, &p.name, p.label.as_deref(), p.ty, &p.description))
        .chain(
            outputs
                .iter()
                .map(|p| (false, &p.name, p.label.as_deref(), p.ty, &p.description)),
        )
    {
        let bound = b.bound.get(name.as_str());
        ui.horizontal(|ui| {
            let arrow = if input { "вход" } else { "выход" };
            let text = format!("{arrow} {} · {}", label.unwrap_or(name), ty_name(ty));
            ui.label(text).on_hover_text(description);
            bind_control(ui, logic, &b.id, name, Some(ty), bound, actions);
        });
    }
}

/// Button that takes a value from a variable of the type: the menu lists the
/// matching variables and offers a new one; a bound value shows a release button.
fn bind_control(
    ui: &mut Ui,
    logic: &flp_core::Logic,
    id: &str,
    name: &str,
    ty: Option<SignalType>,
    bound: Option<&String>,
    actions: &mut Vec<NetAction>,
) {
    let Some(ty) = ty else { return };
    if bound.is_some() {
        if ui
            .small_button("✕")
            .on_hover_text("Отвязать от переменной")
            .clicked()
        {
            actions.push(NetAction::Bind {
                id: id.to_owned(),
                name: name.to_owned(),
                var: None,
            });
        }
        return;
    }
    ui.menu_button("→ переменная", |ui| {
        for v in &logic.vars {
            let vt = v.ty.signal();
            if (ty == SignalType::Any || vt == ty) && ui.button(&v.name).clicked() {
                actions.push(NetAction::Bind {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    var: Some(v.name.clone()),
                });
                ui.close();
            }
        }
        if ty != SignalType::Any && ui.button("＋ Новая переменная").clicked() {
            actions.push(NetAction::BindNew {
                id: id.to_owned(),
                name: name.to_owned(),
                ty,
            });
            ui.close();
        }
    });
}

fn ty_name(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int",
        SignalType::Float => "float",
        SignalType::String => "String",
        SignalType::Any => "любой",
    }
}

fn var_type(ty: SignalType) -> VarType {
    match ty {
        SignalType::Bool => VarType::Bool,
        SignalType::Float => VarType::Float,
        SignalType::String => VarType::String,
        SignalType::Int | SignalType::Any => VarType::Int,
    }
}

pub fn apply(app: &mut EditorApp, actions: Vec<NetAction>) {
    for action in actions {
        let lib = app.lib.clone();
        let pos = crate::panels::free_spot(app);
        match action {
            NetAction::Focus(id) => app.focus_block(&id),
            NetAction::AddWidget(kind) => {
                let Some(doc) = &mut app.doc else { return };
                match doc.edit(&lib, None, |p| p.add_block(&lib, &kind, pos)) {
                    Ok(id) => {
                        app.focus_block(&id);
                        app.dialog = Dialog::for_block(app, &id);
                    }
                    Err(e) => app.info(LogLevel::Error, e.to_string()),
                }
            }
            NetAction::Designer => app.open_web_designer(),
            NetAction::Settings(id) => app.dialog = Dialog::for_block(app, &id),
            NetAction::Toggle { kind, on } => {
                let Some(doc) = &mut app.doc else { return };
                let result = doc.edit(&lib, None, |p| {
                    if on {
                        p.add_block(&lib, kind, pos)?;
                        if kind == WEB_UI {
                            // The page is served by the network service.
                            let nets: Vec<String> = p
                                .logic
                                .blocks
                                .iter()
                                .filter(|b| NETWORKS.iter().any(|(k, _)| *k == b.kind))
                                .map(|b| b.id.clone())
                                .collect();
                            for id in nets {
                                p.set_param(&lib, &id, "web", Some(ParamValue::Bool(true)))?;
                            }
                        }
                        Ok(())
                    } else {
                        let ids: Vec<String> = p
                            .logic
                            .blocks
                            .iter()
                            .filter(|b| b.kind == kind)
                            .map(|b| b.id.clone())
                            .collect();
                        ids.iter().try_for_each(|id| p.remove_block(id))
                    }
                });
                if let Err(e) = result {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
            NetAction::SetParam { id, name, value } => {
                let Some(doc) = &mut app.doc else { return };
                let key = Some(format!("net:{id}.{name}"));
                if let Err(e) = doc.edit(&lib, key, |p| p.set_param(&lib, &id, &name, Some(value)))
                {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
            NetAction::Bind { id, name, var } => {
                let Some(doc) = &mut app.doc else { return };
                if let Err(e) = doc.edit(&lib, None, |p| p.bind(&id, &name, var.as_deref())) {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
            NetAction::BindNew { id, name, ty } => {
                let Some(doc) = &mut app.doc else { return };
                let result = doc.edit(&lib, None, |p| {
                    let base = format!("{id}_{name}");
                    let var = std::iter::once(base.clone())
                        .chain((2..).map(|n| format!("{base}_{n}")))
                        .find(|n| p.logic.var(n).is_none())
                        .expect("infinite range");
                    p.add_var(Var {
                        name: var.clone(),
                        ty: var_type(ty),
                        init: None,
                        description: format!("{id}.{name}"),
                        retain: false,
                    })?;
                    p.bind(&id, &name, Some(&var))
                });
                if let Err(e) = result {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
        }
    }
}
