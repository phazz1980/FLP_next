// Сгенерировано CrabLogic из проекта «modbus_rtu». Ручные правки будут перезаписаны.
// Порядок выполнения: master → panel → temp → temp_out → sensor_ok → relay_cmd → relay

#include <Arduino.h>
#include <flprogModbusRTU.h>
#include <flprogUart.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

// Выходы блоков
int32_t panel_error = 0;
float temp_out_f = 0.0f;
bool temp_ok = false;
bool relay_cmd_out_b = false;

// Обмен сигналами между основной и фоновой задачами
SemaphoreHandle_t crab_bg_lock;

// modbus_rtu_master: общий код
// Hardware UART access shared by Modbus RTU blocks; the guard keeps one copy even
// if the slave and the master ever stop sharing the same text of this section.
#ifndef CRAB_UART_EXECUTOR
#define CRAB_UART_EXECUTOR
FlprogUartExecutor crabUart;
#endif

// master: modbus_rtu_master
// Shared master: modbus_master_read / modbus_master_write blocks access its slaves.
ModbusMasterRTU crabMaster(2, 2, &crabUart);

// panel: modbus_rtu_slave
// Shared slave: modbus_read / modbus_write blocks with slave = "rtu" access its tables.
ModbusSlaveRTU crabModbusRtu(1, &crabUart);

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void crab_bg_task(void *) {
  for (;;) {
    // master: modbus_rtu_master
    crabMaster.pool();

    // panel: modbus_rtu_slave
    crabModbusRtu.pool();
    panel_error = crabModbusRtu.getLastError();

    // temp: modbus_master_read
    temp_out_f = crabMaster.readFloat(10, FLPROG_INPUT_REGISTR, 0);
    temp_ok = crabMaster.slaveStatus(10, false);

    // temp_out: modbus_write
    crabModbusRtu.saveFloat(temp_out_f, FLPROG_INPUT_REGISTR, 0);

    // sensor_ok: modbus_write
    crabModbusRtu.saveBool(temp_ok, FLPROG_DISCRETE_INPUT, 0);

    // relay_cmd: modbus_read
    relay_cmd_out_b = crabModbusRtu.readBool(FLPROG_COIL, 0);

    // relay: modbus_master_write
    crabMaster.saveBool(11, relay_cmd_out_b, FLPROG_COIL, 0);

    vTaskDelay(1);
  }
}

void setup() {
  crab_bg_lock = xSemaphoreCreateMutex();

  // master: modbus_rtu_master
  // UART2 from the board settings
  crabMaster.setSpeedUart(9600);
  crabMaster.setParityUart(0);
  crabMaster.setPinRxUart(16);
  crabMaster.setPinTxUart(17);
  crabMaster.setPinPeDe(4);
  crabMaster.setSlaveAddress(0, 10);
  crabMaster.setPollingPeriod(10, 500UL);
  crabMaster.setTimeOutTime(10, 1000UL);
  crabMaster.configDataTable(10, FLPROG_INPUT_REGISTR, 2);
  crabMaster.configDataTable(10, FLPROG_COIL, 1);
  crabMaster.setSlaveAddress(1, 11);
  crabMaster.setPollingPeriod(11, 500UL);
  crabMaster.setTimeOutTime(11, 1000UL);
  crabMaster.configDataTable(11, FLPROG_INPUT_REGISTR, 2);
  crabMaster.configDataTable(11, FLPROG_COIL, 1);
  crabMaster.begin();

  // panel: modbus_rtu_slave
  // UART1 from the board settings
  crabModbusRtu.setSpeedUart(19200);
  crabModbusRtu.setParityUart(0);
  crabModbusRtu.setPinRxUart(25);
  crabModbusRtu.setPinTxUart(27);
  crabModbusRtu.setPinPeDe(14);
  crabModbusRtu.setSlaveAddress(1);
  crabModbusRtu.configDataTable(FLPROG_INPUT_REGISTR, 2);
  crabModbusRtu.configDataTable(FLPROG_COIL, 1);
  crabModbusRtu.configDataTable(FLPROG_DISCRETE_INPUT, 1);
  crabModbusRtu.begin();

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(crab_bg_task, "crab_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
}
