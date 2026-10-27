#include "flprogModbusUtilites.h"

//------------------namespace flprogModus--------------------

uint16_t flprogModus::modbusCalcCRC(uint16_t length, uint8_t bufferArray[], uint8_t offset)
{
  uint16_t crc = 0xFFFF;
  for (uint16_t pos = offset; pos < length; pos++)
  {
    crc ^= (uint16_t)bufferArray[pos];
    for (uint8_t i = 8; i != 0; i--)
    {
      if ((crc & 0x0001) != 0)
      {
        crc >>= 1;
        crc ^= 0xA001;
      }
      else
      {
        crc >>= 1;
      }
    }
  }
  return crc;
}

uint8_t flprogModus::t35TimeForSpeed(uint16_t portSpeed)
{
  uint16_t speed = portSpeed;
  if (speed > 100)
  {
    speed = flprog::codeFromSpeed(portSpeed);
  }
  switch (speed)
  {
  case FLPROG_SPEED_300:
    return 100;
    break;
  case FLPROG_SPEED_600:
    return 75;
    break;
  case FLPROG_SPEED_1200:
    return 40;
    break;
  case FLPROG_SPEED_2400:
    return 25;
    break;
  case FLPROG_SPEED_4800:
    return 15;
    break;
  case FLPROG_SPEED_9600:
    return 10;
    break;
  case FLPROG_SPEED_14400:
    return 5;
    break;
  case FLPROG_SPEED_19200:
    return 5;
    break;
  case FLPROG_SPEED_28800:
    return 5;
    break;
  case FLPROG_SPEED_38400:
    return 5;
    break;
  default:
    return 5;
    break;
  }
}

uint16_t flprogModus::timeForSendBytes(uint8_t portDataBits, uint8_t portStopBits, uint8_t portParity, uint16_t portSpeed, uint16_t dataSize)
{
  uint16_t speed = portSpeed;
  uint8_t temp = 1 + portDataBits + portStopBits;
  if (speed < 100)
  {
    speed = flprog::speedFromCode(portSpeed);
  }
  if (portParity > 0)
  {
    temp = temp + 1;
  }
  float temp1 = (temp / portDataBits);

  temp1 = temp1 * speed;
  temp1 = 1000 / temp1;
  return (ceil(temp1 * (dataSize + 8) * 8));
}

bool flprogModus::checkCRCOnBuffer(uint16_t length, uint8_t bufferArray[])
{
  uint16_t msgCRC = word(bufferArray[length - 1], bufferArray[length - 2]);
  return flprogModus::modbusCalcCRC((length - 2), bufferArray) == msgCRC;
}

uint16_t flprogModus::slaveRTUPacadgeSize(uint16_t length, uint8_t bufferArray[])
{
  if (length < 2)
  {
    return 0;
  }
  uint8_t function = bufferArray[1];
  if ((function & 128) != 0)
  {
    return 5;
  }
  if ((function > 6) && (function < 15))
  {
    return 0;
  }
  if (function > 16)
  {
    return 0;
  }
  if (function < 15)
  {
    return 8;
  }
  if (length < 7)
  {
    return 0;
  }
  return bufferArray[6] + 9;
}

uint16_t flprogModus::masterRTUPacadgeSize(uint16_t length, uint8_t bufferArray[])
{
  if (length < 2)
  {
    return 0;
  }
  uint8_t function = bufferArray[1];
  if ((function & 128) != 0)
  {
    return 5;
  }
  if ((function > 6) && (function < 15))
  {
    return 0;
  }
  if (function > 16)
  {
    return 0;
  }
  if (function > 4)
  {
    return 8;
  }
  if (length < 3)
  {
    return 0;
  }
  return bufferArray[2] + 5;
}
