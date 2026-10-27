#include "flprogUart.h"

void FlprogUartExecutor::beginUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    RT_HW_Base.uartBegin(number);
}

void FlprogUartExecutor::endUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    RT_HW_Base.uartEnd(number);
}

int FlprogUartExecutor::availableUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 0;
    }
    if (getStatusUart(number) == 0)
    {
        beginUart(number);
        return 0;
    }
    return RT_HW_Base.uartAvailable(number);
}

uint8_t FlprogUartExecutor::readUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 0;
    }
    if (getStatusUart(number) == 0)
    {
        beginUart(number);
        return 0;
    }
    return RT_HW_Base.uartRead(number);
}

void FlprogUartExecutor::writeUart(uint8_t val, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getStatusUart(number) == 0)
    {
        beginUart(number);
        return;
    }
    RT_HW_Base.uartWrite(val, number);
}

void FlprogUartExecutor::printUart(String val, uint8_t number)
{
    for (uint16_t i = 0; i < val.length(); i++)
    {
        writeUart(((uint8_t)val.charAt(i)), number);
    }
}

void FlprogUartExecutor::writeUart(uint8_t *buffer, uint16_t size, uint8_t number)
{
    if (getStatusUart(number) == 0)
    {
        beginUart(number);
        return;
    }
    for (uint16_t i = 0; i < size; i++)
    {
        writeUart(buffer[i], number);
    }
}

void FlprogUartExecutor::setSpeedUart(uint32_t speed, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    uint32_t newSpeed;
    if (speed < 50)
    {
        newSpeed = flprog::speedFromCode(speed);
    }
    else
    {
        newSpeed = speed;
    }
    if (getSpeedUart(number) == newSpeed)
    {
        return;
    }
    RT_HW_Base.uartSetSpeed(newSpeed, number);
    endUart(number);
}

void FlprogUartExecutor::setDataBitUart(uint8_t value, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getDataBitUart(number) == value)
    {
        return;
    }
    RT_HW_Base.uartSetDataBit(value, number);
    endUart(number);
}

void FlprogUartExecutor::setStopBitUart(uint8_t value, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getStopBitUart(number) == value)
    {
        return;
    }
    RT_HW_Base.uartSetStopBit(value, number);

    endUart(number);
}

void FlprogUartExecutor::setParityUart(uint8_t value, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getParityUart(number) == value)
    {
        return;
    }
    RT_HW_Base.uartSetParity(value, number);
    endUart(number);
}

void FlprogUartExecutor::setPinRxUart(uint8_t pin, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getPinRxUart(number) == pin)
    {
        return;
    }
    RT_HW_Base.uartSetPinRX(pin, number);
    endUart(number);
}

void FlprogUartExecutor::setPinTxUart(uint8_t pin, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getPinTxUart(number) == pin)
    {
        return;
    }
    RT_HW_Base.uartSetPinTX(pin, number);
    endUart(number);
}

void FlprogUartExecutor::setPinsUart(uint8_t pinRx, uint8_t pinTx, uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return;
    }
    if (getPinRxUart(number) == pinRx)
    {
        if (getPinTxUart(number) == pinTx)
        {
            return;
        }
    }
    RT_HW_Base.uartSetPins(pinRx, pinTx, number);
    endUart(number);
}

uint32_t FlprogUartExecutor::getSpeedUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 115200;
    }
    return RT_HW_Base.uartGetSpeed(number);
}

uint8_t FlprogUartExecutor::getDataBitUart(uint8_t number)
{
    return RT_HW_Base.uartGetDataBit(number);
}

uint8_t FlprogUartExecutor::getStopBitUart(uint8_t number)
{
    return RT_HW_Base.uartGetStopBit(number);
}

uint8_t FlprogUartExecutor::getParityUart(uint8_t number)
{
    return RT_HW_Base.uartGetParity(number);
}

uint8_t FlprogUartExecutor::getPinRxUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 0;
    }
    return RT_HW_Base.uartGetPinRX(number);
}

uint8_t FlprogUartExecutor::getPinTxUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 0;
    }
    return RT_HW_Base.uartGetPinTX(number);
}

uint8_t FlprogUartExecutor::getStatusUart(uint8_t number)
{
    if ((number == 100) || (number == 200))
    {
        return 0;
    }
    return RT_HW_Base.uartGetStatus(number);
}
