//! Binary frames between the board and the editor.
//!
//! ```text
//! 0xAA 0x55 | type u8 | len u8 | payload (len bytes) | crc16 LE
//! ```
//!
//! CRC-16/CCITT-FALSE over `type`, `len` and the payload. Bytes outside frames are
//! ordinary `Serial.print` output and are passed through as text. Multi-byte
//! numbers are little-endian. Every value travels as 4 raw bytes: bool as 0/1,
//! int as `int32_t`, float as IEEE-754 `float`.

/// Protocol version reported in `HELLO`.
pub const VERSION: u8 = 1;
pub const SYNC: [u8; 2] = [0xAA, 0x55];
pub const MAX_PAYLOAD: usize = 255;
/// Values per `VALUES` frame: header 5 bytes + 4 bytes per value fits in 255.
pub const VALUES_PER_FRAME: usize = 60;

/// Board → host: `fingerprint u32, count u16, version u8`.
pub const HELLO: u8 = 0x01;
/// Board → host: `seq u16, start u16, n u8, n × value u32` (one scan, in chunks).
pub const VALUES: u8 = 0x02;
/// Host → board: ask for `HELLO`.
pub const HELLO_REQ: u8 = 0x10;
/// Host → board: `index u16, on u8, value u32` — force (or release) a value.
pub const FORCE: u8 = 0x11;
/// Host → board: `index u16, value u32` — write a value once (variables, setpoints).
pub const SET: u8 = 0x12;
/// Host → board: release all forced values.
pub const RELEASE_ALL: u8 = 0x13;

/// CRC-16/CCITT-FALSE: poly 0x1021, init 0xFFFF, no reflection.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Builds a frame. Panics if the payload is longer than [`MAX_PAYLOAD`].
pub fn encode(kind: u8, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() <= MAX_PAYLOAD, "payload too long");
    let mut body = Vec::with_capacity(payload.len() + 2);
    body.push(kind);
    body.push(payload.len() as u8);
    body.extend_from_slice(payload);
    let crc = crc16(&body);
    let mut frame = SYNC.to_vec();
    frame.extend_from_slice(&body);
    frame.extend_from_slice(&crc.to_le_bytes());
    frame
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Frame {
        kind: u8,
        payload: Vec<u8>,
    },
    /// A complete line of ordinary serial output.
    Text(String),
}

/// Incremental parser: feed it whatever the port returned.
#[derive(Debug, Default)]
pub struct Parser {
    buf: Vec<u8>,
    text: Vec<u8>,
    /// Frames dropped because of a bad checksum.
    pub crc_errors: u64,
}

impl Parser {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.buf.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            let Some(start) = find_sync(&self.buf) else {
                // Keep a trailing 0xAA: it may start a frame in the next chunk.
                let keep = usize::from(self.buf.last() == Some(&SYNC[0]));
                let text: Vec<u8> = self.buf.drain(..self.buf.len() - keep).collect();
                self.push_text(&text, &mut events);
                break;
            };
            let text: Vec<u8> = self.buf.drain(..start).collect();
            self.push_text(&text, &mut events);
            if self.buf.len() < 4 {
                break;
            }
            let len = usize::from(self.buf[3]);
            let total = 4 + len + 2;
            if self.buf.len() < total {
                break;
            }
            let body = &self.buf[2..4 + len];
            let crc = u16::from_le_bytes([self.buf[4 + len], self.buf[5 + len]]);
            if crc16(body) == crc {
                events.push(Event::Frame {
                    kind: body[0],
                    payload: body[2..].to_vec(),
                });
                self.buf.drain(..total);
            } else {
                // Not a real frame (or corrupted): skip the sync byte and rescan.
                self.crc_errors += 1;
                self.buf.drain(..1);
            }
        }
        events
    }

    fn push_text(&mut self, bytes: &[u8], events: &mut Vec<Event>) {
        for &b in bytes {
            match b {
                b'\n' => {
                    let line = String::from_utf8_lossy(&self.text).trim_end().to_owned();
                    events.push(Event::Text(line));
                    self.text.clear();
                }
                _ if self.text.len() < 512 => self.text.push(b),
                _ => {}
            }
        }
    }
}

fn find_sync(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == SYNC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_check_value() {
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    #[test]
    fn roundtrip_with_text_around() {
        let mut stream = b"boot ok\r\n".to_vec();
        stream.extend(encode(HELLO, &[1, 2, 3]));
        stream.extend(b"temp=21\n");
        stream.extend(encode(VALUES, &[]));
        let mut p = Parser::default();
        let events = p.feed(&stream);
        assert_eq!(
            events,
            [
                Event::Text("boot ok".into()),
                Event::Frame {
                    kind: HELLO,
                    payload: vec![1, 2, 3]
                },
                Event::Text("temp=21".into()),
                Event::Frame {
                    kind: VALUES,
                    payload: vec![]
                },
            ]
        );
    }

    #[test]
    fn frame_split_across_reads() {
        let frame = encode(SET, &[7, 0, 1, 2, 3, 4]);
        let mut p = Parser::default();
        let mut events = Vec::new();
        for chunk in frame.chunks(3) {
            events.extend(p.feed(chunk));
        }
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn corrupted_frame_is_dropped_and_parser_recovers() {
        let mut bad = encode(FORCE, &[1, 2, 3]);
        let last = bad.len() - 1;
        bad[last] ^= 0xFF;
        let mut stream = bad;
        stream.extend(encode(HELLO_REQ, &[]));
        let mut p = Parser::default();
        let events = p.feed(&stream);
        assert!(events.contains(&Event::Frame {
            kind: HELLO_REQ,
            payload: vec![]
        }));
        assert!(p.crc_errors >= 1);
    }
}
