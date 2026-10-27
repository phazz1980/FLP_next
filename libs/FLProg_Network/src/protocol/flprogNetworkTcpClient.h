/*
 * FLProg_Network — flprogNetworkTcpClient.h
 * Назначение: Native TCP client FLProg_Network.
 *
 * Архитектурные заметки:
 * - Управляет connect/reconnect поверх socket lease и TransportIdentity.
 * - Разрыв удалённого TCP server не равен потере transport: generation остаётся прежней.
 * - Диагностические open/reopen/stale counters используются regression-gates.
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
#include "../socket/flprogSocketBackends.h"
#include "flprogNetworkDns.h"
#include "flprogStm32TcpBackend.h"
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
#include <ESP8266WiFi.h>
#endif

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// FLProg_Network v0.8.4 - hardened non-blocking route-aware TCP client.
//
// The object is registered as a managed socket. DNS, transport selection,
// reconnect/backoff and all socket progress continue only from network.pool().
// -----------------------------------------------------------------------------

#ifndef FLPROG_TCP_HOSTNAME_MAX_LENGTH
#define FLPROG_TCP_HOSTNAME_MAX_LENGTH 127
#endif

#define FLPROG_TCP_CLIENT_ERROR_NONE        0
#define FLPROG_TCP_CLIENT_ERROR_TARGET      1
#define FLPROG_TCP_CLIENT_ERROR_DNS         2
#define FLPROG_TCP_CLIENT_ERROR_NO_RESOLVER 3

#define FLPROG_TCP_DNS_POLICY_SHARED  0
#define FLPROG_TCP_DNS_POLICY_PRIVATE 1

#define FLPROG_TCP_DISCONNECT_NONE            0
#define FLPROG_TCP_DISCONNECT_USER_STOP       1
#define FLPROG_TCP_DISCONNECT_INVALID_TARGET  2
#define FLPROG_TCP_DISCONNECT_DNS             3
#define FLPROG_TCP_DISCONNECT_NO_RESOLVER     4
#define FLPROG_TCP_DISCONNECT_NO_TRANSPORT    5
#define FLPROG_TCP_DISCONNECT_NO_LEASE        6
#define FLPROG_TCP_DISCONNECT_CONNECT_TIMEOUT 7
#define FLPROG_TCP_DISCONNECT_CONNECT_FAILED  8
#define FLPROG_TCP_DISCONNECT_PEER_CLOSED     9
#define FLPROG_TCP_DISCONNECT_IO_ERROR       10
#define FLPROG_TCP_DISCONNECT_IO_TIMEOUT     11
#define FLPROG_TCP_DISCONNECT_STALE          12
#define FLPROG_TCP_DISCONNECT_UNSUPPORTED    13
#define FLPROG_TCP_DISCONNECT_UNKNOWN        14

#ifndef FLPROG_TCP_RECONNECT_INITIAL_MS
#define FLPROG_TCP_RECONNECT_INITIAL_MS 1000UL
#endif

#ifndef FLPROG_TCP_RECONNECT_MAX_MS
#define FLPROG_TCP_RECONNECT_MAX_MS 30000UL
#endif

class FLProgPosixTcpClientBackend : public FLProgSocketBackend
{
public:
    FLProgPosixTcpClientBackend() {}
    explicit FLProgPosixTcpClientBackend(uint8_t transportId)
        : _transportId(transportId) {}

    void configure(uint8_t transportId) { _transportId = transportId; }
    void setLocalAddress(IPAddress value) { _localAddress = value; }

    void setNoDelay(bool value)
    {
        _noDelay = value;
        if (_connected || _socketFd >= 0)
        {
            applySocketOptions();
        }
    }
    bool noDelay() const { return _noDelay; }

    void setKeepAlive(
        bool enabled,
        uint32_t idleSeconds,
        uint32_t intervalSeconds,
        uint8_t probeCount)
    {
        _keepAliveEnabled = enabled;
        _keepAliveIdleSeconds = idleSeconds;
        _keepAliveIntervalSeconds = intervalSeconds;
        _keepAliveProbeCount = probeCount;
        if (_connected || _socketFd >= 0)
        {
            applySocketOptions();
        }
    }

    bool keepAliveEnabled() const { return _keepAliveEnabled; }
    bool keepAliveApplied() const { return _keepAliveApplied; }
    int lastOptionError() const { return _lastOptionError; }

    // Adopt a non-blocking socket returned by accept(). Used by TCP Server.
    bool adoptAcceptedSocket(
        int socketFd,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort);
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    // ESP8266 core 3.x has LWIP_SOCKET=0. Accepted clients are transferred
    // through the native WiFiClient wrapper instead of a POSIX descriptor.
    bool adoptAcceptedClient(
        const WiFiClient &client,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort);
#endif

    uint8_t transportId() const override { return _transportId; }

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
    void flush() override {}

    uint16_t localPort() const override { return _localPort; }
    IPAddress remoteIP() const override { return _endpoint.remoteIP; }
    uint16_t remotePort() const override { return _endpoint.remotePort; }

    bool connecting() const { return _connecting; }
    int lastNativeError() const { return _lastNativeError; }
    uint8_t lastBackendEvent() const { return _lastBackendEvent; }
    void clearBackendEvent() { _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE; }

private:
    static bool ipIsZero(IPAddress ip);
    uint8_t pollConnect();
    bool peerClosed();
    void resetSocket();
    void applySocketOptions();

    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    IPAddress _localAddress = IPAddress(0, 0, 0, 0);
    FLProgSocketEndpoint _endpoint;

    int _socketFd = -1;
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    WiFiClient _esp8266Client;
#endif
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

class FLProgNetworkTcpClient : public FLProgNetworkSocket
{
public:
    FLProgNetworkTcpClient();

    bool begin(
        FLProgNetworkInterface &network,
        uint8_t route = 0);

    void end();

    uint8_t connect(IPAddress ip, uint16_t port);
    uint8_t connect(const char *hostname, uint16_t port);
    uint8_t connect(const String &hostname, uint16_t port)
    {
        return connect(hostname.c_str(), port);
    }

    // Graceful close is driven by network.pool().
    void stop();
    void pool() override;

    void setRoute(uint8_t value);
    uint8_t route() const { return _logicalRoute; }

    bool connectionRequested() const { return _connectRequested; }
    bool resolving() const { return _needResolve; }
    bool hostnameMode() const { return _hostnameMode; }
    const char *hostname() const { return _hostname; }
    IPAddress resolvedIP() const { return _resolvedIP; }
    uint8_t resolvedTransportId() const { return _resolvedTransportId; }

    // DNS policy. SHARED keeps the single Network resolver and serializes
    // different simultaneous hostname requests. PRIVATE attaches a resolver
    // owned by this TCP client only while a lookup is active, allowing
    // concurrent lookups without permanently consuming service capacity.
    bool setDnsPolicy(uint8_t value);
    uint8_t dnsPolicy() const { return _dnsPolicy; }
    FLProgNetworkDns *activeDnsResolver();

    // Low-latency option. POSIX/lwIP maps this to TCP_NODELAY. Wiznet has no
    // POSIX Nagle switch; it maps to Sn_MR.ND (No Delayed ACK) instead.
    void setNoDelay(bool value);
    bool noDelay() const { return _noDelay; }

    // Keepalive is best-effort per transport:
    // ESP32: SO_KEEPALIVE + TCP_KEEPIDLE/INTVL/KEEPCNT when available.
    // ESP8266: WiFiClient keepAlive()/disableKeepAlive().
    // W5500/W5100S: automatic 5-second timer units.
    // W5200: cooperative SEND_KEEP from pool(). W5100: unsupported.
    void setKeepAlive(
        bool enabled,
        uint32_t idleSeconds = 60,
        uint32_t intervalSeconds = 10,
        uint8_t probeCount = 3);
    bool keepAliveEnabled() const { return _keepAliveEnabled; }
    bool keepAliveApplied() const;

    void setAutoReconnect(bool value) { _autoReconnect = value; }
    bool autoReconnect() const { return _autoReconnect; }

    void setReconnectOnInitialFailure(bool value)
    {
        _reconnectOnInitialFailure = value;
    }
    bool reconnectOnInitialFailure() const
    {
        return _reconnectOnInitialFailure;
    }

    void setReconnectBackoff(uint32_t initialMs, uint32_t maxMs);
    uint32_t reconnectInitialDelay() const { return _reconnectInitialMs; }
    uint32_t reconnectMaximumDelay() const { return _reconnectMaxMs; }
    uint32_t reconnectCurrentDelay() const { return _reconnectCurrentMs; }
    bool reconnectScheduled() const { return _restartPending; }
    uint32_t reconnectIn() const;

    uint8_t clientError() const { return _clientError; }
    uint8_t dnsError() const { return _dnsError; }
    void clearClientError()
    {
        _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
        _dnsError = 0;
        clearError();
    }

    uint8_t lastDisconnectReason() const { return _lastDisconnectReason; }
    int lastNativeError() const { return _lastNativeError; }
    int lastOptionError() const { return _lastOptionError; }

    uint32_t connectAttemptCount() const { return _connectAttemptCount; }
    uint32_t connectSuccessCount() const { return _connectSuccessCount; }
    uint32_t disconnectCount() const { return _disconnectCount; }
    uint32_t reconnectScheduleCount() const { return _reconnectScheduleCount; }
    uint32_t dnsBusyCount() const { return _dnsBusyCount; }

    void resetTcpStatistics()
    {
        _connectAttemptCount = 0;
        _connectSuccessCount = 0;
        _disconnectCount = 0;
        _reconnectScheduleCount = 0;
        _dnsBusyCount = 0;
    }

    uint8_t logicalRoute() const { return _logicalRoute; }

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgWiznetSocketBackend &wiznetBackend() { return _wiznetBackend; }
#endif
    FLProgPosixTcpClientBackend &wifiBackend() { return _wifiBackend; }
    FLProgPosixTcpClientBackend &wifiApBackend() { return _wifiApBackend; }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgPosixTcpClientBackend &rmiiBackend() { return _rmiiBackend; }
#endif

private:
    static bool validTarget(IPAddress ip, uint16_t port);
    static bool timeReached(uint32_t now, uint32_t target)
    {
        return (int32_t)(now - target) >= 0;
    }

    uint8_t currentResult() const;
    void resetTargetState();
    void startResolvedTarget(IPAddress ip, uint8_t transportId);
    void dnsStep();
    void applyBackendOptions();

    void startIpOpen();
    void scheduleRestart(uint8_t reason, uint32_t delayMs, bool reResolve);
    void serviceRestart();
    void scheduleReconnectFromFailure();
    uint8_t mapFailureReason() const;
    void captureBackendDiagnostics();
    void updateConnectionStatistics();
    void resetReconnectBackoff();
    void advanceReconnectBackoff();

    FLProgNetworkInterface *_tcpNetwork = nullptr;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgWiznetSocketBackend _wiznetBackend;
#endif
    FLProgPosixTcpClientBackend _wifiBackend;
    FLProgPosixTcpClientBackend _wifiApBackend;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgPosixTcpClientBackend _rmiiBackend;
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    FLProgStm32TcpClientBackend _stm32RmiiBackend;
#endif
    FLProgNetworkDns _privateDns;
    bool _privateDnsAttached = false;

    uint8_t _logicalRoute = 0;
    uint16_t _targetPort = 0;
    bool _connectRequested = false;
    bool _hostnameMode = false;
    bool _needResolve = false;
    bool _autoReconnect = true;
    bool _reconnectOnInitialFailure = false;
    bool _everConnected = false;
    bool _wasConnected = false;

    uint8_t _dnsPolicy = FLPROG_TCP_DNS_POLICY_SHARED;

    bool _noDelay = false;
    bool _keepAliveEnabled = false;
    uint32_t _keepAliveIdleSeconds = 60;
    uint32_t _keepAliveIntervalSeconds = 10;
    uint8_t _keepAliveProbeCount = 3;

    bool _restartPending = false;
    bool _restartResolve = false;
    uint32_t _restartAt = 0;
    uint32_t _reconnectInitialMs = FLPROG_TCP_RECONNECT_INITIAL_MS;
    uint32_t _reconnectMaxMs = FLPROG_TCP_RECONNECT_MAX_MS;
    uint32_t _reconnectCurrentMs = FLPROG_TCP_RECONNECT_INITIAL_MS;

    char _hostname[FLPROG_TCP_HOSTNAME_MAX_LENGTH + 1] = {0};
    IPAddress _resolvedIP = IPAddress(0, 0, 0, 0);
    uint8_t _resolvedTransportId = FLPROG_TRANSPORT_NONE;

    uint8_t _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
    uint8_t _dnsError = 0;
    uint8_t _lastDisconnectReason = FLPROG_TCP_DISCONNECT_NONE;
    int _lastNativeError = 0;
    int _lastOptionError = 0;

    uint32_t _connectAttemptCount = 0;
    uint32_t _connectSuccessCount = 0;
    uint32_t _disconnectCount = 0;
    uint32_t _reconnectScheduleCount = 0;
    uint32_t _dnsBusyCount = 0;
};
