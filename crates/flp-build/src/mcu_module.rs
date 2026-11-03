//! Uploading one `.wasm` module to the master firmware over a serial link.
//!
//! Protocol (lines end with `\n`): `put <name> <len> <crc32 hex> <allow,list|->`, the master
//! answers `ready`, then `<len>` raw bytes follow; the master answers `swap_us=…` and `ok`,
//! or `err …`. The module is stored on the board and (re)started without a reset.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

/// Longest module name the master accepts.
pub const MAX_NAME: usize = 15;
/// How long to wait for any answer of the master.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a module could not be uploaded.
#[derive(Debug, thiserror::Error)]
pub enum ModuleUploadError {
    #[error("имя модуля «{0}» не подходит: до {MAX_NAME} символов, только a-z, 0-9 и «_»")]
    BadName(String),
    #[error("в списке прав недопустимое имя «{0}»")]
    BadAllow(String),
    #[error("файл не похож на WASM-модуль (нет заголовка \\0asm)")]
    NotWasm,
    #[error("ошибка связи с платой: {0}")]
    Io(#[from] io::Error),
    #[error("плата не ответила за {} с", ANSWER_TIMEOUT.as_secs())]
    Timeout,
    #[error("мастер-прошивка не отвечает: на плате другая прошивка, не тот порт или плата ещё загружается")]
    NoMaster,
    #[error("плата отклонила модуль: {0}")]
    Rejected(String),
}

/// What the master reported about a stored module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUploaded {
    /// Time the master spent storing and restarting the module, microseconds.
    pub swap_us: Option<u32>,
}

