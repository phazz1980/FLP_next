// Сгенерировано CrabLogic из проекта «blink». Ручные правки будут перезаписаны.
// Порядок выполнения: blink → led

#include <Arduino.h>

// Выходы блоков
bool blink_out = false;

// blink: generator
unsigned long blink_start = 0;

void setup() {
  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
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
}
