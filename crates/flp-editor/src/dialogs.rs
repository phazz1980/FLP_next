//! Pop-up windows with apply and cancel buttons: block properties, variable, sheet.
//!
//! Changes are buffered in the dialog and applied as one undo step on ✓.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use eframe::egui::{self, Color32, RichText, Ui};
use flp_core::expander::{free_expander_id, pin_param, taken_pins};
use flp_core::pins::{pin_uses, BoardLayout, PinUse};
use flp_core::value::parse_duration_ms;
use flp_core::{
    param_input_type, BlockDef, Expander, ExpanderChip, LocalValues, Logic, ParamDef, ParamKind,
    ParamShow, ParamValue, PortDef, SheetInfo, SignalType, Var, VarType,
};

use crate::app::{EditorApp, LogLevel, Selection};

pub enum Dialog {
    Block(BlockDialog),
    Var(VarDialog),
    Sheet(SheetDialog),
    Board(crate::boardcfg::BoardDialog),
    Expander(ExpanderDialog),
}

/// An expander of the project tree: new (`original` is `None`) or being changed.
pub struct ExpanderDialog {
    original: Option<String>,
    form: Expander,
}

pub struct BlockDialog {
    id: String,
    new_id: String,
    kind: String,
    /// Parameter values; `None` means "use the default". Values of `local`
    /// parameters (passwords) come from [`LocalValues`] and go back there.
    values: BTreeMap<String, Option<ParamValue>>,
    /// Parameters that are inputs of the block instead of constants.
    param_in: BTreeSet<String>,
    /// Text being typed for string and duration parameters.
    texts: BTreeMap<String, String>,
    /// Secret parameters shown as plain text (the eye button).
    revealed: BTreeSet<String>,
    /// Page names of the web interface, for the `page` parameter of a widget (empty otherwise).
    pages: Vec<String>,
    /// Where the pin parameter points: a GPIO of the board or a pin of an expander.
    source: PinSource,
    /// The expander being declared while `source` is [`PinSource::New`].
    pending: Expander,
}

/// What the pin parameter of a digital input or output refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinSource {
    Gpio,
    /// An expander of the project, by id.
    Expander(String),
    /// An expander created together with the block.
    New,
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
    background: bool,
    period: u32,
    disabled: bool,
}

enum Outcome {
    Open,
    Apply,
    Cancel,
    Delete,
}

/// Declaration offered in the "new expander" form: the first free id, the first chip.
fn new_expander(logic: Option<&Logic>) -> Expander {
    blank_expander(&logic.map_or_else(|| "pcf1".to_owned(), free_expander_id))
}

/// An expander with the default chip and address under the given id.
fn blank_expander(id: &str) -> Expander {
    let chip = ExpanderChip::default();
    Expander::new(id, chip, Expander::default_address(chip))
}

impl Dialog {
    pub fn for_block(app: &EditorApp, id: &str) -> Option<Self> {
        let doc = app.doc.as_ref()?;
        // A copy of a tag is edited through the tag: the settings are shared.
        let id = doc.project.logic.tag_of(id);
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
            param_in: block.param_in.clone(),
            texts,
            revealed: BTreeSet::new(),
            pages: if is_widget_kind(&block.kind) {
                crate::netpanel::page_names(&doc.project.logic)
            } else {
                Vec::new()
            },
            source: block
                .expander
                .clone()
                .map_or(PinSource::Gpio, PinSource::Expander),
            pending: new_expander(Some(&doc.project.logic)),
        }))
    }

    /// Window for a new web widget on the given page (0-based).
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

    /// Window of an expander of the project; `None` declares a new one.
    pub fn for_expander(app: &EditorApp, id: Option<&str>) -> Option<Self> {
        let logic = &app.doc.as_ref()?.project.logic;
        let form = match id {
            Some(id) => logic.expander(id)?.clone(),
            None => new_expander(Some(logic)),
        };
        Some(Self::Expander(ExpanderDialog {
            original: id.map(str::to_owned),
            form,
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
            background: info.background,
            disabled: info.disabled,
            period: if info.period == 0 {
                flp_core::layout::DEFAULT_BACKGROUND_PERIOD_MS
            } else {
                info.period
            },
        })
    }
}

/// Shows the open dialog, if any, and applies or drops it.
pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.dialog.take() else {
        return;
    };
    let title = match &dialog {
        Dialog::Block(d) => crate::i18n::fmt("Свойства блока «{0}»", &[&d.id]),
        Dialog::Var(d) => match &d.original {
            Some(n) => crate::i18n::fmt("Переменная «{0}»", &[&n]),
            None => crate::i18n::tr("Новая переменная").to_owned(),
        },
        Dialog::Sheet(d) => crate::i18n::fmt("Плата {0}", &[&d.number]),
        Dialog::Board(_) => crate::i18n::tr("Настройки МК").to_owned(),
        Dialog::Expander(d) => match &d.original {
            Some(id) => crate::i18n::fmt("Расширитель «{0}»", &[&id]),
            None => crate::i18n::tr("Новый расширитель").to_owned(),
        },
    };
    let mut open = true;
    let mut outcome = Outcome::Open;
    // Block properties can fill the whole window. The maximized window has its own id,
    // so the normal one keeps its size and position for the restore.
    let full_id = egui::Id::unique("block_dialog_full");
    let mut full: bool = ctx.data(|d| d.get_temp(full_id)).unwrap_or(false);
    let is_block = matches!(dialog, Dialog::Block(_));
    let mut window = egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(380.0);
    if is_block && full {
        let screen = ctx.content_rect();
        window = window
            .id(egui::Id::unique("block_dialog_maximized"))
            .fixed_pos(screen.min + egui::vec2(4.0, 4.0))
            .fixed_size(screen.size() - egui::vec2(24.0, 56.0));
    }
    window.show(ctx, |ui| {
        if is_block {
            let label = if full {
                "Свернуть окно"
            } else {
                "На весь экран"
            };
            if ui.button(crate::i18n::tr(label)).clicked() {
                full = !full;
                ui.ctx().data_mut(|d| d.insert_temp(full_id, full));
            }
        }
        outcome = buttons(
            ui,
            match &dialog {
                Dialog::Var(d) => d.original.is_some(),
                Dialog::Expander(d) => d.original.is_some(),
                _ => false,
            },
        );
        ui.separator();
        match &mut dialog {
            Dialog::Block(d) => block_body(app, ui, d),
            Dialog::Var(d) => var_body(ui, d),
            Dialog::Sheet(d) => sheet_body(ui, d),
            Dialog::Board(d) => {
                d.body(ui, &app.ports);
                if let Some(request) = d.take_editor_request() {
                    app.board_editor = match request {
                        None => Some(crate::boardedit::BoardEditor::new_board(app)),
                        Some(id) => app
                            .lib
                            .boards()
                            .get(&id)
                            .map(crate::boardedit::BoardEditor::edit),
                    };
                }
            }
            Dialog::Expander(d) => expander_body(app, ui, d),
        }
    });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    if !matches!(outcome, Outcome::Open) {
        ctx.data_mut(|d| d.insert_temp(full_id, false));
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
            let lib = app.lib.clone();
            match &dialog {
                Dialog::Var(VarDialog {
                    original: Some(name),
                    ..
                }) => {
                    if let Some(doc) = &mut app.doc {
                        if let Err(e) = doc.edit(&lib, None, |p| p.remove_var(name)) {
                            app.info(LogLevel::Error, e.to_string());
                        }
                    }
                    app.selection = None;
                    false
                }
                Dialog::Expander(ExpanderDialog {
                    original: Some(id), ..
                }) => {
                    // Refused while blocks stand on the chip: the window stays open.
                    let result = match &mut app.doc {
                        Some(doc) => doc.edit(&lib, None, |p| p.remove_expander(id)),
                        None => Ok(()),
                    };
                    match result {
                        Ok(()) => false,
                        Err(e) => {
                            app.info(LogLevel::Error, e.to_string());
                            true
                        }
                    }
                }
                _ => false,
            }
        }
    };
    if keep {
        app.dialog = Some(dialog);
    }
}

