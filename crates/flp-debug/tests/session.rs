//! Session against an emulated board that speaks the firmware side of the protocol.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use flp_core::{PortRef, SignalType};
use flp_debug::protocol::{self, Event, Parser};
use flp_debug::{Command, DebugTable, Entry, Session, Source, Transport, Value};

#[derive(Default)]
struct Board {
    fingerprint: u32,
    values: Vec<u32>,
    forced: Vec<Option<u32>>,
    out: VecDeque<u8>,
    parser: Parser,
    last_values: Option<Instant>,
}

impl Board {
    fn handle(&mut self, bytes: &[u8]) {
        for event in self.parser.feed(bytes) {
            let Event::Frame { kind, payload } = event else {
                continue;
            };
            match kind {
                protocol::HELLO_REQ => {
                    let mut p = self.fingerprint.to_le_bytes().to_vec();
                    p.extend_from_slice(&(self.values.len() as u16).to_le_bytes());
                    p.push(protocol::VERSION);
                    self.out.extend(protocol::encode(protocol::HELLO, &p));
                }
                protocol::FORCE => {
                    let i = usize::from(u16::from_le_bytes([payload[0], payload[1]]));
                    let raw = u32::from_le_bytes(payload[3..7].try_into().unwrap());
                    self.forced[i] = (payload[2] != 0).then_some(raw);
                }
                protocol::SET => {
                    let i = usize::from(u16::from_le_bytes([payload[0], payload[1]]));
                    self.values[i] = u32::from_le_bytes(payload[2..6].try_into().unwrap());
                }
                protocol::RELEASE_ALL => self.forced.iter_mut().for_each(|f| *f = None),
                _ => {}
            }
        }
    }

    /// Sends one scan of values every 20 ms, as the firmware does every 100 ms.
    fn tick(&mut self) {
        if self
            .last_values
            .is_some_and(|t| t.elapsed() < Duration::from_millis(20))
        {
            return;
        }
        self.last_values = Some(Instant::now());
        let current: Vec<u32> = self
            .values
            .iter()
            .zip(&self.forced)
            .map(|(v, f)| f.unwrap_or(*v))
            .collect();
        let mut p = vec![0, 0, 0, 0, current.len() as u8];
        for v in current {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.out.extend(b"debug print\n");
        self.out.extend(protocol::encode(protocol::VALUES, &p));
    }
}

struct FakePort(Arc<Mutex<Board>>);

impl Transport for FakePort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        thread::sleep(Duration::from_millis(5));
        let mut b = self.0.lock().unwrap();
        b.tick();
        let n = buf.len().min(b.out.len());
        for (dst, src) in buf.iter_mut().zip(b.out.drain(..n)) {
            *dst = src;
        }
        Ok(n)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.lock().unwrap().handle(data);
        Ok(())
    }
}

fn table() -> DebugTable {
    DebugTable {
        entries: vec![
            Entry {
                source: Source::Output(PortRef::new("button", "out")),
                ty: SignalType::Bool,
                storage: "button_out".into(),
                cpp_type: "",
            },
            Entry {
                source: Source::Var("setpoint".into()),
                ty: SignalType::Float,
                storage: "var_setpoint".into(),
                cpp_type: "",
            },
        ],
    }
}

fn start(fingerprint: u32) -> (Session, Arc<Mutex<Board>>) {
    let board = Arc::new(Mutex::new(Board {
        fingerprint,
        values: vec![1, 21.5f32.to_bits()],
        forced: vec![None, None],
        ..Default::default()
    }));
    let shared = board.clone();
    let session = Session::start(
        table(),
        Box::new(move || Ok(Box::new(FakePort(shared.clone())) as Box<dyn Transport>)),
        || {},
    );
    (session, board)
}

fn wait_for(session: &Session, what: impl Fn(&flp_debug::Live) -> bool) -> flp_debug::Live {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let live = session.snapshot();
        if what(&live) || Instant::now() > deadline {
            return live;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reads_values_and_forces() {
    let (session, board) = start(table().fingerprint());
    let live = wait_for(&session, |l| l.values.iter().all(Option::is_some));
    assert_eq!(live.schema_matches, Some(true));
    assert_eq!(
        live.values,
        [Some(Value::Bool(true)), Some(Value::Float(21.5))]
    );
    assert!(live.console.iter().any(|l| l == "debug print"));

    session.send(Command::Force {
        index: 0,
        value: Some(Value::Bool(false)),
    });
    let live = wait_for(&session, |l| l.values[0] == Some(Value::Bool(false)));
    assert_eq!(live.values[0], Some(Value::Bool(false)));
    assert_eq!(live.forced[0], Some(Value::Bool(false)));

    // A setpoint typed as an integer is cast to the float slot.
    session.send(Command::Set {
        index: 1,
        value: Value::Int(23),
    });
    let live = wait_for(&session, |l| l.values[1] == Some(Value::Float(23.0)));
    assert_eq!(live.values[1], Some(Value::Float(23.0)));

    session.stop();
    // Stopping releases forcing on the board.
    assert_eq!(board.lock().unwrap().forced, [None, None]);
}

#[test]
fn foreign_firmware_is_reported_and_ignored() {
    let (session, _board) = start(0xDEAD_BEEF);
    let live = wait_for(&session, |l| l.schema_matches.is_some());
    assert_eq!(live.schema_matches, Some(false));
    thread::sleep(Duration::from_millis(100));
    assert!(session.snapshot().values.iter().all(Option::is_none));
}
