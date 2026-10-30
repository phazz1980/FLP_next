// Сгенерировано CrabLogic из проекта «shared_code». Ручные правки будут перезаписаны.
// Порядок выполнения: a → b → c

#include <Arduino.h>

// Выходы блоков
float a_out = 0.0f;
bool a_ok = false;
float b_out = 0.0f;
bool b_ok = false;
int32_t b_raw = 0;
float c_out = 0.0f;
bool c_ok = false;

// probe: общий код
Bus bus_4(4);

// probe: общий код
Bus bus_5(5);

// a: probe
float a_last = 0.0f;

// b: probe
float b_last = 0.0f;

// c: probe
float c_last = 0.0f;

// probe: общий код
float probe_read(Bus &bus) {
  return bus.read();
}

// a: probe
float a_value() {
  return a_last * 2.0f;
}

// b: probe
float b_value() {
  return b_last;
}

// c: probe
float c_value() {
  return c_last * 0.5f;
}

void setup() {
  // probe: общий код
  bus_4.begin();

  // probe: общий код
  bus_5.begin();
}

void loop() {
  // a: probe
  a_last = probe_read(bus_4);
  a_out = a_value();
  a_ok = true;

  // b: probe
  if (a_ok) b_last = probe_read(bus_4);
  b_out = b_value();
  b_ok = true;
  b_raw = (int32_t)b_last;

  // c: probe
  c_last = probe_read(bus_5);
  c_out = c_value();
  c_ok = true;
}
