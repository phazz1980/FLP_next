// Сгенерировано CrabLogic из проекта «pcf_pins». Ручные правки будут перезаписаны.
// Порядок выполнения: button1 → button2 → limit → relay1 → relay2 → lamp

#include <Arduino.h>
#include <Wire.h>

// Выходы блоков
bool button1_out = false;
bool button2_out = false;
bool limit_out = false;

// Расширитель buttons: PCF8574, адрес 0x22
uint16_t crab_x_buttons_in = 0x00FF;
uint16_t crab_x_buttons_out = 0x00FF;
uint16_t crab_x_buttons_sent = 0x00FF;
bool crab_x_buttons_ok = false;
unsigned long crab_x_buttons_try = 0;
unsigned long crab_x_buttons_read = 0;

// Читает выводы: не чаще одного раза в 20 мс; при ошибке остаются прежние значения.
void crab_x_buttons_poll() {
  if (millis() - crab_x_buttons_read < 20UL) {
    return;
  }
  crab_x_buttons_read = millis();
  if (Wire.requestFrom((uint8_t)0x22, (uint8_t)1) == 1) {
    uint16_t word = Wire.read();
    crab_x_buttons_in = word;
  } else {
    // Чип мог потерять питание: выводы будут записаны заново.
    crab_x_buttons_ok = false;
  }
}

// Записывает выводы при изменении; пока чип молчит, повторяет дважды в секунду.
void crab_x_buttons_flush() {
  uint16_t want = (crab_x_buttons_out | 0x0003) & 0x00FF;
  if (crab_x_buttons_ok ? want == crab_x_buttons_sent : millis() - crab_x_buttons_try < 500UL) {
    return;
  }
  Wire.beginTransmission(0x22);
  Wire.write(want & 0xFF);
  crab_x_buttons_ok = Wire.endTransmission() == 0;
  crab_x_buttons_try = millis();
  if (crab_x_buttons_ok) {
    crab_x_buttons_sent = want;
  }
}

// Расширитель relays: PCF8575, адрес 0x20
uint16_t crab_x_relays_in = 0xFFFF;
uint16_t crab_x_relays_out = 0xFFFF;
uint16_t crab_x_relays_sent = 0xFFFF;
bool crab_x_relays_ok = false;
unsigned long crab_x_relays_try = 0;
unsigned long crab_x_relays_read = 0;

// Читает выводы: не чаще одного раза в 20 мс; при ошибке остаются прежние значения.
void crab_x_relays_poll() {
  if (millis() - crab_x_relays_read < 20UL) {
    return;
  }
  crab_x_relays_read = millis();
  if (Wire.requestFrom((uint8_t)0x20, (uint8_t)2) == 2) {
    uint16_t word = Wire.read();
    word |= (uint16_t)Wire.read() << 8;
    crab_x_relays_in = word;
  } else {
    // Чип мог потерять питание: выводы будут записаны заново.
    crab_x_relays_ok = false;
  }
}

// Записывает выводы при изменении; пока чип молчит, повторяет дважды в секунду.
void crab_x_relays_flush() {
  uint16_t want = (crab_x_relays_out | 0x8000) & 0xFFFF;
  if (crab_x_relays_ok ? want == crab_x_relays_sent : millis() - crab_x_relays_try < 500UL) {
    return;
  }
  Wire.beginTransmission(0x20);
  Wire.write(want & 0xFF);
  Wire.write(want >> 8);
  crab_x_relays_ok = Wire.endTransmission() == 0;
  crab_x_relays_try = millis();
  if (crab_x_relays_ok) {
    crab_x_relays_sent = want;
  }
}

void setup() {
  // Расширители: шина и первая запись
  Wire.begin();
  crab_x_buttons_try = millis() - 500UL;
  crab_x_buttons_flush();
  crab_x_relays_try = millis() - 500UL;
  crab_x_relays_flush();

  // lamp: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  crab_x_buttons_poll();  // расширитель buttons
  crab_x_relays_poll();  // расширитель relays

  // button1: digital_in
  button1_out = !((crab_x_buttons_in >> 0) & 1);

  // button2: digital_in
  button2_out = !((crab_x_buttons_in >> 1) & 1);

  // limit: digital_in
  limit_out = !((crab_x_relays_in >> 15) & 1);

  // relay1: digital_out
  bitWrite(crab_x_relays_out, 0, !(button1_out));

  // relay2: digital_out
  bitWrite(crab_x_relays_out, 9, !(button2_out));

  // lamp: digital_out
  digitalWrite(2, (limit_out) ? HIGH : LOW);

  crab_x_buttons_flush();  // расширитель buttons
  crab_x_relays_flush();  // расширитель relays
}
