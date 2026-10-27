#pragma once
#include "flprogModbusData.h"

class Modbus : public AbstractFLProgClass
{
public:
  void setLastError(uint8_t error) { _lastError = error; };
  virtual void begin() = 0;
  uint8_t getLastError() { return _lastError; };
  uint16_t getSkippingEvents() { return _skippingEvents; };
  void setSkippingEvents(uint16_t value) { _skippingEvents = value; };
  void isPause(bool value) { _isPause = value; };
  bool isPause() { return _isPause; };
  void setEnable(bool enable) { _enable = enable; };
  bool getEnable() { return _enable; };


protected:
  virtual void sendTxBuffer() = 0;
  void process_modbus_FC1(ModbusMainData *data, uint8_t table);
  void process_modbus_FC3(ModbusMainData *data, uint8_t table);
  void process_modbus_FC5(ModbusMainData *data);
  void process_modbus_FC6(ModbusMainData *data);
  void process_modbus_FC15(ModbusMainData *data);
  void process_modbus_FC16(ModbusMainData *data);
  void get_FC1(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  void get_FC3(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  void create_Read(int16_t numberRegs);
  void create_Write_FC5(ModbusTable *table, int32_t startAddress);
  void create_Write_FC6(ModbusTable *table, int32_t startAddress);
  void create_Write_FC15(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  void create_Write_FC16(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  void create_PDU(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  void writeMaserData(ModbusTable *table, int32_t startAddress, int16_t numberRegs);
  uint8_t validateSlaveReqest(ModbusMainData *data);
  void buildException(uint8_t exception);
  virtual uint8_t validateRequest() { return 0; };
  void executeSlaveReqest(ModbusMainData *data, uint8_t address);
  uint8_t tabeTypeForFunction(uint8_t function);

  uint8_t _lastError = 0;
  uint8_t _bufferSize = 0;
  uint8_t _buffer[FLPROG_MODBUS_BUFER_SIZE];
  uint32_t _startSendTime;
  uint32_t _timeOfSend;
  bool _isInit = false;
  bool _isPause = false;
  uint16_t _skippingEvents = 0;
  uint16_t _eventsCount = 0;
  bool _enable = true;
};
