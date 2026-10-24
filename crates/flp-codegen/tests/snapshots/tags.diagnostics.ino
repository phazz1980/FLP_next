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

unsigned long flp_st_report_last = 0;

void flp_st_report() {
  if (millis() - flp_st_report_last < 2000UL) {
    return;
  }
  flp_st_report_last = millis();
  uint32_t cycles = flp_st_cycles;
  uint32_t avg = flp_st_take_avg();
  Serial.print("[FLP] цикл, мкс: последний ");
  Serial.print(flp_st_last);
  Serial.print(", средний ");
  Serial.print(avg);
  Serial.print(", макс ");
  Serial.print(flp_st_max);
  Serial.print(", макс период loop ");
  Serial.print(flp_st_period_max);
  Serial.print("; циклов ");
  Serial.print(cycles);
  Serial.print("; своб. память ");
  Serial.println(flp_st_free_heap());
  for (uint8_t k = 0; k < FLP_ST_SHEETS; k++) {
    Serial.print("[FLP]   лист ");
    Serial.print(flp_st_sheet_no[k]);
    Serial.print(": последний ");
    Serial.print(flp_st_sheet_last[k]);
    Serial.print(", макс ");
    Serial.println(flp_st_sheet_max[k]);
  }
}

void setup() {
  Serial.begin(115200);

  // heater: digital_out
  pinMode(26, OUTPUT);
}

void loop() {
  flp_st_begin();

  flp_st_sheet(0);  // лист 1
  // adc: analog_in
  adc_out = analogRead(34);

  // to_c: scale
  to_c_out = 0.0f + ((float)adc_out - 0.0f) * (50.0f - 0.0f) / (4095.0f - 0.0f);
  to_c_out = constrain(to_c_out, 0.0f, 50.0f);

  // temp_w: var_set
  var_temp = to_c_out;

  flp_st_sheet(1);  // лист 2
  // temp_r: var_get
  temp_r_out = var_temp;

  // setpoint_r: var_get
  setpoint_r_out = var_setpoint;

  // error: sub
  error_out = temp_r_out - setpoint_r_out;

  // cold: less
  cold_out = error_out < -1.0f;

  // heating_w: var_set
  var_heating = cold_out;

  // heating_r: var_get
  heating_r_out = var_heating;

  // heater: digital_out
  digitalWrite(26, (heating_r_out) ? HIGH : LOW);

  flp_st_end();
  flp_st_report();
}
