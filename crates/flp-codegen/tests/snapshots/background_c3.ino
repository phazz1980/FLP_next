// Сгенерировано FLP_next из проекта «background». Ручные правки будут перезаписаны.
// Порядок выполнения: button → press → total_r → one → next → total_w → shown → reached → led → blink_led

#include <Arduino.h>
#include <Preferences.h>

// Выходы блоков
bool button_out = false;
bool press_out = false;
float total_r_out = 0.0f;
float one_out = 0.0f;
float next_out = 0.0f;
float shown_out = 0.0f;
bool reached_out = false;

// Периоды фоновых плат
uint32_t flp_bg2_t = 0u - 10u;  // плата 2: раз в 10 мс
uint32_t flp_bg3_t = 10u - 20u;  // плата 3: раз в 20 мс

// Переменные
float var_Nazhatiy_vsego_cfcf80 = 0.0f;

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
float var_Nazhatiy_vsego_cfcf80_saved;  // последнее сохранённое «Нажатий всего»

// press: rising_edge
bool press_prev = false;

void setup() {
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
}

void loop() {
  // Фоновые платы на одноядерном МК: каждая раз в свой период, не больше одной за проход
  uint32_t flp_now = millis();
  bool flp_bg_one = false;
  bool flp_bg2_run = false;
  if (!flp_bg_one && (uint32_t)(flp_now - flp_bg2_t) >= 10u) { flp_bg2_t = flp_now; flp_bg2_run = true; flp_bg_one = true; }
  bool flp_bg3_run = false;
  if (!flp_bg_one && (uint32_t)(flp_now - flp_bg3_t) >= 20u) { flp_bg3_t = flp_now; flp_bg3_run = true; flp_bg_one = true; }


  // button: digital_in
  if (flp_bg2_run) {
    button_out = !digitalRead(13);
  }

  // press: rising_edge
  if (flp_bg2_run) {
    press_out = button_out && !press_prev;
    press_prev = button_out;
  }

  // total_r: var_get
  if (flp_bg2_run) {
    total_r_out = var_Nazhatiy_vsego_cfcf80;
  }

  // next: add
  if (flp_bg2_run) {
    next_out = total_r_out + one_out;
  }

  // total_w: var_set
  if (flp_bg2_run) {
    if (press_out) {
      var_Nazhatiy_vsego_cfcf80 = next_out;
    }
  }

  // shown: var_get
  shown_out = var_Nazhatiy_vsego_cfcf80;

  // reached: greater
  reached_out = shown_out > 4.5f;

  // led: digital_out
  digitalWrite(2, (reached_out) ? HIGH : LOW);

  // blink_led: digital_out
  if (flp_bg3_run) {
    digitalWrite(4, (press_out) ? HIGH : LOW);
  }

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - flp_retain_ms >= 1000) {
    flp_retain_ms = millis();
    if (memcmp(&var_Nazhatiy_vsego_cfcf80, &var_Nazhatiy_vsego_cfcf80_saved, sizeof(var_Nazhatiy_vsego_cfcf80)) != 0) {  // Нажатий всего
      var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
      flp_retain.putBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
    }
  }
}
