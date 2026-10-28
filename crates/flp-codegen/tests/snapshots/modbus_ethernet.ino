// Сгенерировано FLP_next из проекта «modbus_ethernet». Ручные правки будут перезаписаны.
// Порядок выполнения: eth → slave → button → button_reg → online_reg → error_reg → relay_reg → relay

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>
#include <flprogModbusTCP.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
bool eth_ready = false;
int32_t slave_error = 0;
bool button_out = false;
bool relay_reg_out_b = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t flp_bg_lock;
bool button_out_pub = false;
bool button_out_in = false;
bool button_out_pend = false;
bool relay_reg_out_b_pub = false;
bool relay_reg_out_b_in = false;
bool relay_reg_out_b_pend = false;

// eth: ethernet
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgWiznetInterface flpEth(5);
// Network services (MQTT, web, OTA) are polled at most once per period.
uint32_t eth_net_t = 0;
bool eth_net_due = true;
// Web server (port 80): the web_ui block adds its pages to it.
FLProgWebServer eth_web(&flpEth, 80);
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta eth_ota;
bool eth_ota_on = false;
// Firmware upload from the web page: raw HTTP POST streamed into Update (port 8080).
FLProgNetworkTcpServer eth_up;
FLProgNetworkTcpServerClient *eth_up_c = nullptr;
bool eth_up_on = false;
uint8_t eth_up_st = 0;  // 0 idle, 1 headers, 2 body, 3 done (restart pending)
char eth_up_line[160];
uint8_t eth_up_ll = 0;
uint32_t eth_up_len = 0, eth_up_got = 0, eth_up_t = 0, eth_up_try = 0;
bool eth_up_auth = false;

// slave: modbus_tcp_slave
// Shared slave: modbus_read / modbus_write blocks access its tables.
ModbusSlaveTCP flpModbus(&flpEth);

// eth: ethernet
void eth_up_reply(const char *status, const char *text) {
  String r = String("HTTP/1.1 ") + status + "\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: " + String(strlen(text)) + "\r\n\r\n" + text;
  eth_up_c->write((const uint8_t *)r.c_str(), r.length());
}

void eth_up_fail(const char *status, const char *text) {
  if (eth_up_st == 2) {
    Update.abort();
  }
  eth_up_reply(status, text);
  eth_up_c->stop();
  eth_up_c = nullptr;
  eth_up_st = 0;
}

// Header line: the request line is checked for path and password, Content-Length is remembered.
void eth_up_header_line() {
  char *l = eth_up_line;
  if (strncmp(l, "POST /update", 12) == 0) {
    String expect = String("?p=") + "" + " ";
    eth_up_auth = strlen("") == 0 || strstr(l, expect.c_str()) != nullptr;
  } else if (strncasecmp(l, "Content-Length:", 15) == 0) {
    eth_up_len = (uint32_t)strtoul(l + 15, nullptr, 10);
  }
}

