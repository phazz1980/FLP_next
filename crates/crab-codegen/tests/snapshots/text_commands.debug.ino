// Сгенерировано CrabLogic из проекта «text_commands». Ручные правки будут перезаписаны.
// Порядок выполнения: adc → bar → p_text → name → tick → p_send → port2 → rx → command_w → command_r → message → message_w → length → length_w → report → is_on → pump → head → is_set → arg → number → apply → setpoint_w

#include <Arduino.h>

// Выходы блоков
int32_t adc_out = 0;
float bar_out = 0.0f;
String p_text_out = String();
String name_out = String();
bool tick_out = false;
bool p_send_sent = false;
int32_t port2_baud = 0;
String rx_line = String();
bool rx_ready = false;
int32_t rx_available = 0;
String command_r_out = String();
String message_out = String();
int32_t length_out = 0;
bool report_sent = false;
bool is_on_out = false;
String head_out = String();
bool is_set_out = false;
String arg_out = String();
float number_out = 0.0f;
bool number_ok = false;
bool apply_out = false;

// Переменные
String var_message = String();
int32_t var_message_len = 0;
String var_command = String("SET 2.5");
float var_setpoint = 3.0f;

// tick: generator
unsigned long tick_start = 0;

// p_send: uart_send
float p_send_last;
bool p_send_started = false;

// rx: uart_read
String rx_buf;
unsigned long rx_last = 0;

// report: uart_send
bool report_prev = false;

// text_compare: общий код
static inline char crab_lower(char c) {
  return (c >= 'A' && c <= 'Z') ? (char)(c + 32) : c;
}
bool crab_text_match_nocase(const String& a, const String& b, int mode) {
  int la = a.length(), lb = b.length();
  if (lb > la || (mode == 0 && lb != la)) {
    return false;
  }
  int first = mode == 3 ? la - lb : 0;
  int last = (mode == 1 ? la : first + lb) - lb;
  for (int at = first; at <= last; at++) {
    int i = 0;
    while (i < lb && crab_lower(a[at + i]) == crab_lower(b[i])) {
      i++;
    }
    if (i == lb) {
      return true;
    }
  }
  return false;
}

// text_to_num: общий код
// Number from text: spaces around are skipped, a decimal comma is accepted.
bool crab_text_to_float(const String& text, float& out) {
  String t = text;
  t.trim();
  t.replace(',', '.');
  if (t.length() == 0) {
    return false;
  }
  char* end = nullptr;
  double v = strtod(t.c_str(), &end);
  if (end == t.c_str() || *end != '\0') {
    return false;
  }
  out = (float)v;
  return true;
}

// ---- Диагностика: время цикла (мкс), всего и по листам ----
const uint8_t CRAB_ST_SHEETS = 2;
const uint16_t crab_st_sheet_no[CRAB_ST_SHEETS + 1] = { 1, 2, 0 };
uint32_t crab_st_acc[CRAB_ST_SHEETS + 1];
uint32_t crab_st_sheet_last[CRAB_ST_SHEETS + 1];
uint32_t crab_st_sheet_max[CRAB_ST_SHEETS + 1];
uint8_t crab_st_cur = CRAB_ST_SHEETS;
bool crab_st_started = false;
uint32_t crab_st_begin_us = 0;
uint32_t crab_st_mark_us = 0;
uint32_t crab_st_prev_start = 0;
uint32_t crab_st_cycles = 0;
uint32_t crab_st_sum = 0;
uint32_t crab_st_last = 0;
uint32_t crab_st_max = 0;
uint32_t crab_st_period_max = 0;

void crab_st_begin() {
  uint32_t now = micros();
  if (crab_st_started && now - crab_st_prev_start > crab_st_period_max) {
    crab_st_period_max = now - crab_st_prev_start;
  }
  crab_st_started = true;
  crab_st_prev_start = now;
  crab_st_begin_us = now;
  crab_st_mark_us = now;
  crab_st_cur = CRAB_ST_SHEETS;
}

