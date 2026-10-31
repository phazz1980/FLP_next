// Сгенерировано CrabLogic из проекта «param_inputs». Ручные правки будут перезаписаны.
// Порядок выполнения: pot → button → delay → lamp → cap_value → cap

#include <Arduino.h>

// Выходы блоков
int32_t pot_out = 0;
bool button_out = false;
bool delay_out = false;
float cap_value_out = 0.0f;
int32_t cap_out = 0;

// delay: timer_on
bool delay_running = false;
unsigned long delay_start = 0;

void setup() {
  // button: digital_in
  pinMode(12, INPUT_PULLUP);

  // lamp: digital_out
  pinMode(2, OUTPUT);

  // cap_value: constant
  cap_value_out = 2000.0f;
}

void loop() {
  // pot: analog_in
  pot_out = analogRead(34);

  // button: digital_in
  button_out = !digitalRead(12);

  // delay: timer_on
  if (button_out) {
    if (!delay_running) {
      delay_running = true;
      delay_start = millis();
    }
    delay_out = millis() - delay_start >= (uint32_t)(pot_out);
  } else {
    delay_running = false;
    delay_out = false;
  }

  // lamp: digital_out
  digitalWrite(2, (delay_out) ? HIGH : LOW);

  // cap: limit
  cap_out = pot_out < 100.0f ? 100.0f : (pot_out > (cap_value_out) ? (cap_value_out) : pot_out);
}
