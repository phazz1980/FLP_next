// Сгенерировано FLP_next из проекта «background». Ручные правки будут перезаписаны.
// Порядок выполнения: button → press → total_r → one → next → total_w → shown → reached → led → blink_led

#include <Arduino.h>
#include <Preferences.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool button_out = false;
bool press_out = false;
float total_r_out = 0.0f;
float one_out = 0.0f;
float next_out = 0.0f;
float shown_out = 0.0f;
bool reached_out = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
bool press_out_pub = false;
bool press_out_in = false;
bool press_out_pend = false;

// Переменные
float var_Nazhatiy_vsego_cfcf80 = 0.0f;

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
float var_Nazhatiy_vsego_cfcf80_saved;  // последнее сохранённое «Нажатий всего»

// press: rising_edge
bool press_prev = false;

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    // button: digital_in
    button_out = !digitalRead(13);

    // press: rising_edge
    press_out = button_out && !press_prev;
    press_prev = button_out;

    // total_r: var_get
    total_r_out = var_Nazhatiy_vsego_cfcf80;

    // next: add
    next_out = total_r_out + one_out;

    // total_w: var_set
    if (press_out) {
      var_Nazhatiy_vsego_cfcf80 = next_out;
    }

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    press_out_pub = press_out;
    press_out_pend = press_out_pend || press_out;
    xSemaphoreGive(flp_bg_lock);

    // Сохранение переменных во флеш: только изменившиеся значения
    if (millis() - flp_retain_ms >= 1000) {
      flp_retain_ms = millis();
      if (memcmp(&var_Nazhatiy_vsego_cfcf80, &var_Nazhatiy_vsego_cfcf80_saved, sizeof(var_Nazhatiy_vsego_cfcf80)) != 0) {  // Нажатий всего
        var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
        flp_retain.putBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
      }
    }

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Восстановление переменных из флеша
  flp_retain.begin("flp_vars", false);
  if (flp_retain.getBytesLength("v84a2d104") == sizeof(var_Nazhatiy_vsego_cfcf80)) {  // Нажатий всего
    flp_retain.getBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
  }
  var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // one: constant
  one_out = 1.0f;

  // led: digital_out
  pinMode(2, OUTPUT);

  // blink_led: digital_out
  pinMode(4, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  press_out_in = press_out_pub || press_out_pend;
  press_out_pend = false;
  xSemaphoreGive(flp_bg_lock);

  // shown: var_get
  shown_out = var_Nazhatiy_vsego_cfcf80;

  // reached: greater
  reached_out = shown_out > 4.5f;

  // led: digital_out
  digitalWrite(2, (reached_out) ? HIGH : LOW);

  // blink_led: digital_out
  digitalWrite(4, (press_out_in) ? HIGH : LOW);
}