// Execution moves to blocks of sheet k (index in crab_st_sheet_no).
void crab_st_sheet(uint8_t k) {
  uint32_t now = micros();
  if (crab_st_cur < CRAB_ST_SHEETS) {
    crab_st_acc[crab_st_cur] += now - crab_st_mark_us;
  }
  crab_st_cur = k;
  crab_st_mark_us = now;
}

void crab_st_end() {
  crab_st_sheet(CRAB_ST_SHEETS);
  uint32_t t = crab_st_mark_us - crab_st_begin_us;
  crab_st_last = t;
  if (t > crab_st_max) {
    crab_st_max = t;
  }
  crab_st_sum += t;
  crab_st_cycles++;
  for (uint8_t k = 0; k < CRAB_ST_SHEETS; k++) {
    crab_st_sheet_last[k] = crab_st_acc[k];
    if (crab_st_acc[k] > crab_st_sheet_max[k]) {
      crab_st_sheet_max[k] = crab_st_acc[k];
    }
    crab_st_acc[k] = 0;
  }
}

void crab_st_reset() {
  crab_st_max = 0;
  crab_st_period_max = 0;
  for (uint8_t k = 0; k < CRAB_ST_SHEETS; k++) {
    crab_st_sheet_max[k] = 0;
  }
}

// Average since the previous call; starts a new averaging interval.
uint32_t crab_st_take_avg() {
  uint32_t avg = crab_st_cycles ? crab_st_sum / crab_st_cycles : 0;
  crab_st_sum = 0;
  crab_st_cycles = 0;
  return avg;
}

uint32_t crab_st_free_heap() {
#ifdef ESP32
  return ESP.getFreeHeap();
#else
  return 0;
#endif
}

// ---- Онлайн-отладка CrabLogic (протокол v5) ----
// Кадры: 0xAA 0x55, тип, длина, данные, CRC-16/CCITT. Обычный Serial.print не мешает.
#include <mbedtls/md.h>
const uint32_t CRAB_DBG_FINGERPRINT = 0xD1DEAEFFUL;
const uint16_t CRAB_DBG_COUNT = 16;
// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).
void* const crab_dbg_ptr[CRAB_DBG_COUNT + 1] = { &adc_out, &bar_out, &tick_out, &p_send_sent, &port2_baud, &rx_ready, &rx_available, &length_out, &report_sent, &is_on_out, &is_set_out, &number_out, &number_ok, &apply_out, &var_message_len, &var_setpoint, nullptr };
const uint8_t crab_dbg_type[CRAB_DBG_COUNT + 1] = { 1, 2, 0, 0, 1, 0, 1, 1, 0, 0, 0, 2, 0, 0, 1, 2, 0 };  // see type_code in crab-codegen
bool crab_dbg_forced[CRAB_DBG_COUNT + 1];
uint32_t crab_dbg_force_raw[CRAB_DBG_COUNT + 1];
// Образ наблюдаемых значений: от крупных типов к мелким, без дыр; смещения — в crab_dbg_off.
struct __attribute__((packed)) CrabImage {
  int32_t v0;  // adc_out
  float v1;  // bar_out
  int32_t v4;  // port2_baud
  int32_t v6;  // rx_available
  int32_t v7;  // length_out
  float v11;  // number_out
  int32_t v14;  // var_message_len
  float v15;  // var_setpoint
  bool v2;  // tick_out
  bool v3;  // p_send_sent
  bool v5;  // rx_ready
  bool v8;  // report_sent
  bool v9;  // is_on_out
  bool v10;  // is_set_out
  bool v12;  // number_ok
  bool v13;  // apply_out
};
alignas(8) CrabImage crab_img;
alignas(8) CrabImage crab_img_out;
const uint16_t CRAB_IMG_SIZE = 40;
const uint16_t crab_dbg_off[CRAB_DBG_COUNT + 1] = { 0, 4, 32, 33, 8, 34, 12, 16, 35, 36, 37, 20, 38, 39, 24, 28, 0 };
#ifdef ARDUINO_ARCH_ESP32
portMUX_TYPE crab_img_mux = portMUX_INITIALIZER_UNLOCKED;
#define CRAB_IMG_LOCK() portENTER_CRITICAL(&crab_img_mux)
#define CRAB_IMG_UNLOCK() portEXIT_CRITICAL(&crab_img_mux)
#else
#define CRAB_IMG_LOCK()
#define CRAB_IMG_UNLOCK()
#endif

