//! Online debugging support in generated firmware: the signal table and the
//! C++ module that speaks the protocol of `crab-debug`.

use std::fmt::Write as _;

use crab_core::vars::storage_name;
use crab_core::{PortRef, SignalType};
use crab_debug::protocol;
use crab_debug::transport::BAUD;
use crab_debug::{DebugTable, Entry, Kind, Source, TextEntry};

use crate::{locf, Lang, Program};

/// Block outputs in signal order, then variables in declaration order.
/// Strings go to a table of their own: values travel as 4 bytes, strings in `TEXT` frames.
pub fn debug_table(program: &Program) -> DebugTable {
    let mut entries: Vec<Option<Entry>> = vec![None; program.signals.len()];
    let mut texts: Vec<Option<TextEntry>> = vec![None; program.signals.len()];
    for inst in &program.blocks {
        for (&port, &s) in &inst.outputs {
            let signal = &program.signals[s];
            if signal.ty == SignalType::String {
                texts[s] = Some(TextEntry {
                    source: Source::Output(PortRef::new(&inst.block.id, port)),
                    storage: signal.name.clone(),
                });
                continue;
            }
            entries[s] = Some(Entry {
                source: Source::Output(PortRef::new(&inst.block.id, port)),
                ty: signal.ty,
                storage: signal.name.clone(),
                cpp_type: "",
            });
        }
    }
    let mut entries: Vec<Entry> = entries.into_iter().flatten().collect();
    let mut texts: Vec<TextEntry> = texts.into_iter().flatten().collect();
    let vars = program.project.logic.vars.iter();
    texts.extend(
        vars.clone()
            .filter(|v| v.ty.signal() == SignalType::String)
            .map(|v| TextEntry {
                source: Source::Var(v.name.clone()),
                storage: storage_name(&v.name),
            }),
    );
    entries.extend(
        vars.filter(|v| v.ty.signal() != SignalType::String)
            .map(|v| Entry {
                source: Source::Var(v.name.clone()),
                ty: v.ty.signal(),
                storage: storage_name(&v.name),
                cpp_type: v.ty.cpp(),
            }),
    );
    DebugTable { entries, texts }
}

/// Storage kind in firmware: see [`crab_debug::Kind`] for the codes.
/// Values travel as int32 / float bits.
fn type_code(e: &Entry) -> u8 {
    Kind::of(e).code()
}

/// The memory image: a packed structure with the watched values, largest type
/// first (see `crab_debug::image`), copied at the end of every scan. A reader sees
/// one scan only: the report may run on another core, so it takes a copy under a lock.
fn image_module(table: &DebugTable, lang: Lang) -> String {
    let layout = table.image_layout();
    let mut fields = String::new();
    for &i in &layout.order {
        let _ = writeln!(
            fields,
            "  {} v{i};  // {}",
            layout.slots[i].kind.cpp(),
            table.entries[i].storage
        );
    }
    let mut copies = String::new();
    for &i in &layout.order {
        let _ = writeln!(copies, "  crab_img.v{i} = {};", table.entries[i].storage);
    }
    let offsets: Vec<String> = layout
        .slots
        .iter()
        .map(|s| s.offset.to_string())
        .chain(["0".to_owned()])
        .collect();
    format!(
        "{comment}
struct __attribute__((packed)) CrabImage {{
{fields}}};
alignas(8) CrabImage crab_img;
alignas(8) CrabImage crab_img_out;
const uint16_t CRAB_IMG_SIZE = {size};
const uint16_t crab_dbg_off[CRAB_DBG_COUNT + 1] = {{ {offsets} }};
#ifdef ARDUINO_ARCH_ESP32
portMUX_TYPE crab_img_mux = portMUX_INITIALIZER_UNLOCKED;
#define CRAB_IMG_LOCK() portENTER_CRITICAL(&crab_img_mux)
#define CRAB_IMG_UNLOCK() portEXIT_CRITICAL(&crab_img_mux)
#else
#define CRAB_IMG_LOCK()
#define CRAB_IMG_UNLOCK()
#endif

// End of a scan: all watched values are copied at one moment.
void crab_img_snapshot() {{
  CRAB_IMG_LOCK();
{copies}  CRAB_IMG_UNLOCK();
}}
",
        comment = lang.pick(
            "// Образ наблюдаемых значений: от крупных типов к мелким, без дыр; смещения — в crab_dbg_off.",
            "// Image of watched values: largest types first, no gaps; offsets are in crab_dbg_off."
        ),
        size = layout.size,
        offsets = offsets.join(", "),
    )
}

