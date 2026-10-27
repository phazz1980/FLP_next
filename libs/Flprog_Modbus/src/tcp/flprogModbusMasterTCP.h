#pragma once
#include "flprogEthernet.h"
#include "../base/flprogModbus.h"
#include "../base/flprogModbusTCPSlaveServer.h"

class ModbusMasterTCP : public Modbus
{
public:
  ModbusMasterTCP(FLProgAbstractTcpInterface *sourse, uint8_t size);

  void setServerSlavesSize(uint8_t serverIndex, uint8_t size);
  void setServerPort(uint8_t serverIndex, int16_t port);
  int16_t serverPort(uint8_t serverIndex);

  void setServerIpAdress(uint8_t serverIndex, IPAddress ip);
  IPAddress serverIpAdress(uint8_t serverIndex);

  void setServerIpAdressAsNumber(uint8_t serverIndex, uint32_t ip) { setServerIpAdress(serverIndex, flprog::numberToIp(ip)); };
  void setServerHost(uint8_t serverIndex, String host);
  void setSlaveAddress(uint8_t serverIndex, uint8_t slaveIndex, uint16_t addr);
  uint8_t getSlaveAddress(uint8_t serverIndex, uint8_t slaveIndex);

  void setServerMode(uint8_t serverIndex, uint8_t mode);
  void setServerAsTcp(uint8_t serverIndex) { setServerMode(serverIndex, FLPROG_TCP_MODBUS); };
  void setServerAsRtuOverTcp(uint8_t serverIndex) { setServerMode(serverIndex, FLPROG_RTU_OVER_TCP_MODBUS); };
  uint8_t serverMode(uint8_t serverIndex);

  void setDataTable(uint8_t serverIndex, uint8_t slaveAdr, ModbusTable *table);
  void setDataTable(uint8_t serverIndex, uint8_t slaveAdr, uint8_t _table, int32_t dataSize, int32_t *_addresses);

  void configDataTable(uint8_t serverIndex, uint8_t slaveAdr, uint8_t _table, int32_t dataSize);
  void configDataTable(uint8_t serverIndex, uint8_t slaveAdr, uint8_t _table, int32_t dataSize, uint32_t _startAddress);

  void setPollingPeriod(uint8_t serverIndex, uint8_t slaveAdr, uint32_t period, bool isIndex = false);
  uint32_t pollingPeriod(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex);

  void setTimeOutTime(uint8_t serverIndex, uint8_t slaveAdr, uint32_t time, bool isIndex = false);
  uint32_t getTimeOutTime(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);

  void setLongOrder(uint8_t serverIndex, uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setFloatOrder(uint8_t serverIndex, uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setUnsignedlongOrder(uint8_t serverIndex, uint8_t slaveAdr, uint8_t order, bool isIndex = false);
  void setIntOrder(uint8_t serverIndex, uint8_t slaveAdr, uint8_t order, bool isIndex = false);

  void setCallBack(uint8_t serverIndex, uint8_t slaveAdr, FLProgModbusNewDataCallback func, bool isIndex = false);

  uint8_t intOrder(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);
  uint8_t longOrder(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);
  uint8_t floatOrder(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);

  uint8_t unsignedlongOrder(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);

  void saveLong(uint8_t serverIndex, uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveUnsignedLong(uint8_t serverIndex, uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveFloat(uint8_t serverIndex, uint8_t slaveAdr, float value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveInteger(uint8_t serverIndex, uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveByte(uint8_t serverIndex, uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex = false);
  void saveBool(uint8_t serverIndex, uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddress, bool isIndex = false);

  void saveLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  void saveUnsignedLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  void saveFloatByIndex(uint8_t serverIndex, uint8_t slaveAdr, float value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  void saveIntegerByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  void saveByteByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  void saveBoolByIndex(uint8_t serverIndex, uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddressIndex, bool isIndex = false);

  int32_t getAdress(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t index);
  void setAdress(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t index, int32_t address);

  uint8_t readByte(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint16_t readInteger(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  float readFloat(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  int32_t readLong(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  uint32_t readUnsignedLong(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);
  bool readBool(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddress, bool isIndex = false);

  uint8_t readByteByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  uint16_t readIntegerByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  float readFloatByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  int32_t readLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  uint32_t readUnsignedLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);
  bool readBoolByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex = false);

  uint8_t hrSendMode(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex = false);
  void setHrSendMode(uint8_t serverIndex, uint8_t slaveAddres, uint8_t mode, bool isIndex = false);
  uint8_t coilSendMode(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex = false);
  void setCoilSendMode(uint8_t serverIndex, uint8_t slaveAddres, uint8_t mode, bool isIndex = false);

  uint8_t getLastError(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex = false);
  void status(uint8_t serverIndex, uint8_t slaveAdr, bool status, bool isIndex = false);
  bool slaveStatus(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex);
  void setSlavesToServer(uint8_t serverIndex, ModbusSlaveInMaster *table, int16_t size);

  ModbusTCPSlaveServer *servers();
  virtual void pool();

protected:
  virtual void checkAnswer();
  void nextQuery();
  bool createNewTelegramm();
  bool createReadTelegram();
  void sendQuery();
  bool hasServer(uint8_t serverIndex);
  bool createWriteTelegramm(ModbusTCPSlaveServer *writeServer);
  virtual uint8_t validateRequest();
  ModbusTCPSlaveServer *firstWriteServer();
  virtual void sendTxBuffer();
  ModbusTCPSlaveServer *nextReadyServer(ModbusTCPSlaveServer *oldServer);
  bool hasReadyServer();
  bool nextStep();
  bool nextServer();
  bool nextSlave();
  bool nextTable();
  bool nextRegistor();
  uint32_t errorPauseTime();
  uint8_t calculateSendRegSize();
  uint8_t selectSendFunction();

  ModbusTCPSlaveServer *firstReadyServer();
  virtual void getRxBuffer();
  virtual void begin();
  ModbusTCPSlaveServer *server(uint8_t serverIndex);
  virtual void connect(ModbusTCPSlaveServer *server);

  ModbusTCPSlaveServer *_telegrammServer;
  ModbusSlaveInMaster *_telegrammSlave;
  ModbusTable *_telegrammTable;
  uint8_t _telegrammFunction;
  int32_t _telegrammStartAddres;
  int32_t _telegrammNumbeRegs;
  int16_t _telegrammAnswerId = 1;
  ModbusTCPSlaveServer *_currentServer;
  ModbusTCPSlaveServer *_tempCurrentServer;
  ModbusSlaveInMaster *_currentSlave;
  ModbusTable *_currentSlaveTable;
  int32_t _currentSlaveStartAddress = -1;
  int32_t _currentSlaveLastAddress = -1;
  int16_t _serversSize = 1;
  ModbusTCPSlaveServer *_servs = 0;
  uint8_t _mbapBuffer[6];

  // TCP stream state — исправил ander.
  uint8_t _tcpRxMbapSize = 0;
  uint16_t _tcpRxExpectedSize = 0;
  uint16_t _tcpRxBodySize = 0;
  uint16_t _rtuOverTcpRxSize = 0;

  FLProgAbstractTcpInterface *_interface = 0;
};
