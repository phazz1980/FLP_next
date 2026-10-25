//! "Project code" window: the Arduino C++ sketch generated from the schema, read-only.
//! Regenerated whenever the schema or the debugging option changes.

use eframe::egui::{self, RichText, TextEdit};
use flp_codegen::{ArduinoCpp, Backend, Program};

use crate::app::{EditorApp, LogLevel};

pub struct CodeView {
    /// Generated sketch, or the reason it could not be generated.
    code: Result<String, String>,
    /// Revision and debugging option the code was generated for.
    key: Option<(u64, bool)>,
}

impl CodeView {
    pub fn new() -> Self {
        Self {
            code: Ok(String::new()),
            key: None,
        }
    }

    fn refresh(&mut self, app: &EditorApp) {
        let Some(doc) = &app.doc else {
            self.code = Err("Проект не открыт".into());
            self.key = None;
            return;
        };
        let key = (doc.revision, app.debug_firmware);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let project = doc.project.with_local(&app.lib, &app.local);
        self.code = Program::build(&project, &app.lib)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                ArduinoCpp {
                    debug: app.debug_firmware,
                }
                .generate(&p)
                .map_err(|e| e.to_string())
            });
    }
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut view) = app.code_view.take() else {
        return;
    };
    view.refresh(app);
    let mut open = true;
    egui::Window::new("Код проекта (C++)")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .default_height(600.0)
        .show(ctx, |ui| match &view.code {
            Ok(code) => {
                ui.horizontal(|ui| {
                    if ui.button("Копировать").clicked() {
                        ui.ctx().copy_text(code.clone());
                        app.info(LogLevel::Info, "Код проекта скопирован");
                    }
                    let mode = if app.debug_firmware {
                        "с модулем отладки"
                    } else {
                        "без отладки"
                    };
                    ui.weak(format!("Строк: {}, {mode}", code.lines().count()));
                });
                ui.separator();
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut text = code.as_str();
                        ui.add(
                            TextEdit::multiline(&mut text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
            }
            Err(e) => {
                ui.label(RichText::new("Код не сгенерирован:").strong());
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        });
    if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.code_view = Some(view);
    }
}
