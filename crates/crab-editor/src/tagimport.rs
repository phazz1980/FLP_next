//! "Tags → Transfer from project" window: takes the tags another project publishes over
//! Modbus as external tags of this one. The source project is only read.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crab_core::modbus_tags::{self, MqttTopic, PublishedTag};
use crab_core::project::PROJECT_EXT;
use crab_core::ParamValue;
use eframe::egui;

use crate::app::{EditorApp, LogLevel};

#[derive(Default)]
pub struct TagImport {
    source: Option<PathBuf>,
    /// Published tags of the source with the "take it" mark.
    tags: Vec<(PublishedTag, bool)>,
    /// MQTT topics of the source: "take it" mark and the role here (subscription or
    /// publication; a topic the source publishes is a subscription here by default).
    topics: Vec<(MqttTopic, bool, bool)>,
    /// MQTT client settings of the source (broker, port, login) and the "take it" mark.
    settings: Option<(BTreeMap<String, ParamValue>, bool)>,
    error: String,
}

impl TagImport {
    fn load(&mut self, path: PathBuf) {
        let loaded = modbus_tags::load_published(&path)
            .and_then(|t| Ok((t, modbus_tags::load_mqtt_topics(&path)?)))
            .and_then(|(t, m)| Ok((t, m, modbus_tags::load_mqtt_settings(&path)?)));
        match loaded {
            Ok((tags, topics, settings)) => {
                self.settings = settings.map(|s| (s, true));
                self.tags = tags.into_iter().map(|t| (t, true)).collect();
                self.topics = topics
                    .into_iter()
                    .map(|t| {
                        let subscribe = t.published;
                        (t, true, subscribe)
                    })
                    .collect();
                self.error =
                    if self.tags.is_empty() && self.topics.is_empty() && self.settings.is_none() {
                        crate::i18n::tr("В проекте нет блоков Modbus и MQTT").into()
                    } else {
                        String::new()
                    };
            }
            Err(e) => {
                self.tags.clear();
                self.topics.clear();
                self.settings = None;
                self.error = e.to_string();
            }
        }
        self.source = Some(path);
    }
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut view) = app.tag_import.take() else {
        return;
    };
    let mut open = true;
    let mut transfer = false;
    egui::Window::new(crate::i18n::tr("Перенести теги из проекта"))
        .open(&mut open)
        .collapsible(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("Выбрать проект...")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(crate::i18n::tr("Проект, откуда взять теги"))
                        .add_filter(crate::i18n::tr("Проект CrabLogic"), &[PROJECT_EXT])
                        .pick_file()
                    {
                        view.load(path);
                    }
                }
                if ui.button(crate::i18n::tr("Папка проекта...")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        view.load(path);
                    }
                }
            });
            if let Some(path) = &view.source {
                ui.weak(path.display().to_string());
            }
            if !view.error.is_empty() {
                ui.colored_label(ui.visuals().error_fg_color, &view.error);
            }
            for (tag, on) in &mut view.tags {
                let text = format!("{}  ({} {})", tag.name, tag.table, tag.address);
                ui.checkbox(on, text);
            }
            if !view.topics.is_empty() {
                ui.separator();
                ui.label(crate::i18n::tr("MQTT: что это здесь?"));
                for (topic, on, subscribe) in &mut view.topics {
                    ui.horizontal(|ui| {
                        ui.checkbox(on, &topic.topic);
                        ui.radio_value(subscribe, true, crate::i18n::tr("подписка"));
                        ui.radio_value(subscribe, false, crate::i18n::tr("публикация"));
                    });
                }
            }
            if let Some((_, on)) = &mut view.settings {
                ui.separator();
                ui.checkbox(
                    on,
                    crate::i18n::tr("Настройки MQTT-брокера (и включить Wi-Fi, если выключен)"),
                );
            }
            ui.separator();
            let any = view.tags.iter().any(|(_, on)| *on)
                || view.topics.iter().any(|t| t.1)
                || view.settings.as_ref().is_some_and(|s| s.1);
            if ui
                .add_enabled(any, egui::Button::new(crate::i18n::tr("Перенести")))
                .on_hover_text(crate::i18n::tr(
                    "Повторный перенос обновляет адреса уже взятых тегов",
                ))
                .clicked()
            {
                transfer = true;
            }
        });
    if transfer {
        apply(app, &view);
        open = false;
    }
    if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.tag_import = Some(view);
    }
}

fn apply(app: &mut EditorApp, view: &TagImport) {
    let (Some(source), Some(doc)) = (&view.source, &mut app.doc) else {
        return;
    };
    let base = doc.project.dir().to_path_buf();
    let stored = modbus_tags::stored_path(&base, source);
    let tags: Vec<PublishedTag> = view
        .tags
        .iter()
        .filter(|(_, on)| *on)
        .map(|(t, _)| t.clone())
        .collect();
    let lib = app.lib.clone();
    let report = doc
        .edit(&lib, None, |p| {
            Ok::<_, String>(p.logic.import_tags(&stored, &tags))
        })
        .unwrap_or_default();
    app.info(
        LogLevel::Info,
        crate::i18n::fmt(
            "Теги: добавлено {0}, обновлено {1}, не взято {2}",
            &[
                &report.added.len(),
                &report.updated.len(),
                &report.conflicts.len(),
            ],
        ),
    );
    for c in report.conflicts {
        app.info(LogLevel::Warning, c);
    }
    add_settings(app, view);
    add_topics(app, view);
}

/// Copies the MQTT client settings, switching Wi-Fi on when the network needs it.
fn add_settings(app: &mut EditorApp, view: &TagImport) {
    let Some((settings, true)) = &view.settings else {
        return;
    };
    let lib = app.lib.clone();
    let pos = crate::panels::free_spot(app);
    let Some(doc) = &mut app.doc else { return };
    match doc.edit(&lib, None, |p| p.import_mqtt_settings(&lib, settings, pos)) {
        Ok(done) => {
            for line in done {
                app.info(LogLevel::Info, line);
            }
        }
        Err(e) => app.info(LogLevel::Error, e.to_string()),
    }
}

/// Creates the blocks for the chosen MQTT topics, one under another on the current sheet.
fn add_topics(app: &mut EditorApp, view: &TagImport) {
    let chosen: Vec<_> = view.topics.iter().filter(|(_, on, _)| *on).collect();
    if chosen.is_empty() {
        return;
    }
    let lib = app.lib.clone();
    let mut pos = crate::panels::free_spot(app);
    let (mut added, mut skipped) = (0, 0);
    let Some(doc) = &mut app.doc else { return };
    for (topic, _, subscribe) in chosen {
        let result = doc.edit(&lib, None, |p| {
            p.add_mqtt_topic(&lib, &topic.topic, *subscribe, &topic.kind, pos)
        });
        match result {
            Ok(Some(_)) => {
                added += 1;
                pos.y += 90.0;
            }
            _ => skipped += 1,
        }
    }
    app.info(
        LogLevel::Info,
        crate::i18n::fmt(
            "MQTT: добавлено блоков {0}, уже были или не добавлены {1}",
            &[&added, &skipped],
        ),
    );
}
