// Сгенерировано FLP_next из проекта «wifi_modes». Ручные правки будут перезаписаны.
// Порядок выполнения: switch → mode → wifi → led

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>

// Выходы блоков
bool switch_out = false;
int32_t mode_out = 0;
bool wifi_ready = false;
bool wifi_ap_ready = false;
String wifi_address = String();
int32_t wifi_status = 0;
int32_t wifi_error = 0;

// Переменные
int32_t var_Rezhim_Wi_Fi_fb481a = 2;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;
// Stub web page: technical data of the device (port 80).
FLProgWebServer wifi_web(&flpWifi, 80);
// Shared web server: the web_ui block adds its pages to it.
FLProgWebServer &flpWebSrv = wifi_web;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta wifi_ota;
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

void wifi_page(FLProgNetworkHttpServer *server) {
  char mac[18];
  uint8_t *m = flpWifi.mac();
  snprintf(mac, sizeof(mac), "%02X:%02X:%02X:%02X:%02X:%02X", m[0], m[1], m[2], m[3], m[4], m[5]);
  uint32_t sec = millis() / 1000;
  String page = F("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>wifi</title></head><body><h1>FLP_next</h1><table>");
  page += F("<tr><td>IP-адрес</td><td>"); page += flpWifi.localIP().toString(); page += F("</td></tr>");
  page += F("<tr><td>MAC</td><td>"); page += mac; page += F("</td></tr>");
  page += F("<tr><td>Сигнал Wi-Fi</td><td>"); page += String(flpWifi.rssi()); page += F(" дБм</td></tr>");
  page += F("<tr><td>Аптайм</td><td>"); page += String(sec / 3600); page += F(" ч "); page += String(sec / 60 % 60); page += F(" мин</td></tr>");
  page += F("<tr><td>Сборка</td><td>" __DATE__ " " __TIME__ "</td></tr>");
  page += F("<tr><td>Обновление</td><td>по сети (OTA), имя: "); page += wifi_ota.getName(); page += F("</td></tr>");
  page += F("</table>");
#ifdef FLP_WEBUI
  page += F("<p><a href=\"/ui\">Панель управления</a></p>");
#endif
  page += F("<h2>Обновление прошивки</h2><input type=file id=f accept=.bin><button onclick=\"u()\">Загрузить</button> <span id=s></span><script>function u(){var f=f.files[0];if(!f)return;var p=prompt('Пароль','');s.textContent='...';fetch('http://'+location.hostname+':8080/update?p='+encodeURIComponent(p||''),{method:'POST',headers:{'Content-Type':'text/plain'},body:f}).then(function(r){return r.text()}).then(function(x){s.textContent=x}).catch(function(){s.textContent='ошибка'})}</script>");
  page += F("</body></html>");
  server->send(200, "text/html", page);
}

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // switch: digital_in
  pinMode(13, INPUT_PULLUP);

  // wifi: wifi
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("");
  if (True) {
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
  flpWifi.setApSsid("FLP_ESP32");
  flpWifi.setApPassword("");
  wifi_web.addHandler("/", wifi_page);
  wifi_ota.setHostname("wifi");
  wifi_ota.begin(flpWifi);

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // switch: digital_in
  switch_out = !digitalRead(13);

  // mode: var_get
  mode_out = var_Rezhim_Wi_Fi_fb481a;

  // wifi: wifi
  {
    // Client and access point follow the inputs; switched only when they change.
    int32_t mode = mode_out;
    if (mode < 0 || mode > 2) {
      mode = 0;
    }
    uint8_t state = switch_out ? (uint8_t)(mode + 1) : 0;
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
  flpWifi.pool();
  wifi_ready = flpWifi.clientIsReady();
  wifi_ap_ready = flpWifi.apIsReady();
  wifi_status = flpWifi.getStatus();
  wifi_error = flpWifi.getError();
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
  wifi_web.pool();
  wifi_up_poll();
  wifi_ota.pool();

  // led: digital_out
  digitalWrite(2, (wifi_ready) ? HIGH : LOW);
}