/// ✓ / ✗ row at the top of the window.
fn buttons(ui: &mut Ui, deletable: bool) -> Outcome {
    let mut outcome = Outcome::Open;
    ui.horizontal(|ui| {
        let ok = egui::Button::new(
            RichText::new(crate::i18n::tr("✔ Применить")).color(Color32::from_rgb(72, 136, 88)),
        );
        if ui.add(ok).clicked() {
            outcome = Outcome::Apply;
        }
        let cancel = egui::Button::new(
            RichText::new(crate::i18n::tr("✕ Отмена")).color(Color32::from_rgb(186, 74, 66)),
        );
        if ui.add(cancel).clicked() {
            outcome = Outcome::Cancel;
        }
        if deletable && ui.button(crate::i18n::tr("Удалить")).clicked() {
            outcome = Outcome::Delete;
        }
    });
    outcome
}

/// Web widgets (everything `web_*` but the service block) have a place on the page set in
/// the web designer, so their position and size are not edited here.
fn is_widget_kind(kind: &str) -> bool {
    kind.starts_with("web_") && kind != "web_ui"
}

fn block_body(app: &EditorApp, ui: &mut Ui, d: &mut BlockDialog) {
    let Some(doc) = &app.doc else { return };
    let Some(block) = doc.project.logic.block(&d.id) else {
        ui.weak(crate::i18n::tr("Блок удалён"));
        return;
    };
    let Some(lib_block) = app.lib.resolve(&doc.project.logic, block) else {
        ui.colored_label(
            Color32::from_rgb(186, 74, 66),
            crate::i18n::fmt("Неизвестный тип «{0}»", &[&block.kind]),
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
                .show(ui, |ui| {
                    let images = crate::markdown::dir_images(&lib_block.dir);
                    crate::markdown::show_with_images(ui, &docs, None, Some(&images));
                });
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
    ui.label(RichText::new(crate::i18n::tr(&def.info.name)).strong());
    ui.add(egui::Label::new(RichText::new(&def.info.description).weak()).wrap());
    ui.add_space(6.0);
    // Parameters and ports switched off by `when` follow the values being edited.
    let current: BTreeMap<String, ParamValue> = d
        .values
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), v.clone()?)))
        .collect();
    let params: Vec<&ParamDef> = def
        .active_params(&current)
        .filter(|p| {
            !p.fixed
                && !(is_widget_kind(&d.kind)
                    && (matches!(
                        p.name.as_str(),
                        "col" | "row" | "w" | "h" | "page" | "color" | "back"
                    ) || (d.kind == "web_text_input"
                        && matches!(p.name.as_str(), "initial" | "secret"))))
                && !(d.kind == "web_ui" && matches!(p.name.as_str(), "pages" | "enabled"))
        })
        .collect();
    let inputs: Vec<&PortDef> = def.active_inputs(&current).collect();
    let outputs: Vec<&PortDef> = def.active_outputs(&current).collect();
    let can_expander = def.info.expander && pin_param(def).is_some();
    let pins = Pins {
        layout: BoardLayout::of_board(&doc.project.config.board, app.lib.boards()),
        others: pin_uses(&doc.project.logic, &app.lib)
            .into_iter()
            .filter(|u| u.block != d.id)
            .collect(),
        expanders: if can_expander {
            doc.project
                .logic
                .expanders
                .iter()
                .map(|e| ExpanderPins {
                    expander: e.clone(),
                    taken: taken_pins(&doc.project.logic, &app.lib, &e.id, Some(&d.id)),
                })
                .collect()
        } else {
            Vec::new()
        },
        can_expander,
        free_id: free_expander_id(&doc.project.logic),
    };
    egui::Grid::new("block_props")
        .num_columns(3)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label(crate::i18n::tr("Надпись (id)"));
            ui.text_edit_singleline(&mut d.new_id);
            ui.label(RichText::new(crate::i18n::tr("Вход")).weak());
            ui.end_row();
            for &p in &params {
                let hint = match &p.label {
                    Some(_) => format!("{} ({})", p.description, p.name),
                    None => p.description.clone(),
                };
                ui.label(crate::i18n::tr(p.title()))
                    .on_hover_text(crate::i18n::tr(hint.trim()));
                let as_input = d.param_in.contains(&p.name);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!as_input, |ui| {
                        param_editor(ui, p, d, def, &app.local, &pins);
                    });
                });
                if p.name == "wifi_mode" {
                    // The mode comes from the «Mode» input when the last choice is set.
                    let mut on = matches!(d.values.get(&p.name), Some(Some(ParamValue::Int(3))));
                    if ui
                        .checkbox(&mut on, "")
                        .on_hover_text(crate::i18n::tr(
                            "Режим приходит на вход «Режим»: 0 клиент, 1 точка доступа, 2 клиент и точка доступа",
                        ))
                        .changed()
                    {
                        d.values
                            .insert(p.name.clone(), Some(ParamValue::Int(if on { 3 } else { 0 })));
                    }
                } else if as_input
                    || (param_input_type(p).is_some() && def.input(&p.name).is_none())
                {
                    let mut on = as_input;
                    if ui
                        .checkbox(&mut on, "")
                        .on_hover_text(crate::i18n::tr(
                            "Значение приходит на вход блока и читается на каждом скане; число в поле — значение входа, пока он не подключён",
                        ))
                        .changed()
                    {
                        if on {
                            d.param_in.insert(p.name.clone());
                        } else {
                            d.param_in.remove(&p.name);
                        }
                    }
                }
                else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
    if matches!(
        d.kind.as_str(),
        "modbus_master_read" | "modbus_master_write"
    ) {
        external_tag_picker(ui, d, &doc.project.logic);
    }
    let from_board = pins.layout.is_some() && pending_board_index(d, &pins).is_some();
    if pins.can_expander && d.source == PinSource::New && !from_board {
        ui.add_space(6.0);
        new_expander_form(ui, &mut d.pending, &doc.project.logic);
    }
    if !inputs.is_empty() || !outputs.is_empty() {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(crate::i18n::tr("Входы и выходы"))
            .default_open(true)
            .show(ui, |ui| {
                for input in &inputs {
                    let source =
                        doc.project
                            .logic
                            .links
                            .iter()
                            .find(|l| l.to.block == d.id && l.to.port == input.name)
                            .map(|l| format!("← {}", l.from))
                            .or_else(|| {
                                input.default.as_ref().map(|v| {
                                    crate::i18n::fmt("не подключён, {0}", &[&value_text(v)])
                                })
                            })
                            .unwrap_or_else(|| "не подключён".to_owned());
                    ui.label(crate::i18n::fmt(
                        "вход {0} · {1}: {2}",
                        &[&input.name, &type_name(input.ty), &source],
                    ))
                    .on_hover_text(&input.description);
                }
                for output in &outputs {
                    ui.label(crate::i18n::fmt(
                        "выход {0} · {1}",
                        &[&output.name, &type_name(output.ty)],
                    ))
                    .on_hover_text(&output.description);
                }
            });
    }
}

