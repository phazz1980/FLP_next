#pragma once
#include "flprogEthernet.h"
#include "flprogModbusData.h"

class ModbusTCPSlaveServer
{
public:
  ModbusTCPSlaveServer() {};
  void setSlaves(ModbusSlaveInMaster *_table, uint8_t size);
  void setSlavesSize(uint8_t size);
  uint8_t getSlavesSize() { return slavesSize; }

  void setPort(int16_t serverPort) { _port = serverPort; };
  int16_t port() { return _port; };

  void setIpAdress(IPAddress ip);
  IPAddress ipAdress() { return _serverIp; };

  void setHost(String host);
  void setSlaveAddress(uint8_t slaveIndex, uint16_t addr);
  uint8_t getSlaveAddress(uint8_t slaveIndex);

  void setMode(uint8_t mode);
  uint8_t mode() { return _mode; };

  IPAddress getIp() { return _serverIp; };
  char *getHost() { return _serverHost; };
  bool serverAsHost() { return _serverAsHost; };

  bool hasSlave(uint8_t slaveIndex);

  ModbusSlaveInMaster *slaveOnIndex(uint8_t slaveIndex);
  ModbusSlaveInMaster *slave(uint8_t slaveAddr, bool isIndex = false);

  void setDataTable(uint8_t slaveAdr, ModbusTable *table);
  void setDataTable(uint8_t slaveAdr, uint8_t _table, int32_t dataSize, int32_t *_addresses);

  void configDataTable(uint8_t slaveAdr, uint8_t _table, int32_t dataSize);
  void configDataTable(uint8_t slaveAdr, uint8_t _table, int32_t dataSize, uint32_t _startAddress);

  void setPollingPeriod(uint8_t slaveAdr, uint32_t period, bool isIndex = false);
  uint32_t pollingPeriod(uint8_t slaveAddres, bool isIndex);

  void setTimeOutTime(uint8_t slaveAdr, uint32_t time, bool isIndex = false);
  uint32_t getTimeOutTime(uint8_t slaveAdr, bool isIndex = false);

  void setLongOrder(uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setFloatOrder(uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setUnsignedlongOrder(uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setIntOrder(uint8_t slaveAdr, uint8_t order, bool isIndex = false);

  void setCallBack(uint8_t slaveAdr, FLProgModbusNewDataCallback func, bool isIndex = false);

  uint8_t longOrder(uint8_t slaveAdr, bool isIndex = false);
  uint8_t floatOrder(uint8_t slaveAdr, bool isIndex = false);
  uint8_t unsignedlongOrder(uint8_t slaveAdr, bool isIndex = false);
  uint8_t intOrder(uint8_t slaveAdr, bool isIndex = false);

  void saveLong(uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveUnsignedLong(uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveFloat(uint8_t slaveAdr, float value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveInteger(uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveByte(uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveBool(uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddress, bool isIndex = false);

  void saveLongByIndex(uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveUnsignedLongByIndex(uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveFloatByIndex(uint8_t slaveAdr, float value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveIntegerByIndex(uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveByteByIndex(uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveBoolByIndex(uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddress, bool isIndex = false);

  uint8_t readByte(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint16_t readInteger(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  float readFloat(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  int32_t readLong(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint32_t readUnsignedLong(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  bool readBool(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);

  uint8_t readByteByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint16_t readIntegerByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  float readFloatByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  int32_t readLongByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint32_t readUnsignedLongByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  bool readBoolByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);

  uint8_t hrSendMode(uint8_t slaveAddres, bool isIndex = false);
  void setHrSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex = false);
  uint8_t coilSendMode(uint8_t slaveAddres, bool isIndex = false);
  void setCoilSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex = false);

  int16_t getAdress(uint8_t slaveAddres, uint8_t table, int16_t index);
  void setAdress(uint8_t slaveAddres, uint8_t table, int16_t index, int16_t addr);

  uint8_t getLastError(uint8_t slaveAdr, bool isIndex = false);
  void status(uint8_t slaveAdr, bool status, bool isIndex = false);
  bool slaveStatus(uint8_t slaveAddres, bool isIndex);

  ModbusSlaveInMaster *firstWriteSlave();
  ModbusSlaveInMaster *firstReadySlave();
  ModbusSlaveInMaster *nextSlave(ModbusSlaveInMaster *currentSlave);

  bool hasWriteRegisters();
  bool isReady();
  int16_t getPort() { return _port; };

  FLProgEthernetClient *client() { return &_tcpClient; };
  bool connected() { return _tcpClient.connected(); };
  uint8_t connect();
  void stop() { _tcpClient.stop(); };

  void setActive(bool status) { _isActive = status; };
  bool getIsActive();
  void setWorkPause(uint32_t time);
  uint32_t errorPauseTime();

protected:
  int16_t _port = 502;
  IPAddress _serverIp;
  char _serverHost[FLPROG_HOST_NAME_LENGTH] = "";
  bool _serverAsHost = false;
  uint8_t slavesSize = 5;
  ModbusSlaveInMaster *slaves;
  FLProgEthernetClient _tcpClient;
  uint8_t _mode = FLPROG_TCP_MODBUS;
  bool _isActive = true;
  uint32_t _workPause = 0;
  uint32_t _startWorkPauseTime = 0;
  bool _isWorkPause = false;
};