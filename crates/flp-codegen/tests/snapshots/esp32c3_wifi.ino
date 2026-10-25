// Сгенерировано FLP_next из проекта «esp32c3_wifi». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → button → in2 → in3 → blink → led → out2 → out3

#include <Arduino.h>
#include <flprogNetwork.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool button_out = false;
bool in2_out = false;
bool in3_out = false;
bool blink_out = false;

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;

// blink: generator
unsigned long blink_start = 0;

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // wifi: wifi
  flpWifi.setClientSsid("Xiaomi_76D1");
  flpWifi.setClientPassword("");
  flpWifi.setDhcp();

  // button: digital_in
  pinMode(9, INPUT_PULLUP);

  // in2: digital_in
  pinMode(3, INPUT_PULLUP);

  // in3: digital_in
  pinMode(4, INPUT_PULLUP);

  // led: digital_out
  pinMode(8, OUTPUT);

  // out2: digital_out
  pinMode(5, OUTPUT);

  // out3: digital_out
  pinMode(6, OUTPUT);
}

void loop() {
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
  flpWifi.pool();
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

  // button: digital_in
  button_out = !digitalRead(9);

  // in2: digital_in
  in2_out = !digitalRead(3);

  // in3: digital_in
  in3_out = !digitalRead(4);

  // blink: generator
  if (button_out) {
    unsigned long phase = (millis() - blink_start) % (500UL);
    blink_out = phase < 250UL;
  } else {
    blink_start = millis();
    blink_out = false;
  }

  // led: digital_out
  digitalWrite(8, !(blink_out) ? HIGH : LOW);

  // out2: digital_out
  digitalWrite(5, (in2_out) ? HIGH : LOW);

  // out3: digital_out
  digitalWrite(6, (in3_out) ? HIGH : LOW);
}
