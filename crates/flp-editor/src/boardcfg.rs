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
use flp_core::pins::{BoardLayout, BoardModel, BOARD_MODELS};
use flp_core::uart::{chip_uarts, UartConfig, USB_SPEED};
use flp_core::{BoardConfig, Boards};

/// Common ESP32 boards: FQBN and name.
pub(crate) const BOARDS: &[(&str, &str)] = &[
    ("esp32:esp32:esp32", "ESP32 Dev Module"),
    ("esp32:esp32:esp32s2", "ESP32-S2 Dev Module"),
    ("esp32:esp32:esp32s3", "ESP32-S3 Dev Module"),
    ("esp32:esp32:esp32c3", "ESP32-C3 Dev Module"),
    ("esp32:esp32:esp32doit-devkit-v1", "DOIT ESP32 DEVKIT V1"),
    ("esp32:esp32:nodemcu-32s", "NodeMCU-32S"),
    ("esp32:esp32:lolin32", "WEMOS LOLIN32"),
];

/// Human-readable board name: its model (built-in or the user's), the list of common
/// boards, otherwise the last FQBN part.
pub fn board_name<'a>(board: &'a BoardConfig, boards: &'a Boards) -> &'a str {
    let fqbn = board.fqbn.as_str();
    if let Some(m) = board.model.as_deref().and_then(BoardModel::find) {
        if m.fqbn == fqbn {
            return m.name;
        }
    }
    if let Some(b) = board
        .model
        .as_deref()
        .and_then(|id| boards.get(id))
        .filter(|b| b.def.info.fqbn == fqbn)
    {
        return &b.def.info.name;
    }
    BOARDS
        .iter()
        .find(|(f, _)| *f == fqbn)
        .map_or_else(|| fqbn.rsplit(':').next().unwrap_or(fqbn), |(_, name)| name)
}

type MenusResult = Result<Vec<BoardMenu>, String>;

