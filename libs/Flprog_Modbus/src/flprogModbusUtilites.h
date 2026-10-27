#include "Arduino.h"
#include "flprogUtilites.h"
#include "flprogUartBase.h"
#pragma once
#define FLPROG_MODBUS_READY 0
#define FLPROG_MODBUS_WAITING_SENDING 1
#define FLPROG_MODBUS_WAITING_ANSWER 2
#define FLPROG_MODBUS_WAITING_CONNECT_CLIENT 3
#define FLPROG_MODBUS_WAITING_PAUSE_BEFORE_ANSWERING 4

#define FLPROG_TCP_MODBUS 0
#define FLPROG_RTU_OVER_TCP_MODBUS 1
#define FLPROG_KASCADA_CLOUD_MODBUS 2

#define FLPROG_COIL 0
#define FLPROG_DISCRETE_INPUT 1
#define FLPROG_INPUT_REGISTR 3
#define FLPROG_HOLDING_REGISTR 4
#define FLPROG_AB_ORDER 1
#define FLPROG_BA_ORDER 2
#define FLPROG_ABCD_ORDER 1
#define FLPROG_CDAB_ORDER 2
#define FLPROG_BADC_ORDER 3
#define FLPROG_DCBA_ORDER 4

#define FLPROG_SEND_AUTO 0
#define FLPROG_SEND_HR_F6 6
#define FLPROG_SEND_HR_F16 16
#define FLPROG_SEND_COIL_F5 5
#define FLPROG_SEND_COIL_F15 15

typedef void (*FLProgModbusNewDataCallback)(uint8_t table, int32_t adress, int16_t value);

namespace flprogModus
{
  uint16_t modbusCalcCRC(uint16_t length, uint8_t bufferArray[], uint8_t offset = 0);
  uint8_t t35TimeForSpeed(uint16_t portSpeed);
  uint16_t timeForSendBytes(uint8_t portDataBits, uint8_t portStopBits, uint8_t portParity, uint16_t portSpeed, uint16_t dataSize);
  bool checkCRCOnBuffer(uint16_t length, uint8_t bufferArray[]);
  uint16_t slaveRTUPacadgeSize(uint16_t length, uint8_t bufferArray[]);
  uint16_t masterRTUPacadgeSize(uint16_t length, uint8_t bufferArray[]);
};
