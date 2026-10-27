/*
 * FLProg_Network — flprogSocketBackends.h
 * Назначение: Адаптеры socket-операций к конкретным transport backend.
 *
 * Архитектурные заметки:
 * - Маршрутизация выбирается A3, а не legacy-классом клиента.
 * - Backend обязан одинаково трактовать open/close/read/write и ошибки для верхнего socket manager.
 * - Compile-time profile должен исключать недоступные backend-ы из линковки.
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
#include <IPAddress.h>
#include "../flprogNetworkProfile.h"
#include "flprogNetworkSocket.h"
#include "../hardware/flprogWiznetHardware.h"

// -----------------------------------------------------------------------------
// FLProg_Network v0.5.2 - transport backend foundations.
//
// This file contains two deliberately different backend families:
//
// 1. FLProgWiznetSocketBackend
//    Directly drives a leased native Wiznet hardware socket through
//    FLProgWiznetHardware. The implementation is non-blocking: OPEN/CLOSE/SEND
//    progress is observed in pool() and no wait-loop is used.
//
// 2. FLProgLwipSocketBackend
//    A callback adapter for software-stack sockets (Wi-Fi/RMII). It does not
//    change a global default interface. Platform-specific code can bind the
//    callbacks to the exact lwIP/Client implementation appropriate for the
//    selected interface.
//
// UDP packet framing and MACRAW live in dedicated protocol backends. v0.8.4
// keeps TCP CLIENT non-blocking and adds TCP SERVER LISTEN/accepted-session
// support while reusing the same stream I/O and close state machines.
// -----------------------------------------------------------------------------

#define FLPROG_WIZNET_SN_MR      0x0000
#define FLPROG_WIZNET_SN_CR      0x0001
#define FLPROG_WIZNET_SN_IR      0x0002
#define FLPROG_WIZNET_SN_SR      0x0003
#define FLPROG_WIZNET_SN_PORT    0x0004
#define FLPROG_WIZNET_SN_DIPR    0x000C
#define FLPROG_WIZNET_SN_DPORT   0x0010
#define FLPROG_WIZNET_SN_TX_FSR  0x0020
#define FLPROG_WIZNET_SN_TX_WR   0x0024
#define FLPROG_WIZNET_SN_RX_RSR  0x0026
#define FLPROG_WIZNET_SN_RX_RD   0x0028
#define FLPROG_WIZNET_SN_KPALVTR_W5500  0x002F
#define FLPROG_WIZNET_SN_KPALVTR_W5100S 0x0030

#define FLPROG_WIZNET_CMD_OPEN    0x01
#define FLPROG_WIZNET_CMD_LISTEN  0x02
#define FLPROG_WIZNET_CMD_CONNECT 0x04
#define FLPROG_WIZNET_CMD_DISCON  0x08
#define FLPROG_WIZNET_CMD_CLOSE   0x10
#define FLPROG_WIZNET_CMD_SEND      0x20
#define FLPROG_WIZNET_CMD_SEND_KEEP 0x22
#define FLPROG_WIZNET_CMD_RECV      0x40

#define FLPROG_WIZNET_IR_SEND_OK  0x10
#define FLPROG_WIZNET_IR_TIMEOUT  0x08
#define FLPROG_WIZNET_IR_RECV     0x04
#define FLPROG_WIZNET_IR_DISCON   0x02
#define FLPROG_WIZNET_IR_CONNECT  0x01

#define FLPROG_WIZNET_SOCK_CLOSED      0x00
#define FLPROG_WIZNET_SOCK_INIT        0x13
#define FLPROG_WIZNET_SOCK_LISTEN      0x14
#define FLPROG_WIZNET_SOCK_SYNSENT     0x15
#define FLPROG_WIZNET_SOCK_SYNRECV     0x16
#define FLPROG_WIZNET_SOCK_ESTABLISHED 0x17
#define FLPROG_WIZNET_SOCK_CLOSE_WAIT  0x1C
#define FLPROG_WIZNET_SOCK_UDP         0x22

#define FLPROG_WIZNET_MODE_TCP 0x01
#define FLPROG_WIZNET_MODE_UDP 0x02
#define FLPROG_WIZNET_MODE_ND  0x20

// Accepted WIZNET server sockets may otherwise remain in FIN/TIME_WAIT for
// many seconds after a browser aborts a refresh. Bound the graceful phase;
// outbound/client sockets keep their historical close behavior.
#ifndef FLPROG_WIZNET_SERVER_GRACEFUL_CLOSE_MS
#define FLPROG_WIZNET_SERVER_GRACEFUL_CLOSE_MS 5000UL
#endif

#define FLPROG_TCP_BACKEND_EVENT_NONE            0
#define FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED  1
#define FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT 2
#define FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED     3
#define FLPROG_TCP_BACKEND_EVENT_IO_ERROR        4
#define FLPROG_TCP_BACKEND_EVENT_IO_TIMEOUT      5

class FLProgWiznetSocketBackend : public FLProgSocketBackend
{
public:
    FLProgWiznetSocketBackend() {}
    explicit FLProgWiznetSocketBackend(FLProgWiznetHardware &hardware)
        : _hardware(&hardware)
    {
    }

    uint8_t transportId() const override
    {
        return FLPROG_TRANSPORT_WIZNET;
    }

    void hardware(FLProgWiznetHardware &value)
    {
        _hardware = &value;
    }

    FLProgWiznetHardware *hardware() { return _hardware; }
    const FLProgWiznetHardware *hardware() const { return _hardware; }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;

    bool isOpen() const override;
    bool connected() const override;

    int available() override;
    int read() override;
    int read(uint8_t *buffer, size_t length) override;
    int peek() override;

    size_t write(uint8_t value) override
    {
        return write(&value, 1);
    }

    size_t write(const uint8_t *buffer, size_t length) override;
    void flush() override {}

    uint16_t localPort() const override
    {
        return _effectiveLocalPort != 0
            ? _effectiveLocalPort
            : _endpoint.localPort;
    }
    IPAddress remoteIP() const override { return _endpoint.remoteIP; }
    uint16_t remotePort() const override { return _endpoint.remotePort; }

    uint8_t socketStatus() const { return _socketStatus; }
    uint8_t interruptStatus() const { return _interruptStatus; }
    bool sendPending() const { return _sendPending; }
    uint32_t deferredCount() const { return _deferredCount; }

    // TCP server mode: OPEN -> LISTEN. The same socket becomes the accepted
    // client after Sn_SR changes to ESTABLISHED.
    void setServerMode(bool value) { _serverMode = value; }
    bool serverMode() const { return _serverMode; }
    bool listening() const
    {
        return _serverMode && _socketStatus == FLPROG_WIZNET_SOCK_LISTEN;
    }

    // "No delay" on Wiznet means the Sn_MR.ND hardware option: ACK received
    // TCP payload without delayed-ACK waiting. It is not the POSIX Nagle switch.
    void setNoDelayedAck(bool value) { _noDelayedAck = value; }
    bool noDelayedAck() const { return _noDelayedAck; }

    void setKeepAlive(bool enabled, uint32_t intervalMs)
    {
        _keepAliveEnabled = enabled;
        _keepAliveIntervalMs = intervalMs;
    }
    bool keepAliveEnabled() const { return _keepAliveEnabled; }
    uint32_t keepAliveInterval() const { return _keepAliveIntervalMs; }
    bool keepAliveSupported() const;
    bool automaticKeepAliveSupported() const;
    bool manualKeepAliveSupported() const;
    bool keepAliveApplied() const { return _keepAliveApplied; }

    uint8_t lastBackendEvent() const { return _lastBackendEvent; }
    void clearBackendEvent() { _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE; }

private:
    bool validSocket() const;
    bool spiWasDeferred();

    uint8_t read8(uint16_t address, bool &ok);
    uint16_t read16(uint16_t address, bool &ok);
    bool write8(uint16_t address, uint8_t value);
    bool write16(uint16_t address, uint16_t value);
    bool writeBlock(uint16_t address, const uint8_t *buffer, uint16_t length);
    bool readBlock(uint16_t address, uint8_t *buffer, uint16_t length);

    bool issueCommand(uint8_t command);
    bool commandFinished(bool &ok);

    bool configureOpen();
    bool captureServerPeer();
    bool configureTcpOptions();
    bool pollOpen();
    void pollInterrupts();
    void keepAliveStep();

    int readStream(uint8_t *buffer, size_t length, bool advance);

    FLProgWiznetHardware *_hardware = nullptr;

    FLProgSocketEndpoint _endpoint;

    int16_t _socket = FLPROG_SOCKET_NATIVE_NONE;
    uint8_t _protocol = FLPROG_SOCKET_PROTOCOL_NONE;

    uint8_t _socketStatus = FLPROG_WIZNET_SOCK_CLOSED;
    uint8_t _interruptStatus = 0;

    bool _openConfigured = false;
    bool _openCommandSent = false;
    bool _listenCommandSent = false;
    bool _connectCommandSent = false;
    bool _closeCommandSent = false;
    bool _disconnectCommandSent = false;
    uint32_t _disconnectStartedAt = 0;
    bool _sendPrepared = false;
    bool _sendPending = false;
    bool _recvCommandPending = false;
    bool _timeoutSeen = false;
    bool _everConnected = false;
    bool _serverMode = false;
    bool _serverPeerCaptured = false;

    bool _noDelayedAck = false;
    bool _keepAliveEnabled = false;
    bool _keepAliveApplied = false;
    uint32_t _keepAliveIntervalMs = 30000UL;
    uint32_t _lastKeepAliveAt = 0;

    uint8_t _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;

    uint16_t _effectiveLocalPort = 0;

    uint32_t _timeout = 0;
    uint32_t _started = 0;
    uint32_t _deferredCount = 0;
};

// -----------------------------------------------------------------------------
// Software-stack callback backend (Wi-Fi / RMII foundation).
// -----------------------------------------------------------------------------

typedef uint8_t (*FLProgLwipOpenCallback)(
    void *context,
    uint8_t transportId,
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout);

typedef uint8_t (*FLProgLwipPoolCallback)(void *context);
typedef uint8_t (*FLProgLwipCloseCallback)(void *context);
typedef bool (*FLProgLwipBoolCallback)(void *context);
typedef int (*FLProgLwipIntCallback)(void *context);
typedef int (*FLProgLwipReadCallback)(void *context, uint8_t *buffer, size_t length);
typedef size_t (*FLProgLwipWriteCallback)(void *context, const uint8_t *buffer, size_t length);
typedef void (*FLProgLwipVoidCallback)(void *context);
typedef uint16_t (*FLProgLwipPortCallback)(void *context);
typedef IPAddress (*FLProgLwipIpCallback)(void *context);

struct FLProgLwipSocketCallbacks
{
    FLProgLwipOpenCallback open = nullptr;
    FLProgLwipPoolCallback pool = nullptr;
    FLProgLwipCloseCallback close = nullptr;

    FLProgLwipBoolCallback isOpen = nullptr;
    FLProgLwipBoolCallback connected = nullptr;

    FLProgLwipIntCallback available = nullptr;
    FLProgLwipIntCallback readOne = nullptr;
    FLProgLwipReadCallback read = nullptr;
    FLProgLwipIntCallback peek = nullptr;

    FLProgLwipWriteCallback write = nullptr;
    FLProgLwipVoidCallback flush = nullptr;

    FLProgLwipPortCallback localPort = nullptr;
    FLProgLwipIpCallback remoteIP = nullptr;
    FLProgLwipPortCallback remotePort = nullptr;
};

class FLProgLwipSocketBackend : public FLProgSocketBackend
{
public:
    FLProgLwipSocketBackend() {}

    explicit FLProgLwipSocketBackend(uint8_t transportId)
        : _transportId(transportId)
    {
    }

    void configure(
        uint8_t transportId,
        void *context,
        const FLProgLwipSocketCallbacks &callbacks)
    {
        _transportId = transportId;
        _context = context;
        _callbacks = callbacks;
    }

    uint8_t transportId() const override { return _transportId; }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;

    bool isOpen() const override;
    bool connected() const override;

    int available() override;
    int read() override;
    int read(uint8_t *buffer, size_t length) override;
    int peek() override;

    size_t write(uint8_t value) override
    {
        return write(&value, 1);
    }

    size_t write(const uint8_t *buffer, size_t length) override;
    void flush() override;

    uint16_t localPort() const override;
    IPAddress remoteIP() const override;
    uint16_t remotePort() const override;

private:
    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    void *_context = nullptr;
    FLProgLwipSocketCallbacks _callbacks;

    FLProgSocketEndpoint _endpoint;
};
