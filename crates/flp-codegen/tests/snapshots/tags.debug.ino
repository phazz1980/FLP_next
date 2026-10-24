// Сгенерировано FLP_next из проекта «tags». Ручные правки будут перезаписаны.
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

// ---- Онлайн-отладка FLP_next (протокол v1) ----
// Кадры: 0xAA 0x55, тип, длина, данные, CRC-16/CCITT. Обычный Serial.print не мешает.
const uint32_t FLP_DBG_FINGERPRINT = 0x84B418D3UL;
const uint16_t FLP_DBG_COUNT = 10;
// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).
void* const flp_dbg_ptr[FLP_DBG_COUNT + 1] = { &adc_out, &to_c_out, &temp_r_out, &setpoint_r_out, &error_out, &cold_out, &heating_r_out, &var_temp, &var_setpoint, &var_heating, nullptr };
const uint8_t flp_dbg_type[FLP_DBG_COUNT + 1] = { 1, 2, 2, 2, 2, 0, 0, 2, 2, 0, 0 };  // see type_code in flp-codegen
bool flp_dbg_forced[FLP_DBG_COUNT + 1];
uint32_t flp_dbg_force_raw[FLP_DBG_COUNT + 1];
uint8_t flp_dbg_rx[4 + 255 + 2];
uint16_t flp_dbg_rx_len = 0;
uint16_t flp_dbg_seq = 0;
unsigned long flp_dbg_last = 0;

uint16_t flp_dbg_crc(uint16_t crc, uint8_t b) {
  crc ^= (uint16_t)b << 8;
  for (uint8_t i = 0; i < 8; i++) {
    crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
  }
  return crc;
}

void flp_dbg_frame(uint8_t type, const uint8_t* data, uint8_t len) {
  uint16_t crc = flp_dbg_crc(flp_dbg_crc(0xFFFF, type), len);
  for (uint8_t i = 0; i < len; i++) {
    crc = flp_dbg_crc(crc, data[i]);
  }
  const uint8_t head[4] = {0xAA, 0x55, type, len};
  const uint8_t tail[2] = {(uint8_t)(crc & 0xFF), (uint8_t)(crc >> 8)};
  Serial.write(head, 4);
  if (len > 0) {
    Serial.write(data, len);
  }
  Serial.write(tail, 2);
}

uint32_t flp_dbg_get(uint16_t i) {
  uint32_t raw = 0;
  void* p = flp_dbg_ptr[i];
  switch (flp_dbg_type[i]) {
    case 0: raw = *(bool*)p ? 1 : 0; break;
    case 3: raw = (uint32_t)(int32_t)*(int8_t*)p; break;
    case 4: raw = *(uint8_t*)p; break;
    case 5: raw = (uint32_t)(int32_t)*(int16_t*)p; break;
    case 6: raw = *(uint16_t*)p; break;
    case 8: raw = (uint32_t)(int32_t)*(int64_t*)p; break;
    case 9: raw = (uint32_t)*(uint64_t*)p; break;
    case 10: { float f = (float)*(double*)p; memcpy(&raw, &f, 4); break; }
    default: memcpy(&raw, p, 4);
  }
  return raw;
}

