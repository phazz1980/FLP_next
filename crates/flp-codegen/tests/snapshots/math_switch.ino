// Сгенерировано CrabLogic из проекта «math_switch». Ручные правки будут перезаписаны.
// Порядок выполнения: adc → level → dev → lowest → highest → clamp → button → pick → fan → dev_out → max_out

#include <Arduino.h>

// Выходы блоков
int32_t adc_out = 0;
float level_out = 0.0f;
float dev_out = 0.0f;
float lowest_out = 0.0f;
float highest_out = 0.0f;
float clamp_out = 0.0f;
bool button_out = false;
float pick_out = 0.0f;

void setup() {
  // button: digital_in
  pinMode(12, INPUT_PULLUP);
}

void loop() {
  // adc: analog_in
  adc_out = analogRead(34);

  // level: scale
  level_out = 0.0f + ((float)adc_out - 0.0f) * (100.0f - 0.0f) / (4095.0f - 0.0f);
  level_out = constrain(level_out, 0.0f, 100.0f);

  // dev: abs
  dev_out = fabs(level_out);

  // lowest: min
  lowest_out = level_out < 80.0f ? level_out : 80.0f;

  // highest: max
  highest_out = lowest_out > 10.0f ? lowest_out : 10.0f;

  // clamp: limit
  clamp_out = level_out < 20.0f ? 20.0f : (level_out > 90.0f ? 90.0f : level_out);

  // button: digital_in
  button_out = !digitalRead(12);

  // pick: switch
  pick_out = button_out ? 50.0f : clamp_out;

  // fan: pwm_out
  analogWrite(25, (int)(constrain(pick_out, 0.0f, 100.0f) * 2.55f + 0.5f));

  // dev_out: pwm_out
  analogWrite(26, (int)(constrain(dev_out, 0.0f, 100.0f) * 2.55f + 0.5f));

  // max_out: pwm_out
  analogWrite(27, (int)(constrain(highest_out, 0.0f, 100.0f) * 2.55f + 0.5f));
}
