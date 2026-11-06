// Сгенерировано CrabLogic из проекта «sensors». Ручные правки будут перезаписаны.
// Порядок выполнения: air → water → humid → fan → cold → heater_logic → heater → all_ok → fault → fault_led

#include <Arduino.h>
#include <DHT.h>
#include <OneWire.h>
#include <DallasTemperature.h>

// Выходы блоков
float air_temp = 0.0f;
float air_humidity = 0.0f;
bool air_ok = false;
float water_temp = 0.0f;
bool water_ok = false;
bool humid_out = false;
bool cold_out = false;
bool heater_logic_out = false;
bool all_ok_out = false;
bool fault_out = false;

// air: dht22
DHT air_dht(15, DHT22);
unsigned long air_last = 0;

// water: ds18b20
OneWire water_wire(4);
DallasTemperature water_bus(&water_wire);
unsigned long water_requested = 0;

void setup() {
  // air: dht22
  air_dht.begin();

  // water: ds18b20
  water_bus.begin();
  // Do not block the scan for 750 ms: request now, read on a later scan.
  water_bus.setWaitForConversion(false);
  water_bus.requestTemperatures();
  water_requested = millis();

  // fan: digital_out
  pinMode(26, OUTPUT);

  // heater: digital_out
  pinMode(27, OUTPUT);

  // fault_led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // air: dht22
  if (millis() - air_last >= 2000UL) {
    air_last = millis();
    float t = air_dht.readTemperature();
    float h = air_dht.readHumidity();
    air_ok = !isnan(t) && !isnan(h);
    if (air_ok) {
      air_temp = t;
      air_humidity = h;
    }
  }

  // water: ds18b20
  if (millis() - water_requested >= 1000UL) {
    float t = water_bus.getTempCByIndex(0);
    water_ok = t != DEVICE_DISCONNECTED_C;
    if (water_ok) {
      water_temp = t;
    }
    water_bus.requestTemperatures();
    water_requested = millis();
  }

  // humid: hysteresis
  if (air_humidity >= 70.0f) {
    humid_out = true;
  } else if (air_humidity <= 60.0f) {
    humid_out = false;
  }

  // fan: digital_out
  digitalWrite(26, (humid_out) ? HIGH : LOW);

  // cold: hysteresis
  if (water_temp >= 45.0f) {
    cold_out = true;
  } else if (water_temp <= 40.0f) {
    cold_out = false;
  }

  // heater_logic: not
  heater_logic_out = !cold_out;

  // heater: digital_out
  digitalWrite(27, (heater_logic_out) ? HIGH : LOW);

  // all_ok: and
  all_ok_out = air_ok && water_ok;

  // fault: not
  fault_out = !all_ok_out;

  // fault_led: digital_out
  digitalWrite(2, (fault_out) ? HIGH : LOW);
}
