// Сгенерировано CrabLogic из проекта «modbus_tcp_master». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → master → temp → button → led_reg → online_led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <flprogModbusTCP.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
float temp_out_f = 0.0f;
bool temp_ok = false;
bool button_out = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
bool temp_ok_pub = false;
bool temp_ok_in = false;
bool temp_ok_pend = false;
bool button_out_pub = false;
bool button_out_in = false;
bool button_out_pend = false;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Network services (MQTT, web) are polled at most once per period.
uint32_t wifi_net_t = 0;
bool wifi_net_due = true;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;

// master: modbus_tcp_master
// Shared master: modbus_master_read / modbus_master_write blocks (master = tcp) access its slaves.
ModbusMasterTCP flpMasterTcp(&flpWifi, 1);

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    button_out_in = button_out_pub || button_out_pend;
    button_out_pend = false;
    xSemaphoreGive(flp_bg_lock);

    // wifi: wifi
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = 0;
      uint8_t state = (uint8_t)(mode + 1);
      if (state != wifi_state) {
        wifi_state = state;
        if (state & 1) {
          flpWifi.clientOn();
        } else {
          flpWifi.clientOff();
        }
        if (state & 2) {
          flpWifi.apOn();
        } else {
          flpWifi.apOff();
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
      flpWifi.pool();
    }
    wifi_ready = flpWifi.clientIsReady();
    {
      uint32_t ip = 0;
      if (flpWifi.clientIsReady()) {
        ip = (uint32_t)flpWifi.localIP();
      } else if (flpWifi.apIsReady()) {
        ip = (uint32_t)flpWifi.accessPointIP();
      }
      if (ip != wifi_ip_raw) {
        wifi_ip_raw = ip;
        wifi_address = ip ? IPAddress(ip).toString() : String();
      }
    }

    // master: modbus_tcp_master
    flpMasterTcp.pool();

    // temp: modbus_master_read
    temp_out_f = flpMasterTcp.readFloat(0, 1, FLPROG_INPUT_REGISTR, 0);
    temp_ok = flpMasterTcp.slaveStatus(0, 1, false);

    // led_reg: modbus_master_write
    flpMasterTcp.saveBool(0, 1, button_out_in, FLPROG_COIL, 0);

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    temp_ok_pub = temp_ok;
    temp_ok_pend = temp_ok_pend || temp_ok;
    xSemaphoreGive(flp_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // wifi: wifi
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("");
  if (true) {
    flpWifi.setDhcp();
  } else {
    IPAddress addr;
    addr.fromString("192.168.1.177");
    flpWifi.localIP(addr);
    addr.fromString("192.168.1.1");
    flpWifi.gateway(addr);
    flpWifi.dns(addr);
    addr.fromString("255.255.255.0");
    flpWifi.subnet(addr);
    flpWifi.resetDhcp();
  }

  // master: modbus_tcp_master
  flpMasterTcp.setServerIpAdress(0, IPAddress(192, 168, 1, 50));
  flpMasterTcp.setServerPort(0, 502);
  flpMasterTcp.setServerSlavesSize(0, 1);
  flpMasterTcp.setSlaveAddress(0, 0, 1);
  flpMasterTcp.setPollingPeriod(0, 1, 500UL);
  flpMasterTcp.setTimeOutTime(0, 1, 1000UL);
  flpMasterTcp.configDataTable(0, 1, FLPROG_INPUT_REGISTR, 2);
  flpMasterTcp.configDataTable(0, 1, FLPROG_COIL, 1);

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // online_led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  temp_ok_in = temp_ok_pub || temp_ok_pend;
  temp_ok_pend = false;
  xSemaphoreGive(flp_bg_lock);

  // button: digital_in
  button_out = !digitalRead(13);

  // online_led: digital_out
  digitalWrite(2, (temp_ok_in) ? HIGH : LOW);

  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  button_out_pub = button_out;
  button_out_pend = button_out_pend || button_out;
  xSemaphoreGive(flp_bg_lock);
}
