//! Toolbar, project tree with tags and variables, block library and messages.

use std::collections::BTreeMap;

use eframe::egui::{self, Id, RichText, ScrollArea, Ui};
use flp_core::{BlockPos, ParamKind};

use crate::app::{EditorApp, LogLevel, RightTab};
use crate::canvas::DragItem;
use crate::debugview::{self, DebugState};
use crate::dialogs::{value_text, Dialog};
use crate::job::JobKind;

pub fn toolbar(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let has_doc = app.doc.is_some();
        let idle = !app.job_running();
        if ui
            .add_enabled(has_doc, egui::Button::new("Сохранить"))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            app.save();
        }
        let (can_undo, can_redo) = app
            .doc
            .as_ref()
            .map_or((false, false), |d| (d.can_undo(), d.can_redo()));
        if ui
            .add_enabled(can_undo, egui::Button::new("↶"))
            .on_hover_text("Отменить (Ctrl+Z)")
            .clicked()
        {
            app.undo();
        }
        if ui
            .add_enabled(can_redo, egui::Button::new("↷"))
            .on_hover_text("Повторить (Ctrl+Y)")
            .clicked()
        {
            app.redo();
        }
        ui.separator();
        if ui
            .add_enabled(has_doc, egui::Button::new("Проверить"))
            .on_hover_text("F7: проверить схему")
            .clicked()
        {
            app.run_check();
        }
        if ui
            .add_enabled(has_doc && idle, egui::Button::new("Собрать"))
            .on_hover_text("F5: сгенерировать скетч и собрать прошивку")
            .clicked()
        {
            app.start_job(JobKind::Build, ui.ctx());
        }
        if ui
            .add_enabled(has_doc && idle, egui::Button::new("Залить"))
            .on_hover_text("Собрать и залить на плату через выбранный порт")
            .clicked()
        {
            let port = app.port_choice();
            app.start_job(JobKind::Upload { port }, ui.ctx());
        }
        ui.checkbox(&mut app.debug_firmware, "С отладкой").on_hover_text(
            "Собирать прошивку с модулем онлайн-отладки (и фоновую сборку тоже);              после заливки редактор подключится к плате",
        );
        ui.separator();
        ui.label("Порт:");
        let selected = if app.port.is_empty() {
            "авто".to_owned()
        } else {
            app.port.clone()
        };
        egui::ComboBox::from_id_salt("port")
            .selected_text(selected)
            .width(110.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.port, String::new(), "авто")
                    .on_hover_text("Порт из project.toml или единственный найденный");
                for p in &app.ports {
                    let text = if p.description.is_empty() || p.description == p.address {
                        p.address.clone()
                    } else {
                        format!("{} — {}", p.address, p.description)
                    };
                    ui.selectable_value(&mut app.port, p.address.clone(), text);
                }
                if app.ports.is_empty() {
                    ui.weak("портов не найдено");
                }
            });
        if app.scanning_ports() {
            ui.spinner();
        } else if ui
            .small_button("Обновить")
            .on_hover_text("Найти порты заново")
            .clicked()
        {
            app.scan_ports(ui.ctx());
        }
        ui.separator();
        if app.debug.is_none() {
            if ui
                .add_enabled(has_doc && idle, egui::Button::new("Отладка"))
                .on_hover_text("Залить прошивку с модулем отладки и подключиться")
                .clicked()
            {
                app.start_debug(ui.ctx());
            }
            if ui
                .add_enabled(has_doc && idle, egui::Button::new("Подключиться"))
                .on_hover_text("Плата уже прошита отладочной прошивкой этой схемы")
                .clicked()
            {
                app.connect_debug(None, ui.ctx());
            }
        } else {
            if ui.button("Стоп отладки").clicked() {
                app.stop_debug();
            }
            if ui
                .button("Снять форсировки")
                .on_hover_text("Вернуть все форсированные значения схеме")
                .clicked()
            {
                if let Some(d) = &app.debug {
                    d.send(flp_debug::Command::ReleaseAll);
                }
            }
            if let Some(d) = &mut app.debug {
                ui.toggle_value(&mut d.values, "Значения")
                    .on_hover_text("Входы и переменные: смотреть, записывать и форсировать");
                ui.toggle_value(&mut d.diagnostics, "Диагностика")
                    .on_hover_text("Время цикла программы и листов, максимум с начала работы");
            }
        }
    });
}

