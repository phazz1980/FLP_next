//! AI tab: chat with Claude that edits the open project through the shared
//! command set (`flp-ai`). The model runs on a worker thread with its own copy
//! of the project; every edit it makes comes back as one undo step.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, RichText, ScrollArea, Ui};
use flp_ai::agent::{run_turn, Event};
use flp_ai::provider::Claude;
use flp_ai::Context;
use flp_core::{Layout, Library, Logic, Project};
use serde_json::Value;

use crate::theme::Palette;

/// Live values the model may read: published by the editor while debugging.
pub type SharedLive = Arc<Mutex<Option<Vec<(String, String, bool)>>>>;

pub enum Entry {
    User(String),
    Assistant(String),
    Tool {
        name: String,
        input: String,
        ok: Option<bool>,
        result: String,
    },
    Error(String),
}

enum Msg {
    Event(Event),
    Edit(Logic, Layout),
    StopDebug,
    ReloadLibrary,
    Finished {
        history: Vec<Value>,
        error: Option<String>,
    },
}

/// Requests from the worker that the editor carries out.
pub enum Request {
    Apply(Logic, Layout),
    StopDebug,
    ReloadLibrary,
}

#[derive(Default)]
pub struct AiChat {
    pub entries: Vec<Entry>,
    pub input: String,
    history: Vec<Value>,
    worker: Option<Receiver<Msg>>,
}

impl AiChat {
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub fn clear(&mut self) {
        if !self.busy() {
            self.entries.clear();
            self.history.clear();
        }
    }

