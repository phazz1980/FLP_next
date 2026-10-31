//! CrabLogic MCP server: exposes the CrabLogic command set to any MCP client
//! (Claude Desktop, Claude Code, ...) over stdio, one JSON-RPC message per line.
//!
//! The `crablogic-mcp` binary speaks it over stdio; the editor serves it over HTTP
//! on localhost ([`http`]).

pub mod http;

use std::path::{Path, PathBuf};

use flp_ai::folder::FolderContext;
use serde_json::{json, Value};

/// Protocol revisions this server speaks, newest first.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Сервер проекта CrabLogic — визуального программирования ESP32 \
(язык FBD). Схема — блоки с понятными id и связи «блок.порт» → «блок.порт». \
Сервер работает только с проектом, открытым в запущенном редакторе CrabLogic. \
Если открыто несколько окон редактора — спросите пользователя, какое нужно, \
посмотрите list_editors и выберите select_editor. \
Начните с get_schema и list_block_types; после правок смотрите поле check в ответе. \
Правки сразу сохраняются в файл проекта .crab.";

const LIST_EDITORS: &str = "list_editors";
const SELECT_EDITOR: &str = "select_editor";

/// Server state: the open project (if any) and the library override.
pub struct Server {
    ctx: Option<FolderContext>,
    /// From `--library` / `FLP_LIBRARY`; otherwise searched per project.
    library: Option<PathBuf>,
    /// No project was named on the command line: work on the project of a running
    /// editor window, switching when the user opens another one there.
    follow_editor: bool,
    /// Editor project this server last switched to, to switch only on a change.
    followed: Option<PathBuf>,
    /// Editor window (pid) chosen with `select_editor` when several are running.
    selected: Option<u32>,
}

impl Server {
    /// `follow_editor`: work on the project of a running editor window (read from its
    /// heartbeat files) instead of one set with [`Server::open`].
    pub fn new(library: Option<PathBuf>, follow_editor: bool) -> Self {
        Self {
            ctx: None,
            library,
            follow_editor,
            followed: None,
            selected: None,
        }
    }

    /// Sets the project served by the editor itself (HTTP mode): opens it when it
    /// changed, drops the old one when the editor has none.
    pub fn set_project(&mut self, path: Option<&Path>) {
        match path {
            Some(p) if self.followed.as_deref() != Some(p) => match self.open(p) {
                Ok(_) => self.followed = Some(p.to_path_buf()),
                Err(e) => {
                    eprintln!("crablogic-mcp: {e}");
                    self.ctx = None;
                    self.followed = None;
                }
            },
            Some(_) => {}
            None => {
                self.ctx = None;
                self.followed = None;
            }
        }
    }

    /// Picks the editor window to work with and opens its project.
    fn follow_editor_project(&mut self) -> Result<(), String> {
        let editors = flp_core::local::live_editors();
        let chosen = match self
            .selected
            .and_then(|p| editors.iter().find(|e| e.pid == p))
        {
            Some(e) => e,
            None => match editors.as_slice() {
                [] => {
                    return Err(
                        "редактор CrabLogic не запущен: запустите приложение и откройте проект"
                            .into(),
                    )
                }
                [one] => one,
                many => {
                    return Err(format!(
                        "запущено несколько редакторов ({}): спросите пользователя, какой нужен, \
                         и вызовите select_editor с pid",
                        describe(many)
                    ))
                }
            },
        };
        let Some(path) = chosen.project.clone() else {
            return Err("в редакторе нет открытого проекта: откройте его в приложении".into());
        };
        if self.followed.as_ref() != Some(&path) {
            self.open(&path)?;
            self.followed = Some(path);
        }
        Ok(())
    }

    /// Opens (or switches to) the project file `path` (or a folder holding one).
    pub fn open(&mut self, path: &Path) -> Result<Value, String> {
        if !flp_core::project::is_project(path) {
            return Err(format!("{} — не проект CrabLogic (.crab)", path.display()));
        }
        let library = self
            .library
            .clone()
            .or_else(|| find_library(path))
            .ok_or("не найдена папка библиотеки блоков library/; укажите --library")?;
        let ctx = FolderContext::open(path, &library)?;
        self.ctx = Some(ctx);
        eprintln!(
            "crablogic-mcp: проект {}, библиотека {}",
            path.display(),
            library.display()
        );
        Ok(json!({"project": path.display().to_string(), "library": library.display().to_string()}))
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if name == LIST_EDITORS {
            let editors: Vec<Value> = flp_core::local::live_editors()
                .iter()
                .map(|e| {
                    json!({"pid": e.pid, "project": e.project.as_ref().map(|p| p.display().to_string())})
                })
                .collect();
            return Ok(json!({"editors": editors, "selected": self.selected}));
        }
        if name == SELECT_EDITOR {
            let pid = args
                .get("pid")
                .and_then(Value::as_u64)
                .and_then(|p| u32::try_from(p).ok())
                .ok_or("нужен аргумент pid — из list_editors")?;
            if !flp_core::local::live_editors().iter().any(|e| e.pid == pid) {
                return Err(format!("редактор с pid {pid} не запущен; см. list_editors"));
            }
            self.selected = Some(pid);
            self.follow_editor = true;
            self.followed = None;
            self.follow_editor_project()?;
            let project = self.followed.as_ref().map(|p| p.display().to_string());
            return Ok(json!({"selected": pid, "project": project}));
        }
        if self.follow_editor {
            self.follow_editor_project()?;
        }
        let ctx = self
            .ctx
            .as_mut()
            .ok_or("проект не открыт: откройте его в редакторе CrabLogic")?;
        // See edits made in the editor since the last call.
        ctx.refresh().and_then(|()| flp_ai::call(ctx, name, args))
    }
}

