//! Network services in the left panel: Wi-Fi, Ethernet, the web interface and MQTT.
//! A service is a hidden block of the schema: the check box adds or removes it,
//! the settings are its parameters, and every port or text/number parameter can
//! be taken out to the board as a variable (a binding stored in the block).

use crab_core::{bindable_type, ParamKind, ParamValue, SignalType, Var, VarType};
use eframe::egui::{self, Id, Ui};

use crate::app::{EditorApp, LogLevel};
use crate::canvas::DragItem;
use crate::dialogs::{value_text, Dialog};

const NETWORKS: [(&str, &str); 2] = [("wifi", "Wi-Fi"), ("ethernet", "Ethernet")];
const WEB_UI: &str = "web_ui";
const MQTT: &str = "mqtt";
/// MQTT topic blocks of the left panel: (kind, list title, add button).
const TOPICS: [(&str, &str, &str); 2] = [
    ("mqtt_publish", "Публикации", "+ публикация"),
    ("mqtt_subscribe", "Подписки", "+ подписка"),
];

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
    /// Adds a hidden MQTT publication or subscription block.
    AddTopic(&'static str),
    Remove(String),
    Designer,
    Focus(String),
    /// Virtual bench in simulation: the network link is lost or back.
    BenchOffline(bool),
    /// Virtual bench in simulation: the MQTT broker stops answering or comes back.
    BenchBrokerDown(bool),
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
    // The web interface and MQTT ride on a network service: their sections appear with one.
    if has_net {
        service(
            app,
            ui,
            WEB_UI,
            crate::i18n::tr("Веб-интерфейс"),
            true,
            &mut actions,
        );
        service(app, ui, MQTT, "MQTT", true, &mut actions);
        bench_section(app, ui, &mut actions);
    }
    actions
}

/// Faults of the virtual bench for the hidden network blocks, which have no ports on a sheet.
fn bench_section(app: &EditorApp, ui: &mut Ui, actions: &mut Vec<NetAction>) {
    let (Some(sim), Some(doc)) = (&app.sim, &app.doc) else {
        return;
    };
    ui.add_space(4.0);
    ui.label(egui::RichText::new(crate::i18n::tr("Виртуальный стенд")).strong());
    let mut offline = sim.offline();
    if ui
        .checkbox(&mut offline, crate::i18n::tr("Нет связи с сетью"))
        .changed()
    {
        actions.push(NetAction::BenchOffline(offline));
    }
    if doc.project.logic.blocks.iter().any(|b| b.kind == MQTT) {
        let mut down = sim.broker_down();
        if ui
            .checkbox(&mut down, crate::i18n::tr("Брокер не отвечает"))
            .changed()
        {
            actions.push(NetAction::BenchBrokerDown(down));
        }
    }
    ui.separator();
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
    ui.add_space(4.0);
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .id_salt(("net", kind))
        .default_open(block.is_some())
        .show(ui, |ui| {
            // The web page is switched by a parameter: its settings and widgets stay.
            let web_switch = kind == WEB_UI && block.is_some();
            let mut on = match block {
                Some(b) if web_switch => {
                    !matches!(b.params.get("enabled"), Some(ParamValue::Bool(false)))
                }
                _ => block.is_some(),
            };
            let check = ui.add_enabled(
                available || on,
                egui::Checkbox::new(&mut on, crate::i18n::tr("Включить")),
            );
            if !available {
                check.on_hover_text(crate::i18n::tr("Сначала включите Wi-Fi или Ethernet"));
            } else if check.changed() {
                match block {
                    Some(b) if web_switch => actions.push(NetAction::SetParam {
                        id: b.id.clone(),
                        name: "enabled".into(),
                        value: ParamValue::Bool(on),
                    }),
                    _ => actions.push(NetAction::Toggle { kind, on }),
                }
            }
            if kind == WEB_UI
                && ui
                    .button(crate::i18n::tr("Конструктор страницы..."))
                    .clicked()
            {
                actions.push(NetAction::Designer);
            }
            if let Some(b) = block {
                // The whole block with its ports goes to the board, like a topic.
                let pullable = app
                    .lib
                    .resolve(logic, b)
                    .is_some_and(|l| l.def.info.pullable);
                let placed = doc.project.layout.blocks.contains_key(&b.id);
                if pullable {
                    ui.horizontal(|ui| {
                        ui.dnd_drag_source(
                            Id::unique(("service", &b.id)),
                            DragItem::Topic(b.id.clone()),
                            |ui| ui.label(if placed { "▣" } else { "▢" }),
                        )
                        .response
                        .on_hover_text(if placed {
                            crate::i18n::tr("Блок уже на плате; перетащите, чтобы переставить")
                        } else {
                            crate::i18n::tr("Перетащите весь блок на плату")
                        });
                        ui.label(crate::i18n::tr("Блок целиком"));
                    });
                }
                settings(app, ui, b, kind, actions);
            }
            if kind == MQTT && block.is_some() {
                topics_section(app, ui, actions);
            }
        });
    ui.separator();
}