/// Fills table, value type and address of a Modbus master read / write block from a tag
/// taken from another project.
fn external_tag_picker(ui: &mut Ui, d: &mut BlockDialog, logic: &flp_core::Logic) {
    if logic.externals.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Внешний тег"));
        egui::ComboBox::from_id_salt("external_tag")
            .selected_text(crate::i18n::tr("выбрать..."))
            .show_ui(ui, |ui| {
                for e in &logic.externals {
                    if !ui.selectable_label(false, &e.name).clicked() {
                        continue;
                    }
                    let kind = match e.ty {
                        flp_core::VarType::Bool => "bool",
                        flp_core::VarType::Float | flp_core::VarType::Double => "float",
                        _ => "int",
                    };
                    for (name, text) in [("table", e.table.as_str()), ("kind", kind)] {
                        d.texts.insert(name.into(), text.into());
                        d.values
                            .insert(name.into(), Some(ParamValue::Text(text.into())));
                    }
                    d.values
                        .insert("reg".into(), Some(ParamValue::Int(i64::from(e.address))));
                }
            })
            .response
            .on_hover_text(crate::i18n::tr(
                "Подставляет таблицу, тип и адрес тега, перенесённого из другого проекта",
            ));
    });
}

/// An on / off switch: a rounded track with a knob that slides to the side of the state.
fn toggle_switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(34.0, 18.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let visuals = ui.style().interact_selectable(&response, *on);
    let track = if *on {
        Color32::from_rgb(70, 140, 90)
    } else {
        visuals.bg_fill
    };
    let radius = rect.height() / 2.0;
    ui.painter().rect_filled(rect, radius, track);
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    ui.painter().circle_filled(
        egui::pos2(x, rect.center().y),
        radius - 2.5,
        Color32::from_gray(235),
    );
    response
}

/// Board pins and the pins other blocks of the schema take, for the pin list.
struct Pins {
    /// `None` for a board without a pin table: the number is typed then.
    layout: Option<BoardLayout>,
    others: Vec<PinUse>,
    /// Expanders of the project with the pins taken on each, for blocks that may stand
    /// on one (empty otherwise).
    expanders: Vec<ExpanderPins>,
    /// The block type may stand on an expander pin.
    can_expander: bool,
    /// Id a new expander would get.
    free_id: String,
}

