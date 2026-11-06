// Сгенерировано CrabLogic из проекта «modbus_wifi». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → slave → sensor → temp_reg → button → button_reg → online_reg → led_reg → led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <flprogModbusTCP.h>
#include <OneWire.h>
#include <DallasTemperature.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
int32_t slave_error = 0;
float sensor_temp = 0.0f;
bool sensor_ok = false;
bool button_out = false;
bool led_reg_out_b = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;
float sensor_temp_pub = 0.0f;
float sensor_temp_in = 0.0f;
bool button_out_pub = false;
bool button_out_in = false;
bool button_out_pend = false;
bool led_reg_out_b_pub = false;
bool led_reg_out_b_in = false;
bool led_reg_out_b_pend = false;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface crabWifi;
// Network services (MQTT, web) are polled at most once per period.
uint32_t wifi_net_t = 0;
bool wifi_net_due = true;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;

// slave: modbus_tcp_slave
// Shared slave: modbus_read / modbus_write blocks access its tables.
ModbusSlaveTCP crabModbus(&crabWifi);

// sensor: ds18b20
OneWire sensor_wire(4);
DallasTemperature sensor_bus(&sensor_wire);
unsigned long sensor_requested = 0;

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void crab_bg_task(void *) {
  for (;;) {
    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    sensor_temp_in = sensor_temp_pub;
    button_out_in = button_out_pub || button_out_pend;
    button_out_pend = false;
    xSemaphoreGive(crab_bg_lock);

    // wifi: wifi
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = 0;
      uint8_t state = (uint8_t)(mode + 1);
      if (state != wifi_state) {
        wifi_state = state;
        if (state & 1) {
          crabWifi.clientOn();
        } else {
          crabWifi.clientOff();
        }
        if (state & 2) {
          crabWifi.apOn();
        } else {
          crabWifi.apOff();
        }
      }
    }
    {
      uint32_t now = millis();
      wifi_net_due = (uint32_t)(now - wifi_net_t) >= 5;
      if (wifi_net_due) {
        wifi_net_t = now;
      }
    }
    if (wifi_net_due) {
      crabWifi.pool();
    }
    wifi_ready = crabWifi.clientIsReady();
    {
      uint32_t ip = 0;
      if (crabWifi.clientIsReady()) {
        ip = (uint32_t)crabWifi.localIP();
      } else if (crabWifi.apIsReady()) {
        ip = (uint32_t)crabWifi.accessPointIP();
      }
      if (ip != wifi_ip_raw) {
        wifi_ip_raw = ip;
        wifi_address = ip ? IPAddress(ip).toString() : String();
      }
    }

    // slave: modbus_tcp_slave
    crabModbus.pool();
    slave_error = crabModbus.getLastError();

    // temp_reg: modbus_write
    crabModbus.saveFloat(sensor_temp_in, FLPROG_INPUT_REGISTR, 0);

    // button_reg: modbus_write
    crabModbus.saveBool(button_out_in, FLPROG_DISCRETE_INPUT, 0);

    // online_reg: modbus_write
    crabModbus.saveBool(wifi_ready, FLPROG_DISCRETE_INPUT, 1);

    // led_reg: modbus_read
    led_reg_out_b = crabModbus.readBool(FLPROG_COIL, 0);

    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    led_reg_out_b_pub = led_reg_out_b;
    led_reg_out_b_pend = led_reg_out_b_pend || led_reg_out_b;
    xSemaphoreGive(crab_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // wifi: wifi
  crabWifi.setClientSsid("MyNetwork");
  crabWifi.setClientPassword("");
  if (true) {
    crabWifi.setDhcp();
  } else {
    IPAddress addr;
    addr.fromString("192.168.1.177");
    crabWifi.localIP(addr);
    addr.fromString("192.168.1.1");
    crabWifi.gateway(addr);
    crabWifi.dns(addr);
    addr.fromString("255.255.255.0");
    crabWifi.subnet(addr);
    crabWifi.resetDhcp();
  }

  // slave: modbus_tcp_slave
  crabModbus.setSlaveAddress(1);
  crabModbus.setTcpPort(502);
  crabModbus.configDataTable(FLPROG_HOLDING_REGISTR, 10);
  crabModbus.configDataTable(FLPROG_INPUT_REGISTR, 10);
  crabModbus.configDataTable(FLPROG_COIL, 10);
  crabModbus.configDataTable(FLPROG_DISCRETE_INPUT, 10);

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

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
  led_reg_out_b_in = led_reg_out_b_pub || led_reg_out_b_pend;
  led_reg_out_b_pend = false;
  xSemaphoreGive(crab_bg_lock);

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

  // button: digital_in
  button_out = !digitalRead(13);

  // led: digital_out
  digitalWrite(2, (led_reg_out_b_in) ? HIGH : LOW);

  xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
  sensor_temp_pub = sensor_temp;
  button_out_pub = button_out;
  button_out_pend = button_out_pend || button_out;
  xSemaphoreGive(crab_bg_lock);
}