/// Lists of MQTT publications and subscriptions: a topic is a hidden block whose settings
/// are edited here and whose ports are taken out to the board like those of Wi-Fi.
fn topics_section(app: &EditorApp, ui: &mut Ui, actions: &mut Vec<NetAction>) {
    let Some(doc) = &app.doc else { return };
    let logic = &doc.project.logic;
    for (kind, title, add) in TOPICS {
        ui.separator();
        ui.label(egui::RichText::new(title).strong());
        for b in logic.blocks.iter().filter(|b| b.kind == kind) {
            let lib = app.lib.resolve(logic, b);
            let topic = match b.params.get("topic").or_else(|| {
                lib.as_ref()
                    .and_then(|l| l.def.param("topic"))
                    .and_then(|p| p.default.as_ref())
            }) {
                Some(ParamValue::Text(t)) if !t.is_empty() => t.as_str(),
                _ => crate::i18n::tr("без топика"),
            };
            let placed = doc.project.layout.blocks.contains_key(&b.id);
            ui.dnd_drag_source(
                Id::unique(("topic", &b.id)),
                DragItem::Topic(b.id.clone()),
                |ui| ui.label(if placed { "▣" } else { "▢" }),
            )
            .response
            .on_hover_text(if placed {
                crate::i18n::tr("Блок уже на плате; перетащите, чтобы переставить")
            } else {
                crate::i18n::tr("Перетащите на плату")
            });
            egui::CollapsingHeader::new(format!("{topic} ({})", b.id))
                .id_salt(("mqtt_topic", &b.id))
                .default_open(false)
                .show(ui, |ui| {
                    settings(app, ui, b, kind, actions);
                    if let Some(lib) = &lib {
                        topic_ports(ui, b, &lib.def);
                    }
                    if ui.button(crate::i18n::tr("Удалить")).clicked() {
                        actions.push(NetAction::Remove(b.id.clone()));
                    }
                });
        }
        if ui.small_button(add).clicked() {
            actions.push(NetAction::AddTopic(kind));
        }
    }
    ui.separator();
}

/// Ports of a topic block: each can be dragged onto the board. A port of any type
/// takes the type chosen next to it.
fn topic_ports(ui: &mut Ui, b: &crab_core::Block, def: &crab_core::BlockDef) {
    ui.label(crate::i18n::tr("Порты (перетащите на плату):"));
    let ports = def
        .active_inputs(&b.params)
        .map(|p| (p, true))
        .chain(def.active_outputs(&b.params).map(|p| (p, false)));
    for (p, feed) in ports {
        ui.horizontal(|ui| {
            let mut ty = p.ty;
            let bound = b.bound.get(&p.name);
            if ty == SignalType::Any {
                let id = ui.make_persistent_id(("mqtt_any", &b.id, &p.name));
                ty = ui.data_mut(|d| *d.get_temp_mut_or(id, SignalType::Float));
                if bound.is_none() {
                    egui::ComboBox::from_id_salt(id)
                        .selected_text(type_label(ty))
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for t in [
                                SignalType::Bool,
                                SignalType::Int,
                                SignalType::Float,
                                SignalType::String,
                            ] {
                                if ui.selectable_label(ty == t, type_label(t)).clicked() {
                                    ui.data_mut(|d| d.insert_temp(id, t));
                                }
                            }
                        });
                }
            }
            let label = crate::i18n::tr(p.label.as_deref().unwrap_or(&p.name));
            pull_label(
                ui,
                &b.id,
                &p.name,
                label,
                &p.description,
                Some(ty),
                feed,
                bound,
            );
        });
    }
}