pub fn log(app: &mut EditorApp, ui: &mut Ui) {
    let pal = app.theme.palette;
    ui.horizontal(|ui| {
        ui.strong("Сообщения");
        if ui.small_button("Очистить").clicked() {
            app.log.clear();
        }
    });
    let mut focus = None;
    ScrollArea::vertical()
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in &app.log {
                let color = match line.level {
                    LogLevel::Info => pal.text,
                    LogLevel::Ok => pal.ok,
                    LogLevel::Warning => pal.warning,
                    LogLevel::Error => pal.error,
                };
                let text = RichText::new(&line.text).color(color).monospace();
                match &line.block {
                    Some(block) => {
                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                        if resp.on_hover_text("Показать блок").clicked() {
                            focus = Some(block.clone());
                        }
                    }
                    None => {
                        ui.label(text);
                    }
                }
            }
        });
    if let Some(id) = focus {
        app.focus_block(&id);
    }
}

/// Shows side panel contents without letting them widen the panel: long text is
/// truncated and anything wider than the panel is clipped, so the user can drag
/// the panel narrower than its contents.
pub fn fit_width<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let rect = ui.available_rect_before_wrap();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.shrink_clip_rect(rect);
    child.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    let result = add_contents(&mut child);
    // The panel keeps exactly the width the user gave it.
    ui.expand_to_include_rect(rect);
    result
}

/// Runs panel contents invisibly with unwrapped text and returns their natural width
/// (at most `max_width`).
fn measure_width(ui: &mut Ui, max_width: f32, add_contents: impl FnOnce(&mut Ui)) -> f32 {
    let rect = ui.available_rect_before_wrap();
    let wide = egui::Rect::from_min_size(rect.min, egui::vec2(max_width, rect.height()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(wide)
            .sizing_pass()
            .invisible(),
    );
    child.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    add_contents(&mut child);
    ui.expand_to_include_rect(rect);
    child.min_rect().width().min(max_width)
}

/// Expanding a side panel happens in two frames: measure the contents, then apply the width.
#[derive(Clone, Copy)]
enum Fit {
    Measure,
    Apply(f32),
}

/// Resizable side panel with a button that collapses it to a narrow strip and back.
/// On expanding, the panel takes the width of its contents. `width` is the starting
/// width and receives the current one.
pub fn side_panel(
    ui: &mut Ui,
    id: &str,
    left: bool,
    open: &mut bool,
    width: &mut f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let panel = |id: String| {
        if left {
            egui::Panel::left(id)
        } else {
            egui::Panel::right(id)
        }
    };
    // Arrow pointing where the panel goes on click.
    let (hide, show) = if left { ("«", "»") } else { ("»", "«") };
    let fit_id = Id::unique(id).with("fit");
    let fit = ui.data_mut(|d| {
        let fit = d.get_temp::<Fit>(fit_id);
        d.remove::<Fit>(fit_id);
        fit
    });
    if *open {
        let max_width = ui.ctx().content_rect().width() * 0.6;
        let mut builder = panel(id.to_owned())
            .resizable(true)
            // A saved width from a larger window must not swallow the canvas.
            .default_size(width.min(max_width))
            .size_range(80.0..=f32::INFINITY);
        if let Some(Fit::Apply(width)) = fit {
            builder = builder.exact_size(width);
        }
        let mut measured = None;
        let response = builder.show(ui, |ui| {
            let corner = ui.max_rect();
            let inner = corner.width();
            if matches!(fit, Some(Fit::Measure)) {
                measured = Some((measure_width(ui, max_width, add_contents), inner));
            } else {
                fit_width(ui, add_contents);
            }
            // A child ui, so the button never widens the panel.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(corner)
                    .layout(egui::Layout::right_to_left(egui::Align::Min)),
            );
            if child
                .add(egui::Button::new(hide).small())
                .on_hover_text("Свернуть панель")
                .clicked()
            {
                *open = false;
            }
        });
        *width = response.response.rect.width();
        if let Some((content, inner)) = measured {
            // Outer width = contents + frame margins + room for the collapse button.
            let margins = response.response.rect.width() - inner;
            let fitted = (content + margins + 24.0).max(80.0);
            ui.data_mut(|d| d.insert_temp(fit_id, Fit::Apply(fitted)));
            ui.ctx().request_repaint();
        }
    } else {
        panel(format!("{id}_collapsed"))
            .resizable(false)
            .exact_size(28.0)
            .show(ui, |ui| {
                if ui.button(show).on_hover_text("Развернуть панель").clicked() {
                    *open = true;
                    ui.data_mut(|d| d.insert_temp(fit_id, Fit::Measure));
                    ui.ctx().request_repaint();
                }
            });
    }
}