struct ExpanderPins {
    expander: Expander,
    /// Pin of the chip and the block that holds it.
    taken: Vec<(i64, String)>,
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
    if p.name == "page" && !d.pages.is_empty() {
        let page = match current {
            Some(ParamValue::Int(i)) if i >= 1 => i as usize,
            _ => 1,
        };
        let shown = d
            .pages
            .get(page - 1)
            .cloned()
            .unwrap_or_else(|| crate::i18n::fmt("Страница {0}", &[&page]));
        egui::ComboBox::from_id_salt("widget_page")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (i, name) in d.pages.iter().enumerate() {
                    if ui.selectable_label(i + 1 == page, name).clicked() {
                        d.values
                            .insert(p.name.clone(), Some(ParamValue::Int(i as i64 + 1)));
                    }
                }
            });
        return;
    }
    let expander_pin = pins.can_expander && pin_param(def).is_some_and(|q| q.name == p.name);
    if expander_pin && pins.layout.is_none() {
        // A board without a pin table: the source and the pin are chosen separately.
        pin_source_combo(ui, p, d, pins);
        if d.source != PinSource::Gpio {
            expander_pin_combo(ui, p, d, pins);
            return;
        }
    }
    if p.kind == ParamKind::Pin && pins.layout.is_some() {
        let current = match current {
            Some(ParamValue::Int(i)) => Some(i),
            _ => None,
        };
        pin_list_combo(ui, p, d, def, pins, current);
        return;
    }
    if !p.choices.is_empty() {
        let selected = current.as_ref().map_or_else(
            || crate::i18n::tr("не задан").to_owned(),
            |v| crate::i18n::tr(&p.value_label(v)).to_owned(),
        );
        egui::ComboBox::from_id_salt(("param_choice", &p.name))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for c in &p.choices {
                    let on = current.as_ref() == Some(&c.value);
                    if ui.selectable_label(on, crate::i18n::tr(&c.label)).clicked() {
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
            if toggle_switch(ui, &mut v).changed() {
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
            if ui.add(drag).changed()
                || (missing && ui.small_button(crate::i18n::tr("задать")).clicked())
            {
                set(ParamValue::Int(v));
            }
            if missing {
                ui.colored_label(Color32::from_rgb(186, 74, 66), crate::i18n::tr("не задан"));
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
                    .on_hover_text(crate::i18n::tr("Показать или скрыть"))
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

/// The declaration a project gets for an expander soldered on the board: the board's
/// chip, address and bus pins under a free id.
fn board_expander(board: &flp_core::pins::BoardExpander, id: &str) -> Expander {
    let mut e = board.expander.clone();
    e.id = id.to_owned();
    e
}

fn expander_title(e: &Expander) -> String {
    format!("{} · {} 0x{:02X}", e.id, e.chip.name(), e.address)
}

/// Chooses where the pin parameter points: a board GPIO, an expander of the project or
/// a new expander. A change of source clears the pin, since its number means another thing.
fn pin_source_combo(ui: &mut Ui, p: &ParamDef, d: &mut BlockDialog, pins: &Pins) {
    let label = |source: &PinSource| match source {
        PinSource::Gpio => crate::i18n::tr("GPIO платы").to_owned(),
        PinSource::Expander(id) => match pins.expanders.iter().find(|e| e.expander.id == *id) {
            Some(e) => expander_title(&e.expander),
            None => crate::i18n::fmt("{0} (нет в проекте)", &[&id]),
        },
        PinSource::New => crate::i18n::tr("+ добавить расширитель…").to_owned(),
    };
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("pin_source", &p.name))
        .selected_text(label(&d.source))
        .show_ui(ui, |ui| {
            let mut options = vec![PinSource::Gpio];
            options.extend(
                pins.expanders
                    .iter()
                    .map(|e| PinSource::Expander(e.expander.id.clone())),
            );
            options.push(PinSource::New);
            for option in options {
                if ui
                    .selectable_label(d.source == option, label(&option))
                    .clicked()
                {
                    chosen = Some(option);
                }
            }
        })
        .response
        .on_hover_text(crate::i18n::tr(
            "Вывод платы или вывод микросхемы-расширителя PCF по I²C",
        ));
    if let Some(source) = chosen.filter(|s| *s != d.source) {
        d.source = source;
        d.values.insert(p.name.clone(), None);
    }
}

/// Pins P0… of the chosen expander; pins other blocks hold are shown but cannot be chosen.
fn expander_pin_combo(ui: &mut Ui, p: &ParamDef, d: &mut BlockDialog, pins: &Pins) {
    let (chip, taken, chosen_chip): (ExpanderChip, &[(i64, String)], &Expander) = match &d.source {
        PinSource::Expander(id) => match pins.expanders.iter().find(|e| e.expander.id == *id) {
            Some(e) => (e.expander.chip, &e.taken, &e.expander),
            None => {
                ui.colored_label(
                    Color32::from_rgb(186, 74, 66),
                    crate::i18n::tr("расширителя нет в проекте"),
                );
                return;
            }
        },
        _ => (d.pending.chip, &[], &d.pending),
    };
    let current = match d.values.get(&p.name) {
        Some(Some(ParamValue::Int(i))) => Some(*i),
        _ => None,
    };
    let selected = match current {
        Some(v) if (0..i64::from(chip.pins())).contains(&v) => format!("P{v}"),
        Some(v) => crate::i18n::fmt("P{0} (нет у {1})", &[&v, &chip.name()]),
        None => crate::i18n::tr("выбрать…").to_owned(),
    };
    let rows = expander_rows(p, chip, taken, None, |pin| {
        PinChoice::Expander(chosen_chip.id.clone(), pin)
    });
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("expander_pin", &p.name))
        .selected_text(selected)
        .height(400.0)
        .show_ui(ui, |ui| {
            for row in rows {
                if let PinRow::Pin {
                    choice: PinChoice::Expander(_, pin),
                    text,
                    free,
                } = row
                {
                    let button = egui::Button::selectable(current == Some(pin), text);
                    if ui.add_enabled(free, button).clicked() {
                        chosen = Some(pin);
                    }
                }
            }
        });
    if let Some(pin) = chosen {
        d.values.insert(p.name.clone(), Some(ParamValue::Int(pin)));
    }
    if current.is_none() {
        ui.colored_label(Color32::from_rgb(186, 74, 66), crate::i18n::tr("не задан"));
    }
}

/// Declaration of an expander that is created together with the block.
fn new_expander_form(ui: &mut Ui, e: &mut Expander, logic: &Logic) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(crate::i18n::tr("Новый расширитель")).strong());
        expander_fields(ui, e, logic, None);
    });
}

/// Window of an expander in the project tree.
fn expander_body(app: &EditorApp, ui: &mut Ui, d: &mut ExpanderDialog) {
    let Some(doc) = &app.doc else { return };
    expander_fields(ui, &mut d.form, &doc.project.logic, d.original.as_deref());
    if let Some(id) = &d.original {
        let blocks: Vec<&str> = doc
            .project
            .logic
            .blocks
            .iter()
            .filter(|b| b.expander.as_deref() == Some(id.as_str()))
            .map(|b| b.id.as_str())
            .collect();
        ui.add_space(6.0);
        if blocks.is_empty() {
            ui.weak(crate::i18n::tr(
                "На расширителе нет блоков: его можно удалить.",
            ));
        } else {
            ui.weak(crate::i18n::fmt(
                "На расширителе стоят: {0}.",
                &[&blocks.join(", ")],
            ));
        }
    }
}

