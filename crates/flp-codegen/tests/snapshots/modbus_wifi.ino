// Сгенерировано FLP_next из проекта «modbus_wifi». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → slave → sensor → temp_reg → button → button_reg → online_reg → led_reg → led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogModbusTCP.h>
#include <OneWire.h>
#include <DallasTemperature.h>

// Выходы блоков
bool wifi_ready = false;
int32_t slave_error = 0;
float sensor_temp = 0.0f;
bool sensor_ok = false;
bool button_out = false;
bool led_reg_out_b = false;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;

// slave: modbus_tcp_slave
// Shared slave: modbus_read / modbus_write blocks access its tables.
ModbusSlaveTCP flpModbus(&flpWifi);

// sensor: ds18b20
OneWire sensor_wire(4);
DallasTemperature sensor_bus(&sensor_wire);
unsigned long sensor_requested = 0;

void setup() {
  // wifi: wifi
  flpWifi.clientOn();
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("12345678");
  flpWifi.setDhcp();

  // slave: modbus_tcp_slave
  flpModbus.setSlaveAddress(1);
  flpModbus.setTcpPort(502);
  flpModbus.configDataTable(FLPROG_HOLDING_REGISTR, 10);
  flpModbus.configDataTable(FLPROG_INPUT_REGISTR, 10);
  flpModbus.configDataTable(FLPROG_COIL, 10);
  flpModbus.configDataTable(FLPROG_DISCRETE_INPUT, 10);

  // sensor: ds18b20
  sensor_bus.begin();
  // Do not block the scan for 750 ms: request now, read on a later scan.
  sensor_bus.setWaitForConversion(false);
  sensor_bus.requestTemperatures();
  sensor_requested = millis();

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // wifi: wifi
  flpWifi.pool();
  wifi_ready = flpWifi.clientIsReady();

  // slave: modbus_tcp_slave
  flpModbus.pool();
  slave_error = flpModbus.getLastError();

  // sensor: ds18b20
  if (millis() - sensor_requested >= 1000UL) {
    float t = sensor_bus.getTempCByIndex(0);
    sensor_ok = t != DEVICE_DISCONNECTED_C;
    if (sensor_ok) {
      sensor_temp = t;
    }
    sensor_bus.requestTemperatures();
    sensor_requested = millis();
  }

  // temp_reg: modbus_write
  flpModbus.saveFloat(sensor_temp, FLPROG_INPUT_REGISTR, 0);

  // button: digital_in
  button_out = !digitalRead(13);

  // button_reg: modbus_write
  flpModbus.saveBool(button_out, FLPROG_DISCRETE_INPUT, 0);

  // online_reg: modbus_write
  flpModbus.saveBool(wifi_ready, FLPROG_DISCRETE_INPUT, 1);

  // led_reg: modbus_read
  led_reg_out_b = flpModbus.readBool(FLPROG_COIL, 0);

  // led: digital_out
  digitalWrite(2, (led_reg_out_b) ? HIGH : LOW);
}
