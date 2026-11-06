// Сгенерировано CrabLogic из проекта «cycle_report». Ручные правки будут перезаписаны.
// Порядок выполнения: blink → led → cycle

#include <Arduino.h>

// Выходы блоков
bool blink_out = false;

// blink: generator
unsigned long blink_start = 0;

// ---- Диагностика: время цикла (мкс), всего и по листам ----
const uint8_t CRAB_ST_SHEETS = 1;
const uint16_t crab_st_sheet_no[CRAB_ST_SHEETS + 1] = { 1, 0 };
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

unsigned long crab_st_report_last = 0;

void crab_st_report() {
  if (millis() - crab_st_report_last < 2000UL) {
    return;
  }
  crab_st_report_last = millis();
  uint32_t cycles = crab_st_cycles;
  uint32_t avg = crab_st_take_avg();
  Serial.print("[CRAB] цикл, мкс: последний ");
  Serial.print(crab_st_last);
  Serial.print(", средний ");
  Serial.print(avg);
  Serial.print(", макс ");
  Serial.print(crab_st_max);
  Serial.print(", макс период loop ");
  Serial.print(crab_st_period_max);
  Serial.print("; циклов ");
  Serial.print(cycles);
  Serial.print("; своб. память ");
  Serial.println(crab_st_free_heap());
  for (uint8_t k = 0; k < CRAB_ST_SHEETS; k++) {
    Serial.print("[CRAB]   лист ");
    Serial.print(crab_st_sheet_no[k]);
    Serial.print(": последний ");
    Serial.print(crab_st_sheet_last[k]);
    Serial.print(", макс ");
    Serial.println(crab_st_sheet_max[k]);
  }
}

// Отчёт по USB: ядро 0, отдельно от основного цикла
void crab_report_task(void *) {
  for (;;) {
    crab_st_report();
    vTaskDelay(1);
  }
}

void setup() {
  Serial.begin(115200);

  // led: digital_out
  pinMode(2, OUTPUT);

  // Отчёт стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_report_task, "crab_report", 8192, nullptr, 1, nullptr, 0);
}

void loop() {
  crab_st_begin();

  crab_st_sheet(0);  // лист 1
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

  crab_st_end();
}
