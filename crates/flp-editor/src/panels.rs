//! Toolbar, project tree with tags and variables, block library and messages.

use std::collections::BTreeMap;

use eframe::egui::{self, Id, RichText, ScrollArea, Ui};
use flp_core::expander::expander_uses;
use flp_core::{BlockPos, ParamKind, Project};

use crate::app::{BottomTab, EditorApp, LogLevel, RightTab};
use crate::canvas::DragItem;
use crate::debugview::{self, DebugState};
use crate::dialogs::{value_text, Dialog};
use crate::job::JobKind;

/// Busy indicator that repaints 5 times a second; `ui.spinner()` forces a repaint
/// on every frame, which keeps the GPU loaded for as long as a job runs.
pub fn busy(ui: &mut Ui) {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let step = (ui.input(|i| i.time) * 5.0) as usize % FRAMES.len();
    ui.monospace(FRAMES[step]);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
}

/// Paints an icon glyph in the middle of a button.
/// The glyph is drawn by hand: the emoji font sits a little high inside a regular button.
fn paint_glyph(ui: &Ui, resp: &egui::Response, glyph: &str, selected: bool) {
    let color = if selected && resp.enabled() {
        ui.visuals().selection.stroke.color
    } else {
        ui.style().interact(resp).text_color()
    };
    let galley =
        ui.painter()
            .layout_no_wrap(glyph.to_owned(), egui::FontId::proportional(15.0), color);
    let pos = resp.rect.center() - galley.size() / 2.0 + egui::vec2(0.0, 1.5);
    ui.painter().galley(pos, galley, color);
}

/// Icon button with a tooltip; the icon alone is the label.
fn icon(ui: &mut Ui, enabled: bool, glyph: &str, tip: &str) -> bool {
    let resp = ui
        .add_enabled(
            enabled,
            egui::Button::new("")
                .min_size(egui::vec2(30.0, 26.0))
                .frame_when_inactive(false),
        )
        .on_hover_text(tip);
    paint_glyph(ui, &resp, glyph, false);
    resp.clicked()
}

/// Icon toggle button with a tooltip.
fn icon_toggle(ui: &mut Ui, value: &mut bool, glyph: &str, tip: &str) {
    let resp = ui
        .add(
            egui::Button::selectable(*value, "")
                .min_size(egui::vec2(30.0, 26.0))
                // A pressed toggle keeps its highlight.
                .frame_when_inactive(*value),
        )
        .on_hover_text(tip);
    paint_glyph(ui, &resp, glyph, *value);
    if resp.clicked() {
        *value = !*value;
    }
}

/// Shape of a window button; drawn with lines, so it does not depend on font glyphs.
#[derive(Clone, Copy)]
pub enum WinIcon {
    Minimize,
    Maximize,
    Restore,
    ExitFullscreen,
    Close,
}

