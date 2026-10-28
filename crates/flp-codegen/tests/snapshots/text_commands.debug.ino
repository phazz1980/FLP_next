// Сгенерировано FLP_next из проекта «text_commands». Ручные правки будут перезаписаны.
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

// text_to_num: общий код
// Number from text: spaces around are skipped, a decimal comma is accepted.
bool flp_text_to_float(const String& text, float& out) {
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
const uint8_t FLP_ST_SHEETS = 2;
const uint16_t flp_st_sheet_no[FLP_ST_SHEETS + 1] = { 1, 2, 0 };
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

void flp_st_begin() {
  uint32_t now = micros();
  if (flp_st_started && now - flp_st_prev_start > flp_st_period_max) {
    flp_st_period_max = now - flp_st_prev_start;
  }
  flp_st_started = true;
  flp_st_prev_start = now;
  flp_st_begin_us = now;
  flp_st_mark_us = now;
  flp_st_cur = FLP_ST_SHEETS;
}

// Execution moves to blocks of sheet k (index in flp_st_sheet_no).
void flp_st_sheet(uint8_t k) {
  uint32_t now = micros();
  if (flp_st_cur < FLP_ST_SHEETS) {
    flp_st_acc[flp_st_cur] += now - flp_st_mark_us;
  }
  flp_st_cur = k;
  flp_st_mark_us = now;
}

void flp_st_end() {
  flp_st_sheet(FLP_ST_SHEETS);
  uint32_t t = flp_st_mark_us - flp_st_begin_us;
  flp_st_last = t;
  if (t > flp_st_max) {
    flp_st_max = t;
  }
  flp_st_sum += t;
  flp_st_cycles++;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {
    flp_st_sheet_last[k] = flp_st_acc[k];
    if (flp_st_acc[k] > flp_st_sheet_max[k]) {
      flp_st_sheet_max[k] = flp_st_acc[k];
    }
    flp_st_acc[k] = 0;
  }
}

void flp_st_reset() {
  flp_st_max = 0;
  flp_st_period_max = 0;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {
    flp_st_sheet_max[k] = 0;
  }
}

// Average since the previous call; starts a new averaging interval.
uint32_t flp_st_take_avg() {
  uint32_t avg = flp_st_cycles ? flp_st_sum / flp_st_cycles : 0;
  flp_st_sum = 0;
  flp_st_cycles = 0;
  return avg;
}

uint32_t flp_st_free_heap() {
#ifdef ESP32
  return ESP.getFreeHeap();
#else
  return 0;
#endif
}

// ---- Онлайн-отладка FLP_next (протокол v2) ----
// Кадры: 0xAA 0x55, тип, длина, данные, CRC-16/CCITT. Обычный Serial.print не мешает.
const uint32_t FLP_DBG_FINGERPRINT = 0xD1DEAEFFUL;
const uint16_t FLP_DBG_COUNT = 16;
// Наблюдаемые значения: выходы блоков, затем переменные (+ пустой элемент в конце).
void* const flp_dbg_ptr[FLP_DBG_COUNT + 1] = { &adc_out, &bar_out, &tick_out, &p_send_sent, &port2_baud, &rx_ready, &rx_available, &length_out, &report_sent, &is_on_out, &is_set_out, &number_out, &number_ok, &apply_out, &var_message_len, &var_setpoint, nullptr };
const uint8_t flp_dbg_type[FLP_DBG_COUNT + 1] = { 1, 2, 0, 0, 1, 0, 1, 1, 0, 0, 0, 2, 0, 0, 1, 2, 0 };  // see type_code in flp-codegen
bool flp_dbg_forced[FLP_DBG_COUNT + 1];
uint32_t flp_dbg_force_raw[FLP_DBG_COUNT + 1];
uint8_t flp_dbg_rx[4 + 255 + 2];
uint16_t flp_dbg_rx_len = 0;
uint16_t flp_dbg_seq = 0;
unsigned long flp_dbg_last = 0;
uint8_t flp_dbg_ticks = 0;

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
  p[6] = 2;
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

// Строки: кадр TEXT, только когда строка изменилась (до 64 байт).
const uint16_t FLP_DBG_TEXTS = 9;
String* const flp_dbg_text[FLP_DBG_TEXTS] = { &p_text_out, &name_out, &rx_line, &command_r_out, &message_out, &head_out, &arg_out, &var_message, &var_command };
uint16_t flp_dbg_text_crc[FLP_DBG_TEXTS];
bool flp_dbg_text_sent[FLP_DBG_TEXTS];
uint16_t flp_dbg_text_next = 0;

void flp_dbg_texts_reset() {
  for (uint16_t i = 0; i < FLP_DBG_TEXTS; i++) {
    flp_dbg_text_sent[i] = false;
  }
}

void flp_dbg_texts() {
  uint8_t p[2 + 64];
  uint8_t budget = 8;
  for (uint16_t k = 0; k < FLP_DBG_TEXTS && budget > 0; k++) {
    uint16_t i = flp_dbg_text_next;
    flp_dbg_text_next = (i + 1) % FLP_DBG_TEXTS;
    const String& s = *flp_dbg_text[i];
    uint16_t len = s.length();
    if (len > 64) {
      len = 64;
      // Do not cut a UTF-8 character in half.
      while (len > 0 && ((uint8_t)s[len] & 0xC0) == 0x80) {
        len--;
      }
    }
    uint16_t crc = flp_dbg_crc(0xFFFF, (uint8_t)len);
    for (uint16_t j = 0; j < len; j++) {
      crc = flp_dbg_crc(crc, (uint8_t)s[j]);
    }
    if (flp_dbg_text_sent[i] && crc == flp_dbg_text_crc[i]) {
      continue;
    }
    flp_dbg_text_crc[i] = crc;
    flp_dbg_text_sent[i] = true;
    p[0] = i & 0xFF;
    p[1] = i >> 8;
    memcpy(p + 2, s.c_str(), len);
    flp_dbg_frame(0x04, p, 2 + len);
    budget--;
  }
}

void flp_dbg_stats() {
  uint8_t p[25 + 10 * FLP_ST_SHEETS + 8];
  uint32_t head[6] = {flp_st_cycles, flp_st_last, flp_st_max, 0, flp_st_period_max, flp_st_free_heap()};
  head[3] = flp_st_take_avg();
  memcpy(p, head, 24);
  p[24] = FLP_ST_SHEETS;
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {
    uint8_t* q = p + 25 + 10 * k;
    q[0] = flp_st_sheet_no[k] & 0xFF;
    q[1] = flp_st_sheet_no[k] >> 8;
    memcpy(q + 2, &flp_st_sheet_last[k], 4);
    memcpy(q + 6, &flp_st_sheet_max[k], 4);
  }
  flp_dbg_frame(0x03, p, 25 + 10 * FLP_ST_SHEETS);
}

void flp_dbg_command(uint8_t type, const uint8_t* p, uint8_t len) {
  if (type == 0x10) {
    flp_dbg_hello();
    flp_dbg_texts_reset();
    return;
  }
  if (type == 0x14) {
    flp_st_reset();
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
    flp_dbg_texts();
    if (++flp_dbg_ticks >= 5) {
      flp_dbg_ticks = 0;
      flp_dbg_stats();
    }
  }
}

void setup() {
  flp_dbg_setup();

  // UART2: 9600 бод, 8N1, RX GPIO16, TX GPIO17
  Serial2.setRxBufferSize(256);
  Serial2.begin(9600, SERIAL_8N1, 16, 17);

  // Память под строки заранее: меньше дробления кучи.
  p_text_out.reserve(32);
  name_out.reserve(32);
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
  flp_st_begin();

  // Forced variables
  flp_dbg_force(14); flp_dbg_force(15);

  flp_st_sheet(0);  // лист 1
  // adc: analog_in
  adc_out = analogRead(34);
  flp_dbg_force(0);

  // bar: scale
  bar_out = 0.0f + ((float)adc_out - 0.0f) * (10.0f - 0.0f) / (4095.0f - 0.0f);
  bar_out = constrain(bar_out, 0.0f, 10.0f);
  flp_dbg_force(1);

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
  flp_dbg_force(2);

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
  flp_dbg_force(3);

  flp_st_sheet(1);  // лист 2
  // port2: uart_config
  port2_baud = (int32_t)Serial2.baudRate();
  flp_dbg_force(4);

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
  flp_dbg_force(6);
  flp_dbg_force(5);

  // command_w: var_set
  if (rx_ready) {
    var_command = rx_line;
  }

  // command_r: var_get
  command_r_out = var_command;

  flp_st_sheet(0);  // лист 1
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
  flp_dbg_force(7);

  // length_w: var_set
  var_message_len = length_out;
  flp_dbg_force(14);

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
  flp_dbg_force(8);

  flp_st_sheet(1);  // лист 2
  // is_on: text_compare
  {
    String a = command_r_out;
    String b = "ON";
    a.toLowerCase();
    b.toLowerCase();
    is_on_out = a == b;
  }
  flp_dbg_force(9);

  // pump: digital_out
  digitalWrite(26, (is_on_out) ? HIGH : LOW);

  // head: text_substring
  head_out = command_r_out.substring(0, 0 + 4);

  // is_set: text_compare
  is_set_out = head_out.startsWith("SET ");
  flp_dbg_force(10);

  // arg: text_substring
  arg_out = command_r_out.substring(4);

  // number: text_to_num
  {
    float v;
    number_ok = flp_text_to_float(arg_out, v);
    if (number_ok) {
      number_out = v;
    }
  }
  flp_dbg_force(12);
  flp_dbg_force(11);

  // apply: and
  apply_out = is_set_out && number_ok;
  flp_dbg_force(13);

  // setpoint_w: var_set
  if (apply_out) {
    var_setpoint = number_out;
  }
  flp_dbg_force(15);

  flp_st_end();
  flp_dbg_loop();
}
