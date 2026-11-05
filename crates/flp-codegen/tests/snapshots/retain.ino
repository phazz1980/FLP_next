// Сгенерировано CrabLogic из проекта «retain». Ручные правки будут перезаписаны.
// Порядок выполнения: button → press → total_r → one → next → total_w → limit_r → rest → reached → led → wifi_1 → mqtt_1 → mqtt_subscribe_1 → йц2_r1

#define FLPROG_MQTT_MAX_SUBSCRIPTIONS 4U

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
bool button_out = false;
bool press_out = false;
float total_r_out = 0.0f;
float one_out = 0.0f;
float next_out = 0.0f;
float limit_r_out = 0.0f;
float rest_out = 0.0f;
bool reached_out = false;
bool wifi_1_ready = false;
String wifi_1_address = String();
bool mqtt_1_connected = false;
int32_t mqtt_1_error = 0;
String mqtt_subscribe_1_out = String();
bool mqtt_subscribe_1_received = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
String mqtt_subscribe_1_out_pub = String();
String mqtt_subscribe_1_out_in = String();
bool mqtt_subscribe_1_received_pub = false;
bool mqtt_subscribe_1_received_in = false;
bool mqtt_subscribe_1_received_pend = false;

// Переменные
float var_Nazhatiy_vsego_cfcf80 = 0.0f;
float var_Porog_ba5e84 = 10.0f;
String var_yts2_6314dd = String();

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
float var_Nazhatiy_vsego_cfcf80_saved;  // последнее сохранённое «Нажатий всего»
float var_Porog_ba5e84_saved;  // последнее сохранённое «Порог»

// press: rising_edge
bool press_prev = false;

// wifi_1: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Network services (MQTT, web) are polled at most once per period.
uint32_t wifi_1_net_t = 0;
bool wifi_1_net_due = true;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_1_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_1_ip_raw = 0;

// mqtt_1: mqtt
// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.
FLProgNetworkMqttClient flpMqtt;
bool mqtt_1_bound = false;
// The last received message, shared by all subscriptions.
String flpMqttTopic;
String flpMqttPayload;
uint32_t flpMqttSeq = 0;

