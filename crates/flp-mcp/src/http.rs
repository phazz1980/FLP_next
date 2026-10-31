//! MCP over HTTP on localhost, served by the editor: `POST /mcp` with one JSON-RPC
//! message, the response is the JSON-RPC answer (or `202` for notifications).
//! Every request must carry the token, as `Authorization: Bearer <token>` or `?token=`.
//! The server works on the project the editor has open (shared with the editor).

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{handle, Server};

/// Port of the editor's MCP server; fixed so the client config does not change.
pub const PORT: u16 = 47821;
const MAX_BODY: usize = 4 << 20;
const TOKEN_FILE: &str = "mcp_token.txt";

/// Project the editor has open, updated by the editor, read by the server.
pub type SharedProject = Arc<Mutex<Option<PathBuf>>>;

pub struct HttpServer {
    pub addr: SocketAddr,
    pub token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HttpServer {
    /// Starts listening on `127.0.0.1:`[`PORT`] in a background thread.
    pub fn start(library: Option<PathBuf>, project: SharedProject) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, PORT))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let token = token();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (stop, token) = (stop.clone(), token.clone());
            std::thread::spawn(move || serve(listener, library, project, &token, &stop))
        };
        Ok(Self {
            addr,
            token,
            stop,
            thread: Some(thread),
        })
    }

    /// Address for the client config.
    pub fn url(&self) -> String {
        format!("http://{}/mcp", self.addr)
    }

    /// Entry for `.mcp.json` (`mcpServers.crablogic`).
    pub fn config_snippet(&self) -> String {
        format!(
            "{{\n  \"mcpServers\": {{\n    \"crablogic\": {{\n      \"type\": \"http\",\n      \"url\": \"{}\",\n      \"headers\": {{ \"Authorization\": \"Bearer {}\" }}\n    }}\n  }}\n}}",
            self.url(),
            self.token
        )
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Token kept in the settings folder so the client config survives restarts.
fn token() -> String {
    let file = flp_core::local::config_dir().map(|d| d.join(TOKEN_FILE));
    if let Some(t) = file
        .as_ref()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| t.len() >= 32)
    {
        return t;
    }
    let t = random_token();
    if let Some(f) = file {
        let _ = std::fs::create_dir_all(f.parent().unwrap_or(&f));
        let _ = std::fs::write(f, &t);
    }
    t
}

fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..4u32 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(i);
        h.write_u32(std::process::id());
        if let Ok(t) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            h.write_u128(t.as_nanos());
        }
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn serve(
    listener: TcpListener,
    library: Option<PathBuf>,
    project: SharedProject,
    token: &str,
    stop: &AtomicBool,
) {
    let mut server = Server::new(library, false);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let wanted = project.lock().ok().and_then(|p| p.clone());
                server.set_project(wanted.as_deref());
                let _ = connection(stream, &mut server, token);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

struct Request {
    method: String,
    target: String,
    auth: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> io::Result<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    let (mut auth, mut length) = (None, 0usize);
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "authorization" => auth = Some(value.to_string()),
                "content-length" => length = value.parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    if length > MAX_BODY {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method,
        target,
        auth,
        body,
    })
}

fn authorized(req: &Request, token: &str) -> bool {
    let bearer = req
        .auth
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .is_some_and(|t| t == token);
    let query = req
        .target
        .split_once('?')
        .is_some_and(|(_, q)| q.split('&').any(|kv| kv == format!("token={token}")));
    bearer || query
}

fn connection(mut stream: TcpStream, server: &mut Server, token: &str) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let req = read_request(&stream)?;
    let path = req.target.split('?').next().unwrap_or("");
    let (status, body) = if !authorized(&req, token) {
        ("401 Unauthorized", String::new())
    } else if path != "/mcp" {
        ("404 Not Found", String::new())
    } else if req.method != "POST" {
        ("405 Method Not Allowed", String::new())
    } else {
        match String::from_utf8(req.body) {
            Ok(text) => match handle(server, &text) {
                Some(answer) => ("200 OK", answer.to_string()),
                None => ("202 Accepted", String::new()),
            },
            Err(_) => ("400 Bad Request", String::new()),
        }
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(addr: SocketAddr, auth: Option<&str>, body: &str) -> String {
        let mut s = TcpStream::connect(addr).unwrap();
        let auth = auth.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
        write!(
            s,
            "POST /mcp HTTP/1.1\r\nHost: x\r\n{auth}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn token_required_and_project_followed() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let shared: SharedProject = Arc::new(Mutex::new(Some(root.join("examples/pump"))));
        let http = HttpServer::start(Some(root.join("library")), shared.clone()).unwrap();
        let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert!(post(http.addr, None, ping).starts_with("HTTP/1.1 401"));
        assert!(post(http.addr, Some("wrong"), ping).starts_with("HTTP/1.1 401"));
        assert!(post(http.addr, Some(&http.token), ping).starts_with("HTTP/1.1 200"));
        let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(post(http.addr, Some(&http.token), note).starts_with("HTTP/1.1 202"));

        let check = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"check","arguments":{}}}"#;
        assert!(post(http.addr, Some(&http.token), check).contains("\\\"order\\\""));
        *shared.lock().unwrap() = None;
        assert!(post(http.addr, Some(&http.token), check).contains("\"isError\":true"));
    }
}