/// CRC-32 (IEEE 802.3), as the master computes it.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Sends `wasm` to the master as module `name` with the hardware rights in `allow`.
pub fn upload_module<P: Read + Write>(
    port: &mut P,
    name: &str,
    wasm: &[u8],
    allow: &[String],
) -> Result<ModuleUploaded, ModuleUploadError> {
    let valid_ident = |s: &str| {
        !s.is_empty()
            && s.len() <= MAX_NAME
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    if !valid_ident(name) {
        return Err(ModuleUploadError::BadName(name.to_owned()));
    }
    if let Some(bad) = allow.iter().find(|a| !valid_ident(a)) {
        return Err(ModuleUploadError::BadAllow(bad.clone()));
    }
    if !wasm.starts_with(b"\0asm") {
        return Err(ModuleUploadError::NotWasm);
    }
    let rights = if allow.is_empty() {
        "-".to_owned()
    } else {
        allow.join(",")
    };
    // The board may have printed boot messages or answers to earlier commands.
    drain(port)?;
    let header = format!("put {name} {} {:08x} {rights}\n", wasm.len(), crc32(wasm));
    port.write_all(header.as_bytes())?;
    port.flush()?;
    loop {
        let line = read_line(port)?;
        if line == "ready" {
            break;
        }
        if let Some(reason) = line.strip_prefix("err") {
            return Err(ModuleUploadError::Rejected(reason.trim().to_owned()));
        }
    }
    port.write_all(wasm)?;
    port.flush()?;
    let mut swap_us = None;
    loop {
        let line = read_line(port)?;
        if let Some(value) = line.strip_prefix("swap_us=") {
            swap_us = value.trim().parse().ok();
        } else if line == "ok" {
            return Ok(ModuleUploaded { swap_us });
        } else if let Some(reason) = line.strip_prefix("err") {
            return Err(ModuleUploadError::Rejected(reason.trim().to_owned()));
        }
    }
}

/// Waits until the master answers: opening the port can reset a board with native USB
/// (ESP32-C3), and a command sent while it boots is lost. Pings with `stats` until the
/// answer ends with `end`.
pub fn wait_for_master<P: Read + Write>(
    port: &mut P,
    within: Duration,
) -> Result<(), ModuleUploadError> {
    const PING_ANSWER: Duration = Duration::from_millis(700);
    let started = Instant::now();
    while started.elapsed() < within {
        port.write_all(b"stats\n")?;
        port.flush()?;
        let ping = Instant::now();
        while ping.elapsed() < PING_ANSWER {
            match read_line_within(port, PING_ANSWER) {
                Ok(line) if line == "end" => return Ok(()),
                Ok(_) | Err(ModuleUploadError::Timeout) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Err(ModuleUploadError::NoMaster)
}

/// Reads and drops whatever is waiting on the port. Stops after a moment even if the
/// board keeps talking (a module that logs errors in a loop never lets the port go quiet).
fn drain<P: Read>(port: &mut P) -> Result<(), ModuleUploadError> {
    const LIMIT: Duration = Duration::from_millis(300);
    let started = Instant::now();
    let mut buf = [0u8; 256];
    while started.elapsed() < LIMIT {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if is_timeout(&e) => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn is_timeout(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

/// Reads one trimmed line; read timeouts of the port are retried until `ANSWER_TIMEOUT`.
fn read_line<P: Read>(port: &mut P) -> Result<String, ModuleUploadError> {
    read_line_within(port, ANSWER_TIMEOUT)
}

fn read_line_within<P: Read>(port: &mut P, limit: Duration) -> Result<String, ModuleUploadError> {
    let started = Instant::now();
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match port.read(&mut byte) {
            Ok(0) => return Err(ModuleUploadError::Timeout),
            Ok(_) if byte[0] == b'\n' => {
                return Ok(String::from_utf8_lossy(&line).trim().to_owned())
            }
            Ok(_) => line.push(byte[0]),
            Err(e) if is_timeout(&e) => {}
            Err(e) => return Err(e.into()),
        }
        if started.elapsed() > limit {
            return Err(ModuleUploadError::Timeout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A fake master: records what was written, answers from a script once the header is in.
    struct Fake {
        written: Vec<u8>,
        replies: VecDeque<u8>,
        ready: Vec<u8>,
        done: Vec<u8>,
        sent_ready: bool,
    }

    impl Fake {
        fn new(ready: &str, done: &str) -> Self {
            Fake {
                written: Vec::new(),
                replies: VecDeque::new(),
                ready: ready.as_bytes().to_vec(),
                done: done.as_bytes().to_vec(),
                sent_ready: false,
            }
        }
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.replies.pop_front() {
                Some(b) => {
                    buf[0] = b;
                    Ok(1)
                }
                None => Err(io::ErrorKind::TimedOut.into()),
            }
        }
    }

    impl Write for Fake {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(data);
            if !self.sent_ready && self.written.ends_with(b"\n") {
                self.sent_ready = true;
                self.replies.extend(self.ready.clone());
            } else if self.sent_ready && data.starts_with(b"\0asm") {
                self.replies.extend(self.done.clone());
            }
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const WASM: &[u8] = b"\0asm\x01\0\0\0";

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn uploads_and_reports_swap_time() {
        let mut port = Fake::new("ready\n", "swap_us=9274\nok\n");
        let allow = vec!["gpio_write".to_owned(), "millis".to_owned()];
        let done = upload_module(&mut port, "blink", WASM, &allow).unwrap();
        assert_eq!(done.swap_us, Some(9274));
        let header = format!("put blink 8 {:08x} gpio_write,millis\n", crc32(WASM));
        assert!(port.written.starts_with(header.as_bytes()));
        assert!(port.written.ends_with(WASM));
    }

    #[test]
    fn no_rights_are_sent_as_dash() {
        let mut port = Fake::new("ready\n", "ok\n");
        upload_module(&mut port, "hang", WASM, &[]).unwrap();
        assert!(String::from_utf8_lossy(&port.written).contains(" -\n"));
    }

    #[test]
    fn board_error_after_data_is_reported() {
        let mut port = Fake::new("ready\n", "err crc/len\n");
        let e = upload_module(&mut port, "blink", WASM, &[]).unwrap_err();
        assert!(matches!(e, ModuleUploadError::Rejected(r) if r == "crc/len"));
    }

    #[test]
    fn board_error_before_data_is_reported() {
        let mut port = Fake::new("err no memory\n", "");
        let e = upload_module(&mut port, "blink", WASM, &[]).unwrap_err();
        assert!(matches!(e, ModuleUploadError::Rejected(r) if r == "no memory"));
        assert!(!port.written.ends_with(WASM));
    }

    #[test]
    fn rejects_bad_input_without_touching_the_port() {
        let mut port = Fake::new("ready\n", "ok\n");
        assert!(matches!(
            upload_module(&mut port, "Bad Name", WASM, &[]),
            Err(ModuleUploadError::BadName(_))
        ));
        assert!(matches!(
            upload_module(&mut port, "m", b"hello", &[]),
            Err(ModuleUploadError::NotWasm)
        ));
        assert!(matches!(
            upload_module(&mut port, "m", WASM, &["a b".to_owned()]),
            Err(ModuleUploadError::BadAllow(_))
        ));
        assert!(port.written.is_empty());
    }
}
