// Сгенерировано CrabLogic из проекта «mqtt_wasm». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → broker → sub → echo → is_on → led

#define FLPROG_MQTT_MAX_SUBSCRIPTIONS 2U

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
bool broker_connected = false;
int32_t broker_error = 0;
int32_t sub_out_i = 0;
bool sub_received = false;
bool echo_sent = false;
bool is_on_out = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
int32_t sub_out_i_pub = 0;
int32_t sub_out_i_in = 0;

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

// broker: mqtt
// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.
FLProgNetworkMqttClient flpMqtt;
bool broker_bound = false;
// The last received message, shared by all subscriptions.
String flpMqttTopic;
String flpMqttPayload;
uint32_t flpMqttSeq = 0;

// sub: mqtt_subscribe
uint32_t sub_seq = 0;
bool sub_subscribed = false;

// echo: mqtt_publish
bool echo_pending = false;
int32_t echo_last;
bool echo_started = false;

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

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
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
        flpMqtt.setKeepAlive(60);
        flpMqtt.setMessageCallback(flp_mqtt_receive);
        flpMqtt.setAutoReconnect(true);
        flpMqtt.connect();
      }
    }
    broker_connected = flpMqtt.connected();
    broker_error = flpMqtt.error();

    // sub: mqtt_subscribe
    {
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!sub_subscribed && flpMqtt.connected()) {
        sub_subscribed = flpMqtt.subscribe("flp/led");
      }
      sub_received = false;
      if (sub_seq != flpMqttSeq) {
        sub_seq = flpMqttSeq;
        if (flp_mqtt_match("flp/led", flpMqttTopic.c_str())) {
          sub_received = true;
          sub_out_i = flpMqttPayload.toInt();
        }
      }
    }

    // echo: mqtt_publish
    {
      bool changed = !echo_started || sub_out_i != echo_last;
      if (true && changed) {
        echo_pending = true;
      }
      echo_sent = false;
      if (echo_pending && flpMqtt.connected()) {
        String text = String(sub_out_i);
        if (flpMqtt.publish("flp/echo", text.c_str(), false)) {
          echo_sent = true;
          echo_pending = false;
          echo_last = sub_out_i;
          echo_started = true;
        }
      }
    }

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    sub_out_i_pub = sub_out_i;
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
  sub_out_i_in = sub_out_i_pub;
  xSemaphoreGive(flp_bg_lock);

  // is_on: greater
  is_on_out = (float)sub_out_i_in > 0.5f;

  // led: digital_out
  digitalWrite(2, (is_on_out) ? HIGH : LOW);
}
