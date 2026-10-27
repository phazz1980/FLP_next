#include "flprogModbusMasterTCP.h"

ModbusMasterTCP::ModbusMasterTCP(FLProgAbstractTcpInterface *sourse, uint8_t size)
{
  _serversSize = size;
  _servs = new ModbusTCPSlaveServer[_serversSize];
  _interface = sourse;
  _status = FLPROG_MODBUS_READY;

  for (uint8_t i = 0; i < _serversSize; i++)
  {
    _servs[i].client()->setSourse(sourse);
  }
}

ModbusTCPSlaveServer *ModbusMasterTCP::servers()
{
  if (_servs == 0)
  {
    _servs = new ModbusTCPSlaveServer[_serversSize];
  }
  return _servs;
}

ModbusTCPSlaveServer *ModbusMasterTCP::server(uint8_t serverIndex)
{
  if (hasServer(serverIndex))
  {
    return &servers()[serverIndex];
  }
  return 0;
}

uint8_t ModbusMasterTCP::serverMode(uint8_t serverIndex)
{
  if (hasServer(serverIndex))
  {
    return server(serverIndex)->mode();
  }
  return 0;
}

void ModbusMasterTCP::setServerSlavesSize(uint8_t serverIndex, uint8_t size)
{
  if (hasServer(serverIndex))
  {
    server(serverIndex)->setSlavesSize(size);
  }
}

void ModbusMasterTCP::setServerPort(uint8_t serverIndex, int16_t port)
{
  if (hasServer(serverIndex))
  {
    if ((server(serverIndex)->getPort()) != port)
    {
      server(serverIndex)->setPort(port);
    }
  }
}

int16_t ModbusMasterTCP::serverPort(uint8_t serverIndex)
{
  if (hasServer(serverIndex))
  {
    return server(serverIndex)->port();
  }
  return 0;
}

void ModbusMasterTCP::setServerMode(uint8_t serverIndex, uint8_t mode)
{

  if (hasServer(serverIndex))
  {
    server(serverIndex)->setMode(mode);
  }
}

void ModbusMasterTCP::setServerIpAdress(uint8_t serverIndex, IPAddress ip)
{
  if (hasServer(serverIndex))
  {
    if ((server(serverIndex)->getIp()) != ip)
    {
      server(serverIndex)->setIpAdress(ip);
    }
  }
}

IPAddress ModbusMasterTCP::serverIpAdress(uint8_t serverIndex)
{
  if (hasServer(serverIndex))
  {
    return server(serverIndex)->ipAdress();
  }
  return FLPROG_INADDR_NONE;
}

void ModbusMasterTCP::setServerHost(uint8_t serverIndex, String host)
{
  if (hasServer(serverIndex))
  {
    server(serverIndex)->setHost(host);
  }
}

void ModbusMasterTCP::setSlaveAddress(uint8_t serverIndex, uint8_t slaveIndex, uint16_t address)
{
  if (hasServer(serverIndex))
  {
    server(serverIndex)->setSlaveAddress(slaveIndex, address);
  }
}

uint8_t ModbusMasterTCP::getSlaveAddress(uint8_t serverIndex, uint8_t slaveIndex)
{
  if (hasServer(serverIndex))
  {
    return server(serverIndex)->getSlaveAddress(slaveIndex);
  }
  return 0;
}

void ModbusMasterTCP::setDataTable(uint8_t serverIndex, uint8_t slaveAddres, ModbusTable *table)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setDataTable(slaveAddres, table);
}

void ModbusMasterTCP::setDataTable(uint8_t serverIndex, uint8_t slaveAddres, uint8_t _table, int32_t dataSize, int32_t *_adresses)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setDataTable(slaveAddres, _table, dataSize, _adresses);
}

void ModbusMasterTCP::configDataTable(uint8_t serverIndex, uint8_t slaveAddres, uint8_t _table, int32_t dataSize)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->configDataTable(slaveAddres, _table, dataSize);
}

void ModbusMasterTCP::configDataTable(uint8_t serverIndex, uint8_t slaveAddres, uint8_t _table, int32_t dataSize, uint32_t startAddress)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->configDataTable(slaveAddres, _table, dataSize, startAddress);
}

