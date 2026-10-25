//! "MCU settings" dialog: board, upload port, hardware UARTs and board menu
//! options for flashing.
//!
//! Menus (CPU frequency, flash size, partitions...) come from
//! `arduino-cli board details` and are loaded on a worker thread.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::egui::{self, RichText, Ui};
use flp_build::{ArduinoCli, BoardMenu, SerialPort};
use flp_core::pins::{BoardModel, Chip, BOARD_MODELS};
use flp_core::uart::{chip_uarts, UartConfig, UART0_SPEED};
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

/// Human-readable board name: its model, the list of common boards, otherwise the last FQBN part.
pub fn board_name(board: &BoardConfig) -> &str {
    let fqbn = board.fqbn.as_str();
    if let Some(m) = board.model.as_deref().and_then(BoardModel::find) {
        if m.fqbn == fqbn {
            return m.name;
        }
    }
    BOARDS
        .iter()
        .find(|(f, _)| *f == fqbn)
        .map_or_else(|| fqbn.rsplit(':').next().unwrap_or(fqbn), |(_, name)| name)
}

type MenusResult = Result<Vec<BoardMenu>, String>;

pub struct BoardDialog {
    fqbn: String,
    /// Exact board picked from the list; dropped when the FQBN no longer matches it.
    model: Option<&'static BoardModel>,
    port: String,
    options: BTreeMap<String, String>,
    menus: Vec<BoardMenu>,
    /// FQBN the menus were loaded for.
    menus_for: String,
    loading: Option<Receiver<MenusResult>>,
    error: Option<String>,
    /// `[[uart]]` of the project, edited here.
    uarts: Vec<UartConfig>,
    /// Blocks on each UART: (number, block id), to show who uses a port.
    users: Vec<(u8, String)>,
}

/// Speeds offered for a UART; any other can be typed.
const SPEEDS: [u32; 10] = [
    1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800,
];
const FORMATS: [&str; 8] = ["8N1", "8E1", "8O1", "8N2", "8E2", "8O2", "7E1", "7O1"];

impl BoardDialog {
    pub fn new(
        board: &BoardConfig,
        uarts: &[UartConfig],
        users: Vec<(u8, String)>,
        ctx: &egui::Context,
    ) -> Self {
        let mut d = Self {
            uarts: uarts.to_vec(),
            users,
            fqbn: board.fqbn.clone(),
            model: board.model.as_deref().and_then(BoardModel::find),
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
        let model = self
            .model
            .filter(|m| m.fqbn == fqbn)
            .map(|m| m.id.to_owned());
        Ok(BoardConfig {
            fqbn: fqbn.to_owned(),
            model,
            port: (!port.is_empty()).then(|| port.to_owned()),
            options,
        })
    }

    /// `[[uart]]` to write to the project file.
    pub fn uarts(&self) -> Vec<UartConfig> {
        let mut uarts = self.uarts.clone();
        uarts.sort_by_key(|u| u.number);
        uarts
    }

    fn uart_section(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("UART").strong());
        ui.weak(format!(
            "UART0 — USB: заливка, отладка и монитор порта, {UART0_SPEED} бод. Блоки могут только писать в него."
        ));
        let board = BoardConfig {
            fqbn: self.fqbn.trim().to_owned(),
            model: self.model.map(|m| m.id.to_owned()),
            port: None,
            options: BTreeMap::new(),
        };
        let chip = Chip::of_board(&board);
        let mut numbers: Vec<u8> = chip.map_or(vec![1, 2], |c| chip_uarts(c).to_vec());
        // A UART set up for another board stays visible, so it can be switched off.
        for u in &self.uarts {
            if !numbers.contains(&u.number) {
                numbers.push(u.number);
            }
        }
        numbers.sort_unstable();
        for number in numbers {
            let users: Vec<&str> = self
                .users
                .iter()
                .filter(|(n, _)| *n == number)
                .map(|(_, id)| id.as_str())
                .collect();
            let index = self.uarts.iter().position(|u| u.number == number);
            let mut on = index.is_some();
            ui.horizontal(|ui| {
                ui.checkbox(&mut on, format!("UART{number}"));
                if !users.is_empty() {
                    ui.weak(format!("используют: {}", users.join(", ")));
                }
                if chip.is_some_and(|c| !chip_uarts(c).contains(&number)) {
                    ui.colored_label(ui.visuals().error_fg_color, "нет на этой плате");
                }
            });
            match (on, index) {
                (true, None) => self.uarts.push(UartConfig::new(number)),
                (false, Some(i)) => {
                    self.uarts.remove(i);
                }
                _ => {}
            }
            let Some(u) = self.uarts.iter_mut().find(|u| u.number == number) else {
                continue;
            };
            ui.indent(("uart", number), |ui| uart_grid(ui, u));
        }
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
                                picked = Some((*fqbn, None));
                                ui.close();
                            }
                        }
                        ui.separator();
                        for m in BOARD_MODELS {
                            if ui.button(format!("{}  ({})", m.name, m.fqbn)).clicked() {
                                picked = Some((m.fqbn, Some(m)));
                                ui.close();
                            }
                        }
                    });
                    if let Some((fqbn, model)) = picked {
                        if self.fqbn != fqbn {
                            self.fqbn = fqbn.to_owned();
                            self.options.clear();
                        }
                        self.model = model;
                        self.load_menus(ui.ctx());
                    }
                    if let Some(m) = self.model.filter(|m| m.fqbn == self.fqbn.trim()) {
                        ui.label(RichText::new(m.name).weak());
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
        self.uart_section(ui);

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

/// Speed, format, pins and buffer of one UART.
fn uart_grid(ui: &mut Ui, u: &mut UartConfig) {
    let id = u.number;
    egui::Grid::new(("uart_grid", id))
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label("Скорость, бод");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("uart_speed", id))
                    .selected_text(u.speed.to_string())
                    .show_ui(ui, |ui| {
                        for s in SPEEDS {
                            ui.selectable_value(&mut u.speed, s, s.to_string());
                        }
                    });
                ui.add(egui::DragValue::new(&mut u.speed).range(300..=5_000_000));
            });
            ui.end_row();

            ui.label("Формат");
            egui::ComboBox::from_id_salt(("uart_format", id))
                .selected_text(u.format.as_str())
                .show_ui(ui, |ui| {
                    for f in FORMATS {
                        ui.selectable_value(&mut u.format, f.to_owned(), f);
                    }
                })
                .response
                .on_hover_text("биты данных, чётность (N — нет, E — чёт, O — нечет), стоп-биты");
            ui.end_row();

            for (label, pin, hint) in [
                ("RX", &mut u.rx, "-1 — вывод ядра Arduino по умолчанию"),
                ("TX", &mut u.tx, "-1 — вывод ядра Arduino по умолчанию"),
                ("DE/RE (RS-485)", &mut u.de, "-1 — без RS-485"),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(pin).range(-1..=48).prefix("GPIO "))
                    .on_hover_text(hint);
                ui.end_row();
            }

            ui.label("Буфер приёма, байт");
            ui.add(egui::DragValue::new(&mut u.rx_buffer).range(64..=8192));
            ui.end_row();
        });
}