// End of a scan: all watched values are copied at one moment.
void crab_img_snapshot() {
  CRAB_IMG_LOCK();
  crab_img.v0 = adc_out;
  crab_img.v1 = bar_out;
  crab_img.v4 = port2_baud;
  crab_img.v6 = rx_available;
  crab_img.v7 = length_out;
  crab_img.v11 = number_out;
  crab_img.v14 = var_message_len;
  crab_img.v15 = var_setpoint;
  crab_img.v2 = tick_out;
  crab_img.v3 = p_send_sent;
  crab_img.v5 = rx_ready;
  crab_img.v8 = report_sent;
  crab_img.v9 = is_on_out;
  crab_img.v10 = is_set_out;
  crab_img.v12 = number_ok;
  crab_img.v13 = apply_out;
  CRAB_IMG_UNLOCK();
}
uint8_t crab_dbg_rx[4 + 255 + 2];
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

bool crab_dbg_locked() {
  return &CRAB_AUTH_ON != nullptr && !crab_dbg_authed;
}

uint16_t crab_dbg_crc(uint16_t crc, uint8_t b) {
  crc ^= (uint16_t)b << 8;
  for (uint8_t i = 0; i < 8; i++) {
    crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
  }
  return crc;
}

void crab_dbg_frame(uint8_t type, const uint8_t* data, uint8_t len) {
  uint16_t crc = crab_dbg_crc(crab_dbg_crc(0xFFFF, type), len);
  for (uint8_t i = 0; i < len; i++) {
    crc = crab_dbg_crc(crc, data[i]);
  }
  const uint8_t head[4] = {0xAA, 0x55, type, len};
  const uint8_t tail[2] = {(uint8_t)(crc & 0xFF), (uint8_t)(crc >> 8)};
  Serial.write(head, 4);
  if (len > 0) {
    Serial.write(data, len);
  }
  Serial.write(tail, 2);
}

void crab_dbg_put(uint16_t i, uint32_t raw) {
  void* p = crab_dbg_ptr[i];
  int32_t v = (int32_t)raw;
  switch (crab_dbg_type[i]) {
    case 0: *(bool*)p = raw != 0; break;
    case 3: *(int8_t*)p = (int8_t)v; break;
    case 4: *(uint8_t*)p = (uint8_t)v; break;
    case 5: *(int16_t*)p = (int16_t)v; break;
    case 6: *(uint16_t*)p = (uint16_t)v; break;
    case 8: *(int64_t*)p = v; break;
    case 9: *(uint64_t*)p = (uint32_t)raw; break;
    case 10: { float f; memcpy(&f, &raw, 4); *(double*)p = f; break; }
    default: memcpy(p, &raw, 4);
  }
}

// Called after the block that produces value i: a forced value wins.
void crab_dbg_force(uint16_t i) {
  if (crab_dbg_forced[i]) {
    crab_dbg_put(i, crab_dbg_force_raw[i]);
  }
}

// Identity of the project this firmware was built from (see `CRAB_PROJECT_HASH` at the
// end of the sketch); weak, so a build without it still links. Zeros are not shown to a
// host that has not signed in.
extern const uint8_t CRAB_PROJECT_HASH[8] __attribute__((weak));

