//! Debugging through the master firmware (`specs/wasm-only`): the host polls the values
//! of every watched signal of each module with `out <module> <n>` and shows them like
//! the binary session does. The board keeps running its modules; nothing is flashed.

use std::io;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::session::{Command, Live};
use crate::table::{DebugTable, Value};
use crate::transport::{Connector, Transport};

const HISTORY: usize = 600;
const CONSOLE: usize = 500;
/// How often the board is asked which modules it runs.
const LIST_EVERY: Duration = Duration::from_secs(2);
/// Pause between two polls of the board.
const POLL: Duration = Duration::from_millis(150);
/// The board is considered silent after this long without an answer.
const SILENT: Duration = Duration::from_secs(3);

/// A module on the board and where its watched signals go in the debug table.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleMap {
    pub name: String,
    /// Table entry of each watched slot of the module, in the order of `output_i/f(slot, v)`;
    /// `None` for a slot the table does not show (strings).
    pub slots: Vec<Option<usize>>,
    /// CRC-32 of the module bytes; the board reports the same for what it runs.
    pub crc: u32,
}

/// Debugging session with the master firmware: same shape as [`crate::Session`].
pub struct MasterSession {
    live: Arc<Mutex<Live>>,
    tx: Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MasterSession {
    pub fn start(
        table: DebugTable,
        modules: Vec<ModuleMap>,
        connect: Connector,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let n = table.entries.len();
        let live = Arc::new(Mutex::new(Live {
            values: vec![None; n],
            texts: vec![None; table.texts.len()],
            forced: vec![None; n],
            history: vec![Vec::new(); n],
            status: "Подключение...".into(),
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let shared = live.clone();
        let thread = thread::spawn(move || run(table, modules, connect, rx, shared, wake));
        Self {
            live,
            tx,
            thread: Some(thread),
        }
    }

    pub fn snapshot(&self) -> Live {
        self.live.lock().expect("live state").clone()
    }

    pub fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MasterSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(
    table: DebugTable,
    modules: Vec<ModuleMap>,
    connect: Connector,
    rx: Receiver<Command>,
    live: Arc<Mutex<Live>>,
    wake: impl Fn(),
) {
    let started = Instant::now();
    loop {
        let mut port = match connect() {
            Ok(port) => port,
            Err(e) => {
                set_status(&live, false, format!("Нет связи с платой: {e}"));
                wake();
                if wait_or_stop(&rx, Duration::from_secs(1)) {
                    return;
                }
                continue;
            }
        };
        set_status(&live, true, "Подключено, ждём мастер-прошивку...".into());
        wake();
        match serve(&table, &modules, started, port.as_mut(), &rx, &live, &wake) {
            Ok(true) => return,
            Ok(false) => set_status(
                &live,
                false,
                "Мастер не отвечает: на плате другая прошивка? Переподключение...".into(),
            ),
            Err(e) => set_status(&live, false, format!("Связь потеряна: {e}")),
        }
        wake();
        if wait_or_stop(&rx, Duration::from_millis(500)) {
            return;
        }
    }
}

/// Polls the board until asked to stop (`true`) or until it stops answering (`false`).
fn serve(
    table: &DebugTable,
    modules: &[ModuleMap],
    started: Instant,
    port: &mut dyn Transport,
    rx: &Receiver<Command>,
    live: &Arc<Mutex<Live>>,
    wake: &impl Fn(),
) -> io::Result<bool> {
    let mut buf = [0u8; 1024];
    let mut line = Vec::new();
    // Modules asked and not yet answered, oldest first.
    let mut asked: Vec<usize> = Vec::new();
    let mut last_rx = Instant::now();
    let mut last_poll = Instant::now() - POLL;
    let mut last_list = Instant::now() - LIST_EVERY;
    let mut running: Vec<(String, u32)> = Vec::new();
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Stop) | Err(TryRecvError::Disconnected) => return Ok(true),
                Ok(command) => send_command(command, table, modules, port, live)?,
                Err(TryRecvError::Empty) => break,
            }
        }
        if last_list.elapsed() >= LIST_EVERY {
            port.write_all(
                b"list
",
            )?;
            last_list = Instant::now();
        }
        if asked.is_empty() && last_poll.elapsed() >= POLL {
            for (i, m) in modules.iter().enumerate() {
                port.write_all(format!("out {} {}\n", m.name, m.slots.len()).as_bytes())?;
                asked.push(i);
            }
            last_poll = Instant::now();
        }
        let n = port.read(&mut buf)?;
        if n == 0 {
            if last_rx.elapsed() > SILENT {
                return Ok(false);
            }
            // An unanswered request is asked again after a while.
            if !asked.is_empty() && last_poll.elapsed() > Duration::from_secs(1) {
                asked.clear();
            }
            continue;
        }
        last_rx = Instant::now();
        let mut changed = false;
        for &b in &buf[..n] {
            if b != b'\n' {
                line.push(b);
                continue;
            }
            let text = String::from_utf8_lossy(&line).trim().to_owned();
            line.clear();
            changed |= handle_line(
                &text,
                table,
                modules,
                &mut asked,
                &mut running,
                started,
                live,
            );
        }
        if changed {
            wake();
        }
    }
}

/// Handles one line of the board; `true` if the shown data changed.
fn handle_line(
    text: &str,
    table: &DebugTable,
    modules: &[ModuleMap],
    asked: &mut Vec<usize>,
    running: &mut Vec<(String, u32)>,
    started: Instant,
    live: &Arc<Mutex<Live>>,
) -> bool {
    let mut l = live.lock().expect("live state");
    // `list` answer: `<name> run|stop steps=.. restarts=.. out0=.. crc=<hex>`, then `end`.
    if let Some((head, crc)) = text.rsplit_once(" crc=") {
        let name = head.split_whitespace().next().unwrap_or_default();
        let crc = u32::from_str_radix(crc.trim(), 16).unwrap_or(0);
        running.push((name.to_owned(), crc));
        return false;
    }
    if text == "end" {
        let missing = modules
            .iter()
            .find(|m| !running.iter().any(|(n, c)| *n == m.name && *c == m.crc));
        running.clear();
        l.connected = true;
        l.schema_matches = Some(missing.is_none());
        l.status = missing.map_or(String::new(), |m| {
            format!(
                "Модуль «{}» на плате не совпадает со схемой: залейте модули (📦)",
                m.name
            )
        });
        return true;
    }
    if let Some(words) = text.strip_prefix("out") {
        let Some(&m) = asked.first() else {
            return false;
        };
        asked.remove(0);
        let t = started.elapsed().as_secs_f64();
        for (slot, word) in words.split_whitespace().enumerate() {
            let Some(Some(i)) = modules[m].slots.get(slot) else {
                continue;
            };
            let Ok(raw) = u32::from_str_radix(word, 16) else {
                continue;
            };
            let v = Value::from_raw(table.entries[*i].ty, raw);
            l.values[*i] = Some(v);
            let y = match v {
                Value::Bool(b) => f64::from(u8::from(b)),
                Value::Int(x) => f64::from(x),
                Value::Float(x) => f64::from(x),
            };
            let h = &mut l.history[*i];
            h.push((t, y));
            let excess = h.len().saturating_sub(HISTORY);
            h.drain(..excess);
        }
        l.frames += 1;
        return true;
    }
    if text == "ok" || text.is_empty() {
        return false;
    }
    if text.starts_with("err no such module") {
        // The answer to the request for the oldest unanswered module.
        let name = asked
            .first()
            .map_or(String::new(), |&m| modules[m].name.clone());
        if !asked.is_empty() {
            asked.remove(0);
        }
        l.status = format!("На плате нет модуля «{name}»: залейте модули (📦)");
        return true;
    }
    l.console.push(text.to_owned());
    let excess = l.console.len().saturating_sub(CONSOLE);
    l.console.drain(..excess);
    l.console_total += 1;
    true
}

/// Forcing and releasing: the master keeps the forced value of a module output.
fn send_command(
    command: Command,
    table: &DebugTable,
    modules: &[ModuleMap],
    port: &mut dyn Transport,
    live: &Arc<Mutex<Live>>,
) -> io::Result<()> {
    // Module and slot that show table entry `index`.
    let place = |index: usize| {
        modules.iter().find_map(|m| {
            let slot = m.slots.iter().position(|s| *s == Some(index))?;
            Some((m.name.as_str(), slot))
        })
    };
    match command {
        Command::Force { index, value } => {
            let (Some(entry), Some((name, slot))) = (table.entries.get(index), place(index)) else {
                return Ok(());
            };
            let value = value.map(|v| v.cast(entry.ty));
            live.lock().expect("live state").forced[index] = value;
            let text = match value {
                None => format!(
                    "unforce {name} {slot}
"
                ),
                Some(Value::Float(x)) => format!(
                    "force {name} {slot} f {x}
"
                ),
                Some(Value::Int(x)) => format!(
                    "force {name} {slot} i {x}
"
                ),
                Some(Value::Bool(b)) => format!(
                    "force {name} {slot} i {}
",
                    u8::from(b)
                ),
            };
            port.write_all(text.as_bytes())
        }
        Command::ReleaseAll => {
            live.lock().expect("live state").forced.fill(None);
            for m in modules {
                port.write_all(
                    format!(
                        "unforce {} all
",
                        m.name
                    )
                    .as_bytes(),
                )?;
            }
            Ok(())
        }
        // The master has no setpoints, cycle statistics or console input yet.
        _ => Ok(()),
    }
}

fn set_status(live: &Arc<Mutex<Live>>, connected: bool, status: String) {
    let mut l = live.lock().expect("live state");
    l.connected = connected;
    l.status = status;
    if !connected {
        l.schema_matches = None;
        l.stats = None;
    }
}

/// Sleeps up to `d`; returns true if the session was asked to stop meanwhile.
fn wait_or_stop(rx: &Receiver<Command>, d: Duration) -> bool {
    match rx.recv_timeout(d) {
        Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => true,
        Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flp_core::{PortRef, SignalType};

    use crate::table::{Entry, Source};

    fn table() -> DebugTable {
        let entry = |name: &str, ty| Entry {
            source: Source::Output(PortRef::new(name, "out")),
            ty,
            storage: name.into(),
            cpp_type: "",
        };
        DebugTable {
            entries: vec![entry("a", SignalType::Int), entry("b", SignalType::Float)],
            texts: Vec::new(),
        }
    }

    #[test]
    fn answer_goes_to_table_entries() {
        let table = table();
        let modules = vec![ModuleMap {
            name: "sheet1".into(),
            slots: vec![Some(1), None, Some(0)],
            crc: 5,
        }];
        let live = Arc::new(Mutex::new(Live {
            values: vec![None; 2],
            forced: vec![None; 2],
            history: vec![Vec::new(); 2],
            ..Default::default()
        }));
        let mut asked = vec![0];
        let two = format!("out {:08x} 00000000 {:08x}", 1.5f32.to_bits(), 7);
        assert!(handle_line(
            &two,
            &table,
            &modules,
            &mut asked,
            &mut Vec::new(),
            Instant::now(),
            &live
        ));
        let l = live.lock().unwrap();
        assert_eq!(l.values[1], Some(Value::Float(1.5)));
        assert_eq!(l.values[0], Some(Value::Int(7)));
        assert!(asked.is_empty());
    }

    #[test]
    fn missing_module_is_reported() {
        let table = table();
        let modules = vec![ModuleMap {
            name: "sheet1".into(),
            slots: vec![Some(0)],
            crc: 5,
        }];
        let live = Arc::new(Mutex::new(Live::default()));
        let mut asked = vec![0];
        handle_line(
            "err no such module",
            &table,
            &modules,
            &mut asked,
            &mut Vec::new(),
            Instant::now(),
            &live,
        );
        let l = live.lock().unwrap();
        assert!(l.status.contains("sheet1"));
    }

    #[test]
    fn list_compares_module_crc() {
        let table = table();
        let modules = vec![ModuleMap {
            name: "sheet1".into(),
            slots: vec![Some(0)],
            crc: 0xabc,
        }];
        let live = Arc::new(Mutex::new(Live::default()));
        let run = |crc: &str| {
            let mut running = Vec::new();
            let mut asked = Vec::new();
            for line in [
                format!("sheet1 run steps=1 restarts=0 out0=0 crc={crc}"),
                "end".into(),
            ] {
                handle_line(
                    &line,
                    &table,
                    &modules,
                    &mut asked,
                    &mut running,
                    Instant::now(),
                    &live,
                );
            }
            live.lock().unwrap().schema_matches
        };
        assert_eq!(run("00000abc"), Some(true));
        assert_eq!(run("00000001"), Some(false));
    }
}
