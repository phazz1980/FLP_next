// Сгенерировано CrabLogic из проекта «weather». Ручные правки будут перезаписаны.
// Порядок выполнения: air → humid → fan → water → period → tick → save_now → last_water → all_ok → fault → fault_led

#include <Arduino.h>
#include <DHT.h>
#include <OneWire.h>
#include <DallasTemperature.h>
#include <Preferences.h>

// Выходы блоков
float air_temp = 0.0f;
float air_humidity = 0.0f;
bool air_ok = false;
bool humid_out = false;
float water_temp = 0.0f;
bool water_ok = false;
bool period_out = false;
bool tick_out = false;
bool save_now_out = false;
float last_water_out = 0.0f;
bool all_ok_out = false;
bool fault_out = false;

// air: dht22
DHT air_dht(15, DHT22);
unsigned long air_last = 0;

// water: ds18b20
OneWire water_wire(4);
DallasTemperature water_bus(&water_wire);
unsigned long water_requested = 0;

// period: generator
unsigned long period_start = 0;

// last_water: flash_store
Preferences last_water_prefs;
bool last_water_prev = false;

void setup() {
  // air: dht22
  air_dht.begin();

  // fan: digital_out
  pinMode(26, OUTPUT);

  // water: ds18b20
  water_bus.begin();
  // Do not block the scan for 750 ms: request now, read on a later scan.
  water_bus.setWaitForConversion(false);
  water_bus.requestTemperatures();
  water_requested = millis();

  // last_water: flash_store
  last_water_prefs.begin("flp", false);
  last_water_out = last_water_prefs.getFloat("last_water", 0.0f);

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

  // humid: hysteresis
  if (air_humidity >= 70.0f) {
    humid_out = true;
  } else if (air_humidity <= 60.0f) {
    humid_out = false;
  }

  // fan: digital_out
  digitalWrite(26, (humid_out) ? HIGH : LOW);

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

  // period: generator
  if (true) {
    unsigned long phase = (millis() - period_start) % (601000UL);
    period_out = phase < 600000UL;
  } else {
    period_start = millis();
    period_out = false;
  }

  // tick: not
  tick_out = !period_out;

  // save_now: and
  save_now_out = tick_out && water_ok;

  // last_water: flash_store
  // Flash wears out: write only a value that differs from the saved one.
  if (save_now_out && !last_water_prev && water_temp != last_water_out) {
    last_water_out = water_temp;
    last_water_prefs.putFloat("last_water", last_water_out);
  }
  last_water_prev = save_now_out;

  // all_ok: and
  all_ok_out = air_ok && water_ok;

  // fault: not
  fault_out = !all_ok_out;

  // fault_led: digital_out
  digitalWrite(2, (fault_out) ? HIGH : LOW);
}