// mqtt_subscribe_1: mqtt_subscribe
uint32_t mqtt_subscribe_1_seq = 0;
bool mqtt_subscribe_1_subscribed = false;

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
    // wifi_1: wifi
    {
      // Client and access point follow the inputs; switched only when they change.
      int32_t mode = 0;
      uint8_t state = (uint8_t)(mode + 1);
      if (state != wifi_1_state) {
        wifi_1_state = state;
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
      wifi_1_net_due = (uint32_t)(now - wifi_1_net_t) >= 5;
      if (wifi_1_net_due) {
        wifi_1_net_t = now;
      }
    }
    if (wifi_1_net_due) {
      flpWifi.pool();
    }
    wifi_1_ready = flpWifi.clientIsReady();
    {
      uint32_t ip = 0;
      if (flpWifi.clientIsReady()) {
        ip = (uint32_t)flpWifi.localIP();
      } else if (flpWifi.apIsReady()) {
        ip = (uint32_t)flpWifi.accessPointIP();
      }
      if (ip != wifi_1_ip_raw) {
        wifi_1_ip_raw = ip;
        wifi_1_address = ip ? IPAddress(ip).toString() : String();
      }
    }

    // mqtt_1: mqtt
    if (!mqtt_1_bound) {
      // Attach once; the client reconnects by itself after that.
      mqtt_1_bound = flpMqtt.begin(flpWifi);
      if (mqtt_1_bound) {
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
    mqtt_1_connected = flpMqtt.connected();
    mqtt_1_error = flpMqtt.error();

    // mqtt_subscribe_1: mqtt_subscribe
    {
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!mqtt_subscribe_1_subscribed && flpMqtt.connected()) {
        mqtt_subscribe_1_subscribed = flpMqtt.subscribe("flp/command");
      }
      mqtt_subscribe_1_received = false;
      if (mqtt_subscribe_1_seq != flpMqttSeq) {
        mqtt_subscribe_1_seq = flpMqttSeq;
        if (flp_mqtt_match("flp/command", flpMqttTopic.c_str())) {
          mqtt_subscribe_1_received = true;
          mqtt_subscribe_1_out = flpMqttPayload;
        }
      }
    }

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    mqtt_subscribe_1_out_pub = mqtt_subscribe_1_out;
    mqtt_subscribe_1_received_pub = mqtt_subscribe_1_received;
    mqtt_subscribe_1_received_pend = mqtt_subscribe_1_received_pend || mqtt_subscribe_1_received;
    xSemaphoreGive(flp_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_1_address.reserve(32);
  mqtt_subscribe_1_out.reserve(32);
  var_yts2_6314dd.reserve(32);

  // Восстановление переменных из флеша
  flp_retain.begin("flp_vars", false);
  if (flp_retain.getBytesLength("v84a2d104") == sizeof(var_Nazhatiy_vsego_cfcf80)) {  // Нажатий всего
    flp_retain.getBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
  }
  var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
  if (flp_retain.getBytesLength("v49f1d568") == sizeof(var_Porog_ba5e84)) {  // Порог
    flp_retain.getBytes("v49f1d568", &var_Porog_ba5e84, sizeof(var_Porog_ba5e84));
  }
  var_Porog_ba5e84_saved = var_Porog_ba5e84;

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // one: constant
  one_out = 1.0f;

  // led: digital_out
  pinMode(2, OUTPUT);

  // wifi_1: wifi
  flpWifi.setClientSsid("asdf");
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

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  mqtt_subscribe_1_out_in = mqtt_subscribe_1_out_pub;
  mqtt_subscribe_1_received_in = mqtt_subscribe_1_received_pub || mqtt_subscribe_1_received_pend;
  mqtt_subscribe_1_received_pend = false;
  xSemaphoreGive(flp_bg_lock);

  // button: digital_in
  button_out = !digitalRead(13);

  // press: rising_edge
  press_out = button_out && !press_prev;
  press_prev = button_out;

  // total_r: var_get
  total_r_out = var_Nazhatiy_vsego_cfcf80;

  // next: add
  next_out = total_r_out + one_out;

  // total_w: var_set
  if (press_out) {
    var_Nazhatiy_vsego_cfcf80 = next_out;
  }

  // limit_r: var_get
  limit_r_out = var_Porog_ba5e84;

  // rest: sub
  rest_out = total_r_out - limit_r_out;

  // reached: greater
  reached_out = rest_out > -0.5f;

  // led: digital_out
  digitalWrite(2, (reached_out) ? HIGH : LOW);

  // йц2_r1: var_set
  if (mqtt_subscribe_1_received_in) {
    var_yts2_6314dd = mqtt_subscribe_1_out_in;
  }

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - flp_retain_ms >= 1000) {
    flp_retain_ms = millis();
    if (memcmp(&var_Nazhatiy_vsego_cfcf80, &var_Nazhatiy_vsego_cfcf80_saved, sizeof(var_Nazhatiy_vsego_cfcf80)) != 0) {  // Нажатий всего
      var_Nazhatiy_vsego_cfcf80_saved = var_Nazhatiy_vsego_cfcf80;
      flp_retain.putBytes("v84a2d104", &var_Nazhatiy_vsego_cfcf80, sizeof(var_Nazhatiy_vsego_cfcf80));
    }
    if (memcmp(&var_Porog_ba5e84, &var_Porog_ba5e84_saved, sizeof(var_Porog_ba5e84)) != 0) {  // Порог
      var_Porog_ba5e84_saved = var_Porog_ba5e84;
      flp_retain.putBytes("v49f1d568", &var_Porog_ba5e84, sizeof(var_Porog_ba5e84));
    }
  }
}
