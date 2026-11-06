//! Reads the project archive kept in the board's flash (`GET_PROJECT`).

use std::time::{Duration, Instant};

use crate::protocol::{self, Event, Parser};
use crate::transport::Transport;

/// How long one piece may take before the request is repeated.
const RETRY_AFTER: Duration = Duration::from_millis(400);
/// Repeats of a request before giving up; the first one waits longer, because opening
/// the port may reboot the board.
const RETRIES: u32 = 6;
const FIRST_RETRIES: u32 = 14;

#[derive(Debug, thiserror::Error)]
pub enum PullError {
    #[error("ошибка порта: {0}")]
    Io(#[from] std::io::Error),
    #[error("плата не отвечает: нужна прошивка с модулем отладки (собранная с --debug)")]
    Silent,
    #[error("отладка защищена паролем: укажите пароль проекта")]
    NeedPassword,
    #[error("неверный пароль проекта")]
    WrongPassword,
    #[error("в прошивке нет проекта")]
    NoProject,
    #[error("прошивка использует старый протокол отладки: перезалейте её")]
    OldProtocol,
}

/// What was read from the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pulled {
    /// The archive exactly as stored (possibly encrypted, see `crab_core::protect`).
    pub data: Vec<u8>,
    /// Fingerprint of the signal table the firmware was built for.
    pub fingerprint: Option<u32>,
    /// `Project::content_hash` the firmware was built from (not shown while locked).
    pub project_hash: Option<[u8; 8]>,
}

/// Reads the whole project piece by piece. `progress` gets (bytes read, total).
/// `password` signs in to firmware locked with the project password.
pub fn read_project(
    port: &mut dyn Transport,
    password: &str,
    progress: impl FnMut(usize, usize),
) -> Result<Pulled, PullError> {
    let result = read_pieces(port, password, progress);
    // Sign out: a password-locked board closes at once.
    let _ = port.write_all(&protocol::encode(protocol::AUTH_REQ, &[]));
    result
}

fn read_pieces(
    port: &mut dyn Transport,
    password: &str,
    mut progress: impl FnMut(usize, usize),
) -> Result<Pulled, PullError> {
    let mut parser = Parser::default();
    let mut fingerprint = None;
    let mut project_hash = None;
    let mut data: Vec<u8> = Vec::new();
    let mut answered = false;
    let mut key: Option<([u8; 16], [u8; 32])> = None;
    loop {
        let offset = data.len();
        let mut piece = None;
        for _ in 0..(if answered { RETRIES } else { FIRST_RETRIES }) {
            port.write_all(&protocol::encode(protocol::HELLO_REQ, &[]))?;
            port.write_all(&protocol::encode(
                protocol::GET_PROJECT,
                &(offset as u32).to_le_bytes(),
            ))?;
            let asked = Instant::now();
            let mut buf = [0u8; 512];
            while piece.is_none() && asked.elapsed() < RETRY_AFTER {
                let n = port.read(&mut buf)?;
                for event in parser.feed(&buf[..n]) {
                    let Event::Frame { kind, payload } = event else {
                        continue;
                    };
                    match kind {
                        protocol::HELLO if payload.len() >= 6 => {
                            fingerprint = Some(u32::from_le_bytes(
                                payload[..4].try_into().expect("4 bytes"),
                            ));
                            project_hash = payload
                                .get(10..18)
                                .map(|h| <[u8; 8]>::try_from(h).expect("8 bytes"));
                            if payload.get(6) != Some(&protocol::VERSION) {
                                return Err(PullError::OldProtocol);
                            }
                            if payload
                                .get(9)
                                .is_some_and(|f| f & protocol::HELLO_LOCKED != 0)
                            {
                                if password.is_empty() {
                                    return Err(PullError::NeedPassword);
                                }
                                port.write_all(&protocol::encode(protocol::AUTH_REQ, &[]))?;
                            }
                        }
                        protocol::AUTH_CHALLENGE if payload.len() >= 32 => {
                            let mut salt = [0u8; 16];
                            salt.copy_from_slice(&payload[..16]);
                            let k = match key {
                                Some((s, k)) if s == salt => k,
                                _ => {
                                    let k = crab_core::protect::auth_key(password, &salt);
                                    key = Some((salt, k));
                                    k
                                }
                            };
                            let proof = crab_core::protect::auth_proof(&k, &payload[16..32]);
                            port.write_all(&protocol::encode(protocol::AUTH, &proof))?;
                        }
                        protocol::AUTH_FAIL => return Err(PullError::WrongPassword),
                        protocol::PROJECT if payload.len() >= 8 => {
                            let t = u32::from_le_bytes(payload[..4].try_into().expect("4 bytes"));
                            let o = u32::from_le_bytes(payload[4..8].try_into().expect("4 bytes"));
                            if o as usize == offset {
                                piece = Some((t as usize, payload[8..].to_vec()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            if piece.is_some() {
                break;
            }
        }
        let Some((t, bytes)) = piece else {
            return Err(PullError::Silent);
        };
        answered = true;
        if t == 0 {
            return Err(PullError::NoProject);
        }
        if bytes.is_empty() {
            return Err(PullError::Silent);
        }
        data.extend_from_slice(&bytes);
        progress(data.len().min(t), t);
        if data.len() >= t {
            data.truncate(t);
            return Ok(Pulled {
                data,
                fingerprint,
                project_hash,
            });
        }
    }
}
