/*
 * FLProg_Network — flprogNetworkMacRaw.h
 * Назначение: MACRAW helper для низкоуровневых Wiznet кадров.
 *
 * Архитектурные заметки:
 * - Применяется для L2-задач, недоступных обычному TCP/UDP socket API.
 * - Работает только через wired/Wiznet backend и должен быть profile-guarded.
 * - Не использовать MACRAW как обход A3 для обычного прикладного трафика.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Legacy UDP/DNS API остается фасадом над native route-aware реализацией и не дублирует socket state.
 * - Повторные вызовы должны сохранять неблокирующую семантику SUCCESS/WAIT/ERROR, ожидаемую FLProg.
 * - Кэш, TTL и packet boundaries переводятся в compatibility слое, а не в hardware backend.
 * Extended maintenance contract:
 * - The legacy UDP/DNS API remains a facade over the native route-aware implementation and does not duplicate socket state.
 * - Repeated calls keep the non-blocking SUCCESS/WAIT/ERROR semantics expected by FLProg.
 * - Cache, TTL and packet boundaries are translated in the compatibility layer, not in the hardware backend.
 */
#pragma once

#include <Arduino.h>
#include "../socket/flprogNetworkSocket.h"
#include "../hardware/flprogWiznetHardware.h"

class FLProgNetworkInterface;

// FLProg_Network v0.6.1 - Wiznet MACRAW packet layer.
// MACRAW is intentionally Wiznet-only and always reserves native socket 0.
// MF/MFEN and W5500 BCASTB are independent policies and are applied before OPEN.

#ifndef FLPROG_MACRAW_MAX_FRAME_SIZE
#define FLPROG_MACRAW_MAX_FRAME_SIZE 1514
#endif

#ifndef FLPROG_WIZNET_MACRAW_SN_MR
#define FLPROG_WIZNET_MACRAW_SN_MR      0x0000
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_CR
#define FLPROG_WIZNET_MACRAW_SN_CR      0x0001
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_IR
#define FLPROG_WIZNET_MACRAW_SN_IR      0x0002
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_SR
#define FLPROG_WIZNET_MACRAW_SN_SR      0x0003
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_TX_FSR
#define FLPROG_WIZNET_MACRAW_SN_TX_FSR  0x0020
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_TX_WR
#define FLPROG_WIZNET_MACRAW_SN_TX_WR   0x0024
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_RX_RSR
#define FLPROG_WIZNET_MACRAW_SN_RX_RSR  0x0026
#endif
#ifndef FLPROG_WIZNET_MACRAW_SN_RX_RD
#define FLPROG_WIZNET_MACRAW_SN_RX_RD   0x0028
#endif

#ifndef FLPROG_WIZNET_MACRAW_CMD_OPEN
#define FLPROG_WIZNET_MACRAW_CMD_OPEN   0x01
#endif
#ifndef FLPROG_WIZNET_MACRAW_CMD_CLOSE
#define FLPROG_WIZNET_MACRAW_CMD_CLOSE  0x10
#endif
#ifndef FLPROG_WIZNET_MACRAW_CMD_SEND
#define FLPROG_WIZNET_MACRAW_CMD_SEND   0x20
#endif
#ifndef FLPROG_WIZNET_MACRAW_CMD_RECV
#define FLPROG_WIZNET_MACRAW_CMD_RECV   0x40
#endif

#ifndef FLPROG_WIZNET_MACRAW_IR_SEND_OK
#define FLPROG_WIZNET_MACRAW_IR_SEND_OK 0x10
#endif
#ifndef FLPROG_WIZNET_MACRAW_IR_TIMEOUT
#define FLPROG_WIZNET_MACRAW_IR_TIMEOUT 0x08
#endif

#ifndef FLPROG_WIZNET_SOCK_MACRAW
#define FLPROG_WIZNET_SOCK_MACRAW       0x42
#endif

class FLProgMacRawBackend : public FLProgSocketBackend
{
public:
    virtual int beginFrame() { return 0; }
    virtual size_t frameWrite(uint8_t value) { return frameWrite(&value, 1); }
    virtual size_t frameWrite(const uint8_t *buffer, size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }
    virtual int endFrame() { return 0; }

    virtual int parseFrame() { return 0; }
    virtual int frameAvailable() { return 0; }
    virtual int frameRead() { return -1; }
    virtual int frameRead(uint8_t *buffer, size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }
    virtual int framePeek() { return -1; }
    virtual void frameFlush() {}
    virtual uint16_t frameSize() const { return 0; }

