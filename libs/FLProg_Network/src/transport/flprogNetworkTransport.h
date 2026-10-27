/*
 * FLProg_Network — flprogNetworkTransport.h
 * Назначение: A3 transport layer: идентичность transport и generation.
 *
 * Архитектурные заметки:
 * - TransportIdentity = kind + instanceId; generation меняется только при реальном lifecycle transport.
 * - TCP server loss при живом Wi-Fi не должен менять Wi-Fi generation.
 * - NET_RESET и transport loss должны быть видимы socket-слою единообразно.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Transport описывает availability, identity и generation, а не прикладной TCP/HTTP policy.
 * - Реальная смена link/readiness меняет generation один раз и уведомляет socket layer без каскадных ложных reset.
 * - Stub остается inert: включение заголовка не регистрирует backend и не меняет поведение существующей сети.
 * Extended maintenance contract:
 * - Transport describes availability, identity and generation, not application TCP/HTTP policy.
 * - A real link/readiness change increments generation once and notifies the socket layer without cascaded false resets.
 * - A stub stays inert: including its header neither registers a backend nor changes existing network behavior.
 */
#pragma once

#include <Arduino.h>
#include <IPAddress.h>

// -----------------------------------------------------------------------------
// FLProg_Network v0.5.0 - Universal Transport Layer.
//
// This layer deliberately knows nothing about SPI, RT_HW_Base, ETH.h or Wi-Fi
// driver internals. It describes network transports by a common state API and
// provides logical socket ownership for the protocol layers that follow.
//
// v0.5.0 does NOT yet implement TCP or UDP data exchange. The next stages will
// build protocol sockets on top of these transport and lease primitives.
// -----------------------------------------------------------------------------

#ifndef FLPROG_TRANSPORT_NONE
#define FLPROG_TRANSPORT_NONE 0
#endif
#ifndef FLPROG_TRANSPORT_WIFI
#define FLPROG_TRANSPORT_WIFI 1
#endif
#ifndef FLPROG_TRANSPORT_WIZNET
#define FLPROG_TRANSPORT_WIZNET 2
#endif
#ifndef FLPROG_TRANSPORT_RMII
#define FLPROG_TRANSPORT_RMII 3
#endif
#ifndef FLPROG_TRANSPORT_WIFI_AP
#define FLPROG_TRANSPORT_WIFI_AP 4
#endif
#ifndef FLPROG_TRANSPORT_COUNT
#define FLPROG_TRANSPORT_COUNT 5
#endif

// RC6-A.3 transport identity --------------------------------------------------
// A transport is identified by a PAIR: transport kind + instance id.
// The legacy FLPROG_TRANSPORT_* values remain the transport-kind values for
// the already supported backends.  Instance ids are local to one kind:
// WIZNET#0 and WIZNET#1 are different transports, while WIFI#0 and WIZNET#0
// are also different because their kinds differ.
#ifndef FLPROG_TRANSPORT_INSTANCE_INVALID
#define FLPROG_TRANSPORT_INSTANCE_INVALID 0xFFU
#endif
#ifndef FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES
#define FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES 8
#endif

typedef uint8_t FLProgTransportKind;
typedef uint8_t FLProgTransportInstanceId;

