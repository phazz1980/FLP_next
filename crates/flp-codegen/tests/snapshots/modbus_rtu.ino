// Сгенерировано FLP_next из проекта «modbus_rtu». Ручные правки будут перезаписаны.
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
SemaphoreHandle_t flp_bg_lock;

// modbus_rtu_master: общий код
// Hardware UART access shared by Modbus RTU blocks; the guard keeps one copy
// when both the slave and the master are used (`once` works per block type).
#ifndef FLP_UART_EXECUTOR
#define FLP_UART_EXECUTOR
FlprogUartExecutor flpUart;
#endif

// modbus_rtu_slave: общий код
// Hardware UART access shared by Modbus RTU blocks; the guard keeps one copy
// when both the slave and the master are used (`once` works per block type).
#ifndef FLP_UART_EXECUTOR
#define FLP_UART_EXECUTOR
FlprogUartExecutor flpUart;
#endif

// master: modbus_rtu_master
// Shared master: modbus_master_read / modbus_master_write blocks access its slaves.
ModbusMasterRTU flpMaster(2, 2, &flpUart);

// panel: modbus_rtu_slave
// Shared slave: modbus_read / modbus_write blocks with slave = "rtu" access its tables.
ModbusSlaveRTU flpModbusRtu(1, &flpUart);

// Фоновая задача: ядро 0, блоки плат с признаком «фоновая задача»
void flp_bg_task(void *) {
  for (;;) {
    // master: modbus_rtu_master
    flpMaster.pool();

    // panel: modbus_rtu_slave
    flpModbusRtu.pool();
    panel_error = flpModbusRtu.getLastError();

    // temp: modbus_master_read
    temp_out_f = flpMaster.readFloat(10, FLPROG_INPUT_REGISTR, 0);
    temp_ok = flpMaster.slaveStatus(10, false);

    // temp_out: modbus_write
    flpModbusRtu.saveFloat(temp_out_f, FLPROG_INPUT_REGISTR, 0);

    // sensor_ok: modbus_write
    flpModbusRtu.saveBool(temp_ok, FLPROG_DISCRETE_INPUT, 0);

    // relay_cmd: modbus_read
    relay_cmd_out_b = flpModbusRtu.readBool(FLPROG_COIL, 0);

    // relay: modbus_master_write
    flpMaster.saveBool(11, relay_cmd_out_b, FLPROG_COIL, 0);

    vTaskDelay(1);
  }
}

void setup() {
  flp_bg_lock = xSemaphoreCreateMutex();

  // master: modbus_rtu_master
  // UART2 from the board settings
  flpMaster.setSpeedUart(9600);
  flpMaster.setParityUart(0);
  flpMaster.setPinRxUart(16);
  flpMaster.setPinTxUart(17);
  flpMaster.setPinPeDe(4);
  flpMaster.setSlaveAddress(0, 10);
  flpMaster.setPollingPeriod(10, 500UL);
  flpMaster.setTimeOutTime(10, 1000UL);
  flpMaster.configDataTable(10, FLPROG_INPUT_REGISTR, 2);
  flpMaster.configDataTable(10, FLPROG_COIL, 1);
  flpMaster.setSlaveAddress(1, 11);
  flpMaster.setPollingPeriod(11, 500UL);
  flpMaster.setTimeOutTime(11, 1000UL);
  flpMaster.configDataTable(11, FLPROG_INPUT_REGISTR, 2);
  flpMaster.configDataTable(11, FLPROG_COIL, 1);
  flpMaster.begin();

  // panel: modbus_rtu_slave
  // UART1 from the board settings
  flpModbusRtu.setSpeedUart(19200);
  flpModbusRtu.setParityUart(0);
  flpModbusRtu.setPinRxUart(25);
  flpModbusRtu.setPinTxUart(27);
  flpModbusRtu.setPinPeDe(14);
  flpModbusRtu.setSlaveAddress(1);
  flpModbusRtu.configDataTable(FLPROG_INPUT_REGISTR, 2);
  flpModbusRtu.configDataTable(FLPROG_COIL, 1);
  flpModbusRtu.configDataTable(FLPROG_DISCRETE_INPUT, 1);
  flpModbusRtu.begin();

  // Фоновая задача стартует после настройки всех блоков
  xTaskCreatePinnedToCore(flp_bg_task, "flp_bg", 10240, nullptr, 1, nullptr, 0);
}

void loop() {
}
