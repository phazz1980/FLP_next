/*
 * FLProg_Network — flprogEthernetClient.cpp
 * Назначение: Legacy FLProgEthernetClient adapter поверх FLProgNetworkTcpClient/A3.
 *
 * Архитектурные заметки:
 * - Сохраняет старые имена методов и числовые FLPROG_ERROR/SUCCESS/WAIT.
 * - Native result codes нельзя возвращать напрямую: compatibility слой обязан их переводить.
 * - Adapter не восстанавливает старую raw-socket таблицу.
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
#include "flprogEthernetClient.h"

void FLProgEthernetClient::init(FLProgAbstractTcpInterface *sourse)
{
    if (_isInit || sourse == nullptr)
    {
        return;
    }

    _sourse = sourse;

    if (!FLProgNetworkTcpClient::begin(*_sourse, FLPROG_NETWORK_ROUTE_DEFAULT))
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR;
        setFlags();
        return;
    }

    // Legacy client owns its connection attempts; no autonomous reconnect loop.
    setAutoReconnect(false);
    setReconnectOnInitialFailure(false);
    setTimeout(_legacyConnectionTimeout);
    applyDnsCachePolicy();

    _compatStatus = FLPROG_READY_STATUS;
    _compatError = FLPROG_NOT_ERROR;
    _oldCompatStatus = _compatStatus;
    _oldCompatError = _compatError;
    _isInit = true;
}

bool FLProgEthernetClient::sameIpTarget(IPAddress ip, uint16_t port) const
{
    return _legacyTargetValid && !_legacyTargetHost &&
           _legacyTargetPort == port && _legacyTargetIp == ip;
}

bool FLProgEthernetClient::sameHostTarget(const char *host, uint16_t port) const
{
    return _legacyTargetValid && _legacyTargetHost && host != nullptr &&
           _legacyTargetPort == port && strcmp(_legacyTargetHostName, host) == 0;
}

void FLProgEthernetClient::rememberIpTarget(IPAddress ip, uint16_t port)
{
    _legacyTargetValid = true;
    _legacyTargetHost = false;
    _legacyTargetIp = ip;
    _legacyTargetPort = port;
    _legacyTargetHostName[0] = 0;
}

void FLProgEthernetClient::rememberHostTarget(const char *host, uint16_t port)
{
    _legacyTargetValid = true;
    _legacyTargetHost = true;
    _legacyTargetIp = IPAddress(0, 0, 0, 0);
    _legacyTargetPort = port;
    if (host == nullptr)
    {
        _legacyTargetHostName[0] = 0;
        return;
    }
    strncpy(_legacyTargetHostName, host, FLPROG_TCP_HOSTNAME_MAX_LENGTH);
    _legacyTargetHostName[FLPROG_TCP_HOSTNAME_MAX_LENGTH] = 0;
}

uint8_t FLProgEthernetClient::translateLegacyError() const
{
    if (_sourse == nullptr || !_sourse->isReady())
    {
        return FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
    }

    if (clientError() == FLPROG_TCP_CLIENT_ERROR_DNS ||
        clientError() == FLPROG_TCP_CLIENT_ERROR_NO_RESOLVER)
    {
        return FLPROG_ETHERNET_DNS_INVALID_RESPONSE;
    }

    switch (lastDisconnectReason())
    {
    case FLPROG_TCP_DISCONNECT_CONNECT_TIMEOUT:
        return FLPROG_ETHERNET_CLIENT_CONNECT_TIMEOUT_ERROR;
    case FLPROG_TCP_DISCONNECT_PEER_CLOSED:
        return FLPROG_ETHERNET_CLIENT_SOKET_CLOSED_ERROR;
    case FLPROG_TCP_DISCONNECT_NO_TRANSPORT:
    case FLPROG_TCP_DISCONNECT_NO_LEASE:
    case FLPROG_TCP_DISCONNECT_CONNECT_FAILED:
    case FLPROG_TCP_DISCONNECT_INVALID_TARGET:
    case FLPROG_TCP_DISCONNECT_UNSUPPORTED:
        return FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR;
    default:
        break;
    }

    if (error() != FLPROG_SOCKET_ERROR_NONE || clientError() != FLPROG_TCP_CLIENT_ERROR_NONE)
    {
        return FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR;
    }
    return FLPROG_NOT_ERROR;
}

int FLProgEthernetClient::translateCurrentResult(bool dnsContext)
{
    if (FLProgNetworkTcpClient::connected())
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_SUCCESS;
    }

    if (clientError() != FLPROG_TCP_CLIENT_ERROR_NONE ||
        error() != FLPROG_SOCKET_ERROR_NONE ||
        state() == FLPROG_SOCKET_STATE_ERROR)
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = translateLegacyError();
        setFlags();
        return FLPROG_ERROR;
    }

    _compatStatus = (dnsContext && resolving())
                        ? FLPROG_WAIT_ETHERNET_DNS_STATUS
                        : FLPROG_WAIT_ETHERNET_CLIENT_CONNECT_STATUS;
    _compatError = FLPROG_NOT_ERROR;
    setFlags();
    return FLPROG_WAIT;
}

int FLProgEthernetClient::connect(IPAddress ip, uint16_t port)
{
    if (!_isInit || _sourse == nullptr)
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    if (!_sourse->isReady())
    {
        FLProgNetworkTcpClient::stop();
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        _legacyTargetValid = false;
        setFlags();
        return FLPROG_ERROR;
    }

    if (sameIpTarget(ip, port) && connectionRequested())
    {
        return translateCurrentResult(false);
    }

    rememberIpTarget(ip, port);
    clearClientError();
    _compatError = FLPROG_NOT_ERROR;
    (void)FLProgNetworkTcpClient::connect(ip, port);
    return translateCurrentResult(false);
}

int FLProgEthernetClient::connect(const char *host, uint16_t port)
{
    if (!_isInit || _sourse == nullptr)
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    if (!_sourse->isReady())
    {
        FLProgNetworkTcpClient::stop();
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        _legacyTargetValid = false;
        setFlags();
        return FLPROG_ERROR;
    }

    if (sameHostTarget(host, port) && connectionRequested())
    {
        return translateCurrentResult(true);
    }

    rememberHostTarget(host, port);
    applyDnsCachePolicy();
    clearClientError();
    _compatError = FLPROG_NOT_ERROR;
    (void)FLProgNetworkTcpClient::connect(host, port);
    return translateCurrentResult(true);
}

// connected() reports the old boolean-style contract while native state remains available internally.
uint8_t FLProgEthernetClient::connected()
{
    if (_sourse == nullptr || !_sourse->isReady())
    {
        return 0;
    }

    // Legacy Flprog_Ethernet semantics:
    // stop() changes the compatibility status to NOT_READY immediately, even
    // though the native A3 socket may still be completing requestClose().
    // The old FLProgEthernetClient::connected() returned 0 whenever its legacy
    // status was not READY. Preserve that contract here and do not re-promote
    // the compatibility status while a native close is still settling.
    if (_compatStatus != FLPROG_READY_STATUS)
    {
        return 0;
    }

    if (FLProgNetworkTcpClient::connected())
    {
        _compatError = FLPROG_NOT_ERROR;
        return 1;
    }

    return 0;
}

// Stop only this client lease; a client stop is not a NET_RESET.
void FLProgEthernetClient::stop()
{
    FLProgNetworkTcpClient::stop();
    _legacyTargetValid = false;
    _compatStatus = FLPROG_NOT_REDY_STATUS;
    _compatError = FLPROG_NOT_ERROR;
    setFlags();
}

uint8_t FLProgEthernetClient::getError() const
{
    return _compatError;
}

uint8_t FLProgEthernetClient::getSocketNumber() const
{
    const int16_t socket = nativeSocket();
    if (socket >= 0 && socket <= 255)
    {
        return static_cast<uint8_t>(socket);
    }
    return _sourse == nullptr ? 255 : _sourse->maxSoketNum();
}

void FLProgEthernetClient::setDnsCacheStorageTime(uint32_t time)
{
    _legacyDnsCacheStorageTime = time;
    applyDnsCachePolicy();
}

void FLProgEthernetClient::applyDnsCachePolicy()
{
    FLProgNetworkDns *resolver = activeDnsResolver();
    if (resolver == nullptr)
    {
        return;
    }

    // Legacy API is milliseconds. Native DNS stores TTL in seconds.
    const uint32_t seconds =
        (_legacyDnsCacheStorageTime == 0)
            ? 0
            : ((_legacyDnsCacheStorageTime + 999UL) / 1000UL);
    resolver->setMaxCacheTtl(seconds);
}

void FLProgEthernetClient::setFlags()
{
    const uint8_t currentStatus = getStatus();
    const uint8_t currentError = getError();

    if (currentStatus != _oldCompatStatus)
    {
        _oldCompatStatus = currentStatus;
        _isChangeStatus = true;
    }

    if (currentError != _oldCompatError)
    {
        _oldCompatError = currentError;
        _isChangeError = true;
    }
}
