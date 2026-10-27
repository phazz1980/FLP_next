/*
 * FLProg_Network — flprogNetworkSocket.cpp
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
#include "flprogNetworkSocket.h"
#include "../flprogNetwork.h"

FLProgNetworkSocket::FLProgNetworkSocket()
{
    for (uint8_t i = 0; i < FLPROG_TRANSPORT_COUNT; i++)
    {
        _backends[i] = nullptr;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        _backendBindings[i].identity.kind = FLPROG_TRANSPORT_NONE;
        _backendBindings[i].identity.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
        _backendBindings[i].backend = nullptr;
    }
}

FLProgNetworkSocket::~FLProgNetworkSocket()
{
    end();
}

bool FLProgNetworkSocket::begin(
    FLProgNetworkInterface &network,
    uint8_t routeValue,
    uint8_t protocolValue)
{
    if (_network == &network)
    {
        setRoute(routeValue);
        setProtocol(protocolValue);
        return true;
    }

    end();

    _network = &network;
    _route = routeValue;
    _protocol = protocolValue;
    _directTransportIdentity.kind = FLPROG_TRANSPORT_NONE;
    _directTransportIdentity.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    if (!_network->registerManagedSocket(this))
    {
        _network = nullptr;
        setError(FLPROG_SOCKET_ERROR_RESOURCE_FULL);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return false;
    }

    _error = FLPROG_SOCKET_ERROR_NONE;
    _lastError = FLPROG_SOCKET_ERROR_NONE;
    _wasConnected = false;
    _hadAvailableData = false;
    setState(FLPROG_SOCKET_STATE_CLOSED);
    return true;
}

bool FLProgNetworkSocket::begin(
    FLProgNetworkInterface &network,
    const FLProgTransportIdentity &transportIdentityValue,
    uint8_t protocolValue)
{
    if (!transportIdentityValue.valid())
    {
        setError(FLPROG_SOCKET_ERROR_NO_TRANSPORT);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return false;
    }

    end();

    _network = &network;
    _route = FLPROG_NETWORK_ROUTE_DEFAULT;
    _protocol = protocolValue;
    _directTransportIdentity = transportIdentityValue;

    if (!_network->registerManagedSocket(this))
    {
        _network = nullptr;
        _directTransportIdentity.kind = FLPROG_TRANSPORT_NONE;
        _directTransportIdentity.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
        setError(FLPROG_SOCKET_ERROR_RESOURCE_FULL);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return false;
    }

    _error = FLPROG_SOCKET_ERROR_NONE;
    _lastError = FLPROG_SOCKET_ERROR_NONE;
    _wasConnected = false;
    _hadAvailableData = false;
    changeBoundTransport(transportIdentityValue);
    setState(FLPROG_SOCKET_STATE_CLOSED);
    return true;
}

void FLProgNetworkSocket::end()
{
    if (_network == nullptr)
    {
        return;
    }

    abort();

    FLProgNetworkInterface *network = _network;

    // Lifetime safety: abort() releases the normal lease. releaseOwnerSockets()
    // is a defensive owner-scoped cleanup for any orphan lease left by a
    // partially completed backend/rebind path. It cannot release resources
    // owned by other sockets.
    network->releaseOwnerSockets(this);

    _network = nullptr;
    _directTransportIdentity.kind = FLPROG_TRANSPORT_NONE;
    _directTransportIdentity.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    network->unregisterManagedSocket(this);
}

void FLProgNetworkSocket::setRoute(uint8_t value)
{
    const bool hadDirectBinding = _directTransportIdentity.valid();

    if (_route == value && !hadDirectBinding)
    {
        return;
    }

    _route = value;
    _directTransportIdentity.kind = FLPROG_TRANSPORT_NONE;
    _directTransportIdentity.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    if (_wantOpen)
    {
        requestRebind();
    }
}

uint8_t FLProgNetworkSocket::routeClass() const
{
    if (_directTransportIdentity.valid())
    {
        return FLPROG_SOCKET_ROUTE_CLASS_FIXED;
    }

    if (
        _route == FLPROG_NETWORK_ROUTE_WIFI ||
        _route == FLPROG_NETWORK_ROUTE_WIFI_AP ||
        _route == FLPROG_NETWORK_ROUTE_WIZNET ||
        _route == FLPROG_NETWORK_ROUTE_RMII
    )
    {
        return FLPROG_SOCKET_ROUTE_CLASS_FIXED;
    }

    if (_route == FLPROG_NETWORK_ROUTE_ETHERNET)
    {
        return FLPROG_SOCKET_ROUTE_CLASS_WIRED;
    }

    return FLPROG_SOCKET_ROUTE_CLASS_DYNAMIC;
}

uint8_t FLProgNetworkSocket::resolvedRoute() const
{
    switch (_boundTransportId)
    {
        case FLPROG_TRANSPORT_WIFI:
            return FLPROG_NETWORK_ROUTE_WIFI;
        case FLPROG_TRANSPORT_WIFI_AP:
            return FLPROG_NETWORK_ROUTE_WIFI_AP;
        case FLPROG_TRANSPORT_WIZNET:
            return FLPROG_NETWORK_ROUTE_WIZNET;
        case FLPROG_TRANSPORT_RMII:
            return FLPROG_NETWORK_ROUTE_RMII;
        default:
            return FLPROG_NETWORK_ROUTE_DEFAULT;
    }
}

uint32_t FLProgNetworkSocket::transportGeneration() const
{
    if (
        _network != nullptr &&
        _leaseHandle != FLPROG_SOCKET_INVALID_HANDLE
    )
    {
        const FLProgSocketLease *item =
            _network->socketManager().lease(_leaseHandle);
        if (item != nullptr)
        {
            return item->transportGeneration;
        }
    }

    const FLProgNetworkTransport *transport = boundTransport();
    return (transport != nullptr) ? transport->generation() : 0;
}

void FLProgNetworkSocket::requestRebind()
{
    if (_network == nullptr)
    {
        return;
    }

    _rebindRequested = true;
    _events |= FLPROG_SOCKET_EVENT_REBIND_REQUESTED;
}

void FLProgNetworkSocket::setProtocol(uint8_t value)
{
    if (_protocol == value)
    {
        return;
    }

    _protocol = value;

    if (_wantOpen)
    {
        requestRebind();
    }
}

void FLProgNetworkSocket::setBackend(
    uint8_t transportIdValue,
    FLProgSocketBackend *value)
{
    if (
        transportIdValue == FLPROG_TRANSPORT_NONE ||
        transportIdValue >= FLPROG_TRANSPORT_COUNT
    )
    {
        return;
    }

    _backends[transportIdValue] = value;

    FLProgTransportIdentity identity;
    identity.kind = transportIdValue;
    identity.instanceId = 0;
    setBackend(identity, value);
}

void FLProgNetworkSocket::setBackend(
    const FLProgTransportIdentity &transportIdentityValue,
    FLProgSocketBackend *value)
{
    if (!transportIdentityValue.valid())
    {
        return;
    }

    // Update an existing exact slot first.
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (_backendBindings[i].identity == transportIdentityValue)
        {
            _backendBindings[i].backend = value;

            if (
                transportIdentityValue.instanceId == 0 &&
                transportIdentityValue.kind < FLPROG_TRANSPORT_COUNT
            )
            {
                _backends[transportIdentityValue.kind] = value;
            }

            if (
                _wantOpen &&
                transportIdentity() == transportIdentityValue &&
                _state == FLPROG_SOCKET_STATE_UNSUPPORTED
            )
            {
                clearActiveError();
                setState(FLPROG_SOCKET_STATE_RESERVED);
            }
            return;
        }
    }

    // Null only clears an existing mapping; it does not consume a new slot.
    if (value == nullptr)
    {
        if (
            transportIdentityValue.instanceId == 0 &&
            transportIdentityValue.kind < FLPROG_TRANSPORT_COUNT
        )
        {
            _backends[transportIdentityValue.kind] = nullptr;
        }
        return;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (!_backendBindings[i].identity.valid())
        {
            _backendBindings[i].identity = transportIdentityValue;
            _backendBindings[i].backend = value;

            if (
                transportIdentityValue.instanceId == 0 &&
                transportIdentityValue.kind < FLPROG_TRANSPORT_COUNT
            )
            {
                _backends[transportIdentityValue.kind] = value;
            }

            if (
                _wantOpen &&
                transportIdentity() == transportIdentityValue &&
                _state == FLPROG_SOCKET_STATE_UNSUPPORTED
            )
            {
                clearActiveError();
                setState(FLPROG_SOCKET_STATE_RESERVED);
            }
            return;
        }
    }
}

FLProgSocketBackend *FLProgNetworkSocket::backend(uint8_t transportIdValue)
{
    if (transportIdValue >= FLPROG_TRANSPORT_COUNT)
    {
        return nullptr;
    }

    return _backends[transportIdValue];
}

const FLProgSocketBackend *FLProgNetworkSocket::backend(
    uint8_t transportIdValue) const
{
    if (transportIdValue >= FLPROG_TRANSPORT_COUNT)
    {
        return nullptr;
    }

    return _backends[transportIdValue];
}

FLProgSocketBackend *FLProgNetworkSocket::backend(
    const FLProgTransportIdentity &transportIdentityValue)
{
    if (!transportIdentityValue.valid())
    {
        return nullptr;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (_backendBindings[i].identity == transportIdentityValue)
        {
            return _backendBindings[i].backend;
        }
    }

    // Pre-A3 registrations are kind-only and represent instance #0.
    if (
        transportIdentityValue.instanceId == 0 &&
        transportIdentityValue.kind < FLPROG_TRANSPORT_COUNT
    )
    {
        return _backends[transportIdentityValue.kind];
    }

    return nullptr;
}

const FLProgSocketBackend *FLProgNetworkSocket::backend(
    const FLProgTransportIdentity &transportIdentityValue) const
{
    if (!transportIdentityValue.valid())
    {
        return nullptr;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_TRANSPORT_INSTANCES; i++)
    {
        if (_backendBindings[i].identity == transportIdentityValue)
        {
            return _backendBindings[i].backend;
        }
    }

    if (
        transportIdentityValue.instanceId == 0 &&
        transportIdentityValue.kind < FLPROG_TRANSPORT_COUNT
    )
    {
        return _backends[transportIdentityValue.kind];
    }

    return nullptr;
}

void FLProgNetworkSocket::setState(uint8_t value)
{
    if (_state == value)
    {
        return;
    }

    const uint8_t oldState = _state;
    _state = value;
    _stateStarted = millis();

    if (value == FLPROG_SOCKET_STATE_ERROR)
    {
        _events |= FLPROG_SOCKET_EVENT_ERROR;
    }
    else if (value == FLPROG_SOCKET_STATE_STALE)
    {
        _events |= FLPROG_SOCKET_EVENT_STALE;
    }

    onStateChanged(oldState, value);
}

void FLProgNetworkSocket::setError(
    uint8_t value,
    bool remember)
{
    _error = value;

    if (
        remember &&
        value != FLPROG_SOCKET_ERROR_NONE
    )
    {
        _lastError = value;
    }
}

void FLProgNetworkSocket::rememberError(uint8_t value)
{
    if (value != FLPROG_SOCKET_ERROR_NONE)
    {
        _lastError = value;
    }
}

void FLProgNetworkSocket::clearActiveError()
{
    _error = FLPROG_SOCKET_ERROR_NONE;
}

void FLProgNetworkSocket::updatePublicEvents()
{
    const bool nowConnected = connected();

    if (nowConnected && !_wasConnected)
    {
        _events |= FLPROG_SOCKET_EVENT_CONNECTED;
    }
    else if (!nowConnected && _wasConnected)
    {
        _events |= FLPROG_SOCKET_EVENT_DISCONNECTED;
    }

    _wasConnected = nowConnected;

    bool hasData = false;
    if (nowConnected && isOpen() && _activeBackend != nullptr)
    {
        hasData = _activeBackend->available() > 0;
    }

    if (hasData && !_hadAvailableData)
    {
        _events |= FLPROG_SOCKET_EVENT_DATA_AVAILABLE;
    }

    _hadAvailableData = hasData;
}

uint8_t FLProgNetworkSocket::requestOpen()
{
    if (_network == nullptr)
    {
        setError(FLPROG_SOCKET_ERROR_NO_NETWORK);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _wantOpen = true;

    if (
        _state == FLPROG_SOCKET_STATE_ERROR ||
        _state == FLPROG_SOCKET_STATE_UNSUPPORTED
    )
    {
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_REOPEN_REQUIRED);
    }

    return isOpen()
        ? FLPROG_SOCKET_RESULT_SUCCESS
        : FLPROG_SOCKET_RESULT_WAIT;
}

uint8_t FLProgNetworkSocket::requestClose()
{
    _wantOpen = false;

    return (_state == FLPROG_SOCKET_STATE_CLOSED)
        ? FLPROG_SOCKET_RESULT_SUCCESS
        : FLPROG_SOCKET_RESULT_WAIT;
}

void FLProgNetworkSocket::abort()
{
    const bool wasConnected = _wasConnected || connected();
    _wantOpen = false;

    if (_activeBackend != nullptr)
    {
        _activeBackend->close();
    }

    _activeBackend = nullptr;
    releaseLease();

    _boundTransportId = FLPROG_TRANSPORT_NONE;
    _boundTransportInstanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    _previousTransportId = FLPROG_TRANSPORT_NONE;
    _previousTransportInstanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    _rebindRequested = false;
    _reopenCycle = false;
    clearActiveError();
    setState(FLPROG_SOCKET_STATE_CLOSED);

    if (wasConnected)
    {
        _events |= FLPROG_SOCKET_EVENT_DISCONNECTED;
    }
    _wasConnected = false;
    _hadAvailableData = false;
}

void FLProgNetworkSocket::releaseLease()
{
    if (
        _network != nullptr &&
        _leaseHandle != FLPROG_SOCKET_INVALID_HANDLE
    )
    {
        _network->socketManager().release(_leaseHandle, this);
    }

    _leaseHandle = FLPROG_SOCKET_INVALID_HANDLE;
}

FLProgNetworkTransport *FLProgNetworkSocket::boundTransport()
{
    if (
        _network == nullptr ||
        _boundTransportId == FLPROG_TRANSPORT_NONE ||
        _boundTransportInstanceId == FLPROG_TRANSPORT_INSTANCE_INVALID
    )
    {
        return nullptr;
    }

    return _network->transportByIdentity(
        _boundTransportId,
        _boundTransportInstanceId);
}

const FLProgNetworkTransport *FLProgNetworkSocket::boundTransport() const
{
    if (
        _network == nullptr ||
        _boundTransportId == FLPROG_TRANSPORT_NONE ||
        _boundTransportInstanceId == FLPROG_TRANSPORT_INSTANCE_INVALID
    )
    {
        return nullptr;
    }

    return _network->transportByIdentity(
        _boundTransportId,
        _boundTransportInstanceId);
}

FLProgTransportIdentity FLProgNetworkSocket::selectTransportForOpen(
    bool requireReady)
{
    FLProgTransportIdentity result;
    result.kind = FLPROG_TRANSPORT_NONE;
    result.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    if (_network == nullptr)
    {
        return result;
    }

    _routeDecisionCount++;

    if (_directTransportIdentity.valid())
    {
        FLProgNetworkTransport *transport =
            _network->transportByIdentity(_directTransportIdentity);

        if (transport == nullptr)
        {
            return result;
        }

        if (requireReady && !transport->isReady())
        {
            return result;
        }

        return _directTransportIdentity;
    }

    return _network->transportIdentityForRoute(
        _route,
        requireReady
    );
}

void FLProgNetworkSocket::changeBoundTransport(
    const FLProgTransportIdentity &transportIdentityValue)
{
    FLProgTransportIdentity normalized = transportIdentityValue;
    if (!normalized.valid())
    {
        normalized.kind = FLPROG_TRANSPORT_NONE;
        normalized.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
    }

    FLProgTransportIdentity current;
    current.kind = _boundTransportId;
    current.instanceId = _boundTransportInstanceId;

    if (current == normalized)
    {
        return;
    }

    const FLProgTransportIdentity oldTransport = current;

    if (oldTransport.valid())
    {
        _previousTransportId = oldTransport.kind;
        _previousTransportInstanceId = oldTransport.instanceId;
    }

    _boundTransportId = normalized.kind;
    _boundTransportInstanceId = normalized.instanceId;

    FLProgTransportIdentity previous;
    previous.kind = _previousTransportId;
    previous.instanceId = _previousTransportInstanceId;

    if (
        !oldTransport.valid() &&
        normalized.valid() &&
        previous.valid() &&
        previous != normalized
    )
    {
        _transportChangeCount++;
        _events |= FLPROG_SOCKET_EVENT_TRANSPORT_CHANGED;
    }
}

bool FLProgNetworkSocket::updateDynamicRouteBeforeOpen()
{
    if (
        _network == nullptr ||
        !routeIsDynamic()
    )
    {
        return false;
    }

    const FLProgTransportIdentity readyTransport = selectTransportForOpen(true);

    if (
        !readyTransport.valid() ||
        readyTransport == transportIdentity()
    )
    {
        return false;
    }

    if (_activeBackend != nullptr)
    {
        return false;
    }

    releaseLease();
    changeBoundTransport(readyTransport);
    clearActiveError();
    setState(FLPROG_SOCKET_STATE_REOPEN_REQUIRED);
    return true;
}

bool FLProgNetworkSocket::handleRebindRequest()
{
    if (!_rebindRequested)
    {
        return false;
    }

    if (!closeBackendStep())
    {
        return true;
    }

    releaseLease();
    changeBoundTransport(FLPROG_TRANSPORT_NONE);

    _rebindRequested = false;
    _reopenCycle = true;
    clearActiveError();
    setState(FLPROG_SOCKET_STATE_REOPEN_REQUIRED);
    return true;
}

bool FLProgNetworkSocket::ensureLease()
{
    if (_network == nullptr)
    {
        clearActiveError();
        return false;
    }

    if (_leaseHandle != FLPROG_SOCKET_INVALID_HANDLE)
    {
        return true;
    }

    // RC6-A-dev.3-fix2:
    // Never acquire a fresh lease on a transport that is not READY.
    // After generation invalidation a stale lease is released first; while
    // Wi-Fi/Ethernet is still recovering, repeatedly acquiring a new lease
    // would immediately make that new lease stale on the next generation
    // transition. That produced hundreds/thousands of artificial STALE
    // events during one physical reconnect.
    if (!transportIdentity().valid())
    {
        const FLProgTransportIdentity selected = selectTransportForOpen(true);
        changeBoundTransport(selected);
    }

    if (!transportIdentity().valid())
    {
        clearActiveError();
        return false;
    }

    FLProgNetworkTransport *transport = boundTransport();
    if (transport == nullptr || !transport->isReady())
    {
        clearActiveError();
        return false;
    }

    return ensureLeaseForBoundTransport();
}

bool FLProgNetworkSocket::ensureLeaseForBoundTransport()
{
    if (
        _network == nullptr ||
        !transportIdentity().valid()
    )
    {
        return false;
    }

    _leaseHandle =
        _network->socketManager().acquire(
            this,
            transportIdentity(),
            false,
            _preferredNativeSocket
        );

    if (_leaseHandle == FLPROG_SOCKET_INVALID_HANDLE)
    {
        const uint8_t failure = _network->socketManager().lastFailure();

        // Capacity exhaustion is transient in the cooperative model. Keep the
        // socket in WAIT/RESERVED semantics (no active error), but preserve the
        // reason in lastError() so the application can diagnose RESOURCE_FULL.
        if (
            failure == FLPROG_SOCKET_MANAGER_FAILURE_LEASE_FULL ||
            failure == FLPROG_SOCKET_MANAGER_FAILURE_NATIVE_FULL
        )
        {
            rememberError(FLPROG_SOCKET_ERROR_RESOURCE_FULL);
        }
        else if (failure == FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT)
        {
            rememberError(FLPROG_SOCKET_ERROR_NO_TRANSPORT);
        }

        clearActiveError();
        return false;
    }

    setState(FLPROG_SOCKET_STATE_RESERVED);
    return true;
}

bool FLProgNetworkSocket::closeBackendStep()
{
    if (_activeBackend == nullptr)
    {
        return true;
    }

    const uint8_t result = _activeBackend->close();

    if (result == FLPROG_SOCKET_RESULT_WAIT)
    {
        setState(FLPROG_SOCKET_STATE_CLOSING);
        return false;
    }

    _activeBackend = nullptr;
    return true;
}

bool FLProgNetworkSocket::handleStaleLease()
{
    if (
        _network == nullptr ||
        _leaseHandle == FLPROG_SOCKET_INVALID_HANDLE ||
        !_network->socketManager().isStale(_leaseHandle)
    )
    {
        return false;
    }

    // RC6-A dev.16-fix7:
    // One invalidated lease must produce one STALE event/count only.
    //
    // closeBackendStep() changes STALE -> CLOSING while an asynchronous
    // backend close is still pending. The old condition (_state != STALE)
    // therefore counted the same physical stale lease again on every pool()
    // cycle, producing thousands of false staleCount increments.
    const bool staleCloseAlreadyInProgress =
        _state == FLPROG_SOCKET_STATE_STALE ||
        (
            _state == FLPROG_SOCKET_STATE_CLOSING &&
            _error == FLPROG_SOCKET_ERROR_STALE
        );

    if (!staleCloseAlreadyInProgress)
    {
        _staleCount++;
        _events |= FLPROG_SOCKET_EVENT_STALE;
        setError(FLPROG_SOCKET_ERROR_STALE);
        setState(FLPROG_SOCKET_STATE_STALE);
    }

    if (!closeBackendStep())
    {
        return true;
    }

    releaseLease();

    if (routeIsDynamic())
    {
        changeBoundTransport(FLPROG_TRANSPORT_NONE);
    }

    _reopenCycle = true;
    clearActiveError();
    setState(FLPROG_SOCKET_STATE_REOPEN_REQUIRED);
    return true;
}

uint8_t FLProgNetworkSocket::driveOpenStep()
{
    updateDynamicRouteBeforeOpen();

    if (!ensureLease())
    {
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_WAIT_TRANSPORT);
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    FLProgNetworkTransport *transport = boundTransport();

    if (transport == nullptr || !transport->isReady())
    {
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_WAIT_TRANSPORT);
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (!_network->socketManager().activate(_leaseHandle, this))
    {
        const uint8_t failure = _network->socketManager().lastFailure();

        if (failure == FLPROG_SOCKET_MANAGER_FAILURE_NATIVE_FULL)
        {
            rememberError(FLPROG_SOCKET_ERROR_RESOURCE_FULL);
        }
        else if (failure == FLPROG_SOCKET_MANAGER_FAILURE_NO_TRANSPORT)
        {
            rememberError(FLPROG_SOCKET_ERROR_NO_TRANSPORT);
        }
        else if (
            failure == FLPROG_SOCKET_MANAGER_FAILURE_INVALID_HANDLE ||
            failure == FLPROG_SOCKET_MANAGER_FAILURE_OWNER_MISMATCH
        )
        {
            setError(FLPROG_SOCKET_ERROR_NO_LEASE);
            setState(FLPROG_SOCKET_STATE_ERROR);
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        clearActiveError();
        setState(FLPROG_SOCKET_STATE_RESERVED);
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    FLProgSocketBackend *selectedBackend = backend(transportIdentity());

    if (
        selectedBackend == nullptr ||
        selectedBackend->transportId() != _boundTransportId
    )
    {
        setError(FLPROG_SOCKET_ERROR_NO_BACKEND);
        setState(FLPROG_SOCKET_STATE_UNSUPPORTED);
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    _activeBackend = selectedBackend;

    const FLProgSocketLease *item =
        _network->socketManager().lease(_leaseHandle);

    if (item == nullptr)
    {
        setError(FLPROG_SOCKET_ERROR_NO_LEASE);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (!_activeBackend->isOpen())
    {
        const uint8_t result =
            _activeBackend->open(
                *item,
                _protocol,
                _endpoint,
                _timeout
            );

        if (result == FLPROG_SOCKET_RESULT_WAIT)
        {
            clearActiveError();
            setState(FLPROG_SOCKET_STATE_OPENING);
            return result;
        }

        if (result == FLPROG_SOCKET_RESULT_UNSUPPORTED)
        {
            setError(FLPROG_SOCKET_ERROR_UNSUPPORTED);
            setState(FLPROG_SOCKET_STATE_UNSUPPORTED);
            return result;
        }

        if (result != FLPROG_SOCKET_RESULT_SUCCESS)
        {
            setError(FLPROG_SOCKET_ERROR_OPEN_ERROR);
            setState(FLPROG_SOCKET_STATE_ERROR);
            return FLPROG_SOCKET_RESULT_ERROR;
        }
    }

    const uint8_t pollResult = _activeBackend->pool();

    if (pollResult == FLPROG_SOCKET_RESULT_WAIT)
    {
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_OPENING);

        if (
            _timeout > 0 &&
            (uint32_t)(millis() - _stateStarted) >= _timeout
        )
        {
            setError(FLPROG_SOCKET_ERROR_TIMEOUT);
            setState(FLPROG_SOCKET_STATE_ERROR);
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (pollResult != FLPROG_SOCKET_RESULT_SUCCESS)
    {
        setError(FLPROG_SOCKET_ERROR_OPEN_ERROR);
        setState(FLPROG_SOCKET_STATE_ERROR);
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (!_activeBackend->isOpen())
    {
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_OPENING);
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (_state != FLPROG_SOCKET_STATE_OPEN)
    {
        if (_reopenCycle)
        {
            _reopenCount++;
            _reopenCycle = false;
        }

        _openCount++;
        clearActiveError();
        setState(FLPROG_SOCKET_STATE_OPEN);
    }

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

void FLProgNetworkSocket::pool()
{
    if (_network == nullptr)
    {
        return;
    }

    if (!_wantOpen || !_network->workMode())
    {
        if (!closeBackendStep())
        {
            updatePublicEvents();
            return;
        }

        releaseLease();
        changeBoundTransport(FLPROG_TRANSPORT_NONE);
        _rebindRequested = false;
        _reopenCycle = false;

        if (!_network->workMode() && _wantOpen)
        {
            // WAIT is not an error. The requested connection is retained until
            // the Network/transport becomes available again.
            clearActiveError();
            setState(FLPROG_SOCKET_STATE_WAIT_TRANSPORT);
        }
        else
        {
            clearActiveError();
            setState(FLPROG_SOCKET_STATE_CLOSED);
        }

        updatePublicEvents();
        return;
    }

    if (handleRebindRequest())
    {
        updatePublicEvents();
        return;
    }

    if (handleStaleLease())
    {
        updatePublicEvents();
        return;
    }

    if (_state == FLPROG_SOCKET_STATE_ERROR)
    {
        updatePublicEvents();
        return;
    }

    driveOpenStep();
    updatePublicEvents();
}

bool FLProgNetworkSocket::isOpen() const
{
    return
        _state == FLPROG_SOCKET_STATE_OPEN &&
        _activeBackend != nullptr &&
        _activeBackend->isOpen();
}

bool FLProgNetworkSocket::connected() const
{
    return isOpen() && _activeBackend->connected();
}

int16_t FLProgNetworkSocket::nativeSocket() const
{
    if (
        _network == nullptr ||
        _leaseHandle == FLPROG_SOCKET_INVALID_HANDLE
    )
    {
        return FLPROG_SOCKET_NATIVE_NONE;
    }

    return _network->socketManager().nativeSocket(_leaseHandle);
}

int FLProgNetworkSocket::available()
{
    return isOpen() ? _activeBackend->available() : 0;
}

int FLProgNetworkSocket::read()
{
    if (!isOpen())
    {
        return -1;
    }

    const int result = _activeBackend->read();

    if (result < -1)
    {
        setError(FLPROG_SOCKET_ERROR_IO_ERROR);
        _ioErrorCount++;
    }

    return result;
}

int FLProgNetworkSocket::read(uint8_t *buffer, size_t length)
{
    if (!isOpen() || buffer == nullptr || length == 0)
    {
        return 0;
    }

    const int result = _activeBackend->read(buffer, length);

    if (result < 0)
    {
        setError(FLPROG_SOCKET_ERROR_IO_ERROR);
        _ioErrorCount++;
    }

    return result;
}

int FLProgNetworkSocket::peek()
{
    return isOpen() ? _activeBackend->peek() : -1;
}

size_t FLProgNetworkSocket::write(uint8_t value)
{
    return write(&value, 1);
}

size_t FLProgNetworkSocket::write(
    const uint8_t *buffer,
    size_t length)
{
    if (!isOpen() || buffer == nullptr || length == 0)
    {
        return 0;
    }

    const size_t result = _activeBackend->write(buffer, length);

    if (result == 0 && length > 0)
    {
        setError(FLPROG_SOCKET_ERROR_IO_ERROR);
        _ioErrorCount++;
    }

    return result;
}

void FLProgNetworkSocket::flush()
{
    if (isOpen())
    {
        _activeBackend->flush();
    }
}

uint16_t FLProgNetworkSocket::localPort() const
{
    if (_activeBackend != nullptr && _activeBackend->isOpen())
    {
        const uint16_t value = _activeBackend->localPort();
        if (value != 0) return value;
    }

    return _endpoint.localPort;
}

IPAddress FLProgNetworkSocket::remoteIP() const
{
    if (_activeBackend != nullptr && _activeBackend->isOpen())
    {
        const IPAddress value = _activeBackend->remoteIP();
        if (value != IPAddress(0, 0, 0, 0)) return value;
    }

    return _endpoint.remoteIP;
}

uint16_t FLProgNetworkSocket::remotePort() const
{
    if (_activeBackend != nullptr && _activeBackend->isOpen())
    {
        const uint16_t value = _activeBackend->remotePort();
        if (value != 0) return value;
    }

    return _endpoint.remotePort;
}
