// Сгенерировано CrabLogic из проекта «pcf8574». Ручные правки будут перезаписаны.
// Порядок выполнения: inputs → relays → leds

#include <Arduino.h>
#include <Wire.h>

// Выходы блоков
bool inputs_p0 = false;
bool inputs_p1 = false;
bool inputs_p2 = false;
bool inputs_p3 = false;
bool inputs_p4 = false;
bool inputs_p5 = false;
bool inputs_p6 = false;
bool inputs_p7 = false;
bool inputs_ok = false;
bool relays_ok = false;
bool leds_ok = false;

// inputs: pcf8574_in
unsigned long inputs_polled = 0;

// relays: pcf8574_out
uint8_t relays_state = 0;
unsigned long relays_try = 0;

// leds: pcf8574_out
uint8_t leds_state = 0;
unsigned long leds_try = 0;

void setup() {
  // pcf8574_in: общий код
  Wire.begin(21, 22);

  // inputs: pcf8574_in
  // A pin reads as an input while the chip holds it high.
  Wire.beginTransmission(34);
  Wire.write(0xFF);
  Wire.endTransmission();

  // relays: pcf8574_out
  // The first write goes out on the first scan.
  relays_try = millis() - 500UL;

  // leds: pcf8574_out
  // The first write goes out on the first scan.
  leds_try = millis() - 500UL;
}

void loop() {
  // inputs: pcf8574_in
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
      inputs_p6 = (inputs_raw & 0x40) != 0;
      inputs_p7 = (inputs_raw & 0x80) != 0;
      inputs_ok = true;
    } else {
      inputs_ok = false;
    }
  }

  // relays: pcf8574_out
  uint8_t relays_want = 0;
  if (inputs_p0) relays_want |= 0x01;
  if (inputs_p1) relays_want |= 0x02;
  if (false) relays_want |= 0x04;
  if (false) relays_want |= 0x08;
  if (false) relays_want |= 0x10;
  if (false) relays_want |= 0x20;
  if (false) relays_want |= 0x40;
  if (false) relays_want |= 0x80;
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

  // leds: pcf8574_out
  uint8_t leds_want = 0;
  if (inputs_ok) leds_want |= 0x01;
  if (false) leds_want |= 0x02;
  if (false) leds_want |= 0x04;
  if (false) leds_want |= 0x08;
  if (false) leds_want |= 0x10;
  if (false) leds_want |= 0x20;
  if (false) leds_want |= 0x40;
  if (false) leds_want |= 0x80;
  // Write on change; while the chip does not answer, retry twice a second.
  if (leds_ok ? leds_want != leds_state : millis() - leds_try >= 500UL) {
    Wire.beginTransmission(32);
    Wire.write(leds_want);
    leds_ok = Wire.endTransmission() == 0;
    leds_try = millis();
    if (leds_ok) {
      leds_state = leds_want;
    }
  }
}