/// Window button of the custom title bar; returns whether it was clicked.
pub fn window_button(ui: &mut Ui, icon: WinIcon, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(44.0, 30.0), egui::Sense::click());
    let resp = resp.on_hover_text(tip);
    let close = matches!(icon, WinIcon::Close);
    if resp.hovered() {
        let bg = if close {
            egui::Color32::from_rgb(196, 43, 28)
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        };
        ui.painter().rect_filled(rect, 3.0, bg);
    }
    let color = if close && resp.hovered() {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    let stroke = egui::Stroke::new(1.5, color);
    let c = rect.center();
    let square = |center: egui::Pos2, side: f32| {
        egui::Rect::from_center_size(center, egui::vec2(side, side))
    };
    let p = ui.painter();
    match icon {
        WinIcon::Minimize => {
            p.line_segment(
                [
                    egui::pos2(c.x - 6.0, c.y + 3.0),
                    egui::pos2(c.x + 6.0, c.y + 3.0),
                ],
                stroke,
            );
        }
        WinIcon::Maximize => {
            p.rect_stroke(square(c, 12.0), 1.0, stroke, egui::StrokeKind::Inside);
        }
        WinIcon::Restore => {
            p.rect_stroke(
                square(c + egui::vec2(-1.5, 1.5), 9.0),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                square(c + egui::vec2(1.5, -1.5), 9.0),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        WinIcon::ExitFullscreen => {
            p.rect_stroke(square(c, 14.0), 1.0, stroke, egui::StrokeKind::Inside);
            p.rect_stroke(square(c, 6.0), 1.0, stroke, egui::StrokeKind::Inside);
        }
        WinIcon::Close => {
            p.line_segment(
                [c + egui::vec2(-5.5, -5.5), c + egui::vec2(5.5, 5.5)],
                stroke,
            );
            p.line_segment(
                [c + egui::vec2(-5.5, 5.5), c + egui::vec2(5.5, -5.5)],
                stroke,
            );
        }
    }
    resp.clicked()
}

/// Toolbar row; lives in the title bar next to the burger button.
pub fn toolbar(app: &mut EditorApp, ui: &mut Ui) {
    let has_doc = app.doc.is_some();
    let idle = !app.job_running();
    if icon(ui, has_doc, "💾", crate::i18n::tr("Сохранить (Ctrl+S)")) {
        app.save();
    }
    let (can_undo, can_redo) = app
        .doc
        .as_ref()
        .map_or((false, false), |d| (d.can_undo(), d.can_redo()));
    if icon(ui, can_undo, "↶", crate::i18n::tr("Отменить (Ctrl+Z)")) {
        app.undo();
    }
    if icon(ui, can_redo, "↷", crate::i18n::tr("Повторить (Ctrl+Y)")) {
        app.redo();
    }
    ui.separator();
    let board = app.doc.as_ref().map_or(crate::i18n::tr("МК"), |d| {
        crate::boardcfg::board_name(&d.project.config.board, app.lib.boards())
    });
    if ui
        .add_enabled(
            has_doc,
            egui::Button::new(RichText::new(board).size(13.0))
                .min_size(egui::vec2(30.0, 26.0))
                .frame_when_inactive(false),
        )
        .on_hover_text(crate::i18n::tr(
            "Настройки МК: плата, режим подключения, порты",
        ))
        .clicked()
    {
        app.open_board_settings(ui.ctx());
    }
    ui.separator();
    let selected = if app.port.is_empty() {
        crate::i18n::tr("авто").to_owned()
    } else {
        app.port.clone()
    };
    // The plug stays large; the port name beside it is smaller.
    let color = ui.visuals().widgets.inactive.fg_stroke.color;
    let mut shown = egui::text::LayoutJob::default();
    let font = |size: f32| egui::TextFormat {
        font_id: egui::FontId::proportional(size),
        color,
        valign: egui::Align::Center,
        ..Default::default()
    };
    shown.append("🔌 ", 0.0, font(17.0));
    shown.append(&selected, 0.0, font(13.0));
    // Outline instead of a filled background.
    let saved = ui.visuals().widgets.clone();
    let outline = egui::Stroke::new(
        1.0,
        saved.noninteractive.fg_stroke.color.gamma_multiply(0.6),
    );
    let widgets = &mut ui.visuals_mut().widgets;
    for state in [&mut widgets.inactive, &mut widgets.open] {
        state.weak_bg_fill = egui::Color32::TRANSPARENT;
        state.bg_fill = egui::Color32::TRANSPARENT;
        state.bg_stroke = outline;
    }
    egui::ComboBox::from_id_salt("port")
        .selected_text(shown)
        .width(120.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut app.port, String::new(), crate::i18n::tr("авто"))
                .on_hover_text(crate::i18n::tr(
                    "Порт из настроек проекта или единственный найденный",
                ));
            for p in &app.ports {
                let text = if p.description.is_empty() || p.description == p.address {
                    p.address.clone()
                } else {
                    format!("{} — {}", p.address, p.description)
                };
                ui.selectable_value(&mut app.port, p.address.clone(), text);
            }
            if app.ports.is_empty() {
                ui.weak(crate::i18n::tr("портов не найдено"));
            }
        })
        .response
        .on_hover_text(crate::i18n::tr("Порт платы"));
    ui.visuals_mut().widgets = saved;
    if app.scanning_ports() {
        busy(ui);
    } else if icon(ui, true, "↻", crate::i18n::tr("Найти порты заново")) {
        app.scan_ports(ui.ctx());
    }
    ui.separator();
    if icon(ui, has_doc, "✔", crate::i18n::tr("Проверить схему (F7)")) {
        app.run_check();
    }
    if icon(
        ui,
        has_doc,
        "</>",
        crate::i18n::tr("Код C++: скетч Arduino, сгенерированный из схемы"),
    ) {
        app.code_view = Some(crate::codeview::CodeView::new());
    }
    if icon(
        ui,
        has_doc && idle,
        "🔨",
        crate::i18n::tr("Собрать (F5): сгенерировать скетч и собрать прошивку"),
    ) {
        app.start_job(JobKind::Build, ui.ctx());
    }
    if icon(
        ui,
        has_doc && idle,
        "⬆",
        crate::i18n::tr("Залить: собрать и залить на плату через выбранный порт"),
    ) {
        let port = app.port_choice();
        app.start_job(JobKind::Upload { port }, ui.ctx());
    }
    if icon(
        ui,
        app.has_job(),
        "⏹",
        crate::i18n::tr("Стоп: отменить сборку или заливку"),
    ) {
        app.cancel_job();
    }
    icon_toggle(
        ui,
        &mut app.debug_firmware,
        "🛠",
        crate::i18n::tr("С отладкой: собирать прошивку с модулем онлайн-отладки (и фоновую сборку тоже); после заливки редактор подключится к плате"),
    );
    ui.separator();
    if app.debug.is_none() {
        if icon(
            ui,
            has_doc && idle,
            "🔗",
            crate::i18n::tr("Подключиться: плата уже прошита отладочной прошивкой этой схемы"),
        ) {
            app.connect_debug(None, ui.ctx());
        }
    } else {
        if icon(ui, true, "⏹", crate::i18n::tr("Стоп отладки")) {
            app.stop_debug();
        }
        if icon(
            ui,
            true,
            "🔓",
            crate::i18n::tr("Снять форсировки: вернуть форсированные значения схеме"),
        ) {
            if let Some(d) = &app.debug {
                d.send(flp_debug::Command::ReleaseAll);
            }
        }
        if let Some(d) = &mut app.debug {
            icon_toggle(
                ui,
                &mut d.values,
                "📊",
                crate::i18n::tr("Значения: входы и переменные, смотреть, записывать и форсировать"),
            );
            icon_toggle(
                ui,
                &mut d.diagnostics,
                "⏱",
                crate::i18n::tr(
                    "Диагностика: время цикла программы и листов, максимум с начала работы",
                ),
            );
        }
    }
}