// A3 kind namespace. Existing backends keep their legacy numeric values while
// exact Wiznet chip kinds are allocated outside FLPROG_TRANSPORT_COUNT. This
// lets identity/routing evolve without silently re-labeling the validated
// generic WIZNET backend used by W5100/W5100S/W5200/W5500 today.
#ifndef FLPROG_TRANSPORT_KIND_NONE
#define FLPROG_TRANSPORT_KIND_NONE FLPROG_TRANSPORT_NONE
#endif
#ifndef FLPROG_TRANSPORT_KIND_WIFI
#define FLPROG_TRANSPORT_KIND_WIFI FLPROG_TRANSPORT_WIFI
#endif
#ifndef FLPROG_TRANSPORT_KIND_WIZNET_LEGACY
#define FLPROG_TRANSPORT_KIND_WIZNET_LEGACY FLPROG_TRANSPORT_WIZNET
#endif
#ifndef FLPROG_TRANSPORT_KIND_RMII
#define FLPROG_TRANSPORT_KIND_RMII FLPROG_TRANSPORT_RMII
#endif
#ifndef FLPROG_TRANSPORT_KIND_WIFI_AP
#define FLPROG_TRANSPORT_KIND_WIFI_AP FLPROG_TRANSPORT_WIFI_AP
#endif
#ifndef FLPROG_TRANSPORT_KIND_W5500
#define FLPROG_TRANSPORT_KIND_W5500 5
#endif
#ifndef FLPROG_TRANSPORT_KIND_W6100
#define FLPROG_TRANSPORT_KIND_W6100 6
#endif
// Reserved identity for a future optical transport. It is intentionally not
// part of FLPROG_TRANSPORT_COUNT: until a physical backend exists, no socket
// or route may resolve to it.
#ifndef FLPROG_TRANSPORT_KIND_OPTICAL
#define FLPROG_TRANSPORT_KIND_OPTICAL 7
#endif

struct FLProgTransportIdentity
{
    FLProgTransportKind kind = FLPROG_TRANSPORT_NONE;
    FLProgTransportInstanceId instanceId = 0;

    bool operator==(const FLProgTransportIdentity &other) const
    {
        return kind == other.kind && instanceId == other.instanceId;
    }

    bool operator!=(const FLProgTransportIdentity &other) const
    {
        return !(*this == other);
    }

    bool valid() const
    {
        return kind != FLPROG_TRANSPORT_NONE &&
               instanceId != FLPROG_TRANSPORT_INSTANCE_INVALID;
    }
};

#ifndef FLPROG_TRANSPORT_LINK_UNKNOWN
#define FLPROG_TRANSPORT_LINK_UNKNOWN 0
#endif
#ifndef FLPROG_TRANSPORT_LINK_ON
#define FLPROG_TRANSPORT_LINK_ON 1
#endif
#ifndef FLPROG_TRANSPORT_LINK_OFF
#define FLPROG_TRANSPORT_LINK_OFF 2
#endif

#ifndef FLPROG_TRANSPORT_EVENT_NONE
#define FLPROG_TRANSPORT_EVENT_NONE 0x00
#endif
#ifndef FLPROG_TRANSPORT_EVENT_READY_UP
#define FLPROG_TRANSPORT_EVENT_READY_UP 0x01
#endif
#ifndef FLPROG_TRANSPORT_EVENT_READY_DOWN
#define FLPROG_TRANSPORT_EVENT_READY_DOWN 0x02
#endif
#ifndef FLPROG_TRANSPORT_EVENT_IP_CHANGED
#define FLPROG_TRANSPORT_EVENT_IP_CHANGED 0x04
#endif
#ifndef FLPROG_TRANSPORT_EVENT_LINK_CHANGED
#define FLPROG_TRANSPORT_EVENT_LINK_CHANGED 0x08
#endif

#ifndef FLPROG_SOCKET_INVALID_HANDLE
#define FLPROG_SOCKET_INVALID_HANDLE 255
#endif
#ifndef FLPROG_SOCKET_NATIVE_NONE
#define FLPROG_SOCKET_NATIVE_NONE -1
#endif
#ifndef FLPROG_SOCKET_MAX_LEASES
#define FLPROG_SOCKET_MAX_LEASES 16
#endif

typedef bool (*FLProgTransportReadyReader)(void *context);
typedef IPAddress (*FLProgTransportIpReader)(void *context);
typedef uint8_t (*FLProgTransportByteReader)(void *context);

// -----------------------------------------------------------------------------
// Base transport
// -----------------------------------------------------------------------------

class FLProgNetworkTransport
{
public:
    virtual ~FLProgNetworkTransport() {}

    void configure(
        FLProgTransportKind transportKind,
        FLProgTransportInstanceId transportInstanceId,
        void *context,
        FLProgTransportReadyReader readyReader,
        FLProgTransportIpReader ipReader,
        FLProgTransportByteReader linkReader,
        FLProgTransportByteReader socketLimitReader = nullptr);

    // Refresh the cached transport snapshot and create change events.
    virtual void pool();

