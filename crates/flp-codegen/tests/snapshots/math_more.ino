// Сгенерировано CrabLogic из проекта «math_more». Ручные правки будут перезаписаны.
// Порядок выполнения: adc → level → root → square → cube → rest → mode → choose → route → out1 → out2 → out3

#include <Arduino.h>

// Выходы блоков
int32_t adc_out = 0;
float level_out = 0.0f;
float root_out = 0.0f;
float square_out = 0.0f;
float cube_out = 0.0f;
float rest_out = 0.0f;
float mode_out = 0.0f;
float choose_out = 0.0f;
float route_o1 = 0.0f;
float route_o2 = 0.0f;
float route_o3 = 0.0f;

void setup() {
  // mode: constant
  mode_out = 1.0f;
}

void loop() {
  // adc: analog_in
  adc_out = analogRead(34);

  // level: scale
  level_out = 0.0f + ((float)adc_out - 0.0f) * (100.0f - 0.0f) / (4095.0f - 0.0f);
  level_out = constrain(level_out, 0.0f, 100.0f);

  // root: sqrt
  root_out = level_out > 0.0f ? sqrtf(level_out) : 0.0f;

  // square: sq
  square_out = level_out * level_out;

  // cube: pow
  cube_out = powf(level_out, 3.0f);

  // rest: mod
  rest_out = 7.0f != 0.0f ? fmodf(level_out, 7.0f) : 0.0f;

  // choose: mux
  {
    int idx = (int)mode_out;
    if (idx <= 0) {
      choose_out = root_out;
    } else
    if (idx <= 1) {
      choose_out = rest_out;
    } else
    if (idx <= 2) {
      choose_out = 10.0f;
    } else
    choose_out = 20.0f;
  }

  // route: dms
  {
    int idx = (int)mode_out;
    route_o1 = idx == 0 ? choose_out : 0.0f;
    route_o2 = idx == 1 ? choose_out : 0.0f;
    route_o3 = idx == 2 ? choose_out : 0.0f;
  }

  // out1: pwm_out
  analogWrite(25, (int)(constrain(route_o1, 0.0f, 100.0f) * 2.55f + 0.5f));

  // out2: pwm_out
  analogWrite(26, (int)(constrain(route_o2, 0.0f, 100.0f) * 2.55f + 0.5f));

  // out3: pwm_out
  analogWrite(27, (int)(constrain(route_o3, 0.0f, 100.0f) * 2.55f + 0.5f));
}