/// Fields of an expander declaration; `original` is its id while it is being changed.
fn expander_fields(ui: &mut Ui, e: &mut Expander, logic: &Logic, original: Option<&str>) {
    let red = Color32::from_rgb(186, 74, 66);
    {
        egui::Grid::new("new_expander")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(crate::i18n::tr("Имя"));
                ui.text_edit_singleline(&mut e.id);
                ui.end_row();
                ui.label(crate::i18n::tr("Микросхема"));
                egui::ComboBox::from_id_salt("new_expander_chip")
                    .selected_text(e.chip.name())
                    .show_ui(ui, |ui| {
                        for chip in ExpanderChip::ALL {
                            let text = crate::i18n::fmt(
                                "{0} · {1} выводов",
                                &[&chip.name(), &chip.pins()],
                            );
                            if ui.selectable_label(e.chip == chip, text).clicked() {
                                e.chip = chip;
                                if !chip.addresses().contains(&e.address) {
                                    e.address = Expander::default_address(chip);
                                }
                            }
                        }
                    });
                ui.end_row();
                ui.label(crate::i18n::tr("Адрес I²C"));
                ui.add(
                    egui::DragValue::new(&mut e.address)
                        .range(e.chip.addresses())
                        .hexadecimal(2, true, false)
                        .prefix("0x"),
                )
                .on_hover_text(crate::i18n::tr(
                    "Задаётся перемычками A0–A2 на плате расширителя",
                ));
                ui.end_row();
                ui.label(crate::i18n::tr("Шина"));
                ui.horizontal(|ui| {
                    let mut own = e.sda >= 0;
                    if ui
                        .checkbox(&mut own, crate::i18n::tr("свои выводы"))
                        .on_hover_text(crate::i18n::tr(
                            "Иначе шина по умолчанию (GPIO21 и GPIO22 у ESP32)",
                        ))
                        .changed()
                    {
                        (e.sda, e.scl) = if own { (21, 22) } else { (-1, -1) };
                    }
                    if own {
                        ui.add(
                            egui::DragValue::new(&mut e.sda)
                                .range(0..=48)
                                .prefix("SDA "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut e.scl)
                                .range(0..=48)
                                .prefix("SCL "),
                        );
                    }
                });
                ui.end_row();
                ui.label(crate::i18n::tr("Период опроса"));
                let period_ok = e.period_ms().is_some();
                let mut edit = egui::TextEdit::singleline(&mut e.period).desired_width(80.0);
                if !period_ok {
                    edit = edit.text_color(red);
                }
                ui.add(edit)
                    .on_hover_text(crate::i18n::tr("Как часто читаются входы: 20ms, 100ms…"));
                ui.end_row();
            });
        let others = || {
            logic
                .expanders
                .iter()
                .filter(|o| Some(o.id.as_str()) != original)
        };
        if others().any(|o| o.id == e.id) {
            ui.colored_label(red, crate::i18n::fmt("имя «{0}» уже занято", &[&e.id]));
        } else if let Some(problem) = e.problem() {
            ui.colored_label(red, problem);
        } else if let Some(other) =
            others().find(|o| o.address == e.address && o.sda == e.sda && o.scl == e.scl)
        {
            ui.colored_label(
                red,
                crate::i18n::fmt(
                    "адрес 0x{0} занят расширителем «{1}»",
                    &[&format!("{:02X}", e.address), &other.id],
                ),
            );
        }
    }
}

/// What a row of the pin list stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PinChoice {
    /// A GPIO of the board (`-1`: not used).
    Gpio(i64),
    /// A pin of an expander of the project.
    Expander(String, i64),
    /// A pin of an expander soldered on the board that the project does not have yet
    /// (index in the board's list); choosing it declares the expander in the project.
    Board(usize, i64),
    /// Declare an expander that is not on the board.
    NewExpander,
}

/// A row of the pin list: a group heading or something to choose.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PinRow {
    Heading(String),
    Pin {
        choice: PinChoice,
        text: String,
        free: bool,
    },
}

/// `P3` or `P3 · K4` for a pin of an expander.
fn expander_pin_name(pin: i64, label: &str) -> String {
    if label.is_empty() {
        format!("P{pin}")
    } else {
        format!("P{pin} · {label}")
    }
}

/// Rows for the pins of an expander chip. With a `board` table only the pins the board
/// brings out are listed, under the names printed on it; pins other blocks hold and pins
/// without the function of the parameter are shown but cannot be chosen.
fn expander_rows(
    p: &ParamDef,
    chip: ExpanderChip,
    taken: &[(i64, String)],
    board: Option<&flp_core::pins::BoardExpander>,
    choice: impl Fn(i64) -> PinChoice,
) -> Vec<PinRow> {
    let mut rows = Vec::new();
    for pin in 0..i64::from(chip.pins()) {
        let mut name = format!("P{pin}");
        let mut fits = true;
        if let Some(board) = board {
            let Some(info) = board.pin(pin) else { continue };
            name = expander_pin_name(pin, &info.label);
            fits = board.fits(pin, p.mode);
        }
        let holder = taken.iter().find(|(n, _)| *n == pin).map(|(_, b)| b);
        let text = match holder {
            Some(block) => crate::i18n::fmt("{0} — занят: «{1}»", &[&name, &block]),
            None if !fits => crate::i18n::fmt("{0} — не подходит блоку", &[&name]),
            None => name,
        };
        rows.push(PinRow::Pin {
            choice: choice(pin),
            text,
            free: holder.is_none() && fits,
        });
    }
    rows
}

/// Rows for the GPIOs of the board that fit the parameter; pins taken by other blocks
/// (or other parameters of this one) are shown but cannot be chosen.
fn gpio_rows(
    p: &ParamDef,
    d: &BlockDialog,
    def: &BlockDef,
    pins: &Pins,
    layout: &BoardLayout,
) -> Vec<PinRow> {
    let me = PinUse {
        block: d.id.clone(),
        kind: d.kind.clone(),
        param: p.name.clone(),
        gpio: 0,
        mode: p.mode,
        shared: p.shared,
        bus: p.bus.clone(),
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
                    Some((g, crate::i18n::fmt("«{0}» этого блока", &[&q.title()])))
                }
                _ => None,
            }
        })
        .collect();
    let mut rows = Vec::new();
    if p.min.is_some_and(|min| min < 0.0) {
        rows.push(PinRow::Pin {
            choice: PinChoice::Gpio(-1),
            text: crate::i18n::tr("не используется").to_owned(),
            free: true,
        });
    }
    for pin in layout.pins().iter().filter(|pin| pin.fits(p.mode)) {
        let mut text = pin.title();
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
            text += &crate::i18n::fmt(" — занят: {0}", &[&who]);
        } else if let Some(u) = pins.others.iter().find(|u| u.gpio == pin.gpio) {
            text += &crate::i18n::fmt(" — общий с «{0}»", &[&u.block]);
        }
        rows.push(PinRow::Pin {
            choice: PinChoice::Gpio(pin.gpio),
            text,
            free,
        });
    }
    rows
}