fn editor_tools() -> [Value; 2] {
    [
        json!({
            "name": LIST_EDITORS,
            "description": "Запущенные окна редактора CrabLogic и открытый в каждом проект. \
        Если окон несколько, спросите пользователя, какое нужно, и вызовите select_editor.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": SELECT_EDITOR,
            "description": "Выбрать окно редактора (pid из list_editors), с проектом которого работать.",
            "inputSchema": {
                "type": "object",
                "properties": {"pid": {"type": "integer", "description": "pid окна из list_editors"}},
                "required": ["pid"],
            },
        }),
    ]
}

/// Window list for messages: `pid 123 — file`.
fn describe(editors: &[flp_core::local::EditorInfo]) -> String {
    editors
        .iter()
        .map(|e| match &e.project {
            Some(p) => format!("pid {} — {}", e.pid, p.display()),
            None => format!("pid {} — без проекта", e.pid),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Handles one message; returns the response line, or `None` for notifications.
pub fn handle(server: &mut Server, line: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    // Notifications (no id) get no response.
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|v| PROTOCOL_VERSIONS.contains(v))
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "crablogic", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => {
            let tools: Vec<Value> = editor_tools().into_iter()
                .chain(flp_ai::tools().into_iter().map(|t| {
                    json!({"name": t.name, "description": t.description, "inputSchema": t.input_schema})
                }))
                .collect();
            json!({"tools": tools})
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let (text, is_error) = match server.call(name, &args) {
                Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                Err(e) => (e, true),
            };
            json!({"content": [{"type": "text", "text": text}], "isError": is_error})
        }
        _ => return Some(error(id, -32601, &format!("method not found: {method}"))),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// `library/` in the project folder or its parents, then next to the executable.
fn find_library(project: &Path) -> Option<PathBuf> {
    let starts = [
        std::path::absolute(project).ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
    ];
    starts
        .into_iter()
        .flatten()
        .flat_map(|s| s.ancestors().map(|a| a.join("library")).collect::<Vec<_>>())
        .find(|d| d.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn ctx() -> Server {
        let mut s = Server {
            ctx: None,
            library: Some(root().join("library")),
            follow_editor: false,
            followed: None,
            selected: None,
        };
        s.open(&root().join("examples/pump")).unwrap();
        s
    }

    fn rpc(s: &mut Server, msg: Value) -> Option<Value> {
        handle(s, &msg.to_string())
    }

    fn call(s: &mut Server, name: &str, args: Value) -> Value {
        rpc(
            s,
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
                   "params": {"name": name, "arguments": args}}),
        )
        .unwrap()["result"]
            .clone()
    }

    #[test]
    fn select_unknown_editor_fails() {
        let mut s = Server {
            ctx: None,
            library: None,
            follow_editor: true,
            followed: None,
            selected: None,
        };
        let r = call(&mut s, SELECT_EDITOR, json!({"pid": 4_000_000_000u64}));
        assert_eq!(r["isError"], true);
        assert_eq!(call(&mut s, SELECT_EDITOR, json!({}))["isError"], true);
    }

    #[test]
    fn handshake_and_tools() {
        let mut c = ctx();
        let init = rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                   "params": {"protocolVersion": "2025-03-26", "capabilities": {}}}),
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none());

        let list = rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "get_schema"));
        assert!(tools.iter().any(|t| t["name"] == LIST_EDITORS));
        assert!(tools.iter().all(|t| t["inputSchema"]["type"] == "object"));
    }

    #[test]
    fn read_only_call_and_errors() {
        let mut c = ctx();
        let r = rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                   "params": {"name": "check", "arguments": {}}}),
        )
        .unwrap();
        assert_eq!(r["result"]["isError"], false);
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"order\""));

        let r = rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                   "params": {"name": "remove_block", "arguments": {"id": "nope"}}}),
        )
        .unwrap();
        assert_eq!(r["result"]["isError"], true);

        let r = rpc(
            &mut c,
            json!({"jsonrpc": "2.0", "id": 5, "method": "resources/list"}),
        )
        .unwrap();
        assert_eq!(r["error"]["code"], -32601);
        assert_eq!(handle(&mut c, "{oops").unwrap()["error"]["code"], -32700);
    }
}
