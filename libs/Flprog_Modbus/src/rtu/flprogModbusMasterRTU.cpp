#include "flprogModbusMasterRTU.h"

ModbusMasterRTU::ModbusMasterRTU(uint8_t portNumber, uint8_t size, FlprogAbstractUartExecutor *executor)
{
  slavesSize = size;
  _executor = executor;
  slaves = new ModbusSlaveInMaster[slavesSize];
  _status = FLPROG_MODBUS_READY;
  setUart(portNumber);
}

void ModbusMasterRTU::begin()
{
  _executor->beginUart(_uartPortNumber);
  if (_pinPeDe >= 0)
  {
    pinMode(_pinPeDe, OUTPUT);
    digitalWrite(_pinPeDe, LOW);
  }
  _isInit = true;
}

ModbusSlaveInMaster *ModbusMasterRTU::slave(uint8_t adr)
{
  for (uint8_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].slaveAddress() == adr)
    {
      return &slaves[i];
    }
  }
  return 0;
}

ModbusSlaveInMaster *ModbusMasterRTU::slaveOnIndex(uint8_t slaveIndex)
{
  if (hasSlaveOnIndex(slaveIndex))
  {
    return &slaves[slaveIndex];
  }
  return 0;
}

void ModbusMasterRTU::pool()
{
  if (!_enable)
  {
    return;
  }
  if (_eventsCount < _skippingEvents)
  {
    _eventsCount++;
    return;
  }
  _eventsCount = 0;
  if (_isPause)
  {
    return;
  }
  if (!_isInit)
  {
    begin();
    return;
  }

  if (_status == FLPROG_MODBUS_WAITING_SENDING)
  {
    if (flprog::isTimer(_startSendTime, _timeOfSend))
    {
      _status = FLPROG_MODBUS_WAITING_ANSWER;
      _startSendTime = millis();
      offPeDePin();
    }
    else
    {
      return;
    }
  }

  if (_status == FLPROG_MODBUS_WAITING_PAUSE_BEFORE_ANSWERING)
  {
    if ((flprog::isTimer(_startPauseTime, _pauseTime)))
    {
      _status = FLPROG_MODBUS_READY;
    }
    return;
  }

  if (_status == FLPROG_MODBUS_WAITING_ANSWER)
  {
    checkAnswer();
  }
  if (_status == FLPROG_MODBUS_READY)
  {
    nextQuery();
  }
}

void ModbusMasterRTU::checkAnswer()
{
  if (flprog::isTimer(_startSendTime, (_telegrammSlave->getTimeOutTime())))
  {
    _telegrammSlave->setLastError(244);
    _telegrammSlave->setWorkPause(errorPauseTime());
    _status = FLPROG_MODBUS_READY;
    _bufferSize = 0;
    return;
  }
  if (checkAvaliblePacage())
  {
    uint8_t exception = validateRequest();
    if (exception > 0)
    {
      _telegrammSlave->setLastError(exception);
      _status = FLPROG_MODBUS_WAITING_PAUSE_BEFORE_ANSWERING;
      _startPauseTime = millis();
      _bufferSize = 0;
      return;
    }
    _telegrammSlave->setLastError(0);
    writeMaserData(_telegrammTable, _telegrammStartAddres, _telegrammNumbeRegs);
    _bufferSize = 0;
    _status = FLPROG_MODBUS_WAITING_PAUSE_BEFORE_ANSWERING;
    _startPauseTime = millis();
    return;
  }
  if (_bufferSize == 0)
  {
    return;
  }
  if (flprog::isTimer(_time, 50))
  {
    _bufferSize = 0;
  }
}

uint8_t ModbusMasterRTU::validateRequest()
{
  if (!(flprogModus::checkCRCOnBuffer(_bufferSize, _buffer)))
  {
    return 252;
  }
  if ((_buffer[1] & 0x80) != 0)
  {
    return _buffer[2];
  }
  if (!(_telegrammSlave->isSupportFunction(_buffer[1])))
  {
    return 1;
  }
  return 0;
}

void ModbusMasterRTU::nextQuery()
{
  if (!createNewTelegramm())
  {
    return;
  }
  sendQuery();
}

bool ModbusMasterRTU::hasWriteRegisters()
{
  return !(firstWriteSlave() == 0);
}