    /// Sends the typed message; the model works in the background.
    pub fn send(
        &mut self,
        project: &Project,
        lib: &Library,
        lib_dir: Option<PathBuf>,
        live: SharedLive,
        ctx: &egui::Context,
    ) {
        let text = self.input.trim().to_owned();
        if text.is_empty() || self.busy() {
            return;
        }
        self.input.clear();
        self.entries.push(Entry::User(text.clone()));
        let provider = match Claude::from_env() {
            Ok(p) => p,
            Err(e) => {
                self.entries.push(Entry::Error(e));
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        let mut history = std::mem::take(&mut self.history);
        let mut worker_ctx = WorkerContext {
            project: project.clone(),
            lib: lib.clone(),
            lib_dir: lib_dir.unwrap_or_else(|| PathBuf::from("library")),
            live,
            tx: tx.clone(),
            wake: ctx.clone(),
        };
        let wake = ctx.clone();
        thread::spawn(move || {
            let result = run_turn(&provider, &mut worker_ctx, &mut history, &text, &mut |e| {
                let _ = tx.send(Msg::Event(e));
                wake.request_repaint();
            });
            let _ = tx.send(Msg::Finished {
                history,
                error: result.err(),
            });
            wake.request_repaint();
        });
        self.worker = Some(rx);
    }

    /// Takes worker messages; returns what the editor must do.
    pub fn poll(&mut self) -> Vec<Request> {
        let Some(rx) = &self.worker else {
            return Vec::new();
        };
        let mut requests = Vec::new();
        let mut finished = false;
        for msg in rx.try_iter() {
            match msg {
                Msg::Event(Event::Text(t)) => self.entries.push(Entry::Assistant(t)),
                Msg::Event(Event::ToolCall { name, input }) => self.entries.push(Entry::Tool {
                    name,
                    input: compact(&input),
                    ok: None,
                    result: String::new(),
                }),
                Msg::Event(Event::ToolResult {
                    name: done,
                    ok,
                    text,
                }) => {
                    if let Some(Entry::Tool {
                        name,
                        ok: slot,
                        result,
                        ..
                    }) = self
                        .entries
                        .iter_mut()
                        .rev()
                        .find(|e| matches!(e, Entry::Tool { ok: None, .. }))
                    {
                        if *name == done {
                            *slot = Some(ok);
                            *result = text;
                        }
                    }
                }
                Msg::Edit(logic, layout) => requests.push(Request::Apply(logic, layout)),
                Msg::StopDebug => requests.push(Request::StopDebug),
                Msg::ReloadLibrary => requests.push(Request::ReloadLibrary),
                Msg::Finished { history, error } => {
                    self.history = history;
                    if let Some(e) = error {
                        self.entries.push(Entry::Error(e));
                    }
                    finished = true;
                }
            }
        }
        if finished {
            self.worker = None;
        }
        requests
    }
}

/// Context for the worker: a private copy of the project, edits mirrored to the editor.
struct WorkerContext {
    project: Project,
    lib: Library,
    lib_dir: PathBuf,
    live: SharedLive,
    tx: Sender<Msg>,
    wake: egui::Context,
}

impl Context for WorkerContext {
    fn project(&self) -> &Project {
        &self.project
    }

    fn library(&self) -> &Library {
        &self.lib
    }

    fn library_dir(&self) -> PathBuf {
        self.lib_dir.clone()
    }

    fn edit(
        &mut self,
        edit: &mut dyn FnMut(&mut Project) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut changed = self.project.clone();
        edit(&mut changed)?;
        let _ = self
            .tx
            .send(Msg::Edit(changed.logic.clone(), changed.layout.clone()));
        self.wake.request_repaint();
        self.project = changed;
        Ok(())
    }

    fn build(&mut self, upload: bool, port: Option<String>, debug: bool) -> Result<String, String> {
        if upload {
            // The debug session holds the serial port.
            let _ = self.tx.send(Msg::StopDebug);
            self.wake.request_repaint();
            thread::sleep(Duration::from_millis(800));
        }
        flp_ai::folder::build_project(&self.project, &self.lib, upload, port, debug)
    }

    fn live_values(&mut self) -> Result<Vec<(String, String, bool)>, String> {
        self.live
            .lock()
            .expect("live values")
            .clone()
            .ok_or_else(|| {
                "отладка не запущена: попросите пользователя нажать «Отладка»".to_owned()
            })
    }

    fn reload_library(&mut self) -> Result<(), String> {
        let (lib, errors) = Library::load(&self.lib_dir).map_err(|e| e.to_string())?;
        self.lib = lib;
        let _ = self.tx.send(Msg::ReloadLibrary);
        match errors.first() {
            Some(e) => Err(e.to_string()),
            None => Ok(()),
        }
    }
}

fn compact(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 120 {
        format!("{}…", s.chars().take(120).collect::<String>())
    } else {
        s
    }
}

/// Draws the chat. Returns true when the user pressed «Отправить».
pub fn show(chat: &mut AiChat, pal: &Palette, ui: &mut Ui, has_project: bool) -> bool {
    let mut send = false;
    let busy = chat.busy();
    egui::Panel::bottom("ai_input").show(ui, |ui| {
        ui.add_space(4.0);
        let edit = egui::TextEdit::multiline(&mut chat.input)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .hint_text("Что сделать со схемой? Например: «Добавь мигание светодиода на GPIO2 раз в секунду»");
        let resp = ui.add_enabled(has_project && !busy, edit);
        let ctrl_enter = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
        ui.horizontal(|ui| {
            if chat.busy() {
                ui.spinner();
                ui.label("Claude работает...");
            } else if ui
                .add_enabled(has_project, egui::Button::new("Отправить"))
                .on_hover_text("Ctrl+Enter")
                .clicked()
                || ctrl_enter
            {
                send = true;
            }
            if ui.add_enabled(!chat.busy(), egui::Button::new("Новый разговор")).clicked() {
                chat.clear();
            }
        });
    });
    ScrollArea::vertical()
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if chat.entries.is_empty() {
                ui.weak("Опишите задачу словами — помощник прочитает схему, добавит и соединит блоки и проверит результат. Каждую его правку можно отменить Ctrl+Z.\n\nНужен ключ Claude в переменной окружения ANTHROPIC_API_KEY.");
            }
            for (i, e) in chat.entries.iter().enumerate() {
                match e {
                    Entry::User(t) => {
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("Вы: {t}")).strong());
                    }
                    Entry::Assistant(t) => {
                        ui.label(t);
                    }
                    Entry::Tool { name, input, ok, result } => {
                        let (mark, color) = match ok {
                            None => ("…", pal.weak),
                            Some(true) => ("✔", pal.ok),
                            Some(false) => ("✖", pal.error),
                        };
                        egui::CollapsingHeader::new(RichText::new(format!("{mark} {name}")).color(color).small())
                            .id_salt(("ai_tool", i))
                            .show(ui, |ui| {
                                ui.monospace(input);
                                if !result.is_empty() {
                                    ui.separator();
                                    ui.monospace(compact(&Value::String(result.clone())));
                                }
                            });
                    }
                    Entry::Error(t) => {
                        ui.colored_label(pal.error, t);
                    }
                }
            }
        });
    send
}
