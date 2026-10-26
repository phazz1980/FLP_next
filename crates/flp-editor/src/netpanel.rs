//! Network services in the left panel: Wi-Fi, Ethernet and the web interface.
//! A service is a single block of the schema: the check box adds or removes it,
//! the settings are its parameters, and a port can be taken out to the sheet as a
//! variable.

use eframe::egui::{self, RichText, Ui};
use flp_core::{ParamKind, ParamValue, PortRef, SignalType, Var, VarType};

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
    Expose {
        id: String,
        port: String,
        input: bool,
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
    service(app, ui, WEB_UI, "Веб-интерфейс", has_net, &mut actions);
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
                    } else {
                        let text = match current {
                            Some(_) if p.show == flp_core::ParamShow::Secret => "••••".to_owned(),
                            Some(v) => value_text(v),
                            None => "не задан".to_owned(),
                        };
                        ui.label(format!("{label}: {text}"))
                            .on_hover_text(&p.description);
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
        let bound = if input {
            logic
                .links
                .iter()
                .find(|l| l.to.block == b.id && l.to.port == *name)
                .map(|l| l.from.to_string())
        } else {
            logic
                .links
                .iter()
                .find(|l| l.from.block == b.id && l.from.port == *name)
                .map(|l| l.to.to_string())
        };
        ui.horizontal(|ui| {
            let arrow = if input { "вход" } else { "выход" };
            ui.label(format!(
                "{arrow} {} · {}",
                label.unwrap_or(name),
                ty_name(ty)
            ))
            .on_hover_text(description);
            match bound {
                Some(to) => {
                    ui.label(RichText::new(format!("↔ {to}")).weak());
                }
                None => {
                    if ty != SignalType::Any
                        && ui
                            .small_button("→ переменная")
                            .on_hover_text("Создать переменную и поставить её блок на плату")
                            .clicked()
                    {
                        actions.push(NetAction::Expose {
                            id: b.id.clone(),
                            port: name.clone(),
                            input,
                        });
                    }
                }
            }
        });
    }
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
            NetAction::Expose { id, port, input } => {
                let Some(doc) = &mut app.doc else { return };
                let ty = {
                    let logic = &doc.project.logic;
                    logic
                        .block(&id)
                        .and_then(|b| app.lib.resolve(logic, b))
                        .and_then(|l| {
                            if input {
                                l.def.input(&port).map(|p| p.ty)
                            } else {
                                l.def.output(&port).map(|p| p.ty)
                            }
                        })
                };
                let Some(ty) = ty else { continue };
                let result = doc.edit(&lib, None, |p| {
                    let base = format!("{id}_{port}");
                    let name = std::iter::once(base.clone())
                        .chain((2..).map(|n| format!("{base}_{n}")))
                        .find(|n| p.logic.var(n).is_none())
                        .expect("infinite range");
                    p.add_var(Var {
                        name: name.clone(),
                        ty: var_type(ty),
                        init: None,
                        description: format!("{id}.{port}"),
                        retain: false,
                    })?;
                    let block = p.add_var_block(&name, !input, pos)?;
                    let (from, to) = if input {
                        (PortRef::new(&block, "out"), PortRef::new(&id, &port))
                    } else {
                        (PortRef::new(&id, &port), PortRef::new(&block, "in"))
                    };
                    p.connect(&lib, from, to)
                });
                if let Err(e) = result {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
        }
    }
}
