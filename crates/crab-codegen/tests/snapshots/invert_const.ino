// Сгенерировано CrabLogic из проекта «invert_const». Ручные правки будут перезаписаны.
// Порядок выполнения: temp → corrected → hot → led

#include <Arduino.h>
#include <OneWire.h>
#include <DallasTemperature.h>

// Выходы блоков
float temp_temp = 0.0f;
bool temp_ok = false;
float corrected_out = 0.0f;
bool hot_out = false;

// temp: ds18b20
OneWire temp_wire(4);
DallasTemperature temp_bus(&temp_wire);
unsigned long temp_requested = 0;

void setup() {
  // temp: ds18b20
  temp_bus.begin();
  // Do not block the scan for 750 ms: request now, read on a later scan.
  temp_bus.setWaitForConversion(false);
  temp_bus.requestTemperatures();
  temp_requested = millis();

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // temp: ds18b20
  if (millis() - temp_requested >= 1000UL) {
    float t = temp_bus.getTempCByIndex(0);
    temp_ok = t != DEVICE_DISCONNECTED_C;
    if (temp_ok) {
      temp_temp = t;
    }
    temp_bus.requestTemperatures();
    temp_requested = millis();
  }

  // corrected: add
  corrected_out = temp_temp + -0.5f;

  // hot: greater
  hot_out = corrected_out > 25.0f;

  // led: digital_out
  digitalWrite(2, (!hot_out) ? HIGH : LOW);
}