void ModbusMasterTCP::setPollingPeriod(uint8_t serverIndex, uint8_t slaveAddres, uint32_t period, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setPollingPeriod(slaveAddres, period, isIndex);
}

uint32_t ModbusMasterTCP::pollingPeriod(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->pollingPeriod(slaveAddres, isIndex);
}

void ModbusMasterTCP::setTimeOutTime(uint8_t serverIndex, uint8_t slaveAddres, uint32_t time, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setTimeOutTime(slaveAddres, time, isIndex);
}

uint32_t ModbusMasterTCP::getTimeOutTime(uint8_t serverIndex, uint8_t slaveAdr, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->getTimeOutTime(slaveAdr, isIndex);
}

void ModbusMasterTCP::setLongOrder(uint8_t serverIndex, uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setLongOrder(slaveAddres, order, isIndex);
}

void ModbusMasterTCP::setFloatOrder(uint8_t serverIndex, uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setFloatOrder(slaveAddres, order, isIndex);
}

void ModbusMasterTCP::setUnsignedlongOrder(uint8_t serverIndex, uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setUnsignedlongOrder(slaveAddres, order, isIndex);
}

void ModbusMasterTCP::setIntOrder(uint8_t serverIndex, uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setIntOrder(slaveAddres, order, isIndex);
}

void ModbusMasterTCP::setCallBack(uint8_t serverIndex, uint8_t slaveAdr, FLProgModbusNewDataCallback func, bool isIndex)
{

  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setCallBack(slaveAdr, func, isIndex);
}

uint8_t ModbusMasterTCP::intOrder(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->intOrder(slaveAddres, isIndex);
}

uint8_t ModbusMasterTCP::longOrder(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->longOrder(slaveAddres, isIndex);
}

uint8_t ModbusMasterTCP::floatOrder(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->floatOrder(slaveAddres, isIndex);
}

uint8_t ModbusMasterTCP::unsignedlongOrder(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->unsignedlongOrder(slaveAddres, isIndex);
}

