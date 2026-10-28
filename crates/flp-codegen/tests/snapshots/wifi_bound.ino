// Сгенерировано FLP_next из проекта «wifi_modes». Ручные правки будут перезаписаны.
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
SemaphoreHandle_t flp_bg_lock;

// Переменные
String var_Set_d8006b = String("MyNetwork");
bool var_Wi_Fi_vklyuchen_4f8fe0 = true;
bool var_Wi_Fi_gotov_6c4b6b = false;
String var_IP_adres_1ea70f = String();

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
String var_Set_d8006b_saved;  // последнее сохранённое «Сеть»

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Network services (MQTT, web, OTA) are polled at most once per period.
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
// Web server (port 80): the web_ui block adds its pages to it.
FLProgWebServer wifi_web(&flpWifi, 80);
// Shared web server: the web_ui block adds its pages to it.
FLProgWebServer &flpWebSrv = wifi_web;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta wifi_ota;
bool wifi_ota_on = false;
#define FLP_WEB_UPD 1
// Firmware upload from the web page: raw HTTP POST streamed into Update (port 8080).
FLProgNetworkTcpServer wifi_up;
FLProgNetworkTcpServerClient *wifi_up_c = nullptr;
bool wifi_up_on = false;
uint8_t wifi_up_st = 0;  // 0 idle, 1 headers, 2 body, 3 done (restart pending)
char wifi_up_line[160];
uint8_t wifi_up_ll = 0;
uint32_t wifi_up_len = 0, wifi_up_got = 0, wifi_up_t = 0, wifi_up_try = 0;
bool wifi_up_auth = false;

// wifi: wifi
void wifi_up_reply(const char *status, const char *text) {
  String r = String("HTTP/1.1 ") + status + "\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: " + String(strlen(text)) + "\r\n\r\n" + text;
  wifi_up_c->write((const uint8_t *)r.c_str(), r.length());
}

void wifi_up_fail(const char *status, const char *text) {
  if (wifi_up_st == 2) {
    Update.abort();
  }
  wifi_up_reply(status, text);
  wifi_up_c->stop();
  wifi_up_c = nullptr;
  wifi_up_st = 0;
}

// Header line: the request line is checked for path and password, Content-Length is remembered.
void wifi_up_header_line() {
  char *l = wifi_up_line;
  if (strncmp(l, "POST /update", 12) == 0) {
    String expect = String("?p=") + "" + " ";
    wifi_up_auth = strlen("") == 0 || strstr(l, expect.c_str()) != nullptr;
  } else if (strncasecmp(l, "Content-Length:", 15) == 0) {
    wifi_up_len = (uint32_t)strtoul(l + 15, nullptr, 10);
  }
}

