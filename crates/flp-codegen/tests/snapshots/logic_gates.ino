// Сгенерировано CrabLogic из проекта «logic_gates». Ручные правки будут перезаписаны.
// Порядок выполнения: a → b → both → any → differ → not_a → led_and → led_or → led_xor → led_not

#include <Arduino.h>

// Выходы блоков
bool a_out = false;
bool b_out = false;
bool both_out = false;
bool any_out = false;
bool differ_out = false;
bool not_a_out = false;

void setup() {
  // a: digital_in
  pinMode(12, INPUT_PULLUP);

  // b: digital_in
  pinMode(14, INPUT_PULLUP);

  // led_and: digital_out
  pinMode(25, OUTPUT);

  // led_or: digital_out
  pinMode(26, OUTPUT);

  // led_xor: digital_out
  pinMode(27, OUTPUT);

  // led_not: digital_out
  pinMode(33, OUTPUT);
}

void loop() {
  // a: digital_in
  a_out = !digitalRead(12);

  // b: digital_in
  b_out = !digitalRead(14);

  // both: and
  both_out = a_out && b_out;

  // any: or
  any_out = a_out || b_out;

  // differ: xor
  differ_out = a_out != b_out;

  // not_a: not
  not_a_out = !a_out;

  // led_and: digital_out
  digitalWrite(25, (both_out) ? HIGH : LOW);

  // led_or: digital_out
  digitalWrite(26, (any_out) ? HIGH : LOW);

  // led_xor: digital_out
  digitalWrite(27, (differ_out) ? HIGH : LOW);

  // led_not: digital_out
  digitalWrite(33, (not_a_out) ? HIGH : LOW);
}
