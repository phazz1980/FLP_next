// Сгенерировано CrabLogic из проекта «tag_copies». Ручные правки будут перезаписаны.
// Порядок выполнения: button → button_2 → lamp → blink → lamp_2

#include <Arduino.h>

// Выходы блоков
bool button_out = false;
bool button_2_out = false;
bool blink_out = false;

void setup() {
  // button: digital_in
  pinMode(4, INPUT_PULLUP);

  // lamp: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // button: digital_in
  button_out = !digitalRead(4);

  // button_2: digital_in
  button_2_out = button_out;

  // lamp: digital_out
  digitalWrite(2, (button_out) ? HIGH : LOW);

  // blink: not
  blink_out = !button_2_out;

  // lamp_2: digital_out
  digitalWrite(2, (blink_out) ? HIGH : LOW);
}
