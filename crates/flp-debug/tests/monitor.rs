//! Port monitor against a fake port: text lines, a prompt without `\n`,
//! dropped debug frames, sending and reconnecting.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use flp_debug::{protocol, Monitor, Transport};

#[derive(Default)]
struct Wire {
    to_host: VecDeque<u8>,
    from_host: Vec<u8>,
    /// The next read fails, as when the cable is pulled.
    drop_link: bool,
}

struct FakePort(Arc<Mutex<Wire>>);

impl Transport for FakePort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        thread::sleep(Duration::from_millis(5));
        let mut w = self.0.lock().unwrap();
        if std::mem::take(&mut w.drop_link) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let n = buf.len().min(w.to_host.len());
        for (dst, src) in buf.iter_mut().zip(w.to_host.drain(..n)) {
            *dst = src;
        }
        Ok(n)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.lock().unwrap().from_host.extend_from_slice(data);
        Ok(())
    }
}

fn start() -> (Monitor, Arc<Mutex<Wire>>, Arc<AtomicUsize>) {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let opened = Arc::new(AtomicUsize::new(0));
    let (w, o) = (wire.clone(), opened.clone());
    let monitor = Monitor::start(
        Box::new(move || {
            o.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakePort(w.clone())) as Box<dyn Transport>)
        }),
        || {},
    );
    (monitor, wire, opened)
}

/// Collects lines until `count` arrived or time is up.
fn lines(monitor: &Monitor, count: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut all = Vec::new();
    while all.len() < count && Instant::now() < deadline {
        all.extend(monitor.take_lines());
        thread::sleep(Duration::from_millis(10));
    }
    all
}

#[test]
fn shows_text_and_drops_frames() {
    let (monitor, wire, _) = start();
    {
        let mut w = wire.lock().unwrap();
        w.to_host.extend(b"hello\r\n");
        w.to_host
            .extend(protocol::encode(protocol::VALUES, &[0, 0, 0, 0, 0]));
        w.to_host.extend("t=21.5\nВвод> ".as_bytes());
    }
    assert_eq!(lines(&monitor, 3), ["hello", "t=21.5", "Ввод>"]);
    assert!(monitor.status().0);
}

#[test]
fn sends_typed_text() {
    let (monitor, wire, _) = start();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !monitor.status().0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    monitor.send(b"ON\r\n".to_vec());
    while wire.lock().unwrap().from_host.is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(wire.lock().unwrap().from_host, b"ON\r\n");
}

#[test]
fn reconnects_after_drop() {
    let (monitor, wire, opened) = start();
    wire.lock().unwrap().drop_link = true;
    let deadline = Instant::now() + Duration::from_secs(5);
    while opened.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(opened.load(Ordering::SeqCst) >= 2);
    wire.lock().unwrap().to_host.extend(b"again\n");
    assert_eq!(lines(&monitor, 1), ["again"]);
    monitor.stop();
}
