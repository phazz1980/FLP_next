//! Bottom «Port monitor» tab: a plain serial monitor without debugging, and the
//! firmware console while a debugging session holds the port.

use std::collections::VecDeque;

use eframe::egui::{self, RichText, ScrollArea, TextStyle, Ui};
use flp_debug::{Live, Monitor};

use crate::theme::Palette;

/// Lines kept in the tab.
const KEEP: usize = 5000;
/// Speeds offered for the monitor; the firmware uses `flp_debug::transport::BAUD`.
const BAUDS: [u32; 9] = [
    9600, 19200, 38400, 57600, 74880, 115200, 230400, 460800, 921600,
];

/// What is appended to a typed line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    None,
    Lf,
    CrLf,
}

impl Ending {
    fn text(self) -> &'static str {
        match self {
            Ending::None => "",
            Ending::Lf => "\n",
            Ending::CrLf => "\r\n",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Ending::None => "без окончания",
            Ending::Lf => "\\n",
            Ending::CrLf => "\\r\\n",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Printed by the board.
    Board,
    /// Typed by the user and sent.
    Sent,
    /// Connected, disconnected and other notes of the editor.
    Note,
}

struct Line {
    time: String,
    text: String,
    kind: Kind,
}

pub struct PortConsole {
    lines: VecDeque<Line>,
    /// Lines received during a pause; shown when it ends.
    held: Vec<Line>,
    /// The user pressed «Подключить»: keep the monitor open whenever the port is free.
    pub want: bool,
    /// Running monitor with the port and speed it was opened with.
    monitor: Option<(Monitor, String, u32)>,
    pub baud: u32,
    pub input: String,
    pub ending: Ending,
    pub timestamps: bool,
    pub autoscroll: bool,
    pub paused: bool,
    pub filter: String,
    /// A debugging session holds the port: the tab is its console.
    in_debug: bool,
    /// Console lines of the session already shown (its `console_total`).
    debug_seen: u64,
    connected: bool,
    /// The debugging session talks to firmware built for this schema.
    debug_online: bool,
    /// The schema reads UART0 (`uart_read`): console input has a reader on the board.
    pub debug_reader: bool,
    /// Lines typed in the debug console, to send as `TEXT_IN`.
    debug_out: Vec<Vec<u8>>,
    status: String,
    /// The AI uploads firmware: the port stays free until it finishes.
    pub hold: bool,
    /// Lines that came while the tab was not shown.
    pub unread: usize,
}

impl Default for PortConsole {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            held: Vec::new(),
            want: false,
            monitor: None,
            baud: flp_debug::transport::BAUD,
            input: String::new(),
            ending: Ending::CrLf,
            timestamps: true,
            autoscroll: true,
            paused: false,
            filter: String::new(),
            in_debug: false,
            debug_seen: 0,
            connected: false,
            debug_online: false,
            debug_reader: false,
            debug_out: Vec::new(),
            status: "Отключено".into(),
            hold: false,
            unread: 0,
        }
    }
}