/// Index in the board's list of the expander the dialog declares, if it is one of them.
fn pending_board_index(d: &BlockDialog, pins: &Pins) -> Option<usize> {
    let layout = pins.layout.as_ref()?;
    layout.expanders().iter().position(|b| b.is(&d.pending))
}

/// The whole pin list of a parameter: GPIOs of the board first, then, for blocks that can
/// stand on an expander, the pins of the project's expanders and of the ones soldered on
/// the board. A row of an expander also tells where the pin is: no separate source.
fn pin_rows(p: &ParamDef, d: &BlockDialog, def: &BlockDef, pins: &Pins) -> Vec<PinRow> {
    let Some(layout) = pins.layout.as_ref() else {
        return Vec::new();
    };
    let mut rows = gpio_rows(p, d, def, pins, layout);
    if !(pins.can_expander && pin_param(def).is_some_and(|q| q.name == p.name)) {
        return rows;
    }
    for e in &pins.expanders {
        rows.push(PinRow::Heading(expander_title(&e.expander)));
        rows.extend(expander_rows(
            p,
            e.expander.chip,
            &e.taken,
            layout.expander_for(&e.expander),
            |pin| PinChoice::Expander(e.expander.id.clone(), pin),
        ));
    }
    // Expanders soldered on the board that the project does not use yet.
    for (i, b) in layout.expanders().iter().enumerate() {
        if pins.expanders.iter().any(|e| b.is(&e.expander)) {
            continue;
        }
        let list = expander_rows(p, b.expander.chip, &[], Some(b), |pin| {
            PinChoice::Board(i, pin)
        });
        if list
            .iter()
            .any(|r| matches!(r, PinRow::Pin { free: true, .. }))
        {
            rows.push(PinRow::Heading(crate::i18n::fmt(
                "Плата: {0} · 0x{1}",
                &[&b.title, &format!("{:02X}", b.expander.address)],
            )));
            rows.extend(list);
        }
    }
    rows
}

/// What the dialog points at now, in terms of the list rows.
fn current_choice(
    d: &BlockDialog,
    p: &ParamDef,
    pins: &Pins,
    gpio: Option<i64>,
) -> Option<PinChoice> {
    let own = match d.values.get(&p.name) {
        Some(Some(ParamValue::Int(i))) => Some(*i),
        _ => None,
    };
    match &d.source {
        PinSource::Gpio => gpio.map(PinChoice::Gpio),
        PinSource::Expander(id) => own.map(|v| PinChoice::Expander(id.clone(), v)),
        PinSource::New => pending_board_index(d, pins)
            .zip(own)
            .map(|(i, v)| PinChoice::Board(i, v)),
    }
}

/// Text of the closed list.
fn pin_list_caption(
    p: &ParamDef,
    d: &BlockDialog,
    pins: &Pins,
    layout: &BoardLayout,
    gpio: Option<i64>,
) -> String {
    let own = match d.values.get(&p.name) {
        Some(Some(ParamValue::Int(i))) => Some(*i),
        _ => None,
    };
    let on_chip = |chip: ExpanderChip, board: Option<&flp_core::pins::BoardExpander>, v: i64| {
        let label = board
            .and_then(|b| b.pin(v))
            .map_or("", |i| i.label.as_str());
        if (0..i64::from(chip.pins())).contains(&v) {
            expander_pin_name(v, label)
        } else {
            crate::i18n::fmt("P{0} (нет у {1})", &[&v, &chip.name()])
        }
    };
    match &d.source {
        PinSource::Gpio => match gpio {
            None => crate::i18n::tr("выбрать…").to_owned(),
            Some(v) if v < 0 => crate::i18n::tr("не используется").to_owned(),
            Some(v) => match layout.pin(v).filter(|pin| pin.fits(p.mode)) {
                Some(pin) => pin.title(),
                None => crate::i18n::fmt("GPIO{0} (не подходит)", &[&v]),
            },
        },
        PinSource::Expander(id) => match pins.expanders.iter().find(|e| e.expander.id == *id) {
            None => crate::i18n::fmt("{0} (нет в проекте)", &[&id]),
            Some(e) => match own {
                None => crate::i18n::fmt("{0} · выбрать…", &[&e.expander.id]),
                Some(v) => format!(
                    "{} · {}",
                    e.expander.id,
                    on_chip(e.expander.chip, layout.expander_for(&e.expander), v)
                ),
            },
        },
        PinSource::New => {
            let board = layout.expander_for(&d.pending);
            let who = match board {
                Some(b) => b.title.clone(),
                None => crate::i18n::tr("новый расширитель").to_owned(),
            };
            match own {
                None => crate::i18n::fmt("{0} · выбрать…", &[&who]),
                Some(v) => format!("{who} · {}", on_chip(d.pending.chip, board, v)),
            }
        }
    }
}

/// Drop-down list with every pin the parameter can use: GPIOs of the board and, for a
/// digital input or output, pins of expanders. Picking a row sets the source and the pin.
fn pin_list_combo(
    ui: &mut Ui,
    p: &ParamDef,
    d: &mut BlockDialog,
    def: &BlockDef,
    pins: &Pins,
    gpio: Option<i64>,
) {
    let Some(layout) = pins.layout.as_ref() else {
        return;
    };
    let rows = pin_rows(p, d, def, pins);
    let current = current_choice(d, p, pins, gpio);
    let expander_pin = pins.can_expander && pin_param(def).is_some_and(|q| q.name == p.name);
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("pin", &p.name))
        .selected_text(pin_list_caption(p, d, pins, layout, gpio))
        .height(400.0)
        .show_ui(ui, |ui| {
            for row in rows {
                match row {
                    PinRow::Heading(title) => {
                        ui.add_space(4.0);
                        ui.weak(title);
                    }
                    PinRow::Pin { choice, text, free } => {
                        let button =
                            egui::Button::selectable(current.as_ref() == Some(&choice), text);
                        if ui.add_enabled(free, button).clicked() {
                            chosen = Some(choice);
                        }
                    }
                }
            }
            if expander_pin {
                ui.separator();
                let own_expander =
                    d.source == PinSource::New && pending_board_index(d, pins).is_none();
                if ui
                    .selectable_label(own_expander, crate::i18n::tr("+ добавить расширитель…"))
                    .clicked()
                {
                    chosen = Some(PinChoice::NewExpander);
                }
            }
        })
        .response
        .on_hover_text(crate::i18n::fmt("Выводы {0}", &[&layout.name()]));
    let unset = if d.source == PinSource::Gpio {
        gpio.is_none()
    } else {
        !has_pin(d, p)
    };
    if unset {
        ui.colored_label(Color32::from_rgb(186, 74, 66), crate::i18n::tr("не задан"));
    }
    let Some(choice) = chosen else { return };
    let name = p.name.clone();
    match choice {
        PinChoice::Gpio(g) => {
            d.source = PinSource::Gpio;
            d.values.insert(name, Some(ParamValue::Int(g)));
        }
        PinChoice::Expander(id, pin) => {
            d.source = PinSource::Expander(id);
            d.values.insert(name, Some(ParamValue::Int(pin)));
        }
        PinChoice::Board(i, pin) => {
            if let Some(board) = layout.expanders().get(i) {
                if pending_board_index(d, pins) != Some(i) || d.source != PinSource::New {
                    d.pending = board_expander(board, &pins.free_id);
                }
                d.source = PinSource::New;
                d.values.insert(name, Some(ParamValue::Int(pin)));
            }
        }
        PinChoice::NewExpander => {
            d.source = PinSource::New;
            d.pending = blank_expander(&pins.free_id);
            d.values.insert(name, None);
        }
    }
}

