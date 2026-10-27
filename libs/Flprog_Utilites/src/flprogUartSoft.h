#pragma once
#include <Arduino.h>

#include "flprogUartBase.h"

#ifdef ARDUINO_ARCH_ESP8266
#define FLPROG_SOFTWARE_SERIAL
#endif

#ifdef ARDUINO_ARCH_AVR
#define FLPROG_SOFTWARE_SERIAL
#endif

#ifdef FLPROG_SOFTWARE_SERIAL
#include <SoftwareSerial.h>
struct FLProgSoftSerialStruct
{
    uint32_t speed = 115200;
    uint8_t status = 0;
    uint8_t rx = 4;
    uint8_t tx = 0;
    SoftwareSerial *port = 0;
};

static FLProgSoftSerialStruct flprogSoftUart;
#endif

class FlprogSoftUartExecutor : public FlprogAbstractUartExecutor
{
public:
    virtual void beginUart(uint8_t number) override;
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
