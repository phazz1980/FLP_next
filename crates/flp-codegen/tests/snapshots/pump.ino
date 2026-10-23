// Сгенерировано FLP_next из проекта «pump». Ручные правки будут перезаписаны.
// Порядок выполнения: sensor → pressure → low → delay → pump

#include <Arduino.h>

// Выходы блоков
int32_t sensor_out = 0;
float pressure_out = 0.0f;
bool low_out = false;
bool delay_out = false;

// delay: timer_on
bool delay_running = false;
unsigned long delay_start = 0;

void setup() {
  // pump: digital_out
  pinMode(26, OUTPUT);
}

void loop() {
  // sensor: analog_in
  sensor_out = analogRead(34);

  // pressure: scale
  pressure_out = 0.0f + ((float)sensor_out - 0.0f) * (10.0f - 0.0f) / (4095.0f - 0.0f);
  pressure_out = constrain(pressure_out, 0.0f, 10.0f);

  // low: less
  low_out = pressure_out < 2.0f;

  // delay: timer_on
  if (low_out) {
    if (!delay_running) {
      delay_running = true;
      delay_start = millis();
    }
    delay_out = millis() - delay_start >= 5000UL;
  } else {
    delay_running = false;
    delay_out = false;
  }

  // pump: digital_out
  digitalWrite(26, (delay_out) ? HIGH : LOW);
}
