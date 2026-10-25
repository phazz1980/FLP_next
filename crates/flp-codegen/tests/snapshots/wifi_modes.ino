// Сгенерировано FLP_next из проекта «wifi_modes». Ручные правки будут перезаписаны.
// Порядок выполнения: switch → mode → wifi → led

#include <Arduino.h>
#include <flprogNetwork.h>

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

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);

  // switch: digital_in
  pinMode(13, INPUT_PULLUP);

  // wifi: wifi
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("");
  flpWifi.setDhcp();
  flpWifi.setApSsid("FLP_ESP32");
  flpWifi.setApPassword("");

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

  // led: digital_out
  digitalWrite(2, (wifi_ready) ? HIGH : LOW);
}
