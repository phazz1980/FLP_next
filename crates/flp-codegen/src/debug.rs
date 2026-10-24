//! Online debugging support in generated firmware: the signal table and the
//! C++ module that speaks the protocol of `flp-debug`.

use std::fmt::Write as _;

use flp_core::vars::storage_name;
use flp_core::{PortRef, SignalType};
use flp_debug::protocol;
use flp_debug::transport::BAUD;
use flp_debug::{DebugTable, Entry, Source};

use crate::Program;

/// Block outputs in signal order, then variables in declaration order.
pub fn debug_table(program: &Program) -> DebugTable {
    let mut entries: Vec<Option<Entry>> = vec![None; program.signals.len()];
    for inst in &program.blocks {
        for (&port, &s) in &inst.outputs {
            let signal = &program.signals[s];
            entries[s] = Some(Entry {
                source: Source::Output(PortRef::new(&inst.block.id, port)),
                ty: signal.ty,
                storage: signal.name.clone(),
            });
        }
    }
    let mut entries: Vec<Entry> = entries.into_iter().flatten().collect();
    entries.extend(program.project.logic.vars.iter().map(|v| Entry {
        source: Source::Var(v.name.clone()),
        ty: v.ty,
        storage: storage_name(&v.name),
    }));
    DebugTable { entries }
}

fn type_code(ty: SignalType) -> u8 {
    match ty {
        SignalType::Bool => 0,
        SignalType::Int => 1,
        SignalType::Float => 2,
    }
}

/// The firmware side of the protocol, placed after all globals of the sketch.
pub fn runtime(table: &DebugTable) -> String {
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
        .map(|e| type_code(e.ty).to_string())
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
const uint8_t flp_dbg_type[FLP_DBG_COUNT + 1] = {{ {types} }};  // 0 bool, 1 int, 2 float
bool flp_dbg_forced[FLP_DBG_COUNT + 1];
uint32_t flp_dbg_force_raw[FLP_DBG_COUNT + 1];
uint8_t flp_dbg_rx[4 + 255 + 2];
uint16_t flp_dbg_rx_len = 0;
uint16_t flp_dbg_seq = 0;
unsigned long flp_dbg_last = 0;

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
  if (flp_dbg_type[i] == 0) {{
    raw = *(bool*)flp_dbg_ptr[i] ? 1 : 0;
  }} else {{
    memcpy(&raw, flp_dbg_ptr[i], 4);
  }}
  return raw;
}}

void flp_dbg_put(uint16_t i, uint32_t raw) {{
  if (flp_dbg_type[i] == 0) {{
    *(bool*)flp_dbg_ptr[i] = raw != 0;
  }} else {{
    memcpy(flp_dbg_ptr[i], &raw, 4);
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

void flp_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {{
  if (type == 0x{hello_req:02X}) {{
    flp_dbg_hello();
    return;
  }}
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
    flp_dbg_values();
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
        per_frame = protocol::VALUES_PER_FRAME,
        baud = BAUD,
    );
    out
}