void crab_dbg_hello() {
  uint8_t p[18] = {0};
  memcpy(p, &CRAB_DBG_FINGERPRINT, 4);
  p[4] = CRAB_DBG_COUNT & 0xFF;
  p[5] = CRAB_DBG_COUNT >> 8;
  p[6] = 5;
  p[7] = CRAB_IMG_SIZE & 0xFF;
  p[8] = CRAB_IMG_SIZE >> 8;
  p[9] = crab_dbg_locked() ? 1 : 0;
  if (&CRAB_PROJECT_HASH != nullptr && !crab_dbg_locked()) {
    memcpy(p + 10, CRAB_PROJECT_HASH, 8);
  }
  crab_dbg_frame(0x01, p, 18);
}

// The image of one scan in pieces: seq u16, offset u16, bytes.
void crab_dbg_image() {
  uint8_t p[4 + 240];
  CRAB_IMG_LOCK();
  memcpy(&crab_img_out, &crab_img, sizeof(crab_img));
  CRAB_IMG_UNLOCK();
  crab_dbg_seq++;
  for (uint16_t off = 0; off < CRAB_IMG_SIZE; off += 240) {
    uint16_t n = (CRAB_IMG_SIZE - off) > 240 ? 240 : (CRAB_IMG_SIZE - off);
    p[0] = crab_dbg_seq & 0xFF;
    p[1] = crab_dbg_seq >> 8;
    p[2] = off & 0xFF;
    p[3] = off >> 8;
    memcpy(p + 4, (uint8_t*)&crab_img_out + off, n);
    crab_dbg_frame(0x05, p, 4 + n);
  }
}

// Строки: кадр TEXT, только когда строка изменилась (до 64 байт).
const uint16_t CRAB_DBG_TEXTS = 9;
String* const crab_dbg_text[CRAB_DBG_TEXTS] = { &p_text_out, &name_out, &rx_line, &command_r_out, &message_out, &head_out, &arg_out, &var_message, &var_command };
uint16_t crab_dbg_text_crc[CRAB_DBG_TEXTS];
bool crab_dbg_text_sent[CRAB_DBG_TEXTS];
uint16_t crab_dbg_text_next = 0;

void crab_dbg_texts_reset() {
  for (uint16_t i = 0; i < CRAB_DBG_TEXTS; i++) {
    crab_dbg_text_sent[i] = false;
  }
}

void crab_dbg_texts() {
  uint8_t p[2 + 64];
  uint8_t budget = 8;
  for (uint16_t k = 0; k < CRAB_DBG_TEXTS && budget > 0; k++) {
    uint16_t i = crab_dbg_text_next;
    crab_dbg_text_next = (i + 1) % CRAB_DBG_TEXTS;
    const String& s = *crab_dbg_text[i];
    uint16_t len = s.length();
    if (len > 64) {
      len = 64;
      // Do not cut a UTF-8 character in half.
      while (len > 0 && ((uint8_t)s[len] & 0xC0) == 0x80) {
        len--;
      }
    }
    uint16_t crc = crab_dbg_crc(0xFFFF, (uint8_t)len);
    for (uint16_t j = 0; j < len; j++) {
      crc = crab_dbg_crc(crc, (uint8_t)s[j]);
    }
    if (crab_dbg_text_sent[i] && crc == crab_dbg_text_crc[i]) {
      continue;
    }
    crab_dbg_text_crc[i] = crc;
    crab_dbg_text_sent[i] = true;
    p[0] = i & 0xFF;
    p[1] = i >> 8;
    memcpy(p + 2, s.c_str(), len);
    crab_dbg_frame(0x04, p, 2 + len);
    budget--;
  }
}

