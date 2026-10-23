// Сгенерировано FLP_next из проекта «button». Ручные правки будут перезаписаны.
// Порядок выполнения: button → led

#include <Arduino.h>

// Выходы блоков
bool button_out = false;

void setup() {
  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // button: digital_in
  button_out = !digitalRead(13);

  // led: digital_out
  digitalWrite(2, (button_out) ? HIGH : LOW);
}
