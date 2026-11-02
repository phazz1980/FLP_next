//! Session against an emulated board that speaks the firmware side of the protocol.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use flp_core::{PortRef, SignalType};
use flp_debug::protocol::{self, Event, Parser};
use flp_debug::{Command, DebugTable, Entry, Session, Source, TextEntry, Transport, Value};

#[derive(Default)]
struct Board {
    fingerprint: u32,
    /// Protocol version the board reports.
    version: u8,
    values: Vec<u32>,
    forced: Vec<Option<u32>>,
    /// Bytes received in `TEXT_IN` frames.
    text_in: Vec<u8>,
    out: VecDeque<u8>,
    parser: Parser,
    last_values: Option<Instant>,
    seq: u16,
    /// Password gate: salt and access key; `authed` once the host proved the password.
    gate: Option<([u8; 16], [u8; 32])>,
    authed: bool,
}

impl Board {
    fn handle(&mut self, bytes: &[u8]) {
        for event in self.parser.feed(bytes) {
            let Event::Frame { kind, payload } = event else {
                continue;
            };
            if let Some((salt, key)) = self.gate {
                match kind {
                    protocol::AUTH_REQ => {
                        let mut p = salt.to_vec();
                        p.extend_from_slice(&[5; 16]);
                        self.out
                            .extend(protocol::encode(protocol::AUTH_CHALLENGE, &p));
                        continue;
                    }
                    protocol::AUTH => {
                        let ok = payload == flp_core::protect::auth_proof(&key, &[5; 16]);
                        self.authed = ok;
                        let frame = if ok {
                            protocol::AUTH_OK
                        } else {
                            protocol::AUTH_FAIL
                        };
                        self.out.extend(protocol::encode(frame, &[]));
                        if ok {
                            // The firmware repeats `HELLO`, now unlocked.
                            self.handle(&protocol::encode(protocol::HELLO_REQ, &[]));
                        }
                        continue;
                    }
                    protocol::HELLO_REQ => {}
                    _ if !self.authed => continue,
                    _ => {}
                }
            }
            match kind {
                protocol::HELLO_REQ => {
                    let mut p = self.fingerprint.to_le_bytes().to_vec();
                    p.extend_from_slice(&(self.values.len() as u16).to_le_bytes());
                    p.push(self.version);
                    p.extend_from_slice(&(table().image_layout().size as u16).to_le_bytes());
                    p.push(u8::from(self.gate.is_some() && !self.authed));
                    // Zeros while locked, like the firmware.
                    let hidden = self.gate.is_some() && !self.authed;
                    p.extend_from_slice(&if hidden { [0; 8] } else { HASH });
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
                protocol::TEXT_IN => self.text_in.extend_from_slice(&payload),
                _ => {}
            }
        }
    }

    /// Sends one scan of values every 20 ms, as the firmware does every 100 ms.
    fn tick(&mut self) {
        if self.gate.is_some() && !self.authed {
            return;
        }
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
        // The image as the firmware lays it out; sent in two pieces to test assembly.
        let layout = table().image_layout();
        let mut image = vec![0u8; layout.size];
        for (v, slot) in current.iter().zip(&layout.slots) {
            let bytes = v.to_le_bytes();
            image[slot.offset..slot.offset + slot.kind.size()]
                .copy_from_slice(&bytes[..slot.kind.size()]);
        }
        self.seq = self.seq.wrapping_add(1);
        let cut = image.len() / 2;
        self.out.extend(b"debug print\n");
        for (offset, part) in [(0, &image[..cut]), (cut, &image[cut..])] {
            let mut p = self.seq.to_le_bytes().to_vec();
            p.extend_from_slice(&(offset as u16).to_le_bytes());
            p.extend_from_slice(part);
            self.out.extend(protocol::encode(protocol::IMAGE, &p));
        }
        let mut text = vec![0, 0];
        text.extend_from_slice("P=2.5 бар".as_bytes());
        self.out.extend(protocol::encode(protocol::TEXT, &text));
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

const HASH: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

fn table() -> DebugTable {
    DebugTable {
        texts: vec![TextEntry {
            source: Source::Var("message".into()),
            storage: "var_message".into(),
        }],
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
        version: protocol::VERSION,
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
    let live = wait_for(&session, |l| l.texts[0].is_some());
    assert_eq!(live.texts[0].as_deref(), Some("P=2.5 бар"));

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

#[test]
fn console_after_counts_dropped_lines() {
    // The buffer holds the last 3 of 10 lines; the editor has shown 8.
    let live = flp_debug::Live {
        console: vec!["8".into(), "9".into(), "10".into()],
        console_total: 10,
        ..Default::default()
    };
    assert_eq!(live.console_after(8), ["9", "10"]);
    assert_eq!(live.console_after(10), [] as [String; 0]);
    // Seen fewer than were dropped: everything kept is new.
    assert_eq!(live.console_after(2).len(), 3);
}

#[test]
fn console_input_goes_in_chunks() {
    let (session, board) = start(table().fingerprint());
    wait_for(&session, |l| l.schema_matches == Some(true));
    let long: Vec<u8> = (0..450).map(|i| b'a' + (i % 26) as u8).collect();
    session.send(Command::TextIn(long.clone()));
    let deadline = Instant::now() + Duration::from_secs(3);
    while board.lock().unwrap().text_in.len() < long.len() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(board.lock().unwrap().text_in, long);
}

#[test]
fn old_protocol_is_reported() {
    let board = Arc::new(Mutex::new(Board {
        fingerprint: table().fingerprint(),
        version: 2,
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
    let live = wait_for(&session, |l| l.schema_matches.is_some());
    assert_eq!(live.schema_matches, Some(false));
    assert!(live.status.contains("старый протокол"), "{}", live.status);
    thread::sleep(Duration::from_millis(100));
    assert!(session.snapshot().values.iter().all(Option::is_none));
}

fn start_locked(board_password: &str, host_password: &str) -> Session {
    let salt = [4u8; 16];
    let board = Arc::new(Mutex::new(Board {
        fingerprint: table().fingerprint(),
        version: protocol::VERSION,
        values: vec![1, 21.5f32.to_bits()],
        forced: vec![None, None],
        gate: Some((salt, flp_core::protect::auth_key(board_password, &salt))),
        ..Default::default()
    }));
    Session::start_with(
        table(),
        Box::new(move || Ok(Box::new(FakePort(board.clone())) as Box<dyn Transport>)),
        host_password.to_owned(),
        || {},
    )
}

#[test]
fn locked_board_opens_with_the_password() {
    let session = start_locked("секрет", "секрет");
    let live = wait_for(&session, |l| l.values.iter().all(Option::is_some));
    assert_eq!(live.schema_matches, Some(true));
    assert!(!live.needs_password);
    assert_eq!(live.project_hash, Some(HASH));
    assert_eq!(live.values[0], Some(Value::Bool(true)));
}

#[test]
fn locked_board_waits_for_a_password_and_takes_it_later() {
    let session = start_locked("секрет", "");
    let live = wait_for(&session, |l| l.needs_password);
    assert!(live.needs_password && live.values.iter().all(Option::is_none));
    assert_eq!(live.project_hash, None, "hidden until sign-in");
    assert!(live.status.contains("пароль"), "{}", live.status);
    session.send(Command::Password("секрет".into()));
    let live = wait_for(&session, |l| l.values.iter().all(Option::is_some));
    assert!(!live.needs_password);
    assert_eq!(live.schema_matches, Some(true));
}

#[test]
fn locked_board_turns_a_wrong_password_away() {
    let session = start_locked("секрет", "другой");
    let live = wait_for(&session, |l| l.status.contains("Неверный пароль"));
    assert!(live.status.contains("Неверный пароль"), "{}", live.status);
    assert!(live.values.iter().all(Option::is_none));
}
