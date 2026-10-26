//! Network services in the left panel: Wi-Fi, Ethernet and the web interface.
//! A service is a hidden block of the schema: the check box adds or removes it,
//! the settings are its parameters, and every port or text/number parameter can
//! be taken out to the board as a variable (a binding stored in the block).

use eframe::egui::{self, Id, Ui};
use flp_core::{bindable_type, ParamKind, ParamValue, SignalType, Var, VarType};

use crate::app::{EditorApp, LogLevel};
use crate::canvas::DragItem;
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
                        if p.name == "wifi_mode" && def.input("mode").is_some() {
                            // Pulled out as the `mode` input: the choice becomes «set by input».
                            pull_label(
                                ui,
                                &b.id,
                                "mode",
                                label,
                                &p.description,
                                Some(SignalType::Int),
                                true,
                                b.bound.get("mode"),
                                actions,
                            );
                        } else {
                            ui.label(label);
                        }
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
                    } else if p.kind == ParamKind::Bool && !p.runtime {
                        // `use_<port>` / `out_<port>` switch a port on: it is active only
                        // when pulled out to the board, so there is no check box.
                        let port = p
                            .name
                            .strip_prefix("use_")
                            .or_else(|| p.name.strip_prefix("out_"))
                            .filter(|n| def.input(n).is_some() || def.output(n).is_some());
                        if let Some(port) = port {
                            let input = def.input(port).map(|d| d.ty);
                            let ty = input.or(def.output(port).map(|d| d.ty));
                            pull_label(
                                ui,
                                &b.id,
                                port,
                                label,
                                &p.description,
                                ty,
                                input.is_some(),
                                b.bound.get(port),
                                actions,
                            );
                        } else {
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
                        }
                    } else if let Some(var) = b.bound.get(&p.name) {
                        pull_label(
                            ui,
                            &b.id,
                            &p.name,
                            label,
                            &p.description,
                            bindable_type(p.kind, false, p.runtime),
                            true,
                            Some(var),
                            actions,
                        );
                    } else {
                        pull_label(
                            ui,
                            &b.id,
                            &p.name,
                            label,
                            &p.description,
                            bindable_type(p.kind, false, p.runtime),
                            true,
                            None,
                            actions,
                        );
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
                            ParamKind::Bool => {
                                let mut v = matches!(current, Some(ParamValue::Bool(true)));
                                if ui.checkbox(&mut v, "").changed() {
                                    actions.push(set(ParamValue::Bool(v)));
                                }
                            }
                            _ => {
                                ui.label(current.map_or_else(|| "не задан".to_owned(), value_text));
                            }
                        }
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

/// Name of a setting or port: dragging it onto the sheet takes the value out to the
/// board (a variable is created automatically); a bound one shows its variable and a
/// release button.
#[allow(clippy::too_many_arguments)]
fn pull_label(
    ui: &mut Ui,
    id: &str,
    name: &str,
    label: &str,
    hint: &str,
    ty: Option<SignalType>,
    feed: bool,
    bound: Option<&String>,
    actions: &mut Vec<NetAction>,
) {
    let Some(ty) = ty else {
        ui.label(label).on_hover_text(hint);
        return;
    };
    let item = DragItem::Setting {
        id: id.to_owned(),
        name: name.to_owned(),
        ty,
        feed,
    };
    ui.dnd_drag_source(Id::unique(("pull", id, name)), item, |ui| ui.label(label))
        .response
        .on_hover_text(format!(
            "{hint}
Перетащите на плату"
        ));
    if let Some(var) = bound {
        ui.label(format!("↔ {var}"));
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
    }
}

