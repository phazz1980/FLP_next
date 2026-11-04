// Сгенерировано CrabLogic из проекта «net_eth_wasm». Ручные правки будут перезаписаны.
// Порядок выполнения: eth → wifi → clock → work → led → eth_len → wifi_len

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool eth_ready = false;
String eth_address = String();
bool wifi_ready = false;
String wifi_address = String();
bool clock_valid = false;
int32_t clock_hour = 0;
int32_t clock_minute = 0;
bool work_out = false;
int32_t eth_len_out = 0;
int32_t wifi_len_out = 0;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
bool eth_ready_pub = false;
bool eth_ready_in = false;
bool eth_ready_pend = false;
String eth_address_pub = String();
String eth_address_in = String();
bool wifi_ready_pub = false;
bool wifi_ready_in = false;
bool wifi_ready_pend = false;
String wifi_address_pub = String();
String wifi_address_in = String();
bool clock_valid_pub = false;
bool clock_valid_in = false;
bool clock_valid_pend = false;

// eth: ethernet
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgWiznetInterface flpEth(15);
// Network services (MQTT, web, OTA) are polled at most once per period.
uint32_t eth_net_t = 0;
bool eth_net_due = true;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta eth_ota;
bool eth_ota_on = false;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t eth_ip_raw = 0;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Network services (MQTT, web, OTA) are polled at most once per period.
uint32_t wifi_net_t = 0;
bool wifi_net_due = true;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta wifi_ota;
bool wifi_ota_on = false;

// clock: ntp
// The FLProg_Network SNTP service is the same on every board and network: it keeps
// the time in UTC itself, the zone and daylight saving are added below.
FLProgNetworkNtp clock_ntp;
bool clock_bound = false;
uint32_t clock_tick = 0;
uint32_t clock_retry = 0;

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    // eth: ethernet
    {
      uint32_t now = millis();
      eth_net_due = (uint32_t)(now - eth_net_t) >= 5;
      if (eth_net_due) {
        eth_net_t = now;
      }
    }
    if (eth_net_due) {
      flpEth.pool();
    }
    eth_ready = flpEth.ethernetIsReady();
    {
      uint32_t ip = flpEth.ethernetIsReady() ? (uint32_t)flpEth.localIP() : 0;
      if (ip != eth_ip_raw) {
        eth_ip_raw = ip;
        eth_address = ip ? IPAddress(ip).toString() : String();
      }
    }
    if (eth_net_due && true) {
      if (!eth_ota_on) {
        eth_ota_on = true;
        if (""[0] != 0) {
          eth_ota.setPasswordValue("");
        }
        eth_ota.begin(flpEth);
      }
      eth_ota.pool();
    }

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
    if (wifi_net_due && true) {
      if (!wifi_ota_on) {
        wifi_ota_on = true;
        if (""[0] != 0) {
          wifi_ota.setPasswordValue("");
        }
        wifi_ota.begin(flpWifi);
      }
      wifi_ota.pool();
    }

    // clock: ntp
    if (!clock_bound) {
      // Attach once; the service waits for the network and repeats requests by itself.
      clock_bound = clock_ntp.begin(flpEth);
      if (clock_bound) {
        clock_ntp.setServer("pool.ntp.org");
        clock_ntp.setRequestPeriodMs(3600000);
      }
    }
    // The time is read once per second.
    if (clock_bound && (uint32_t)(millis() - clock_tick) >= 1000) {
      clock_tick = millis();
      // After a failed request the service would wait for the whole period: ask again in 30 s.
      if (clock_ntp.failed() && (uint32_t)(millis() - clock_retry) >= 30000) {
        clock_retry = millis();
        clock_ntp.syncNow();
      }
      // Stays true while the service asks the server again.
      bool ok = clock_ntp.hasTime();
      clock_valid = ok;
      if (ok) {
        uint32_t utc = clock_ntp.unixTime();
        time_t local = (time_t)((int64_t)utc + (long)(3.0f * 3600) + 0);
        struct tm lt;
        gmtime_r(&local, &lt);
        clock_hour = lt.tm_hour;
        clock_minute = lt.tm_min;
      }
    }

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    eth_ready_pub = eth_ready;
    eth_ready_pend = eth_ready_pend || eth_ready;
    eth_address_pub = eth_address;
    wifi_ready_pub = wifi_ready;
    wifi_ready_pend = wifi_ready_pend || wifi_ready;
    wifi_address_pub = wifi_address;
    clock_valid_pub = clock_valid;
    clock_valid_pend = clock_valid_pend || clock_valid;
    xSemaphoreGive(flp_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  eth_address.reserve(32);
  wifi_address.reserve(32);

  // eth: ethernet
  flpEth.setSpiPins(14, 12, 13);
  flpEth.mac(0x78, 0xAC, 0xC0, 0x2C, 0x3E, 41);
  if (true) {
    flpEth.setDhcp();
  } else {
    IPAddress addr;
    addr.fromString("192.168.1.177");
    flpEth.localIP(addr);
    addr.fromString("192.168.1.1");
    flpEth.gateway(addr);
    flpEth.dns(addr);
    addr.fromString("255.255.255.0");
    flpEth.subnet(addr);
    flpEth.resetDhcp();
  }
  eth_ota.setHostname("eth");

  // wifi: wifi
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("secret123");
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
  wifi_ota.setHostname("wifi");

  // led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  eth_ready_in = eth_ready_pub || eth_ready_pend;
  eth_ready_pend = false;
  eth_address_in = eth_address_pub;
  wifi_ready_in = wifi_ready_pub || wifi_ready_pend;
  wifi_ready_pend = false;
  wifi_address_in = wifi_address_pub;
  clock_valid_in = clock_valid_pub || clock_valid_pend;
  clock_valid_pend = false;
  xSemaphoreGive(flp_bg_lock);

  // work: and
  work_out = eth_ready_in && wifi_ready_in && clock_valid_in;

  // led: digital_out
  digitalWrite(2, (work_out) ? HIGH : LOW);

  // eth_len: text_length
  eth_len_out = (int32_t)eth_address_in.length();

  // wifi_len: text_length
  wifi_len_out = (int32_t)wifi_address_in.length();
}
