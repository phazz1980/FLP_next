/*
 * FLProg_Network — flprogNetworkTransport.cpp
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
#include "flprogNetworkTransport.h"

// -----------------------------------------------------------------------------
// FLProgNetworkTransport
// -----------------------------------------------------------------------------

void FLProgNetworkTransport::configure(
    FLProgTransportKind transportKind,
    FLProgTransportInstanceId transportInstanceId,
    void *context,
    FLProgTransportReadyReader readyReader,
    FLProgTransportIpReader ipReader,
    FLProgTransportByteReader linkReader,
    FLProgTransportByteReader socketLimitReader)
{
    _transportKind = transportKind;
    _transportInstanceId = transportInstanceId;
    _context = context;
    _readyReader = readyReader;
    _ipReader = ipReader;
    _linkReader = linkReader;
    _socketLimitReader = socketLimitReader;

    resetSnapshot();
}

void FLProgNetworkTransport::advanceGeneration()
{
    _generation++;
    // Generation value 0 is reserved for "not captured / unknown" in
    // diagnostics. Keep the live generation non-zero even after wraparound.
    if (_generation == 0)
    {
        _generation = 1;
    }
}

void FLProgNetworkTransport::invalidateSessions()
{
    // Session invalidation is intentionally generation-only. READY/IP/link are
    // interface facts and must not be cleared just because existing sockets
    // have become stale. This separation prevents false READY=0 indications.
    advanceGeneration();
}

void FLProgNetworkTransport::resetSnapshot()
{
    // Rebinding the transport object invalidates sessions and clears the cached
    // snapshot. Keep generation monotonic so an old lease can never become
    // current again after a restart to the same READY/IP state.
    invalidateSessions();

    _snapshotInitialized = false;
    _ready = false;
    _ip = IPAddress(0, 0, 0, 0);
    _linkStatus = FLPROG_TRANSPORT_LINK_UNKNOWN;
    _events = FLPROG_TRANSPORT_EVENT_NONE;
}

void FLProgNetworkTransport::pool()
{
    const bool newReady =
        (_readyReader != nullptr)
            ? _readyReader(_context)
            : false;

    const IPAddress newIp =
        (_ipReader != nullptr)
            ? _ipReader(_context)
            : IPAddress(0, 0, 0, 0);

    const uint8_t newLink =
        (_linkReader != nullptr)
            ? _linkReader(_context)
            : FLPROG_TRANSPORT_LINK_UNKNOWN;

    if (!_snapshotInitialized)
    {
        _snapshotInitialized = true;
        _ready = newReady;
        _ip = newIp;
        _linkStatus = newLink;

        if (_ready)
        {
            _events |= FLPROG_TRANSPORT_EVENT_READY_UP;
        }

        return;
    }

    bool identityChanged = false;

    if (newReady != _ready)
    {
        _events |= newReady
            ? FLPROG_TRANSPORT_EVENT_READY_UP
            : FLPROG_TRANSPORT_EVENT_READY_DOWN;

        _ready = newReady;
        identityChanged = true;
    }

    if (newIp != _ip)
    {
        _ip = newIp;
        _events |= FLPROG_TRANSPORT_EVENT_IP_CHANGED;
        identityChanged = true;
    }

    if (newLink != _linkStatus)
    {
        _linkStatus = newLink;
        _events |= FLPROG_TRANSPORT_EVENT_LINK_CHANGED;
    }

    // One snapshot transition is one transport identity transition even when
    // ready and IP change together in the same cooperative pool() pass.
    if (identityChanged)
    {
        advanceGeneration();
    }
}

uint8_t FLProgNetworkTransport::nativeSocketLimit() const
{
    return (_socketLimitReader != nullptr)
        ? _socketLimitReader(_context)
        : 0;
}

int16_t FLProgNetworkTransport::reserveNativeSocket(
    const void *owner,
    int16_t preferredSocket)
{
    (void)owner;
    (void)preferredSocket;
    return FLPROG_SOCKET_NATIVE_NONE;
}

bool FLProgNetworkTransport::releaseNativeSocket(
    int16_t socket,
    const void *owner)
{
    (void)socket;
    (void)owner;
    return false;
}

void FLProgNetworkTransport::releaseNativeSockets(const void *owner)
{
    (void)owner;
}

const void *FLProgNetworkTransport::nativeSocketOwner(
    int16_t socket) const
{
    (void)socket;
    return nullptr;
}

// -----------------------------------------------------------------------------
// FLProgWiznetTransport
// -----------------------------------------------------------------------------

FLProgWiznetTransport::FLProgWiznetTransport()
{
    for (uint8_t i = 0; i < 8; i++)
    {
        _nativeOwners[i] = nullptr;
    }
}

int16_t FLProgWiznetTransport::reserveNativeSocket(
    const void *owner,
    int16_t preferredSocket)
{
    if (owner == nullptr)
    {
        return FLPROG_SOCKET_NATIVE_NONE;
    }

    uint8_t limit = nativeSocketLimit();
    if (limit > 8)
    {
        limit = 8;
    }

    if (preferredSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        if (preferredSocket < 0 || preferredSocket >= limit)
        {
            return FLPROG_SOCKET_NATIVE_NONE;
        }

        const uint8_t socket = (uint8_t)preferredSocket;

        if (_nativeOwners[socket] != nullptr)
        {
            return FLPROG_SOCKET_NATIVE_NONE;
        }

        _nativeOwners[socket] = owner;
        return socket;
    }

    for (uint8_t socket = 0; socket < limit; socket++)
    {
        if (_nativeOwners[socket] == nullptr)
        {
            _nativeOwners[socket] = owner;
            return socket;
        }
    }

    return FLPROG_SOCKET_NATIVE_NONE;
}

bool FLProgWiznetTransport::releaseNativeSocket(
    int16_t socket,
    const void *owner)
{
    if (socket < 0 || socket >= 8)
    {
        return false;
    }

    const uint8_t index = (uint8_t)socket;

    if (_nativeOwners[index] == nullptr)
    {
        return false;
    }

    if (
        owner != nullptr &&
        _nativeOwners[index] != owner
    )
    {
        return false;
    }

    _nativeOwners[index] = nullptr;
    return true;
}

void FLProgWiznetTransport::releaseNativeSockets(
    const void *owner)
{
    if (owner == nullptr)
    {
        return;
    }

    for (uint8_t socket = 0; socket < 8; socket++)
    {
        if (_nativeOwners[socket] == owner)
        {
            _nativeOwners[socket] = nullptr;
        }
    }
}

const void *FLProgWiznetTransport::nativeSocketOwner(
    int16_t socket) const
{
    if (socket < 0 || socket >= 8)
    {
        return nullptr;
    }

    return _nativeOwners[(uint8_t)socket];
}

uint8_t FLProgWiznetTransport::nativeSocketReservedCount() const
{
    uint8_t limit = nativeSocketLimit();
    if (limit > 8) limit = 8;
    uint8_t count = 0;
    for (uint8_t socket = 0; socket < limit; socket++)
        if (_nativeOwners[socket] != nullptr) count++;
    return count;
}

uint8_t FLProgWiznetTransport::nativeSocketAvailableCount() const
{
    uint8_t limit = nativeSocketLimit();
    if (limit > 8) limit = 8;
    const uint8_t reserved = nativeSocketReservedCount();
    return reserved < limit ? (uint8_t)(limit - reserved) : 0;
}

// -----------------------------------------------------------------------------
// FLProgSocketManager
// -----------------------------------------------------------------------------

FLProgSocketManager::FLProgSocketManager()
{
    _lastFailure = FLPROG_SOCKET_MANAGER_FAILURE_NONE;

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        _transports[i] = nullptr;
    }
}

bool FLProgSocketManager::registerTransport(
    FLProgNetworkTransport *transport)
{
    if (transport == nullptr || !transport->identity().valid())
    {
        return false;
    }

    int8_t freeIndex = -1;

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        FLProgNetworkTransport *current = _transports[i];

        if (current == nullptr)
        {
            if (freeIndex < 0)
            {
                freeIndex = (int8_t)i;
            }
            continue;
        }

        // Re-registering the same object with the same identity is idempotent.
        if (current == transport)
        {
            return current->identity() == transport->identity();
        }

        // One identity may never resolve to two different transport objects.
        if (current->identity() == transport->identity())
        {
            return false;
        }
    }

    if (freeIndex < 0)
    {
        return false;
    }

    _transports[(uint8_t)freeIndex] = transport;
    return true;
}

FLProgNetworkTransport *FLProgSocketManager::transportByIdentity(
    FLProgTransportKind kind,
    FLProgTransportInstanceId instanceId)
{
    if (
        kind == FLPROG_TRANSPORT_NONE ||
        instanceId == FLPROG_TRANSPORT_INSTANCE_INVALID
    )
    {
        return nullptr;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        FLProgNetworkTransport *transport = _transports[i];
        if (
            transport != nullptr &&
            transport->matchesIdentity(kind, instanceId)
        )
        {
            return transport;
        }
    }

    return nullptr;
}

const FLProgNetworkTransport *FLProgSocketManager::transportByIdentity(
    FLProgTransportKind kind,
    FLProgTransportInstanceId instanceId) const
{
    if (
        kind == FLPROG_TRANSPORT_NONE ||
        instanceId == FLPROG_TRANSPORT_INSTANCE_INVALID
    )
    {
        return nullptr;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        const FLProgNetworkTransport *transport = _transports[i];
        if (
            transport != nullptr &&
            transport->matchesIdentity(kind, instanceId)
        )
        {
            return transport;
        }
    }

    return nullptr;
}

FLProgNetworkTransport *FLProgSocketManager::transportById(
    uint8_t id)
{
    FLProgNetworkTransport *transport = transportByIdentity(id, 0);
    if (transport != nullptr)
    {
        return transport;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        transport = _transports[i];
        if (transport != nullptr && transport->kind() == id)
        {
            return transport;
        }
    }

    return nullptr;
}

const FLProgNetworkTransport *FLProgSocketManager::transportById(
    uint8_t id) const
{
    const FLProgNetworkTransport *transport = transportByIdentity(id, 0);
    if (transport != nullptr)
    {
        return transport;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        transport = _transports[i];
        if (transport != nullptr && transport->kind() == id)
        {
            return transport;
        }
    }

    return nullptr;
}

uint8_t FLProgSocketManager::transportCount() const
{
    uint8_t count = 0;
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (_transports[i] != nullptr)
        {
            count++;
        }
    }
    return count;
}

uint8_t FLProgSocketManager::transportInstanceCount(
    FLProgTransportKind kind) const
{
    uint8_t count = 0;
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (_transports[i] != nullptr && _transports[i]->kind() == kind)
        {
            count++;
        }
    }
    return count;
}

uint8_t FLProgSocketManager::acquire(
    const void *ownerValue,
    const FLProgTransportIdentity &selectedIdentity,
    bool requireReady,
    int16_t preferredNativeSocket)
{
    clearFailure();

    if (ownerValue == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_INVALID_OWNER);
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    if (!selectedIdentity.valid())
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT);
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    FLProgNetworkTransport *transport =
        transportByIdentity(selectedIdentity);

    if (transport == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT);
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    if (requireReady && !transport->isReady())
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NOT_READY);
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    uint8_t freeHandle = FLPROG_SOCKET_INVALID_HANDLE;

    for (uint8_t i = 0; i < FLPROG_SOCKET_MAX_LEASES; i++)
    {
        if (!_leases[i].used)
        {
            freeHandle = i;
            break;
        }
    }

    if (freeHandle == FLPROG_SOCKET_INVALID_HANDLE)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_LEASE_FULL);
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    FLProgSocketLease &item = _leases[freeHandle];

    item.used = true;
    item.owner = ownerValue;
    item.transportKind = selectedIdentity.kind;
    item.transportInstanceId = selectedIdentity.instanceId;
    item.transportId = selectedIdentity.kind;
    item.nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    item.requestedNativeSocket = preferredNativeSocket;
    item.transportGeneration = transport->generation();

    if (transport->usesNativeSocketPool() && transport->isReady())
    {
        item.nativeSocket =
            transport->reserveNativeSocket(
                ownerValue,
                item.requestedNativeSocket);

        if (item.nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
        {
            item = FLProgSocketLease();
            setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NATIVE_FULL);
            return FLPROG_SOCKET_INVALID_HANDLE;
        }
    }

    return freeHandle;
}

uint8_t FLProgSocketManager::acquire(
    const void *ownerValue,
    uint8_t selectedTransport,
    bool requireReady,
    int16_t preferredNativeSocket)
{
    FLProgTransportIdentity identity;
    identity.kind = selectedTransport;
    identity.instanceId = 0;
    return acquire(
        ownerValue,
        identity,
        requireReady,
        preferredNativeSocket);
}

bool FLProgSocketManager::activate(
    uint8_t handle,
    const void *ownerValue)
{
    clearFailure();

    FLProgSocketLease *item = lease(handle);

    if (item == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_INVALID_HANDLE);
        return false;
    }

    if (ownerValue == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_INVALID_OWNER);
        return false;
    }

    if (item->owner != ownerValue)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_OWNER_MISMATCH);
        return false;
    }

    FLProgNetworkTransport *transport =
        transportByIdentity(item->transportIdentity());

    if (transport == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT);
        return false;
    }

    if (!transport->isReady())
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NOT_READY);
        return false;
    }

    if (transport->usesNativeSocketPool())
    {
        // A native slot is valid only while it is still owned by this lease
        // owner. Recover defensively if an external cleanup released it.
        if (
            item->nativeSocket != FLPROG_SOCKET_NATIVE_NONE &&
            transport->nativeSocketOwner(item->nativeSocket) != ownerValue
        )
        {
            item->nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
        }

        if (item->nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
        {
            item->nativeSocket =
                transport->reserveNativeSocket(
                    ownerValue,
                    item->requestedNativeSocket);

            if (item->nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
            {
                setFailure(FLPROG_SOCKET_MANAGER_FAILURE_NATIVE_FULL);
                return false;
            }
        }
    }

    item->transportGeneration = transport->generation();
    return true;
}

bool FLProgSocketManager::release(
    uint8_t handle,
    const void *ownerValue)
{
    clearFailure();

    FLProgSocketLease *item = lease(handle);

    if (item == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_INVALID_HANDLE);
        return false;
    }

    if (
        ownerValue != nullptr &&
        item->owner != ownerValue
    )
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_OWNER_MISMATCH);
        return false;
    }

    FLProgNetworkTransport *transport =
        transportByIdentity(item->transportIdentity());

    if (
        transport != nullptr &&
        item->nativeSocket != FLPROG_SOCKET_NATIVE_NONE
    )
    {
        transport->releaseNativeSocket(
            item->nativeSocket,
            item->owner
        );
    }

    *item = FLProgSocketLease();
    return true;
}

uint8_t FLProgSocketManager::releaseOwner(
    const void *ownerValue)
{
    clearFailure();

    if (ownerValue == nullptr)
    {
        setFailure(FLPROG_SOCKET_MANAGER_FAILURE_INVALID_OWNER);
        return 0;
    }

    uint8_t count = 0;

    for (uint8_t i = 0; i < FLPROG_SOCKET_MAX_LEASES; i++)
    {
        if (
            _leases[i].used &&
            _leases[i].owner == ownerValue
        )
        {
            if (release(i, ownerValue))
            {
                count++;
            }
        }
    }

    return count;
}

void FLProgSocketManager::clear()
{
    clearFailure();

    for (uint8_t i = 0; i < FLPROG_SOCKET_MAX_LEASES; i++)
    {
        if (_leases[i].used)
        {
            release(i, nullptr);
        }
    }
}

bool FLProgSocketManager::isValid(uint8_t handle) const
{
    return
        handle < FLPROG_SOCKET_MAX_LEASES &&
        _leases[handle].used;
}

bool FLProgSocketManager::isStale(uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);

    if (item == nullptr)
    {
        return true;
    }

    const FLProgNetworkTransport *transport =
        transportByIdentity(item->transportIdentity());

    if (transport == nullptr || !transport->isReady())
    {
        return true;
    }

    return
        item->transportGeneration !=
        transport->generation();
}

const FLProgSocketLease *FLProgSocketManager::lease(
    uint8_t handle) const
{
    if (!isValid(handle))
    {
        return nullptr;
    }

    return &_leases[handle];
}

FLProgSocketLease *FLProgSocketManager::lease(
    uint8_t handle)
{
    if (!isValid(handle))
    {
        return nullptr;
    }

    return &_leases[handle];
}

FLProgTransportIdentity FLProgSocketManager::transportIdentity(
    uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);
    if (item != nullptr)
    {
        return item->transportIdentity();
    }

    FLProgTransportIdentity result;
    result.kind = FLPROG_TRANSPORT_NONE;
    result.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    return result;
}

FLProgTransportInstanceId FLProgSocketManager::transportInstanceId(
    uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);
    return (item != nullptr)
        ? item->transportInstanceId
        : FLPROG_TRANSPORT_INSTANCE_INVALID;
}

uint8_t FLProgSocketManager::transportId(
    uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);
    return (item != nullptr)
        ? item->transportId
        : FLPROG_TRANSPORT_NONE;
}

int16_t FLProgSocketManager::nativeSocket(
    uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);
    return (item != nullptr)
        ? item->nativeSocket
        : FLPROG_SOCKET_NATIVE_NONE;
}

const void *FLProgSocketManager::owner(
    uint8_t handle) const
{
    const FLProgSocketLease *item = lease(handle);
    return (item != nullptr)
        ? item->owner
        : nullptr;
}

uint8_t FLProgSocketManager::activeLeaseCount() const
{
    uint8_t count = 0;

    for (uint8_t i = 0; i < FLPROG_SOCKET_MAX_LEASES; i++)
    {
        if (_leases[i].used)
        {
            count++;
        }
    }

    return count;
}

uint8_t FLProgSocketManager::activeLeaseCount(
    uint8_t selectedTransport) const
{
    uint8_t count = 0;

    for (uint8_t i = 0; i < FLPROG_SOCKET_MAX_LEASES; i++)
    {
        if (
            _leases[i].used &&
            _leases[i].transportId == selectedTransport
        )
        {
            count++;
        }
    }

    return count;
}

uint8_t FLProgSocketManager::freeLeaseCount(
    uint8_t selectedTransport) const
{
    const FLProgNetworkTransport *transport = transportById(selectedTransport);
    if (transport == nullptr)
    {
        return 0;
    }

    const uint8_t globalFree = freeLeaseCount();
    if (!transport->usesNativeSocketPool())
    {
        return globalFree;
    }

    const FLProgWiznetTransport *wiznet =
        static_cast<const FLProgWiznetTransport *>(transport);
    const uint8_t nativeFree = wiznet->nativeSocketAvailableCount();
    return globalFree < nativeFree ? globalFree : nativeFree;
}
