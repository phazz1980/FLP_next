//! "Theme settings" window: choose a theme, copy it, edit colours with live preview,
//! save or delete user themes.

use eframe::egui::{self, RichText, Ui};

use crate::app::{EditorApp, LogLevel};
use crate::theme::{self, Theme};

pub struct ThemeEditor {
    /// The current theme as last saved; restored when the window closes unsaved.
    original: Theme,
}

impl ThemeEditor {
    pub fn new(current: &Theme) -> Self {
        Self {
            original: current.clone(),
        }
    }
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut ed) = app.theme_editor.take() else {
        return;
    };
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Настройка темы"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .default_height(560.0)
        .show(ctx, |ui| body(app, ui, &mut ed));
    if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.theme_editor = Some(ed);
    } else if app.theme != ed.original {
        app.set_theme(ctx, ed.original);
    }
}

fn body(app: &mut EditorApp, ui: &mut Ui, ed: &mut ThemeEditor) {
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Тема"));
        let mut chosen = None;
        egui::ComboBox::from_id_salt("theme_pick")
            .selected_text(&ed.original.name)
            .width(220.0)
            .show_ui(ui, |ui| {
                for t in &app.themes {
                    let label = if t.builtin {
                        crate::i18n::fmt("{0} (встроенная)", &[&t.name])
                    } else {
                        t.name.clone()
                    };
                    if ui
                        .selectable_label(t.name == ed.original.name, label)
                        .clicked()
                    {
                        chosen = Some(t.clone());
                    }
                }
            });
        if let Some(t) = chosen {
            ed.original = t.clone();
            app.set_theme(&ctx, t);
        }
    });

    let builtin = app.theme.builtin;
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Название"));
        ui.add_enabled(
            !builtin,
            egui::TextEdit::singleline(&mut app.theme.name).desired_width(180.0),
        );
        let mut dark = app.theme.dark;
        if ui
            .add_enabled(
                !builtin,
                egui::Checkbox::new(&mut dark, crate::i18n::tr("Тёмная основа")),
            )
            .changed()
        {
            app.theme.dark = dark;
            theme::apply(&ctx, &app.theme);
        }
    });

    ui.horizontal(|ui| {
        if ui
            .button(crate::i18n::tr("Создать копию"))
            .on_hover_text(crate::i18n::tr("Новая своя тема на основе текущей"))
            .clicked()
        {
            copy(app, ed, &ctx);
        }
        let changed = app.theme != ed.original;
        if ui
            .add_enabled(
                !builtin && changed,
                egui::Button::new(crate::i18n::tr("Сохранить")),
            )
            .clicked()
        {
            save(app, ed);
        }
        if ui
            .add_enabled(
                changed,
                egui::Button::new(crate::i18n::tr("Отменить правки")),
            )
            .clicked()
        {
            app.set_theme(&ctx, ed.original.clone());
        }
        if ui
            .add_enabled(!builtin, egui::Button::new(crate::i18n::tr("Удалить")))
            .clicked()
        {
            delete(app, ed, &ctx);
        }
    });
    if builtin {
        ui.weak(crate::i18n::tr(
            "Встроенную тему менять нельзя — создайте копию.",
        ));
    }
    ui.separator();

    let mut palette = app.theme.palette;
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut group = "";
        egui::Grid::new("theme_colors")
            .num_columns(3)
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (g, _, label, color) in palette.entries_mut() {
                    if g != group {
                        group = g;
                        ui.label("");
                        ui.label(RichText::new(g).strong());
                        ui.end_row();
                    }
                    ui.monospace(format!("{}, {}, {}", color.r(), color.g(), color.b()));
                    ui.label(label);
                    ui.add_enabled_ui(!builtin, |ui| {
                        ui.color_edit_button_srgba(color);
                    });
                    ui.end_row();
                }
            });
    });
    if palette != app.theme.palette {
        app.theme.palette = palette;
        theme::apply(&ctx, &app.theme);
    }
}

fn unique_name(app: &EditorApp, base: &str) -> String {
    let taken = |n: &str| app.themes.iter().any(|t| t.name == n);
    let mut name = crate::i18n::fmt("{0} (копия)", &[&base]);
    let mut i = 2;
    while taken(&name) {
        name = crate::i18n::fmt("{0} (копия {1})", &[&base, &i]);
        i += 1;
    }
    name
}

fn copy(app: &mut EditorApp, ed: &mut ThemeEditor, ctx: &egui::Context) {
    let mut t = app.theme.clone();
    t.name = unique_name(app, &t.name);
    t.builtin = false;
    if let Err(e) = theme::save(&t) {
        app.info(
            LogLevel::Error,
            crate::i18n::fmt("Тема не сохранена: {0}", &[&e]),
        );
        return;
    }
    app.themes = theme::load_all();
    ed.original = t.clone();
    app.set_theme(ctx, t);
}

fn save(app: &mut EditorApp, ed: &mut ThemeEditor) {
    let name = app.theme.name.trim().to_owned();
    if name.is_empty() {
        app.info(
            LogLevel::Error,
            crate::i18n::tr("У темы должно быть название"),
        );
        return;
    }
    if name != ed.original.name && app.themes.iter().any(|t| t.name == name) {
        app.info(
            LogLevel::Error,
            crate::i18n::fmt("Тема «{0}» уже есть", &[&name]),
        );
        return;
    }
    app.theme.name = name;
    let result = theme::save(&app.theme).and_then(|()| {
        if ed.original.name != app.theme.name {
            theme::remove(&ed.original.name)
        } else {
            Ok(())
        }
    });
    match result {
        Ok(()) => {
            app.themes = theme::load_all();
            ed.original = app.theme.clone();
            theme::save_choice(&app.theme.name);
            app.info(
                LogLevel::Info,
                crate::i18n::fmt("Тема «{0}» сохранена", &[&app.theme.name]),
            );
        }
        Err(e) => app.info(
            LogLevel::Error,
            crate::i18n::fmt("Тема не сохранена: {0}", &[&e]),
        ),
    }
}

fn delete(app: &mut EditorApp, ed: &mut ThemeEditor, ctx: &egui::Context) {
    if let Err(e) = theme::remove(&ed.original.name) {
        app.info(
            LogLevel::Error,
            crate::i18n::fmt("Тема не удалена: {0}", &[&e]),
        );
        return;
    }
    app.themes = theme::load_all();
    let fallback = if ed.original.dark {
        theme::dark()
    } else {
        theme::light()
    };
    ed.original = fallback.clone();
    app.set_theme(ctx, fallback);
}