/// Project tree as in FLProg: controller → tags (inputs, outputs, variables) → sheets.
pub fn left(app: &mut EditorApp, ui: &mut Ui) {
    let pal = app.theme.palette;
    ui.strong("Дерево проекта");
    ui.separator();
    // Online: tags show live values and offer writing and forcing.
    let mut debug = app.debug.take();
    let Some(doc) = &app.doc else {
        app.debug = debug;
        ui.weak("Проект не открыт");
        return;
    };
    let logic = &doc.project.logic;
    let mut focus = None;
    let mut sheet_click = None;
    let mut new_var = false;
    let mut edit_var = None;
    let mut place_var = None;

    // Tags: blocks with a pin, split by direction.
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for b in &logic.blocks {
        let Some(lib) = app.lib.resolve(logic, b) else {
            continue;
        };
        let Some(pin) = lib.def.params.iter().find(|p| p.kind == ParamKind::Pin) else {
            continue;
        };
        let pin = b.params.get(&pin.name).map_or("?".to_owned(), value_text);
        let text = format!("{} <{}> = GPIO{pin}", b.id, lib.def.info.name);
        if lib.def.outputs.is_empty() {
            outputs.push((b.id.clone(), text));
        } else {
            inputs.push((b.id.clone(), text));
        }
    }

    ScrollArea::vertical().auto_shrink([ui.is_sizing_pass(), false]).show(ui, |ui| {
        let title = format!(
            "Контроллер: {} ({})",
            doc.project.config.project.name, doc.project.config.board.fqbn
        );
        egui::CollapsingHeader::new(title)
            .default_open(true)
            .show(ui, |ui| {
                egui::CollapsingHeader::new("Теги")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (title, items) in [("Входы", &inputs), ("Выходы", &outputs)] {
                            egui::CollapsingHeader::new(title)
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (id, text) in items {
                                        let sel = app.selection.as_ref().is_some_and(|s| s.has_block(id));
                                        let index = debug.as_ref().and_then(|d| {
                                            d.inputs.iter().find(|(p, _)| p.block == *id).map(|&(_, i)| i)
                                        });
                                        let resp = ui.selectable_label(sel, live_text(text, debug.as_ref(), index));
                                        if resp.clicked() {
                                            focus = Some(id.clone());
                                        }
                                        if let (Some(d), Some(index)) = (&mut debug, index) {
                                            resp.context_menu(|ui| debugview::value_menu(ui, d, index));
                                        }
                                    }
                                    if items.is_empty() {
                                        ui.weak("нет");
                                    }
                                });
                        }
                        egui::CollapsingHeader::new("Переменные")
                            .default_open(true)
                            .show(ui, |ui| {
                                if ui.selectable_label(false, "+ Добавить переменную").clicked() {
                                    new_var = true;
                                }
                                for var in &logic.vars {
                                    let used = logic
                                        .blocks
                                        .iter()
                                        .any(|b| b.var_name() == Some(var.name.as_str()));
                                    let index = debug.as_ref().and_then(|d| d.table.var_index(&var.name));
                                    let plain = live_text(
                                        &format!("{} <{}>", var.name, var.ty.name()),
                                        debug.as_ref(),
                                        index,
                                    );
                                    let text = if used {
                                        RichText::new(plain)
                                    } else {
                                        RichText::new(format!("{plain}  (не используется)"))
                                            .color(pal.warning)
                                    };
                                    let resp = ui
                                        .dnd_drag_source(
                                            Id::unique(("var", &var.name)),
                                            DragItem::Var(var.name.clone()),
                                            |ui| ui.label(text),
                                        )
                                        .response;
                                    let resp = resp.on_hover_text(format!(
                                        "{}\nПеретащите на плату: чтение или запись определяется тем, с какой стороны подключена связь. Двойной щелчок — изменить.",
                                        var.description
                                    ));
                                    if resp.double_clicked() {
                                        edit_var = Some(var.name.clone());
                                    }
                                    resp.context_menu(|ui| {
                                        if let (Some(d), Some(index)) = (&mut debug, index) {
                                            ui.menu_button("Отладка", |ui| debugview::value_menu(ui, d, index));
                                            ui.separator();
                                        }
                                        if ui.button("Поставить чтение на плату").clicked() {
                                            place_var = Some((var.name.clone(), false));
                                        }
                                        if ui.button("Поставить запись на плату").clicked() {
                                            place_var = Some((var.name.clone(), true));
                                        }
                                        if ui.button("Изменить...").clicked() {
                                            edit_var = Some(var.name.clone());
                                        }
                                    });
                                }
                            });
                    });
                egui::CollapsingHeader::new("Платы")
                    .default_open(true)
                    .show(ui, |ui| {
                        let mut by_sheet: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
                        for b in &logic.blocks {
                            let s = doc.project.layout.blocks.get(&b.id).map_or(1, |p| p.sheet);
                            by_sheet.entry(s).or_default().push(&b.id);
                        }
                        for s in app.sheets() {
                            let name = doc.project.layout.sheet(s).map_or("", |i| i.name.as_str());
                            let title = if name.is_empty() {
                                format!("Плата {s}")
                            } else {
                                format!("Плата {s}: {name}")
                            };
                            let blocks = by_sheet.remove(&s).unwrap_or_default();
                            let header = egui::CollapsingHeader::new(title)
                                .id_salt(("sheet", s))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for id in &blocks {
                                        let sel = app.selection.as_ref().is_some_and(|s| s.has_block(id));
                                        if ui.selectable_label(sel, *id).clicked() {
                                            focus = Some(id.to_string());
                                        }
                                    }
                                });
                            if header.header_response.clicked() {
                                sheet_click = Some(s);
                            }
                        }
                    });
            });
    });

    app.debug = debug;
    if let Some(s) = sheet_click {
        app.sheet = s;
    }
    if let Some(id) = focus {
        app.focus_block(&id);
    }
    if new_var {
        app.dialog = Dialog::for_var(app, None);
    }
    if let Some(name) = edit_var {
        app.dialog = Dialog::for_var(app, Some(&name));
    }
    if let Some((name, write)) = place_var {
        let lib = app.lib.clone();
        let pos = free_spot(app);
        if let Some(doc) = &mut app.doc {
            match doc.edit(&lib, None, |p| p.add_var_block(&name, write, pos)) {
                Ok(id) => app.focus_block(&id),
                Err(e) => app.info(LogLevel::Error, e.to_string()),
            }
        }
    }
}