impl PortConsole {
    /// Tab title: the console while debugging, the monitor otherwise.
    pub fn title(&self) -> &'static str {
        if self.in_debug {
            "Консоль отладки"
        } else {
            "Монитор порта"
        }
    }

    /// Closes the monitor so that an upload or a debugging session can take the port.
    /// It reopens by itself once the port is free (see [`Self::sync`]).
    pub fn release(&mut self) {
        if let Some((monitor, port, _)) = self.monitor.take() {
            monitor.stop();
            self.connected = false;
            self.note(format!("{port} освобождён"));
        }
    }

    /// Called every frame. `debug`: live state of the running debugging session;
    /// `port`: the board port to open; `busy`: the port is taken by an upload.
    pub fn sync(
        &mut self,
        debug: Option<&Live>,
        port: Option<String>,
        busy: bool,
        ctx: &egui::Context,
    ) {
        if let Some(live) = debug {
            if !self.in_debug {
                self.release();
                self.in_debug = true;
                self.debug_seen = 0;
                self.note("консоль отладки".into());
            }
            // A new session (reconnect) counts its lines from zero again.
            if live.console_total < self.debug_seen {
                self.debug_seen = 0;
            }
            let fresh: Vec<String> = live.console_after(self.debug_seen).to_vec();
            self.debug_seen = live.console_total;
            for text in fresh {
                self.push(Kind::Board, text);
            }
            self.connected = live.connected;
            self.debug_online = live.connected && live.schema_matches == Some(true);
            self.status = live.status.clone();
            return;
        }
        if self.in_debug {
            self.in_debug = false;
            self.note("отладка остановлена".into());
        }

        let target = match (self.want && !busy, port) {
            (true, Some(port)) => Some(port),
            (true, None) => {
                self.status = "Выберите порт платы на панели инструментов".into();
                None
            }
            (false, _) => None,
        };
        let same = |m: &(Monitor, String, u32)| Some(&m.1) == target.as_ref() && m.2 == self.baud;
        if self.monitor.as_ref().is_some_and(|m| !same(m)) {
            self.release();
        }
        if let (None, Some(port)) = (&self.monitor, target) {
            let ctx = ctx.clone();
            let monitor = Monitor::start(
                flp_debug::transport::serial_at(&port, self.baud),
                move || ctx.request_repaint(),
            );
            self.monitor = Some((monitor, port, self.baud));
        }

        let state = self.monitor.as_ref().map(|(monitor, port, baud)| {
            let (connected, status) = monitor.status();
            (connected, status, monitor.take_lines(), port.clone(), *baud)
        });
        match state {
            Some((connected, status, lines, port, baud)) => {
                if connected != self.connected {
                    let text = if connected {
                        format!("подключено к {port}, {baud} бод")
                    } else {
                        status.clone()
                    };
                    self.note(text);
                }
                self.connected = connected;
                self.status = format!("{port}: {status}");
                for text in lines {
                    self.push(Kind::Board, text);
                }
            }
            None => {
                self.connected = false;
                if !self.want {
                    self.status = "Отключено".into();
                } else if busy {
                    self.status = "Порт занят заливкой".into();
                }
            }
        }
    }

    /// Whether typed lines can go to the board now: to the port, or while
    /// debugging to the `uart_read` block on UART0.
    fn can_send(&self) -> bool {
        if self.in_debug {
            self.debug_online && self.debug_reader
        } else {
            self.connected
        }
    }

    fn send_input(&mut self) {
        if !self.can_send() {
            return;
        }
        let line = std::mem::take(&mut self.input);
        let bytes = format!("{line}{}", self.ending.text()).into_bytes();
        if self.in_debug {
            self.debug_out.push(bytes);
        } else if let Some((monitor, ..)) = &self.monitor {
            monitor.send(bytes);
        }
        self.push(Kind::Sent, line);
    }

    /// Lines typed in the debug console since the last call.
    pub fn take_debug_input(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.debug_out)
    }

    fn note(&mut self, text: String) {
        self.push(Kind::Note, text);
    }

    fn push(&mut self, kind: Kind, text: String) {
        let line = Line {
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            text,
            kind,
        };
        self.unread += 1;
        if self.paused && kind == Kind::Board {
            self.held.push(line);
            let excess = self.held.len().saturating_sub(KEEP);
            self.held.drain(..excess);
            return;
        }
        self.lines.push_back(line);
        while self.lines.len() > KEEP {
            self.lines.pop_front();
        }
    }

    fn resume(&mut self) {
        self.paused = false;
        self.lines.extend(self.held.drain(..));
        while self.lines.len() > KEEP {
            self.lines.pop_front();
        }
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.held.clear();
    }
}

