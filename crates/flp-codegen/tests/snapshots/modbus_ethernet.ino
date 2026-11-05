// Сгенерировано CrabLogic из проекта «modbus_ethernet». Ручные правки будут перезаписаны.
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
// Network services (MQTT, web) are polled at most once per period.
uint32_t eth_net_t = 0;
bool eth_net_due = true;

// slave: modbus_tcp_slave
// Shared slave: modbus_read / modbus_write blocks access its tables.
ModbusSlaveTCP flpModbus(&flpEth);

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
