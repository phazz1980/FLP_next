// Сгенерировано CrabLogic из проекта «wifi_modes». Ручные правки будут перезаписаны.
// Порядок выполнения: switch → mode → wifi → led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool switch_out = false;
int32_t mode_out = 0;
bool wifi_ready = false;
bool wifi_ap_ready = false;
String wifi_address = String();
int32_t wifi_status = 0;
int32_t wifi_error = 0;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;
bool switch_out_pub = false;
bool switch_out_in = false;
bool switch_out_pend = false;
int32_t mode_out_pub = 0;
int32_t mode_out_in = 0;
bool wifi_ready_pub = false;
bool wifi_ready_in = false;
bool wifi_ready_pend = false;

// Переменные
int32_t var_Rezhim_Wi_Fi_fb481a = 2;

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

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void crab_bg_task(void *) {
  for (;;) {
    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    switch_out_in = switch_out_pub || switch_out_pend;
    switch_out_pend = false;
    mode_out_in = mode_out_pub;
    xSemaphoreGive(crab_bg_lock);

    // wifi: wifi
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = mode_out_in;
      if (mode < 0 || mode > 2) {
        mode = 0;
      }
      uint8_t state = switch_out_in ? (uint8_t)(mode + 1) : 0;
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
    wifi_ap_ready = crabWifi.apIsReady();
    wifi_status = crabWifi.getStatus();
    wifi_error = crabWifi.getError();
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

    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    wifi_ready_pub = wifi_ready;
    wifi_ready_pend = wifi_ready_pend || wifi_ready;
    xSemaphoreGive(crab_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // switch: digital_in
  pinMode(13, INPUT_PULLUP);

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
  crabWifi.setApSsid("CRAB_ESP32");
  crabWifi.setApPassword("");

  // led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
  wifi_ready_in = wifi_ready_pub || wifi_ready_pend;
  wifi_ready_pend = false;
  xSemaphoreGive(crab_bg_lock);

  // switch: digital_in
  switch_out = !digitalRead(13);

  // mode: var_get
  mode_out = var_Rezhim_Wi_Fi_fb481a;

  // led: digital_out
  digitalWrite(2, (wifi_ready_in) ? HIGH : LOW);

  xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
  switch_out_pub = switch_out;
  switch_out_pend = switch_out_pend || switch_out;
  mode_out_pub = mode_out;
  xSemaphoreGive(crab_bg_lock);
}