/// The firmware side of the protocol, placed after all globals of the sketch
/// and after the cycle time module of [`crate::stats`].
/// `text_in`: a block reads USB, so the module keeps the console input
/// (`TEXT_IN` frames) for it in `crab_dbg_input`.
/// `background`: a task on core 0 exists; its pass time goes into `STATS` as a trailer.
pub fn runtime(table: &DebugTable, text_in: bool, background: bool, lang: Lang) -> String {
    let n = table.entries.len();
    let ptrs: Vec<String> = table
        .entries
        .iter()
        .map(|e| format!("&{}", e.storage))
        .chain(["nullptr".to_owned()])
        .collect();
    let types: Vec<String> = table
        .entries
        .iter()
        .map(|e| type_code(e).to_string())
        .chain(["0".to_owned()])
        .collect();
    let mut out = String::new();
    let _ = write!(
        out,
        r#"
{head}
#include <mbedtls/md.h>
const uint32_t CRAB_DBG_FINGERPRINT = 0x{fingerprint:08X}UL;
const uint16_t CRAB_DBG_COUNT = {n};
{watched}
void* const crab_dbg_ptr[CRAB_DBG_COUNT + 1] = {{ {ptrs} }};
const uint8_t crab_dbg_type[CRAB_DBG_COUNT + 1] = {{ {types} }};  // see type_code in crab-codegen
bool crab_dbg_forced[CRAB_DBG_COUNT + 1];
uint32_t crab_dbg_force_raw[CRAB_DBG_COUNT + 1];
{image_struct}uint8_t crab_dbg_rx[4 + 255 + 2];
uint16_t crab_dbg_rx_len = 0;
uint16_t crab_dbg_seq = 0;
unsigned long crab_dbg_last = 0;
uint8_t crab_dbg_ticks = 0;

// Password gate: CRAB_AUTH_* come from the end of the sketch and exist only when the
// project has a password. Locked means nothing is sent and no command is taken.
extern const uint8_t CRAB_AUTH_ON __attribute__((weak));
extern const uint8_t CRAB_AUTH_KEY[32] __attribute__((weak));
extern const uint8_t CRAB_AUTH_SALT[16] __attribute__((weak));
bool crab_dbg_authed = false;
unsigned long crab_dbg_auth_seen = 0;
unsigned long crab_dbg_auth_block = 0;
bool crab_dbg_nonce_ok = false;
uint8_t crab_dbg_nonce[16];

bool crab_dbg_locked() {{
  return &CRAB_AUTH_ON != nullptr && !crab_dbg_authed;
}}

uint16_t crab_dbg_crc(uint16_t crc, uint8_t b) {{
  crc ^= (uint16_t)b << 8;
  for (uint8_t i = 0; i < 8; i++) {{
    crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
  }}
  return crc;
}}

void crab_dbg_frame(uint8_t type, const uint8_t* data, uint8_t len) {{
  uint16_t crc = crab_dbg_crc(crab_dbg_crc(0xFFFF, type), len);
  for (uint8_t i = 0; i < len; i++) {{
    crc = crab_dbg_crc(crc, data[i]);
  }}
  const uint8_t head[4] = {{0x{s0:02X}, 0x{s1:02X}, type, len}};
  const uint8_t tail[2] = {{(uint8_t)(crc & 0xFF), (uint8_t)(crc >> 8)}};
  Serial.write(head, 4);
  if (len > 0) {{
    Serial.write(data, len);
  }}
  Serial.write(tail, 2);
}}

void crab_dbg_put(uint16_t i, uint32_t raw) {{
  void* p = crab_dbg_ptr[i];
  int32_t v = (int32_t)raw;
  switch (crab_dbg_type[i]) {{
    case 0: *(bool*)p = raw != 0; break;
    case 3: *(int8_t*)p = (int8_t)v; break;
    case 4: *(uint8_t*)p = (uint8_t)v; break;
    case 5: *(int16_t*)p = (int16_t)v; break;
    case 6: *(uint16_t*)p = (uint16_t)v; break;
    case 8: *(int64_t*)p = v; break;
    case 9: *(uint64_t*)p = (uint32_t)raw; break;
    case 10: {{ float f; memcpy(&f, &raw, 4); *(double*)p = f; break; }}
    default: memcpy(p, &raw, 4);
  }}
}}

// Called after the block that produces value i: a forced value wins.
void crab_dbg_force(uint16_t i) {{
  if (crab_dbg_forced[i]) {{
    crab_dbg_put(i, crab_dbg_force_raw[i]);
  }}
}}

// Identity of the project this firmware was built from (see `CRAB_PROJECT_HASH` at the
// end of the sketch); weak, so a build without it still links. Zeros are not shown to a
// host that has not signed in.
extern const uint8_t CRAB_PROJECT_HASH[8] __attribute__((weak));

void crab_dbg_hello() {{
  uint8_t p[18] = {{0}};
  memcpy(p, &CRAB_DBG_FINGERPRINT, 4);
  p[4] = CRAB_DBG_COUNT & 0xFF;
  p[5] = CRAB_DBG_COUNT >> 8;
  p[6] = {version};
  p[7] = CRAB_IMG_SIZE & 0xFF;
  p[8] = CRAB_IMG_SIZE >> 8;
  p[9] = crab_dbg_locked() ? {locked} : 0;
  if (&CRAB_PROJECT_HASH != nullptr && !crab_dbg_locked()) {{
    memcpy(p + 10, CRAB_PROJECT_HASH, 8);
  }}
  crab_dbg_frame(0x{hello:02X}, p, 18);
}}

// The image of one scan in pieces: seq u16, offset u16, bytes.
void crab_dbg_image() {{
  uint8_t p[4 + {chunk}];
  CRAB_IMG_LOCK();
  memcpy(&crab_img_out, &crab_img, sizeof(crab_img));
  CRAB_IMG_UNLOCK();
  crab_dbg_seq++;
  for (uint16_t off = 0; off < CRAB_IMG_SIZE; off += {chunk}) {{
    uint16_t n = (CRAB_IMG_SIZE - off) > {chunk} ? {chunk} : (CRAB_IMG_SIZE - off);
    p[0] = crab_dbg_seq & 0xFF;
    p[1] = crab_dbg_seq >> 8;
    p[2] = off & 0xFF;
    p[3] = off >> 8;
    memcpy(p + 4, (uint8_t*)&crab_img_out + off, n);
    crab_dbg_frame(0x{image:02X}, p, 4 + n);
  }}
}}

{texts}{text_in}void crab_dbg_stats() {{
  uint8_t p[25 + 10 * CRAB_ST_SHEETS + 8];
  uint32_t head[6] = {{crab_st_cycles, crab_st_last, crab_st_max, 0, crab_st_period_max, crab_st_free_heap()}};
  head[3] = crab_st_take_avg();
  memcpy(p, head, 24);
  p[24] = CRAB_ST_SHEETS;
  for (uint8_t k = 0; k < CRAB_ST_SHEETS; k++) {{
    uint8_t* q = p + 25 + 10 * k;
    q[0] = crab_st_sheet_no[k] & 0xFF;
    q[1] = crab_st_sheet_no[k] >> 8;
    memcpy(q + 2, &crab_st_sheet_last[k], 4);
    memcpy(q + 6, &crab_st_sheet_max[k], 4);
  }}
{bg_tail}  crab_dbg_frame(0x{stats:02X}, p, 25 + 10 * CRAB_ST_SHEETS{bg_len});
}}

// The project archive in flash (see `CRAB_PROJECT` at the end of the sketch); weak,
// so a sketch generated without it still links and answers "no project".
extern const uint8_t CRAB_PROJECT[] PROGMEM __attribute__((weak));
extern const uint32_t CRAB_PROJECT_SIZE __attribute__((weak));

// One piece of the project: total u32, offset u32, bytes.
void crab_dbg_project(const uint8_t* p, uint8_t len) {{
  uint32_t total = (&CRAB_PROJECT_SIZE != nullptr) ? CRAB_PROJECT_SIZE : 0;
  uint32_t off = 0;
  if (len >= 4) {{
    memcpy(&off, p, 4);
  }}
  uint32_t n = off < total ? total - off : 0;
  if (n > {project_chunk}) {{
    n = {project_chunk};
  }}
  uint8_t out[8 + {project_chunk}];
  memcpy(out, &total, 4);
  memcpy(out + 4, &off, 4);
  if (n > 0) {{
    memcpy_P(out + 8, CRAB_PROJECT + off, n);
  }}
  crab_dbg_frame(0x{project:02X}, out, 8 + n);
}}

// Password check: a challenge (salt + random nonce), then HMAC-SHA256 of the nonce.
void crab_dbg_auth_challenge() {{
  uint8_t out[32];
  for (uint8_t k = 0; k < 16; k += 4) {{
    uint32_t r = esp_random();
    memcpy(crab_dbg_nonce + k, &r, 4);
  }}
  crab_dbg_nonce_ok = true;
  memcpy(out, CRAB_AUTH_SALT, 16);
  memcpy(out + 16, crab_dbg_nonce, 16);
  crab_dbg_frame(0x{auth_challenge:02X}, out, 32);
}}

void crab_dbg_auth_check(const uint8_t* p, uint8_t len) {{
  if (!crab_dbg_nonce_ok || len != 32 || (long)(millis() - crab_dbg_auth_block) < 0) {{
    return;
  }}
  crab_dbg_nonce_ok = false;
  uint8_t proof[32];
  mbedtls_md_hmac(mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), CRAB_AUTH_KEY, 32,
                  crab_dbg_nonce, 16, proof);
  uint8_t diff = 0;
  for (uint8_t k = 0; k < 32; k++) {{
    diff |= proof[k] ^ p[k];
  }}
  if (diff == 0) {{
    crab_dbg_authed = true;
    crab_dbg_auth_seen = millis();
    crab_dbg_frame(0x{auth_ok:02X}, nullptr, 0);
    crab_dbg_hello();
  }} else {{
    crab_dbg_auth_block = millis() + 1000;
    crab_dbg_frame(0x{auth_fail:02X}, nullptr, 0);
  }}
}}

void crab_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {{
  if (&CRAB_AUTH_ON != nullptr) {{
    if (type == 0x{auth_req:02X}) {{
      crab_dbg_authed = false;  // also the way to sign out
      crab_dbg_auth_challenge();
      return;
    }}
    if (type == 0x{auth:02X}) {{
      crab_dbg_auth_check(p, len);
      return;
    }}
    if (crab_dbg_authed) {{
      crab_dbg_auth_seen = millis();
    }} else {{
      if (type == 0x{hello_req:02X}) {{
        crab_dbg_hello();
      }}
      return;
    }}
    if (type == 0x{ping:02X}) {{
      return;
    }}
  }}
  if (type == 0x{get_project:02X}) {{
    crab_dbg_project(p, len);
    return;
  }}
  if (type == 0x{hello_req:02X}) {{
    crab_dbg_hello();{texts_reset}
    return;
  }}
  if (type == 0x{reset_stats:02X}) {{
    crab_st_reset();
    return;
  }}{text_in_command}
  if (type == 0x{release_all:02X}) {{
    for (uint16_t i = 0; i < CRAB_DBG_COUNT; i++) {{
      crab_dbg_forced[i] = false;
    }}
    return;
  }}
  if (len < 2) {{
    return;
  }}
  uint16_t i = p[0] | (p[1] << 8);
  if (i >= CRAB_DBG_COUNT) {{
    return;
  }}
  uint32_t raw;
  if (type == 0x{force:02X} && len >= 7) {{
    memcpy(&raw, p + 3, 4);
    crab_dbg_forced[i] = p[2] != 0;
    crab_dbg_force_raw[i] = raw;
    crab_dbg_force(i);
  }} else if (type == 0x{set:02X} && len >= 6) {{
    memcpy(&raw, p + 2, 4);
    crab_dbg_put(i, raw);
  }}
}}

void crab_dbg_poll() {{
  while (Serial.available() > 0) {{
    uint8_t b = Serial.read();
    if (crab_dbg_rx_len == 0 && b != 0x{s0:02X}) {{
      continue;
    }}
    if (crab_dbg_rx_len == 1 && b != 0x{s1:02X}) {{
      crab_dbg_rx_len = (b == 0x{s0:02X}) ? 1 : 0;
      continue;
    }}
    crab_dbg_rx[crab_dbg_rx_len++] = b;
    if (crab_dbg_rx_len >= 4 && crab_dbg_rx_len == 4 + crab_dbg_rx[3] + 2) {{
      uint16_t end = crab_dbg_rx_len - 2;
      uint16_t crc = 0xFFFF;
      for (uint16_t k = 2; k < end; k++) {{
        crc = crab_dbg_crc(crc, crab_dbg_rx[k]);
      }}
      if ((crc & 0xFF) == crab_dbg_rx[end] && (crc >> 8) == crab_dbg_rx[end + 1]) {{
        crab_dbg_command(crab_dbg_rx[2], crab_dbg_rx + 4, crab_dbg_rx[3]);
      }}
      crab_dbg_rx_len = 0;
    }}
  }}
}}

void crab_dbg_setup() {{
  Serial.begin({baud});
  crab_dbg_hello();
}}

void crab_dbg_loop() {{
  crab_dbg_poll();
  if (crab_dbg_authed && millis() - crab_dbg_auth_seen > {idle}UL) {{
    crab_dbg_authed = false;
    crab_dbg_hello();
  }}
  if (crab_dbg_locked()) {{
    return;
  }}
  if (millis() - crab_dbg_last >= 100) {{
    crab_dbg_last = millis();
    crab_dbg_image();{texts_loop}
    if (++crab_dbg_ticks >= 5) {{
      crab_dbg_ticks = 0;
      crab_dbg_stats();
    }}
  }}
}}
"#,
        version = protocol::VERSION,
        fingerprint = table.fingerprint(),
        ptrs = ptrs.join(", "),
        types = types.join(", "),
        s0 = protocol::SYNC[0],
        s1 = protocol::SYNC[1],
        hello = protocol::HELLO,
        image = protocol::IMAGE,
        get_project = protocol::GET_PROJECT,
        project = protocol::PROJECT,
        project_chunk = protocol::PROJECT_CHUNK,
        hello_req = protocol::HELLO_REQ,
        locked = protocol::HELLO_LOCKED,
        auth_challenge = protocol::AUTH_CHALLENGE,
        auth_ok = protocol::AUTH_OK,
        auth_fail = protocol::AUTH_FAIL,
        auth_req = protocol::AUTH_REQ,
        auth = protocol::AUTH,
        ping = protocol::PING,
        idle = protocol::AUTH_IDLE_MS,
        force = protocol::FORCE,
        set = protocol::SET,
        release_all = protocol::RELEASE_ALL,
        stats = protocol::STATS,
        reset_stats = protocol::RESET_STATS,
        chunk = protocol::IMAGE_CHUNK,
        baud = BAUD,
        head = locf!(
            lang,
            "// ---- Онлайн-отладка CrabLogic (протокол v{}) ----
// Кадры: 0xAA 0x55, тип, длина, данные, CRC-16/CCITT. Обычный Serial.print не мешает.",
            "// ---- CrabLogic online debugging (protocol v{}) ----
// Frames: 0xAA 0x55, type, length, data, CRC-16/CCITT. Plain Serial.print does not interfere.",
            protocol::VERSION
        ),
        watched = lang.pick(
            "// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).",
            "// Watched values: block outputs, then variables (+ an empty element at the end)."
        ),
        texts = texts_module(table, lang),
        image_struct = image_module(table, lang),
        bg_tail = if background {
            "  memcpy(p + 25 + 10 * CRAB_ST_SHEETS, &crab_st_bg_last, 4);
  memcpy(p + 29 + 10 * CRAB_ST_SHEETS, &crab_st_bg_max, 4);
"
        } else {
            ""
        },
        bg_len = if background { " + 8" } else { "" },
        text_in = if text_in {
            format!(
                "{}
{TEXT_IN_MODULE}",
                lang.pick(
                    "// Ввод из консоли отладки (кадры TEXT_IN) для «Чтения из UART» с USB.",
                    "// Debug console input (TEXT_IN frames) for \"UART read\" on USB."
                )
            )
        } else {
            String::new()
        },
        text_in_command = if text_in {
            format!(
                "\n  if (type == 0x{:02X}) {{\n    crab_dbg_in_push(p, len);\n    return;\n  }}",
                protocol::TEXT_IN
            )
        } else {
            String::new()
        },
        texts_reset = if table.texts.is_empty() {
            ""
        } else {
            "\n    crab_dbg_texts_reset();"
        },
        texts_loop = if table.texts.is_empty() {
            ""
        } else {
            "\n    crab_dbg_texts();"
        },
    );
    out
}

