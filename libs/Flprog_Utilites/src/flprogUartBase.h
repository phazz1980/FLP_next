#pragma once
#include <Arduino.h>
#include "flprogUtilites.h"

struct FLProgUartParametrs
{
    uint32_t speed = 115200;
    uint8_t dataBit = 8;
    uint8_t parity = 0;
    uint8_t stopBit = 1;
    uint8_t number = 255;
};

namespace flprog
{
    uint32_t speedFromCode(uint8_t code);
    uint8_t codeFromSpeed(int32_t speed);
    int serialCodeForParametrs(byte portDataBits, byte portStopBits, byte portParity);
};

class FlprogAbstractUartExecutor
{
public:
    virtual void beginUart(uint8_t number) = 0;
    virtual void endUart(uint8_t number) = 0;
    virtual int availableUart(uint8_t number) = 0;
    virtual uint8_t readUart(uint8_t number) = 0;
    virtual void writeUart(uint8_t val, uint8_t number) = 0;
    virtual void printUart(String val, uint8_t number) = 0;
    virtual void writeUart(uint8_t *buffer, uint16_t size, uint8_t number) = 0;
    virtual uint32_t getSpeedUart(uint8_t number) = 0;
    virtual void setSpeedUart(uint32_t speed, uint8_t number) = 0;
    virtual uint8_t getStatusUart(uint8_t number) = 0;
    virtual uint8_t getDataBitUart(uint8_t number) = 0;
    virtual void setDataBitUart(uint8_t value, uint8_t number) = 0;
    virtual uint8_t getStopBitUart(uint8_t number) = 0;
    virtual void setStopBitUart(uint8_t value, uint8_t number) = 0;
    virtual uint8_t getParityUart(uint8_t number);
    virtual void setParityUart(uint8_t value, uint8_t number) = 0;
    virtual uint8_t getPinRxUart(uint8_t number);
    virtual void setPinRxUart(uint8_t pin, uint8_t number) = 0;
    virtual uint8_t getPinTxUart(uint8_t number) = 0;
    virtual void setPinTxUart(uint8_t pin, uint8_t number) = 0;
    virtual void setPinsUart(uint8_t pinRx, uint8_t pinTx, uint8_t number) = 0;
    virtual void setBluetoothName(String name) { (void)name; };
    virtual void setBluetoothTargetName(String name) { (void)name; };
    virtual void setBluetoothIsMasterName(bool isMaster) { (void)isMaster; };
};