ModbusSlaveInMaster *ModbusMasterRTU::slaveForAddress(int16_t adress)
{
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].slaveAddress() == adress)
    {
      return &slaves[i];
    }
  }
  return 0;
}

ModbusSlaveInMaster *ModbusMasterRTU::firstWriteSlave()
{
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if ((slaves[i].status()) && (slaves[i].hasWriteRegisters()))
    {
      return &slaves[i];
    }
  }
  return 0;
}

bool ModbusMasterRTU::createNewTelegramm()
{
  if (hasWriteRegisters())
  {
    return createWriteTelegramm();
  }
  if (!nextStep())
  {
    return false;
  }
  return createReadTelegram();
}

bool ModbusMasterRTU::createReadTelegram()
{
  _telegrammSlave = _currentSlave;
  _telegrammTable = _currentSlaveTable;
  _telegrammStartAddres = _currentSlaveStartAddress;
  _telegrammNumbeRegs = _telegrammTable->readRegsSize(_telegrammStartAddres);
  _currentSlaveLastAddress = _telegrammStartAddres + _telegrammNumbeRegs - 1;
  if (_currentSlaveTable->tableType() == FLPROG_COIL)
  {
    _telegrammFunction = 1;
  }
  if (_telegrammTable->tableType() == FLPROG_DISCRETE_INPUT)
  {
    _telegrammFunction = 2;
  }
  if (_telegrammTable->tableType() == FLPROG_INPUT_REGISTR)
  {
    _telegrammFunction = 4;
  }
  if (_telegrammTable->tableType() == FLPROG_HOLDING_REGISTR)
  {
    _telegrammFunction = 3;
  }
  return true;
}

uint8_t ModbusMasterRTU::calculateSendRegSize()
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

uint8_t ModbusMasterRTU::selectSendFunction()
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

bool ModbusMasterRTU::createWriteTelegramm()
{
  _telegrammSlave = firstWriteSlave();
  _telegrammTable = _telegrammSlave->firstWriteTable();
  _telegrammStartAddres = _telegrammTable->firstWriteAddress();
  _telegrammNumbeRegs = calculateSendRegSize();
  _telegrammFunction = selectSendFunction();
  for (uint8_t i = 0; i < _telegrammNumbeRegs; i++)
  {
    _telegrammTable->resetWriteFlag(_telegrammStartAddres + i);
  }
  return true;
}

void ModbusMasterRTU::sendQuery()
{
  _buffer[0] = _telegrammSlave->slaveAddress();
  _buffer[1] = _telegrammFunction;
  create_PDU(_telegrammTable, _telegrammStartAddres, _telegrammNumbeRegs);
}

bool ModbusMasterRTU::nextStep()
{
  if (!hasReadySlave())
  {
    return false;
  }
  if (_currentSlave == 0)
  {
    _currentSlave = firstReadySlave();
    _currentSlaveTable = _currentSlave->firstTabe();
    _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
    return true;
  }
  if (!_currentSlave->isReady())
  {
    return nextSlave();
  }
  return nextRegistor();
}

ModbusSlaveInMaster *ModbusMasterRTU::firstReadySlave()
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

bool ModbusMasterRTU::nextRegistor()
{
  _currentSlaveStartAddress = _currentSlaveTable->findNextStartAddres(_currentSlaveLastAddress);
  if (_currentSlaveStartAddress == -1)
  {
    return nextTable();
  }
  return true;
}

bool ModbusMasterRTU::nextSlave()
{
  if (_currentSlave != 0)
  {
    _currentSlave->lastReqest(millis());
    _currentSlave->setInit();
  }
  _currentSlave = nextSlave(_currentSlave);
  if (_currentSlave == 0)
  {
    return false;
  }
  _currentSlaveTable = _currentSlave->firstTabe();
  _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
  return true;
}

ModbusSlaveInMaster *ModbusMasterRTU::nextSlave(ModbusSlaveInMaster *currentTempSlave)
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
      if (currentTempSlave == &slaves[i])
      {
        isFindSlave = true;
      }
    }
  }
  return 0;
}

bool ModbusMasterRTU::nextTable()
{
  _currentSlaveTable = _currentSlave->nextTable(_currentSlaveTable);
  if (_currentSlaveTable == 0)
  {
    return nextSlave();
  }

  _currentSlaveStartAddress = _currentSlaveTable->getMinAdress();
  return true;
}

