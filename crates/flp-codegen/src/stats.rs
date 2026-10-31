//! Cycle time diagnostics in generated firmware: the whole scan and each sheet.
//!
//! `loop()` calls `flp_st_begin()` first, `flp_st_sheet(k)` where the execution
//! order moves to blocks of another sheet and `flp_st_end()` after the logic.
//! Debug builds send the figures in `STATS` frames; normal builds with
//! `[diagnostics] enabled = true` print them as text.

use std::fmt::Write as _;

use flp_debug::protocol::MAX_STAT_SHEETS;

use crate::{Instance, Lang, Program};

/// Sheet a block is drawn on; blocks without coordinates are on sheet 1.
pub fn sheet_of(program: &Program, inst: &Instance) -> u32 {
    program
        .project
        .layout
        .blocks
        .get(&inst.block.id)
        .map_or(1, |p| p.sheet)
}

/// Measured sheets in ascending order, at most [`MAX_STAT_SHEETS`]; the time of
/// further sheets is counted to the sheet executed before them.
pub fn measured_sheets<'a>(
    program: &Program,
    looped: impl Iterator<Item = &'a Instance<'a>>,
) -> Vec<u32> {
    let mut sheets: Vec<u32> = looped.map(|i| sheet_of(program, i)).collect();
    sheets.sort_unstable();
    sheets.dedup();
    sheets.truncate(MAX_STAT_SHEETS);
    sheets
}

/// Pass time of the background task (core 0): from taking the inputs to publishing
/// the outputs, without the pause between passes.
const BG_MODULE: &str = r#"uint32_t flp_st_bg_last = 0;
uint32_t flp_st_bg_max = 0;
uint32_t flp_st_bg_start = 0;

void flp_st_bg_begin() {
  flp_st_bg_start = micros();
}

void flp_st_bg_end() {
  flp_st_bg_last = micros() - flp_st_bg_start;
  if (flp_st_bg_last > flp_st_bg_max) {
    flp_st_bg_max = flp_st_bg_last;
  }
}
"#;

/// Measurement code, placed after all globals of the sketch.
pub fn runtime(sheets: &[u32], background: bool, lang: Lang) -> String {
    let numbers: Vec<String> = sheets
        .iter()
        .map(u32::to_string)
        .chain(["0".to_owned()])
        .collect();
    format!(
        r#"
// ---- {title} ----
const uint8_t FLP_ST_SHEETS = {n};
const uint16_t flp_st_sheet_no[FLP_ST_SHEETS + 1] = {{ {numbers} }};
uint32_t flp_st_acc[FLP_ST_SHEETS + 1];
uint32_t flp_st_sheet_last[FLP_ST_SHEETS + 1];
uint32_t flp_st_sheet_max[FLP_ST_SHEETS + 1];
uint8_t flp_st_cur = FLP_ST_SHEETS;
bool flp_st_started = false;
uint32_t flp_st_begin_us = 0;
uint32_t flp_st_mark_us = 0;
uint32_t flp_st_prev_start = 0;
uint32_t flp_st_cycles = 0;
uint32_t flp_st_sum = 0;
uint32_t flp_st_last = 0;
uint32_t flp_st_max = 0;
uint32_t flp_st_period_max = 0;
{bg_globals}
void flp_st_begin() {{
  uint32_t now = micros();
  if (flp_st_started && now - flp_st_prev_start > flp_st_period_max) {{
    flp_st_period_max = now - flp_st_prev_start;
  }}
  flp_st_started = true;
  flp_st_prev_start = now;
  flp_st_begin_us = now;
  flp_st_mark_us = now;
  flp_st_cur = FLP_ST_SHEETS;
}}

// Execution moves to blocks of sheet k (index in flp_st_sheet_no).
void flp_st_sheet(uint8_t k) {{
  uint32_t now = micros();
  if (flp_st_cur < FLP_ST_SHEETS) {{
    flp_st_acc[flp_st_cur] += now - flp_st_mark_us;
  }}
  flp_st_cur = k;
  flp_st_mark_us = now;
}}

void flp_st_end() {{
  flp_st_sheet(FLP_ST_SHEETS);
  uint32_t t = flp_st_mark_us - flp_st_begin_us;
  flp_st_last = t;
  if (t > flp_st_max) {{
    flp_st_max = t;
  }}
  flp_st_sum += t;
  flp_st_cycles++;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {{
    flp_st_sheet_last[k] = flp_st_acc[k];
    if (flp_st_acc[k] > flp_st_sheet_max[k]) {{
      flp_st_sheet_max[k] = flp_st_acc[k];
    }}
    flp_st_acc[k] = 0;
  }}
}}

void flp_st_reset() {{{bg_reset}
  flp_st_max = 0;
  flp_st_period_max = 0;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {{
    flp_st_sheet_max[k] = 0;
  }}
}}

// Average since the previous call; starts a new averaging interval.
uint32_t flp_st_take_avg() {{
  uint32_t avg = flp_st_cycles ? flp_st_sum / flp_st_cycles : 0;
  flp_st_sum = 0;
  flp_st_cycles = 0;
  return avg;
}}

uint32_t flp_st_free_heap() {{
#ifdef ESP32
  return ESP.getFreeHeap();
#else
  return 0;
#endif
}}
"#,
        title = lang.pick(
            "Диагностика: время цикла (мкс), всего и по листам",
            "Diagnostics: cycle time (us), total and per sheet"
        ),
        n = sheets.len(),
        numbers = numbers.join(", "),
        bg_globals = if background { BG_MODULE } else { "" },
        bg_reset = if background {
            "
  flp_st_bg_max = 0;"
        } else {
            ""
        },
    )
}

