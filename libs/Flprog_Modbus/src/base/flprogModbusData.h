#pragma once
#include "flprogModbusUtilites.h"

class ModbusTable
{
public:
  ModbusTable(uint8_t _table, int32_t dataSize, int32_t startAddress = 0);
  ModbusTable(uint8_t _table, int32_t dataSize, int32_t *_addresses);

  void init(int32_t dataSize, int32_t startAddress);

  void init(int32_t dataSize, int32_t *_addresses);

  void init(int32_t dataSize);

  int32_t indexForAddres(int32_t addr);
  int32_t adressForIndex(int32_t index);
  void adressForIndex(int32_t index, int32_t addr);

  int32_t firstWriteAddress();
  int16_t writeRegsSize(int32_t startAddress);
  int16_t readRegsSize(int32_t startAddress);
  int32_t findNextStartAddres(int32_t address);
  int32_t getMinAdress();
  int32_t getMaxAdress();
  void resetWriteFlag(int32_t address);
  bool hasAdress(int32_t address);

  void setData(int32_t address, bool value);
  void setData(int32_t address, int16_t value);

  void setDataByIndex(int32_t addressIndex, bool value);
  void setDataByIndex(int32_t addressIndex, int16_t value);

  void writeRegister(int32_t address, bool value);
  void writeRegister(int32_t address, int16_t value);
  int16_t readWorldRegister(int32_t address);
  bool readBoolRegister(int32_t address);

  int16_t readWorldRegisterByIndex(int32_t addressIndex);
  bool readBoolRegisterByIndex(int32_t addressIndex);

  uint8_t tableType() { return _table; };

  void setCallBack(FLProgModbusNewDataCallback func) { _newDataCallback = func; };

private:
  int16_t recursiveWriteRegsSize(int32_t startAddress, int16_t result);
  int16_t recursiveReadRegsSize(int32_t startAddress, int16_t result);

  int32_t _minAdress = -1;
  int32_t _maxAdress = -1;
  uint8_t _table = 0;
  int32_t _tableSize = 0;
  int32_t *_adresses = 0;
  int16_t *_worldData;
  bool *_boolData;
  bool *_sendRegisters;

  FLProgModbusNewDataCallback _newDataCallback = 0;
};

class ModbusMainData
{
public:
  ModbusMainData() {};
  void setDataTable(ModbusTable *_table);
  void setDataTable(uint8_t _table, int32_t dataSize, int32_t *_adresses);
  void configDataTable(uint8_t _table, int32_t dataSize, int32_t startAddress = 0);

  void setIntOrder(uint8_t order) { _intOrder = order; };
  void setLongOrder(uint8_t order) { _longOrder = order; };
  void setFloatOrder(uint8_t order) { _floatOrder = order; };
  void setUnsignedlongOrder(uint8_t order) { _unsignedlongOrder = order; };

  uint8_t intOrder() { return _intOrder; };
  uint8_t longOrder() { return _longOrder; };
  uint8_t floatOrder() { return _floatOrder; };
  uint8_t unsignedlongOrder() { return _unsignedlongOrder; };

  int32_t indexForAddres(int32_t addr, uint8_t table);
  bool checkModbusAddres(int32_t addr, uint8_t table);
  bool checkModbusRange(int32_t startAddr, int32_t addrNumber, uint8_t table);

  void saveLong(int32_t value, uint8_t table, int32_t startAddress);
  void saveUnsignedLong(uint32_t value, uint8_t table, int32_t startAddress);
  void saveFloat(float value, uint8_t table, int32_t startAddress);
  void saveInteger(int16_t value, uint8_t table, int32_t startAddress);
  void saveByte(uint8_t value, uint8_t table, int32_t startAddress);
  void saveBool(bool value, uint8_t table, int32_t startAddress);

  void saveLongByIndex(int32_t value, uint8_t table, int32_t startAddress);
  void saveUnsignedLongByIndex(uint32_t value, uint8_t table, int32_t startAddress);
  void saveFloatByIndex(float value, uint8_t table, int32_t startAddress);
  void saveIntegerByIndex(int16_t value, uint8_t table, int32_t startAddress);
  void saveByteByIndex(uint8_t value, uint8_t table, int32_t startAddress);
  void saveBoolByIndex(bool value, uint8_t table, int32_t startAddress);

