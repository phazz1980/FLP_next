/*
 * FLProg_Network — flprogNetworkSocket.h
 * Назначение: A3 socket manager/lease lifecycle.
 *
 * Архитектурные заметки:
 * - Связывает логический socket с конкретным TransportIdentity и generation.
 * - STALE означает, что lease относится к старому поколению transport и требует reopen.
 * - LOCAL_REOPEN одного socket не должен сбрасывать соседние sockets того же transport.
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
#include "../transport/flprogNetworkTransport.h"

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// FLProg_Network v0.5.2 - Общий Socket API.
//
// Один общий автомат состояний для будущих TCP/UDP/RAW сокетов.
// Слой отвечает за route/transport binding, lease, stale-контроль и общий
// I/O API. Он не имеет зависимостей от SPI, RT_HW_Base, регистров Wiznet, WiFiClient, lwIP или
// ETH.h. Аппаратная/протокольная работа передаётся FLProgSocketBackend.
// -----------------------------------------------------------------------------

#define FLPROG_SOCKET_PROTOCOL_NONE     0
#define FLPROG_SOCKET_PROTOCOL_STREAM   1
#define FLPROG_SOCKET_PROTOCOL_DATAGRAM 2
#define FLPROG_SOCKET_PROTOCOL_RAW      3

// -----------------------------------------------------------------------------
// RC6-A public core contract.
//
// The numeric values used by rc.5 are intentionally preserved. New values are
// appended so existing user code that stores or compares legacy constants keeps
// the same meaning. Legacy FLPROG_SOCKET_* names below are compatibility aliases.
// -----------------------------------------------------------------------------
enum FLProgNetworkResult : uint8_t
{
    FLPROG_NETWORK_RESULT_SUCCESS = 0,
    FLPROG_NETWORK_RESULT_WAIT = 1,
    FLPROG_NETWORK_RESULT_ERROR = 2,
    FLPROG_NETWORK_RESULT_UNSUPPORTED = 3
};

enum FLProgNetworkSocketState : uint8_t
{
    FLPROG_NETWORK_SOCKET_STATE_CLOSED = 0,
    FLPROG_NETWORK_SOCKET_STATE_WAIT_TRANSPORT = 1,
    FLPROG_NETWORK_SOCKET_STATE_RESERVED = 2,
    FLPROG_NETWORK_SOCKET_STATE_OPENING = 3,
    FLPROG_NETWORK_SOCKET_STATE_OPEN = 4,
    FLPROG_NETWORK_SOCKET_STATE_CLOSING = 5,
    FLPROG_NETWORK_SOCKET_STATE_STALE = 6,
    FLPROG_NETWORK_SOCKET_STATE_REOPEN_REQUIRED = 7,
    FLPROG_NETWORK_SOCKET_STATE_ERROR = 8,
    FLPROG_NETWORK_SOCKET_STATE_UNSUPPORTED = 9
};

enum FLProgNetworkSocketError : uint8_t
{
    FLPROG_NETWORK_SOCKET_ERROR_NONE = 0,
    FLPROG_NETWORK_SOCKET_ERROR_NO_NETWORK = 1,
    FLPROG_NETWORK_SOCKET_ERROR_NO_TRANSPORT = 2,
    FLPROG_NETWORK_SOCKET_ERROR_NO_LEASE = 3,
    FLPROG_NETWORK_SOCKET_ERROR_NO_BACKEND = 4,
    FLPROG_NETWORK_SOCKET_ERROR_OPEN_ERROR = 5,
    FLPROG_NETWORK_SOCKET_ERROR_IO_ERROR = 6,
    FLPROG_NETWORK_SOCKET_ERROR_TIMEOUT = 7,
    FLPROG_NETWORK_SOCKET_ERROR_STALE = 8,
    FLPROG_NETWORK_SOCKET_ERROR_RESOURCE_FULL = 9,
    FLPROG_NETWORK_SOCKET_ERROR_CONNECT_ERROR = 10,
    FLPROG_NETWORK_SOCKET_ERROR_UNSUPPORTED = 11
};

enum FLProgNetworkSocketEvent : uint8_t
{
    FLPROG_NETWORK_EVENT_NONE = 0x00,
    // rc.5 compatibility: transport changed/stale keep their original bits.
    FLPROG_NETWORK_EVENT_TRANSPORT_CHANGED = 0x01,
    FLPROG_NETWORK_EVENT_STALE = 0x04,
    FLPROG_NETWORK_EVENT_CONNECTED = 0x08,
    FLPROG_NETWORK_EVENT_DISCONNECTED = 0x10,
    FLPROG_NETWORK_EVENT_DATA_AVAILABLE = 0x20,
    FLPROG_NETWORK_EVENT_ERROR = 0x40
};

// Result compatibility aliases.
#define FLPROG_SOCKET_RESULT_SUCCESS     FLPROG_NETWORK_RESULT_SUCCESS
#define FLPROG_SOCKET_RESULT_WAIT        FLPROG_NETWORK_RESULT_WAIT
#define FLPROG_SOCKET_RESULT_ERROR       FLPROG_NETWORK_RESULT_ERROR
#define FLPROG_SOCKET_RESULT_UNSUPPORTED FLPROG_NETWORK_RESULT_UNSUPPORTED

// State compatibility aliases.
#define FLPROG_SOCKET_STATE_CLOSED          FLPROG_NETWORK_SOCKET_STATE_CLOSED
#define FLPROG_SOCKET_STATE_WAIT_TRANSPORT  FLPROG_NETWORK_SOCKET_STATE_WAIT_TRANSPORT
#define FLPROG_SOCKET_STATE_RESERVED        FLPROG_NETWORK_SOCKET_STATE_RESERVED
#define FLPROG_SOCKET_STATE_OPENING         FLPROG_NETWORK_SOCKET_STATE_OPENING
#define FLPROG_SOCKET_STATE_OPEN            FLPROG_NETWORK_SOCKET_STATE_OPEN
#define FLPROG_SOCKET_STATE_CLOSING         FLPROG_NETWORK_SOCKET_STATE_CLOSING
#define FLPROG_SOCKET_STATE_STALE           FLPROG_NETWORK_SOCKET_STATE_STALE
#define FLPROG_SOCKET_STATE_REOPEN_REQUIRED FLPROG_NETWORK_SOCKET_STATE_REOPEN_REQUIRED
#define FLPROG_SOCKET_STATE_ERROR           FLPROG_NETWORK_SOCKET_STATE_ERROR
#define FLPROG_SOCKET_STATE_UNSUPPORTED     FLPROG_NETWORK_SOCKET_STATE_UNSUPPORTED

// Error compatibility aliases. rc.5 OPEN/IO/REGISTRY_FULL map to the RC6 names.
#define FLPROG_SOCKET_ERROR_NONE          FLPROG_NETWORK_SOCKET_ERROR_NONE
#define FLPROG_SOCKET_ERROR_NO_NETWORK    FLPROG_NETWORK_SOCKET_ERROR_NO_NETWORK
#define FLPROG_SOCKET_ERROR_NO_TRANSPORT  FLPROG_NETWORK_SOCKET_ERROR_NO_TRANSPORT
#define FLPROG_SOCKET_ERROR_NO_LEASE      FLPROG_NETWORK_SOCKET_ERROR_NO_LEASE
#define FLPROG_SOCKET_ERROR_NO_BACKEND    FLPROG_NETWORK_SOCKET_ERROR_NO_BACKEND
#define FLPROG_SOCKET_ERROR_OPEN          FLPROG_NETWORK_SOCKET_ERROR_OPEN_ERROR
#define FLPROG_SOCKET_ERROR_OPEN_ERROR    FLPROG_NETWORK_SOCKET_ERROR_OPEN_ERROR
#define FLPROG_SOCKET_ERROR_IO            FLPROG_NETWORK_SOCKET_ERROR_IO_ERROR
#define FLPROG_SOCKET_ERROR_IO_ERROR      FLPROG_NETWORK_SOCKET_ERROR_IO_ERROR
#define FLPROG_SOCKET_ERROR_TIMEOUT       FLPROG_NETWORK_SOCKET_ERROR_TIMEOUT
#define FLPROG_SOCKET_ERROR_STALE         FLPROG_NETWORK_SOCKET_ERROR_STALE
#define FLPROG_SOCKET_ERROR_REGISTRY_FULL FLPROG_NETWORK_SOCKET_ERROR_RESOURCE_FULL
#define FLPROG_SOCKET_ERROR_RESOURCE_FULL FLPROG_NETWORK_SOCKET_ERROR_RESOURCE_FULL
#define FLPROG_SOCKET_ERROR_CONNECT_ERROR FLPROG_NETWORK_SOCKET_ERROR_CONNECT_ERROR
#define FLPROG_SOCKET_ERROR_UNSUPPORTED   FLPROG_NETWORK_SOCKET_ERROR_UNSUPPORTED

#define FLPROG_SOCKET_ROUTE_CLASS_FIXED   0
#define FLPROG_SOCKET_ROUTE_CLASS_WIRED   1
#define FLPROG_SOCKET_ROUTE_CLASS_DYNAMIC 2

// Event compatibility aliases. REBIND_REQUESTED remains an internal legacy bit.
#define FLPROG_SOCKET_EVENT_NONE              FLPROG_NETWORK_EVENT_NONE
#define FLPROG_SOCKET_EVENT_TRANSPORT_CHANGED FLPROG_NETWORK_EVENT_TRANSPORT_CHANGED
#define FLPROG_SOCKET_EVENT_REBIND_REQUESTED  0x02
#define FLPROG_SOCKET_EVENT_STALE             FLPROG_NETWORK_EVENT_STALE
#define FLPROG_SOCKET_EVENT_CONNECTED         FLPROG_NETWORK_EVENT_CONNECTED
#define FLPROG_SOCKET_EVENT_DISCONNECTED      FLPROG_NETWORK_EVENT_DISCONNECTED
#define FLPROG_SOCKET_EVENT_DATA_AVAILABLE    FLPROG_NETWORK_EVENT_DATA_AVAILABLE
#define FLPROG_SOCKET_EVENT_ERROR             FLPROG_NETWORK_EVENT_ERROR

struct FLProgSocketEndpoint
{
    uint16_t localPort = 0;
    IPAddress remoteIP = IPAddress(0, 0, 0, 0);
    uint16_t remotePort = 0;
};

// Контракт backend для одной конкретной реализации transport/protocol.
class FLProgSocketBackend
{
public:
    virtual ~FLProgSocketBackend() {}

    virtual uint8_t transportId() const = 0;

    virtual uint8_t open(
        const FLProgSocketLease &lease,
        uint8_t protocol,
        const FLProgSocketEndpoint &endpoint,
        uint32_t timeout)
    {
        (void)lease;
        (void)protocol;
        (void)endpoint;
        (void)timeout;
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    virtual uint8_t pool() { return FLPROG_SOCKET_RESULT_SUCCESS; }
    virtual uint8_t close() { return FLPROG_SOCKET_RESULT_SUCCESS; }

    virtual bool isOpen() const { return false; }
    virtual bool connected() const { return false; }

    virtual int available() { return 0; }
    virtual int read() { return -1; }

    virtual int read(uint8_t *buffer, size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }

    virtual int peek() { return -1; }

    virtual size_t write(uint8_t value)
    {
        return write(&value, 1);
    }

    virtual size_t write(const uint8_t *buffer, size_t length)
    {
        (void)buffer;
        (void)length;
        return 0;
    }

    virtual void flush() {}

    virtual uint16_t localPort() const { return 0; }
    virtual IPAddress remoteIP() const { return IPAddress(0, 0, 0, 0); }
    virtual uint16_t remotePort() const { return 0; }
};

class FLProgNetworkSocket
{
public:
    FLProgNetworkSocket();
    virtual ~FLProgNetworkSocket();

    // Регистрирует сокет в Network Manager. После begin() в пользовательском loop нужен только
    // network.pool().
    bool begin(
        FLProgNetworkInterface &network,
        uint8_t route,
        uint8_t protocol);

    // RC6-A.3 A3-4: advanced exact transport binding. This bypasses route
    // resolution and binds the socket to one exact transport identity.
    // Existing route-based begin() remains the default/high-level API.
    bool begin(
        FLProgNetworkInterface &network,
        const FLProgTransportIdentity &transportIdentity,
        uint8_t protocol);

    void end();

    bool isAttached() const { return _network != nullptr; }


    void setRoute(uint8_t route);
    uint8_t route() const { return _route; }

    uint8_t routeClass() const;
    bool routeIsDynamic() const
    {
        return routeClass() != FLPROG_SOCKET_ROUTE_CLASS_FIXED;
    }

    // Request a controlled close/release/reselect cycle while keeping the
    // desired-open flag set. Fixed routes re-open on the same transport;
    // WIRED/AUTO/DEFAULT re-evaluate currently ready transports.
    void requestRebind();

    uint8_t previousTransportId() const { return _previousTransportId; }
    FLProgTransportInstanceId previousTransportInstanceId() const
    {
        return _previousTransportInstanceId;
    }
    FLProgTransportIdentity previousTransportIdentity() const
    {
        FLProgTransportIdentity result;
        result.kind = _previousTransportId;
        result.instanceId = _previousTransportInstanceId;
        return result;
    }

    uint8_t events() const { return _events; }
    uint8_t takeEvents()
    {
        const uint8_t value = _events;
        _events = FLPROG_SOCKET_EVENT_NONE;
        return value;
    }

    bool transportChanged() const
    {
        return (_events & FLPROG_SOCKET_EVENT_TRANSPORT_CHANGED) != 0;
    }

    void setProtocol(uint8_t protocol);
    uint8_t protocol() const { return _protocol; }

    void setBackend(uint8_t transportId, FLProgSocketBackend *backend);

    // RC6-A.3 A3-5: exact backend binding. Multiple backend objects of the
    // same transport kind may coexist when their instance ids differ.
    void setBackend(
        const FLProgTransportIdentity &transportIdentity,
        FLProgSocketBackend *backend);

    FLProgSocketBackend *backend(uint8_t transportId);
    const FLProgSocketBackend *backend(uint8_t transportId) const;
    FLProgSocketBackend *backend(const FLProgTransportIdentity &transportIdentity);
    const FLProgSocketBackend *backend(
        const FLProgTransportIdentity &transportIdentity) const;

    void localPort(uint16_t value) { _endpoint.localPort = value; }
    uint16_t configuredLocalPort() const { return _endpoint.localPort; }

    void remoteIP(IPAddress value) { _endpoint.remoteIP = value; }
    IPAddress configuredRemoteIP() const { return _endpoint.remoteIP; }

    void remotePort(uint16_t value) { _endpoint.remotePort = value; }
    uint16_t configuredRemotePort() const { return _endpoint.remotePort; }

    void endpoint(
        uint16_t localPort,
        IPAddress remoteIp,
        uint16_t remotePort)
    {
        _endpoint.localPort = localPort;
        _endpoint.remoteIP = remoteIp;
        _endpoint.remotePort = remotePort;
    }

    const FLProgSocketEndpoint &configuredEndpoint() const { return _endpoint; }

    uint8_t requestOpen();
    uint8_t requestClose();

    // Немедленное логическое освобождение. Backend получает один вызов close().
    void abort();

    bool wantsOpen() const { return _wantOpen; }
    bool connectionRequested() const { return _wantOpen; }

    // Requested route is route(). resolvedRoute() reports the transport that is
    // actually selected/bound at the moment. DEFAULT means not resolved yet.
    uint8_t resolvedRoute() const;

    // transportId() is the public diagnostic name; boundTransportId() is kept
    // as an rc.5 compatibility accessor.
    uint8_t transportId() const { return _boundTransportId; }
    FLProgTransportInstanceId transportInstanceId() const
    {
        return _boundTransportInstanceId;
    }
    FLProgTransportIdentity transportIdentity() const
    {
        FLProgTransportIdentity result;
        result.kind = _boundTransportId;
        result.instanceId = _boundTransportInstanceId;
        return result;
    }
    uint32_t transportGeneration() const;

    // Вызывается автоматически из FLProgNetworkInterface::pool().
    virtual void pool();

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t lastError() const { return _lastError; }

    void clearError()
    {
        _error = FLPROG_SOCKET_ERROR_NONE;
        _lastError = FLPROG_SOCKET_ERROR_NONE;
        if (_state == FLPROG_SOCKET_STATE_ERROR)
        {
            setState(FLPROG_SOCKET_STATE_REOPEN_REQUIRED);
        }
    }

    bool isOpen() const;
    bool connected() const;

    bool stale() const
    {
        return
            _state == FLPROG_SOCKET_STATE_STALE ||
            _state == FLPROG_SOCKET_STATE_REOPEN_REQUIRED;
    }

    uint8_t leaseHandle() const { return _leaseHandle; }
    uint8_t boundTransportId() const { return _boundTransportId; }
    FLProgTransportInstanceId boundTransportInstanceId() const
    {
        return _boundTransportInstanceId;
    }
    FLProgTransportIdentity boundTransportIdentity() const
    {
        return transportIdentity();
    }
    bool usesExactTransportBinding() const
    {
        return _directTransportIdentity.valid();
    }
    int16_t nativeSocket() const;

    // Byte-stream contract:
    // - available() is the number of bytes readable immediately without blocking;
    // - connected()==true together with available()==0 is a normal state;
    // - read(buffer,length) may return any N where 0 < N < length;
    // - protocol frame boundaries are never inferred by Network.
    int available();
    int read();
    int read(uint8_t *buffer, size_t length);
    int peek();

    size_t write(uint8_t value);
    size_t write(const uint8_t *buffer, size_t length);

    void flush();

    uint16_t localPort() const;
    IPAddress remoteIP() const;
    uint16_t remotePort() const;

    void setTimeout(uint32_t value) { _timeout = value; }
    uint32_t timeout() const { return _timeout; }

    uint32_t openCount() const { return _openCount; }
    uint32_t reopenCount() const { return _reopenCount; }
    uint32_t staleCount() const { return _staleCount; }
    uint32_t ioErrorCount() const { return _ioErrorCount; }
    uint32_t transportChangeCount() const { return _transportChangeCount; }
    uint32_t routeDecisionCount() const { return _routeDecisionCount; }

    void resetStatistics()
    {
        _openCount = 0;
        _reopenCount = 0;
        _staleCount = 0;
        _ioErrorCount = 0;
        _transportChangeCount = 0;
        _routeDecisionCount = 0;
    }

protected:
    virtual void onStateChanged(uint8_t oldState, uint8_t newState)
    {
        (void)oldState;
        (void)newState;
    }

    // Some hardware protocols are tied to a specific native socket.
    // MACRAW on Wiznet is the primary v0.6.1 user and requires socket 0.
    void preferredNativeSocket(int16_t value)
    {
        _preferredNativeSocket = value;
    }

    int16_t preferredNativeSocket() const
    {
        return _preferredNativeSocket;
    }

private:
    friend class FLProgNetworkInterface;

    void setState(uint8_t value);
    void setError(uint8_t value, bool remember = true);
    void rememberError(uint8_t value);
    void clearActiveError();
    void updatePublicEvents();

    void releaseLease();
    bool ensureLease();
    bool ensureLeaseForBoundTransport();

    FLProgNetworkTransport *boundTransport();
    const FLProgNetworkTransport *boundTransport() const;

    bool closeBackendStep();
    bool handleStaleLease();
    bool handleRebindRequest();
    bool updateDynamicRouteBeforeOpen();
    FLProgTransportIdentity selectTransportForOpen(bool requireReady);
    void changeBoundTransport(const FLProgTransportIdentity &transportIdentity);
    void changeBoundTransport(uint8_t transportId)
    {
        FLProgTransportIdentity identity;
        identity.kind = transportId;
        identity.instanceId = (transportId == FLPROG_TRANSPORT_NONE)
            ? FLPROG_TRANSPORT_INSTANCE_INVALID
            : 0;
        changeBoundTransport(identity);
    }
    uint8_t driveOpenStep();

    FLProgNetworkInterface *_network = nullptr;

    struct BackendBinding
    {
        FLProgTransportIdentity identity;
        FLProgSocketBackend *backend = nullptr;
    };

    // Legacy kind-only table is retained for source/binary-style behavior of
    // the validated single-instance paths. Exact A3 bindings are authoritative
    // when present and allow e.g. W5500#0 + W5500#1 simultaneously.
    FLProgSocketBackend *_backends[FLPROG_TRANSPORT_COUNT];
    BackendBinding _backendBindings[FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES];
    FLProgSocketBackend *_activeBackend = nullptr;

    FLProgSocketEndpoint _endpoint;

    uint8_t _route = 0;
    uint8_t _protocol = FLPROG_SOCKET_PROTOCOL_NONE;

    uint8_t _leaseHandle = FLPROG_SOCKET_INVALID_HANDLE;
    int16_t _preferredNativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    uint8_t _boundTransportId = FLPROG_TRANSPORT_NONE;
    FLProgTransportInstanceId _boundTransportInstanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    uint8_t _previousTransportId = FLPROG_TRANSPORT_NONE;
    FLProgTransportInstanceId _previousTransportInstanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    // Valid only for the advanced exact-binding begin() overload. In normal
    // route mode this stays invalid and the identity is resolved by NetworkDevice.
    FLProgTransportIdentity _directTransportIdentity;

    uint8_t _events = FLPROG_SOCKET_EVENT_NONE;
    bool _rebindRequested = false;

    uint8_t _state = FLPROG_SOCKET_STATE_CLOSED;
    uint8_t _error = FLPROG_SOCKET_ERROR_NONE;
    uint8_t _lastError = FLPROG_SOCKET_ERROR_NONE;

    bool _wantOpen = false;
    bool _wasConnected = false;
    bool _hadAvailableData = false;
    bool _reopenCycle = false;

    uint32_t _timeout = 5000;
    uint32_t _stateStarted = 0;

    uint32_t _openCount = 0;
    uint32_t _reopenCount = 0;
    uint32_t _staleCount = 0;
    uint32_t _ioErrorCount = 0;
    uint32_t _transportChangeCount = 0;
    uint32_t _routeDecisionCount = 0;
};
