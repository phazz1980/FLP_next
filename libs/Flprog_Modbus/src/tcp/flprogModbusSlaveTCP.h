#pragma once
#include "flprogEthernet.h"
#include "../base/flprogModbus.h"

class ModbusSlaveTCP : public Modbus
{
public:
  ModbusSlaveTCP(FLProgAbstractTcpInterface *sourse);

  virtual void pool();

  void setSlaveAddress(uint8_t adr) { _slaveAddres = adr; };
  uint8_t slaveAddress() { return _slaveAddres; };

  void setTcpPort(int16_t _port);
  uint16_t tcpPort() { return _port; };
  void setKaScadaCloudIp(uint8_t newFirst_octet, uint8_t newSecond_octet, uint8_t newThird_octet, uint8_t newFourth_octet);
  void setKaScadaCloudIp(IPAddress ip);
  void setKaScadaCloudIpNmber(uint32_t ip) { setKaScadaCloudIp(flprog::numberToIp(ip)); };
  IPAddress kaScadaCloudIp() { return _cloudIp; };
  void setKaScadaCloudHost(String host);
  void setKaScadaCloudPort(int16_t newPort);
  uint16_t kaScadaCloudPort() { return _cloudPort; };
  void setKaScadaCloudDevceId(String id);
  String kaScadaCloudDevceId() { return _deniceId; };
  void setData(ModbusMainData *data) { _data = data; };
  void setDataTable(ModbusTable *table) { mainData()->setDataTable(table); };
  void setDataTable(uint8_t _table, uint32_t dataSize, int32_t *addresses) { mainData()->setDataTable(_table, dataSize, addresses); };
  void configDataTable(uint8_t table, uint32_t dataSize, uint32_t startAddress = 0) { mainData()->configDataTable(table, dataSize, startAddress); };

  void saveLong(int32_t val, uint8_t table, int32_t address) { mainData()->saveLong(val, table, address); };
  void saveUnsignedLong(uint32_t val, uint8_t table, int32_t address) { mainData()->saveUnsignedLong(val, table, address); };
  void saveFloat(float val, uint8_t table, int32_t address) { mainData()->saveFloat(val, table, address); };
  void saveInteger(uint16_t val, uint8_t table, int32_t address) { mainData()->saveInteger(val, table, address); };
  void saveByte(uint8_t val, uint8_t table, int32_t address) { mainData()->saveByte(val, table, address); };
  void saveBool(bool val, uint8_t table, int32_t address) { mainData()->saveBool(val, table, address); };

  void saveLongByIndex(int32_t val, uint8_t table, int32_t addressIndex) { mainData()->saveLongByIndex(val, table, addressIndex); };
  void saveUnsignedLongByIndex(uint32_t val, uint8_t table, int32_t addressIndex) { mainData()->saveUnsignedLong(val, table, addressIndex); };
  void saveFloatByIndex(float val, uint8_t table, int32_t addressIndex) { mainData()->saveFloatByIndex(val, table, addressIndex); };
  void saveIntegerByIndex(uint16_t val, uint8_t table, int32_t addressIndex) { mainData()->saveIntegerByIndex(val, table, addressIndex); };
  void saveByteByIndex(uint8_t val, uint8_t table, int32_t addressIndex) { mainData()->saveByteByIndex(val, table, addressIndex); };
  void saveBoolByIndex(bool val, uint8_t table, int32_t addressIndex) { mainData()->saveBoolByIndex(val, table, addressIndex); };

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

  void setMode(uint8_t mode);
  void byTcp() { setMode(FLPROG_TCP_MODBUS); };
  void byRtuOverTcp() { setMode(FLPROG_RTU_OVER_TCP_MODBUS); };
  void byKasCadaCloud() { setMode(FLPROG_KASCADA_CLOUD_MODBUS); };
  uint8_t mode() { return _mode; };

  int32_t getAdress(uint8_t table, int32_t index) { return mainData()->getAdress(table, index); };
  void setAdress(uint8_t table, int32_t index, int32_t addr) { mainData()->setAdress(table, index, addr); };

  ModbusMainData *mainData();

  void setLongOrder(uint8_t order) { mainData()->setLongOrder(order); };
  void setFloatOrder(uint8_t order) { mainData()->setFloatOrder(order); };
  void setUnsignedlongOrder(uint8_t order) { mainData()->setUnsignedlongOrder(order); };
  void setIntOrder(uint8_t order) { mainData()->setIntOrder(order); };

  uint8_t intOrder() { return mainData()->intOrder(); };
  uint8_t longOrder() { return mainData()->longOrder(); };
  uint8_t floatOrder() { return mainData()->floatOrder(); };
  uint8_t unsignedlongOrder() { return mainData()->unsignedlongOrder(); };

  uint32_t getTimeOutTime() { return mainData()->getTimeOutTime(); };
  void setTimeOutTime(uint32_t time) { mainData()->setTimeOutTime(time); };
  void setCallBack(FLProgModbusNewDataCallback func);

protected:
  virtual uint8_t validateRequest();
  virtual void begin();
  virtual void getRxBuffer();
  virtual void sendTxBuffer();
  uint8_t rxBuffer();
  void connect();

  uint8_t _mode = FLPROG_TCP_MODBUS;
  uint8_t _slaveAddres = 1;
  FLProgEthernetServer _server;
  int16_t _port = 502;
  ModbusMainData *_data = 0;

  uint8_t _mbapBuffer[6];

  // TCP stream state — исправил ander.
  uint8_t _tcpRxMbapSize = 0;
  uint16_t _tcpRxExpectedSize = 0;
  uint16_t _tcpRxBodySize = 0;
  uint16_t _rtuOverTcpRxSize = 0;
  uint32_t _tcpRxStartTime = 0;

  FLProgAbstractTcpInterface *_interface = 0;

  FLProgEthernetClient _tcpClient;
  int16_t _cloudPort = 25000;
  String _deniceId;
  IPAddress _cloudIp = IPAddress(94, 250, 249, 225);
  char _cloudHost[FLPROG_HOST_NAME_LENGTH] = "";
  bool _cloudAdressAsHost = false;
  uint32_t _kaScadaCloudTimeOutStartTime;
};
