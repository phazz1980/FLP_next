// Сгенерировано CrabLogic из проекта «mqtt». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → broker → sensor → temp_pub → button → button_pub → led_sub → temp_w → button_w → led_r → led

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
int32_t wifi_status = 0;
int32_t wifi_error = 0;
bool broker_connected = false;
int32_t broker_error = 0;
float sensor_temp = 0.0f;
bool sensor_ok = false;
bool temp_pub_sent = false;
bool button_out = false;
bool button_pub_sent = false;
bool led_sub_out_b = false;
bool led_sub_received = false;
bool led_r_out = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;

// Переменные
float var_Temperatura_32c585 = 0.0f;
bool var_Knopka_c26f61 = false;
bool var_Svetodiod_84f54c = false;

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
      wifi_net_due = (uint32_t)(now - wifi_net_t) >= (5);
      if (wifi_net_due) {
        wifi_net_t = now;
      }
    }
    if (wifi_net_due) {
      crabWifi.pool();
    }
    wifi_ready = crabWifi.clientIsReady();
    wifi_status = crabWifi.getStatus();
    wifi_error = crabWifi.getError();
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
        crabMqtt.setCredentials("device", "");
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

    // temp_pub: mqtt_publish
    {
      bool changed = !temp_pub_started || fabsf(var_Temperatura_32c585 - temp_pub_last) >= 0.2f;
      if (true && changed) {
        temp_pub_pending = true;
      }
      temp_pub_sent = false;
      if (temp_pub_pending && crabMqtt.connected()) {
        String text = String(var_Temperatura_32c585, 1);
        if (crabMqtt.publish("crab/temp", text.c_str(), false)) {
          temp_pub_sent = true;
          temp_pub_pending = false;
          temp_pub_last = var_Temperatura_32c585;
          temp_pub_started = true;
        }
      }
    }

    // button_pub: mqtt_publish
    {
      bool changed = !button_pub_started || var_Knopka_c26f61 != button_pub_last;
      if (true && changed) {
        button_pub_pending = true;
      }
      button_pub_sent = false;
      if (button_pub_pending && crabMqtt.connected()) {
        String text = var_Knopka_c26f61 ? "1" : "0";
        if (crabMqtt.publish("crab/button", text.c_str(), true)) {
          button_pub_sent = true;
          button_pub_pending = false;
          button_pub_last = var_Knopka_c26f61;
          button_pub_started = true;
        }
      }
    }

    // led_sub: mqtt_subscribe
    {
      // The client keeps the subscription across reconnects; ask again only until it is accepted.
      if (!led_sub_subscribed && crabMqtt.connected()) {
        led_sub_subscribed = crabMqtt.subscribe("crab/led");
      }
      led_sub_received = false;
      if (led_sub_seq != crabMqttSeq) {
        led_sub_seq = crabMqttSeq;
        if (crab_mqtt_match("crab/led", crabMqttTopic.c_str())) {
          led_sub_received = true;
          {
            String p = crabMqttPayload;
            p.trim();
            p.toLowerCase();
            led_sub_out_b = p == "1" || p == "true" || p == "on";
          }
        }
      }
    }
    var_Svetodiod_84f54c = led_sub_out_b;

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // wifi: wifi
  crabWifi.setClientSsid("MyNetwork");
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
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
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

  // temp_w: var_set
  var_Temperatura_32c585 = sensor_temp;

  // button_w: var_set
  var_Knopka_c26f61 = button_out;

  // led_r: var_get
  led_r_out = var_Svetodiod_84f54c;

  // led: digital_out
  digitalWrite(2, (led_r_out) ? HIGH : LOW);
}
