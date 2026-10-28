// Сгенерировано FLP_next из проекта «mqtt». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → broker → sensor → temp_pub → button → button_pub → led_sub → led

#define FLPROG_MQTT_MAX_SUBSCRIPTIONS 4U

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <OneWire.h>
#include <DallasTemperature.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool broker_connected = false;
int32_t broker_error = 0;
float sensor_temp = 0.0f;
bool sensor_ok = false;
bool temp_pub_sent = false;
bool button_out = false;
bool button_pub_sent = false;
bool led_sub_out_b = false;
bool led_sub_received = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
float sensor_temp_pub = 0.0f;
float sensor_temp_in = 0.0f;
bool button_out_pub = false;
bool button_out_in = false;
bool button_out_pend = false;
bool led_sub_out_b_pub = false;
bool led_sub_out_b_in = false;
bool led_sub_out_b_pend = false;

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

// broker: mqtt
// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.
FLProgNetworkMqttClient flpMqtt;
bool broker_bound = false;
// The last received message, shared by all subscriptions.
String flpMqttTopic;
String flpMqttPayload;
uint32_t flpMqttSeq = 0;

// sensor: ds18b20
OneWire sensor_wire(4);
DallasTemperature sensor_bus(&sensor_wire);
unsigned long sensor_requested = 0;

// temp_pub: mqtt_publish
bool temp_pub_pending = false;
float temp_pub_last;
bool temp_pub_started = false;

// button_pub: mqtt_publish
bool button_pub_pending = false;
bool button_pub_last;
bool button_pub_started = false;

// led_sub: mqtt_subscribe
uint32_t led_sub_seq = 0;
bool led_sub_subscribed = false;

// mqtt: общий код
void flp_mqtt_receive(const char *topic, const uint8_t *payload, size_t length, bool retained) {
  (void)retained;
  flpMqttTopic = topic;
  flpMqttPayload = "";
  flpMqttPayload.reserve(length);
  for (size_t i = 0; i < length; i++) {
    flpMqttPayload += (char)payload[i];
  }
  flpMqttSeq++;
}

// Topic against a filter with + (one level) and # (the rest).
bool flp_mqtt_match(const char *f, const char *t) {
  for (;;) {
    const char *fe = strchr(f, '/');
    const char *te = strchr(t, '/');
    size_t fn = fe ? (size_t)(fe - f) : strlen(f);
    size_t tn = te ? (size_t)(te - t) : strlen(t);
    if (fn == 1 && *f == '#') {
      return true;
    }
    if (!(fn == 1 && *f == '+') && (fn != tn || strncmp(f, t, fn) != 0)) {
      return false;
    }
    if (!fe || !te) {
      return !fe && !te;
    }
    f = fe + 1;
    t = te + 1;
  }
}

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
    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    sensor_temp_in = sensor_temp_pub;
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

    // broker: mqtt
    if (!broker_bound) {
      // Attach once; the client reconnects by itself after that.
      broker_bound = flpMqtt.begin(flpWifi);
      if (broker_bound) {
        flpMqtt.setBroker("broker.hivemq.com");
        flpMqtt.setPort(1883);
        char cid[24];
        snprintf(cid, sizeof(cid), "flp-%08lx", (unsigned long)ESP.getEfuseMac());
        flpMqtt.setClientId(cid);
        flpMqtt.setCredentials("device", "");
        flpMqtt.setKeepAlive(60);
        flpMqtt.setMessageCallback(flp_mqtt_receive);
        flpMqtt.setAutoReconnect(true);
        flpMqtt.connect();
      }
    }
    broker_connected = flpMqtt.connected();
    broker_error = flpMqtt.error();

    // temp_pub: mqtt_publish
    {
      bool changed = !temp_pub_started || fabsf(sensor_temp_in - temp_pub_last) >= 0.2f;
      if (true && changed) {
        temp_pub_pending = true;
      }
      temp_pub_sent = false;
      if (temp_pub_pending && flpMqtt.connected()) {
        String text = String(sensor_temp_in, 1);
        if (flpMqtt.publish("flp/temp", text.c_str(), false)) {
          temp_pub_sent = true;
          temp_pub_pending = false;
          temp_pub_last = sensor_temp_in;
          temp_pub_started = true;
        }
      }
    }

    // button_pub: mqtt_publish
    {
      bool changed = !button_pub_started || button_out_in != button_pub_last;
      if (true && changed) {
        button_pub_pending = true;
      }
      button_pub_sent = false;
      if (button_pub_pending && flpMqtt.connected()) {
        String text = button_out_in ? "1" : "0";
        if (flpMqtt.publish("flp/button", text.c_str(), true)) {
          button_pub_sent = true;
          button_pub_pending = false;
          button_pub_last = button_out_in;
          button_pub_started = true;
        }
      }
    }

    // led_sub: mqtt_subscribe
    {
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!led_sub_subscribed && flpMqtt.connected()) {
        led_sub_subscribed = flpMqtt.subscribe("flp/led");
      }
      led_sub_received = false;
      if (led_sub_seq != flpMqttSeq) {
        led_sub_seq = flpMqttSeq;
        if (flp_mqtt_match("flp/led", flpMqttTopic.c_str())) {
          led_sub_received = true;
          {
            String p = flpMqttPayload;
            p.trim();
            p.toLowerCase();
            led_sub_out_b = p == "1" || p == "true" || p == "on";
          }
        }
      }
    }

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    led_sub_out_b_pub = led_sub_out_b;
    led_sub_out_b_pend = led_sub_out_b_pend || led_sub_out_b;
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
  wifi_ota.setHostname("wifi");

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
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  led_sub_out_b_in = led_sub_out_b_pub || led_sub_out_b_pend;
  led_sub_out_b_pend = false;
  xSemaphoreGive(flp_bg_lock);

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
  digitalWrite(2, (led_sub_out_b_in) ? HIGH : LOW);

  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  sensor_temp_pub = sensor_temp;
  button_out_pub = button_out;
  button_out_pend = button_out_pend || button_out;
  xSemaphoreGive(flp_bg_lock);
}
