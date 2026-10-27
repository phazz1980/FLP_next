#include "flprogModbus.h"

//------------------Modbus------------------------------------

void Modbus::process_modbus_FC1(ModbusMainData *data, uint8_t table)
{
  ModbusTable *dataTable = data->tableForType(table);
  uint8_t bytesNo, bitsNo;
  int32_t currentCoil, coil;
  bool value;
  int32_t startCoil = word(_buffer[2], _buffer[3]);
  int32_t coilNo = word(_buffer[4], _buffer[5]);
  bytesNo = uint8_t(coilNo / 8);
  if (coilNo % 8 != 0)
  {
    bytesNo++;
  }
  _buffer[2] = bytesNo;
  _bufferSize = 3;
  bitsNo = 0;
  for (currentCoil = 0; currentCoil < coilNo; currentCoil++)
  {
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      coil = startCoil + currentCoil;
      value = dataTable->readBoolRegister(coil);
      bitWrite(_buffer[_bufferSize], bitsNo, value);
      bitsNo++;
      if (bitsNo > 7)
      {
        bitsNo = 0;
        _bufferSize++;
      }
    }
  }
  if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
  {
    if (coilNo % 8 != 0)
    {
      _bufferSize++;
    }
  }
  sendTxBuffer();
}

void Modbus::process_modbus_FC3(ModbusMainData *data, uint8_t table)
{
  ModbusTable *dataTable = data->tableForType(table);
  int32_t startAddress = word(_buffer[2], _buffer[3]);
  int32_t byteRegsno = word(_buffer[4], _buffer[5]);
  int32_t i;
  int16_t value;
  _buffer[2] = byteRegsno * 2;
  _bufferSize = 3;
  for (i = startAddress; i < startAddress + byteRegsno; i++)
  {
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      value = dataTable->readWorldRegister(i);
      _buffer[_bufferSize] = highByte(value);
      _bufferSize++;
      _buffer[_bufferSize] = lowByte(value);
      _bufferSize++;
    }
  }
  sendTxBuffer();
}

void Modbus::process_modbus_FC5(ModbusMainData *data)
{
  int32_t address = word(_buffer[2], _buffer[3]);
  ModbusTable *table = data->tableForType(FLPROG_COIL);
  bool value = (_buffer[4] == 0xff);
  table->writeRegister(address, value);
  _bufferSize = 6;
  sendTxBuffer();
}

void Modbus::process_modbus_FC6(ModbusMainData *data)
{
  int32_t address = word(_buffer[2], _buffer[3]);
  ModbusTable *table = data->tableForType(FLPROG_HOLDING_REGISTR);
  int16_t value = word(_buffer[4], _buffer[5]);
  table->writeRegister(address, value);
  _bufferSize = 6;
  sendTxBuffer();
}

void Modbus::process_modbus_FC15(ModbusMainData *data)
{
  uint8_t frameByte, bitsNo;
  uint8_t currentCoil;
  int32_t address;
  int32_t startCoil = word(_buffer[2], _buffer[3]);
  int32_t coilNo = word(_buffer[4], _buffer[5]);
  bitsNo = 0;
  frameByte = 7;
  bool value;
  ModbusTable *table = data->tableForType(FLPROG_COIL);
  for (currentCoil = 0; currentCoil < coilNo; currentCoil++)
  {
    address = startCoil + currentCoil;
    value = bitRead(_buffer[frameByte], bitsNo);
    table->writeRegister(address, value);
    bitsNo++;
    if (bitsNo > 7)
    {
      bitsNo = 0;
      frameByte++;
    }
  }
  _bufferSize = 6;
  sendTxBuffer();
}

void Modbus::process_modbus_FC16(ModbusMainData *data)
{
  int16_t value;
  int32_t address;
  ModbusTable *table = data->tableForType(FLPROG_HOLDING_REGISTR);
  int32_t startAddress = _buffer[2] << 8 | _buffer[3];
  int32_t byteRegsno = _buffer[4] << 8 | _buffer[5];
  int16_t i;
  _buffer[4] = 0;
  _buffer[5] = byteRegsno;
  _bufferSize = 6;
  for (i = 0; i < byteRegsno; i++)
  {
    address = startAddress + i;
    value = word(_buffer[7 + i * 2], _buffer[8 + i * 2]);
    table->writeRegister(address, value);
  }
  sendTxBuffer();
}