/// Bottom panel: editor messages and the port monitor / debug console.
pub fn bottom(app: &mut EditorApp, ui: &mut Ui, open: &mut bool) {
    ui.horizontal(|ui| {
        let (arrow, hint) = if *open {
            ("▾", crate::i18n::tr("Свернуть панель"))
        } else {
            ("▴", crate::i18n::tr("Развернуть панель"))
        };
        if ui.small_button(arrow).on_hover_text(hint).clicked() {
            *open = !*open;
        }
        let tab = app.bottom_tab;
        ui.selectable_value(
            &mut app.bottom_tab,
            BottomTab::Log,
            crate::i18n::tr("Сообщения"),
        );
        let title = match app.console.unread {
            0 => app.console.title().to_owned(),
            n => format!("{} ({n})", app.console.title()),
        };
        ui.selectable_value(&mut app.bottom_tab, BottomTab::Console, title);
        if app.bottom_tab != tab {
            // Picking a tab of the folded panel opens it.
            *open = true;
        }
        if app.bottom_tab == BottomTab::Log {
            ui.separator();
            if *open {
                if ui.small_button(crate::i18n::tr("Очистить")).clicked() {
                    app.log.clear();
                }
            } else if let Some(line) = app.log.last() {
                // The folded panel keeps the latest message in view.
                let pal = app.theme.palette;
                let color = match line.level {
                    LogLevel::Info => pal.text,
                    LogLevel::Ok => pal.ok,
                    LogLevel::Warning => pal.warning,
                    LogLevel::Error => pal.error,
                };
                let full = line.text.to_string();
                let text = full.lines().next().unwrap_or("");
                ui.add(egui::Label::new(RichText::new(text).color(color).monospace()).truncate());
            }
        }
    });
    if !*open {
        return;
    }
    ui.separator();
    match app.bottom_tab {
        BottomTab::Log => log(app, ui),
        BottomTab::Console => {
            let pal = app.theme.palette;
            crate::console::show(&mut app.console, &pal, ui);
        }
    }
}

fn log(app: &mut EditorApp, ui: &mut Ui) {
    let pal = app.theme.palette;
    let mut focus = None;
    let mut help = None;
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
                let text = RichText::new(line.text.to_string())
                    .color(color)
                    .monospace();
                ui.horizontal_wrapped(|ui| {
                    match &line.block {
                        Some(block) => {
                            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if resp
                                .on_hover_text(crate::i18n::tr("Показать блок"))
                                .clicked()
                            {
                                focus = Some(block.clone());
                            }
                        }
                        None => {
                            ui.label(text);
                        }
                    }
                    if let Some(target) = &line.help {
                        if ui.link(crate::i18n::tr("Что это?")).clicked() {
                            help = Some(target.clone());
                        }
                    }
                });
            }
        });
    if let Some(id) = focus {
        app.focus_block(&id);
    }
    if let Some(target) = help {
        app.open_help(&target);
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

/// Resizable side panel with a button that collapses it to a narrow strip and back.
/// On expanding, the panel returns to the width it had before folding. `width` is the
/// starting width and receives the current one.
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
        let fit = d.get_temp::<f32>(fit_id);
        d.remove::<f32>(fit_id);
        fit
    });
    if *open {
        let max_width = ui.ctx().content_rect().width() * 0.6;
        let mut builder = panel(id.to_owned())
            .resizable(true)
            // A saved width from a larger window must not swallow the canvas.
            .default_size(width.min(max_width))
            .size_range(80.0..=f32::INFINITY);
        if let Some(width) = fit {
            builder = builder.exact_size(width);
        }
        let response = builder.show(ui, |ui| {
            let corner = ui.max_rect();
            fit_width(ui, add_contents);
            // A child ui, so the button never widens the panel.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(corner)
                    .layout(egui::Layout::right_to_left(egui::Align::Min)),
            );
            if child
                .add(egui::Button::new(hide).small())
                .on_hover_text(crate::i18n::tr("Свернуть панель"))
                .clicked()
            {
                *open = false;
            }
        });
        *width = response.response.rect.width();
    } else {
        panel(format!("{id}_collapsed"))
            .resizable(false)
            .exact_size(28.0)
            .show(ui, |ui| {
                if ui
                    .button(show)
                    .on_hover_text(crate::i18n::tr("Развернуть панель"))
                    .clicked()
                {
                    *open = true;
                    // Back to the width it had before folding.
                    let restored = *width;
                    ui.data_mut(|d| d.insert_temp(fit_id, restored));
                    ui.ctx().request_repaint();
                }
            });
    }
}

