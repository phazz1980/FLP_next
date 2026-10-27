/*
 * FLProg_Network — flprogStm32TcpBackend.h
 * Назначение: STM32-specific TCP backend adapter.
 *
 * Архитектурные заметки:
 * - Изолирует различия STM32 networking core от native TCP API.
 * - Компиляция STM32 проверяется отдельно от аппаратного W5500 gate.
 * - Верхний слой не должен содержать условные STM32 ветки.
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
#include "../flprogNetworkPlatform.h"
#include "../socket/flprogNetworkSocket.h"
#include "../socket/flprogSocketBackends.h"

#ifndef FLPROG_TCP_SERVER_MAX_CLIENTS
#define FLPROG_TCP_SERVER_MAX_CLIENTS 8
#endif

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
#include <STM32Ethernet.h>
#include <utility/stm32_eth.h>
#include <lwip/mem.h>
#include <lwip/tcp.h>
#include <lwip/ip_addr.h>
#endif

// -----------------------------------------------------------------------------
// STM32 RMII TCP backend.
//
// STM32Ethernet intentionally builds lwIP with LWIP_SOCKET=0 on its normal
// Arduino configuration. Using EthernetClient::connect() would also introduce a
// blocking wait loop. FLProg_Network therefore uses the same raw tcp_pcb layer
// already exposed by STM32Ethernet, but advances all connection state from
// Network.pool(). No new public FLProg API is introduced by this adapter.
// -----------------------------------------------------------------------------

#if FLPROG_NETWORK_HAS_STM32_ETHERNET

class FLProgStm32TcpClientBackend : public FLProgSocketBackend
{
public:
    FLProgStm32TcpClientBackend() {}
    ~FLProgStm32TcpClientBackend() override { resetConnection(); }

    uint8_t transportId() const override { return FLPROG_TRANSPORT_RMII; }

    void setLocalAddress(IPAddress value) { _localAddress = value; }
    void setNoDelay(bool value);
    bool noDelay() const { return _noDelay; }

    void setKeepAlive(
        bool enabled,
        uint32_t idleSeconds,
        uint32_t intervalSeconds,
        uint8_t probeCount);
    bool keepAliveEnabled() const { return _keepAliveEnabled; }
    bool keepAliveApplied() const { return _keepAliveApplied; }

    bool adoptAccepted(
        tcp_struct *client,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort);

    // Used only when a server must reject a connection that it already accepted.
    static void destroyAccepted(tcp_struct *client);

    uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout) override;

    uint8_t pool() override;
    uint8_t close() override;

    bool isOpen() const override { return _connected; }
    bool connected() const override { return _connected; }

    int available() override;
    int read() override;
    int read(uint8_t *buffer, size_t length) override;
    int peek() override;
    size_t write(uint8_t value) override { return write(&value, 1); }
    size_t write(const uint8_t *buffer, size_t length) override;
    void flush() override;

    uint16_t localPort() const override { return _localPort; }
    IPAddress remoteIP() const override { return _endpoint.remoteIP; }
    uint16_t remotePort() const override { return _endpoint.remotePort; }

    bool connecting() const { return _connecting; }
    int lastNativeError() const { return _lastNativeError; }
    int lastOptionError() const { return _lastOptionError; }
    uint8_t lastBackendEvent() const { return _lastBackendEvent; }
    void clearBackendEvent() { _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE; }

private:
    static bool ipIsZero(IPAddress ip);
    static void ipToLwip(IPAddress ip, ip_addr_t &out);
    void resetConnection();
    void applyTcpOptions();

    IPAddress _localAddress = IPAddress(0, 0, 0, 0);
    FLProgSocketEndpoint _endpoint;
    tcp_struct *_tcp = nullptr;

    bool _connecting = false;
    bool _connected = false;
    uint16_t _localPort = 0;
    uint32_t _timeout = 0;
    uint32_t _started = 0;
    int _lastNativeError = 0;
    int _lastOptionError = 0;
    uint8_t _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;

    bool _noDelay = false;
    bool _keepAliveEnabled = false;
    bool _keepAliveApplied = false;
    uint32_t _keepAliveIdleSeconds = 60;
    uint32_t _keepAliveIntervalSeconds = 10;
    uint8_t _keepAliveProbeCount = 3;
};

class FLProgStm32TcpServerListener
{
public:
    FLProgStm32TcpServerListener();
    ~FLProgStm32TcpServerListener() { close(); }

    bool begin(IPAddress localIp, uint16_t port, uint8_t backlog);
    void close();

    bool listening() const { return _listener != nullptr; }
    uint16_t localPort() const { return _localPort; }
    int lastNativeError() const { return _lastNativeError; }

    // 1 = accepted, 0 = none waiting, -1 = listener error.
    int acceptOne(
        tcp_struct *&client,
        IPAddress &remoteIp,
        uint16_t &remotePort);

private:
    static err_t acceptCallback(void *arg, tcp_pcb *newpcb, err_t err);
    static IPAddress lwipToIp(const ip_addr_t &ip);
    void clearPending();

    tcp_pcb *_listener = nullptr;
    tcp_struct *_pending[FLPROG_TCP_SERVER_MAX_CLIENTS];
    uint16_t _localPort = 0;
    int _lastNativeError = 0;
};

#endif // FLPROG_NETWORK_HAS_STM32_ETHERNET
