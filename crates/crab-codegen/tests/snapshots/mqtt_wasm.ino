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
SemaphoreHandle_t crab_bg_lock;
int32_t sub_out_i_pub = 0;
int32_t sub_out_i_in = 0;

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

// broker: mqtt
// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.
FLProgNetworkMqttClient crabMqtt;
bool broker_bound = false;
// The last received message, shared by all subscriptions.
String crabMqttTopic;
String crabMqttPayload;
uint32_t crabMqttSeq = 0;

// sub: mqtt_subscribe
uint32_t sub_seq = 0;
bool sub_subscribed = false;

// echo: mqtt_publish
bool echo_pending = false;
int32_t echo_last;
bool echo_started = false;

// mqtt: общий код
void crab_mqtt_receive(const char *topic, const uint8_t *payload, size_t length, bool retained) {
  (void)retained;
  crabMqttTopic = topic;
  crabMqttPayload = "";
  crabMqttPayload.reserve(length);
  for (size_t i = 0; i < length; i++) {
    crabMqttPayload += (char)payload[i];
  }
  crabMqttSeq++;
}

// Topic against a filter with + (one level) and # (the rest).
bool crab_mqtt_match(const char *f, const char *t) {
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

    // broker: mqtt
    if (!broker_bound) {
      // Attach once; the client reconnects by itself after that.
      broker_bound = crabMqtt.begin(crabWifi);
      if (broker_bound) {
        crabMqtt.setBroker("broker.hivemq.com");
        crabMqtt.setPort(1883);
        char cid[24];
        snprintf(cid, sizeof(cid), "crab-%08lx", (unsigned long)ESP.getEfuseMac());
        crabMqtt.setClientId(cid);
        crabMqtt.setKeepAlive(60);
        crabMqtt.setMessageCallback(crab_mqtt_receive);
        crabMqtt.setAutoReconnect(true);
        crabMqtt.connect();
      }
    }
    broker_connected = crabMqtt.connected();
    broker_error = crabMqtt.error();

    // sub: mqtt_subscribe
    {
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!sub_subscribed && crabMqtt.connected()) {
        sub_subscribed = crabMqtt.subscribe("crab/led");
      }
      sub_received = false;
      if (sub_seq != crabMqttSeq) {
        sub_seq = crabMqttSeq;
        if (crab_mqtt_match("crab/led", crabMqttTopic.c_str())) {
          sub_received = true;
          sub_out_i = crabMqttPayload.toInt();
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
      if (echo_pending && crabMqtt.connected()) {
        String text = String(sub_out_i);
        if (crabMqtt.publish("crab/echo", text.c_str(), false)) {
          echo_sent = true;
          echo_pending = false;
          echo_last = sub_out_i;
          echo_started = true;
        }
      }
    }

    xSemaphoreTake(crab_bg_lock, portMAX_DELAY);
    sub_out_i_pub = sub_out_i;
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
  sub_out_i_in = sub_out_i_pub;
  xSemaphoreGive(crab_bg_lock);

  // is_on: greater
  is_on_out = (float)sub_out_i_in > 0.5f;

  // led: digital_out
  digitalWrite(2, (is_on_out) ? HIGH : LOW);
}