/// Text report of normal firmware, printed every `interval_ms`.
pub fn text_report(interval_ms: u32, background: bool, lang: Lang) -> String {
    let t = |ru: &'static str, en: &'static str| lang.pick(ru, en);
    let bg_report = if background {
        format!(
            "
  Serial.print(\"{}\");
  Serial.print(flp_st_bg_last);
  Serial.print(\"{}\");
  Serial.println(flp_st_bg_max);",
            t(
                "[FLP]   ядро 0 (фоновая задача): последний проход ",
                "[FLP]   core 0 (background task): last pass "
            ),
            t(", макс ", ", max ")
        )
    } else {
        String::new()
    };
    let mut out = String::new();
    let _ = write!(
        out,
        r#"
unsigned long flp_st_report_last = 0;

void flp_st_report() {{
  if (millis() - flp_st_report_last < {interval_ms}UL) {{
    return;
  }}
  flp_st_report_last = millis();
  uint32_t cycles = flp_st_cycles;
  uint32_t avg = flp_st_take_avg();
  Serial.print("{cycle}");
  Serial.print(flp_st_last);
  Serial.print("{avg}");
  Serial.print(avg);
  Serial.print("{max}");
  Serial.print(flp_st_max);
  Serial.print("{period}");
  Serial.print(flp_st_period_max);
  Serial.print("{cycles}");
  Serial.print(cycles);
  Serial.print("{heap}");
  Serial.println(flp_st_free_heap());{bg_report}
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {{
    Serial.print("{sheet}");
    Serial.print(flp_st_sheet_no[k]);
    Serial.print("{last}");
    Serial.print(flp_st_sheet_last[k]);
    Serial.print("{max}");
    Serial.println(flp_st_sheet_max[k]);
  }}
}}
"#,
        cycle = t("[FLP] цикл, мкс: последний ", "[FLP] cycle, us: last "),
        avg = t(", средний ", ", average "),
        max = t(", макс ", ", max "),
        period = t(", макс период loop ", ", max loop period "),
        cycles = t("; циклов ", "; cycles "),
        heap = t("; своб. память ", "; free memory "),
        sheet = t("[FLP]   лист ", "[FLP]   sheet "),
        last = t(": последний ", ": last "),
    );
    out
}
