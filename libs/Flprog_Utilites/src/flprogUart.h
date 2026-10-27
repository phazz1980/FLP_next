#pragma once
#include <Arduino.h>
#include "flprogUartBase.h"

#define FLPROG_BASE_SERIAL

class FlprogUartExecutor : public FlprogAbstractUartExecutor
{
public:
    virtual void beginUart(uint8_t number);
    virtual void endUart(uint8_t number);
    virtual int availableUart(uint8_t number);
    virtual uint8_t readUart(uint8_t number);
    virtual void writeUart(uint8_t val, uint8_t number);
    virtual void printUart(String val, uint8_t number);
    virtual void writeUart(uint8_t *buffer, uint16_t size, uint8_t number);
    virtual uint32_t getSpeedUart(uint8_t number);
    virtual void setSpeedUart(uint32_t speed, uint8_t number);
    virtual uint8_t getStatusUart(uint8_t number);
    virtual uint8_t getDataBitUart(uint8_t number);
    virtual void setDataBitUart(uint8_t value, uint8_t number);
    virtual uint8_t getStopBitUart(uint8_t number);
    virtual void setStopBitUart(uint8_t value, uint8_t number);
    virtual uint8_t getParityUart(uint8_t number);
    virtual void setParityUart(uint8_t value, uint8_t number);
    virtual uint8_t getPinRxUart(uint8_t number);
    virtual void setPinRxUart(uint8_t pin, uint8_t number);
    virtual uint8_t getPinTxUart(uint8_t number);
    virtual void setPinTxUart(uint8_t pin, uint8_t number);
    virtual void setPinsUart(uint8_t pinRx, uint8_t pinTx, uint8_t number);
};
