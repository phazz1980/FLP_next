#pragma once
#include "flprogModbusRTU.h"

class ModbusSlaveRTU : public ModbusRTU
{
public:
  ModbusSlaveRTU() {};
  ModbusSlaveRTU(uint8_t number, FlprogAbstractUartExecutor *executor);
  void begin();
  void pool();
  void setSlaveAddress(uint8_t address) { _slaveAddres = address; };
  uint8_t slaveAddress() { return _slaveAddres; };
  void setData(ModbusMainData *data) { _data = data; };

  void setDataTable(ModbusTable *table) { mainData()->setDataTable(table); };
  void setDataTable(uint8_t table, uint32_t dataSize, int32_t *addresses) { mainData()->setDataTable(table, dataSize, addresses); };
  void configDataTable(uint8_t table, uint32_t dataSize, int32_t startAddress = 0) { mainData()->configDataTable(table, dataSize, startAddress); };

  void saveLong(int32_t val, uint8_t table, int32_t address) { mainData()->saveLong(val, table, address); };
  void saveUnsignedLong(uint32_t val, uint8_t table, int32_t address) { mainData()->saveUnsignedLong(val, table, address); };
  void saveFloat(float val, uint8_t table, int32_t address) { mainData()->saveFloat(val, table, address); };
  void saveInteger(int16_t val, uint8_t table, int32_t address) { mainData()->saveInteger(val, table, address); };
  void saveByte(uint8_t val, uint8_t table, int32_t address) { mainData()->saveByte(val, table, address); };
  void saveBool(bool val, uint8_t table, int32_t address) { mainData()->saveBool(val, table, address); };

  void saveLongByIndex(int32_t val, uint8_t table, int32_t address) { mainData()->saveLongByIndex(val, table, address); };
  void saveUnsignedLongByIndex(uint32_t val, uint8_t table, int32_t address) { mainData()->saveUnsignedLongByIndex(val, table, address); };
  void saveFloatByIndex(float val, uint8_t table, int32_t address) { mainData()->saveFloatByIndex(val, table, address); };
  void saveIntegerByIndex(int16_t val, uint8_t table, int32_t address) { mainData()->saveIntegerByIndex(val, table, address); };
  void saveByteByIndex(uint8_t val, uint8_t table, int32_t address) { mainData()->saveByteByIndex(val, table, address); };
  void saveBoolByIndex(bool val, uint8_t table, int32_t address) { mainData()->saveBoolByIndex(val, table, address); };

  uint8_t readByte(uint8_t table, int32_t address) { return mainData()->readByte(table, address); };
  uint16_t readInteger(uint8_t table, int32_t address) { return mainData()->readInteger(table, address); };
  float readFloat(uint8_t table, int32_t address) { return mainData()->readFloat(table, address); };
  int32_t readLong(uint8_t table, int32_t address) { return mainData()->readLong(table, address); };
  uint32_t readUnsignedLong(uint8_t table, int32_t address) { return mainData()->readUnsignedLong(table, address); };
  bool readBool(uint8_t table, int32_t address) { return mainData()->readBool(table, address); };

  uint8_t readByteByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readByteByIndex(table, addressIndex); };
  uint16_t readIntegerByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readIntegerByIndex(table, addressIndex); };
  float readFloatByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readFloatByIndex(table, addressIndex); };
  int32_t readLongByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readLongByIndex(table, addressIndex); };
  uint32_t readUnsignedLongByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readUnsignedLongByIndex(table, addressIndex); };
  bool readBoolByIndex(uint8_t table, int32_t addressIndex) { return mainData()->readBoolByIndex(table, addressIndex); };

  void setLongOrder(uint8_t order) { mainData()->setLongOrder(order); };
  void setFloatOrder(uint8_t order) { mainData()->setFloatOrder(order); };
  void setUnsignedlongOrder(uint8_t order) { mainData()->setUnsignedlongOrder(order); };
  void setIntOrder(uint8_t order) { mainData()->setIntOrder(order); };

  uint8_t intOrder() { return mainData()->intOrder(); };
  uint8_t longOrder() { return mainData()->longOrder(); };
  uint8_t floatOrder() { return mainData()->floatOrder(); };
  uint8_t unsignedlongOrder() { return mainData()->unsignedlongOrder(); };

  uint32_t getTimeOutTime() { return mainData()->getTimeOutTime(); };
  void setTimeOutTime(uint32_t order) { mainData()->setTimeOutTime(order); };

  int32_t getAdress(uint8_t table, int32_t index) { return mainData()->getAdress(table, index); };
  void setAdress(uint8_t table, int32_t index, int32_t addrress) { mainData()->setAdress(table, index, addrress); };

  ModbusMainData *mainData();
  void setCallBack(FLProgModbusNewDataCallback func);

  uint32_t getPauseTime() { return _pauseTime; };
  void setPauseTime(uint32_t time) { _pauseTime = time; };

protected:
  uint8_t _slaveAddres = 1;
  virtual uint8_t validateRequest();
  virtual uint16_t rtuPacadgeSize(uint16_t length, uint8_t bufferArray[]);

private:
  ModbusMainData *_data;
  uint32_t _startPauseTime;
  uint32_t _pauseTime = 10;
};