void flp_dbg_put(uint16_t i, uint32_t raw) {
  void* p = flp_dbg_ptr[i];
  int32_t v = (int32_t)raw;
  switch (flp_dbg_type[i]) {
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
void flp_dbg_force(uint16_t i) {
  if (flp_dbg_forced[i]) {
    flp_dbg_put(i, flp_dbg_force_raw[i]);
  }
}

void flp_dbg_hello() {
  uint8_t p[7];
  memcpy(p, &FLP_DBG_FINGERPRINT, 4);
  p[4] = FLP_DBG_COUNT & 0xFF;
  p[5] = FLP_DBG_COUNT >> 8;
  p[6] = 1;
  flp_dbg_frame(0x01, p, 7);
}

void flp_dbg_values() {
  uint8_t p[5 + 4 * 60];
  flp_dbg_seq++;
  for (uint16_t start = 0; start < FLP_DBG_COUNT; start += 60) {
    uint8_t n = (FLP_DBG_COUNT - start) > 60 ? 60 : (uint8_t)(FLP_DBG_COUNT - start);
    p[0] = flp_dbg_seq & 0xFF;
    p[1] = flp_dbg_seq >> 8;
    p[2] = start & 0xFF;
    p[3] = start >> 8;
    p[4] = n;
    for (uint8_t k = 0; k < n; k++) {
      uint32_t raw = flp_dbg_get(start + k);
      memcpy(p + 5 + 4 * k, &raw, 4);
    }
    flp_dbg_frame(0x02, p, 5 + 4 * n);
  }
}

void flp_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {
  if (type == 0x10) {
    flp_dbg_hello();
    return;
  }
  if (type == 0x13) {
    for (uint16_t i = 0; i < FLP_DBG_COUNT; i++) {
      flp_dbg_forced[i] = false;
    }
    return;
  }
  if (len < 2) {
    return;
  }
  uint16_t i = p[0] | (p[1] << 8);
  if (i >= FLP_DBG_COUNT) {
    return;
  }
  uint32_t raw;
  if (type == 0x11 && len >= 7) {
    memcpy(&raw, p + 3, 4);
    flp_dbg_forced[i] = p[2] != 0;
    flp_dbg_force_raw[i] = raw;
    flp_dbg_force(i);
  } else if (type == 0x12 && len >= 6) {
    memcpy(&raw, p + 2, 4);
    flp_dbg_put(i, raw);
  }
}

void flp_dbg_poll() {
  while (Serial.available() > 0) {
    uint8_t b = Serial.read();
    if (flp_dbg_rx_len == 0 && b != 0xAA) {
      continue;
    }
    if (flp_dbg_rx_len == 1 && b != 0x55) {
      flp_dbg_rx_len = (b == 0xAA) ? 1 : 0;
      continue;
    }
    flp_dbg_rx[flp_dbg_rx_len++] = b;
    if (flp_dbg_rx_len >= 4 && flp_dbg_rx_len == 4 + flp_dbg_rx[3] + 2) {
      uint16_t end = flp_dbg_rx_len - 2;
      uint16_t crc = 0xFFFF;
      for (uint16_t k = 2; k < end; k++) {
        crc = flp_dbg_crc(crc, flp_dbg_rx[k]);
      }
      if ((crc & 0xFF) == flp_dbg_rx[end] && (crc >> 8) == flp_dbg_rx[end + 1]) {
        flp_dbg_command(flp_dbg_rx[2], flp_dbg_rx + 4, flp_dbg_rx[3]);
      }
      flp_dbg_rx_len = 0;
    }
  }
}

void flp_dbg_setup() {
  Serial.begin(115200);
  flp_dbg_hello();
}

void flp_dbg_loop() {
  flp_dbg_poll();
  if (millis() - flp_dbg_last >= 100) {
    flp_dbg_last = millis();
    flp_dbg_values();
  }
}

void setup() {
  flp_dbg_setup();

  // heater: digital_out
  pinMode(26, OUTPUT);
}

void loop() {
  // Forced variables
  flp_dbg_force(7); flp_dbg_force(8); flp_dbg_force(9);

  // adc: analog_in
  adc_out = analogRead(34);
  flp_dbg_force(0);

  // to_c: scale
  to_c_out = 0.0f + ((float)adc_out - 0.0f) * (50.0f - 0.0f) / (4095.0f - 0.0f);
  to_c_out = constrain(to_c_out, 0.0f, 50.0f);
  flp_dbg_force(1);

  // temp_w: var_set
  var_temp = to_c_out;
  flp_dbg_force(7);

  // temp_r: var_get
  temp_r_out = var_temp;
  flp_dbg_force(2);

  // setpoint_r: var_get
  setpoint_r_out = var_setpoint;
  flp_dbg_force(3);

  // error: sub
  error_out = temp_r_out - setpoint_r_out;
  flp_dbg_force(4);

  // cold: less
  cold_out = error_out < -1.0f;
  flp_dbg_force(5);

  // heating_w: var_set
  var_heating = cold_out;
  flp_dbg_force(9);

  // heating_r: var_get
  heating_r_out = var_heating;
  flp_dbg_force(6);

  // heater: digital_out
  digitalWrite(26, (heating_r_out) ? HIGH : LOW);

  flp_dbg_loop();
}
