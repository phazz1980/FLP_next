//! Online debugging support in generated firmware: the signal table and the
//! C++ module that speaks the protocol of `flp-debug`.

use std::fmt::Write as _;

use flp_core::vars::storage_name;
use flp_core::{PortRef, SignalType};
use flp_debug::protocol;
use flp_debug::transport::BAUD;
use flp_debug::{DebugTable, Entry, Source, TextEntry};

use crate::Program;

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

/// Storage kind in firmware: 0 bool, 1 int32, 2 float, 3 int8, 4 uint8, 5 int16,
/// 6 uint16, 7 uint32, 8 int64, 9 uint64, 10 double. Values travel as int32 / float bits.
fn type_code(e: &Entry) -> u8 {
    match e.cpp_type {
        "char" | "int8_t" => 3,
        "byte" | "uint8_t" => 4,
        "short" | "int16_t" => 5,
        "word" | "uint16_t" => 6,
        "unsigned int" | "unsigned long" | "uint32_t" => 7,
        "long long" | "int64_t" => 8,
        "unsigned long long" | "uint64_t" => 9,
        "double" => 10,
        _ => match e.ty {
            SignalType::Bool => 0,
            SignalType::Int => 1,
            SignalType::Float => 2,
            SignalType::String | SignalType::Any | SignalType::Number => {
                unreachable!("strings are not in the debug table")
            }
        },
    }
}