bool ModbusMasterRTU::hasReadySlave()
{
  for (int16_t i = 0; i < slavesSize; i++)
  {
    if (slaves[i].isReady())
    {
      return true;
    }
  }
  return false;
}

bool ModbusMasterRTU::hasSlaveOnIndex(uint8_t slaveIndex)
{
  return slaveIndex < slavesSize;
}

void ModbusMasterRTU::saveLong(uint8_t slaveAddres, int32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }

  sl->saveLong(value, table, startAddress);
}

void ModbusMasterRTU::saveUnsignedLong(uint8_t slaveAddres, uint32_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveUnsignedLong(value, table, startAddress);
}

void ModbusMasterRTU::saveFloat(uint8_t slaveAddres, float value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveFloat(value, table, startAddress);
}

void ModbusMasterRTU::saveInteger(uint8_t slaveAddres, int16_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveInteger(value, table, startAddress);
}

void ModbusMasterRTU::saveByte(uint8_t slaveAddres, uint8_t value, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveByte(value, table, startAddress);
}

void ModbusMasterRTU::saveBool(uint8_t slaveAddres, bool val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveBool(val, table, address);
}

void ModbusMasterRTU::saveLongByIndex(uint8_t slaveAddres, int32_t val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveLongByIndex(val, table, address);
}

void ModbusMasterRTU::saveUnsignedLongByIndex(uint8_t slaveAddres, uint32_t val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveUnsignedLongByIndex(val, table, address);
}

void ModbusMasterRTU::saveFloatByIndex(uint8_t slaveAddres, float val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveFloatByIndex(val, table, address);
}

void ModbusMasterRTU::saveIntegerByIndex(uint8_t slaveAddres, int16_t val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveIntegerByIndex(val, table, address);
}

void ModbusMasterRTU::saveByteByIndex(uint8_t slaveAddres, uint8_t val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveByteByIndex(val, table, address);
}

void ModbusMasterRTU::saveBoolByIndex(uint8_t slaveAddres, bool val, uint8_t table, int32_t address, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->saveBoolByIndex(val, table, address);
}

uint8_t ModbusMasterRTU::readByte(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readByte(table, startAddress);
}

int16_t ModbusMasterRTU::readInteger(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{

  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readInteger(table, startAddress);
}

float ModbusMasterRTU::readFloat(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readFloat(table, startAddress);
}

int32_t ModbusMasterRTU::readLong(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readLong(table, startAddress);
}

uint32_t ModbusMasterRTU::readUnsignedLong(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readUnsignedLong(table, startAddress);
}

bool ModbusMasterRTU::readBool(uint8_t slaveAddres, uint8_t table, int32_t startAddress, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readBool(table, startAddress);
}

uint8_t ModbusMasterRTU::readByteByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readByteByIndex(table, addressIndex);
}

int16_t ModbusMasterRTU::readIntegerByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readIntegerByIndex(table, addressIndex);
}

float ModbusMasterRTU::readFloatByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readFloatByIndex(table, addressIndex);
}

int32_t ModbusMasterRTU::readLongByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readLongByIndex(table, addressIndex);
}

uint32_t ModbusMasterRTU::readUnsignedLongByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readUnsignedLongByIndex(table, addressIndex);
}

bool ModbusMasterRTU::readBoolByIndex(uint8_t slaveAddres, uint8_t table, int32_t addressIndex, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->readBoolByIndex(table, addressIndex);
}

int32_t ModbusMasterRTU::getAdress(uint8_t slaveAddres, uint8_t table, int32_t index)
{
  ModbusSlaveInMaster *sl;
  sl = slaveOnIndex(slaveAddres);
  if (sl == 0)
  {
    return -1;
  }
  return sl->getAdress(table, index);
}

void ModbusMasterRTU::setAdress(uint8_t slaveAddres, uint8_t table, int32_t index, int32_t address)
{
  ModbusSlaveInMaster *sl;
  sl = slaveOnIndex(slaveAddres);
  if (sl == 0)
  {
    return;
  }
  sl->setAdress(table, index, address);
}

void ModbusMasterRTU::setDataTable(uint8_t slaveAddres, ModbusTable *table)
{
  ModbusSlaveInMaster *sl;
  sl = slave(slaveAddres);
  if (sl == 0)
  {
    return;
  }
  sl->setDataTable(table);
}

