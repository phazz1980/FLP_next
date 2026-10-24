// Сгенерировано FLP_next из проекта «modbus_ethernet». Ручные правки будут перезаписаны.
// Порядок выполнения: eth → slave → button → button_reg → online_reg → error_reg → relay_reg → relay

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogModbusTCP.h>

// Выходы блоков
bool eth_ready = false;
int32_t slave_error = 0;
bool button_out = false;
bool relay_reg_out_b = false;

// eth: ethernet
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgWiznetInterface flpEth(5);

// slave: modbus_tcp_slave
// Shared slave: modbus_read / modbus_write blocks access its tables.
ModbusSlaveTCP flpModbus(&flpEth);

void setup() {
  // eth: ethernet
  flpEth.mac(0x78, 0xAC, 0xC0, 0x2C, 0x3E, 40);
  {
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
}

void loop() {
  // eth: ethernet
  flpEth.pool();
  eth_ready = flpEth.ethernetIsReady();

  // slave: modbus_tcp_slave
  flpModbus.pool();
  slave_error = flpModbus.getLastError();

  // button: digital_in
  button_out = !digitalRead(13);

  // button_reg: modbus_write
  flpModbus.saveBool(button_out, FLPROG_DISCRETE_INPUT, 0);

  // online_reg: modbus_write
  flpModbus.saveBool(eth_ready, FLPROG_DISCRETE_INPUT, 1);

  // error_reg: modbus_write
  flpModbus.saveInteger((uint16_t)(slave_error), FLPROG_INPUT_REGISTR, 0);

  // relay_reg: modbus_read
  relay_reg_out_b = flpModbus.readBool(FLPROG_COIL, 0);

  // relay: digital_out
  digitalWrite(26, (relay_reg_out_b) ? HIGH : LOW);
}
