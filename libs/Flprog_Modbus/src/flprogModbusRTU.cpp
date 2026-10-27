#include "flprogModbusRTU.h"

void ModbusRTU::onPeDePin()
{
  if (_pinPeDe < 0)
  {
    return;
  }
  digitalWrite(_pinPeDe, HIGH);
}

void ModbusRTU::offPeDePin()
{
  if (_pinPeDe < 0)
  {
    return;
  }
  digitalWrite(_pinPeDe, LOW);
}

uint8_t ModbusRTU::rxBuffer()
{
  bool bBuffOverflow = false;
  _bufferSize = 0;
  while (_executor->availableUart(_uartPortNumber))
  {
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      _buffer[_bufferSize] = _executor->readUart(_uartPortNumber);
    }
    else
    {
      _executor->readUart(_uartPortNumber);
      bBuffOverflow = true;
    }
    _bufferSize++;
  }
  if (bBuffOverflow)
  {
    return -3;
  }

  return _bufferSize;
}

void ModbusRTU::sendTxBuffer()
{
  clearRXBuffer();
  if (_buffer[0] == 0)
  {
    _bufferSize = 0;
    return;
  }
  onPeDePin();
  uint16_t crc = flprogModus::modbusCalcCRC(_bufferSize, _buffer);
  _buffer[_bufferSize] = lowByte(crc);
  _bufferSize++;
  _buffer[_bufferSize] = highByte(crc);
  _bufferSize++;
  _executor->writeUart(_buffer, _bufferSize, _uartPortNumber);
  uint8_t dataBits = 8;
  uint8_t stopBits = 1;
  uint8_t portParity = 0;
  uint16_t portSpeed = _executor->getSpeedUart(_uartPortNumber);
  _timeOfSend = flprogModus::timeForSendBytes(dataBits, stopBits, portParity, portSpeed, _bufferSize);
  _startSendTime = millis();
  _status = FLPROG_MODBUS_WAITING_SENDING;
  _bufferSize = 0;
}

bool ModbusRTU::checkAvaliblePacage()
{
  while (_executor->availableUart(_uartPortNumber))
  {
    _time = millis();
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      _buffer[_bufferSize] = _executor->readUart(_uartPortNumber);

      _bufferSize++;
      if (_bufferSize == (rtuPacadgeSize(_bufferSize, _buffer)))
      {
        return true;
      }
    }
  }
  return false;
}

uint16_t ModbusRTU::rtuPacadgeSize(uint16_t length, uint8_t bufferArray[])
{
  (void)length;
  (void)bufferArray;
  return 0;
}

void ModbusRTU::clearRXBuffer()
{
  while (_executor->availableUart(_uartPortNumber))
  {
    _executor->readUart(_uartPortNumber);
  }
}