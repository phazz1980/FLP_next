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

void setup() {
  // heater: digital_out
  pinMode(26, OUTPUT);
}

void loop() {
  // adc: analog_in
  adc_out = analogRead(34);

  // to_c: scale
  to_c_out = 0.0f + ((float)adc_out - 0.0f) * (50.0f - 0.0f) / (4095.0f - 0.0f);
  to_c_out = constrain(to_c_out, 0.0f, 50.0f);

  // temp_w: var_set
  var_temp = to_c_out;

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
}
