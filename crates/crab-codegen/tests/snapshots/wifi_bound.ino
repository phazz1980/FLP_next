// Сгенерировано CrabLogic из проекта «wifi_modes». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → ready → led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <Preferences.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool ready_out = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;

// Переменные
String var_Set_d8006b = String("MyNetwork");
bool var_Wi_Fi_vklyuchen_4f8fe0 = true;
bool var_Wi_Fi_gotov_6c4b6b = false;
String var_IP_adres_1ea70f = String();

// Сохранение переменных во флеш
Preferences crab_retain;
uint32_t crab_retain_ms = 0;
String var_Set_d8006b_saved;  // последнее сохранённое «Сеть»

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface crabWifi;
// Network services (MQTT, web) are polled at most once per period.
uint32_t wifi_net_t = 0;
bool wifi_net_due = true;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Settings signature (FNV-1a): compared every cycle without building a String.
uint32_t wifi_cfg = 0;
bool wifi_cfg_set = false;
static void wifi_mix(uint32_t &h, const char *s) {
  for (; *s; s++) h = (h ^ (uint8_t)*s) * 16777619u;
  h = (h ^ '|') * 16777619u;
}
static void wifi_mix(uint32_t &h, const String &s) { wifi_mix(h, s.c_str()); }
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void crab_bg_task(void *) {
  for (;;) {
    // wifi: wifi
    {
      // Settings written while running are applied when they change.
      uint32_t cfg = 2166136261u;
      wifi_mix(cfg, var_Set_d8006b.c_str());
      wifi_mix(cfg, "");
      wifi_mix(cfg, (true) ? "1" : "0");
      wifi_mix(cfg, "192.168.1.177");
      wifi_mix(cfg, "192.168.1.1");
      wifi_mix(cfg, "255.255.255.0");
      wifi_mix(cfg, "CRAB_ESP32");
      wifi_mix(cfg, "");
      if (!wifi_cfg_set || cfg != wifi_cfg) {
        bool first = !wifi_cfg_set;
        wifi_cfg_set = true;
        wifi_cfg = cfg;
        crabWifi.setClientSsid(var_Set_d8006b.c_str());
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
        if (!first) {
          crabWifi.clientOff();
          crabWifi.apOff();
          wifi_state = 255;
        }
      }
    }
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = 0;
      uint8_t state = var_Wi_Fi_vklyuchen_4f8fe0 ? (uint8_t)(mode + 1) : 0;
      if (!String(var_Set_d8006b.c_str()).length()) {
        state &= ~1;
      }
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
    var_IP_adres_1ea70f = wifi_address;
    var_Wi_Fi_gotov_6c4b6b = wifi_ready;

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);
  var_Set_d8006b.reserve(32);
  var_IP_adres_1ea70f.reserve(32);

  // Восстановление переменных из флеша
  crab_retain.begin("crab_vars", false);
  if (crab_retain.isKey("v74c5bd66")) {  // Сеть
    var_Set_d8006b = crab_retain.getString("v74c5bd66");
  }
  var_Set_d8006b_saved = var_Set_d8006b;

  // led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  // ready: var_get
  ready_out = var_Wi_Fi_gotov_6c4b6b;

  // led: digital_out
  digitalWrite(2, (ready_out) ? HIGH : LOW);

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - crab_retain_ms >= 1000) {
    crab_retain_ms = millis();
    if (var_Set_d8006b != var_Set_d8006b_saved) {  // Сеть
      var_Set_d8006b_saved = var_Set_d8006b;
      crab_retain.putString("v74c5bd66", var_Set_d8006b);
    }
  }
}
