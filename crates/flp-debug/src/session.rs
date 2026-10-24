//! Host side of a debugging session: runs on its own thread, keeps the latest
//! values and reconnects when the board drops off.

use std::io;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::{self, Event, Parser, Stats};
use crate::table::{DebugTable, Value};
use crate::transport::{Connector, Transport};

/// What the editor shows. Shared with the session thread.
#[derive(Debug, Clone, Default)]
pub struct Live {
    pub connected: bool,
    /// `Some(false)` when the board runs firmware for another schema.
    pub schema_matches: Option<bool>,
    /// Latest value per table entry.
    pub values: Vec<Option<Value>>,
    /// Forced values per table entry, as requested by the host.
    pub forced: Vec<Option<Value>>,
    /// History of each entry for charts: (seconds since start, value).
    pub history: Vec<Vec<(f64, f64)>>,
    /// Ordinary serial output of the sketch, newest last.
    pub console: Vec<String>,
    /// Problems to show to the user (connection lost, wrong schema).
    pub status: String,
    pub frames: u64,
    /// Latest cycle time report of the board.
    pub stats: Option<Stats>,
}

const HISTORY: usize = 600;
const CONSOLE: usize = 500;

#[derive(Debug, Clone)]
pub enum Command {
    Force {
        index: usize,
        value: Option<Value>,
    },
    Set {
        index: usize,
        value: Value,
    },
    ReleaseAll,
    /// Reset the cycle time maxima on the board.
    ResetStats,
    Stop,
}