void wifi_up_poll() {
  if (!wifi_up_on) {
    if (millis() - wifi_up_try > 2000) {
      wifi_up_try = millis();
      wifi_up_on = wifi_up.begin(flpWifi, 8080);
    }
    return;
  }
  wifi_up.pool();
  if (wifi_up_c == nullptr) {
    wifi_up_c = wifi_up.takeNewClient();
    if (wifi_up_c == nullptr) {
      return;
    }
    wifi_up_st = 1;
    wifi_up_ll = 0;
    wifi_up_len = 0;
    wifi_up_got = 0;
    wifi_up_auth = false;
    wifi_up_t = millis();
  }
  if (wifi_up_st == 3) {
    if (millis() - wifi_up_t > 800) {
      ESP.restart();
    }
    return;
  }
  if (!wifi_up_c->active() || millis() - wifi_up_t > 10000) {
    if (wifi_up_st == 2) {
      Update.abort();
    }
    wifi_up_c->stop();
    wifi_up_c = nullptr;
    wifi_up_st = 0;
    return;
  }
  if (wifi_up_st == 1) {
    for (uint16_t n = 0; n < 512 && wifi_up_st == 1; n++) {
      int b = wifi_up_c->read();
      if (b < 0) {
        break;
      }
      wifi_up_t = millis();
      if (b == '\n') {
        wifi_up_line[wifi_up_ll] = 0;
        if (wifi_up_ll > 0 && wifi_up_line[wifi_up_ll - 1] == '\r') {
          wifi_up_line[wifi_up_ll - 1] = 0;
        }
        bool empty = wifi_up_line[0] == 0;
        if (!empty) {
          wifi_up_header_line();
        }
        wifi_up_ll = 0;
        if (empty) {
          if (!wifi_up_auth) {
            wifi_up_fail("403 Forbidden", "Неверный пароль");
          } else if (wifi_up_len == 0 || !Update.begin(wifi_up_len)) {
            wifi_up_fail("500 Internal Server Error", "Нет места под прошивку");
          } else {
            wifi_up_st = 2;
          }
        }
      } else if (wifi_up_ll < sizeof(wifi_up_line) - 1) {
        wifi_up_line[wifi_up_ll++] = (char)b;
      }
    }
  }
  if (wifi_up_st == 2) {
    for (uint8_t n = 0; n < 4 && wifi_up_got < wifi_up_len; n++) {
      uint8_t buf[512];
      uint32_t want = wifi_up_len - wifi_up_got;
      int got = wifi_up_c->read(buf, want < sizeof(buf) ? want : sizeof(buf));
      if (got <= 0) {
        break;
      }
      wifi_up_t = millis();
      if (Update.write(buf, (size_t)got) != (size_t)got) {
        wifi_up_fail("500 Internal Server Error", "Ошибка записи прошивки");
        return;
      }
      wifi_up_got += (uint32_t)got;
    }
    if (wifi_up_got >= wifi_up_len) {
      if (Update.end(true)) {
        wifi_up_reply("200 OK", "Прошивка загружена, устройство перезагружается");
        wifi_up_st = 3;
        wifi_up_t = millis();
      } else {
        wifi_up_fail("500 Internal Server Error", "Прошивка не принята");
      }
    }
  }
}

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
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
      wifi_mix(cfg, "FLP_ESP32");
      wifi_mix(cfg, "");
      if (!wifi_cfg_set || cfg != wifi_cfg) {
        bool first = !wifi_cfg_set;
        wifi_cfg_set = true;
        wifi_cfg = cfg;
        flpWifi.setClientSsid(var_Set_d8006b.c_str());
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
        if (!first) {
          flpWifi.clientOff();
          flpWifi.apOff();
          wifi_state = 255;
        }
      }
    }
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = 0;
      uint8_t state = var_Wi_Fi_vklyuchen_4f8fe0 ? (uint8_t)(mode + 1) : 0;
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
      wifi_web.pool();
    }
    if (wifi_net_due && true && true) {
      wifi_up_poll();
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
    var_IP_adres_1ea70f = wifi_address;
    var_Wi_Fi_gotov_6c4b6b = wifi_ready;

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);
  var_Set_d8006b.reserve(32);
  var_IP_adres_1ea70f.reserve(32);

  // Восстановление переменных из флеша
  flp_retain.begin("flp_vars", false);
  if (flp_retain.isKey("v74c5bd66")) {  // Сеть
    var_Set_d8006b = flp_retain.getString("v74c5bd66");
  }
  var_Set_d8006b_saved = var_Set_d8006b;

  // wifi: wifi
  wifi_ota.setHostname("wifi");

  // led: digital_out
  pinMode(2, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  // ready: var_get
  ready_out = var_Wi_Fi_gotov_6c4b6b;

  // led: digital_out
  digitalWrite(2, (ready_out) ? HIGH : LOW);

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - flp_retain_ms >= 1000) {
    flp_retain_ms = millis();
    if (var_Set_d8006b != var_Set_d8006b_saved) {  // Сеть
      var_Set_d8006b_saved = var_Set_d8006b;
      flp_retain.putString("v74c5bd66", var_Set_d8006b);
    }
  }
}