  uint8_t readByte(uint8_t table, int32_t startAddress);
  int16_t readInteger(uint8_t table, int32_t startAddress);
  float readFloat(uint8_t table, int32_t startAddress);
  int32_t readLong(uint8_t table, int32_t startAddress);
  uint32_t readUnsignedLong(uint8_t table, int32_t startAddress);
  bool readBool(uint8_t table, int32_t startAddress);

  uint8_t readByteByIndex(uint8_t table, int32_t startAddressIndex);
  int16_t readIntegerByIndex(uint8_t table, int32_t startAddressIndex);
  float readFloatByIndex(uint8_t table, int32_t startAddressIndex);
  int32_t readLongByIndex(uint8_t table, int32_t startAddressIndex);
  uint32_t readUnsignedLongByIndex(uint8_t table, int32_t startAddressIndex);
  bool readBoolByIndex(uint8_t table, int32_t startAddressIndex);

  bool isSupportFunction(uint8_t function);
  ModbusTable *tableForType(uint8_t table);
  bool hasTable(uint8_t table);
  void setCallBack(FLProgModbusNewDataCallback func);

  int32_t getAdress(uint8_t table, int32_t index);
  void setAdress(uint8_t table, int32_t index, int32_t addr);

  void setWorkPause(uint32_t time);
  void setTimeOutTime(uint32_t time) { _timeOutTime = time; };
  uint32_t getTimeOutTime() { return _timeOutTime; };

protected:
  void saveForByteWithOrder(unsigned char *sourse, uint8_t table, int32_t startAddress, uint8_t order);
  void saveForByteWithOrderByIndex(unsigned char *sourse, uint8_t table, int32_t startAddress, uint8_t order);

  void readForByteWithOrder(unsigned char *sourse, uint8_t table, int32_t startAddress, uint8_t order);
  void readForByteWithOrderByIndex(unsigned char *sourse, uint8_t table, int32_t startAddress, uint8_t order);

  ModbusTable *tableForStartArddres(uint8_t table, int32_t startAddress, bool isTwoWord);

  virtual bool canSaveTable(uint8_t table);

  ModbusTable *_tableCoil = 0;
  ModbusTable *_tableDiscreteInput = 0;
  ModbusTable *_tableInputRegistr = 0;
  ModbusTable *_tableHoldingRegistr = 0;
  uint8_t _intOrder = 1;
  uint8_t _longOrder = 1;
  uint8_t _floatOrder = 1;
  uint8_t _unsignedlongOrder = 1;
  uint32_t _workPause = 0;
  uint32_t _startWorkPauseTime = 0;
  bool _isWorkPause = false;
  uint32_t _timeOutTime = 1000;
};

class ModbusSlaveInMaster : public ModbusMainData
{
public:
  ModbusSlaveInMaster() {};
  ModbusSlaveInMaster(uint8_t addr) { setSlaveAddress(addr); };
  uint8_t getLastError();
  void lastReqest(uint32_t time) { _lastReqestTime = time; };
  uint32_t lastReqest() { return _lastReqestTime; };
  bool isInit() { return _initFlag; };
  void setInit() { _initFlag = true; };
  void resetInit() { _initFlag = false; };

  void setPollingPeriod(uint32_t period);
  uint32_t pollingPeriod() { return _pollingPeriod; };
  bool hasWriteRegisters();
  void setLastError(uint8_t error);
  ModbusTable *firstWriteTable();
  ModbusTable *firstTabe();
  ModbusTable *lastTable();
  int16_t nextTable(int16_t table);
  ModbusTable *nextTable(ModbusTable *table);
  bool isReady();
  void setSlaveAddress(uint8_t adr);
  uint8_t slaveAddress() { return _slaveAddres; };
  uint8_t getSlaveAddress() { return _slaveAddres; };

  uint8_t hrSendMode() { return _hrSendMode; };
  void hrSendMode(uint8_t mode) { _hrSendMode = mode; };
  uint8_t coilSendMode() { return _coilSendMode; };
  void coilSendMode(uint8_t mode) { _coilSendMode = mode; };

  bool status() { return _isActive; };
  void status(bool status) { _isActive = status; };
  bool slaveStatus() { return _isActive; };

protected:
  uint8_t _hrSendMode = FLPROG_SEND_AUTO;
  uint8_t _coilSendMode = FLPROG_SEND_AUTO;
  bool _isActive = true;
  uint32_t _lastReqestTime;
  bool _initFlag = false;
  uint8_t _slaveAddres = 1;
  uint8_t _lastError;
  uint32_t _pollingPeriod = 1000;
  virtual bool canSaveTable(uint8_t table);
};