pub struct Session {
    live: Arc<Mutex<Live>>,
    tx: Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Session {
    /// Starts the session. `wake` is called whenever new data arrives.
    pub fn start(table: DebugTable, connect: Connector, wake: impl Fn() + Send + 'static) -> Self {
        let n = table.entries.len();
        let live = Arc::new(Mutex::new(Live {
            values: vec![None; n],
            forced: vec![None; n],
            history: vec![Vec::new(); n],
            status: "Подключение...".into(),
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let shared = live.clone();
        let thread = thread::spawn(move || run(table, connect, rx, shared, wake));
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

impl Drop for Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(
    table: DebugTable,
    connect: Connector,
    rx: Receiver<Command>,
    live: Arc<Mutex<Live>>,
    wake: impl Fn(),
) {
    let started = Instant::now();
    let expected = table.fingerprint();
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
        set_status(&live, true, "Подключено, ждём ответа платы...".into());
        wake();
        match serve(&table, expected, started, port.as_mut(), &rx, &live, &wake) {
            Ok(Stop::Requested) => {
                // Leave the board in normal operation.
                let _ = port.write_all(&protocol::encode(protocol::RELEASE_ALL, &[]));
                return;
            }
            Ok(Stop::Silent) => {
                set_status(&live, false, "Плата не отвечает, переподключение...".into())
            }
            Err(e) => set_status(&live, false, format!("Связь потеряна: {e}")),
        }
        wake();
        if wait_or_stop(&rx, Duration::from_millis(500)) {
            return;
        }
    }
}

enum Stop {
    Requested,
    Silent,
}

fn serve(
    table: &DebugTable,
    expected: u32,
    started: Instant,
    port: &mut dyn Transport,
    rx: &Receiver<Command>,
    live: &Arc<Mutex<Live>>,
    wake: &impl Fn(),
) -> io::Result<Stop> {
    let mut parser = Parser::default();
    let mut buf = [0u8; 1024];
    let mut last_rx = Instant::now();
    let mut last_hello = Instant::now() - Duration::from_secs(10);
    let mut greeted = false;
    loop {
        // Commands from the editor.
        loop {
            match rx.try_recv() {
                Ok(Command::Stop) | Err(TryRecvError::Disconnected) => return Ok(Stop::Requested),
                Ok(cmd) => apply_command(table, port, live, cmd)?,
                Err(TryRecvError::Empty) => break,
            }
        }
        if !greeted && last_hello.elapsed() > Duration::from_millis(700) {
            port.write_all(&protocol::encode(protocol::HELLO_REQ, &[]))?;
            last_hello = Instant::now();
        }

        let n = port.read(&mut buf)?;
        if n == 0 {
            if last_rx.elapsed() > Duration::from_secs(3) {
                return Ok(Stop::Silent);
            }
            continue;
        }
        last_rx = Instant::now();
        let mut changed = false;
        for event in parser.feed(&buf[..n]) {
            let mut l = live.lock().expect("live state");
            match event {
                Event::Text(line) => {
                    l.console.push(line);
                    let excess = l.console.len().saturating_sub(CONSOLE);
                    l.console.drain(..excess);
                }
                Event::Frame { kind, payload } => {
                    l.frames += 1;
                    match kind {
                        protocol::HELLO if payload.len() >= 6 => {
                            let fp = u32::from_le_bytes(payload[0..4].try_into().expect("4 bytes"));
                            greeted = true;
                            let ok = fp == expected;
                            l.schema_matches = Some(ok);
                            l.status = if ok {
                                "Онлайн".into()
                            } else {
                                "Прошивка на плате собрана для другой схемы — перезалейте её".into()
                            };
                            // Re-apply forcing after a board reset.
                            let forced: Vec<(usize, Value)> = l
                                .forced
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| v.map(|v| (i, v)))
                                .collect();
                            drop(l);
                            if ok {
                                for (index, value) in forced {
                                    port.write_all(&force_frame(index, Some(value)))?;
                                }
                            }
                            changed = true;
                            continue;
                        }
                        protocol::STATS => {
                            if l.schema_matches == Some(true) {
                                l.stats = Stats::decode(&payload);
                            }
                        }
                        protocol::VALUES if payload.len() >= 5 => {
                            if l.schema_matches != Some(true) {
                                // Layout unknown or foreign: values would be misread.
                                continue;
                            }
                            let start = usize::from(u16::from_le_bytes([payload[2], payload[3]]));
                            let count = usize::from(payload[4]);
                            let t = started.elapsed().as_secs_f64();
                            let (chunks, _) = payload[5..].as_chunks::<4>();
                            for (k, raw) in chunks.iter().take(count).enumerate() {
                                let i = start + k;
                                let Some(entry) = table.entries.get(i) else {
                                    break;
                                };
                                let raw = u32::from_le_bytes(*raw);
                                let v = Value::from_raw(entry.ty, raw);
                                l.values[i] = Some(v);
                                let y = match v {
                                    Value::Bool(b) => f64::from(u8::from(b)),
                                    Value::Int(x) => f64::from(x),
                                    Value::Float(x) => f64::from(x),
                                };
                                let h = &mut l.history[i];
                                h.push((t, y));
                                let excess = h.len().saturating_sub(HISTORY);
                                h.drain(..excess);
                            }
                        }
                        _ => {}
                    }
                }
            }
            changed = true;
        }
        if changed {
            wake();
        }
    }
}

fn apply_command(
    table: &DebugTable,
    port: &mut dyn Transport,
    live: &Arc<Mutex<Live>>,
    cmd: Command,
) -> io::Result<()> {
    match cmd {
        Command::Force { index, value } => {
            let Some(entry) = table.entries.get(index) else {
                return Ok(());
            };
            let value = value.map(|v| v.cast(entry.ty));
            live.lock().expect("live state").forced[index] = value;
            port.write_all(&force_frame(index, value))
        }
        Command::Set { index, value } => {
            let Some(entry) = table.entries.get(index) else {
                return Ok(());
            };
            let mut payload = (index as u16).to_le_bytes().to_vec();
            payload.extend_from_slice(&value.cast(entry.ty).to_raw().to_le_bytes());
            port.write_all(&protocol::encode(protocol::SET, &payload))
        }
        Command::ReleaseAll => {
            live.lock()
                .expect("live state")
                .forced
                .iter_mut()
                .for_each(|f| *f = None);
            port.write_all(&protocol::encode(protocol::RELEASE_ALL, &[]))
        }
        Command::ResetStats => port.write_all(&protocol::encode(protocol::RESET_STATS, &[])),
        Command::Stop => Ok(()),
    }
}

fn force_frame(index: usize, value: Option<Value>) -> Vec<u8> {
    let mut payload = (index as u16).to_le_bytes().to_vec();
    payload.push(u8::from(value.is_some()));
    payload.extend_from_slice(&value.map_or(0, Value::to_raw).to_le_bytes());
    protocol::encode(protocol::FORCE, &payload)
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
