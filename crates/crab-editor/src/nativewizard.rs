//! Steps of the wizard for the native part of a block built on an Arduino library
//! (`specs/native-wizard`). Draws the fields of [`LibraryWizard`]; the block editor keeps the
//! rows of the form in step with them.

use crab_core::native_wizard::{LibraryWizard, WizAction, WizArg, WizOutput, WizParam, WizType};
use eframe::egui::{self, RichText, Ui};

use crate::theme::Palette;

/// Names of the steps, in order.
const STEPS: [&str; 6] = [
    "1. Библиотека",
    "2. Объект",
    "3. Запуск",
    "4. Выходы",
    "5. Действия",
    "6. Проверка",
];

/// Words an error text starts with, per step (`LibraryWizard::validate`).
const STEP_ERRORS: [&[&str]; 5] = [
    &["Библиотека"],
    &["Объект", "Параметр", "Аргумент"],
    &["Запуск"],
    &["Выход"],
    &["Действие", "Вход"],
];

/// Number of the step an error belongs to.
fn step_of(error: &str) -> Option<usize> {
    STEP_ERRORS
        .iter()
        .position(|words| words.iter().any(|w| error.starts_with(w)))
}

fn type_label(ty: WizType) -> &'static str {
    match ty {
        WizType::Pin => "пин",
        WizType::Int => "int",
        WizType::Float => "float",
        WizType::Bool => "bool",
    }
}

fn type_combo(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    ty: &mut WizType,
    pin: bool,
) {
    let all: &[WizType] = if pin {
        &[WizType::Pin, WizType::Int, WizType::Float, WizType::Bool]
    } else {
        &[WizType::Bool, WizType::Int, WizType::Float]
    };
    egui::ComboBox::from_id_salt(id)
        .width(70.0)
        .selected_text(crate::i18n::tr(type_label(*ty)))
        .show_ui(ui, |ui| {
            for t in all {
                ui.selectable_value(ty, *t, crate::i18n::tr(type_label(*t)));
            }
        });
}

/// A one-line text field of the given width.
fn field(ui: &mut Ui, text: &mut String, width: f32, hint: &str) {
    ui.add(
        egui::TextEdit::singleline(text)
            .desired_width(width)
            .hint_text(crate::i18n::tr(hint))
            .font(egui::TextStyle::Monospace),
    );
}

fn remove_button(ui: &mut Ui) -> bool {
    ui.small_button("✕")
        .on_hover_text(crate::i18n::tr("Убрать строку"))
        .clicked()
}

/// Draws the current step. Returns `true` when `w` changed.
pub fn wizard_ui(ui: &mut Ui, w: &mut LibraryWizard, step: &mut usize, pal: &Palette) -> bool {
    let before = w.clone();
    let errors = w.validate();
    ui.horizontal_wrapped(|ui| {
        for (i, name) in STEPS.iter().enumerate() {
            let bad = errors.iter().any(|e| step_of(e) == Some(i));
            let mut text = RichText::new(crate::i18n::tr(name));
            if bad {
                text = text.color(pal.error);
            }
            if ui.selectable_label(*step == i, text).clicked() {
                *step = i;
            }
        }
    });
    ui.separator();
    match *step {
        0 => step_library(ui, w),
        1 => step_object(ui, w),
        2 => step_init(ui, w),
        3 => step_outputs(ui, w),
        4 => step_actions(ui, w),
        _ => {}
    }
    if *step < 5 {
        for e in errors.iter().filter(|e| step_of(e) == Some(*step)) {
            ui.colored_label(pal.error, e);
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(*step > 0, egui::Button::new(crate::i18n::tr("← Назад")))
            .clicked()
        {
            *step -= 1;
        }
        if ui
            .add_enabled(*step < 5, egui::Button::new(crate::i18n::tr("Далее →")))
            .clicked()
        {
            *step += 1;
        }
    });
    *w != before
}

fn step_library(ui: &mut Ui, w: &mut LibraryWizard) {
    ui.weak(crate::i18n::tr(
        "Библиотека Arduino, на которой сделан датчик. Если её ещё нет на компьютере, \
         мастер-прошивка скачает её сама по этому имени.",
    ));
    egui::Grid::new("wiz_lib").num_columns(2).show(ui, |ui| {
        ui.label(crate::i18n::tr("Имя в менеджере библиотек"));
        field(ui, &mut w.lib, 260.0, "например GyverHX711");
        ui.end_row();
        ui.label(crate::i18n::tr("Заголовочный файл"));
        field(ui, &mut w.header, 260.0, "например GyverHX711.h");
        ui.end_row();
    });
}

