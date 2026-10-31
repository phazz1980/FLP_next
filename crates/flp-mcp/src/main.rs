//! `crablogic-mcp` — MCP server over stdio, one JSON-RPC message per line.
//!
//! Usage: `crablogic-mcp [project file] [--library <dir>]`. Without a project it works
//! with the project of a running editor window (`list_editors` / `select_editor`).
//! Only JSON-RPC goes to stdout; diagnostics go to stderr.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use flp_mcp::{handle, Server};

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
    let mut server = Server::new(library, project.is_none());
    match project {
        Some(project) => {
            if let Err(e) = server.open(&project) {
                eprintln!("ошибка: {e}");
                return ExitCode::from(2);
            }
        }
        None => eprintln!("crablogic-mcp: проект не назван, следую за проектом редактора"),
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
