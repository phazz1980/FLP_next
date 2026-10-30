// Сгенерировано FLP_next из проекта «pcf_pins». Ручные правки будут перезаписаны.
// Порядок выполнения: button1 → button2 → limit → relay1 → relay2 → lamp

#include <Arduino.h>
#include <Wire.h>

// Выходы блоков
bool button1_out = false;
bool button2_out = false;
bool limit_out = false;

// Расширитель buttons: PCF8574, адрес 0x22
uint16_t flp_x_buttons_in = 0x00FF;
uint16_t flp_x_buttons_out = 0x00FF;
uint16_t flp_x_buttons_sent = 0x00FF;
bool flp_x_buttons_ok = false;
unsigned long flp_x_buttons_try = 0;
unsigned long flp_x_buttons_read = 0;

// Читает выводы: не чаще одного раза в 20 мс; при ошибке остаются прежние значения.
void flp_x_buttons_poll() {
  if (millis() - flp_x_buttons_read < 20UL) {
    return;
  }
  flp_x_buttons_read = millis();
  if (Wire.requestFrom((uint8_t)0x22, (uint8_t)1) == 1) {
    uint16_t word = Wire.read();
    flp_x_buttons_in = word;
  } else {
    // Чип мог потерять питание: выводы будут записаны заново.
    flp_x_buttons_ok = false;
  }
}

// Записывает выводы при изменении; пока чип молчит, повторяет дважды в секунду.
void flp_x_buttons_flush() {
  uint16_t want = (flp_x_buttons_out | 0x0003) & 0x00FF;
  if (flp_x_buttons_ok ? want == flp_x_buttons_sent : millis() - flp_x_buttons_try < 500UL) {
    return;
  }
  Wire.beginTransmission(0x22);
  Wire.write(want & 0xFF);
  flp_x_buttons_ok = Wire.endTransmission() == 0;
  flp_x_buttons_try = millis();
  if (flp_x_buttons_ok) {
    flp_x_buttons_sent = want;
  }
}

// Расширитель relays: PCF8575, адрес 0x20
uint16_t flp_x_relays_in = 0xFFFF;
uint16_t flp_x_relays_out = 0xFFFF;
uint16_t flp_x_relays_sent = 0xFFFF;
bool flp_x_relays_ok = false;
unsigned long flp_x_relays_try = 0;
unsigned long flp_x_relays_read = 0;

// Читает выводы: не чаще одного раза в 20 мс; при ошибке остаются прежние значения.
void flp_x_relays_poll() {
  if (millis() - flp_x_relays_read < 20UL) {
    return;
  }
  flp_x_relays_read = millis();
  if (Wire.requestFrom((uint8_t)0x20, (uint8_t)2) == 2) {
    uint16_t word = Wire.read();
    word |= (uint16_t)Wire.read() << 8;
    flp_x_relays_in = word;
  } else {
    // Чип мог потерять питание: выводы будут записаны заново.
    flp_x_relays_ok = false;
  }
}

// Записывает выводы при изменении; пока чип молчит, повторяет дважды в секунду.
void flp_x_relays_flush() {
  uint16_t want = (flp_x_relays_out | 0x8000) & 0xFFFF;
  if (flp_x_relays_ok ? want == flp_x_relays_sent : millis() - flp_x_relays_try < 500UL) {
    return;
  }
  Wire.beginTransmission(0x20);
  Wire.write(want & 0xFF);
  Wire.write(want >> 8);
  flp_x_relays_ok = Wire.endTransmission() == 0;
  flp_x_relays_try = millis();
  if (flp_x_relays_ok) {
    flp_x_relays_sent = want;
  }
}

void setup() {
  // Расширители: шина и первая запись
  Wire.begin();
  flp_x_buttons_try = millis() - 500UL;
  flp_x_buttons_flush();
  flp_x_relays_try = millis() - 500UL;
  flp_x_relays_flush();

  // lamp: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  flp_x_buttons_poll();  // расширитель buttons
  flp_x_relays_poll();  // расширитель relays

  // button1: digital_in
  button1_out = !((flp_x_buttons_in >> 0) & 1);

  // button2: digital_in
  button2_out = !((flp_x_buttons_in >> 1) & 1);

  // limit: digital_in
  limit_out = !((flp_x_relays_in >> 15) & 1);

  // relay1: digital_out
  bitWrite(flp_x_relays_out, 0, !(button1_out));

  // relay2: digital_out
  bitWrite(flp_x_relays_out, 9, !(button2_out));

  // lamp: digital_out
  digitalWrite(2, (limit_out) ? HIGH : LOW);

  flp_x_buttons_flush();  // расширитель buttons
  flp_x_relays_flush();  // расширитель relays
}