/// Header controls and the lines of the tab.
pub fn show(console: &mut PortConsole, pal: &Palette, ui: &mut Ui) {
    console.unread = 0;
    ui.horizontal(|ui| {
        let color = if console.connected { pal.ok } else { pal.weak };
        ui.label(RichText::new("●").color(color));
        if !console.in_debug {
            let text = if console.want {
                "Отключить"
            } else {
                "Подключить"
            };
            if ui.button(text).clicked() {
                console.want = !console.want;
            }
            egui::ComboBox::from_id_salt("monitor_baud")
                .selected_text(format!("{} бод", console.baud))
                .show_ui(ui, |ui| {
                    for b in BAUDS {
                        ui.selectable_value(&mut console.baud, b, b.to_string());
                    }
                });
        }
        ui.label(RichText::new(&console.status).color(pal.weak));
        ui.separator();
        ui.checkbox(&mut console.timestamps, "Время");
        ui.checkbox(&mut console.autoscroll, "Автопрокрутка");
        let pause = if console.paused {
            format!("Продолжить (+{})", console.held.len())
        } else {
            "Пауза".to_owned()
        };
        if ui.selectable_label(console.paused, pause).clicked() {
            if console.paused {
                console.resume();
            } else {
                console.paused = true;
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut console.filter)
                .hint_text("фильтр")
                .desired_width(120.0),
        );
        if ui.small_button("Копировать").clicked() {
            let text = visible(console)
                .map(|l| format_line(console, l))
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(text);
        }
        if ui.small_button("Очистить").clicked() {
            console.clear();
        }
    });

    // The input line stays at the bottom; lines take the rest.
    egui::Panel::bottom("console_input")
        .show_separator_line(false)
        .show(ui, |ui| input_row(console, ui));

    let rows: Vec<&Line> = visible(console).collect();
    let row_height = ui.text_style_height(&TextStyle::Monospace);
    ScrollArea::both()
        .auto_shrink(false)
        .stick_to_bottom(console.autoscroll)
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for line in &rows[range] {
                let color = match line.kind {
                    Kind::Board => pal.text,
                    Kind::Sent => pal.accent,
                    Kind::Note => pal.weak,
                };
                let text = RichText::new(format_line(console, line))
                    .color(color)
                    .monospace();
                ui.add(egui::Label::new(text).extend());
            }
        });
}

fn input_row(console: &mut PortConsole, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let enabled = console.can_send();
        let hint = if console.in_debug && !console.debug_reader {
            "в схеме нет блока «Чтение из UART» на UART0 — строку некому прочитать"
        } else if console.in_debug && !enabled {
            "плата не на связи"
        } else if console.in_debug {
            "строка для «Чтения из UART» на UART0, Enter — отправить"
        } else if enabled {
            "строка для платы, Enter — отправить"
        } else {
            "монитор не подключён"
        };
        let width = (ui.available_width() - 220.0).max(120.0);
        let edit = ui.add_enabled(
            enabled,
            egui::TextEdit::singleline(&mut console.input)
                .hint_text(hint)
                .desired_width(width),
        );
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        egui::ComboBox::from_id_salt("monitor_ending")
            .selected_text(console.ending.label())
            .show_ui(ui, |ui| {
                for e in [Ending::None, Ending::Lf, Ending::CrLf] {
                    ui.selectable_value(&mut console.ending, e, e.label());
                }
            });
        let send = ui
            .add_enabled(enabled, egui::Button::new("Отправить"))
            .clicked();
        if enter || send {
            console.send_input();
            edit.request_focus();
        }
    });
}

fn visible(console: &PortConsole) -> impl Iterator<Item = &Line> {
    let filter = console.filter.trim().to_lowercase();
    console
        .lines
        .iter()
        .filter(move |l| filter.is_empty() || l.text.to_lowercase().contains(&filter))
}

fn format_line(console: &PortConsole, line: &Line) -> String {
    let mark = match line.kind {
        Kind::Board => "",
        Kind::Sent => "→ ",
        Kind::Note => "— ",
    };
    if console.timestamps {
        format!("{}  {mark}{}", line.time, line.text)
    } else {
        format!("{mark}{}", line.text)
    }
}