fn step_object(ui: &mut Ui, w: &mut LibraryWizard) {
    ui.weak(crate::i18n::tr(
        "Библиотека работает через объект класса. Параметры блока (пины, числа) пользователь \
         задаёт на схеме, а здесь вы решаете, какие из них передать конструктору.",
    ));
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Класс"));
        field(ui, &mut w.class, 220.0, "например GyverHX711");
    });
    ui.add_space(4.0);
    ui.label(RichText::new(crate::i18n::tr("Параметры блока")).strong());
    let mut drop = None;
    for (i, p) in w.params.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            field(ui, &mut p.name, 140.0, "имя, латиницей");
            type_combo(ui, ("wiz_param_ty", i), &mut p.ty, true);
            if remove_button(ui) {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        let name = w.params.remove(i).name;
        // An argument that named it falls back to a constant to be filled in.
        for a in &mut w.args {
            if matches!(a, WizArg::Param(n) if *n == name) {
                *a = WizArg::Const(String::new());
            }
        }
    }
    if ui.button(crate::i18n::tr("+ Параметр")).clicked() {
        w.params.push(WizParam {
            name: String::new(),
            ty: WizType::Pin,
        });
    }
    ui.add_space(6.0);
    ui.label(RichText::new(crate::i18n::tr("Аргументы конструктора, по порядку")).strong());
    let names: Vec<String> = w.params.iter().map(|p| p.name.trim().to_owned()).collect();
    let mut drop = None;
    for (i, a) in w.args.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let mut is_param = matches!(a, WizArg::Param(_));
            ui.selectable_value(&mut is_param, true, crate::i18n::tr("параметр"));
            ui.selectable_value(&mut is_param, false, crate::i18n::tr("константа"));
            match (is_param, &mut *a) {
                (true, WizArg::Param(_)) | (false, WizArg::Const(_)) => {}
                (true, a) => *a = WizArg::Param(names.first().cloned().unwrap_or_default()),
                (false, a) => *a = WizArg::Const(String::new()),
            }
            match a {
                WizArg::Param(n) => {
                    egui::ComboBox::from_id_salt(("wiz_arg", i))
                        .selected_text(n.as_str())
                        .show_ui(ui, |ui| {
                            for name in &names {
                                ui.selectable_value(n, name.clone(), name);
                            }
                        });
                }
                WizArg::Const(t) => field(ui, t, 180.0, "например HX_GAIN128_A или 9600"),
            }
            if remove_button(ui) {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        w.args.remove(i);
    }
    if ui.button(crate::i18n::tr("+ Аргумент")).clicked() {
        w.args.push(WizArg::Const(String::new()));
    }
}

fn step_init(ui: &mut Ui, w: &mut LibraryWizard) {
    ui.weak(crate::i18n::tr(
        "Вызовы, которые делаются один раз после создания объекта и при смене параметров, \
         например begin(). Если библиотеке ничего не нужно, оставьте пусто.",
    ));
    let mut drop = None;
    for (i, c) in w.init.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("объект.").monospace());
            field(ui, c, 240.0, "например begin()");
            if remove_button(ui) {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        w.init.remove(i);
    }
    if ui.button(crate::i18n::tr("+ Вызов")).clicked() {
        w.init.push(String::new());
    }
}

fn step_outputs(ui: &mut Ui, w: &mut LibraryWizard) {
    ui.weak(crate::i18n::tr(
        "Что блок отдаёт в схему: имя выхода, тип и вызов библиотеки, который даёт значение. \
         «Когда» — условие обновления: вызов вроде available() или имя выхода bool выше. \
         Пусто — обновлять каждый цикл.",
    ));
    let mut drop = None;
    for (i, o) in w.outputs.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            field(ui, &mut o.name, 110.0, "имя выхода");
            type_combo(ui, ("wiz_out_ty", i), &mut o.ty, false);
            ui.label(RichText::new("= объект.").monospace());
            field(ui, &mut o.expr, 150.0, "например read()");
            ui.label(crate::i18n::tr("когда"));
            field(ui, &mut o.when, 130.0, "всегда");
            if remove_button(ui) {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        w.outputs.remove(i);
    }
    if ui.button(crate::i18n::tr("+ Выход")).clicked() {
        w.outputs.push(WizOutput {
            name: String::new(),
            ty: WizType::Float,
            expr: String::new(),
            when: String::new(),
        });
    }
}

fn step_actions(ui: &mut Ui, w: &mut LibraryWizard) {
    ui.weak(crate::i18n::tr(
        "Команды блоку: когда вход bool меняется с 0 на 1, вызывается метод библиотеки \
         (например tare() у весов). Вход появится у блока на схеме.",
    ));
    let mut drop = None;
    for (i, a) in w.actions.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::i18n::tr("по входу"));
            field(ui, &mut a.input, 110.0, "имя входа");
            ui.label(RichText::new("→ объект.").monospace());
            field(ui, &mut a.call, 170.0, "например tare()");
            if remove_button(ui) {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        w.actions.remove(i);
    }
    if ui.button(crate::i18n::tr("+ Действие")).clicked() {
        w.actions.push(WizAction {
            input: String::new(),
            call: String::new(),
        });
    }
}