    FLProgTransportKind kind() const { return _transportKind; }
    FLProgTransportInstanceId instanceId() const { return _transportInstanceId; }
    FLProgTransportIdentity identity() const
    {
        FLProgTransportIdentity result;
        result.kind = _transportKind;
        result.instanceId = _transportInstanceId;
        return result;
    }
    bool matchesIdentity(
        FLProgTransportKind transportKind,
        FLProgTransportInstanceId transportInstanceId) const
    {
        return _transportKind == transportKind &&
               _transportInstanceId == transportInstanceId;
    }

    // Backward-compatible names.  In RC6-A.3 id() means transport kind and
    // interfaceId() means transport instance id.  New code should use
    // kind()/instanceId().
    uint8_t id() const { return kind(); }
    uint8_t interfaceId() const { return instanceId(); }

    bool isReady() const { return _ready; }
    IPAddress localIP() const { return _ip; }
    uint8_t linkStatus() const { return _linkStatus; }

    // Monotonic session identity. It changes whenever already-created socket
    // sessions may no longer be valid: ready up/down, local IP change, or an
    // explicit transport restart/reconfigure. Protocol sockets compare the
    // captured lease generation with this value to detect STALE sessions.
    uint32_t generation() const { return _generation; }

    uint8_t events() const { return _events; }
    uint8_t takeEvents()
    {
        const uint8_t value = _events;
        _events = FLPROG_TRANSPORT_EVENT_NONE;
        return value;
    }

    // resetSnapshot() clears the cached transport snapshot and is used when the
    // transport object itself is configured/rebound.
    void resetSnapshot();

    // Invalidate only socket/session identity. This MUST NOT change the cached
    // operational READY/IP/link snapshot: restarting or reconfiguring a native
    // interface must not make public READY falsely drop to 0 while the physical
    // link is still up. Existing leases become STALE through generation only.
    void invalidateSessions();

    virtual bool usesNativeSocketPool() const { return false; }
    virtual uint8_t nativeSocketLimit() const;

    // Native socket reservation is meaningful for Wiznet. lwIP-based
    // transports (Wi-Fi/RMII) leave native allocation to the software stack.
    virtual int16_t reserveNativeSocket(const void *owner, int16_t preferredSocket = FLPROG_SOCKET_NATIVE_NONE);
    virtual bool releaseNativeSocket(int16_t socket, const void *owner);
    virtual void releaseNativeSockets(const void *owner);
    virtual const void *nativeSocketOwner(int16_t socket) const;

protected:
    void *_context = nullptr;
    FLProgTransportReadyReader _readyReader = nullptr;
    FLProgTransportIpReader _ipReader = nullptr;
    FLProgTransportByteReader _linkReader = nullptr;
    FLProgTransportByteReader _socketLimitReader = nullptr;

    FLProgTransportKind _transportKind = FLPROG_TRANSPORT_NONE;
    FLProgTransportInstanceId _transportInstanceId = 0;

    bool _snapshotInitialized = false;
    bool _ready = false;
    IPAddress _ip = IPAddress(0, 0, 0, 0);
    uint8_t _linkStatus = FLPROG_TRANSPORT_LINK_UNKNOWN;

    uint8_t _events = FLPROG_TRANSPORT_EVENT_NONE;
    uint32_t _generation = 0;

    void advanceGeneration();
};

// -----------------------------------------------------------------------------
// Concrete transport adapters
// -----------------------------------------------------------------------------

class FLProgWiFiTransport : public FLProgNetworkTransport
{
public:
    void begin(
        FLProgTransportInstanceId transportInstanceId,
        void *context,
        FLProgTransportReadyReader readyReader,
        FLProgTransportIpReader ipReader,
        FLProgTransportByteReader linkReader)
    {
        configure(
            FLPROG_TRANSPORT_WIFI,
            transportInstanceId,
            context,
            readyReader,
            ipReader,
            linkReader,
            nullptr
        );
    }
};