    int available() override { return frameAvailable(); }
    int read() override { return frameRead(); }
    int read(uint8_t *buffer, size_t length) override
    {
        return frameRead(buffer, length);
    }
    int peek() override { return framePeek(); }
    size_t write(uint8_t value) override { return frameWrite(value); }
    size_t write(const uint8_t *buffer, size_t length) override
    {
        return frameWrite(buffer, length);
    }
    void flush() override { frameFlush(); }
};

class FLProgWiznetMacRawBackend : public FLProgMacRawBackend
{
public:
    FLProgWiznetMacRawBackend() {}
    explicit FLProgWiznetMacRawBackend(FLProgWiznetHardware &hardware)
        : _hardware(&hardware) {}

    uint8_t transportId() const override { return FLPROG_TRANSPORT_WIZNET; }

    void hardware(FLProgWiznetHardware &value) { _hardware = &value; }
    void setMacFilterEnabled(bool value) { _macFilterEnabled = value; }
    void setBroadcastBlocking(bool value) { _broadcastBlocking = value; }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;
    bool isOpen() const override;
    bool connected() const override { return isOpen(); }

    int beginFrame() override;
    size_t frameWrite(const uint8_t *buffer, size_t length) override;
    int endFrame() override;

    int parseFrame() override;
    int frameAvailable() override;
    int frameRead() override;
    int frameRead(uint8_t *buffer, size_t length) override;
    int framePeek() override;
    void frameFlush() override;
    uint16_t frameSize() const override { return _rxFrameSize; }

    bool sendPending() const { return _sendPending; }
    uint32_t deferredCount() const { return _deferredCount; }
    uint32_t invalidFrameCount() const { return _invalidFrameCount; }

private:
    bool validSocket() const;
    bool spiWasDeferred();
    uint8_t read8(uint16_t address, bool &ok);
    uint16_t read16(uint16_t address, bool &ok);
    bool write8(uint16_t address, uint8_t value);
    bool write16(uint16_t address, uint16_t value);
    bool issueCommand(uint8_t command);
    bool commandFinished(bool &ok);

    bool configureOpen();
    bool pollOpen();
    void pollInterrupts();
    bool commitRxFrame();
    int readCurrentFrame(uint8_t *buffer, size_t length, bool advance);
    void resetFrameState();
    void resetSocketState();

    FLProgWiznetHardware *_hardware = nullptr;
    int16_t _socket = FLPROG_SOCKET_NATIVE_NONE;
    uint8_t _socketStatus = 0;

    bool _macFilterEnabled = false;
    bool _broadcastBlocking = false;

    bool _openConfigured = false;
    bool _openCommandSent = false;
    bool _closeCommandSent = false;
    bool _sendPending = false;
    bool _recvCommandPending = false;

    bool _txActive = false;
    uint16_t _txWritePointer = 0;
    uint16_t _txCapacity = 0;
    uint16_t _txLength = 0;

    bool _rxActive = false;
    uint16_t _rxFrameStart = 0;
    uint16_t _rxReadPointer = 0;
    uint16_t _rxFrameEnd = 0;
    uint16_t _rxFrameSize = 0;
    uint16_t _rxRemaining = 0;

    uint32_t _timeout = 0;
    uint32_t _started = 0;
    uint32_t _deferredCount = 0;
    uint32_t _invalidFrameCount = 0;
};

class FLProgNetworkMacRaw : public FLProgNetworkSocket
{
public:
    FLProgNetworkMacRaw();

    bool begin(FLProgNetworkInterface &network);
    void stop();

    void setMacFilterEnabled(bool value);
    bool macFilterEnabled() const { return _macFilterEnabled; }

    // BCASTB is meaningful only on W5500 and stays independent from MF/MFEN.
    void setBroadcastBlocking(bool value)
    {
        _broadcastBlocking = value;
        _broadcastBlockingExplicit = true;
        requestRebind();
    }
    bool broadcastBlocking() const { return _broadcastBlocking; }

    void pool() override;

    int beginFrame();
    size_t write(uint8_t value);
    size_t write(const uint8_t *buffer, size_t length);
    int endFrame();

    int parseFrame();
    int available();
    int read();
    int read(uint8_t *buffer, size_t length);
    int peek();
    void flush();
    uint16_t frameSize() const;

    uint32_t invalidFrameCount() const
    {
        return _wiznetBackend.invalidFrameCount();
    }

private:
    FLProgNetworkInterface *_rawNetwork = nullptr;
    FLProgWiznetMacRawBackend _wiznetBackend;

    bool _macFilterEnabled = false;
    bool _broadcastBlocking = false;
    bool _broadcastBlockingExplicit = false;
};