/// The firmware side of the protocol, placed after all globals of the sketch
/// and after the cycle time module of [`crate::stats`].
/// `text_in`: a block reads USB, so the module keeps the console input
/// (`TEXT_IN` frames) for it in `flp_dbg_input`.
/// `background`: a task on core 0 exists; its pass time goes into `STATS` as a trailer.
pub fn runtime(table: &DebugTable, text_in: bool, background: bool) -> String {
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
// ---- Онлайн-отладка FLP_next (протокол v{version}) ----
// Кадры: 0xAA 0x55, тип, длина, данные, CRC-16/CCITT. Обычный Serial.print не мешает.
const uint32_t FLP_DBG_FINGERPRINT = 0x{fingerprint:08X}UL;
const uint16_t FLP_DBG_COUNT = {n};
// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).
void* const flp_dbg_ptr[FLP_DBG_COUNT + 1] = {{ {ptrs} }};
const uint8_t flp_dbg_type[FLP_DBG_COUNT + 1] = {{ {types} }};  // see type_code in flp-codegen
bool flp_dbg_forced[FLP_DBG_COUNT + 1];
uint32_t flp_dbg_force_raw[FLP_DBG_COUNT + 1];
uint8_t flp_dbg_rx[4 + 255 + 2];
uint16_t flp_dbg_rx_len = 0;
uint16_t flp_dbg_seq = 0;
unsigned long flp_dbg_last = 0;
uint8_t flp_dbg_ticks = 0;

uint16_t flp_dbg_crc(uint16_t crc, uint8_t b) {{
  crc ^= (uint16_t)b << 8;
  for (uint8_t i = 0; i < 8; i++) {{
    crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
  }}
  return crc;
}}

void flp_dbg_frame(uint8_t type, const uint8_t* data, uint8_t len) {{
  uint16_t crc = flp_dbg_crc(flp_dbg_crc(0xFFFF, type), len);
  for (uint8_t i = 0; i < len; i++) {{
    crc = flp_dbg_crc(crc, data[i]);
  }}
  const uint8_t head[4] = {{0x{s0:02X}, 0x{s1:02X}, type, len}};
  const uint8_t tail[2] = {{(uint8_t)(crc & 0xFF), (uint8_t)(crc >> 8)}};
  Serial.write(head, 4);
  if (len > 0) {{
    Serial.write(data, len);
  }}
  Serial.write(tail, 2);
}}

uint32_t flp_dbg_get(uint16_t i) {{
  uint32_t raw = 0;
  void* p = flp_dbg_ptr[i];
  switch (flp_dbg_type[i]) {{
    case 0: raw = *(bool*)p ? 1 : 0; break;
    case 3: raw = (uint32_t)(int32_t)*(int8_t*)p; break;
    case 4: raw = *(uint8_t*)p; break;
    case 5: raw = (uint32_t)(int32_t)*(int16_t*)p; break;
    case 6: raw = *(uint16_t*)p; break;
    case 8: raw = (uint32_t)(int32_t)*(int64_t*)p; break;
    case 9: raw = (uint32_t)*(uint64_t*)p; break;
    case 10: {{ float f = (float)*(double*)p; memcpy(&raw, &f, 4); break; }}
    default: memcpy(&raw, p, 4);
  }}
  return raw;
}}

void flp_dbg_put(uint16_t i, uint32_t raw) {{
  void* p = flp_dbg_ptr[i];
  int32_t v = (int32_t)raw;
  switch (flp_dbg_type[i]) {{
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
void flp_dbg_force(uint16_t i) {{
  if (flp_dbg_forced[i]) {{
    flp_dbg_put(i, flp_dbg_force_raw[i]);
  }}
}}

void flp_dbg_hello() {{
  uint8_t p[7];
  memcpy(p, &FLP_DBG_FINGERPRINT, 4);
  p[4] = FLP_DBG_COUNT & 0xFF;
  p[5] = FLP_DBG_COUNT >> 8;
  p[6] = {version};
  flp_dbg_frame(0x{hello:02X}, p, 7);
}}

void flp_dbg_values() {{
  uint8_t p[5 + 4 * {per_frame}];
  flp_dbg_seq++;
  for (uint16_t start = 0; start < FLP_DBG_COUNT; start += {per_frame}) {{
    uint8_t n = (FLP_DBG_COUNT - start) > {per_frame} ? {per_frame} : (uint8_t)(FLP_DBG_COUNT - start);
    p[0] = flp_dbg_seq & 0xFF;
    p[1] = flp_dbg_seq >> 8;
    p[2] = start & 0xFF;
    p[3] = start >> 8;
    p[4] = n;
    for (uint8_t k = 0; k < n; k++) {{
      uint32_t raw = flp_dbg_get(start + k);
      memcpy(p + 5 + 4 * k, &raw, 4);
    }}
    flp_dbg_frame(0x{values:02X}, p, 5 + 4 * n);
  }}
}}

{texts}{text_in}void flp_dbg_stats() {{
  uint8_t p[25 + 10 * FLP_ST_SHEETS + 8];
  uint32_t head[6] = {{flp_st_cycles, flp_st_last, flp_st_max, 0, flp_st_period_max, flp_st_free_heap()}};
  head[3] = flp_st_take_avg();
  memcpy(p, head, 24);
  p[24] = FLP_ST_SHEETS;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {{
    uint8_t* q = p + 25 + 10 * k;
    q[0] = flp_st_sheet_no[k] & 0xFF;
    q[1] = flp_st_sheet_no[k] >> 8;
    memcpy(q + 2, &flp_st_sheet_last[k], 4);
    memcpy(q + 6, &flp_st_sheet_max[k], 4);
  }}
{bg_tail}  flp_dbg_frame(0x{stats:02X}, p, 25 + 10 * FLP_ST_SHEETS{bg_len});
}}

void flp_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {{
  if (type == 0x{hello_req:02X}) {{
    flp_dbg_hello();{texts_reset}
    return;
  }}
  if (type == 0x{reset_stats:02X}) {{
    flp_st_reset();
    return;
  }}{text_in_command}
  if (type == 0x{release_all:02X}) {{
    for (uint16_t i = 0; i < FLP_DBG_COUNT; i++) {{
      flp_dbg_forced[i] = false;
    }}
    return;
  }}
  if (len < 2) {{
    return;
  }}
  uint16_t i = p[0] | (p[1] << 8);
  if (i >= FLP_DBG_COUNT) {{
    return;
  }}
  uint32_t raw;
  if (type == 0x{force:02X} && len >= 7) {{
    memcpy(&raw, p + 3, 4);
    flp_dbg_forced[i] = p[2] != 0;
    flp_dbg_force_raw[i] = raw;
    flp_dbg_force(i);
  }} else if (type == 0x{set:02X} && len >= 6) {{
    memcpy(&raw, p + 2, 4);
    flp_dbg_put(i, raw);
  }}
}}

void flp_dbg_poll() {{
  while (Serial.available() > 0) {{
    uint8_t b = Serial.read();
    if (flp_dbg_rx_len == 0 && b != 0x{s0:02X}) {{
      continue;
    }}
    if (flp_dbg_rx_len == 1 && b != 0x{s1:02X}) {{
      flp_dbg_rx_len = (b == 0x{s0:02X}) ? 1 : 0;
      continue;
    }}
    flp_dbg_rx[flp_dbg_rx_len++] = b;
    if (flp_dbg_rx_len >= 4 && flp_dbg_rx_len == 4 + flp_dbg_rx[3] + 2) {{
      uint16_t end = flp_dbg_rx_len - 2;
      uint16_t crc = 0xFFFF;
      for (uint16_t k = 2; k < end; k++) {{
        crc = flp_dbg_crc(crc, flp_dbg_rx[k]);
      }}
      if ((crc & 0xFF) == flp_dbg_rx[end] && (crc >> 8) == flp_dbg_rx[end + 1]) {{
        flp_dbg_command(flp_dbg_rx[2], flp_dbg_rx + 4, flp_dbg_rx[3]);
      }}
      flp_dbg_rx_len = 0;
    }}
  }}
}}

void flp_dbg_setup() {{
  Serial.begin({baud});
  flp_dbg_hello();
}}

void flp_dbg_loop() {{
  flp_dbg_poll();
  if (millis() - flp_dbg_last >= 100) {{
    flp_dbg_last = millis();
    flp_dbg_values();{texts_loop}
    if (++flp_dbg_ticks >= 5) {{
      flp_dbg_ticks = 0;
      flp_dbg_stats();
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
        values = protocol::VALUES,
        hello_req = protocol::HELLO_REQ,
        force = protocol::FORCE,
        set = protocol::SET,
        release_all = protocol::RELEASE_ALL,
        stats = protocol::STATS,
        reset_stats = protocol::RESET_STATS,
        per_frame = protocol::VALUES_PER_FRAME,
        baud = BAUD,
        texts = texts_module(table),
        bg_tail = if background {
            "  memcpy(p + 25 + 10 * FLP_ST_SHEETS, &flp_st_bg_last, 4);
  memcpy(p + 29 + 10 * FLP_ST_SHEETS, &flp_st_bg_max, 4);
"
        } else {
            ""
        },
        bg_len = if background { " + 8" } else { "" },
        text_in = if text_in { TEXT_IN_MODULE } else { "" },
        text_in_command = if text_in {
            format!(
                "\n  if (type == 0x{:02X}) {{\n    flp_dbg_in_push(p, len);\n    return;\n  }}",
                protocol::TEXT_IN
            )
        } else {
            String::new()
        },
        texts_reset = if table.texts.is_empty() {
            ""
        } else {
            "\n    flp_dbg_texts_reset();"
        },
        texts_loop = if table.texts.is_empty() {
            ""
        } else {
            "\n    flp_dbg_texts();"
        },
    );
    out
}

/// Strings of the table: each one goes in a `TEXT` frame when its checksum changes,
/// at most a few frames per tick so values are not delayed. Empty without strings.
fn texts_module(table: &DebugTable) -> String {
    if table.texts.is_empty() {
        return String::new();
    }
    let ptrs: Vec<String> = table
        .texts
        .iter()
        .map(|t| format!("&{}", t.storage))
        .collect();
    format!(
        r#"// Строки: кадр TEXT, только когда строка изменилась (до {max} байт).
const uint16_t FLP_DBG_TEXTS = {n};
String* const flp_dbg_text[FLP_DBG_TEXTS] = {{ {ptrs} }};
uint16_t flp_dbg_text_crc[FLP_DBG_TEXTS];
bool flp_dbg_text_sent[FLP_DBG_TEXTS];
uint16_t flp_dbg_text_next = 0;

void flp_dbg_texts_reset() {{
  for (uint16_t i = 0; i < FLP_DBG_TEXTS; i++) {{
    flp_dbg_text_sent[i] = false;
  }}
}}

void flp_dbg_texts() {{
  uint8_t p[2 + {max}];
  uint8_t budget = {budget};
  for (uint16_t k = 0; k < FLP_DBG_TEXTS && budget > 0; k++) {{
    uint16_t i = flp_dbg_text_next;
    flp_dbg_text_next = (i + 1) % FLP_DBG_TEXTS;
    const String& s = *flp_dbg_text[i];
    uint16_t len = s.length();
    if (len > {max}) {{
      len = {max};
      // Do not cut a UTF-8 character in half.
      while (len > 0 && ((uint8_t)s[len] & 0xC0) == 0x80) {{
        len--;
      }}
    }}
    uint16_t crc = flp_dbg_crc(0xFFFF, (uint8_t)len);
    for (uint16_t j = 0; j < len; j++) {{
      crc = flp_dbg_crc(crc, (uint8_t)s[j]);
    }}
    if (flp_dbg_text_sent[i] && crc == flp_dbg_text_crc[i]) {{
      continue;
    }}
    flp_dbg_text_crc[i] = crc;
    flp_dbg_text_sent[i] = true;
    p[0] = i & 0xFF;
    p[1] = i >> 8;
    memcpy(p + 2, s.c_str(), len);
    flp_dbg_frame(0x{kind:02X}, p, 2 + len);
    budget--;
  }}
}}

"#,
        n = table.texts.len(),
        ptrs = ptrs.join(", "),
        max = protocol::TEXT_MAX,
        budget = 8,
        kind = protocol::TEXT,
    )
}

/// Console input of the debug session: a ring buffer that `uart_read` on USB
/// reads through `flp_dbg_input` the way it reads a `HardwareSerial`.
const TEXT_IN_MODULE: &str = r#"// Ввод из консоли отладки (кадры TEXT_IN) для «Чтения из UART» с USB.
uint8_t flp_dbg_in_buf[256];
uint8_t flp_dbg_in_head = 0;
uint8_t flp_dbg_in_tail = 0;

void flp_dbg_in_push(const uint8_t* p, uint8_t len) {
  for (uint8_t k = 0; k < len; k++) {
    uint8_t next = flp_dbg_in_head + 1;
    if (next == flp_dbg_in_tail) {
      return;  // full: the rest of the line is dropped
    }
    flp_dbg_in_buf[flp_dbg_in_head] = p[k];
    flp_dbg_in_head = next;
  }
}

struct FlpDebugInput {
  int available() { return (uint8_t)(flp_dbg_in_head - flp_dbg_in_tail); }
  int read() {
    if (flp_dbg_in_head == flp_dbg_in_tail) {
      return -1;
    }
    return flp_dbg_in_buf[flp_dbg_in_tail++];
  }
};
FlpDebugInput flp_dbg_input;

"#;
