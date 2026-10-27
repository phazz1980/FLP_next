/*
 * FLProg_Network — flprogNetworkTcpServer.h
 * Назначение: Native TCP server FLProg_Network.
 *
 * Архитектурные заметки:
 * - Принимает клиентов через A3/socket backend выбранного transport.
 * - Сервер не должен владеть NetworkDevice: он получает интерфейс и route извне.
 * - Legacy FLProgEthernetServer является только фасадом над этим классом.
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
#include "../service/flprogNetworkService.h"
#include "../socket/flprogNetworkSocket.h"
#include "../socket/flprogSocketBackends.h"
#include "flprogNetworkTcpClient.h"

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// FLProg_Network v0.8.4 - non-blocking route-aware TCP Server.
//
// One server binds to one selected transport at a time. A live server is not
// migrated merely because a higher-priority interface appears. Rebinding is
// performed only after the bound transport is lost/stale or the route changes.
//
// Wiznet:
//   a LISTEN socket becomes the connected client socket after ESTABLISHED;
//   the service then opens another LISTEN socket when capacity is available.
//
// ESP32 POSIX/lwIP:
//   one non-blocking listener accepts fds; each accepted fd is adopted by the
//   same stream backend used by FLProgNetworkTcpClient.
// ESP8266 core 3.x:
//   LWIP_SOCKET is disabled; WiFiServer/WiFiClient wrap the raw-lwIP path.
// -----------------------------------------------------------------------------

#ifndef FLPROG_TCP_SERVER_MAX_CLIENTS
#define FLPROG_TCP_SERVER_MAX_CLIENTS 8
#endif

#ifndef FLPROG_TCP_SERVER_DEFAULT_MAX_CLIENTS
#define FLPROG_TCP_SERVER_DEFAULT_MAX_CLIENTS 4
#endif

#ifndef FLPROG_TCP_SERVER_LISTEN_BACKLOG
#define FLPROG_TCP_SERVER_LISTEN_BACKLOG 4
#endif

#ifndef FLPROG_TCP_SERVER_ACCEPT_WORK_BUDGET
#define FLPROG_TCP_SERVER_ACCEPT_WORK_BUDGET 2
#endif

#define FLPROG_TCP_SERVER_SHUTDOWN_RUNNING   0
#define FLPROG_TCP_SERVER_SHUTDOWN_REQUESTED 1
#define FLPROG_TCP_SERVER_SHUTDOWN_DRAINING  2
#define FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE  3

#define FLPROG_TCP_SERVER_ERROR_NONE          0
#define FLPROG_TCP_SERVER_ERROR_CONFIG        1
#define FLPROG_TCP_SERVER_ERROR_ATTACH        2
#define FLPROG_TCP_SERVER_ERROR_NO_TRANSPORT  3
#define FLPROG_TCP_SERVER_ERROR_LISTEN        4
#define FLPROG_TCP_SERVER_ERROR_RESOURCE      5
#define FLPROG_TCP_SERVER_ERROR_ACCEPT        6

#define FLPROG_TCP_SERVER_EVENT_NONE              0x00
#define FLPROG_TCP_SERVER_EVENT_NEW_CLIENT        0x01
#define FLPROG_TCP_SERVER_EVENT_CLIENT_CLOSED     0x02
#define FLPROG_TCP_SERVER_EVENT_LISTEN_UP         0x04
#define FLPROG_TCP_SERVER_EVENT_LISTEN_DOWN       0x08
#define FLPROG_TCP_SERVER_EVENT_TRANSPORT_CHANGED 0x10
#define FLPROG_TCP_SERVER_EVENT_RESOURCE_WAIT     0x20
#define FLPROG_TCP_SERVER_EVENT_CLIENT_IDLE        0x40
#define FLPROG_TCP_SERVER_EVENT_SHUTDOWN_COMPLETE  0x80

class FLProgNetworkTcpServerClient : public FLProgNetworkSocket
{
public:
    FLProgNetworkTcpServerClient();

    bool active() const { return _used && _accepted; }
    bool accepted() const { return _accepted; }
    bool pendingAccept() const { return _pendingAccept; }
    bool listenerSlot() const { return _used && _listener; }
    uint8_t slotId() const { return _slotId; }
    uint8_t transportId() const { return _transportId; }

    void stop();

    uint32_t acceptedAt() const { return _acceptedAt; }
    uint32_t lastActivityAt() const { return _lastActivityAt; }
    uint32_t idleFor() const
    {
        return _accepted ? (uint32_t)(millis() - _lastActivityAt) : 0;
    }
    uint32_t rxBytes() const { return _rxBytes; }
    uint32_t txBytes() const { return _txBytes; }

    int read();
    int read(uint8_t *buffer, size_t length);
    size_t write(uint8_t value);
    size_t write(const uint8_t *buffer, size_t length);

    FLProgWiznetSocketBackend &wiznetBackend() { return _wiznetBackend; }
    const FLProgWiznetSocketBackend &wiznetBackend() const { return _wiznetBackend; }
    FLProgPosixTcpClientBackend &posixBackend() { return _posixBackend; }
    const FLProgPosixTcpClientBackend &posixBackend() const { return _posixBackend; }

private:
    friend class FLProgNetworkTcpServer;

    bool startWiznetListener(
        FLProgNetworkInterface &network,
        uint16_t port,
        uint8_t slotId,
        bool noDelay,
        bool keepAlive,
        uint32_t keepAliveIntervalMs);

    bool adoptPosix(
        FLProgNetworkInterface &network,
        uint8_t transportId,
        int socketFd,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort,
        uint8_t slotId,
        bool noDelay,
        bool keepAlive,
        uint32_t keepAliveIdleSeconds,
        uint32_t keepAliveIntervalSeconds,
        uint8_t keepAliveProbeCount);
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    bool adoptEsp8266(
        FLProgNetworkInterface &network,
        uint8_t transportId,
        const WiFiClient &client,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort,
        uint8_t slotId,
        bool noDelay,
        bool keepAlive,
        uint32_t keepAliveIdleSeconds,
        uint32_t keepAliveIntervalSeconds,
        uint8_t keepAliveProbeCount);
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    bool adoptStm32(
        FLProgNetworkInterface &network,
        tcp_struct *client,
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort,
        uint8_t slotId,
        bool noDelay,
        bool keepAlive,
        uint32_t keepAliveIdleSeconds,
        uint32_t keepAliveIntervalSeconds,
        uint8_t keepAliveProbeCount);
#endif

    bool promoteAccepted();
    void requestCleanup();
    bool cleanupComplete() const;
    void releaseSlot();

    static uint8_t fixedRouteForTransport(uint8_t transportId);

    FLProgWiznetSocketBackend _wiznetBackend;
    FLProgPosixTcpClientBackend _posixBackend;
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    FLProgStm32TcpClientBackend _stm32Backend;
#endif

    uint8_t _slotId = 0xFF;
    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    bool _used = false;
    bool _listener = false;
    bool _accepted = false;
    bool _pendingAccept = false;
    bool _cleanupRequested = false;

    uint32_t _acceptedAt = 0;
    uint32_t _lastActivityAt = 0;
    uint32_t _rxBytes = 0;
    uint32_t _txBytes = 0;
};

class FLProgPosixTcpServerListener
{
public:
    FLProgPosixTcpServerListener() {}
    ~FLProgPosixTcpServerListener() { close(); }

    bool begin(
        uint8_t transportId,
        IPAddress localIp,
        uint16_t port,
        uint8_t backlog);

    void close();
    bool listening() const { return _listening; }
    uint8_t transportId() const { return _transportId; }
    uint16_t localPort() const { return _localPort; }
    int lastNativeError() const { return _lastNativeError; }

    // 1 = accepted, 0 = no pending connection, -1 = error.
    int acceptOne(
        int &socketFd,
        IPAddress &remoteIp,
        uint16_t &remotePort);
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    int acceptOne(
        WiFiClient &client,
        IPAddress &remoteIp,
        uint16_t &remotePort);
#endif

private:
    int _socketFd = -1;
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    alignas(WiFiServer) uint8_t _esp8266ServerStorage[sizeof(WiFiServer)];
    WiFiServer *_esp8266Server = nullptr;
#endif
    uint8_t _transportId = FLPROG_TRANSPORT_NONE;
    uint16_t _localPort = 0;
    bool _listening = false;
    int _lastNativeError = 0;
};

class FLProgNetworkTcpServer : public FLProgNetworkService
{
public:
    FLProgNetworkTcpServer();
    ~FLProgNetworkTcpServer() override;

    // Client storage and listener registrations have a single owner.
    FLProgNetworkTcpServer(const FLProgNetworkTcpServer &) = delete;
    FLProgNetworkTcpServer &operator=(const FLProgNetworkTcpServer &) = delete;

    bool begin(
        FLProgNetworkInterface &network,
        uint16_t port,
        uint8_t route = 0,
        uint8_t maxClients = FLPROG_TCP_SERVER_DEFAULT_MAX_CLIENTS);

    // Internal child-service start. The object is owned and pooled by its
    // parent service and therefore must not consume a managed-service slot.
    // Used only for composite services such as legacy Web STA+SoftAP.
    bool beginChild(
        FLProgNetworkInterface &network,
        uint16_t port,
        uint8_t route = 0,
        uint8_t maxClients = FLPROG_TCP_SERVER_DEFAULT_MAX_CLIENTS);

    void end();
    void pool() override;

    void setRoute(uint8_t value);
    uint8_t route() const { return _route; }

    uint16_t port() const { return _port; }
    uint8_t maxClients() const { return _maxClients; }
    bool setMaxClients(uint8_t value);

    bool listening() const;
    uint8_t boundTransportId() const { return _boundTransportId; }
    uint32_t boundTransportGeneration() const { return _boundGeneration; }

    uint8_t clientCount() const;
    // Low-level Wiznet listener snapshot for diagnostics. Returns CLOSED (0)
    // when this server currently owns no Wiznet socket.
    uint8_t wiznetSocketStatus() const;
    bool wiznetSendPending() const;
    uint8_t lastWiznetCloseEvent() const { return _lastWiznetCloseEvent; }
    FLProgNetworkTcpServerClient *client(uint8_t slot);
    const FLProgNetworkTcpServerClient *client(uint8_t slot) const;

    // First newly accepted client, clearing its new-client bit.
    FLProgNetworkTcpServerClient *takeNewClient();

    // First connected client with received data waiting.
    FLProgNetworkTcpServerClient *availableClient();

    void disconnectAll();

    void setNoDelay(bool value);
    bool noDelay() const { return _noDelay; }

    void setKeepAlive(
        bool enabled,
        uint32_t idleSeconds = 60,
        uint32_t intervalSeconds = 10,
        uint8_t probeCount = 3);

    bool keepAliveEnabled() const { return _keepAliveEnabled; }

    void setListenBacklog(uint8_t value)
    {
        _listenBacklog = value == 0 ? 1 : value;
    }
    uint8_t listenBacklog() const { return _listenBacklog; }

    void setAcceptWorkBudget(uint8_t value)
    {
        _acceptWorkBudget = value == 0 ? 1 : value;
    }
    uint8_t acceptWorkBudget() const { return _acceptWorkBudget; }

    void setClientIdleTimeout(uint32_t value) { _clientIdleTimeoutMs = value; }
    uint32_t clientIdleTimeout() const { return _clientIdleTimeoutMs; }

    // Cooperative whole-server shutdown. New accepts stop immediately.
    // Existing clients are closed through the normal socket close state
    // machine from network.pool(); no user-side disconnectAll()/end() sequence
    // is required.
    void requestShutdown();
    uint8_t shutdownState() const { return _shutdownState; }
    bool shuttingDown() const
    {
        return
            _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_REQUESTED ||
            _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_DRAINING;
    }
    bool shutdownComplete() const
    {
        return _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE;
    }

    // Sustainable accepted-client capacity while retaining a LISTEN resource.
    // For Wiznet this accounts for chip socket count, STRICT ACD and sockets
    // owned by other protocol objects. POSIX transports are bounded by logical
    // leases, managed socket slots and maxClients.
    uint8_t effectiveClientCapacity() const;
    uint8_t acceptCapacityRemaining() const;
    uint8_t serverOwnedWiznetSocketCount() const;
    uint8_t externalWiznetSocketReservationCount() const;

    uint8_t serverError() const { return _serverError; }
    int lastNativeError() const { return _lastNativeError; }
    void clearServerError()
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
        _lastNativeError = 0;
    }

    uint8_t events() const { return _events; }
    uint8_t takeEvents()
    {
        const uint8_t result = _events;
        _events = FLPROG_TCP_SERVER_EVENT_NONE;
        return result;
    }

    uint32_t listenOpenCount() const { return _listenOpenCount; }
    uint32_t acceptCount() const { return _acceptCount; }
    uint32_t rejectCount() const { return _rejectCount; }
    uint32_t disconnectCount() const { return _disconnectCount; }
    uint32_t resourceWaitCount() const { return _resourceWaitCount; }
    uint32_t transportChangeCount() const { return _transportChangeCount; }
    uint32_t idleTimeoutCount() const { return _idleTimeoutCount; }
    uint32_t shutdownCount() const { return _shutdownCount; }

    int16_t wiznetListenerSocket() const;
    uint8_t wiznetSocketsReserved() const;
    uint8_t wiznetSocketsAvailable() const;

    void resetStatistics();

private:
    static bool timeTransportUsable(
        FLProgNetworkInterface &network,
        uint8_t transportId,
        uint32_t generation);

    static uint8_t fixedRouteForTransport(uint8_t transportId);

    bool ensureClientCapacity(uint8_t required);
    int findFreeSlot() const;
    int findWiznetListenerSlot() const;
    void reapSlots();
    void processPendingAccepts();
    void bindTransportIfNeeded();
    void handleTransportLoss();
    void ensureListener();
    void ensureWiznetListener();
    void ensurePosixListener();
    void acceptPosixClients();
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    void ensureStm32Listener();
    void acceptStm32Clients();
#endif
    void serviceShutdown();
    bool anyUsedSlot() const;
    void closePosixListener();
    bool reservePosixListenerLease();
    void releasePosixListenerLease();
    void noteResourceWait();
    void updateListenEventState();

    FLProgNetworkInterface *_network = nullptr;
    // Allocate exactly the requested number of client slots at begin().
    // Legacy WebServer uses maxClients=1, so two transport-specific WebServer
    // objects no longer reserve 8 TCP client backends each in static DRAM.
    FLProgNetworkTcpServerClient *_clients = nullptr;
    uint8_t _clientCapacity = 0;
    FLProgPosixTcpServerListener _posixListener;
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    FLProgStm32TcpServerListener _stm32Listener;
#endif

    uint16_t _port = 0;
    uint8_t _route = 0;
    uint8_t _maxClients = FLPROG_TCP_SERVER_DEFAULT_MAX_CLIENTS;
    uint8_t _listenBacklog = FLPROG_TCP_SERVER_LISTEN_BACKLOG;
    uint8_t _acceptWorkBudget = FLPROG_TCP_SERVER_ACCEPT_WORK_BUDGET;
    uint32_t _clientIdleTimeoutMs = 0;
    uint8_t _availableCursor = 0;
    uint8_t _newClientCursor = 0;
    uint8_t _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE;

    uint8_t _boundTransportId = FLPROG_TRANSPORT_NONE;
    uint32_t _boundGeneration = 0;
    uint8_t _lastTransportId = FLPROG_TRANSPORT_NONE;

    uint8_t _posixListenerLease = FLPROG_SOCKET_INVALID_HANDLE;

    bool _active = false;
    bool _lastListeningState = false;
    bool _noDelay = false;
    bool _keepAliveEnabled = false;
    uint32_t _keepAliveIdleSeconds = 60;
    uint32_t _keepAliveIntervalSeconds = 10;
    uint8_t _keepAliveProbeCount = 3;

    uint8_t _newClientMask = 0;
    uint8_t _events = FLPROG_TCP_SERVER_EVENT_NONE;
    uint8_t _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
    int _lastNativeError = 0;
    uint8_t _lastWiznetCloseEvent = FLPROG_TCP_BACKEND_EVENT_NONE;

    uint32_t _listenOpenCount = 0;
    uint32_t _acceptCount = 0;
    uint32_t _rejectCount = 0;
    uint32_t _disconnectCount = 0;
    uint32_t _resourceWaitCount = 0;
    uint32_t _transportChangeCount = 0;
    uint32_t _idleTimeoutCount = 0;
    uint32_t _shutdownCount = 0;
};
