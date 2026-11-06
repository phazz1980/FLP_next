//! Reading the project from an emulated board.

use std::collections::VecDeque;
use std::io;

use crab_debug::protocol::{self, Event, Parser};
use crab_debug::{read_project, PullError, Transport};

/// Answers `GET_PROJECT` the way the generated firmware does.
struct Board {
    project: Vec<u8>,
    version: u8,
    /// Drops the first answer to every request, as a noisy cable would.
    lossy: bool,
    dropped: bool,
    parser: Parser,
    out: VecDeque<u8>,
    /// Password gate: the access key and whether the host has signed in.
    gate: Option<([u8; 16], [u8; 32])>,
    authed: bool,
    nonce: [u8; 16],
}

impl Board {
    fn new(project: Vec<u8>) -> Self {
        Self {
            project,
            version: protocol::VERSION,
            lossy: false,
            dropped: false,
            parser: Parser::default(),
            out: VecDeque::new(),
            gate: None,
            authed: false,
            nonce: [9; 16],
        }
    }

    /// A board locked with `password`, as the generated firmware does it.
    fn locked(project: Vec<u8>, password: &str) -> Self {
        let mut board = Self::new(project);
        let salt = [3u8; 16];
        board.gate = Some((salt, crab_core::protect::auth_key(password, &salt)));
        board
    }
}

impl Transport for Board {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.out.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.out.pop_front().unwrap();
        }
        Ok(n)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        for event in self.parser.feed(data) {
            let Event::Frame { kind, payload } = event else {
                continue;
            };
            if let Some((salt, key)) = self.gate {
                if kind == protocol::AUTH_REQ {
                    let mut p = salt.to_vec();
                    p.extend_from_slice(&self.nonce);
                    self.out
                        .extend(protocol::encode(protocol::AUTH_CHALLENGE, &p));
                    continue;
                }
                if kind == protocol::AUTH {
                    let right = crab_core::protect::auth_proof(&key, &self.nonce);
                    let (frame, ok) = if payload == right {
                        (protocol::AUTH_OK, true)
                    } else {
                        (protocol::AUTH_FAIL, false)
                    };
                    self.authed = ok;
                    self.out.extend(protocol::encode(frame, &[]));
                    continue;
                }
                if !self.authed && kind != protocol::HELLO_REQ {
                    continue;
                }
            }
            match kind {
                protocol::HELLO_REQ => {
                    let mut p = 0xDEAD_BEEFu32.to_le_bytes().to_vec();
                    let locked = self.gate.is_some() && !self.authed;
                    p.extend_from_slice(&[0, 0, self.version, 0, 0, u8::from(locked)]);
                    self.out.extend(protocol::encode(protocol::HELLO, &p));
                }
                protocol::GET_PROJECT => {
                    if self.lossy && !self.dropped {
                        self.dropped = true;
                        continue;
                    }
                    self.dropped = false;
                    let off = u32::from_le_bytes(payload[..4].try_into().unwrap()) as usize;
                    let end = (off + protocol::PROJECT_CHUNK).min(self.project.len());
                    let mut p = (self.project.len() as u32).to_le_bytes().to_vec();
                    p.extend_from_slice(&(off as u32).to_le_bytes());
                    p.extend_from_slice(&self.project[off.min(end)..end]);
                    self.out.extend(protocol::encode(protocol::PROJECT, &p));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn sample(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 % 251) as u8).collect()
}

#[test]
fn reads_in_pieces() {
    let project = sample(1000);
    let mut steps = Vec::new();
    let pulled = read_project(&mut Board::new(project.clone()), "", |got, total| {
        steps.push((got, total));
    })
    .unwrap();
    assert_eq!(pulled.data, project);
    assert_eq!(pulled.fingerprint, Some(0xDEAD_BEEF));
    assert_eq!(steps.last(), Some(&(1000, 1000)));
    assert_eq!(steps.len(), 5);
}

#[test]
fn repeats_a_lost_request() {
    let project = sample(300);
    let mut board = Board::new(project.clone());
    board.lossy = true;
    assert_eq!(
        read_project(&mut board, "", |_, _| {}).unwrap().data,
        project
    );
}

#[test]
fn board_without_project() {
    let err = read_project(&mut Board::new(Vec::new()), "", |_, _| {}).unwrap_err();
    assert!(matches!(err, PullError::NoProject), "{err}");
}

#[test]
fn old_protocol_is_reported() {
    let mut board = Board::new(sample(10));
    board.version = 2;
    let err = read_project(&mut board, "", |_, _| {}).unwrap_err();
    assert!(matches!(err, PullError::OldProtocol), "{err}");
}

#[test]
fn locked_board_opens_with_the_right_password() {
    let project = sample(300);
    let mut board = Board::locked(project.clone(), "секрет");
    let pulled = read_project(&mut board, "секрет", |_, _| {}).unwrap();
    assert_eq!(pulled.data, project);
}

#[test]
fn locked_board_needs_a_password() {
    let mut board = Board::locked(sample(10), "секрет");
    let err = read_project(&mut board, "", |_, _| {}).unwrap_err();
    assert!(matches!(err, PullError::NeedPassword), "{err}");
}

#[test]
fn locked_board_refuses_a_wrong_password() {
    let mut board = Board::locked(sample(10), "секрет");
    let err = read_project(&mut board, "другой", |_, _| {}).unwrap_err();
    assert!(matches!(err, PullError::WrongPassword), "{err}");
}