fn type_label(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => crate::i18n::tr("логический"),
        SignalType::Int => crate::i18n::tr("целый"),
        SignalType::Float => crate::i18n::tr("дробный"),
        SignalType::String => crate::i18n::tr("строка"),
        SignalType::Any => crate::i18n::tr("любой"),
        SignalType::Number => crate::i18n::tr("число"),
        SignalType::Same => crate::i18n::tr("однотипный"),
    }
}

/// Parameters and ports of one service block.
fn settings(
    app: &EditorApp,
    ui: &mut Ui,
    b: &crab_core::Block,
    kind: &str,
    actions: &mut Vec<NetAction>,
) {
    let Some(doc) = &app.doc else { return };
    let logic = &doc.project.logic;
    let Some(lib) = app.lib.resolve(logic, b) else {
        return;
    };
    let def = &lib.def;
    for p in def
        .active_params(&b.params)
        .filter(|p| !(kind == WEB_UI && (p.name == "pages" || p.name == "enabled")))
    {
        let label = crate::i18n::tr(p.label.as_deref().unwrap_or(&p.name));
        // Values kept by the program (Wi-Fi password) are not in the project file.
        let stored = p.local.as_ref().and_then(|key_param| {
            let key = match b
                .params
                .get(key_param)
                .or(def.param(key_param)?.default.as_ref())
            {
                Some(ParamValue::Text(s)) => s.as_str(),
                _ => "",
            };
            app.local
                .get(&b.kind, &p.name, key)
                .map(|v| ParamValue::Text(v.to_owned()))
        });
        let current = stored
            .as_ref()
            .or(b.params.get(&p.name))
            .or(p.default.as_ref());
        ui.horizontal(|ui| {
            if !p.choices.is_empty() {
                let selected = current.map_or_else(
                    || crate::i18n::tr("не задан").to_owned(),
                    |v| crate::i18n::tr(&p.value_label(v)).to_owned(),
                );
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
                    );
                } else {
                    ui.label(label);
                }
                egui::ComboBox::from_id_salt(("net_param", &b.id, &p.name))
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for c in &p.choices {
                            let sel = current == Some(&c.value);
                            if ui
                                .selectable_label(sel, crate::i18n::tr(&c.label))
                                .clicked()
                            {
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
                );
                // A parameter made an input in the block settings is fed by its input.
                if b.param_in.contains(&p.name) {
                    return;
                }
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
                        let secret = p.show == crab_core::ParamShow::Secret;
                        let reveal = ui.make_persistent_id(("net_reveal", &b.id, &p.name));
                        let shown =
                            secret && ui.data(|d| d.get_temp::<bool>(reveal)).unwrap_or(false);
                        let edit = egui::TextEdit::singleline(&mut v)
                            .password(secret && !shown)
                            .desired_width(110.0);
                        // Parameters kept by the program under this one's value
                        // (SSID -> password).
                        let dependents: Vec<&crab_core::ParamDef> = def
                            .params
                            .iter()
                            .filter(|q| q.local.as_deref() == Some(p.name.as_str()))
                            .collect();
                        let changed = if dependents.is_empty() {
                            ui.add(edit).changed()
                        } else {
                            // Remembered keys (entered SSIDs) are suggested while typing.
                            let known: std::collections::BTreeSet<&str> = dependents
                                .iter()
                                .flat_map(|q| app.local.keys(kind, &q.name))
                                .collect();
                            let id = ui.make_persistent_id(("net_suggest", &b.id, &p.name));
                            let out = egui::CompletionPopup::new(id)
                                .word_boundary(|_| false)
                                .show(ui, edit, |query| {
                                    let typed = query.text.to_lowercase();
                                    known
                                        .iter()
                                        .filter(|k| {
                                            **k != query.text && k.to_lowercase().contains(&typed)
                                        })
                                        .map(|k| egui::Suggestion::new(*k))
                                        .collect()
                                });
                            out.accepted.is_some() || out.text_edit.response.changed()
                        };
                        if secret
                            && ui
                                .selectable_label(shown, "\u{1f441}")
                                .on_hover_text(crate::i18n::tr("Показать или скрыть"))
                                .clicked()
                        {
                            ui.data_mut(|d| d.insert_temp(reveal, !shown));
                        }
                        if changed {
                            // A new key brings its stored values (the password of that SSID).
                            for q in &dependents {
                                let stored = app.local.get(kind, &q.name, &v).unwrap_or_default();
                                actions.push(NetAction::SetParam {
                                    id: b.id.clone(),
                                    name: q.name.clone(),
                                    value: ParamValue::Text(stored.to_owned()),
                                });
                            }
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
    if ui.button(crate::i18n::tr("Настройки...")).clicked() {
        actions.push(NetAction::Settings(b.id.clone()));
    }
    if kind == WEB_UI {
        pages_section(app, ui, actions);
        if ui
            .button(crate::i18n::tr("Конструктор страницы..."))
            .clicked()
        {
            actions.push(NetAction::Designer);
        }
    }
}

fn widget_blocks(logic: &crab_core::Logic) -> impl Iterator<Item = &crab_core::Block> {
    logic
        .blocks
        .iter()
        .filter(|w| w.kind.starts_with("web_") && w.kind != WEB_UI && w.tag.is_none())
}

/// Names of the web interface pages (at least one).
pub fn page_names(logic: &crab_core::Logic) -> Vec<String> {
    let names: Vec<String> = logic
        .blocks
        .iter()
        .find(|b| b.kind == WEB_UI)
        .and_then(|b| match b.params.get("pages") {
            Some(ParamValue::Text(t)) => Some(t.split(';').map(|n| n.trim().to_owned()).collect()),
            _ => None,
        })
        .unwrap_or_default();
    if names.is_empty() {
        vec![DEFAULT_PAGE.to_owned()]
    } else {
        names
    }
}

const DEFAULT_PAGE: &str = "Главная";

/// 1-based page of a widget, clamped to the existing pages.
fn widget_page(w: &crab_core::Block, pages: usize) -> usize {
    match w.params.get("page") {
        Some(ParamValue::Int(i)) => (*i).clamp(1, pages as i64) as usize,
        _ => 1,
    }
}

/// Pages with their widgets (they are made in the web designer): the name of the page and
/// its widgets; a click on a widget finds it on the sheet, a double click opens its settings.
fn pages_section(app: &EditorApp, ui: &mut Ui, actions: &mut Vec<NetAction>) {
    let Some(doc) = &app.doc else { return };
    let logic = &doc.project.logic;
    let names = page_names(logic);
    ui.separator();
    for (i, name) in names.iter().enumerate() {
        ui.label(format!("{}. {name}", i + 1));
        ui.indent(("page", i), |ui| {
            for w in widget_blocks(logic).filter(|w| widget_page(w, names.len()) == i + 1) {
                let caption = match w.params.get("caption") {
                    Some(ParamValue::Text(c)) if !c.is_empty() => c.as_str(),
                    _ => crate::i18n::tr("без имени"),
                };
                let sel = app.selection.as_ref().is_some_and(|s| s.has_block(&w.id));
                // Like a variable: a widget that is on no sheet yet is highlighted.
                let placed = std::iter::once(w.id.as_str())
                    .chain(logic.tag_copies(&w.id))
                    .any(|id| doc.project.layout.blocks.contains_key(id));
                let text = egui::RichText::new(format!("{caption} ({})", w.id));
                let text = if placed {
                    text
                } else {
                    text.color(ui.visuals().warn_fg_color)
                };
                let text = if sel { text.strong() } else { text };
                let resp = crate::canvas::drag_source(
                    ui,
                    Id::unique(("widget", &w.id)),
                    DragItem::Topic(w.id.clone()),
                    |ui| ui.label(text),
                )
                .on_hover_text(crate::i18n::tr(
                    "Щелчок — найти на плате, двойной щелчок — параметры. Перетащите на плату: можно несколько раз, все изображения работают с одним виджетом. Не поставленный на плату виджет подсвечен.",
                ));
                if resp.double_clicked() {
                    actions.push(NetAction::Settings(w.id.clone()));
                } else if resp.clicked() {
                    actions.push(NetAction::Focus(w.id.clone()));
                }
            }
        });
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
        .on_hover_text(crate::i18n::fmt(
            "{0}
Перетащите на плату",
            &[&hint],
        ));
    if let Some(var) = bound {
        ui.label(format!("↔ {var}"));
    }
}

/// Takes a setting out to the board: creates a service variable, binds it, puts its block
/// on the sheet and switches the port on. Returns the id of the block.
pub fn pull_out(
    p: &mut crab_core::Project,
    lib: &crab_core::Library,
    id: &str,
    name: &str,
    ty: SignalType,
    feed: bool,
    pos: crab_core::BlockPos,
) -> Result<String, crab_core::EditError> {
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
    p.switch_port(lib, id, name, true)?;
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
        SignalType::Int | SignalType::Any | SignalType::Number | SignalType::Same => VarType::Int,
    }
}

pub fn apply(app: &mut EditorApp, actions: Vec<NetAction>) {
    for action in actions {
        let lib = app.lib.clone();
        let pos = crate::panels::free_spot(app);
        match action {
            NetAction::Focus(id) => app.focus_block(&id),
            NetAction::BenchOffline(on) => {
                if let Some(s) = &mut app.sim {
                    s.set_offline(on);
                }
            }
            NetAction::BenchBrokerDown(on) => {
                if let Some(s) = &mut app.sim {
                    s.set_broker_down(on);
                }
            }
            NetAction::Designer => app.open_web_designer(),
            NetAction::Settings(id) => app.dialog = Dialog::for_block(app, &id),
            NetAction::Toggle { kind, on } => {
                let Some(doc) = &mut app.doc else { return };
                let result = doc.edit(&lib, None, |p| {
                    if on {
                        let id = p.add_block(&lib, kind, pos)?;
                        // The block waits in the panel until it is dragged onto a sheet.
                        p.layout.blocks.remove(&id);
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
            NetAction::AddTopic(kind) => {
                let Some(doc) = &mut app.doc else { return };
                // The topic stays in the panel until it is dragged onto a sheet.
                let added = doc.edit(&lib, None, |p| {
                    let id = p.add_block(&lib, kind, pos)?;
                    p.layout.blocks.remove(&id);
                    Ok::<_, crab_core::EditError>(())
                });
                if let Err(e) = added {
                    app.info(LogLevel::Error, e.to_string());
                }
            }
            NetAction::Remove(id) => {
                let Some(doc) = &mut app.doc else { return };
                if let Err(e) = doc.edit(&lib, None, |p| p.remove_block(&id)) {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crab_core::{BlockPos, Library, Project};

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
        assert_eq!(p.logic.block(&out).unwrap().kind, crab_core::vars::VAR_GET);
        let inp = pull_out(&mut p, &lib, &id, "en", SignalType::Bool, true, POS).unwrap();
        assert_eq!(p.logic.block(&inp).unwrap().kind, crab_core::vars::VAR_SET);
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
        assert_eq!(
            p.logic.block(&first).unwrap().kind,
            crab_core::vars::VAR_SET
        );
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
        assert_eq!(
            p.logic.block(&again).unwrap().kind,
            crab_core::vars::VAR_SET
        );
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

    #[test]
    fn removing_last_block_releases_the_binding() {
        let (lib, _) =
            Library::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library")).unwrap();
        let mut p = Project::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/wifi_bound"
        ))
        .unwrap();
        let before = p.logic.block("wifi").unwrap().clone();
        let first = pull_out(&mut p, &lib, "wifi", "mode", SignalType::Int, true, POS).unwrap();
        let second = pull_out(&mut p, &lib, "wifi", "mode", SignalType::Int, true, POS).unwrap();
        // One block left: the binding stays.
        p.remove_block(&first).unwrap();
        p.release_unused_services(&lib).unwrap();
        assert!(p.logic.block("wifi").unwrap().bound.contains_key("mode"));
        // The last one gone: everything is back.
        p.remove_block(&second).unwrap();
        p.release_unused_services(&lib).unwrap();
        let after = p.logic.block("wifi").unwrap();
        assert!(!after.bound.contains_key("mode"));
        // The mode is back (explicitly or by default).
        let mode = |b: &crab_core::Block| b.params.get("wifi_mode").cloned();
        assert_eq!(
            mode(after).unwrap_or(ParamValue::Int(0)),
            mode(&before).unwrap_or(ParamValue::Int(0))
        );
        assert!(p.logic.vars.iter().all(|v| !v.service));
    }
}
