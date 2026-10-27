/*
 * FLProg_Network — flprogNetworkUdp.h
 * Назначение: Native UDP transport FLProg_Network.
 *
 * Архитектурные заметки:
 * - Поддерживает unicast/multicast и пакетную семантику beginPacket/write/endPacket.
 * - Маршрут и socket lifecycle делегируются A3; legacy UDP adapter не должен обходить этот слой.
 * - Multicast MAC/helper код должен компилироваться только там, где он реально нужен.
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
#include "../flprogNetworkPlatform.h"
#include "../socket/flprogNetworkSocket.h"
#include "../hardware/flprogWiznetHardware.h"
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
#include <ESP8266WiFi.h>
#include <WiFiUdp.h>
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
#include <STM32Ethernet.h>
#include <EthernetUdp.h>
#endif

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// FLProg_Network v0.6.2 - UDP Packet Layer + Multicast + DNS hostname send.
//
// API model intentionally follows the familiar Arduino UDP packet lifecycle:
//   begin() / stop()
//   parsePacket()
//   available() / read() / peek() / flush()
//   beginPacket() / write() / endPacket()
//   remoteIP() / remotePort()
//
// FLProgNetworkUdp is still managed by FLProgNetworkInterface::pool(), so no
// additional udp.pool() call is required in a user loop.
//
// Backends:
// - FLProgWiznetUdpBackend: direct W5100/W5100S/W5200/W5500 hardware socket.
// - FLProgPosixUdpBackend: ESP32 POSIX/lwIP; ESP8266 WiFiUDP adapter.
// - FLProgArduinoUdpBackend<T>: optional adapter for any Arduino UDP-compatible
//   object, useful on platforms such as RP2040/Pico W.
//
// v0.6.1 added route-aware IPv4 multicast. MACRAW is implemented by the
// separate FLProgNetworkMacRaw object. v0.6.2 adds non-blocking hostname
// resolution through the Network Manager default FLProgNetworkDns resolver.
// -----------------------------------------------------------------------------

#ifndef FLPROG_UDP_MAX_PACKET_SIZE
#define FLPROG_UDP_MAX_PACKET_SIZE 1472
#endif

#ifndef FLPROG_UDP_RESULT_SUCCESS
#define FLPROG_UDP_RESULT_SUCCESS 1
#endif
#ifndef FLPROG_UDP_RESULT_ERROR
#define FLPROG_UDP_RESULT_ERROR 0
#endif

// Wiznet socket registers/commands. Keep guards because flprogSocketBackends.h
// already defines the same family-wide offsets.
#ifndef FLPROG_WIZNET_SN_MR
#define FLPROG_WIZNET_SN_MR      0x0000
#endif
#ifndef FLPROG_WIZNET_SN_CR
#define FLPROG_WIZNET_SN_CR      0x0001
#endif
#ifndef FLPROG_WIZNET_SN_IR
#define FLPROG_WIZNET_SN_IR      0x0002
#endif
#ifndef FLPROG_WIZNET_SN_SR
#define FLPROG_WIZNET_SN_SR      0x0003
#endif
#ifndef FLPROG_WIZNET_SN_PORT
#define FLPROG_WIZNET_SN_PORT    0x0004
#endif
#ifndef FLPROG_WIZNET_SN_DHAR
#define FLPROG_WIZNET_SN_DHAR    0x0006
#endif
#ifndef FLPROG_WIZNET_SN_DIPR
#define FLPROG_WIZNET_SN_DIPR    0x000C
#endif
#ifndef FLPROG_WIZNET_SN_DPORT
#define FLPROG_WIZNET_SN_DPORT   0x0010
#endif
#ifndef FLPROG_WIZNET_SN_TX_FSR
#define FLPROG_WIZNET_SN_TX_FSR  0x0020
#endif
#ifndef FLPROG_WIZNET_SN_TX_WR
#define FLPROG_WIZNET_SN_TX_WR   0x0024
#endif
#ifndef FLPROG_WIZNET_SN_RX_RSR
#define FLPROG_WIZNET_SN_RX_RSR  0x0026
#endif
#ifndef FLPROG_WIZNET_SN_RX_RD
#define FLPROG_WIZNET_SN_RX_RD   0x0028
#endif

#ifndef FLPROG_WIZNET_CMD_OPEN
#define FLPROG_WIZNET_CMD_OPEN   0x01
#endif
#ifndef FLPROG_WIZNET_CMD_CLOSE
#define FLPROG_WIZNET_CMD_CLOSE  0x10
#endif
#ifndef FLPROG_WIZNET_CMD_SEND
#define FLPROG_WIZNET_CMD_SEND   0x20
#endif
#ifndef FLPROG_WIZNET_CMD_RECV
#define FLPROG_WIZNET_CMD_RECV   0x40
#endif

#ifndef FLPROG_WIZNET_IR_SEND_OK
#define FLPROG_WIZNET_IR_SEND_OK 0x10
#endif
#ifndef FLPROG_WIZNET_IR_TIMEOUT
#define FLPROG_WIZNET_IR_TIMEOUT 0x08
#endif

#ifndef FLPROG_WIZNET_SOCK_CLOSED
#define FLPROG_WIZNET_SOCK_CLOSED 0x00
#endif
#ifndef FLPROG_WIZNET_SOCK_UDP
#define FLPROG_WIZNET_SOCK_UDP    0x22
#endif
#ifndef FLPROG_WIZNET_MODE_UDP
#define FLPROG_WIZNET_MODE_UDP    0x02
#endif

// -----------------------------------------------------------------------------
// UDP-specific backend contract
// -----------------------------------------------------------------------------

class FLProgUdpBackend : public FLProgSocketBackend
{
public:
    virtual ~FLProgUdpBackend() {}

    virtual int beginPacket(IPAddress ip, uint16_t port)
    {
        (void)ip;
        (void)port;
        return 0;
    }

    virtual size_t packetWrite(uint8_t value)
    {
        return packetWrite(&value, 1);
    }

    virtual size_t packetWrite(
        const uint8_t *buffer,
        size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }

    virtual int endPacket() { return 0; }

    virtual int parsePacket() { return 0; }
    virtual int packetAvailable() { return 0; }
    virtual int packetRead() { return -1; }

    virtual int packetRead(
        uint8_t *buffer,
        size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }

    virtual int packetPeek() { return -1; }
    virtual void packetFlush() {}

    virtual IPAddress packetRemoteIP() const
    {
        return IPAddress(0, 0, 0, 0);
    }

    virtual uint16_t packetRemotePort() const { return 0; }
    virtual uint16_t packetSize() const { return 0; }

    virtual void setBroadcastBlocking(bool value)
    {
        (void)value;
    }

    virtual void setLocalAddress(IPAddress value)
    {
        (void)value;
    }

    // Configure IPv4 multicast before socket OPEN. Backends that cannot
    // guarantee route-aware multicast return false.
    virtual bool configureMulticast(
        bool enabled,
        IPAddress group,
        uint16_t port)
    {
        (void)enabled;
        (void)group;
        (void)port;
        return !enabled;
    }

    // Map the common socket API to the current UDP packet.
    int available() override { return packetAvailable(); }
    int read() override { return packetRead(); }

    int read(
        uint8_t *buffer,
        size_t length) override
    {
        return packetRead(buffer, length);
    }

    int peek() override { return packetPeek(); }

    size_t write(uint8_t value) override
    {
        return packetWrite(value);
    }

    size_t write(
        const uint8_t *buffer,
        size_t length) override
    {
        return packetWrite(buffer, length);
    }

    void flush() override { packetFlush(); }

    IPAddress remoteIP() const override
    {
        return packetRemoteIP();
    }

    uint16_t remotePort() const override
    {
        return packetRemotePort();
    }
};

// -----------------------------------------------------------------------------
// Direct Wiznet UDP backend
// -----------------------------------------------------------------------------

class FLProgWiznetUdpBackend : public FLProgUdpBackend
{
public:
    FLProgWiznetUdpBackend() {}

    explicit FLProgWiznetUdpBackend(
        FLProgWiznetHardware &hardware)
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

    void setBroadcastBlocking(bool value) override
    {
        _broadcastBlocking = value;
    }

    bool broadcastBlocking() const
    {
        return _broadcastBlocking;
    }

    bool configureMulticast(
        bool enabled,
        IPAddress group,
        uint16_t port) override
    {
        _multicastEnabled = enabled;
        _multicastGroup = group;
        _multicastPort = port;
        return true;
    }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;

    bool isOpen() const override;
    bool connected() const override { return isOpen(); }

    int beginPacket(IPAddress ip, uint16_t port) override;
    size_t packetWrite(
        const uint8_t *buffer,
        size_t length) override;
    int endPacket() override;

    int parsePacket() override;
    int packetAvailable() override;
    int packetRead() override;
    int packetRead(
        uint8_t *buffer,
        size_t length) override;
    int packetPeek() override;
    void packetFlush() override;

    IPAddress packetRemoteIP() const override
    {
        return _rxRemoteIP;
    }

    uint16_t packetRemotePort() const override
    {
        return _rxRemotePort;
    }

    uint16_t packetSize() const override
    {
        return _rxPacketSize;
    }

    uint16_t localPort() const override
    {
        return _effectiveLocalPort;
    }

    uint8_t socketStatus() const { return _socketStatus; }
    bool sendPending() const { return _sendPending; }
    uint32_t deferredCount() const { return _deferredCount; }

private:
    bool validSocket() const;
    bool spiWasDeferred();

    uint8_t read8(uint16_t address, bool &ok);
    uint16_t read16(uint16_t address, bool &ok);

    bool write8(uint16_t address, uint8_t value);
    bool write16(uint16_t address, uint16_t value);
    bool writeBlock(
        uint16_t address,
        const uint8_t *buffer,
        uint16_t length);

    bool issueCommand(uint8_t command);
    bool commandFinished(bool &ok);

    bool configureOpen();
    bool pollOpen();
    void pollInterrupts();

    bool commitRxPacket();
    int readCurrentPacket(
        uint8_t *buffer,
        size_t length,
        bool advance);

    void resetPacketState();
    void resetSocketState();

    FLProgWiznetHardware *_hardware = nullptr;

    FLProgSocketEndpoint _endpoint;

    int16_t _socket = FLPROG_SOCKET_NATIVE_NONE;
    uint8_t _socketStatus = FLPROG_WIZNET_SOCK_CLOSED;

    bool _broadcastBlocking = false;
    bool _multicastEnabled = false;
    IPAddress _multicastGroup = IPAddress(0, 0, 0, 0);
    uint16_t _multicastPort = 0;

    bool _openConfigured = false;
    bool _openCommandSent = false;
    bool _closeCommandSent = false;

    bool _sendPending = false;
    bool _recvCommandPending = false;

    bool _txActive = false;
    uint16_t _txWritePointer = 0;
    uint16_t _txCapacity = 0;
    uint16_t _txLength = 0;
    IPAddress _txRemoteIP = IPAddress(0, 0, 0, 0);
    uint16_t _txRemotePort = 0;

    bool _rxActive = false;
    uint16_t _rxPacketStart = 0;
    uint16_t _rxReadPointer = 0;
    uint16_t _rxPacketEnd = 0;
    uint16_t _rxPacketSize = 0;
    uint16_t _rxRemaining = 0;
    IPAddress _rxRemoteIP = IPAddress(0, 0, 0, 0);
    uint16_t _rxRemotePort = 0;

    uint16_t _effectiveLocalPort = 0;

    uint32_t _timeout = 0;
    uint32_t _started = 0;
    uint32_t _deferredCount = 0;
};

// -----------------------------------------------------------------------------
// Built-in Wi-Fi UDP backend. ESP32 uses non-blocking POSIX/lwIP sockets;
// ESP8266 core 3.x uses WiFiUDP because LWIP_SOCKET is disabled.
// The backend remains bound to the selected transport's local IPv4 address.
// -----------------------------------------------------------------------------

class FLProgPosixUdpBackend : public FLProgUdpBackend
{
public:
    FLProgPosixUdpBackend() {}

    explicit FLProgPosixUdpBackend(uint8_t transportId)
        : _transportId(transportId)
    {
    }

    ~FLProgPosixUdpBackend() override;

    void configure(uint8_t transportId)
    {
        _transportId = transportId;
    }

    uint8_t transportId() const override
    {
        return _transportId;
    }

    void setLocalAddress(IPAddress value) override
    {
        _localAddress = value;
    }

    IPAddress localAddress() const
    {
        return _localAddress;
    }

    bool configureMulticast(
        bool enabled,
        IPAddress group,
        uint16_t port) override
    {
        _multicastEnabled = enabled;
        _multicastGroup = group;
        _multicastPort = port;
        return true;
    }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;

    bool isOpen() const override
    {
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
        return _esp8266Open;
#else
        return _socketFd >= 0;
#endif
    }

    bool connected() const override
    {
        return isOpen();
    }

    int beginPacket(IPAddress ip, uint16_t port) override;
    size_t packetWrite(
        const uint8_t *buffer,
        size_t length) override;
    int endPacket() override;

    int parsePacket() override;
    int packetAvailable() override;
    int packetRead() override;
    int packetRead(
        uint8_t *buffer,
        size_t length) override;
    int packetPeek() override;
    void packetFlush() override;

    IPAddress packetRemoteIP() const override
    {
        return _rxRemoteIP;
    }

    uint16_t packetRemotePort() const override
    {
        return _rxRemotePort;
    }

    uint16_t packetSize() const override
    {
        return _rxLength;
    }

    uint16_t localPort() const override
    {
        return _localPort;
    }

private:
    bool ensureTxBuffer();
    bool ensureRxBuffer();
    void clearRxPacket();
    void freeBuffers();

    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    int _socketFd = -1;
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    WiFiUDP _esp8266Udp;
    bool _esp8266Open = false;
#endif

    IPAddress _localAddress = IPAddress(0, 0, 0, 0);
    uint16_t _localPort = 0;

    bool _multicastEnabled = false;
    bool _multicastJoined = false;
    IPAddress _multicastGroup = IPAddress(0, 0, 0, 0);
    uint16_t _multicastPort = 0;
    IPAddress _joinedMulticastGroup = IPAddress(0, 0, 0, 0);
    IPAddress _joinedMulticastInterface = IPAddress(0, 0, 0, 0);

    uint8_t *_txBuffer = nullptr;
    size_t _txLength = 0;
    bool _txActive = false;
    IPAddress _txRemoteIP = IPAddress(0, 0, 0, 0);
    uint16_t _txRemotePort = 0;

    uint8_t *_rxBuffer = nullptr;
    size_t _rxLength = 0;
    size_t _rxPosition = 0;
    IPAddress _rxRemoteIP = IPAddress(0, 0, 0, 0);
    uint16_t _rxRemotePort = 0;
};

// -----------------------------------------------------------------------------
// Generic Arduino UDP adapter.
// The caller owns TUdp and is responsible for choosing the correct interface.
// -----------------------------------------------------------------------------

template <typename TUdp>
class FLProgArduinoUdpBackend : public FLProgUdpBackend
{
public:
    FLProgArduinoUdpBackend() {}

    FLProgArduinoUdpBackend(
        uint8_t transportId,
        TUdp &udp)
        : _transportId(transportId),
          _udp(&udp)
    {
    }

    void configure(
        uint8_t transportId,
        TUdp &udp)
    {
        _transportId = transportId;
        _udp = &udp;
    }

    uint8_t transportId() const override
    {
        return _transportId;
    }

    bool configureMulticast(
        bool enabled,
        IPAddress group,
        uint16_t port) override
    {
        (void)group;
        (void)port;
        _multicastRequested = enabled;
        return !enabled;
    }

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override
    {
        (void)lease;
        (void)timeout;

        if (
            _udp == nullptr ||
            _multicastRequested ||
            protocol != FLPROG_SOCKET_PROTOCOL_DATAGRAM
        )
        {
            return FLPROG_SOCKET_RESULT_UNSUPPORTED;
        }

        if (_open)
        {
            return FLPROG_SOCKET_RESULT_SUCCESS;
        }

        _localPort = endpoint.localPort;

        if (_udp->begin(_localPort) == 0)
        {
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        _open = true;
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    uint8_t close() override
    {
        if (_udp != nullptr && _open)
        {
            _udp->stop();
        }

        _open = false;
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    bool isOpen() const override { return _open; }
    bool connected() const override { return _open; }

    int beginPacket(IPAddress ip, uint16_t port) override
    {
        return (_udp != nullptr && _open)
            ? _udp->beginPacket(ip, port)
            : 0;
    }

    size_t packetWrite(
        const uint8_t *buffer,
        size_t length) override
    {
        return (_udp != nullptr && _open)
            ? _udp->write(buffer, length)
            : 0;
    }

    int endPacket() override
    {
        return (_udp != nullptr && _open)
            ? _udp->endPacket()
            : 0;
    }

    int parsePacket() override
    {
        _packetSize =
            (_udp != nullptr && _open)
                ? _udp->parsePacket()
                : 0;

        return _packetSize;
    }

    int packetAvailable() override
    {
        return (_udp != nullptr && _open)
            ? _udp->available()
            : 0;
    }

    int packetRead() override
    {
        return (_udp != nullptr && _open)
            ? _udp->read()
            : -1;
    }

    int packetRead(
        uint8_t *buffer,
        size_t length) override
    {
        return (_udp != nullptr && _open)
            ? _udp->read(buffer, length)
            : 0;
    }

    int packetPeek() override
    {
        return (_udp != nullptr && _open)
            ? _udp->peek()
            : -1;
    }

    void packetFlush() override
    {
        if (_udp != nullptr && _open)
        {
            _udp->flush();
        }
    }

    IPAddress packetRemoteIP() const override
    {
        return (_udp != nullptr && _open)
            ? _udp->remoteIP()
            : IPAddress(0, 0, 0, 0);
    }

    uint16_t packetRemotePort() const override
    {
        return (_udp != nullptr && _open)
            ? _udp->remotePort()
            : 0;
    }

    uint16_t packetSize() const override
    {
        return (uint16_t)_packetSize;
    }

    uint16_t localPort() const override
    {
        return _localPort;
    }

private:
    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    TUdp *_udp = nullptr;

    bool _open = false;
    bool _multicastRequested = false;
    uint16_t _localPort = 0;
    int _packetSize = 0;
};

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
// Cooperative UDP adapter for STM32 native Ethernet MAC/RMII.
// STM32Ethernet's scheduler is advanced from pool(); multicast uses
// EthernetUDP::beginMulticast().
class FLProgStm32UdpBackend : public FLProgUdpBackend
{
public:
    uint8_t transportId() const override { return FLPROG_TRANSPORT_RMII; }
    bool configureMulticast(bool enabled, IPAddress group, uint16_t port) override;
    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;
    uint8_t pool() override;
    uint8_t close() override;
    bool isOpen() const override { return _open; }
    bool connected() const override { return _open; }
    int beginPacket(IPAddress ip, uint16_t port) override;
    size_t packetWrite(const uint8_t *buffer, size_t length) override;
    int endPacket() override;
    int parsePacket() override;
    int packetAvailable() override;
    int packetRead() override;
    int packetRead(uint8_t *buffer, size_t length) override;
    int packetPeek() override;
    void packetFlush() override;
    IPAddress packetRemoteIP() const override;
    uint16_t packetRemotePort() const override;
    uint16_t packetSize() const override { return (uint16_t)_packetSize; }
    uint16_t localPort() const override { return _localPort; }

private:
    mutable EthernetUDP _udp;
    bool _open = false;
    bool _multicastEnabled = false;
    IPAddress _multicastGroup = IPAddress(0, 0, 0, 0);
    uint16_t _multicastPort = 0;
    uint16_t _localPort = 0;
    int _packetSize = 0;
};
#endif

// -----------------------------------------------------------------------------
// User-facing UDP object
// -----------------------------------------------------------------------------

class FLProgNetworkUdp : public FLProgNetworkSocket
{
public:
    FLProgNetworkUdp();

    bool begin(
        FLProgNetworkInterface &network,
        uint8_t route,
        uint16_t localPort);

    bool beginMulticast(
        FLProgNetworkInterface &network,
        uint8_t route,
        IPAddress group,
        uint16_t port);

    bool joinMulticast(IPAddress group, uint16_t port);
    bool leaveMulticast();
    bool multicastEnabled() const { return _multicastEnabled; }
    IPAddress multicastGroup() const { return _multicastGroup; }
    uint16_t multicastPort() const { return _multicastPort; }

    void stop();

    void setUdpBackend(
        uint8_t transportId,
        FLProgUdpBackend *backend);

    FLProgUdpBackend *udpBackend(uint8_t transportId);
    const FLProgUdpBackend *udpBackend(uint8_t transportId) const;

    // Called automatically by Network Manager.
    void pool() override;

    int beginPacket(IPAddress ip, uint16_t port);

    // Non-blocking hostname overload. Returns 0 while DNS is pending; call it
    // again after network.pool() advances the resolver. Once the result is in
    // cache, the call behaves like beginPacket(IPAddress, port).
    int beginPacket(const char *hostname, uint16_t port);
    int beginPacket(const String &hostname, uint16_t port)
    {
        return beginPacket(hostname.c_str(), port);
    }

    size_t write(uint8_t value);
    size_t write(
        const uint8_t *buffer,
        size_t length);
    int endPacket();

    int parsePacket();
    int available();
    int read();
    int read(
        uint8_t *buffer,
        size_t length);
    int peek();
    void flush();

    IPAddress remoteIP() const;
    uint16_t remotePort() const;
    uint16_t packetSize() const;

    void setBroadcastBlocking(bool value)
    {
        _broadcastBlocking = value;
        _broadcastBlockingExplicit = true;
    }

    bool broadcastBlocking() const
    {
        return _broadcastBlocking;
    }

private:
    FLProgUdpBackend *currentUdpBackend();
    const FLProgUdpBackend *currentUdpBackend() const;

    FLProgNetworkInterface *_udpNetwork = nullptr;

    FLProgUdpBackend *_udpBackends[FLPROG_TRANSPORT_COUNT];

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgWiznetUdpBackend _wiznetBackend;
#endif
    FLProgPosixUdpBackend _wifiPosixBackend;
    FLProgPosixUdpBackend _wifiApPosixBackend;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgPosixUdpBackend _rmiiPosixBackend;
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    FLProgStm32UdpBackend _stm32RmiiBackend;
#endif

    bool _broadcastBlocking = false;
    bool _broadcastBlockingExplicit = false;

    bool _multicastEnabled = false;
    IPAddress _multicastGroup = IPAddress(0, 0, 0, 0);
    uint16_t _multicastPort = 0;
};
