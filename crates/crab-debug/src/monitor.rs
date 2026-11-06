//! Port monitor for firmware without a debugging session: shows the text the
//! board prints and sends lines typed by the user. Debug frames, if the firmware
//! has the module, are dropped by the same parser.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::{Event, Parser};
use crate::transport::{Connector, Transport};

/// Lines kept while nobody takes them.
const PENDING: usize = 5000;
/// A line without `\n` is shown after the board has been quiet this long.
const FLUSH_AFTER: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Default)]
struct Shared {
    connected: bool,
    status: String,
    lines: Vec<String>,
}

enum Request {
    Send(Vec<u8>),
    Stop,
}

pub struct Monitor {
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Request>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Monitor {
    /// Opens the port on its own thread and reopens it after a drop (a reset or
    /// an unplugged cable). `wake` is called whenever something changes.
    pub fn start(connect: Connector, wake: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            status: "Подключение...".into(),
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let state = shared.clone();
        let thread = thread::spawn(move || run(connect, rx, state, wake));
        Self {
            shared,
            tx,
            thread: Some(thread),
        }
    }

    /// Lines received since the previous call.
    pub fn take_lines(&self) -> Vec<String> {
        std::mem::take(&mut self.shared.lock().expect("monitor state").lines)
    }

    /// Whether the port is open, and a status text for the user.
    pub fn status(&self) -> (bool, String) {
        let s = self.shared.lock().expect("monitor state");
        (s.connected, s.status.clone())
    }

    /// Writes bytes to the board; dropped while the port is closed.
    pub fn send(&self, data: Vec<u8>) {
        let _ = self.tx.send(Request::Send(data));
    }

    /// Closes the port and waits for it to be released.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(connect: Connector, rx: Receiver<Request>, shared: Arc<Mutex<Shared>>, wake: impl Fn()) {
    loop {
        match connect() {
            Ok(mut port) => {
                set_status(&shared, true, "Подключено".into());
                wake();
                match serve(port.as_mut(), &rx, &shared, &wake) {
                    Ok(()) => return,
                    Err(e) => set_status(&shared, false, format!("Связь потеряна: {e}")),
                }
            }
            Err(e) => set_status(&shared, false, format!("Порт не открывается: {e}")),
        }
        wake();
        if wait_or_stop(&rx, Duration::from_secs(1)) {
            return;
        }
    }
}

/// Reads until the editor asks to stop (`Ok`) or the port fails.
fn serve(
    port: &mut dyn Transport,
    rx: &Receiver<Request>,
    shared: &Arc<Mutex<Shared>>,
    wake: &impl Fn(),
) -> std::io::Result<()> {
    let mut parser = Parser::default();
    let mut buf = [0u8; 1024];
    let mut last_rx = Instant::now();
    loop {
        loop {
            match rx.try_recv() {
                Ok(Request::Send(data)) => port.write_all(&data)?,
                Ok(Request::Stop) | Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => break,
            }
        }
        let n = port.read(&mut buf)?;
        let mut lines = Vec::new();
        if n > 0 {
            last_rx = Instant::now();
            for event in parser.feed(&buf[..n]) {
                if let Event::Text(line) = event {
                    lines.push(line);
                }
            }
        } else if last_rx.elapsed() >= FLUSH_AFTER {
            lines.extend(parser.flush_text());
        }
        if !lines.is_empty() {
            let mut s = shared.lock().expect("monitor state");
            s.lines.extend(lines);
            let excess = s.lines.len().saturating_sub(PENDING);
            s.lines.drain(..excess);
            drop(s);
            wake();
        }
    }
}

fn set_status(shared: &Arc<Mutex<Shared>>, connected: bool, status: String) {
    let mut s = shared.lock().expect("monitor state");
    s.connected = connected;
    s.status = status;
}

/// Sleeps up to `d`; returns true if the monitor was asked to stop meanwhile.
/// Lines typed while the port is closed are dropped.
fn wait_or_stop(rx: &Receiver<Request>, d: Duration) -> bool {
    let deadline = Instant::now() + d;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
            Ok(Request::Send(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
        }
    }
}
