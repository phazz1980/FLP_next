// Сгенерировано CrabLogic из проекта «kc868_a6_relays». Ручные правки будут перезаписаны.
// Порядок выполнения: inputs → relays

#include <Arduino.h>
#include <Wire.h>

// Выходы блоков
bool inputs_p0 = false;
bool inputs_p1 = false;
bool inputs_p2 = false;
bool inputs_p3 = false;
bool inputs_p4 = false;
bool inputs_p5 = false;
bool inputs_ok = false;
bool relays_ok = false;

// inputs: kincony_kc868_a6_inputs
unsigned long inputs_polled = 0;

// relays: kincony_kc868_a6_relays
uint8_t relays_state = 0;
unsigned long relays_try = 0;

void setup() {
  // kincony_kc868_a6_inputs: общий код
  Wire.begin(4, 15);

  // inputs: kincony_kc868_a6_inputs
  // A pin reads as an input while the chip holds it high.
  Wire.beginTransmission(34);
  Wire.write(0xFF);
  Wire.endTransmission();

  // relays: kincony_kc868_a6_relays
  // The first write goes out on the first scan.
  relays_try = millis() - 500UL;
}

void loop() {
  // inputs: kincony_kc868_a6_inputs
  if (millis() - inputs_polled >= 20UL) {
    inputs_polled = millis();
    if (Wire.requestFrom((uint8_t)34, (uint8_t)1) == 1) {
      uint8_t inputs_raw = Wire.read();
      inputs_raw = ~inputs_raw;
      inputs_p0 = (inputs_raw & 0x01) != 0;
      inputs_p1 = (inputs_raw & 0x02) != 0;
      inputs_p2 = (inputs_raw & 0x04) != 0;
      inputs_p3 = (inputs_raw & 0x08) != 0;
      inputs_p4 = (inputs_raw & 0x10) != 0;
      inputs_p5 = (inputs_raw & 0x20) != 0;
      inputs_ok = true;
    } else {
      inputs_ok = false;
    }
  }

  // relays: kincony_kc868_a6_relays
  uint8_t relays_want = 0;
  if (inputs_p0) relays_want |= 0x01;
  if (inputs_p1) relays_want |= 0x02;
  if (inputs_p2) relays_want |= 0x04;
  if (inputs_p3) relays_want |= 0x08;
  if (inputs_p4) relays_want |= 0x10;
  if (inputs_p5) relays_want |= 0x20;
  relays_want = ~relays_want;
  // Write on change; while the chip does not answer, retry twice a second.
  if (relays_ok ? relays_want != relays_state : millis() - relays_try >= 500UL) {
    Wire.beginTransmission(36);
    Wire.write(relays_want);
    relays_ok = Wire.endTransmission() == 0;
    relays_try = millis();
    if (relays_ok) {
      relays_state = relays_want;
    }
  }
}
