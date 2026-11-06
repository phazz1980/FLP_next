// Сгенерировано CrabLogic из проекта «toggle». Ручные правки будут перезаписаны.
// Порядок выполнения: button → clean → toggle → led

#include <Arduino.h>

// Выходы блоков
bool button_out = false;
bool clean_out = false;
bool toggle_out = false;

// clean: debounce
bool clean_last = false;
unsigned long clean_changed = 0;

// toggle: t_trigger
bool toggle_prev = false;

void setup() {
  // button: digital_in
  pinMode(12, INPUT_PULLUP);

  // led: digital_out
  pinMode(25, OUTPUT);
}

void loop() {
  // button: digital_in
  button_out = !digitalRead(12);

  // clean: debounce
  if (button_out != clean_last) {
    clean_last = button_out;
    clean_changed = millis();
  }
  if (millis() - clean_changed >= 30UL) {
    clean_out = clean_last;
  }

  // toggle: t_trigger
  if (false) {
    toggle_out = false;
  } else if (clean_out && !toggle_prev) {
    toggle_out = !toggle_out;
  }
  toggle_prev = clean_out;

  // led: digital_out
  digitalWrite(25, (toggle_out) ? HIGH : LOW);
}
