// Сгенерировано CrabLogic из проекта «cycle_report». Ручные правки будут перезаписаны.
// Порядок выполнения: blink → led → cycle

#include <Arduino.h>

// Выходы блоков
bool blink_out = false;

// blink: generator
unsigned long blink_start = 0;

// ---- Диагностика: время цикла (мкс), всего и по листам ----
const uint8_t FLP_ST_SHEETS = 1;
const uint16_t flp_st_sheet_no[FLP_ST_SHEETS + 1] = { 1, 0 };
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

// Отчёт по USB: ядро 0, отдельно от основного цикла
void flp_report_task(void *) {
  for (;;) {
    flp_st_report();
    vTaskDelay(1);
  }
}

void setup() {
  Serial.begin(115200);

  // led: digital_out
  pinMode(2, OUTPUT);

  // Отчёт стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_report_task, "flp_report", 8192, nullptr, 1, nullptr, 0);
}

void loop() {
  flp_st_begin();

  flp_st_sheet(0);  // лист 1
  // blink: generator
  if (true) {
    unsigned long phase = (millis() - blink_start) % (1000UL);
    blink_out = phase < 500UL;
  } else {
    blink_start = millis();
    blink_out = false;
  }

  // led: digital_out
  digitalWrite(2, (blink_out) ? HIGH : LOW);

  flp_st_end();
}
