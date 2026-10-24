//! `flp-mcp` — MCP server: exposes the FLP_next command set to any MCP client
//! (Claude Desktop, Claude Code, ...) over stdio, one JSON-RPC message per line.
//!
//! Usage: `flp-mcp [project folder] [--library <dir>]`. Without a folder the
//! client opens one with the `open_project` tool (also used to switch).
//! Only JSON-RPC goes to stdout; diagnostics go to stderr.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use flp_ai::folder::FolderContext;
use serde_json::{json, Value};

/// Protocol revisions this server speaks, newest first.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Сервер проекта FLP_next — визуального программирования ESP32 \
(FBD, как в FLProg). Схема — блоки с понятными id и связи «блок.порт» → «блок.порт». \
Если проект не открыт или пользователь называет другую папку — вызовите open_project. \
Начните с get_schema и list_block_types; после правок смотрите поле check в ответе. \
Правки сразу сохраняются в папку проекта.";

const OPEN_PROJECT: &str = "open_project";

/// Server state: the open project (if any) and the library override.
struct Server {
    ctx: Option<FolderContext>,
    /// From `--library` / `FLP_LIBRARY`; otherwise searched per project.
    library: Option<PathBuf>,
}

impl Server {
    /// Opens (or switches to) the project folder `path`.
    fn open(&mut self, path: &Path) -> Result<Value, String> {
        if !path.join("project.toml").is_file() {
            return Err(format!("в папке {} нет project.toml", path.display()));
        }
        let library = self
            .library
            .clone()
            .or_else(|| find_library(path))
            .ok_or("не найдена папка библиотеки блоков library/; укажите --library")?;
        let ctx = FolderContext::open(path, &library)?;
        self.ctx = Some(ctx);
        eprintln!(
            "flp-mcp: проект {}, библиотека {}",
            path.display(),
            library.display()
        );
        Ok(json!({"project": path.display().to_string(), "library": library.display().to_string()}))
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if name == OPEN_PROJECT {
            let path = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or("нужен аргумент path — папка проекта")?;
            return self.open(Path::new(path));
        }
        let ctx = self
            .ctx
            .as_mut()
            .ok_or("проект не открыт: сначала вызовите open_project с папкой проекта")?;
        // See edits made in the editor since the last call.
        ctx.refresh().and_then(|()| flp_ai::call(ctx, name, args))
    }
}

fn open_project_tool() -> Value {
    json!({
        "name": OPEN_PROJECT,
        "description": "Открыть папку проекта FLP_next (с project.toml) или переключиться на другую. \
    Остальные команды работают с открытым проектом.",
        "inputSchema": {
            "type": "object",
            "properties": {"path": {"type": "string", "description": "Абсолютный путь к папке проекта"}},
            "required": ["path"],
        },
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let mut project = None;
    let mut library = std::env::var_os("FLP_LIBRARY").map(PathBuf::from);
    while let Some(a) = args.next() {
        if a == "--library" {
            library = args.next().map(PathBuf::from);
        } else {
            project = Some(PathBuf::from(a));
        }
    }
    let mut server = Server { ctx: None, library };
    match project {
        Some(project) => {
            if let Err(e) = server.open(&project) {
                eprintln!("ошибка: {e}");
                return ExitCode::from(2);
            }
        }
        None => eprintln!("flp-mcp: проект не открыт, ждём open_project"),
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle(&mut server, &line) {
            if writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Handles one message; returns the response line, or `None` for notifications.
fn handle(server: &mut Server, line: &str) -> Option<Value> {
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
                "serverInfo": {"name": "flp-next", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => {
            let tools: Vec<Value> = std::iter::once(open_project_tool())
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
        project.canonicalize().ok(),
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
    fn open_project_on_demand() {
        let mut s = Server {
            ctx: None,
            library: None,
        };
        assert_eq!(call(&mut s, "check", json!({}))["isError"], true);
        let bad = call(
            &mut s,
            OPEN_PROJECT,
            json!({"path": root().join("crates").to_str()}),
        );
        assert_eq!(bad["isError"], true);
        // Library is found by walking up from the project folder.
        let pump = root().join("examples/pump");
        let ok = call(&mut s, OPEN_PROJECT, json!({"path": pump.to_str()}));
        assert_eq!(ok["isError"], false);
        assert_eq!(call(&mut s, "check", json!({}))["isError"], false);
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
        assert!(tools.iter().any(|t| t["name"] == OPEN_PROJECT));
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