/// Strings of the table: each one goes in a `TEXT` frame when its checksum changes,
/// at most a few frames per tick so values are not delayed. Empty without strings.
fn texts_module(table: &DebugTable, lang: Lang) -> String {
    if table.texts.is_empty() {
        return String::new();
    }
    let ptrs: Vec<String> = table
        .texts
        .iter()
        .map(|t| format!("&{}", t.storage))
        .collect();
    format!(
        r#"{comment}
const uint16_t CRAB_DBG_TEXTS = {n};
String* const crab_dbg_text[CRAB_DBG_TEXTS] = {{ {ptrs} }};
uint16_t crab_dbg_text_crc[CRAB_DBG_TEXTS];
bool crab_dbg_text_sent[CRAB_DBG_TEXTS];
uint16_t crab_dbg_text_next = 0;

void crab_dbg_texts_reset() {{
  for (uint16_t i = 0; i < CRAB_DBG_TEXTS; i++) {{
    crab_dbg_text_sent[i] = false;
  }}
}}

void crab_dbg_texts() {{
  uint8_t p[2 + {max}];
  uint8_t budget = {budget};
  for (uint16_t k = 0; k < CRAB_DBG_TEXTS && budget > 0; k++) {{
    uint16_t i = crab_dbg_text_next;
    crab_dbg_text_next = (i + 1) % CRAB_DBG_TEXTS;
    const String& s = *crab_dbg_text[i];
    uint16_t len = s.length();
    if (len > {max}) {{
      len = {max};
      // Do not cut a UTF-8 character in half.
      while (len > 0 && ((uint8_t)s[len] & 0xC0) == 0x80) {{
        len--;
      }}
    }}
    uint16_t crc = crab_dbg_crc(0xFFFF, (uint8_t)len);
    for (uint16_t j = 0; j < len; j++) {{
      crc = crab_dbg_crc(crc, (uint8_t)s[j]);
    }}
    if (crab_dbg_text_sent[i] && crc == crab_dbg_text_crc[i]) {{
      continue;
    }}
    crab_dbg_text_crc[i] = crc;
    crab_dbg_text_sent[i] = true;
    p[0] = i & 0xFF;
    p[1] = i >> 8;
    memcpy(p + 2, s.c_str(), len);
    crab_dbg_frame(0x{kind:02X}, p, 2 + len);
    budget--;
  }}
}}

"#,
        n = table.texts.len(),
        ptrs = ptrs.join(", "),
        comment = locf!(
            lang,
            "// Строки: кадр TEXT, только когда строка изменилась (до {} байт).",
            "// Strings: a TEXT frame only when the string has changed (up to {} bytes).",
            protocol::TEXT_MAX
        ),
        max = protocol::TEXT_MAX,
        budget = 8,
        kind = protocol::TEXT,
    )
}

/// Console input of the debug session: a ring buffer that `uart_read` on USB
/// reads through `crab_dbg_input` the way it reads a `HardwareSerial`.
const TEXT_IN_MODULE: &str = r#"uint8_t crab_dbg_in_buf[256];
uint8_t crab_dbg_in_head = 0;
uint8_t crab_dbg_in_tail = 0;

void crab_dbg_in_push(const uint8_t* p, uint8_t len) {
  for (uint8_t k = 0; k < len; k++) {
    uint8_t next = crab_dbg_in_head + 1;
    if (next == crab_dbg_in_tail) {
      return;  // full: the rest of the line is dropped
    }
    crab_dbg_in_buf[crab_dbg_in_head] = p[k];
    crab_dbg_in_head = next;
  }
}

struct CrabDebugInput {
  int available() { return (uint8_t)(crab_dbg_in_head - crab_dbg_in_tail); }
  int read() {
    if (crab_dbg_in_head == crab_dbg_in_tail) {
      return -1;
    }
    return crab_dbg_in_buf[crab_dbg_in_tail++];
  }
};
CrabDebugInput crab_dbg_input;

"#;