void crab_dbg_stats() {
  uint8_t p[25 + 10 * CRAB_ST_SHEETS + 8];
  uint32_t head[6] = {crab_st_cycles, crab_st_last, crab_st_max, 0, crab_st_period_max, crab_st_free_heap()};
  head[3] = crab_st_take_avg();
  memcpy(p, head, 24);
  p[24] = CRAB_ST_SHEETS;
  for (uint8_t k = 0; k < CRAB_ST_SHEETS; k++) {
    uint8_t* q = p + 25 + 10 * k;
    q[0] = crab_st_sheet_no[k] & 0xFF;
    q[1] = crab_st_sheet_no[k] >> 8;
    memcpy(q + 2, &crab_st_sheet_last[k], 4);
    memcpy(q + 6, &crab_st_sheet_max[k], 4);
  }
  crab_dbg_frame(0x03, p, 25 + 10 * CRAB_ST_SHEETS);
}

// The project archive in flash (see `CRAB_PROJECT` at the end of the sketch); weak,
// so a sketch generated without it still links and answers "no project".
extern const uint8_t CRAB_PROJECT[] PROGMEM __attribute__((weak));
extern const uint32_t CRAB_PROJECT_SIZE __attribute__((weak));

// One piece of the project: total u32, offset u32, bytes.
void crab_dbg_project(const uint8_t* p, uint8_t len) {
  uint32_t total = (&CRAB_PROJECT_SIZE != nullptr) ? CRAB_PROJECT_SIZE : 0;
  uint32_t off = 0;
  if (len >= 4) {
    memcpy(&off, p, 4);
  }
  uint32_t n = off < total ? total - off : 0;
  if (n > 240) {
    n = 240;
  }
  uint8_t out[8 + 240];
  memcpy(out, &total, 4);
  memcpy(out + 4, &off, 4);
  if (n > 0) {
    memcpy_P(out + 8, CRAB_PROJECT + off, n);
  }
  crab_dbg_frame(0x06, out, 8 + n);
}

// Password check: a challenge (salt + random nonce), then HMAC-SHA256 of the nonce.
void crab_dbg_auth_challenge() {
  uint8_t out[32];
  for (uint8_t k = 0; k < 16; k += 4) {
    uint32_t r = esp_random();
    memcpy(crab_dbg_nonce + k, &r, 4);
  }
  crab_dbg_nonce_ok = true;
  memcpy(out, CRAB_AUTH_SALT, 16);
  memcpy(out + 16, crab_dbg_nonce, 16);
  crab_dbg_frame(0x07, out, 32);
}

void crab_dbg_auth_check(const uint8_t* p, uint8_t len) {
  if (!crab_dbg_nonce_ok || len != 32 || (long)(millis() - crab_dbg_auth_block) < 0) {
    return;
  }
  crab_dbg_nonce_ok = false;
  uint8_t proof[32];
  mbedtls_md_hmac(mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), CRAB_AUTH_KEY, 32,
                  crab_dbg_nonce, 16, proof);
  uint8_t diff = 0;
  for (uint8_t k = 0; k < 32; k++) {
    diff |= proof[k] ^ p[k];
  }
  if (diff == 0) {
    crab_dbg_authed = true;
    crab_dbg_auth_seen = millis();
    crab_dbg_frame(0x08, nullptr, 0);
    crab_dbg_hello();
  } else {
    crab_dbg_auth_block = millis() + 1000;
    crab_dbg_frame(0x09, nullptr, 0);
  }
}