class FLProgWiFiApTransport : public FLProgNetworkTransport
{
public:
    void begin(
        FLProgTransportInstanceId transportInstanceId,
        void *context,
        FLProgTransportReadyReader readyReader,
        FLProgTransportIpReader ipReader,
        FLProgTransportByteReader linkReader)
    {
        configure(
            FLPROG_TRANSPORT_WIFI_AP,
            transportInstanceId,
            context,
            readyReader,
            ipReader,
            linkReader,
            nullptr
        );
    }
};

class FLProgWiznetTransport : public FLProgNetworkTransport
{
public:
    FLProgWiznetTransport();

    void begin(
        FLProgTransportInstanceId transportInstanceId,
        void *context,
        FLProgTransportReadyReader readyReader,
        FLProgTransportIpReader ipReader,
        FLProgTransportByteReader linkReader,
        FLProgTransportByteReader socketLimitReader)
    {
        configure(
            FLPROG_TRANSPORT_WIZNET,
            transportInstanceId,
            context,
            readyReader,
            ipReader,
            linkReader,
            socketLimitReader
        );
    }

    bool usesNativeSocketPool() const override { return true; }
    int16_t reserveNativeSocket(const void *owner, int16_t preferredSocket = FLPROG_SOCKET_NATIVE_NONE) override;
    bool releaseNativeSocket(int16_t socket, const void *owner) override;
    void releaseNativeSockets(const void *owner) override;
    const void *nativeSocketOwner(int16_t socket) const override;
    uint8_t nativeSocketReservedCount() const;
    uint8_t nativeSocketAvailableCount() const;
    bool nativeSocketReserved(int16_t socket) const { return nativeSocketOwner(socket) != nullptr; }

private:
    const void *_nativeOwners[8];
};

class FLProgRmiiTransport : public FLProgNetworkTransport
{
public:
    void begin(
        FLProgTransportInstanceId transportInstanceId,
        void *context,
        FLProgTransportReadyReader readyReader,
        FLProgTransportIpReader ipReader,
        FLProgTransportByteReader linkReader)
    {
        configure(
            FLPROG_TRANSPORT_RMII,
            transportInstanceId,
            context,
            readyReader,
            ipReader,
            linkReader,
            nullptr
        );
    }
};

// -----------------------------------------------------------------------------
// Socket manager internal diagnostics
// -----------------------------------------------------------------------------

enum FLProgSocketManagerFailure : uint8_t
{
    FLPROG_SOCKET_MANAGER_FAILURE_NONE = 0,
    FLPROG_SOCKET_MANAGER_FAILURE_INVALID_OWNER = 1,
    FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT = 2,
    FLPROG_SOCKET_MANAGER_FAILURE_NOT_READY = 3,
    FLPROG_SOCKET_MANAGER_FAILURE_LEASE_FULL = 4,
    FLPROG_SOCKET_MANAGER_FAILURE_NATIVE_FULL = 5,
    FLPROG_SOCKET_MANAGER_FAILURE_INVALID_HANDLE = 6,
    FLPROG_SOCKET_MANAGER_FAILURE_OWNER_MISMATCH = 7
};

// -----------------------------------------------------------------------------
// Logical socket lease
// -----------------------------------------------------------------------------

struct FLProgSocketLease
{
    bool used = false;
    const void *owner = nullptr;

    // RC6-A.3: a lease is bound to the exact transport identity.
    FLProgTransportKind transportKind = FLPROG_TRANSPORT_NONE;
    FLProgTransportInstanceId transportInstanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    // Backward-compatible mirror of transportKind. Existing diagnostics and
    // protocol code may continue to read transportId while A3 migration is in
    // progress. New code should use transportIdentity().
    uint8_t transportId = FLPROG_TRANSPORT_NONE;

    FLProgTransportIdentity transportIdentity() const
    {
        FLProgTransportIdentity result;
        result.kind = transportKind;
        result.instanceId = transportInstanceId;
        return result;
    }

    int16_t nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    int16_t requestedNativeSocket = FLPROG_SOCKET_NATIVE_NONE;

    // Generation of the transport at the moment the protocol socket was
    // attached. A mismatch means the future TCP/UDP object must reopen.
    uint32_t transportGeneration = 0;
};

class FLProgSocketManager
{
public:
    FLProgSocketManager();

