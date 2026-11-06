// Сгенерировано CrabLogic из проекта «tags». Ручные правки будут перезаписаны.
// Порядок выполнения: adc → to_c → temp_w → temp_r → setpoint_r → error → cold → heating_w → heating_r → heater

#include <Arduino.h>

// Выходы блоков
int32_t adc_out = 0;
float to_c_out = 0.0f;
float temp_r_out = 0.0f;
float setpoint_r_out = 0.0f;
float error_out = 0.0f;
bool cold_out = false;
bool heating_r_out = false;

// Переменные
float var_temp = 0.0f;
float var_setpoint = 22.0f;
bool var_heating = false;

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
const uint32_t CRAB_DBG_FINGERPRINT = 0x84B418D3UL;
const uint16_t CRAB_DBG_COUNT = 10;
// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).
void* const crab_dbg_ptr[CRAB_DBG_COUNT + 1] = { &adc_out, &to_c_out, &temp_r_out, &setpoint_r_out, &error_out, &cold_out, &heating_r_out, &var_temp, &var_setpoint, &var_heating, nullptr };
const uint8_t crab_dbg_type[CRAB_DBG_COUNT + 1] = { 1, 2, 2, 2, 2, 0, 0, 2, 2, 0, 0 };  // see type_code in crab-codegen
bool crab_dbg_forced[CRAB_DBG_COUNT + 1];
uint32_t crab_dbg_force_raw[CRAB_DBG_COUNT + 1];
// Образ наблюдаемых значений: от крупных типов к мелким, без дыр; смещения — в crab_dbg_off.
struct __attribute__((packed)) CrabImage {
  int32_t v0;  // adc_out
  float v1;  // to_c_out
  float v2;  // temp_r_out
  float v3;  // setpoint_r_out
  float v4;  // error_out
  float v7;  // var_temp
  float v8;  // var_setpoint
  bool v5;  // cold_out
  bool v6;  // heating_r_out
  bool v9;  // var_heating
};
alignas(8) CrabImage crab_img;
alignas(8) CrabImage crab_img_out;
const uint16_t CRAB_IMG_SIZE = 31;
const uint16_t crab_dbg_off[CRAB_DBG_COUNT + 1] = { 0, 4, 8, 12, 16, 28, 29, 20, 24, 30, 0 };
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
  crab_img.v1 = to_c_out;
  crab_img.v2 = temp_r_out;
  crab_img.v3 = setpoint_r_out;
  crab_img.v4 = error_out;
  crab_img.v7 = var_temp;
  crab_img.v8 = var_setpoint;
  crab_img.v5 = cold_out;
  crab_img.v6 = heating_r_out;
  crab_img.v9 = var_heating;
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
    if (++crab_dbg_ticks >= 5) {
      crab_dbg_ticks = 0;
      crab_dbg_stats();
    }
  }
}

// Отчёт по USB: ядро 0, отдельно от основного цикла
void crab_report_task(void *) {
  for (;;) {
    crab_dbg_loop();
    vTaskDelay(1);
  }
}

void setup() {
  crab_dbg_setup();

  // heater: digital_out
  pinMode(26, OUTPUT);

  // Отчёт стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_report_task, "crab_report", 8192, nullptr, 1, nullptr, 0);
}

void loop() {
  crab_st_begin();

  // Forced variables
  crab_dbg_force(7); crab_dbg_force(8); crab_dbg_force(9);

  crab_st_sheet(0);  // лист 1
  // adc: analog_in
  adc_out = analogRead(34);
  crab_dbg_force(0);

  // to_c: scale
  to_c_out = 0.0f + ((float)adc_out - 0.0f) * (50.0f - 0.0f) / (4095.0f - 0.0f);
  to_c_out = constrain(to_c_out, 0.0f, 50.0f);
  crab_dbg_force(1);

  // temp_w: var_set
  var_temp = to_c_out;
  crab_dbg_force(7);

  crab_st_sheet(1);  // лист 2
  // temp_r: var_get
  temp_r_out = var_temp;
  crab_dbg_force(2);

  // setpoint_r: var_get
  setpoint_r_out = var_setpoint;
  crab_dbg_force(3);

  // error: sub
  error_out = temp_r_out - setpoint_r_out;
  crab_dbg_force(4);

  // cold: less
  cold_out = error_out < -1.0f;
  crab_dbg_force(5);

  // heating_w: var_set
  var_heating = cold_out;
  crab_dbg_force(9);

  // heating_r: var_get
  heating_r_out = var_heating;
  crab_dbg_force(6);

  // heater: digital_out
  digitalWrite(26, (heating_r_out) ? HIGH : LOW);

  crab_img_snapshot();

  crab_st_end();
}