void eth_up_poll() {
  if (!eth_up_on) {
    if (millis() - eth_up_try > 2000) {
      eth_up_try = millis();
      eth_up_on = eth_up.begin(flpEth, 8080);
    }
    return;
  }
  eth_up.pool();
  if (eth_up_c == nullptr) {
    eth_up_c = eth_up.takeNewClient();
    if (eth_up_c == nullptr) {
      return;
    }
    eth_up_st = 1;
    eth_up_ll = 0;
    eth_up_len = 0;
    eth_up_got = 0;
    eth_up_auth = false;
    eth_up_t = millis();
  }
  if (eth_up_st == 3) {
    if (millis() - eth_up_t > 800) {
      ESP.restart();
    }
    return;
  }
  if (!eth_up_c->active() || millis() - eth_up_t > 10000) {
    if (eth_up_st == 2) {
      Update.abort();
    }
    eth_up_c->stop();
    eth_up_c = nullptr;
    eth_up_st = 0;
    return;
  }
  if (eth_up_st == 1) {
    for (uint16_t n = 0; n < 512 && eth_up_st == 1; n++) {
      int b = eth_up_c->read();
      if (b < 0) {
        break;
      }
      eth_up_t = millis();
      if (b == '\n') {
        eth_up_line[eth_up_ll] = 0;
        if (eth_up_ll > 0 && eth_up_line[eth_up_ll - 1] == '\r') {
          eth_up_line[eth_up_ll - 1] = 0;
        }
        bool empty = eth_up_line[0] == 0;
        if (!empty) {
          eth_up_header_line();
        }
        eth_up_ll = 0;
        if (empty) {
          if (!eth_up_auth) {
            eth_up_fail("403 Forbidden", "Неверный пароль");
          } else if (eth_up_len == 0 || !Update.begin(eth_up_len)) {
            eth_up_fail("500 Internal Server Error", "Нет места под прошивку");
          } else {
            eth_up_st = 2;
          }
        }
      } else if (eth_up_ll < sizeof(eth_up_line) - 1) {
        eth_up_line[eth_up_ll++] = (char)b;
      }
    }
  }
  if (eth_up_st == 2) {
    for (uint8_t n = 0; n < 4 && eth_up_got < eth_up_len; n++) {
      uint8_t buf[512];
      uint32_t want = eth_up_len - eth_up_got;
      int got = eth_up_c->read(buf, want < sizeof(buf) ? want : sizeof(buf));
      if (got <= 0) {
        break;
      }
      eth_up_t = millis();
      if (Update.write(buf, (size_t)got) != (size_t)got) {
        eth_up_fail("500 Internal Server Error", "Ошибка записи прошивки");
        return;
      }
      eth_up_got += (uint32_t)got;
    }
    if (eth_up_got >= eth_up_len) {
      if (Update.end(true)) {
        eth_up_reply("200 OK", "Прошивка загружена, устройство перезагружается");
        eth_up_st = 3;
        eth_up_t = millis();
      } else {
        eth_up_fail("500 Internal Server Error", "Прошивка не принята");
      }
    }
  }
}

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    button_out_in = button_out_pub || button_out_pend;
    button_out_pend = false;
    xSemaphoreGive(flp_bg_lock);

    // eth: ethernet
    {
      uint32_t now = millis();
      eth_net_due = (uint32_t)(now - eth_net_t) >= 5;
      if (eth_net_due) {
        eth_net_t = now;
      }
    }
    if (eth_net_due) {
      flpEth.pool();
    }
    eth_ready = flpEth.ethernetIsReady();
    if (eth_net_due && true) {
      eth_web.pool();
    }
    if (eth_net_due && true && true) {
      eth_up_poll();
    }
    if (eth_net_due && true) {
      if (!eth_ota_on) {
        eth_ota_on = true;
        if (""[0] != 0) {
          eth_ota.setPasswordValue("");
        }
        eth_ota.begin(flpEth);
      }
      eth_ota.pool();
    }

    // slave: modbus_tcp_slave
    flpModbus.pool();
    slave_error = flpModbus.getLastError();

    // button_reg: modbus_write
    flpModbus.saveBool(button_out_in, FLPROG_DISCRETE_INPUT, 0);

    // online_reg: modbus_write
    flpModbus.saveBool(eth_ready, FLPROG_DISCRETE_INPUT, 1);

    // error_reg: modbus_write
    flpModbus.saveInteger((uint16_t)(slave_error), FLPROG_INPUT_REGISTR, 0);

    // relay_reg: modbus_read
    relay_reg_out_b = flpModbus.readBool(FLPROG_COIL, 0);

    xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
    relay_reg_out_b_pub = relay_reg_out_b;
    relay_reg_out_b_pend = relay_reg_out_b_pend || relay_reg_out_b;
    xSemaphoreGive(flp_bg_lock);

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // eth: ethernet
  flpEth.mac(0x78, 0xAC, 0xC0, 0x2C, 0x3E, 40);
  if (false) {
    flpEth.setDhcp();
  } else {
    IPAddress addr;
    addr.fromString("192.168.1.50");
    flpEth.localIP(addr);
    addr.fromString("192.168.1.1");
    flpEth.gateway(addr);
    flpEth.dns(addr);
    addr.fromString("255.255.255.0");
    flpEth.subnet(addr);
    flpEth.resetDhcp();
  }
  eth_ota.setHostname("eth");

  // slave: modbus_tcp_slave
  flpModbus.setSlaveAddress(1);
  flpModbus.setTcpPort(502);
  flpModbus.configDataTable(FLPROG_INPUT_REGISTR, 2);
  flpModbus.configDataTable(FLPROG_COIL, 2);
  flpModbus.configDataTable(FLPROG_DISCRETE_INPUT, 2);

  // button: digital_in
  pinMode(13, INPUT_PULLUP);

  // relay: digital_out
  pinMode(26, OUTPUT);

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  relay_reg_out_b_in = relay_reg_out_b_pub || relay_reg_out_b_pend;
  relay_reg_out_b_pend = false;
  xSemaphoreGive(flp_bg_lock);

  // button: digital_in
  button_out = !digitalRead(13);

  // relay: digital_out
  digitalWrite(26, (relay_reg_out_b_in) ? HIGH : LOW);

  xSemaphoreTake(flp_bg_lock, portMAX_DELAY);
  button_out_pub = button_out;
  button_out_pend = button_out_pend || button_out;
  xSemaphoreGive(flp_bg_lock);
}