    bool registerTransport(FLProgNetworkTransport *transport);

    // RC6-A.3 exact lookup by transport identity.
    FLProgNetworkTransport *transportByIdentity(
        FLProgTransportKind kind,
        FLProgTransportInstanceId instanceId);
    const FLProgNetworkTransport *transportByIdentity(
        FLProgTransportKind kind,
        FLProgTransportInstanceId instanceId) const;

    FLProgNetworkTransport *transportByIdentity(
        const FLProgTransportIdentity &identity)
    {
        return transportByIdentity(identity.kind, identity.instanceId);
    }
    const FLProgNetworkTransport *transportByIdentity(
        const FLProgTransportIdentity &identity) const
    {
        return transportByIdentity(identity.kind, identity.instanceId);
    }

    // Legacy kind-only lookup.  It prefers instance 0.  If instance 0 is not
    // registered it falls back to the first registered instance of that kind,
    // preserving compatibility with pre-A3 custom transports.
    FLProgNetworkTransport *transportById(uint8_t id);
    const FLProgNetworkTransport *transportById(uint8_t id) const;

    uint8_t transportCount() const;
    uint8_t transportInstanceCount(FLProgTransportKind kind) const;

    // Reserve a logical protocol socket.
    //
    // requireReady=true:
    //   fail when the selected transport is not ready.
    //
    // requireReady=false:
    //   create a logical lease in advance. For Wiznet, the hardware socket is
    //   assigned later by activate() when the transport becomes available.
    uint8_t acquire(
        const void *owner,
        const FLProgTransportIdentity &transportIdentity,
        bool requireReady = true,
        int16_t preferredNativeSocket = FLPROG_SOCKET_NATIVE_NONE);

    // Legacy kind-only acquire remains source-compatible and binds to
    // instance #0, matching pre-A3 behavior.
    uint8_t acquire(
        const void *owner,
        uint8_t transportId,
        bool requireReady = true,
        int16_t preferredNativeSocket = FLPROG_SOCKET_NATIVE_NONE);

    // Attach/re-attach a lease to the current transport generation.
    // For Wiznet this also reserves one of the native hardware sockets.
    bool activate(uint8_t handle, const void *owner);

    bool release(uint8_t handle, const void *owner = nullptr);
    uint8_t releaseOwner(const void *owner);
    void clear();

    bool isValid(uint8_t handle) const;
    bool isStale(uint8_t handle) const;

    const FLProgSocketLease *lease(uint8_t handle) const;
    FLProgSocketLease *lease(uint8_t handle);

    FLProgTransportIdentity transportIdentity(uint8_t handle) const;
    FLProgTransportInstanceId transportInstanceId(uint8_t handle) const;
    uint8_t transportId(uint8_t handle) const;
    int16_t nativeSocket(uint8_t handle) const;
    const void *owner(uint8_t handle) const;

    uint8_t activeLeaseCount() const;
    uint8_t activeLeaseCount(uint8_t transportId) const;
    uint8_t leaseLimit() const { return FLPROG_SOCKET_MAX_LEASES; }
    uint8_t freeLeaseCount() const
    {
        const uint8_t used = activeLeaseCount();
        return used >= FLPROG_SOCKET_MAX_LEASES
            ? 0
            : (uint8_t)(FLPROG_SOCKET_MAX_LEASES - used);
    }
    uint8_t freeLeaseCount(uint8_t transportId) const;

    // Advanced/internal diagnostic. Public sockets map LEASE_FULL and
    // NATIVE_FULL to FLPROG_NETWORK_SOCKET_ERROR_RESOURCE_FULL.
    uint8_t lastFailure() const { return _lastFailure; }
    void clearFailure() { _lastFailure = FLPROG_SOCKET_MANAGER_FAILURE_NONE; }

private:
    void setFailure(uint8_t value) { _lastFailure = value; }
    FLProgNetworkTransport *_transports[FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES];
    FLProgSocketLease _leases[FLPROG_SOCKET_MAX_LEASES];
    uint8_t _lastFailure = FLPROG_SOCKET_MANAGER_FAILURE_NONE;
};