pub fn right(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.right_tab, RightTab::Library, "Библиотека блоков");
        ui.selectable_value(&mut app.right_tab, RightTab::Ai, "ИИ-помощник");
    });
    ui.separator();
    match app.right_tab {
        RightTab::Library => library(app, ui),
        RightTab::Ai => {
            let pal = app.theme.palette;
            let has_project = app.doc.is_some();
            if crate::ai::show(&mut app.ai, &pal, ui, has_project) {
                app.send_ai(ui.ctx());
            }
        }
    }
}

fn library(app: &mut EditorApp, ui: &mut Ui) {
    let filter_id = Id::unique("library_filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Поиск блока"));
    ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
    let needle = filter.to_lowercase();

    let mut groups: BTreeMap<&str, Vec<(&str, String, &str)>> = BTreeMap::new();
    for b in app.lib.iter() {
        let info = &b.def.info;
        let hay = format!("{} {} {}", info.kind, info.name, info.description).to_lowercase();
        if needle.is_empty() || hay.contains(&needle) {
            let symbol = info.symbol.as_deref().unwrap_or("");
            let text = if symbol.is_empty() || symbol == info.name {
                info.name.clone()
            } else {
                format!("{}  ({symbol})", info.name)
            };
            groups
                .entry(&info.group)
                .or_default()
                .push((&info.kind, text, &info.description));
        }
    }
    let mut add = None;
    ScrollArea::vertical()
        .auto_shrink([ui.is_sizing_pass(), false])
        .show(ui, |ui| {
            for (group, blocks) in &groups {
                egui::CollapsingHeader::new(*group)
                    .default_open(!needle.is_empty())
                    .show(ui, |ui| {
                        for (kind, text, description) in blocks {
                            let resp = ui
                                .dnd_drag_source(
                                    Id::unique(("lib", *kind)),
                                    DragItem::Block(kind.to_string()),
                                    |ui| ui.label(text),
                                )
                                .response;
                            if resp.on_hover_text(*description).double_clicked() {
                                add = Some(kind.to_string());
                            }
                        }
                    });
            }
            if groups.is_empty() {
                ui.weak("Ничего не найдено");
            }
        });
    if let Some(kind) = add {
        let lib = app.lib.clone();
        let pos = free_spot(app);
        if let Some(doc) = &mut app.doc {
            match doc.edit(&lib, None, |p| p.add_block(&lib, &kind, pos)) {
                Ok(id) => app.focus_block(&id),
                Err(e) => app.info(LogLevel::Error, e.to_string()),
            }
        }
    }
}

