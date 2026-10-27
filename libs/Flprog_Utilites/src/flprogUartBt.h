#pragma once
#include <Arduino.h>
#define RT_HW_ESP32_PERMIT_BLE
#include "flprogUartBase.h"

#ifdef RT_HW_CORE_ESP32S
#define FLPROG_BLUETOOTH_SERIAL
#include "BluetoothSerial.h"
struct FLProgBluetoothSerialStruct
{
    String name = "";
    String parentName = "";
    bool isMaster = false;
    BluetoothSerial *port = 0;
    uint8_t status = 0;
};

static FLProgBluetoothSerialStruct flprogBluetoothUart;
#endif

class FlprogBtUartExecutor : public FlprogAbstractUartExecutor
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
    virtual void setBluetoothName(String name);
    virtual void setBluetoothTargetName(String name);
    virtual void setBluetoothIsMasterName(bool isMaster);
};