/// The dialog has a pin number for the parameter.
fn has_pin(d: &BlockDialog, p: &ParamDef) -> bool {
    matches!(d.values.get(&p.name), Some(Some(ParamValue::Int(_))))
}

fn var_body(ui: &mut Ui, d: &mut VarDialog) {
    egui::Grid::new("var_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label(crate::i18n::tr("Имя"));
            ui.text_edit_singleline(&mut d.name);
            ui.end_row();
            ui.label(crate::i18n::tr("Тип"));
            egui::ComboBox::from_id_salt("var_type")
                .selected_text(d.ty.name())
                .height(400.0)
                .show_ui(ui, |ui| {
                    for ty in VarType::ALL {
                        ui.selectable_value(&mut d.ty, ty, ty.name());
                    }
                });
            ui.end_row();
            ui.label(crate::i18n::tr("Начальное значение"));
            let hint = match d.ty.signal() {
                SignalType::Bool => "false",
                SignalType::Int => "0",
                SignalType::Float => "0.0",
                SignalType::String | SignalType::Any | SignalType::Number | SignalType::Same => crate::i18n::tr("текст"),
            };
            let valid = parse_init(d.ty, &d.init).is_ok();
            let mut edit = egui::TextEdit::singleline(&mut d.init).hint_text(hint);
            if !valid {
                edit = edit.text_color(Color32::from_rgb(186, 74, 66));
            }
            ui.add(edit);
            ui.end_row();
            ui.label(crate::i18n::tr("Описание"));
            ui.text_edit_singleline(&mut d.description);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.retain, crate::i18n::tr("Сохранять в EEPROM")).on_hover_text(
                crate::i18n::tr("Значение хранится во флеш-памяти (NVS) и восстанавливается после перезагрузки.                  Запись — только когда значение изменилось, не чаще раза в секунду."),
            );
            ui.end_row();
            if let Some((_, en)) = &mut d.block {
                ui.label("");
                ui.checkbox(en, crate::i18n::tr("Вход разрешения записи (En)")).on_hover_text(
                    crate::i18n::tr("Только для этого блока: у него появляется логический вход En,                      значение записывается в переменную, только пока En = true."),
                );
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    ui.weak(crate::i18n::tr("Переменную можно читать на любой плате сколько угодно раз, а записывать — в одном месте. Если её никто не записывает, она работает как уставка."));
}

fn sheet_body(ui: &mut Ui, d: &mut SheetDialog) {
    egui::Grid::new("sheet_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label(crate::i18n::tr("Наименование"));
            ui.text_edit_singleline(&mut d.name);
            ui.end_row();
            ui.label(crate::i18n::tr("Комментарии"));
            ui.add(egui::TextEdit::multiline(&mut d.comment).desired_rows(3));
            ui.end_row();
            ui.label(crate::i18n::tr("Отключение"));
            ui.checkbox(&mut d.disabled, "")
                .on_hover_text(
                    crate::i18n::tr("Блоки отключённой платы не входят в сборку, а ошибки в них не проверяются.                      Блоки других плат, которые брали от них значения, останутся без источника."),
                );
            ui.end_row();
            ui.label(crate::i18n::tr("Фоновая задача"));
            ui.checkbox(&mut d.background, "")
                .on_hover_text(
                    crate::i18n::tr("Блоки этой платы выполняются в отдельной задаче FreeRTOS на ядре 0 (на ESP32 с двумя ядрами), \
                     не замедляя основной цикл: медленные датчики, запись во флеш. Сетевые блоки (Wi-Fi, MQTT, веб, Modbus) \
                     и так уходят на ядро 0 сами, на какой бы плате ни стояли. \
                     Связи с блоками других плат передают значения снимками в начале и конце прохода."),
                );
            ui.end_row();
            if d.background {
                ui.label(crate::i18n::tr("Период, мс"));
                ui.add(egui::DragValue::new(&mut d.period).range(1..=1000).suffix(crate::i18n::tr(" мс")))
                    .on_hover_text(
                        crate::i18n::tr("Только для одноядерных МК (ESP32-C3, S2…): второго ядра нет, поэтому плата \
                         остаётся в loop() и выполняется раз в этот период, не больше одной платы за проход; \
                         фазы фоновых плат разнесены по периоду."),
                    );
                ui.end_row();
            }
        });
}