/// Position below the lowest block of the active sheet.
fn free_spot(app: &EditorApp) -> BlockPos {
    let sheet = app.sheet;
    let bottom = app.doc.as_ref().map_or(0.0, |d| {
        d.project
            .layout
            .blocks
            .values()
            .filter(|p| p.sheet == sheet)
            .map(|p| p.y + 80.0)
            .fold(0.0_f32, f32::max)
    });
    BlockPos {
        sheet,
        x: 40.0,
        y: bottom + 20.0,
    }
}

pub fn theme_menu(app: &mut EditorApp, ui: &mut Ui) {
    let mut chosen = None;
    for t in &app.themes {
        if ui.radio(t.name == app.theme.name, &t.name).clicked() {
            chosen = Some(t.clone());
        }
    }
    if let Some(t) = chosen {
        app.set_theme(ui.ctx(), t);
        app.theme_editor = None;
    }
    ui.separator();
    if ui.button("Настройка темы…").clicked() {
        app.theme_editor = Some(crate::themeedit::ThemeEditor::new(&app.theme));
        ui.close();
    }
}

/// Tag text with its live value appended while debugging (`F` when forced).
fn live_text(text: &str, dbg: Option<&DebugState>, index: Option<usize>) -> String {
    let Some((d, i)) = dbg.zip(index).filter(|(d, _)| d.online()) else {
        return text.to_owned();
    };
    match (d.live.values[i], d.live.forced[i].is_some()) {
        (Some(v), true) => format!("{text} = {v} F"),
        (Some(v), false) => format!("{text} = {v}"),
        (None, _) => text.to_owned(),
    }
}
