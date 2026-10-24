// Сгенерировано FLP_next из проекта «retain». Ручные правки будут перезаписаны.
// Порядок выполнения: button → press → total_r → one → next → total_w → limit_r → rest → reached → led

#include <Arduino.h>
#include <Preferences.h>

// Выходы блоков
bool button_out = false;
bool press_out = false;
float total_r_out = 0.0f;
float one_out = 0.0f;
float next_out = 0.0f;
float limit_r_out = 0.0f;
float rest_out = 0.0f;
bool reached_out = false;

// Переменные
float var_Nazhatiy_vsego_cfcf80 = 0.0f;
float var_Porog_ba5e84 = 10.0f;

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
float var_Nazhatiy_vsego_cfcf80_saved;  // последнее сохранённое «Нажатий всего»
float var_Porog_ba5e84_saved;  // последнее сохранённое «Порог»

// press: rising_edge
bool press_prev = false;

void setup() {
  // Восстановление переменных из флеша
  flp_retain.begin("flp_vars", false);
  if (flp_retain.getBytesLength("v84a2d104") == sizeof(var_Nazhatiy_vsego_cfcf80)) {  // Нажатий всего
    flp_retain.getBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
  }
  var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
  if (flp_retain.getBytesLength("v49f1d568") == sizeof(var_Porog_ba5e84)) {  // Порог
    flp_retain.getBytes("v49f1d568", &var_Porog_ba5e84, sizeof(var_Porog_ba5e84));
  }
  var_Porog_ba5e84_saved = var_Porog_ba5e84;

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // one: constant
  one_out = 1.0f;

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
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

  // limit_r: var_get
  limit_r_out = var_Porog_ba5e84;

  // rest: sub
  rest_out = total_r_out - limit_r_out;

  // reached: greater
  reached_out = rest_out > -0.5f;

  // led: digital_out
  digitalWrite(2, (reached_out) ? HIGH : LOW);

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - flp_retain_ms >= 1000) {
    flp_retain_ms = millis();
    if (memcmp(&var_Nazhatiy_vsego_cfcf80, &var_Nazhatiy_vsego_cfcf80_saved, sizeof(var_Nazhatiy_vsego_cfcf80)) != 0) {  // Нажатий всего
      var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
      flp_retain.putBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
    }
    if (memcmp(&var_Porog_ba5e84, &var_Porog_ba5e84_saved, sizeof(var_Porog_ba5e84)) != 0) {  // Порог
      var_Porog_ba5e84_saved = var_Porog_ba5e84;
      flp_retain.putBytes("v49f1d568", &var_Porog_ba5e84, sizeof(var_Porog_ba5e84));
    }
  }
}
