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
    /// Session id of the external client, to continue the conversation.
    Session(String),
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
    /// `claude` session of the external client (MCP mode).
    session: Option<String>,
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
            self.session = None;
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
        let provider = match flp_ai::provider::from_env() {
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
            let result = run_turn(
                provider.as_ref(),
                &mut worker_ctx,
                &mut history,
                &text,
                &mut |e| {
                    let _ = tx.send(Msg::Event(e));
                    wake.request_repaint();
                },
            );
            let _ = tx.send(Msg::Finished {
                history,
                error: result.err(),
            });
            wake.request_repaint();
        });
        self.worker = Some(rx);
    }

    /// MCP mode: the request goes to the external client (`claude -p`), which edits the
    /// project through the editor's MCP server instead of the API.
    pub fn send_external(
        &mut self,
        endpoint: (String, String),
        env: Vec<(&'static str, String)>,
        ctx: &egui::Context,
    ) {
        let text = self.input.trim().to_owned();
        if text.is_empty() || self.busy() {
            return;
        }
        self.input.clear();
        self.entries.push(Entry::User(text.clone()));
        let (tx, rx) = mpsc::channel();
        let session = self.session.clone();
        let wake = ctx.clone();
        thread::spawn(move || {
            let result = external::run(&endpoint, &env, session.as_deref(), &text, &mut |m| {
                let _ = tx.send(m);
                wake.request_repaint();
            });
            let _ = tx.send(Msg::Finished {
                history: Vec::new(),
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
                Msg::Session(id) => self.session = Some(id),
                Msg::Edit(logic, layout) => requests.push(Request::Apply(logic, layout)),
                Msg::StopDebug => requests.push(Request::StopDebug),
                Msg::ReloadLibrary => requests.push(Request::ReloadLibrary),
                Msg::Finished { history, error } => {
                    if !history.is_empty() {
                        self.history = history;
                    }
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
        flp_core::user_library_dir(&self.lib_dir)
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
                crate::i18n::tr("отладка не запущена: попросите пользователя нажать «Отладка»")
                    .to_owned()
            })
    }

    fn reload_library(&mut self) -> Result<(), String> {
        let (lib, errors) = Library::load_with_user(&self.lib_dir).map_err(|e| e.to_string())?;
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
            .hint_text(crate::i18n::tr("Что сделать со схемой? Например: «Добавь мигание светодиода на GPIO2 раз в секунду»"));
        let resp = ui.add_enabled(has_project && !busy, edit);
        let ctrl_enter = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
        ui.horizontal(|ui| {
            if chat.busy() {
                crate::panels::busy(ui);
                ui.label(crate::i18n::tr("Claude работает..."));
            } else if ui
                .add_enabled(has_project, egui::Button::new(crate::i18n::tr("Отправить")))
                .on_hover_text("Ctrl+Enter")
                .clicked()
                || ctrl_enter
            {
                send = true;
            }
            if ui.add_enabled(!chat.busy(), egui::Button::new(crate::i18n::tr("Новый разговор"))).clicked() {
                chat.clear();
            }
        });
    });
    ScrollArea::vertical()
        .auto_shrink([ui.is_sizing_pass(), false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if chat.entries.is_empty() {
                ui.weak(crate::i18n::tr("Опишите задачу словами — помощник прочитает схему, добавит и соединит блоки и проверит результат. Каждую его правку можно отменить Ctrl+Z.\n\nНужен ключ Claude в переменной окружения ANTHROPIC_API_KEY. Для другого сервиса с OpenAI-совместимым API задайте FLP_AI_BASE_URL, FLP_AI_KEY и FLP_AI_MODEL."));
            }
            for (i, e) in chat.entries.iter().enumerate() {
                match e {
                    Entry::User(t) => {
                        ui.add_space(6.0);
                        ui.label(RichText::new(crate::i18n::fmt("Вы: {0}", &[&t])).strong());
                    }
                    Entry::Assistant(t) => {
                        ui.label(t);
                    }
                    Entry::Tool { name, input, ok, result } => {
                        let (mark, color) = match ok {
                            None => ("…", pal.weak),
                            Some(true) => ("✔", pal.ok),
                            Some(false) => ("✕", pal.error),
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

/// Runs `claude -p` against the editor's MCP server and turns its stream into chat events.
mod external {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    use serde_json::{json, Value};

    use super::{Event, Msg};

    const TOOLS_OFF: &str = "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,Task,NotebookEdit";
    const INTRO: &str = "Ты помощник CrabLogic. Работай только через инструменты MCP-сервера crablogic \
        с проектом, открытым в редакторе: начни с get_schema и list_block_types, после правок смотри поле check. \
        Отвечай по-русски.\n\nЗадача: ";

    pub fn run(
        endpoint: &(String, String),
        env: &[(&str, String)],
        session: Option<&str>,
        text: &str,
        emit: &mut dyn FnMut(Msg),
    ) -> Result<(), String> {
        let dir = std::env::temp_dir().join("crablogic-ai");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let config = dir.join("mcp.json");
        let body = json!({"mcpServers": {"crablogic": {
            "type": "http",
            "url": endpoint.0,
            "headers": {"Authorization": format!("Bearer {}", endpoint.1)},
        }}});
        std::fs::write(&config, body.to_string()).map_err(|e| e.to_string())?;

        let args = |cmd: &mut Command| {
            cmd.args(["-p", "--output-format", "stream-json", "--verbose"])
                .arg("--mcp-config")
                .arg(&config)
                .args(["--strict-mcp-config", "--allowedTools", "mcp__crablogic"])
                .args(["--disallowedTools", TOOLS_OFF]);
            if let Some(id) = session {
                cmd.args(["--resume", id]);
            }
            cmd.envs(env.iter().map(|(k, v)| (*k, v)))
                .current_dir(&dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        };
        let mut cmd = Command::new("claude");
        args(&mut cmd);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            // `claude` installed through npm is a .cmd script.
            Err(_) => {
                let mut cmd = Command::new("cmd");
                cmd.args(["/C", "claude"]);
                args(&mut cmd);
                cmd.spawn().map_err(|_| {
                    "Не найден Claude Code (команда `claude`). Установите его или выключите MCP, чтобы использовать API.".to_string()
                })?
            }
        };
        let prompt = if session.is_some() {
            text.to_owned()
        } else {
            format!("{INTRO}{text}")
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let stderr = child.stderr.take();
        let mut names: HashMap<String, String> = HashMap::new();
        let mut failure = None;
        for line in BufReader::new(child.stdout.take().ok_or("нет вывода claude")?).lines()
        {
            let Ok(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            handle(&v, &mut names, &mut failure, emit);
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if let Some(e) = failure {
            return Err(e);
        }
        if !status.success() {
            let mut err = String::new();
            if let Some(mut s) = stderr {
                let _ = std::io::Read::read_to_string(&mut s, &mut err);
            }
            return Err(format!("claude завершился с ошибкой: {}", err.trim()));
        }
        Ok(())
    }

    /// One stream-json line: assistant text and tool calls, tool results, final result.
    fn handle(
        v: &Value,
        names: &mut HashMap<String, String>,
        failure: &mut Option<String>,
        emit: &mut dyn FnMut(Msg),
    ) {
        if let Some(id) = v["session_id"].as_str() {
            emit(Msg::Session(id.to_owned()));
        }
        match v["type"].as_str() {
            Some("assistant") => {
                for block in v["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => {
                            if let Some(t) = block["text"].as_str().filter(|t| !t.trim().is_empty())
                            {
                                emit(Msg::Event(Event::Text(t.to_owned())));
                            }
                        }
                        Some("tool_use") => {
                            let name = short(block["name"].as_str().unwrap_or(""));
                            if let Some(id) = block["id"].as_str() {
                                names.insert(id.to_owned(), name.clone());
                            }
                            emit(Msg::Event(Event::ToolCall {
                                name,
                                input: block["input"].clone(),
                            }));
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for block in v["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] != "tool_result" {
                        continue;
                    }
                    let name = block["tool_use_id"]
                        .as_str()
                        .and_then(|id| names.get(id).cloned())
                        .unwrap_or_default();
                    let text = match &block["content"] {
                        Value::String(s) => s.clone(),
                        Value::Array(a) => a
                            .iter()
                            .filter_map(|b| b["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => String::new(),
                    };
                    emit(Msg::Event(Event::ToolResult {
                        name,
                        ok: block["is_error"] != true,
                        text,
                    }));
                }
            }
            Some("result") if v["is_error"] == true => {
                *failure = Some(v["result"].as_str().unwrap_or("ошибка claude").to_owned());
            }
            _ => {}
        }
    }

    /// `mcp__crablogic__add_block` -> `add_block`.
    fn short(name: &str) -> String {
        name.rsplit("__").next().unwrap_or(name).to_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn stream_lines_become_events() {
            let mut names = HashMap::new();
            let mut failure = None;
            let mut got = Vec::new();
            let mut emit = |m: Msg| got.push(m);
            let lines = [
                r#"{"type":"assistant","session_id":"s1","message":{"content":[{"type":"text","text":"Смотрю схему"},{"type":"tool_use","id":"t1","name":"mcp__crablogic__get_schema","input":{}}]}}"#,
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"{}"}]}]}}"#,
                r#"{"type":"result","is_error":true,"result":"плохо"}"#,
            ];
            for l in lines {
                handle(
                    &serde_json::from_str(l).unwrap(),
                    &mut names,
                    &mut failure,
                    &mut emit,
                );
            }
            assert_eq!(failure.as_deref(), Some("плохо"));
            assert!(matches!(&got[0], Msg::Session(s) if s == "s1"));
            assert!(
                matches!(&got[2], Msg::Event(Event::ToolCall { name, .. }) if name == "get_schema")
            );
            assert!(
                matches!(&got[3], Msg::Event(Event::ToolResult { name, ok: true, .. }) if name == "get_schema")
            );
        }
    }
}
