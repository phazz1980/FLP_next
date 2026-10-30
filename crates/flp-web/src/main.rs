//! `crablogic-web` — the web constructor of the ESP32 web interface (`/`, also `/designer`).
//! A tiny local HTTP server (std only): it serves the page and forwards `POST /api/<command>` to the same command set
//! that the AI chat and the MCP server use, so every edit is validated and saved
//! to the project file exactly as there.
//!
//! Usage: `crablogic-web <project> [--library <dir>] [--port <n>]`; open
//! `http://127.0.0.1:<port>/` (default 8080). Listens on the loopback only.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use flp_ai::folder::FolderContext;
use serde_json::{json, Value};

const DESIGNER: &str = include_str!("designer.html");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PORT: u16 = 8080;
/// Largest request body accepted (commands are small JSON objects).
const MAX_BODY: usize = 1 << 20;
/// The page pings the server every few seconds; with no request for this long the
/// server exits, so it never outlives the browser tab (or the editor that started it).
const IDLE_LIMIT: Duration = Duration::from_secs(300);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("crablogic-web: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut project: Option<PathBuf> = None;
    let mut library: Option<PathBuf> = std::env::var_os("FLP_LIBRARY").map(PathBuf::from);
    let mut port = DEFAULT_PORT;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--library" => library = Some(args.next().ok_or("после --library нужна папка")?.into()),
            "--port" => {
                port = args
                    .next()
                    .and_then(|p| p.parse().ok())
                    .ok_or("после --port нужен номер порта")?
            }
            _ => project = Some(a.into()),
        }
    }
    let project =
        project.ok_or("использование: crablogic-web <проект.crab> [--library папка] [--port N]")?;
    let library = library
        .or_else(|| find_library(&project))
        .ok_or("не найдена папка библиотеки блоков library/; укажите --library")?;
    let mut ctx = FolderContext::open(&project, &library)?;

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("не удалось занять порт {port}: {e}"))?;
    println!(
        "Конструктор веб-интерфейса: http://127.0.0.1:{port}/  (проект {})",
        project.display()
    );
    let started = Instant::now();
    let last_seen = std::sync::Arc::new(AtomicU64::new(0));
    let watch = last_seen.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(10));
        let idle = started
            .elapsed()
            .saturating_sub(Duration::from_secs(watch.load(Ordering::Relaxed)));
        if idle > IDLE_LIMIT {
            println!("crablogic-web: нет запросов {} с, выход.", idle.as_secs());
            std::process::exit(0);
        }
    });
    for stream in listener.incoming().flatten() {
        last_seen.store(started.elapsed().as_secs(), Ordering::Relaxed);
        // One user: requests are handled one at a time, edits never race.
        if let Err(e) = handle(stream, &mut ctx, &library, port) {
            eprintln!("crablogic-web: {e}");
        }
    }
    Ok(())
}

struct Request {
    method: String,
    path: String,
    host: String,
    content_type: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or("пустой запрос")?.to_owned();
    let path = parts.next().ok_or("нет пути")?.to_owned();
    let (mut host, mut content_type, mut length) = (String::new(), String::new(), 0usize);
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).map_err(|e| e.to_string())?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let v = v.trim();
            match k.to_ascii_lowercase().as_str() {
                "host" => host = v.to_owned(),
                "content-type" => content_type = v.to_ascii_lowercase(),
                "content-length" => length = v.parse().map_err(|_| "плохой Content-Length")?,
                _ => {}
            }
        }
    }
    if length > MAX_BODY {
        return Err("слишком большой запрос".into());
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Request {
        method,
        path,
        host,
        content_type,
        body,
    })
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn handle(
    mut stream: TcpStream,
    ctx: &mut FolderContext,
    library: &Path,
    port: u16,
) -> Result<(), String> {
    let req = read_request(&stream)?;
    // Only the loopback names: a foreign page cannot reach the editor via DNS rebinding.
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !local.contains(&req.host) {
        respond(&mut stream, "403 Forbidden", "text/plain", b"forbidden");
        return Ok(());
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/" | "/designer") => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            DESIGNER.replace("{{VERSION}}", VERSION).as_bytes(),
        ),
        ("GET", "/ui_page.html") => match device_page(library) {
            Ok(page) => respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                page.as_bytes(),
            ),
            Err(e) => respond(
                &mut stream,
                "500 Internal Server Error",
                "text/plain",
                e.as_bytes(),
            ),
        },
        ("POST", path) if path.starts_with("/api/") => {
            // A plain cross-site form cannot send JSON, so this blocks forged posts.
            if !req.content_type.starts_with("application/json") {
                respond(
                    &mut stream,
                    "415 Unsupported Media Type",
                    "text/plain",
                    b"json only",
                );
                return Ok(());
            }
            let args: Value = if req.body.is_empty() {
                json!({})
            } else {
                serde_json::from_slice(&req.body).map_err(|e| format!("плохой JSON: {e}"))?
            };
            let answer = match ctx
                .refresh()
                .and_then(|()| flp_ai::call(ctx, &path["/api/".len()..], &args))
            {
                Ok(v) => json!({"ok": true, "result": v}),
                Err(e) => json!({"ok": false, "error": e}),
            };
            respond(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                answer.to_string().as_bytes(),
            );
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
    Ok(())
}

/// The page the device serves at `/`: it is the raw string in the `web_ui` block template,
/// so the preview in the constructor is always the page that goes into the firmware.
fn device_page(library: &Path) -> Result<String, String> {
    const OPEN: &str = "R\"FLPUI(";
    const CLOSE: &str = ")FLPUI\"";
    let path = library.join("web_ui").join("esp32.cpp.tmpl");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("не прочитать {}: {e}", path.display()))?;
    let start = text.find(OPEN).ok_or("в шаблоне web_ui нет страницы")? + OPEN.len();
    let len = text[start..]
        .find(CLOSE)
        .ok_or("страница в шаблоне web_ui не закрыта")?;
    Ok(text[start..start + len].to_owned())
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

    #[test]
    fn preview_page_is_the_one_from_the_block_template() {
        let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
        let page = device_page(&library).unwrap();
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.contains("/ui/layout") && page.contains("/ui/state"));
    }
}
