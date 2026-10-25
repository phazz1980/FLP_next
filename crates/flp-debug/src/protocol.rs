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
pub const VERSION: u8 = 2;
pub const SYNC: [u8; 2] = [0xAA, 0x55];
pub const MAX_PAYLOAD: usize = 255;
/// Values per `VALUES` frame: header 5 bytes + 4 bytes per value fits in 255.
pub const VALUES_PER_FRAME: usize = 60;

/// Board → host: `fingerprint u32, count u16, version u8`.
pub const HELLO: u8 = 0x01;
/// Board → host: `seq u16, start u16, n u8, n × value u32` (one scan, in chunks).
pub const VALUES: u8 = 0x02;
/// Board → host, every 500 ms: `cycles u32, last u32, max u32, avg u32,
/// period_max u32, free_heap u32, n u8, n × (sheet u16, last u32, max u32)`.
/// Times in microseconds; `cycles` counts scans since the last report.
pub const STATS: u8 = 0x03;
/// Board → host, when a string changes (and all of them after `HELLO_REQ`):
/// `index u16` in [`crate::DebugTable::texts`], then UTF-8 bytes (at most [`TEXT_MAX`]).
pub const TEXT: u8 = 0x04;
/// Longest string sent in `TEXT`; longer ones are cut at a character boundary.
pub const TEXT_MAX: usize = 64;
/// Sheets measured separately; `STATS` with this many still fits in 255 bytes.
pub const MAX_STAT_SHEETS: usize = 23;
/// Host → board: ask for `HELLO`.
pub const HELLO_REQ: u8 = 0x10;
/// Host → board: `index u16, on u8, value u32` — force (or release) a value.
pub const FORCE: u8 = 0x11;
/// Host → board: `index u16, value u32` — write a value once (variables, setpoints).
pub const SET: u8 = 0x12;
/// Host → board: release all forced values.
pub const RELEASE_ALL: u8 = 0x13;

/// Host → board: reset the maxima of `STATS`.
pub const RESET_STATS: u8 = 0x14;
/// Host → board: bytes typed in the debug console, for the program to read from
/// UART0 (the port itself carries frames while debugging).
pub const TEXT_IN: u8 = 0x15;
/// Bytes per `TEXT_IN` frame; longer input goes in several frames.
pub const TEXT_IN_CHUNK: usize = 200;

/// Cycle time figures of the board, decoded from `STATS`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Scans since the previous report.
    pub cycles: u32,
    /// Time of the logic in the last scan, µs.
    pub last_us: u32,
    /// Longest logic time since start or reset, µs.
    pub max_us: u32,
    /// Average logic time since the previous report, µs.
    pub avg_us: u32,
    /// Longest time between two `loop()` starts (logic + debugging + system), µs.
    pub period_max_us: u32,
    pub free_heap: u32,
    pub sheets: Vec<SheetStats>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetStats {
    pub sheet: u32,
    pub last_us: u32,
    pub max_us: u32,
}

impl Stats {
    pub fn decode(p: &[u8]) -> Option<Self> {
        let u32_at = |i: usize| {
            p.get(i..i + 4)
                .map(|b| u32::from_le_bytes(b.try_into().expect("4 bytes")))
        };
        let n = usize::from(*p.get(24)?);
        let sheets = (0..n)
            .map(|k| {
                let at = 25 + 10 * k;
                let sheet = u16::from_le_bytes([*p.get(at)?, *p.get(at + 1)?]);
                Some(SheetStats {
                    sheet: u32::from(sheet),
                    last_us: u32_at(at + 2)?,
                    max_us: u32_at(at + 6)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            cycles: u32_at(0)?,
            last_us: u32_at(4)?,
            max_us: u32_at(8)?,
            avg_us: u32_at(12)?,
            period_max_us: u32_at(16)?,
            free_heap: u32_at(20)?,
            sheets,
        })
    }
}

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

    /// Takes an unfinished line (a prompt without `\n`), if any. The monitor calls
    /// it when the board has been quiet for a while.
    pub fn flush_text(&mut self) -> Option<String> {
        if self.text.is_empty() {
            return None;
        }
        let line = String::from_utf8_lossy(&self.text).trim_end().to_owned();
        self.text.clear();
        Some(line)
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
    fn stats_decode() {
        let mut p = Vec::new();
        for v in [10u32, 120, 900, 130, 2000, 200_000] {
            p.extend(v.to_le_bytes());
        }
        p.push(1);
        p.extend(2u16.to_le_bytes());
        p.extend(40u32.to_le_bytes());
        p.extend(300u32.to_le_bytes());
        let s = Stats::decode(&p).unwrap();
        assert_eq!(s.max_us, 900);
        assert_eq!(s.free_heap, 200_000);
        assert_eq!(
            s.sheets,
            [SheetStats {
                sheet: 2,
                last_us: 40,
                max_us: 300
            }]
        );
        assert_eq!(Stats::decode(&p[..30]), None);
    }

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
