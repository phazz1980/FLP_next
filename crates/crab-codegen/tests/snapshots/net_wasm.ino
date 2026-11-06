// Сгенерировано CrabLogic из проекта «net_wasm». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → clock → after8 → before20 → work → led → addr_len → time_len

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool clock_valid = false;
int32_t clock_hour = 0;
int32_t clock_minute = 0;
int32_t clock_unix = 0;
String clock_text = String();
bool after8_out = false;
bool before20_out = false;
bool work_out = false;
int32_t addr_len_out = 0;
int32_t time_len_out = 0;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;
String wifi_address_pub = String();
String wifi_address_in = String();
bool clock_valid_pub = false;
bool clock_valid_in = false;
bool clock_valid_pend = false;
int32_t clock_hour_pub = 0;
int32_t clock_hour_in = 0;
String clock_text_pub = String();
String clock_text_in = String();

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

// clock: ntp
// The FLProg_Network SNTP service is the same on every board and network: it keeps
// the time in UTC itself, the zone and daylight saving are added below.
FLProgNetworkNtp clock_ntp;
bool clock_bound = false;
uint32_t clock_tick = 0;
uint32_t clock_retry = 0;

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void crab_bg_task(void *) {
  for (;;) {
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

    // clock: ntp
    if (!clock_bound) {
      // Attach once; the service waits for the network and repeats requests by itself.
      clock_bound = clock_ntp.begin(crabWifi);
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
        clock_unix = (int32_t)utc;
        time_t local = (time_t)((int64_t)utc + (long)(3.0f * 3600) + 0);
        struct tm lt;
        gmtime_r(&local, &lt);
        clock_hour = lt.tm_hour;
        clock_minute = lt.tm_min;
        char buf[9];
        snprintf(buf, sizeof(buf), "%02d:%02d:%02d", lt.tm_hour, lt.tm_min, lt.tm_sec);
        clock_text = buf;
      }
    }

    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    wifi_address_pub = wifi_address;
    clock_valid_pub = clock_valid;
    clock_valid_pend = clock_valid_pend || clock_valid;
    clock_hour_pub = clock_hour;
    clock_text_pub = clock_text;
    xSemaphoreGive(crab_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);
  clock_text.reserve(32);

  // wifi: wifi
  crabWifi.setClientSsid("MyNetwork");
  crabWifi.setClientPassword("secret123");
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

  // led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
  wifi_address_in = wifi_address_pub;
  clock_valid_in = clock_valid_pub || clock_valid_pend;
  clock_valid_pend = false;
  clock_hour_in = clock_hour_pub;
  clock_text_in = clock_text_pub;
  xSemaphoreGive(crab_bg_lock);

  // after8: greater
  after8_out = (float)clock_hour_in > 7.5f;

  // before20: less
  before20_out = (float)clock_hour_in < 20.0f;

  // work: and
  work_out = after8_out && before20_out && clock_valid_in;

  // led: digital_out
  digitalWrite(2, (work_out) ? HIGH : LOW);

  // addr_len: text_length
  addr_len_out = (int32_t)wifi_address_in.length();

  // time_len: text_length
  time_len_out = (int32_t)clock_text_in.length();
}