void crab_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {
  if (&CRAB_AUTH_ON != nullptr) {
    if (type == 0x17) {
      crab_dbg_authed = false;  // also the way to sign out
      crab_dbg_auth_challenge();
      return;
    }
    if (type == 0x18) {
      crab_dbg_auth_check(p, len);
      return;
    }
    if (crab_dbg_authed) {
      crab_dbg_auth_seen = millis();
    } else {
      if (type == 0x10) {
        crab_dbg_hello();
      }
      return;
    }
    if (type == 0x19) {
      return;
    }
  }
  if (type == 0x16) {
    crab_dbg_project(p, len);
    return;
  }
  if (type == 0x10) {
    crab_dbg_hello();
    crab_dbg_texts_reset();
    return;
  }
  if (type == 0x14) {
    crab_st_reset();
    return;
  }
  if (type == 0x13) {
    for (uint16_t i = 0; i < CRAB_DBG_COUNT; i++) {
      crab_dbg_forced[i] = false;
    }
    return;
  }
  if (len < 2) {
    return;
  }
  uint16_t i = p[0] | (p[1] << 8);
  if (i >= CRAB_DBG_COUNT) {
    return;
  }
  uint32_t raw;
  if (type == 0x11 && len >= 7) {
    memcpy(&raw, p + 3, 4);
    crab_dbg_forced[i] = p[2] != 0;
    crab_dbg_force_raw[i] = raw;
    crab_dbg_force(i);
  } else if (type == 0x12 && len >= 6) {
    memcpy(&raw, p + 2, 4);
    crab_dbg_put(i, raw);
  }
}

void crab_dbg_poll() {
  while (Serial.available() > 0) {
    uint8_t b = Serial.read();
    if (crab_dbg_rx_len == 0 && b != 0xAA) {
      continue;
    }
    if (crab_dbg_rx_len == 1 && b != 0x55) {
      crab_dbg_rx_len = (b == 0xAA) ? 1 : 0;
      continue;
    }
    crab_dbg_rx[crab_dbg_rx_len++] = b;
    if (crab_dbg_rx_len >= 4 && crab_dbg_rx_len == 4 + crab_dbg_rx[3] + 2) {
      uint16_t end = crab_dbg_rx_len - 2;
      uint16_t crc = 0xFFFF;
      for (uint16_t k = 2; k < end; k++) {
        crc = crab_dbg_crc(crc, crab_dbg_rx[k]);
      }
      if ((crc & 0xFF) == crab_dbg_rx[end] && (crc >> 8) == crab_dbg_rx[end + 1]) {
        crab_dbg_command(crab_dbg_rx[2], crab_dbg_rx + 4, crab_dbg_rx[3]);
      }
      crab_dbg_rx_len = 0;
    }
  }
}

void crab_dbg_setup() {
  Serial.begin(115200);
  crab_dbg_hello();
}

void crab_dbg_loop() {
  crab_dbg_poll();
  if (crab_dbg_authed && millis() - crab_dbg_auth_seen > 3000UL) {
    crab_dbg_authed = false;
    crab_dbg_hello();
  }
  if (crab_dbg_locked()) {
    return;
  }
  if (millis() - crab_dbg_last >= 100) {
    crab_dbg_last = millis();
    crab_dbg_image();
    crab_dbg_texts();
    if (++crab_dbg_ticks >= 5) {
      crab_dbg_ticks = 0;
      crab_dbg_stats();
    }
  }
}

void setup() {
  crab_dbg_setup();

  // UART2: 9600 бод, 8N1, RX GPIO16, TX GPIO17
  Serial2.setRxBufferSize(256);
  Serial2.begin(9600, SERIAL_8N1, 16, 17);

  // Память под строки заранее: меньше дробления кучи.
  p_text_out.reserve(20);
  name_out.reserve(10);
  rx_line.reserve(32);
  command_r_out.reserve(32);
  message_out.reserve(32);
  head_out.reserve(32);
  arg_out.reserve(32);
  var_message.reserve(32);
  var_command.reserve(32);

  // name: text_const
  name_out = "насос";

  // rx: uart_read
  rx_buf.reserve(64);

  // pump: digital_out
  pinMode(26, OUTPUT);
}