/// Nesting of a tree group; it sets how loud the title is.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Level {
    Top,
    Mid,
    Sub,
}

/// Collapsible group of the project tree: a title (click to fold), compact tools
/// next to the title in the same row, and the indented body. Open by default.
pub(crate) fn group(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    level: Level,
    tools: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui),
) {
    use egui::collapsing_header::CollapsingState;
    let id = ui.make_persistent_id(("tree", id));
    if level != Level::Sub {
        ui.add_space(4.0);
    }
    let accent = ui.visuals().hyperlink_color;
    let text = match level {
        Level::Top => RichText::new(title).strong().color(accent),
        Level::Mid => RichText::new(title).strong(),
        Level::Sub => RichText::new(title),
    };
    let mut clicked = false;
    CollapsingState::load_with_default_open(ui.ctx(), id, true)
        .show_header(ui, |ui| {
            clicked = ui
                .add(egui::Label::new(text).sense(egui::Sense::click()))
                .clicked();
            // Tools sit right next to the title, not at the panel edge.
            tools(ui);
        })
        .body(body);
    if clicked {
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, true);
        state.toggle(ui);
        state.store(ui.ctx());
    }
    if level == Level::Mid {
        ui.separator();
    }
}

/// Project tree: controller → tags (inputs, outputs, variables) → sheets.
pub fn left(app: &mut EditorApp, ui: &mut Ui) {
    let pal = app.theme.palette;
    // The tree is read more than edited: slightly larger text and looser rows.
    {
        let style = ui.style_mut();
        for id in [
            egui::TextStyle::Body,
            egui::TextStyle::Button,
            egui::TextStyle::Small,
        ] {
            if let Some(font) = style.text_styles.get_mut(&id) {
                font.size += 1.5;
            }
        }
        style.spacing.item_spacing.y += 3.0;
        style.spacing.button_padding.y += 1.0;
    }
    ui.strong(crate::i18n::tr("Дерево проекта"));
    ui.separator();
    // Online: tags show live values and offer writing and forcing.
    let mut debug = app.debug.take();
    let Some(doc) = &app.doc else {
        app.debug = debug;
        ui.weak(crate::i18n::tr("Проект не открыт"));
        return;
    };
    let logic = &doc.project.logic;
    let mut focus = None;
    // Tree key of the clicked row and its images on the board, to step through.
    let mut cycle: Option<(String, Vec<String>)> = None;
    let mut sheet_click = None;
    let mut new_var = false;
    let mut edit_var = None;
    let mut new_expander = false;
    let mut edit_expander = None;
    let mut place_var = None;
    let mut open_block: Option<String> = None;
    let mut place_block: Option<String> = None;
    let mut remove_tag: Option<String> = None;
    let mut net_actions = Vec::new();

    // Tags: blocks with a pin, split by direction.
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    // Copies of a tag share its row.
    for b in logic.blocks.iter().filter(|b| b.tag.is_none()) {
        let Some(lib) = app.lib.resolve(logic, b) else {
            continue;
        };
        let Some(pin) = lib.def.params.iter().find(|p| p.kind == ParamKind::Pin) else {
            continue;
        };
        let pin = b.params.get(&pin.name).map_or("?".to_owned(), value_text);
        let text = match &b.expander {
            Some(e) => format!("{}  ·  {e}.P{pin}", b.id),
            None => format!("{}  ·  GPIO{pin}", b.id),
        };
        let name = lib.def.info.name.clone();
        if lib.def.outputs.is_empty() {
            outputs.push((b.id.clone(), text, name));
        } else {
            inputs.push((b.id.clone(), text, name));
        }
    }
    // Library blocks with a pin, offered by the "add" menus of the two groups.
    let mut input_kinds = Vec::new();
    let mut output_kinds = Vec::new();
    let model = doc.project.config.board.model.as_deref();
    for b in app.lib.iter() {
        if !b
            .def
            .params
            .iter()
            .any(|p| p.kind == ParamKind::Pin && !p.fixed)
            || !b.def.info.fits_board(model)
        {
            continue;
        }
        let item = (
            b.def.info.kind.clone(),
            b.def.info.name.clone(),
            b.def.info.description.clone(),
        );
        if b.def.outputs.is_empty() {
            output_kinds.push(item);
        } else {
            input_kinds.push(item);
        }
    }
    input_kinds.sort_by(|a, b| a.1.cmp(&b.1));
    output_kinds.sort_by(|a, b| a.1.cmp(&b.1));
    let mut add_kind = None;

    ScrollArea::vertical().auto_shrink([ui.is_sizing_pass(), false]).show(ui, |ui| {
        let title = crate::i18n::fmt("Контроллер: {0} ({1})", &[&doc.project.config.project.name, &crate::boardcfg::board_name(&doc.project.config.board, app.lib.boards())]);
        group(ui, "controller", &title, Level::Top, |_| {}, |ui| {
            group(ui, "tags", crate::i18n::tr("Теги"), Level::Mid, |_| {}, |ui| {
                for (key, title, items, kinds, hint) in [
                    ("inputs", crate::i18n::tr("Входы"), &inputs, &input_kinds, crate::i18n::tr("Добавить вход")),
                    ("outputs", crate::i18n::tr("Выходы"), &outputs, &output_kinds, crate::i18n::tr("Добавить выход")),
                ] {
                    let tools = |ui: &mut Ui| {
                        ui.menu_button("+", |ui| {
                            for (kind, name, description) in kinds {
                                if ui.button(name).on_hover_text(description).clicked() {
                                    add_kind = Some(kind.clone());
                                    ui.close();
                                }
                            }
                            if kinds.is_empty() {
                                ui.weak(crate::i18n::tr("В библиотеке нет таких блоков"));
                            }
                        })
                        .response
                        .on_hover_text(hint);
                    };
                    group(ui, key, title, Level::Sub, tools, |ui| {
                        for (id, text, name) in items {
                            // Images on the board: the tag itself and its placed copies.
                            let images = placed_images(
                                &doc.project,
                                std::iter::once(id.as_str()).chain(logic.tag_copies(id)),
                            );
                            let placed = !images.is_empty();
                            let sel = app
                                .selection
                                .as_ref()
                                .is_some_and(|s| s.has_block(id) || images.iter().any(|b| s.has_block(b)));
                            let index = debug.as_ref().and_then(|d| {
                                d.inputs.iter().find(|(p, _)| p.block == *id).map(|&(_, i)| i)
                            });
                            let shown = live_text(text, debug.as_ref(), index);
                            // Like an unused variable: in the tree, but not on any sheet.
                            let shown = if placed {
                                RichText::new(shown)
                            } else {
                                RichText::new(crate::i18n::fmt("{0}  (не используется)", &[&shown]))
                                    .color(pal.warning)
                            };
                            let resp = drag_row(
                                ui,
                                Id::unique(("tag", id)),
                                DragItem::Topic(id.clone()),
                                sel,
                                shown,
                            )
                            .on_hover_text(if placed {
                                crate::i18n::fmt("{0}\nЩелчок — найти на плате, повторный — следующую копию. Двойной щелчок — свойства; перетащите на плату, чтобы поставить ещё одну копию.", &[&name])
                            } else {
                                crate::i18n::fmt("{0}\nПеретащите на плату. Двойной щелчок — свойства.", &[&name])
                            });
                            if resp.double_clicked() {
                                open_block = Some(id.clone());
                            } else if resp.clicked() && placed {
                                cycle = Some((format!("tag:{id}"), images));
                            }
                            resp.context_menu(|ui| {
                                if let (Some(d), Some(index)) = (&mut debug, index) {
                                    ui.menu_button(crate::i18n::tr("Отладка"), |ui| debugview::value_menu(ui, d, index));
                                    ui.separator();
                                }
                                if !placed && ui.button(crate::i18n::tr("Поставить на плату")).clicked() {
                                    place_block = Some(id.clone());
                                }
                                if ui.button(crate::i18n::tr("Свойства...")).clicked() {
                                    open_block = Some(id.clone());
                                }
                                ui.separator();
                                if ui.button(crate::i18n::tr("Удалить из проекта")).clicked() {
                                    remove_tag = Some(id.clone());
                                }
                            });
                        }
                        if items.is_empty() {
                            ui.weak(crate::i18n::tr("нет"));
                        }
                    });
                }
                let tools = |ui: &mut Ui| {
                    if ui.small_button("+").on_hover_text(crate::i18n::tr("Добавить переменную")).clicked() {
                        new_var = true;
                    }
                };
                group(ui, "vars", crate::i18n::tr("Переменные"), Level::Sub, tools, |ui| {
                    for var in logic.vars.iter().filter(|v| !v.service) {
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
                            RichText::new(crate::i18n::fmt("{0}  (не используется)", &[&plain])).color(pal.warning)
                        };
                        let images = placed_images(
                            &doc.project,
                            logic
                                .blocks
                                .iter()
                                .filter(|b| b.var_name() == Some(var.name.as_str()))
                                .map(|b| b.id.as_str()),
                        );
                        let sel = app
                            .selection
                            .as_ref()
                            .is_some_and(|s| images.iter().any(|b| s.has_block(b)));
                        let resp = drag_row(
                            ui,
                            Id::unique(("var", &var.name)),
                            DragItem::Var(var.name.clone()),
                            sel,
                            text,
                        );
                        let resp = resp.on_hover_text(crate::i18n::fmt("{0}\nЩелчок — найти на плате, повторный — следующий блок. Перетащите на плату: чтение или запись определяется тем, с какой стороны подключена связь. Двойной щелчок — изменить.", &[&var.description]));
                        if resp.double_clicked() {
                            edit_var = Some(var.name.clone());
                        } else if resp.clicked() && !images.is_empty() {
                            cycle = Some((format!("var:{}", var.name), images));
                        }
                        resp.context_menu(|ui| {
                            if let (Some(d), Some(index)) = (&mut debug, index) {
                                ui.menu_button(crate::i18n::tr("Отладка"), |ui| debugview::value_menu(ui, d, index));
                                ui.separator();
                            }
                            if ui.button(crate::i18n::tr("Поставить чтение на плату")).clicked() {
                                place_var = Some((var.name.clone(), false));
                            }
                            if ui.button(crate::i18n::tr("Поставить запись на плату")).clicked() {
                                place_var = Some((var.name.clone(), true));
                            }
                            if ui.button(crate::i18n::tr("Изменить...")).clicked() {
                                edit_var = Some(var.name.clone());
                            }
                        });
                    }
                });
                let tools = |ui: &mut Ui| {
                    if ui.small_button("+").on_hover_text(crate::i18n::tr("Добавить расширитель")).clicked() {
                        new_expander = true;
                    }
                };
                group(ui, "expanders", crate::i18n::tr("Расширители"), Level::Sub, tools, |ui| {
                    for e in &logic.expanders {
                        let taken = expander_uses(logic, &app.lib)
                            .iter()
                            .filter(|u| u.expander == e.id)
                            .count();
                        let text = crate::i18n::fmt("{0} · {1} 0x{2} · занято {3} из {4}", &[&e.id, &e.chip.name(), &format!("{:02X}", e.address), &taken, &e.chip.pins()]);
                        let resp = ui.selectable_label(false, text).on_hover_text(
                            crate::i18n::tr("Микросхема-расширитель по I²C. Двойной щелчок — изменить; выбрать вывод можно в свойствах входа или выхода."),
                        );
                        if resp.double_clicked() {
                            edit_expander = Some(e.id.clone());
                        }
                        resp.context_menu(|ui| {
                            if ui.button(crate::i18n::tr("Изменить...")).clicked() {
                                edit_expander = Some(e.id.clone());
                                ui.close();
                            }
                        });
                    }
                    if logic.expanders.is_empty() {
                        ui.weak(crate::i18n::tr("нет"));
                    }
                });
            });
            net_actions = crate::netpanel::show(app, ui);
            group(ui, "boards", crate::i18n::tr("Платы"), Level::Mid, |_| {}, |ui| {
                let mut by_sheet: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
                for b in &logic.blocks {
                    let s = doc.project.layout.blocks.get(&b.id).map_or(1, |p| p.sheet);
                    by_sheet.entry(s).or_default().push(&b.id);
                }
                for s in app.sheets() {
                    let name = doc.project.layout.sheet(s).map_or("", |i| i.name.as_str());
                    let mut title = if name.is_empty() {
                        crate::i18n::fmt("Плата {0}", &[&s])
                    } else {
                        crate::i18n::fmt("Плата {0}: {1}", &[&s, &name])
                    };
                    if doc.project.layout.is_background(s) {
                        title.push_str(crate::i18n::tr(" (фон)"));
                    }
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
    if let Some((key, images)) = cycle {
        // A repeated click on the same row shows the next image.
        let index = match &app.tree_cycle {
            Some((last, i)) if *last == key => (i + 1) % images.len(),
            _ => 0,
        };
        app.focus_block(&images[index]);
        app.tree_cycle = Some((key, index));
    }
    if let Some(kind) = add_kind {
        add_to_tree(app, &kind);
    }
    if let Some(id) = open_block {
        app.dialog = Dialog::for_block(app, &id);
    }
    if let Some(id) = place_block {
        let lib = app.lib.clone();
        let pos = free_spot(app);
        if let Some(doc) = &mut app.doc {
            let placed = doc.edit(&lib, None, |p| {
                p.layout.blocks.insert(id.clone(), pos);
                Ok::<_, flp_core::EditError>(())
            });
            match placed {
                Ok(()) => app.focus_block(&id),
                Err(e) => app.info(LogLevel::Error, e.to_string()),
            }
        }
    }
    if let Some(id) = remove_tag {
        let lib = app.lib.clone();
        if let Some(doc) = &mut app.doc {
            match doc.edit(&lib, None, |p| p.remove_tag(&id)) {
                Ok(()) => app.selection = None,
                Err(e) => app.info(LogLevel::Error, e.to_string()),
            }
        }
    }
    crate::netpanel::apply(app, net_actions);
    if new_var {
        app.dialog = Dialog::for_var(app, None);
    }
    if let Some(name) = edit_var {
        app.dialog = Dialog::for_var(app, Some(&name));
    }
    if new_expander {
        app.dialog = Dialog::for_expander(app, None);
    }
    if let Some(id) = edit_expander {
        app.dialog = Dialog::for_expander(app, Some(&id));
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
        ui.selectable_value(
            &mut app.right_tab,
            RightTab::Library,
            crate::i18n::tr("Библиотека блоков"),
        );
        ui.selectable_value(
            &mut app.right_tab,
            RightTab::Ai,
            crate::i18n::tr("ИИ-помощник"),
        );
    });
    ui.separator();
    match app.right_tab {
        RightTab::Library => library(app, ui),
        RightTab::Ai => {
            let pal = app.theme.palette;
            let has_project = app.doc.is_some();
            ui.horizontal(|ui| {
                let running = app.mcp.running();
                let label = if running {
                    "Выключить внешний ИИ (MCP)"
                } else {
                    "Включить внешний ИИ (MCP)"
                };
                if ui
                    .selectable_label(running, crate::i18n::tr(label))
                    .on_hover_text(crate::i18n::tr(
                        "Сервер MCP для внешнего ИИ (Claude Code и др.): он работает с проектом, открытым здесь",
                    ))
                    .clicked()
                {
                    app.mcp.toggle(app.lib_dir.clone());
                }
                if running && ui.button(crate::i18n::tr("Промпт...")).clicked() {
                    app.mcp.show_window();
                }
                if ui.button(crate::i18n::tr("Настройки ИИ...")).clicked() {
                    app.ai_conf.show();
                }
            });
            ui.separator();
            if crate::ai::show(&mut app.ai, &pal, ui, has_project) {
                app.send_ai(ui.ctx());
            }
        }
    }
}

/// Library group of the board input/output blocks, not shown in the library panel.
const IO_GROUP: &str = "Входы и выходы";
/// Web widgets are added from the web interface section of the project tree.
const WEB_GROUP: &str = "Веб-интерфейс";

fn library(app: &mut EditorApp, ui: &mut Ui) {
    let filter_id = Id::unique("library_filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    ui.add(egui::TextEdit::singleline(&mut filter).hint_text(crate::i18n::tr("Поиск блока")));
    ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
    let needle = filter.to_lowercase();

    // Built-in blocks come with the program and cannot be changed; user blocks
    // live in user_library/ and are made in the block editor.
    let tab_id = Id::unique("library_user_tab");
    let mut user_tab: bool = ui.data_mut(|d| d.get_temp(tab_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.selectable_value(&mut user_tab, false, crate::i18n::tr("Встроенные"));
        ui.selectable_value(&mut user_tab, true, crate::i18n::tr("Пользовательские"));
    });
    ui.data_mut(|d| d.insert_temp(tab_id, user_tab));
    if user_tab {
        if let Some(kind) = crate::userlib::show(app, ui, &needle) {
            add_to_schema(app, &kind);
        }
        return;
    }

    let mut groups: BTreeMap<&str, Vec<(&str, String, &str)>> = BTreeMap::new();
    let model = app
        .doc
        .as_ref()
        .and_then(|d| d.project.config.board.model.as_deref());
    for b in app.lib.iter() {
        let info = &b.def.info;
        // Board inputs and outputs are added from the tag tree on the left.
        // Blocks of a user board are offered in projects of that board only.
        if app.lib.is_user(&info.kind)
            || info.group == IO_GROUP
            || info.group == WEB_GROUP
            || info.hidden
            || !info.fits_board(model)
        {
            continue;
        }
        let hay = format!(
            "{} {} {} {}",
            info.kind,
            info.name,
            crate::i18n::tr(&info.name),
            info.description
        )
        .to_lowercase();
        if needle.is_empty() || hay.contains(&needle) {
            let symbol = info.symbol.as_deref().unwrap_or("");
            let name = crate::i18n::tr(&info.name);
            let text = if symbol.is_empty() || symbol == info.name {
                name.to_owned()
            } else {
                format!("{name}  ({symbol})")
            };
            groups
                .entry(&info.group)
                .or_default()
                .push((&info.kind, text, &info.description));
        }
    }
    let mut add = None;
    // (type, action): 0 — open, 1 — copy, 2 — delete.
    let mut edit: Option<(String, u8)> = None;
    let lib = &app.lib;
    ScrollArea::vertical()
        .auto_shrink([ui.is_sizing_pass(), false])
        .show(ui, |ui| {
            for (group, blocks) in &groups {
                egui::CollapsingHeader::new(crate::i18n::tr(group))
                    .default_open(!needle.is_empty())
                    .show(ui, |ui| {
                        for (kind, text, description) in blocks {
                            let resp = crate::canvas::drag_source(
                                ui,
                                Id::unique(("lib", *kind)),
                                DragItem::Block(kind.to_string()),
                                |ui| ui.label(text),
                            );
                            let resp = resp.on_hover_text(crate::i18n::tr(description));
                            if resp.double_clicked() {
                                add = Some(kind.to_string());
                            }
                            resp.context_menu(|ui| {
                                let user = lib.is_user(kind);
                                let open = if user {
                                    crate::i18n::tr("Изменить блок...")
                                } else {
                                    crate::i18n::tr("Посмотреть блок...")
                                };
                                if ui.button(open).clicked() {
                                    edit = Some((kind.to_string(), 0));
                                }
                                if ui
                                    .button(crate::i18n::tr("Новый блок на основе этого..."))
                                    .clicked()
                                {
                                    edit = Some((kind.to_string(), 1));
                                }
                                if user {
                                    ui.separator();
                                    if ui.button(crate::i18n::tr("Удалить блок...")).clicked()
                                    {
                                        edit = Some((kind.to_string(), 2));
                                    }
                                }
                            });
                        }
                    });
            }
            if groups.is_empty() {
                ui.weak(crate::i18n::tr("Ничего не найдено"));
            }
        });
    if let Some((kind, action)) = edit {
        if let Some(block) = app.lib.get(&kind).cloned() {
            match action {
                0 => app.block_editor = Some(crate::blockedit::BlockEditor::edit(app, &block)),
                1 => app.block_editor = Some(crate::blockedit::BlockEditor::copy_of(app, &block)),
                _ => app.block_delete = Some(kind),
            }
        }
    }
    if let Some(kind) = add {
        add_to_schema(app, &kind);
    }
}

/// A row of the project tree that can be dragged onto a sheet. egui's drag source senses
/// only drags, so the clicks (and double clicks) come from the label inside it.
/// Blocks of `ids` that stand on a sheet, in reading order: by sheet, then top to
/// bottom and left to right.
fn placed_images<'a>(project: &Project, ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut images: Vec<(&BlockPos, &str)> = ids
        .filter_map(|id| Some((project.layout.blocks.get(id)?, id)))
        .collect();
    images.sort_by(|(a, _), (b, _)| {
        (a.sheet, a.y, a.x)
            .partial_cmp(&(b.sheet, b.y, b.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    images.into_iter().map(|(_, id)| id.to_owned()).collect()
}

fn drag_row(
    ui: &mut Ui,
    id: Id,
    item: DragItem,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    crate::canvas::drag_source(ui, id, item, |ui| ui.selectable_label(selected, text))
}

/// Adds an input or output of type `kind` to the project. Like a variable, it stays in
/// the tree until it is dragged onto a sheet.
fn add_to_tree(app: &mut EditorApp, kind: &str) {
    let lib = app.lib.clone();
    let pos = free_spot(app);
    let Some(doc) = &mut app.doc else { return };
    let added = doc.edit(&lib, None, |p| {
        let id = p.add_block(&lib, kind, pos)?;
        p.layout.blocks.remove(&id);
        Ok::<_, flp_core::EditError>(id)
    });
    match added {
        Ok(id) => app.dialog = Dialog::for_block(app, &id),
        Err(e) => app.info(LogLevel::Error, e.to_string()),
    }
}

/// Adds a block of type `kind` to the active sheet (double click in the library).
fn add_to_schema(app: &mut EditorApp, kind: &str) {
    let lib = app.lib.clone();
    let pos = free_spot(app);
    if let Some(doc) = &mut app.doc {
        match doc.edit(&lib, None, |p| p.add_block(&lib, kind, pos)) {
            Ok(id) => app.focus_block(&id),
            Err(e) => app.info(LogLevel::Error, e.to_string()),
        }
    }
}

/// Position below the lowest block of the active sheet.
pub fn free_spot(app: &EditorApp) -> BlockPos {
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
        if ui
            .radio(t.name == app.theme.name, crate::i18n::tr(&t.name))
            .clicked()
        {
            chosen = Some(t.clone());
        }
    }
    if let Some(t) = chosen {
        app.set_theme(ui.ctx(), t);
        app.theme_editor = None;
    }
    ui.separator();
    if ui.button(crate::i18n::tr("Настройка темы…")).clicked() {
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