fn apply(app: &mut EditorApp, dialog: &Dialog) -> Result<(), String> {
    let lib = app.lib.clone();
    let doc = app
        .doc
        .as_mut()
        .ok_or(crate::i18n::tr("проект не открыт"))?;
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
            let new_id = d.new_id.trim().to_owned();
            let block = doc
                .project
                .logic
                .block(&d.id)
                .ok_or(crate::i18n::tr("блок удалён"))?;
            let def = lib
                .resolve(&doc.project.logic, block)
                .ok_or(crate::i18n::tr("неизвестный тип блока"))?
                .def
                .clone();
            let is_local = |name: &str| def.param(name).is_some_and(|p| p.local.is_some());
            // The pin parameter on an expander is a pin of the chip: set with the source.
            let chip_pin = (def.info.expander && d.source != PinSource::Gpio)
                .then(|| pin_param(&def).map(|q| q.name.clone()))
                .flatten();
            let before = block.param_in.clone();
            doc.edit(&lib, None, |p| {
                for name in d.param_in.symmetric_difference(&before) {
                    p.set_param_input(&lib, &d.id, name, d.param_in.contains(name))?;
                }
                // Back to a GPIO: drops the chip pin number the parameter still holds.
                if d.source == PinSource::Gpio
                    && p.logic.block(&d.id).is_some_and(|b| b.expander.is_some())
                {
                    p.set_block_expander(&lib, &d.id, None)?;
                }
                for (name, value) in &d.values {
                    if is_local(name) || chip_pin.as_ref() == Some(name) {
                        continue;
                    }
                    let now = p.logic.block(&d.id).and_then(|b| b.params.get(name));
                    if now != value.as_ref() {
                        p.set_param(&lib, &d.id, name, value.clone())?;
                    }
                }
                if let Some(name) = &chip_pin {
                    let Some(Some(ParamValue::Int(pin))) = d.values.get(name) else {
                        return Err(flp_core::EditError::BadExpander(
                            crate::i18n::tr("выберите вывод расширителя").to_owned(),
                        ));
                    };
                    let target = match &d.source {
                        PinSource::Expander(id) => id.clone(),
                        _ => {
                            p.add_expander(d.pending.clone())?;
                            d.pending.id.clone()
                        }
                    };
                    p.set_block_expander(&lib, &d.id, Some((&target, *pin)))?;
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
        Dialog::Expander(d) => {
            let mut e = d.form.clone();
            e.id = e.id.trim().to_owned();
            match &d.original {
                Some(id) => doc.edit(&lib, None, |p| p.update_expander(id, e)),
                None => doc.edit(&lib, None, |p| p.add_expander(e)),
            }
            .map_err(|e| e.to_string())?;
        }
        Dialog::Sheet(d) => {
            let info = SheetInfo {
                number: d.number,
                name: d.name.trim().to_owned(),
                comment: d.comment.trim().to_owned(),
                background: d.background,
                disabled: d.disabled,
                period: if d.period == flp_core::layout::DEFAULT_BACKGROUND_PERIOD_MS {
                    0
                } else {
                    d.period
                },
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
    let bad = || crate::i18n::fmt("«{0}» не подходит к типу {1}", &[&t, &ty.name()]);
    let value = match ty.signal() {
        SignalType::Bool => match t {
            "true" | "1" | "да" => ParamValue::Bool(true),
            "false" | "0" | "нет" => ParamValue::Bool(false),
            _ => return Err(bad()),
        },
        SignalType::Int => ParamValue::Int(t.parse().map_err(|_| bad())?),
        SignalType::Float => ParamValue::Float(t.replace(',', ".").parse().map_err(|_| bad())?),
        // Text keeps its spaces.
        SignalType::String | SignalType::Any | SignalType::Number | SignalType::Same => {
            ParamValue::Text(text.to_owned())
        }
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
    fn new_expander_is_valid_and_gets_a_free_id() {
        let mut logic = Logic::default();
        let first = new_expander(Some(&logic));
        assert_eq!(first.id, "pcf1");
        assert_eq!(first.problem(), None);
        logic.expanders.push(first);
        assert_eq!(new_expander(Some(&logic)).id, "pcf2");
    }

    #[test]
    fn a_board_expander_becomes_the_projects_own_under_a_free_id() {
        use flp_core::library::PinMode;
        use flp_core::pins::{BoardExpander, BoardExpanderPin, PinFunction};
        let mut chip = Expander::new("relays", ExpanderChip::Pcf8574, 0x24);
        chip.sda = 4;
        chip.scl = 5;
        let board = BoardExpander {
            title: "Реле".into(),
            expander: chip,
            pins: vec![BoardExpanderPin {
                pin: 0,
                label: "K1".into(),
                functions: [PinFunction::Dout].into(),
            }],
        };
        let e = board_expander(&board, "pcf1");
        assert_eq!(e.id, "pcf1");
        assert!(board.is(&e));
        assert_eq!(e.problem(), None);
        // A relay takes an output block, not an input one.
        assert!(board.fits(0, PinMode::Out));
        assert!(!board.fits(0, PinMode::In));
        // The same chip under another address is no longer the board's.
        let mut other = e.clone();
        other.address = 0x25;
        assert!(!board.is(&other));
    }

    #[test]
    fn expander_rows_list_the_pins_of_the_board_with_their_names() {
        use flp_core::pins::{BoardExpander, BoardExpanderPin, PinFunction};
        use flp_core::Library;
        let (lib, _) =
            Library::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library")).unwrap();
        let pin_param = |kind: &str| {
            lib.get(kind)
                .unwrap()
                .def
                .params
                .iter()
                .find(|p| p.kind == ParamKind::Pin)
                .unwrap()
                .clone()
        };
        let pin = |pin, label: &str| BoardExpanderPin {
            pin,
            label: label.into(),
            functions: [PinFunction::Dout].into(),
        };
        let board = BoardExpander {
            title: "Реле".into(),
            expander: Expander::new("relays", ExpanderChip::Pcf8574, 0x24),
            pins: vec![pin(0, "K1"), pin(1, "K2")],
        };
        let choice = |n| PinChoice::Board(0, n);
        let taken = [(1, "лампа".to_owned())];

        let rows = expander_rows(
            &pin_param("digital_out"),
            ExpanderChip::Pcf8574,
            &taken,
            Some(&board),
            choice,
        );
        let row = |choice, text: &str, free| PinRow::Pin {
            choice,
            text: text.into(),
            free,
        };
        assert_eq!(
            rows,
            [
                row(choice(0), "P0 · K1", true),
                row(choice(1), "P1 · K2 — занят: «лампа»", false),
            ]
        );

        // The relays only drive levels: an input block cannot stand on them.
        let rows = expander_rows(
            &pin_param("digital_in"),
            ExpanderChip::Pcf8574,
            &[],
            Some(&board),
            choice,
        );
        assert_eq!(
            rows[0],
            row(choice(0), "P0 · K1 — не подходит блоку", false)
        );

        // An expander the board knows nothing about offers every pin of the chip.
        let rows = expander_rows(
            &pin_param("digital_out"),
            ExpanderChip::Pcf8574,
            &[],
            None,
            choice,
        );
        assert_eq!(rows.len(), 8);
    }

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