uint8_t Modbus::tabeTypeForFunction(uint8_t function)
{
  if ((function == 1) || (function == 5) || (function == 15))
  {
    return FLPROG_COIL;
  }
  if (function == 2)
  {
    return FLPROG_DISCRETE_INPUT;
  }
  if ((function == 3) || (function == 6) || (function == 16))
  {
    return FLPROG_HOLDING_REGISTR;
  }
  if (function == 4)
  {
    return FLPROG_INPUT_REGISTR;
  }
  return 100;
}

uint8_t Modbus::validateSlaveReqest(ModbusMainData *data)
{
  if (data == 0)
  {
    return 1;
  }
  if (!(data->isSupportFunction(_buffer[1])))
  {
    return 1;
  }
  uint32_t startAdress = word(_buffer[2], _buffer[3]);
  uint32_t count;
  uint8_t function = _buffer[1];
  uint8_t workTableType = tabeTypeForFunction(function);
  if (workTableType == 100)
  {
    return 2;
  }
  if ((function == 5) || (function == 6))
  {
    if (!(data->checkModbusAddres(startAdress, workTableType)))
    {
      return 2;
    }
  }
  else
  {
    count = word(_buffer[4], _buffer[5]);
    if (count > 25)
    {
      return 2;
    }
    if (!(data->checkModbusRange(startAdress, count, workTableType)))
    {
      return 2;
    }
  }
  return 0;
}

void Modbus::executeSlaveReqest(ModbusMainData *data, uint8_t address)
{
  if (address > 0)
  {
    if ((_buffer[0] != address) && (_buffer[0] != 0))
    {
      _status = FLPROG_MODBUS_READY;
      _bufferSize = 0;
      return;
    }
  }
  uint8_t exception = validateRequest();
  if (exception > 0)
  {
    setLastError(exception);
    if (exception < 252)
    {
      buildException(exception);
      sendTxBuffer();
      return;
    }
    _bufferSize = 0;
    _status = FLPROG_MODBUS_READY;
    return;
  }
  setLastError(0);
  if (_buffer[1] == 1)
  {
    process_modbus_FC1(data, FLPROG_COIL);
    return;
  }
  if (_buffer[1] == 2)
  {
    process_modbus_FC1(data, FLPROG_DISCRETE_INPUT);
    return;
  }
  if (_buffer[1] == 5)
  {
    process_modbus_FC5(data);
    return;
  }

  if (_buffer[1] == 15)
  {
    process_modbus_FC15(data);
    return;
  }

  if (_buffer[1] == 3)
  {
    process_modbus_FC3(data, FLPROG_HOLDING_REGISTR);
    return;
  }
  if (_buffer[1] == 4)
  {
    process_modbus_FC3(data, FLPROG_INPUT_REGISTR);
    return;
  }
  if (_buffer[1] == 6)
  {
    process_modbus_FC6(data);
    return;
  }
  if (_buffer[1] == 16)
  {
    process_modbus_FC16(data);
    return;
  }
}

void Modbus::buildException(uint8_t exception)
{
  uint8_t func = _buffer[1];
  _buffer[0] = 1;
  _buffer[1] = func + 0x80;
  _buffer[2] = exception;
  _bufferSize = 3;
}