/// Takes a setting out to the board: creates a service variable, binds it, puts its block
/// on the sheet and switches the port on. Returns the id of the block.
pub fn pull_out(
    p: &mut flp_core::Project,
    lib: &flp_core::Library,
    id: &str,
    name: &str,
    ty: SignalType,
    feed: bool,
    pos: flp_core::BlockPos,
) -> Result<String, flp_core::EditError> {
    let block = p.logic.block(id);
    let def = block.and_then(|b| lib.get(&b.kind)).map(|l| &l.def);
    // Already taken out: one more block of the same variable.
    if let Some(var) = block.and_then(|b| b.bound.get(name)).cloned() {
        return p.add_var_block(&var, feed, pos);
    }
    // The variable starts with the current value of the parameter.
    let init = def.filter(|d| d.output(name).is_none()).and_then(|d| {
        // The `mode` input continues the chosen mode.
        let source = if name == "mode" { "wifi_mode" } else { name };
        block
            .and_then(|b| b.params.get(source))
            .or_else(|| d.param(source).and_then(|q| q.default.as_ref()))
            .filter(|v| name != "mode" || matches!(v, ParamValue::Int(0..=2)))
            .cloned()
    });
    let base = format!("{id}_{name}");
    let var = std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}_{n}")))
        .find(|n| p.logic.var(n).is_none())
        .expect("infinite range");
    p.add_var(Var {
        name: var.clone(),
        ty: var_type(ty),
        init,
        description: format!("{id}.{name}"),
        retain: false,
        service: true,
    })?;
    p.bind(id, name, Some(&var))?;
    switch_port(p, lib, id, name, true)?;
    if name == "mode"
        && p.logic
            .block(id)
            .is_some_and(|b| b.params.contains_key("wifi_mode") || b.kind == "wifi")
    {
        p.set_param(lib, id, "wifi_mode", Some(ParamValue::Int(3)))?;
    }
    p.add_var_block(&var, feed, pos)
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
                let on = var.is_some();
                let result = doc.edit(&lib, None, |p| {
                    p.bind(&id, &name, var.as_deref())?;
                    switch_port(p, &lib, &id, &name, on)
                });
                if let Err(e) = result {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
        }
    }
}

/// Turns the `use_<port>` / `out_<port>` switch of a port on or off, when the block has one.
fn switch_param(
    p: &flp_core::Project,
    lib: &flp_core::Library,
    id: &str,
    port: &str,
) -> Option<String> {
    let block = p.logic.block(id)?;
    let def = &lib.resolve(&p.logic, block)?.def;
    ["use_", "out_"]
        .iter()
        .map(|prefix| format!("{prefix}{port}"))
        .find(|n| def.param(n).is_some())
}

fn switch_port(
    p: &mut flp_core::Project,
    lib: &flp_core::Library,
    id: &str,
    port: &str,
    on: bool,
) -> Result<(), flp_core::EditError> {
    match switch_param(p, lib, id, port) {
        Some(name) => p.set_param(lib, id, &name, Some(ParamValue::Bool(on))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flp_core::{BlockPos, Library, Project};

    const POS: BlockPos = BlockPos {
        sheet: 1,
        x: 0.0,
        y: 0.0,
    };

    #[test]
    fn pulled_output_reads_and_input_writes() {
        let (lib, _) =
            Library::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library")).unwrap();
        let mut p = Project::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/wifi_bound"
        ))
        .unwrap();
        let id = "wifi".to_owned();
        let out = pull_out(&mut p, &lib, &id, "address", SignalType::String, false, POS).unwrap();
        assert_eq!(p.logic.block(&out).unwrap().kind, flp_core::vars::VAR_GET);
        let inp = pull_out(&mut p, &lib, &id, "en", SignalType::Bool, true, POS).unwrap();
        assert_eq!(p.logic.block(&inp).unwrap().kind, flp_core::vars::VAR_SET);
    }

    #[test]
    fn pulled_param_is_an_ordinary_variable() {
        let (lib, _) =
            Library::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library")).unwrap();
        let mut p = Project::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/wifi_bound"
        ))
        .unwrap();
        let first = pull_out(
            &mut p,
            &lib,
            "wifi",
            "subnet",
            SignalType::String,
            true,
            POS,
        )
        .unwrap();
        assert_eq!(p.logic.block(&first).unwrap().kind, flp_core::vars::VAR_SET);
        let again = pull_out(
            &mut p,
            &lib,
            "wifi",
            "subnet",
            SignalType::String,
            true,
            POS,
        )
        .unwrap();
        assert_eq!(p.logic.block(&again).unwrap().kind, flp_core::vars::VAR_SET);
        let var = p
            .logic
            .block(&first)
            .unwrap()
            .var_name()
            .unwrap()
            .to_owned();
        assert!(p.logic.var(&var).unwrap().init.is_some());
    }

    #[test]
    fn pulled_mode_switches_to_input() {
        let (lib, _) =
            Library::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library")).unwrap();
        let mut p = Project::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/wifi_bound"
        ))
        .unwrap();
        pull_out(&mut p, &lib, "wifi", "mode", SignalType::Int, true, POS).unwrap();
        assert_eq!(
            p.logic.block("wifi").unwrap().params.get("wifi_mode"),
            Some(&ParamValue::Int(3))
        );
    }
}