void ModbusMasterTCP::saveLong(uint8_t serverIndex, uint8_t slaveAddres, int32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveLong(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveUnsignedLong(uint8_t serverIndex, uint8_t slaveAddres, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveUnsignedLong(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveFloat(uint8_t serverIndex, uint8_t slaveAddres, float value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveFloat(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveInteger(uint8_t serverIndex, uint8_t slaveAddres, uint16_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveInteger(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveByte(uint8_t serverIndex, uint8_t slaveAddres, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveByte(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveBool(uint8_t serverIndex, uint8_t slaveAddres, bool value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveBool(slaveAddres, value, table, startAddress, isIndex);
}

void ModbusMasterTCP::saveLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, int32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveLongByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

void ModbusMasterTCP::saveUnsignedLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint32_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveUnsignedLongByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

void ModbusMasterTCP::saveFloatByIndex(uint8_t serverIndex, uint8_t slaveAdr, float value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveFloatByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

void ModbusMasterTCP::saveIntegerByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint16_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveIntegerByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

void ModbusMasterTCP::saveByteByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveByteByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

void ModbusMasterTCP::saveBoolByIndex(uint8_t serverIndex, uint8_t slaveAdr, bool value, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->saveBoolByIndex(slaveAdr, value, table, startAddressIndex, isIndex);
}

uint8_t ModbusMasterTCP::readByte(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->readByte(slaveAddres, table, startAddress, isIndex);
}

uint16_t ModbusMasterTCP::readInteger(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->readInteger(slaveAddres, table, startAddress, isIndex);
}

float ModbusMasterTCP::readFloat(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->readFloat(slaveAddres, table, startAddress, isIndex);
}

int32_t ModbusMasterTCP::readLong(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->readLong(slaveAddres, table, startAddress, isIndex);
}

uint32_t ModbusMasterTCP::readUnsignedLong(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->readUnsignedLong(slaveAddres, table, startAddress, isIndex);
}

bool ModbusMasterTCP::readBool(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readBool(slaveAddres, table, startAddress, isIndex);
}

uint8_t ModbusMasterTCP::readByteByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readByteByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

uint16_t ModbusMasterTCP::readIntegerByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readIntegerByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

float ModbusMasterTCP::readFloatByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readFloatByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

int32_t ModbusMasterTCP::readLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readLongByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

uint32_t ModbusMasterTCP::readUnsignedLongByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readUnsignedLongByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

bool ModbusMasterTCP::readBoolByIndex(uint8_t serverIndex, uint8_t slaveAdr, uint8_t table, int32_t startAddressIndex, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->readBoolByIndex(slaveAdr, table, startAddressIndex, isIndex);
}

uint8_t ModbusMasterTCP::hrSendMode(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->hrSendMode(slaveAddres, isIndex);
}

void ModbusMasterTCP::setHrSendMode(uint8_t serverIndex, uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setHrSendMode(slaveAddres, mode, isIndex);
}

uint8_t ModbusMasterTCP::coilSendMode(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 0;
  }
  return serv->coilSendMode(slaveAddres, isIndex);
}

void ModbusMasterTCP::setCoilSendMode(uint8_t serverIndex, uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setCoilSendMode(slaveAddres, mode, isIndex);
}

int32_t ModbusMasterTCP::getAdress(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t index)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return -1;
  }
  return serv->getAdress(slaveAddres, table, index);
}

void ModbusMasterTCP::setAdress(uint8_t serverIndex, uint8_t slaveAddres, uint8_t table, int32_t index, int32_t address)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->setAdress(slaveAddres, table, index, address);
}

uint8_t ModbusMasterTCP::getLastError(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return 60;
  }
  return serv->getLastError(slaveAddres, isIndex);
}

void ModbusMasterTCP::status(uint8_t serverIndex, uint8_t slaveAddres, bool status, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return;
  }
  serv->status(slaveAddres, status, isIndex);
}

bool ModbusMasterTCP::slaveStatus(uint8_t serverIndex, uint8_t slaveAddres, bool isIndex)
{
  ModbusTCPSlaveServer *serv = server(serverIndex);
  if (serv == 0)
  {
    return false;
  }
  return serv->slaveStatus(slaveAddres, isIndex);
}

void ModbusMasterTCP::connect(ModbusTCPSlaveServer *server)
{
  if (_tempCurrentServer != server)
  {
    _tempCurrentServer = server;
  }
  if (server == 0)
  {
    _status = FLPROG_MODBUS_READY;
    return;
  }
  if (_status == FLPROG_MODBUS_WAITING_CONNECT_CLIENT)
  {
    if (flprog::isTimer(_startSendTime, (_telegrammSlave->getTimeOutTime())))
    {
      _telegrammSlave->setLastError(244);
      _tempCurrentServer->stop();
      _tempCurrentServer->setWorkPause(errorPauseTime());
      _status = FLPROG_MODBUS_READY;
      return;
    }
  }
  else
  {
    _startSendTime = millis();
  }
  if (!_tempCurrentServer->connected())
  {
    uint8_t result = _tempCurrentServer->connect();
    if (result == FLPROG_WAIT)
    {
      _status = FLPROG_MODBUS_WAITING_CONNECT_CLIENT;
      return;
    }
    if (result == FLPROG_ERROR)
    {
      _telegrammSlave->setLastError(244);
      _tempCurrentServer->stop();
      _status = FLPROG_MODBUS_READY;
      nextServer();
      return;
    }
    return;
  }
  if (_telegrammServer->mode() == FLPROG_TCP_MODBUS)
  {
    /*
      ИСПРАВИЛ ander — Modbus TCP Master / Ethernet.

      БЫЛО:
      запрос Modbus TCP отправлялся двумя последовательными write():

        _tempCurrentServer->client()->write(_mbapBuffer, 6);
        _tempCurrentServer->client()->write(_buffer, _bufferSize);

      После аналогичного подтверждённого исправления Slave по Ethernet
      Master переведён на отправку полного Modbus TCP ADU одним write().

      СТАЛО — исправил ander:
      [MBAP 6 bytes] + [Unit ID + PDU] собираются в один tcpFrame и
      отправляются одним вызовом write(). RTU over TCP не изменяется.
    */

    // БЫЛО — исправил ander:
    // _tempCurrentServer->client()->write(_mbapBuffer, 6);
    // _tempCurrentServer->client()->write(_buffer, _bufferSize);

    // СТАЛО — исправил ander:
    uint8_t tcpFrame[FLPROG_MODBUS_BUFER_SIZE + 6];
    for (uint8_t i = 0; i < 6; i++)
    {
      tcpFrame[i] = _mbapBuffer[i];
    }
    for (uint16_t i = 0; i < _bufferSize; i++)
    {
      tcpFrame[i + 6] = _buffer[i];
    }
    _tempCurrentServer->client()->write(tcpFrame, ((uint16_t)_bufferSize) + 6);
  }
  else
  {
    // RTU over TCP — оставляем прежнюю отправку одним write().
    _tempCurrentServer->client()->write(_buffer, _bufferSize);
  }
  // СТАЛО — исправил ander: новый ответ начинаем с чистого TCP accumulator.
  _tcpRxMbapSize = 0;
  _tcpRxExpectedSize = 0;
  _tcpRxBodySize = 0;
  _rtuOverTcpRxSize = 0;
  _status = FLPROG_MODBUS_WAITING_ANSWER;
  _bufferSize = 0;
  _startSendTime = millis();
}

bool ModbusMasterTCP::hasServer(uint8_t serverIndex)
{
  return serverIndex < _serversSize;
}

void ModbusMasterTCP::pool()
{
  if (!_enable)
  {
    return;
  }
  if (_interface == 0)
  {
    return;
  }
  if (_eventsCount < _skippingEvents)
  {
    _eventsCount++;
    return;
  }
  _eventsCount = 0;

  if (!_interface->isReady())
  {
    return;
  }

  if (!_isInit)
  {
    begin();
  }
  if (_status == FLPROG_MODBUS_WAITING_CONNECT_CLIENT)
  {
    connect(_telegrammServer);
    return;
  }
  if (_status == FLPROG_MODBUS_WAITING_ANSWER)
  {
    checkAnswer();
    return;
  }
  if (_status == FLPROG_MODBUS_READY)
  {
    nextQuery();
  }
}

void ModbusMasterTCP::checkAnswer()
{
  if (flprog::isTimer(_startSendTime, (_telegrammSlave->getTimeOutTime())))
  {
    _telegrammSlave->setWorkPause(errorPauseTime());
    _telegrammSlave->setLastError(244);
    _tempCurrentServer->stop();
    _tcpRxMbapSize = 0;
    _tcpRxExpectedSize = 0;
    _tcpRxBodySize = 0;
    _rtuOverTcpRxSize = 0;
    _status = FLPROG_MODBUS_READY;
    return;
  }
  getRxBuffer();
  if (_bufferSize == 0)
  {
    return;
  }
  if (_telegrammServer->mode() == FLPROG_TCP_MODBUS)
  {
    if (_mbapBuffer[0] != highByte(_telegrammAnswerId))
    {
      return;
    }
    if (_mbapBuffer[1] != lowByte(_telegrammAnswerId))
    {
      return;
    }
  }
  uint8_t exception = validateRequest();
  if (exception > 0)
  {
    _telegrammSlave->setLastError(exception);
    _status = FLPROG_MODBUS_READY;
    return;
  }
  _telegrammSlave->setLastError(0);
  writeMaserData(_telegrammTable, _telegrammStartAddres, _telegrammNumbeRegs);
}

void ModbusMasterTCP::getRxBuffer()
{
  uint8_t mode = _telegrammServer->mode();

  /*
    БЫЛО — исправил ander:
    currentByteIndex и _bufferSize начинались заново при каждом вызове,
    поэтому код предполагал, что весь TCP frame уже лежит в available().

    СТАЛО — исправил ander:
    Modbus TCP: 6 байт MBAP -> Length -> ровно Length байт Unit ID + PDU.
    RTU over TCP: RTU frame сохраняется между pool() до полной длины.
  */
  _bufferSize = 0;

  if (mode == FLPROG_TCP_MODBUS)
  {
    while (_tempCurrentServer->client()->available())
    {
      uint8_t currentByte = _tempCurrentServer->client()->read();
      if (_tcpRxMbapSize < 6)
      {
        _mbapBuffer[_tcpRxMbapSize++] = currentByte;
        if (_tcpRxMbapSize == 6)
        {
          _tcpRxExpectedSize = (uint16_t)word(_mbapBuffer[4], _mbapBuffer[5]);
          if ((_tcpRxExpectedSize == 0) || (_tcpRxExpectedSize > FLPROG_MODBUS_BUFER_SIZE))
          {
            _tcpRxMbapSize = 0;
            _tcpRxExpectedSize = 0;
            _tcpRxBodySize = 0;
            return;
          }
        }
        continue;
      }
      if (_tcpRxBodySize < _tcpRxExpectedSize)
      {
        _buffer[_tcpRxBodySize++] = currentByte;
      }
      if ((_tcpRxMbapSize == 6) && (_tcpRxBodySize >= _tcpRxExpectedSize))
      {
        break; // следующий ADU оставляем в socket до следующего pool()
      }
    }

    if ((_tcpRxMbapSize < 6) || (_tcpRxExpectedSize == 0) ||
        (_tcpRxBodySize < _tcpRxExpectedSize))
    {
      return;
    }
    _bufferSize = (uint8_t)_tcpRxExpectedSize;
    _tcpRxMbapSize = 0;
    _tcpRxExpectedSize = 0;
    _tcpRxBodySize = 0;
    return;
  }

  while (_tempCurrentServer->client()->available())
  {
    uint8_t currentByte = _tempCurrentServer->client()->read();
    if (_rtuOverTcpRxSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      _buffer[_rtuOverTcpRxSize++] = currentByte;
    }
    uint16_t expectedNow = flprogModus::masterRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
    if ((expectedNow > 0) && (_rtuOverTcpRxSize >= expectedNow))
    {
      break;
    }
  }
  uint16_t expected = flprogModus::masterRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
  if ((expected == 0) || (_rtuOverTcpRxSize < expected))
  {
    return;
  }
  _bufferSize = (uint8_t)expected;
  _rtuOverTcpRxSize = 0;
}

uint8_t ModbusMasterTCP::validateRequest()
{
  uint16_t pacadgeSize;
  if (_telegrammServer->mode() == FLPROG_RTU_OVER_TCP_MODBUS)
  {
    pacadgeSize = flprogModus::masterRTUPacadgeSize(_bufferSize, _buffer);
    if (pacadgeSize == 0)
    {
      return 253;
    }
    if (pacadgeSize > _bufferSize)
    {
      return 253;
    }
    if (!(flprogModus::checkCRCOnBuffer(pacadgeSize, _buffer)))
    {
      return 252;
    }
  }
  else
  {
    pacadgeSize = (uint16_t)word(_mbapBuffer[4], _mbapBuffer[5]);
    if ((pacadgeSize == 0) || (pacadgeSize != _bufferSize))
    {
      return 253;
    }
    // СТАЛО — исправил ander: строгая MBAP/correlation проверка.
    if ((_mbapBuffer[2] != 0) || (_mbapBuffer[3] != 0))
    {
      return 253;
    }
    if (_buffer[0] != _telegrammSlave->slaveAddress())
    {
      return 253;
    }
  }

  if ((_buffer[1] & 0x80) != 0)
  {
    if ((_buffer[1] & 0x7F) != _telegrammFunction)
    {
      return 253;
    }
    return _buffer[2];
  }
  if (_buffer[1] != _telegrammFunction)
  {
    return 253;
  }
  if (!(_telegrammSlave->isSupportFunction(_buffer[1])))
  {
    return 1;
  }
  return 0;
}

void ModbusMasterTCP::nextQuery()
{
  if (!createNewTelegramm())
  {
    return;
  }
  sendQuery();
}

bool ModbusMasterTCP::createNewTelegramm()
{
  ModbusTCPSlaveServer *writeServer = firstWriteServer();
  if (writeServer != 0)
  {
    return createWriteTelegramm(writeServer);
  }
  if (!nextStep())
  {
    return false;
  }
  return createReadTelegram();
}

bool ModbusMasterTCP::createReadTelegram()
{
  _telegrammServer = _currentServer;
  _telegrammSlave = _currentSlave;
  _telegrammTable = _currentSlaveTable;
  _telegrammStartAddres = _currentSlaveStartAddress;
  _telegrammNumbeRegs = _currentSlaveTable->readRegsSize(_telegrammStartAddres);
  _currentSlaveLastAddress = _telegrammStartAddres + _telegrammNumbeRegs - 1;
  if (_currentSlaveTable->tableType() == FLPROG_COIL)
  {
    _telegrammFunction = 1;
  }
  if (_currentSlaveTable->tableType() == FLPROG_DISCRETE_INPUT)
  {
    _telegrammFunction = 2;
  }
  if (_currentSlaveTable->tableType() == FLPROG_INPUT_REGISTR)
  {
    _telegrammFunction = 4;
  }
  if (_currentSlaveTable->tableType() == FLPROG_HOLDING_REGISTR)
  {
    _telegrammFunction = 3;
  }
  return true;
}

bool ModbusMasterTCP::nextStep()
{
  if (!hasReadyServer())
  {
    return false;
  }
  if (_currentServer == 0)
  {
    _currentServer = firstReadyServer();
    _currentSlave = _currentServer->firstReadySlave();
    _currentSlaveTable = _currentSlave->firstTabe();
    _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
    return true;
  }
  if (!_currentServer->isReady())
  {
    return nextServer();
  }
  return nextRegistor();
}

bool ModbusMasterTCP::nextRegistor()
{
  if (!_currentSlave->isReady())
  {
    return nextSlave();
  }
  _currentSlaveStartAddress = _currentSlaveTable->findNextStartAddres(_currentSlaveLastAddress);
  if (_currentSlaveStartAddress == -1)
  {
    return nextTable();
  }
  return true;
}

bool ModbusMasterTCP::nextTable()
{
  if (!_currentSlave->isReady())
  {
    return nextSlave();
  }
  _currentSlaveTable = _currentSlave->nextTable(_currentSlaveTable);
  if (_currentSlaveTable == 0)
  {
    return nextSlave();
  }

  _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
  return true;
}

bool ModbusMasterTCP::nextSlave()
{
  if (_currentSlave != 0)
  {
    _currentSlave->lastReqest(millis());
    _currentSlave->setInit();
  }
  _currentSlave = _currentServer->nextSlave(_currentSlave);
  if (_currentSlave == 0)
  {
    return nextServer();
  }
  _currentSlaveTable = _currentSlave->firstTabe();
  _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
  return true;
}

bool ModbusMasterTCP::nextServer()
{
  _currentServer = nextReadyServer(_currentServer);
  if (_currentServer == 0)
  {
    return false;
  }
  _currentSlave = _currentServer->firstReadySlave();
  _currentSlaveTable = _currentSlave->firstTabe();
  _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
  return true;
}

ModbusTCPSlaveServer *ModbusMasterTCP::firstReadyServer()
{
  for (int16_t i = 0; i < _serversSize; i++)
  {
    if (servers()[i].isReady())
    {
      return &servers()[i];
    }
  }
  return 0;
}

ModbusTCPSlaveServer *ModbusMasterTCP::nextReadyServer(ModbusTCPSlaveServer *oldServer)
{
  bool isFindServer = false;
  for (int16_t i = 0; i < _serversSize; i++)
  {
    if (isFindServer)
    {
      if (server(i)->isReady())
      {
        return server(i);
      }
    }
    else
    {
      if (oldServer == server(i))
      {
        isFindServer = true;
      }
    }
  }
  return 0;
}

bool ModbusMasterTCP::hasReadyServer()
{
  for (int16_t i = 0; i < _serversSize; i++)
  {
    if (server(i)->isReady())
    {
      return true;
    }
  }
  return false;
}

void ModbusMasterTCP::sendQuery()

{
  _telegrammAnswerId++;
  _mbapBuffer[0] = highByte(_telegrammAnswerId);
  _mbapBuffer[1] = lowByte(_telegrammAnswerId);
  _mbapBuffer[2] = 0;
  _mbapBuffer[3] = 0;
  _buffer[0] = _telegrammSlave->slaveAddress();
  _buffer[1] = _telegrammFunction;
  create_PDU(_telegrammTable, _telegrammStartAddres, _telegrammNumbeRegs);
  _startSendTime = millis();
}

uint8_t ModbusMasterTCP::calculateSendRegSize()
{
  if (_telegrammTable->tableType() == FLPROG_HOLDING_REGISTR)
  {
    if (_telegrammSlave->hrSendMode() == FLPROG_SEND_HR_F6)
    {
      return 1;
    }
  }
  else
  {
    if (_telegrammSlave->coilSendMode() == FLPROG_SEND_COIL_F5)
    {
      return 1;
    }
  }
  return _telegrammTable->writeRegsSize(_telegrammStartAddres);
}

uint8_t ModbusMasterTCP::selectSendFunction()
{
  if (_telegrammTable->tableType() == FLPROG_HOLDING_REGISTR)
  {
    if (_telegrammSlave->hrSendMode() == FLPROG_SEND_HR_F6)
    {
      return FLPROG_SEND_HR_F6;
    }
    if (_telegrammSlave->hrSendMode() == FLPROG_SEND_HR_F16)
    {
      return FLPROG_SEND_HR_F16;
    }
    if (_telegrammNumbeRegs == 1)
    {
      return FLPROG_SEND_HR_F6;
    }
    else
    {
      return FLPROG_SEND_HR_F16;
    }
  }
  if (_telegrammSlave->coilSendMode() == FLPROG_SEND_COIL_F5)
  {
    return FLPROG_SEND_COIL_F5;
  }
  if (_telegrammSlave->coilSendMode() == FLPROG_SEND_COIL_F15)
  {
    return FLPROG_SEND_COIL_F15;
  }
  if (_telegrammNumbeRegs == 1)
  {
    return FLPROG_SEND_COIL_F5;
  }
  else
  {
    return FLPROG_SEND_COIL_F15;
  }
}

bool ModbusMasterTCP::createWriteTelegramm(ModbusTCPSlaveServer *writeServer)
{
  _telegrammServer = writeServer;
  _telegrammSlave = _telegrammServer->firstWriteSlave();
  _telegrammTable = _telegrammSlave->firstWriteTable();
  _telegrammStartAddres = _telegrammTable->firstWriteAddress();
  _telegrammNumbeRegs = calculateSendRegSize();
  _telegrammFunction = selectSendFunction();
  for (uint16_t i = 0; i < _telegrammNumbeRegs; i++)
  {
    _telegrammTable->resetWriteFlag(_telegrammStartAddres + i);
  }
  return true;
}

ModbusTCPSlaveServer *ModbusMasterTCP::firstWriteServer()
{
  for (int16_t i = 0; i < _serversSize; i++)
  {
    if (server(i)->hasWriteRegisters())
    {
      return server(i);
    }
  }
  return 0;
}



void ModbusMasterTCP::sendTxBuffer()
{
  if (_telegrammServer->mode() == FLPROG_TCP_MODBUS)
  {
    _mbapBuffer[4] = highByte(_bufferSize);
    _mbapBuffer[5] = lowByte(_bufferSize);
  }
  else
  {
    uint16_t crc = flprogModus::modbusCalcCRC(_bufferSize, _buffer);
    _buffer[_bufferSize] = lowByte(crc);
    _bufferSize++;
    _buffer[_bufferSize] = highByte(crc);
    _bufferSize++;
  }
  connect(_telegrammServer);
}

void ModbusMasterTCP::setSlavesToServer(uint8_t serverIndex, ModbusSlaveInMaster table[], int16_t size)
{
  if (hasServer(serverIndex))
  {
    server(serverIndex)->setSlaves(table, size);
  }
}

void ModbusMasterTCP::begin()
{
  _tcpRxMbapSize = 0;
  _tcpRxExpectedSize = 0;
  _tcpRxBodySize = 0;
  _rtuOverTcpRxSize = 0;

  for (uint8_t i = 0; i < _serversSize; i++)
  {
    _servs[i].stop();
  }
  _isInit = true;
}

uint32_t ModbusMasterTCP::errorPauseTime()
{
  uint32_t result = 0;
  for (uint8_t i = 0; i < _serversSize; i++)
  {
    result = result + (_servs[i].errorPauseTime());
  }
  return result;
}