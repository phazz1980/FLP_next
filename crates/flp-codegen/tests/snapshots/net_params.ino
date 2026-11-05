// Сгенерировано CrabLogic из проекта «net_params». Ручные правки будут перезаписаны.
// Порядок выполнения: net_r → pass_r → wifi → broker_r → login_r → broker → sub

#define FLPROG_MQTT_MAX_SUBSCRIPTIONS 4U

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
String net_r_out = String();
String pass_r_out = String();
bool wifi_ready = false;
String wifi_address = String();
String broker_r_out = String();
String login_r_out = String();
bool broker_connected = false;
int32_t broker_error = 0;
String sub_out = String();
bool sub_received = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
String net_r_out_pub = String();
String net_r_out_in = String();
String pass_r_out_pub = String();
String pass_r_out_in = String();
String broker_r_out_pub = String();
String broker_r_out_in = String();
String login_r_out_pub = String();
String login_r_out_in = String();

// Переменные
String var_Set_d8006b = String();
String var_Parol_23764f = String();
String var_Broker_a91631 = String();
String var_Login_498544 = String();

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
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

// broker: mqtt
// Settings signature (FNV-1a): compared every cycle without building a String.
uint32_t broker_cfg = 0;
bool broker_cfg_set = false;
static void broker_mix(uint32_t &h, const char *s) {
  for (; *s; s++) h = (h ^ (uint8_t)*s) * 16777619u;
  h = (h ^ '|') * 16777619u;
}
static void broker_mix(uint32_t &h, const String &s) { broker_mix(h, s.c_str()); }
uint32_t broker_sig = 0;
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
String sub_topic;

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
static uint32_t broker_signature() {
  uint32_t cfg = 2166136261u;
  broker_mix(cfg, (broker_r_out_in));
  broker_mix(cfg, (login_r_out_in));
  broker_mix(cfg, (login_r_out_in));
  broker_mix(cfg, (login_r_out_in));
  cfg = (cfg ^ (uint32_t)(1883)) * 16777619u;
  return cfg;
}

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    net_r_out_in = net_r_out_pub;
    pass_r_out_in = pass_r_out_pub;
    broker_r_out_in = broker_r_out_pub;
    login_r_out_in = login_r_out_pub;
    xSemaphoreGive(flp_bg_lock);

    // wifi: wifi
    {
      // Settings written while running are applied when they change.
      uint32_t cfg = 2166136261u;
      wifi_mix(cfg, (net_r_out_in));
      wifi_mix(cfg, (pass_r_out_in));
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
        flpWifi.setClientSsid((net_r_out_in));
        flpWifi.setClientPassword((pass_r_out_in));
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
      uint8_t state = (uint8_t)(mode + 1);
      if (!String((net_r_out_in)).length()) {
        state &= ~1;
      }
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

    // broker: mqtt
    if (!broker_bound) {
      // Attach once; the client reconnects by itself after that.
      broker_bound = flpMqtt.begin(flpWifi);
      if (broker_bound) {
        flpMqtt.setBroker(String((broker_r_out_in)).c_str());
        flpMqtt.setPort(1883);
        if (String((login_r_out_in)).length()) flpMqtt.setCredentials(String((login_r_out_in)).c_str(), String((login_r_out_in)).c_str()); else flpMqtt.clearCredentials();
        {
          String cid = String((login_r_out_in));
          if (!cid.length()) {
            char auto_id[24];
            snprintf(auto_id, sizeof(auto_id), "flp-%08lx", (unsigned long)ESP.getEfuseMac());
            cid = auto_id;
          }
          flpMqtt.setClientId(cid.c_str());
        }
        flpMqtt.setKeepAlive(60);
        flpMqtt.setMessageCallback(flp_mqtt_receive);
        flpMqtt.setAutoReconnect(true);
        broker_sig = broker_signature();
        flpMqtt.connect();
      }
    } else if (broker_signature() != broker_sig) {
      // Settings written while running: reconnect with the new ones.
      broker_sig = broker_signature();
      flpMqtt.disconnect();
      flpMqtt.setBroker(String((broker_r_out_in)).c_str());
      flpMqtt.setPort(1883);
      if (String((login_r_out_in)).length()) flpMqtt.setCredentials(String((login_r_out_in)).c_str(), String((login_r_out_in)).c_str()); else flpMqtt.clearCredentials();
      {
        String cid = String((login_r_out_in));
        if (!cid.length()) {
          char auto_id[24];
          snprintf(auto_id, sizeof(auto_id), "flp-%08lx", (unsigned long)ESP.getEfuseMac());
          cid = auto_id;
        }
        flpMqtt.setClientId(cid.c_str());
      }
      flpMqtt.connect();
    }
    broker_connected = flpMqtt.connected();
    broker_error = flpMqtt.error();

    // sub: mqtt_subscribe
    {
      if (sub_topic != String((login_r_out_in))) {
        sub_topic = String((login_r_out_in));
        sub_subscribed = false;
      }
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!sub_subscribed && flpMqtt.connected()) {
        sub_subscribed = flpMqtt.subscribe(String((login_r_out_in)).c_str());
      }
      sub_received = false;
      if (sub_seq != flpMqttSeq) {
        sub_seq = flpMqttSeq;
        if (flp_mqtt_match(String((login_r_out_in)).c_str(), flpMqttTopic.c_str())) {
          sub_received = true;
          sub_out = flpMqttPayload;
        }
      }
    }

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  net_r_out.reserve(32);
  pass_r_out.reserve(32);
  wifi_address.reserve(32);
  broker_r_out.reserve(32);
  login_r_out.reserve(32);
  sub_out.reserve(32);
  var_Set_d8006b.reserve(32);
  var_Parol_23764f.reserve(32);
  var_Broker_a91631.reserve(32);
  var_Login_498544.reserve(32);


  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  // net_r: var_get
  net_r_out = var_Set_d8006b;

  // pass_r: var_get
  pass_r_out = var_Parol_23764f;

  // broker_r: var_get
  broker_r_out = var_Broker_a91631;

  // login_r: var_get
  login_r_out = var_Login_498544;

  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  net_r_out_pub = net_r_out;
  pass_r_out_pub = pass_r_out;
  broker_r_out_pub = broker_r_out;
  login_r_out_pub = login_r_out;
  xSemaphoreGive(flp_bg_lock);
}