void loop() {
  crab_st_begin();

  // Forced variables
  crab_dbg_force(14); crab_dbg_force(15);

  crab_st_sheet(0);  // лист 1
  // adc: analog_in
  adc_out = analogRead(34);
  crab_dbg_force(0);

  // bar: scale
  bar_out = 0.0f + ((float)adc_out - 0.0f) * (10.0f - 0.0f) / (4095.0f - 0.0f);
  bar_out = constrain(bar_out, 0.0f, 10.0f);
  crab_dbg_force(1);

  // p_text: num_to_text
  p_text_out = "P=";
  p_text_out += String(bar_out, 1);
  p_text_out += " бар";

  // tick: generator
  if (true) {
    unsigned long phase = (millis() - tick_start) % (1000UL);
    tick_out = phase < 500UL;
  } else {
    tick_start = millis();
    tick_out = false;
  }
  crab_dbg_force(2);

  // p_send: uart_send
  {
    // On change; the first value after start goes out too.
    bool changed = !p_send_started || fabsf(bar_out - p_send_last) >= 0.05f;
    bool fire = true && changed;
    p_send_sent = false;
    if (fire) {
      float data = bar_out;
      if ((size_t)Serial2.availableForWrite() >= sizeof(data)) {
        Serial2.write((const uint8_t*)&data, sizeof(data));
        p_send_sent = true;
      }
      if (p_send_sent) {
        p_send_last = bar_out;
        p_send_started = true;
      }
    }
  }
  crab_dbg_force(3);

  crab_st_sheet(1);  // лист 2
  // port2: uart_config
  port2_baud = (int32_t)Serial2.baudRate();
  crab_dbg_force(4);

  // rx: uart_read
  rx_ready = false;
  // One line per cycle: the next one waits in the UART buffer.
  while (Serial2.available() > 0) {
    char c = (char)Serial2.read();
    rx_last = millis();
    if (c == '\r') {
      continue;
    }
    bool end = c == '\n';
    if (!end) {
      rx_buf += c;
      end = rx_buf.length() >= 64;
    }
    if (end) {
      rx_line = rx_buf;
      rx_buf = "";
      rx_ready = true;
      break;
    }
  }
  rx_available = Serial2.available();
  crab_dbg_force(6);
  crab_dbg_force(5);

  // command_w: var_set
  if (rx_ready) {
    var_command = rx_line;
  }

  // command_r: var_get
  command_r_out = var_command;

  crab_st_sheet(0);  // лист 1
  // message: text_concat
  message_out = name_out;
  message_out += "; ";
  message_out += p_text_out;
  message_out += "; ";
  message_out += command_r_out;

  // message_w: var_set
  var_message = message_out;

  // length: text_length
  length_out = (int32_t)message_out.length();
  crab_dbg_force(7);

  // length_w: var_set
  var_message_len = length_out;
  crab_dbg_force(14);

  // report: uart_send
  {
    bool fire = tick_out && !report_prev;
    report_prev = tick_out;
    report_sent = false;
    if (fire) {
      const String& text = message_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        report_sent = true;
      }
    }
  }
  crab_dbg_force(8);

  crab_st_sheet(1);  // лист 2
  // is_on: text_compare
  is_on_out = crab_text_match_nocase(command_r_out, "ON", 0);
  crab_dbg_force(9);

  // pump: digital_out
  digitalWrite(26, (is_on_out) ? HIGH : LOW);

  // head: text_substring
  head_out = command_r_out.substring(0, 0 + 4);

  // is_set: text_compare
  is_set_out = head_out.startsWith("SET ");
  crab_dbg_force(10);

  // arg: text_substring
  arg_out = command_r_out.substring(4);

  // number: text_to_num
  {
    float v;
    number_ok = crab_text_to_float(arg_out, v);
    if (number_ok) {
      number_out = v;
    }
  }
  crab_dbg_force(12);
  crab_dbg_force(11);

  // apply: and
  apply_out = is_set_out && number_ok;
  crab_dbg_force(13);

  // setpoint_w: var_set
  if (apply_out) {
    var_setpoint = number_out;
  }
  crab_dbg_force(15);

  crab_img_snapshot();

  crab_st_end();
  crab_dbg_loop();
}