void Modbus::get_FC1(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{
  uint8_t currentByte = 3;
  uint8_t currentBit = 0;
  bool value;
  int32_t currentIndex = startAddress;
  for (uint8_t i = 0; i < numberRegs; i++)
  {
    value = bitRead(_buffer[currentByte], currentBit);
    table->writeRegister(currentIndex, value);
    currentIndex++;
    currentBit++;
    if (currentBit > 7)
    {
      currentBit = 0;
      currentByte++;
    }
  }
}

void Modbus::get_FC3(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{

  int32_t currentIndex = startAddress;
  uint8_t currentByte = 3;
  int16_t value;

  for (int32_t i = 0; i < numberRegs; i++)
  {
    value = word(_buffer[currentByte], _buffer[currentByte + 1]);
    table->writeRegister((currentIndex + i), value);
    currentByte += 2;
  }
}

void Modbus::create_Read(int16_t numberRegs)
{
  _buffer[4] = highByte(numberRegs);
  _buffer[5] = lowByte(numberRegs);
  _bufferSize = _bufferSize + 2;
}

void Modbus::create_Write_FC5(ModbusTable *table, int32_t startAddress)
{
  if (table->readBoolRegister(startAddress))
  {
    _buffer[4] = 0xFF;
  }
  else
  {
    _buffer[4] = 0;
  }
  _buffer[5] = 0;
  _bufferSize = _bufferSize + 2;
}

void Modbus::create_Write_FC6(ModbusTable *table, int32_t startAddress)
{
  int16_t value = table->readWorldRegister(startAddress);
  _buffer[4] = highByte(value);
  _buffer[5] = lowByte(value);
  _bufferSize = _bufferSize + 2;
}

void Modbus::create_Write_FC15(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{
  bool value;
  _buffer[4] = highByte(numberRegs);
  _buffer[5] = lowByte(numberRegs);
  _bufferSize = _bufferSize + 3;
  uint8_t byteCounter = 0;
  uint8_t bitCounter = 0;
  uint8_t temp = 0;
  for (int32_t i = 0; i < numberRegs; i++)
  {
    if (bitCounter == 7)
    {
      _buffer[7 + byteCounter] = temp;
      _bufferSize++;
      byteCounter++;
      bitCounter = 0;
      temp = 0;
    }
    value = table->readBoolRegister(startAddress + i);
    bitWrite(temp, bitCounter, value);
    bitCounter++;
  }
  if (bitCounter < 7)
  {
    _buffer[7 + byteCounter] = temp;
    _bufferSize++;
  }
  _buffer[6] = byteCounter + 1;
}

void Modbus::create_Write_FC16(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{
  uint16_t value;
  _buffer[4] = highByte(numberRegs);
  _buffer[5] = lowByte(numberRegs);
  _buffer[6] = numberRegs * 2;
  _bufferSize = _bufferSize + 3;
  uint8_t byteCounter = 0;
  for (int16_t i = 0; i < numberRegs; i++)
  {
    value = table->readWorldRegister(startAddress + i);
    _buffer[7 + byteCounter] = highByte(value);
    _buffer[8 + byteCounter] = lowByte(value);
    _bufferSize = _bufferSize + 2;
    byteCounter = byteCounter + 2;
  }
}

void Modbus::create_PDU(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{
  _buffer[2] = highByte(startAddress);
  _buffer[3] = lowByte(startAddress);
  _bufferSize = 4;
  if (_buffer[1] < 5)
  {
    create_Read(numberRegs);
  }
  if (_buffer[1] == 5)
  {
    create_Write_FC5(table, startAddress);
  }
  if (_buffer[1] == 6)
  {
    create_Write_FC6(table, startAddress);
  }
  if (_buffer[1] == 15)
  {
    create_Write_FC15(table, startAddress, numberRegs);
  }
  if (_buffer[1] == 16)
  {
    create_Write_FC16(table, startAddress, numberRegs);
  }
  sendTxBuffer();

}

void Modbus::writeMaserData(ModbusTable *table, int32_t startAddress, int16_t numberRegs)
{
  if ((_buffer[1] == 1) || (_buffer[1] == 2))
  {
    get_FC1(table, startAddress, numberRegs);
    _status = FLPROG_MODBUS_READY;
    return;
  }
  if ((_buffer[1] == 3) || (_buffer[1] == 4))
  {
    get_FC3(table, startAddress, numberRegs);
    _status = FLPROG_MODBUS_READY;
    return;
  }
  _status = FLPROG_MODBUS_READY;
}
