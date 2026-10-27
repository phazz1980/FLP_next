#include "flprogModbusTCPSlaveServer.h"

void ModbusTCPSlaveServer::setSlaves(ModbusSlaveInMaster *_table, uint8_t size)
{
  slaves = _table;
  slavesSize = size;
}

void ModbusTCPSlaveServer::setIpAdress(IPAddress ip)
{
  _serverAsHost = false;
  _serverIp = ip;
  _tcpClient.stop();
}

void ModbusTCPSlaveServer::setHost(String host)
{
  _tcpClient.stop();
  if (host.length() == 0)
  {
    return;
  }
  _serverAsHost = true;
  if (host.equals(String(_serverHost)))
  {
    return;
  }
  host.toCharArray(_serverHost, FLPROG_HOST_NAME_LENGTH);
}

void ModbusTCPSlaveServer::setMode(uint8_t mode)
{
  if (mode > 1)
  {
    return;
  }
  if (_mode == mode)
  {
    return;
  }
  _mode = mode;
}

void ModbusTCPSlaveServer::setSlavesSize(uint8_t size)
{
  slavesSize = size;
  slaves = new ModbusSlaveInMaster[slavesSize];
}

bool ModbusTCPSlaveServer::hasSlave(uint8_t slaveIndex)
{
  return slaveIndex < slavesSize;
}

ModbusSlaveInMaster *ModbusTCPSlaveServer::slaveOnIndex(uint8_t slaveIndex)
{
  if (slaves == 0)
  {
    slaves = new ModbusSlaveInMaster[slavesSize];
  }
  if (hasSlave(slaveIndex))
  {
    return &slaves[slaveIndex];
  }
  return 0;
}

ModbusSlaveInMaster *ModbusTCPSlaveServer::slave(uint8_t slaveAddr, bool isIndex)
{
  if (isIndex)
  {
    return slaveOnIndex(slaveAddr);
  }
  if (slaves == 0)
  {
    slaves = new ModbusSlaveInMaster[slavesSize];
  }

  for (uint8_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].slaveAddress() == slaveAddr)
    {
      return &slaves[i];
    }
  }
  return 0;
}

void ModbusTCPSlaveServer::setDataTable(uint8_t slaveAddres, ModbusTable *table)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, false);
  if (sl == 0)
  {
    return;
  }
  sl->setDataTable(table);
}

void ModbusTCPSlaveServer::setDataTable(uint8_t slaveAddres, uint8_t table, int32_t dataSize, int32_t *addresses)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, false);
  if (sl == 0)
  {
    return;
  }
  sl->setDataTable(table, dataSize, addresses);
}

void ModbusTCPSlaveServer::configDataTable(uint8_t slaveAddres, uint8_t table, int32_t dataSize)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, false);
  if (sl == 0)
  {
    return;
  }
  sl->configDataTable(table, dataSize);
}

void ModbusTCPSlaveServer::configDataTable(uint8_t slaveAddres, uint8_t table, int32_t dataSize, uint32_t startAddress)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, false);
  if (sl == 0)
  {
    return;
  }
  sl->configDataTable(table, dataSize, startAddress);
}

void ModbusTCPSlaveServer::setSlaveAddress(uint8_t slaveIndex, uint16_t addr)
{
  ModbusSlaveInMaster *sl = slave(slaveIndex, true);
  if (sl == 0)
  {
    return;
  }
  sl->setSlaveAddress(addr);
}

uint8_t ModbusTCPSlaveServer::getSlaveAddress(uint8_t slaveIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveIndex, true);
  if (sl == 0)
  {
    return 0;
  }
  return sl->getSlaveAddress();
}

void ModbusTCPSlaveServer::setPollingPeriod(uint8_t slaveAddres, uint32_t period, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setPollingPeriod(period);
}

uint32_t ModbusTCPSlaveServer::pollingPeriod(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->pollingPeriod();
}

void ModbusTCPSlaveServer::setTimeOutTime(uint8_t slaveAddres, uint32_t time, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setTimeOutTime(time);
}

uint32_t ModbusTCPSlaveServer::getTimeOutTime(uint8_t slaveAdr, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->getTimeOutTime();
}

void ModbusTCPSlaveServer::setLongOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setLongOrder(order);
}

void ModbusTCPSlaveServer::setFloatOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setFloatOrder(order);
}

void ModbusTCPSlaveServer::setUnsignedlongOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setUnsignedlongOrder(order);
}

void ModbusTCPSlaveServer::setIntOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setIntOrder(order);
}

void ModbusTCPSlaveServer::setCallBack(uint8_t slaveAdr, FLProgModbusNewDataCallback func, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->setCallBack(func);
}

uint8_t ModbusTCPSlaveServer::intOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->intOrder();
}

uint8_t ModbusTCPSlaveServer::longOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->longOrder();
}

uint8_t ModbusTCPSlaveServer::floatOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->floatOrder();
}

uint8_t ModbusTCPSlaveServer::unsignedlongOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->unsignedlongOrder();
}

uint8_t ModbusTCPSlaveServer::hrSendMode(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->hrSendMode();
}

void ModbusTCPSlaveServer::setHrSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->hrSendMode(mode);
}

uint8_t ModbusTCPSlaveServer::coilSendMode(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->coilSendMode();
}

void ModbusTCPSlaveServer::setCoilSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->coilSendMode(mode);
}