pub struct BoardDialog {
    fqbn: String,
    /// Id of the exact board picked from the list (built-in model or user board);
    /// dropped when the FQBN no longer matches it.
    model: Option<String>,
    /// User boards: their pin tables, FQBNs and presets.
    boards: Boards,
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
    /// UARTs set up by a settings block on a sheet: (number, block id).
    from_blocks: Vec<(u8, String)>,
    /// The user asked for the board editor: `Some(None)` — a new board, `Some(Some(id))` —
    /// the board `id`. The caller takes it with [`BoardDialog::take_editor_request`].
    editor_request: Option<Option<String>>,
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
        from_blocks: Vec<(u8, String)>,
        boards: &Boards,
        ctx: &egui::Context,
    ) -> Self {
        let mut d = Self {
            uarts: uarts.to_vec(),
            users,
            from_blocks,
            editor_request: None,
            fqbn: board.fqbn.clone(),
            model: board.model.clone(),
            boards: boards.clone(),
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
            return Err(crate::i18n::fmt(
                "неверное имя платы «{0}»: ожидается вид пакет:архитектура:плата",
                &[&fqbn],
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
            .clone()
            .filter(|id| self.model_fqbn(id) == Some(fqbn));
        Ok(BoardConfig {
            fqbn: fqbn.to_owned(),
            model,
            port: (!port.is_empty()).then(|| port.to_owned()),
            options,
        })
    }

    /// FQBN of a board model: built-in or user.
    fn model_fqbn(&self, id: &str) -> Option<&str> {
        BoardModel::find(id)
            .map(|m| m.fqbn)
            .or_else(|| self.boards.get(id).map(|b| b.def.info.fqbn.as_str()))
    }

    /// Name of the picked board model, while the FQBN still belongs to it.
    fn model_title(&self) -> Option<&str> {
        let id = self.model.as_deref()?;
        if self.model_fqbn(id)? != self.fqbn.trim() {
            return None;
        }
        BoardModel::find(id)
            .map(|m| m.name)
            .or_else(|| self.boards.get(id).map(|b| b.def.info.name.as_str()))
    }

    /// `[[uart]]` to write to the project file.
    pub fn uarts(&self) -> Vec<UartConfig> {
        let mut uarts = self.uarts.clone();
        uarts.sort_by_key(|u| u.number);
        uarts
    }

    fn uart_section(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("UART").strong());
        ui.weak(crate::i18n::fmt("USB — заливка, отладка и монитор порта, {0} бод, без настроек. UART0 у ESP32 занят USB-мостом, у ESP32-C3/S3 — свободный порт.", &[&USB_SPEED]));
        let board = BoardConfig {
            fqbn: self.fqbn.trim().to_owned(),
            model: self.model.clone(),
            port: None,
            options: BTreeMap::new(),
        };
        let chip = BoardLayout::of_board(&board, &self.boards).map(|l| l.chip());
        let mut numbers: Vec<u8> = chip.map_or(vec![1, 2], |c| chip_uarts(c).to_vec());
        // A UART set up for another board stays visible, so it can be switched off.
        for u in &self.uarts {
            if !numbers.contains(&u.number) {
                numbers.push(u.number);
            }
        }
        numbers.sort_unstable();
        for number in numbers {
            if let Some((_, id)) = self.from_blocks.iter().find(|(n, _)| *n == number) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("UART{number}")).strong());
                    ui.weak(crate::i18n::fmt(
                        "настроен блоком «{0}» на схеме — меняйте его свойства",
                        &[&id],
                    ));
                });
                continue;
            }
            let users: Vec<&str> = self
                .users
                .iter()
                .filter(|(n, id)| *n == number && !self.from_blocks.iter().any(|(_, b)| b == id))
                .map(|(_, id)| id.as_str())
                .collect();
            let index = self.uarts.iter().position(|u| u.number == number);
            let mut on = index.is_some();
            ui.horizontal(|ui| {
                ui.checkbox(&mut on, format!("UART{number}"));
                if !users.is_empty() {
                    ui.weak(crate::i18n::fmt("используют: {0}", &[&users.join(", ")]));
                }
                if chip.is_some_and(|c| !chip_uarts(c).contains(&number)) {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        crate::i18n::tr("нет на этой плате"),
                    );
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

    /// What the user asked of the board editor since the last call (see the field).
    pub fn take_editor_request(&mut self) -> Option<Option<String>> {
        self.editor_request.take()
    }

    pub fn body(&mut self, ui: &mut Ui, ports: &[SerialPort]) {
        self.poll();
        egui::Grid::new("board_main")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(crate::i18n::tr("Плата (FQBN)"));
                ui.horizontal(|ui| {
                    let edit =
                        ui.add(egui::TextEdit::singleline(&mut self.fqbn).desired_width(220.0));
                    if edit.lost_focus() {
                        self.load_menus(ui.ctx());
                    }
                    // (FQBN, board id, presets of a user board)
                    let mut picked: Option<(String, Option<String>, Option<BTreeMap<_, _>>)> = None;
                    ui.menu_button("▾", |ui| {
                        for (fqbn, name) in BOARDS {
                            if ui.button(*name).on_hover_text(*fqbn).clicked() {
                                picked = Some(((*fqbn).to_owned(), None, None));
                                ui.close();
                            }
                        }
                        ui.separator();
                        for m in BOARD_MODELS {
                            if ui.button(m.name).on_hover_text(m.fqbn).clicked() {
                                picked = Some((m.fqbn.to_owned(), Some(m.id.to_owned()), None));
                                ui.close();
                            }
                        }
                        if !self.boards.is_empty() {
                            ui.separator();
                            for b in self.boards.iter() {
                                let info = &b.def.info;
                                if ui.button(&info.name).on_hover_text(&info.fqbn).clicked() {
                                    picked = Some((
                                        info.fqbn.clone(),
                                        Some(info.id.clone()),
                                        Some(info.options.clone()),
                                    ));
                                    ui.close();
                                }
                            }
                        }
                    });
                    if let Some((fqbn, model, presets)) = picked {
                        if self.fqbn != fqbn {
                            self.fqbn = fqbn;
                            self.options.clear();
                        }
                        if let Some(presets) = presets {
                            self.options = presets;
                        }
                        self.model = model;
                        self.load_menus(ui.ctx());
                    }
                    if let Some(name) = self.model_title() {
                        ui.label(RichText::new(name).weak());
                    }
                    let user_board = self
                        .model
                        .clone()
                        .filter(|id| self.model_title().is_some() && self.boards.get(id).is_some());
                    if let Some(id) = user_board {
                        if ui
                            .small_button(crate::i18n::tr("Изменить плату..."))
                            .on_hover_text(crate::i18n::tr(
                                "Выводы, периферия и описание этой платы",
                            ))
                            .clicked()
                        {
                            self.editor_request = Some(Some(id));
                        }
                    }
                    if ui
                        .small_button(crate::i18n::tr("Новая плата..."))
                        .on_hover_text(crate::i18n::tr(
                            "Описать свою плату: выводы, периферию, схемы",
                        ))
                        .clicked()
                    {
                        self.editor_request = Some(None);
                    }
                });
                ui.end_row();

                ui.label(crate::i18n::tr("Порт"));
                egui::ComboBox::from_id_salt("board_port")
                    .selected_text(if self.port.is_empty() {
                        crate::i18n::tr("авто")
                    } else {
                        self.port.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.port, String::new(), crate::i18n::tr("авто"));
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
        ui.label(RichText::new(crate::i18n::tr("Параметры прошивки")).strong());
        if self.loading.is_some() {
            ui.horizontal(|ui| {
                crate::panels::busy(ui);
                ui.label(crate::i18n::tr("Читаю параметры платы из arduino-cli..."));
            });
            return;
        }
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
            if !self.options.is_empty() {
                ui.label(crate::i18n::tr("Заданные параметры сохранятся как есть:"));
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
        if ui
            .button(crate::i18n::tr("Сбросить по умолчанию"))
            .clicked()
        {
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
            ui.label(crate::i18n::tr("Скорость, бод"));
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

            ui.label(crate::i18n::tr("Формат"));
            egui::ComboBox::from_id_salt(("uart_format", id))
                .selected_text(u.format.as_str())
                .show_ui(ui, |ui| {
                    for f in FORMATS {
                        ui.selectable_value(&mut u.format, f.to_owned(), f);
                    }
                })
                .response
                .on_hover_text(crate::i18n::tr(
                    "биты данных, чётность (N — нет, E — чёт, O — нечет), стоп-биты",
                ));
            ui.end_row();

            for (label, pin, hint) in [
                (
                    "RX",
                    &mut u.rx,
                    crate::i18n::tr("-1 — вывод ядра Arduino по умолчанию"),
                ),
                (
                    "TX",
                    &mut u.tx,
                    crate::i18n::tr("-1 — вывод ядра Arduino по умолчанию"),
                ),
                (
                    "DE/RE (RS-485)",
                    &mut u.de,
                    crate::i18n::tr("-1 — без RS-485"),
                ),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(pin).range(-1..=48).prefix("GPIO "))
                    .on_hover_text(hint);
                ui.end_row();
            }

            ui.label(crate::i18n::tr("Буфер приёма, байт"));
            ui.add(egui::DragValue::new(&mut u.rx_buffer).range(64..=8192));
            ui.end_row();
        });
}

#[cfg(test)]
mod tests {
    use flp_core::boards::{BoardDef, BoardInfo};
    use flp_core::UserBoard;

    use super::*;

    fn boards() -> Boards {
        let mut boards = Boards::default();
        boards.insert(UserBoard {
            def: BoardDef {
                info: BoardInfo {
                    id: "kc868".into(),
                    name: "KC868".into(),
                    chip: "esp32".into(),
                    fqbn: "esp32:esp32:esp32".into(),
                    version: 1,
                    options: BTreeMap::new(),
                },
                pins: Vec::new(),
                expander_pins: Vec::new(),
                devices: Vec::new(),
            },
            dir: "kc868".into(),
        });
        boards
    }

    fn project_board(model: Option<&str>) -> BoardConfig {
        BoardConfig {
            fqbn: "esp32:esp32:esp32".into(),
            model: model.map(str::to_owned),
            port: None,
            options: BTreeMap::new(),
        }
    }

    #[test]
    fn a_user_board_has_its_name_and_a_stale_model_is_dropped() {
        let boards = boards();
        assert_eq!(board_name(&project_board(Some("kc868")), &boards), "KC868");
        // Another FQBN than the board's: the generic name.
        let mut other = project_board(Some("kc868"));
        other.fqbn = "esp32:esp32:esp32s3".into();
        assert_eq!(board_name(&other, &boards), "ESP32-S3 Dev Module");
        assert_eq!(
            board_name(&project_board(None), &boards),
            "ESP32 Dev Module"
        );

        // An empty FQBN keeps the dialog from asking arduino-cli for menus.
        let mut empty = project_board(Some("kc868"));
        empty.fqbn.clear();
        let mut dialog = BoardDialog::new(
            &empty,
            &[],
            Vec::new(),
            Vec::new(),
            &boards,
            &egui::Context::default(),
        );
        dialog.fqbn = "esp32:esp32:esp32".into();
        assert_eq!(dialog.model_title(), Some("KC868"));
        assert_eq!(dialog.config().unwrap().model.as_deref(), Some("kc868"));
        dialog.fqbn = "esp32:esp32:esp32s3".into();
        assert_eq!(dialog.model_title(), None);
        assert_eq!(dialog.config().unwrap().model, None);
    }
}