void ModbusMasterRTU::setDataTable(uint8_t slaveAddres, uint8_t _table, int32_t dataSize, int32_t *_adresses)
{
  ModbusSlaveInMaster *sl;
  sl = slave(slaveAddres);
  if (sl == 0)
  {
    return;
  }
  sl->setDataTable(_table, dataSize, _adresses);
}

void ModbusMasterRTU::configDataTable(uint8_t slaveAddres, uint8_t _table, int32_t dataSize)
{
  ModbusSlaveInMaster *sl;
  sl = slave(slaveAddres);
  if (sl == 0)
  {
    return;
  }
  sl->configDataTable(_table, dataSize);
}

void ModbusMasterRTU::configDataTable(uint8_t slaveAddres, uint8_t _table, int32_t dataSize, int32_t _startAddress)
{
  ModbusSlaveInMaster *sl;
  sl = slave(slaveAddres);
  if (sl == 0)
  {
    return;
  }
  sl->configDataTable(_table, dataSize, _startAddress);
}

void ModbusMasterRTU::setSlaveAddress(uint8_t slaveIndex, uint8_t address)
{
  if (hasSlaveOnIndex(slaveIndex))
  {
    slaves[slaveIndex].setSlaveAddress(address);
  }
}

uint8_t ModbusMasterRTU::getSlaveAddress(uint8_t slaveIndex)
{
  if (hasSlaveOnIndex(slaveIndex))
  {
    return slaves[slaveIndex].getSlaveAddress();
  }
  return 0;
}

void ModbusMasterRTU::setPollingPeriod(uint8_t slaveAddres, uint32_t period, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setPollingPeriod(period);
}

uint32_t ModbusMasterRTU::pollingPeriod(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->pollingPeriod();
}

uint8_t ModbusMasterRTU::intOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->intOrder();
}

uint8_t ModbusMasterRTU::longOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->longOrder();
}

uint8_t ModbusMasterRTU::floatOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->floatOrder();
}

uint8_t ModbusMasterRTU::unsignedlongOrder(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->unsignedlongOrder();
}

uint8_t ModbusMasterRTU::hrSendMode(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->hrSendMode();
}

void ModbusMasterRTU::setHrSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->hrSendMode(mode);
}

uint8_t ModbusMasterRTU::coilSendMode(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->coilSendMode();
}

void ModbusMasterRTU::setCoilSendMode(uint8_t slaveAddres, uint8_t mode, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->coilSendMode(mode);
}

bool ModbusMasterRTU::slaveStatus(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->slaveStatus();
}

void ModbusMasterRTU::setTimeOutTime(uint8_t slaveAddres, uint32_t time, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setTimeOutTime(time);
}

uint32_t ModbusMasterRTU::getTimeOutTime(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 0;
  }
  return sl->getTimeOutTime();
}

void ModbusMasterRTU::setLongOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setLongOrder(order);
}

void ModbusMasterRTU::setCallBack(uint8_t slaveAddres, FLProgModbusNewDataCallback func, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setCallBack(func);
}

void ModbusMasterRTU::setFloatOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setFloatOrder(order);
}

void ModbusMasterRTU::setUnsignedlongOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setUnsignedlongOrder(order);
}

void ModbusMasterRTU::setIntOrder(uint8_t slaveAddres, uint8_t order, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->setIntOrder(order);
}

uint8_t ModbusMasterRTU::getLastError(uint8_t slaveAddres, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return 50;
  }
  return sl->getLastError();
}

void ModbusMasterRTU::status(uint8_t slaveAddres, bool status, bool isIndex)
{
  ModbusSlaveInMaster *sl;
  if (isIndex)
  {
    sl = slaveOnIndex(slaveAddres);
  }
  else
  {
    sl = slave(slaveAddres);
  }
  if (sl == 0)
  {
    return;
  }
  sl->status(status);
}

uint16_t ModbusMasterRTU::rtuPacadgeSize(uint16_t length, uint8_t bufferArray[])
{
  return flprogModus::masterRTUPacadgeSize(length, bufferArray);
}

uint32_t ModbusMasterRTU::errorPauseTime()
{
  uint32_t result = 0;
  for (uint8_t i = 0; i < slavesSize; i++)
  {
    result = result + (slaves[i].pollingPeriod()) + (slaves[i].getTimeOutTime());
  }
  return result;
}