void ModbusTCPSlaveServer::saveLong(uint8_t slaveAddres, int32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveLong(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveUnsignedLong(uint8_t slaveAddres, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveUnsignedLong(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveFloat(uint8_t slaveAddres, float value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveFloat(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveInteger(uint8_t slaveAddres, uint16_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveInteger(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveByte(uint8_t slaveAddres, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveByte(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveBool(uint8_t slaveAddres, bool value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveBool(value, table, startAddress);
}

void ModbusTCPSlaveServer::saveLongByIndex(uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveLongByIndex(value, table, startAddressIndex);
}

void ModbusTCPSlaveServer::saveUnsignedLongByIndex(uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveUnsignedLongByIndex(value, table, startAddressIndex);
}

void ModbusTCPSlaveServer::saveFloatByIndex(uint8_t slaveAdr, float value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveFloatByIndex(value, table, startAddressIndex);
}

void ModbusTCPSlaveServer::saveIntegerByIndex(uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveIntegerByIndex(value, table, startAddressIndex);
}

void ModbusTCPSlaveServer::saveByteByIndex(uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveByteByIndex(value, table, startAddressIndex);
}

void ModbusTCPSlaveServer::saveBoolByIndex(uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->saveBoolByIndex(value, table, startAddressIndex);
}

uint8_t ModbusTCPSlaveServer::readByte(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->readByte(table, startAddress);
}

uint16_t ModbusTCPSlaveServer::readInteger(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->readInteger(table, startAddress);
}

float ModbusTCPSlaveServer::readFloat(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->readFloat(table, startAddress);
}

int32_t ModbusTCPSlaveServer::readLong(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->readLong(table, startAddress);
}

uint32_t ModbusTCPSlaveServer::readUnsignedLong(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 0;
  }
  return sl->readUnsignedLong(table, startAddress);
}

bool ModbusTCPSlaveServer::readBool(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readBool(table, startAddress);
}

uint8_t ModbusTCPSlaveServer::readByteByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readByteByIndex(table, startAddressIndex);
}

uint16_t ModbusTCPSlaveServer::readIntegerByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readIntegerByIndex(table, startAddressIndex);
}

float ModbusTCPSlaveServer::readFloatByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readFloatByIndex(table, startAddressIndex);
}

int32_t ModbusTCPSlaveServer::readLongByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readLongByIndex(table, startAddressIndex);
}

uint32_t ModbusTCPSlaveServer::readUnsignedLongByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readUnsignedLongByIndex(table, startAddressIndex);
}

bool ModbusTCPSlaveServer::readBoolByIndex(uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAdr, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->readBoolByIndex(table, startAddressIndex);
}

int16_t ModbusTCPSlaveServer::getAdress(uint8_t slaveAddres, uint8_t table, int16_t index)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, true);
  if (sl == 0)
  {
    return -1;
  }
  return sl->getAdress(table, index);
}

void ModbusTCPSlaveServer::setAdress(uint8_t slaveAddres, uint8_t table, int16_t index, int16_t addr)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, true);
  if (sl == 0)
  {
    return;
  }
  sl->setAdress(table, index, addr);
}

uint8_t ModbusTCPSlaveServer::getLastError(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return 70;
  }
  return sl->getLastError();
}

void ModbusTCPSlaveServer::status(uint8_t slaveAddres, bool status, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return;
  }
  sl->status(status);
}

bool ModbusTCPSlaveServer::slaveStatus(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl = slave(slaveAddres, isIndex);
  if (sl == 0)
  {
    return false;
  }
  return sl->slaveStatus();
}

bool ModbusTCPSlaveServer::hasWriteRegisters()
{
  if (_isWorkPause)
  {
    if (flprog::isTimer(_startWorkPauseTime, _workPause))
    {
      _isWorkPause = false;
    }
    else
    {
      return false;
    }
  }
  if (!_isActive)
  {
    return false;
  }
  return !(firstWriteSlave() == 0);
}

ModbusSlaveInMaster *ModbusTCPSlaveServer::firstWriteSlave()
{
  if (!_isActive)
  {
    return 0;
  }
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if ((slaves[i].status()) && (slaves[i].hasWriteRegisters()))
    {
      return &slaves[i];
    }
  }
  return 0;
}

ModbusSlaveInMaster *ModbusTCPSlaveServer::firstReadySlave()
{

  for (int16_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].isReady())
    {
      return &slaves[i];
    }
  }
  return 0;
}

ModbusSlaveInMaster *ModbusTCPSlaveServer::nextSlave(ModbusSlaveInMaster *currentSlave)
{
  bool isFindSlave = false;
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if (isFindSlave)
    {
      if (slaves[i].isReady())
      {
        return &slaves[i];
      }
    }
    else
    {
      if (currentSlave == &slaves[i])
      {
        isFindSlave = true;
      }
    }
  }
  return 0;
}

bool ModbusTCPSlaveServer::isReady()
{
  if (_isWorkPause)
  {
    if (flprog::isTimer(_startWorkPauseTime, _workPause))
    {
      _isWorkPause = false;
    }
    else
    {
      return false;
    }
  }
  if (!_isActive)
  {
    return false;
  }
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].isReady())
    {
      return true;
    }
  }
  return false;
}

uint8_t ModbusTCPSlaveServer::connect()
{
  if (serverAsHost())
  {
    return _tcpClient.connect(getHost(), getPort());
  }
  return _tcpClient.connect(getIp(), getPort());
}

bool ModbusTCPSlaveServer::getIsActive()
{
  return _isActive;
}

void ModbusTCPSlaveServer::setWorkPause(uint32_t time)
{
  _startWorkPauseTime = millis();
  _workPause = time;
  _isWorkPause = true;
}

uint32_t ModbusTCPSlaveServer::errorPauseTime()
{
  uint32_t result = 0;
  for (uint8_t i = 0; i < slavesSize; i++)
  {
    result = result + (slaves[i].pollingPeriod()) + (slaves[i].getTimeOutTime());
  }
  return result;
}