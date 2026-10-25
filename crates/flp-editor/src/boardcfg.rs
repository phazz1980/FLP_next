//! "MCU settings" dialog: board, upload port and board menu options for flashing.
//!
//! Menus (CPU frequency, flash size, partitions...) come from
//! `arduino-cli board details` and are loaded on a worker thread.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::egui::{self, RichText, Ui};
use flp_build::{ArduinoCli, BoardMenu, SerialPort};
use flp_core::BoardConfig;

/// Common ESP32 boards: FQBN and name.
const BOARDS: &[(&str, &str)] = &[
    ("esp32:esp32:esp32", "ESP32 Dev Module"),
    ("esp32:esp32:esp32s2", "ESP32-S2 Dev Module"),
    ("esp32:esp32:esp32s3", "ESP32-S3 Dev Module"),
    ("esp32:esp32:esp32c3", "ESP32-C3 Dev Module"),
    ("esp32:esp32:esp32doit-devkit-v1", "DOIT ESP32 DEVKIT V1"),
    ("esp32:esp32:nodemcu-32s", "NodeMCU-32S"),
    ("esp32:esp32:lolin32", "WEMOS LOLIN32"),
];

type MenusResult = Result<Vec<BoardMenu>, String>;

pub struct BoardDialog {
    fqbn: String,
    port: String,
    options: BTreeMap<String, String>,
    menus: Vec<BoardMenu>,
    /// FQBN the menus were loaded for.
    menus_for: String,
    loading: Option<Receiver<MenusResult>>,
    error: Option<String>,
}

impl BoardDialog {
    pub fn new(board: &BoardConfig, ctx: &egui::Context) -> Self {
        let mut d = Self {
            fqbn: board.fqbn.clone(),
            port: board.port.clone().unwrap_or_default(),
            options: board.options.clone(),
            menus: Vec::new(),
            menus_for: String::new(),
            loading: None,
            error: None,
        };
        d.load_menus(ctx);
        d
    }

    fn load_menus(&mut self, ctx: &egui::Context) {
        let fqbn = self.fqbn.trim().to_owned();
        if fqbn.is_empty() || fqbn == self.menus_for {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let job_fqbn = fqbn.clone();
        thread::spawn(move || {
            let result = ArduinoCli::locate()
                .and_then(|cli| cli.board_menus(&job_fqbn))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.menus_for = fqbn;
        self.loading = Some(rx);
        self.error = None;
    }

    fn poll(&mut self) {
        let Some(rx) = &self.loading else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.loading = None;
        match result {
            Ok(menus) => self.menus = menus,
            Err(e) => {
                self.menus.clear();
                self.error = Some(e);
            }
        }
    }

    /// Settings to write to the project file: only options that differ from the board defaults.
    pub fn config(&self) -> Result<BoardConfig, String> {
        let fqbn = self.fqbn.trim();
        if fqbn.split(':').count() < 3 {
            return Err(format!(
                "неверное имя платы «{fqbn}»: ожидается вид пакет:архитектура:плата"
            ));
        }
        let port = self.port.trim();
        let options = self
            .options
            .iter()
            .filter(|(k, v)| {
                // Without loaded menus keep everything as is.
                self.menus.is_empty()
                    || self
                        .menus
                        .iter()
                        .any(|m| &m.key == *k && m.default_value() != Some(v.as_str()))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(BoardConfig {
            fqbn: fqbn.to_owned(),
            port: (!port.is_empty()).then(|| port.to_owned()),
            options,
        })
    }

    pub fn body(&mut self, ui: &mut Ui, ports: &[SerialPort]) {
        self.poll();
        egui::Grid::new("board_main")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Плата (FQBN)");
                ui.horizontal(|ui| {
                    let edit =
                        ui.add(egui::TextEdit::singleline(&mut self.fqbn).desired_width(220.0));
                    if edit.lost_focus() {
                        self.load_menus(ui.ctx());
                    }
                    let mut picked = None;
                    ui.menu_button("▾", |ui| {
                        for (fqbn, name) in BOARDS {
                            if ui.button(format!("{name}  ({fqbn})")).clicked() {
                                picked = Some(*fqbn);
                                ui.close();
                            }
                        }
                    });
                    if let Some(fqbn) = picked {
                        if self.fqbn != fqbn {
                            self.fqbn = fqbn.to_owned();
                            self.options.clear();
                        }
                        self.load_menus(ui.ctx());
                    }
                });
                ui.end_row();

                ui.label("Порт");
                egui::ComboBox::from_id_salt("board_port")
                    .selected_text(if self.port.is_empty() {
                        "авто"
                    } else {
                        self.port.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.port, String::new(), "авто");
                        for p in ports {
                            ui.selectable_value(
                                &mut self.port,
                                p.address.clone(),
                                format!("{} — {}", p.address, p.description),
                            );
                        }
                    });
                ui.end_row();
            });

        ui.separator();
        ui.label(RichText::new("Параметры прошивки").strong());
        if self.loading.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Читаю параметры платы из arduino-cli...");
            });
            return;
        }
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
            if !self.options.is_empty() {
                ui.label("Заданные параметры сохранятся как есть:");
                for (k, v) in &self.options {
                    ui.monospace(format!("{k} = {v}"));
                }
            }
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                egui::Grid::new("board_menus")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for menu in &self.menus {
                            menu_row(ui, menu, &mut self.options);
                            ui.end_row();
                        }
                    });
            });
        if ui.button("Сбросить по умолчанию").clicked() {
            self.options.clear();
        }
    }
}

/// Label and drop-down for one board menu; non-default values are shown in bold.
fn menu_row(ui: &mut Ui, menu: &BoardMenu, options: &mut BTreeMap<String, String>) {
    let default = menu.default_value().unwrap_or_default().to_owned();
    let current = options.get(&menu.key).cloned().unwrap_or(default.clone());
    let label = if menu.label.is_empty() {
        &menu.key
    } else {
        &menu.label
    };
    let text = RichText::new(label);
    ui.label(if current != default {
        text.strong()
    } else {
        text
    })
    .on_hover_text(&menu.key);
    let shown = menu
        .values
        .iter()
        .find(|v| v.value == current)
        .map_or(current.clone(), |v| v.label.clone());
    let mut selected = current.clone();
    egui::ComboBox::from_id_salt(("board_menu", &menu.key))
        .selected_text(shown)
        .width(240.0)
        .show_ui(ui, |ui| {
            for v in &menu.values {
                let text = if v.label.is_empty() {
                    &v.value
                } else {
                    &v.label
                };
                ui.selectable_value(&mut selected, v.value.clone(), text);
            }
        });
    if selected != current {
        if selected == default {
            options.remove(&menu.key);
        } else {
            options.insert(menu.key.clone(), selected);
        }
    }
}
