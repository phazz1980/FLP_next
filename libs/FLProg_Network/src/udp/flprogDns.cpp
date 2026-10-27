/*
 * FLProg_Network — flprogDns.cpp
 * Реализация legacy FLProgDNSClient поверх FLProgNetworkDns.
 *
 * Здесь намеренно нет старого ручного построения DNS-пакетов и прямой работы
 * с socket index. Этим занимается native resolver через managed UDP/A3.
 * Расширенный контракт сопровождения:
 * - Legacy UDP/DNS API остается фасадом над native route-aware реализацией и не дублирует socket state.
 * - Повторные вызовы должны сохранять неблокирующую семантику SUCCESS/WAIT/ERROR, ожидаемую FLProg.
 * - Кэш, TTL и packet boundaries переводятся в compatibility слое, а не в hardware backend.
 * Extended maintenance contract:
 * - The legacy UDP/DNS API remains a facade over the native route-aware implementation and does not duplicate socket state.
 * - Repeated calls keep the non-blocking SUCCESS/WAIT/ERROR semantics expected by FLProg.
 * - Cache, TTL and packet boundaries are translated in the compatibility layer, not in the hardware backend.
 */
#include "flprogDns.h"

void FLProgDNSClient::setSourse(FLProgAbstractTcpInterface *sourse)
{
    if (_sourse == sourse && _dnsAttached)
    {
        return;
    }

    // A DNS object owns only its resolver service. Switching NET source must not
    // reset the transport or touch sockets owned by other services.
    if (_dnsAttached)
    {
        _dns.end();
        _dnsAttached = false;
    }

    _sourse = sourse;
    _activeHost = "";
    _compatError = FLPROG_NOT_ERROR;

    if (_sourse == nullptr)
    {
        _compatStatus = FLPROG_NOT_REDY_STATUS;
        setFlags();
        return;
    }

    if (!ensureAttached())
    {
        _compatStatus = FLPROG_NOT_REDY_STATUS;
        _compatError = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        setFlags();
        return;
    }

    // A private native resolver is used so one legacy DNS object keeps the same
    // independent transaction/cache lifecycle it had in Flprog_Ethernet.
    _compatStatus = FLPROG_READY_STATUS;
    _compatError = FLPROG_NOT_ERROR;
    _oldCompatStatus = _compatStatus;
    _oldCompatError = _compatError;
    _isChangeStatus = false;
    _isChangeError = false;
}

bool FLProgDNSClient::ensureAttached()
{
    if (_dnsAttached)
    {
        return true;
    }

    if (_sourse == nullptr)
    {
        return false;
    }

    if (!_dns.begin(*_sourse, FLPROG_NETWORK_ROUTE_DEFAULT))
    {
        return false;
    }

    _dnsAttached = true;

    // Native DNS cache is disabled for this compatibility object. The adapter
    // below reproduces the legacy one-entry, fixed-millisecond cache exactly;
    // otherwise DNS TTL could silently change old project behaviour.
    _dns.setMaxCacheTtl(0);
    _dns.setAttempts(LEGACY_DNS_ATTEMPTS);
    return true;
}

void FLProgDNSClient::copyIpToArray(IPAddress address, uint8_t *result) const
{
    if (result == nullptr)
    {
        return;
    }

    result[0] = address[0];
    result[1] = address[1];
    result[2] = address[2];
    result[3] = address[3];
}

bool FLProgDNSClient::checkLegacyCache(
    const char *hostname,
    uint8_t *result)
{
    if (
        hostname == nullptr ||
        result == nullptr ||
        _cachedHost.length() == 0)
    {
        return false;
    }

    // Unsigned millis subtraction is wrap-safe for the practical cache periods
    // used by FLProg projects.
    if ((uint32_t)(millis() - _cacheStarted) >= _cacheStorageTime)
    {
        _cachedHost = "";
        _cachedIp = IPAddress(0, 0, 0, 0);
        return false;
    }

    if (!_cachedHost.equals(String(hostname)))
    {
        return false;
    }

    if (_cachedIp == IPAddress(0, 0, 0, 0))
    {
        _cachedHost = "";
        return false;
    }

    copyIpToArray(_cachedIp, result);
    return true;
}

void FLProgDNSClient::storeLegacyCache(
    const char *hostname,
    IPAddress address)
{
    if (hostname == nullptr || address == IPAddress(0, 0, 0, 0))
    {
        return;
    }

    _cachedHost = String(hostname);
    _cachedIp = address;
    _cacheStarted = millis();
}

uint8_t FLProgDNSClient::translateNativeError(uint8_t nativeError) const
{
    // Keep old error categories where a direct equivalent exists. Several
    // richer native errors intentionally collapse into INVALID_RESPONSE because
    // legacy generated code knows only the older, smaller error vocabulary.
    switch (nativeError)
    {
    case FLPROG_DNS_ERROR_NONE:
        return FLPROG_NOT_ERROR;

    case FLPROG_DNS_ERROR_NO_NETWORK:
    case FLPROG_DNS_ERROR_NO_TRANSPORT:
        return FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;

    case FLPROG_DNS_ERROR_NO_SERVER:
        return FLPROG_ETHERNET_DNS_INVALID_SERVER;

    case FLPROG_DNS_ERROR_UDP:
    case FLPROG_DNS_ERROR_SEND:
        return FLPROG_ETHERNET_UDP_SOKET_START_ERROR;

    case FLPROG_DNS_ERROR_TIMEOUT:
        return FLPROG_ETHERNET_UDP_TIMEOUT_ERROR;

    case FLPROG_DNS_ERROR_TRUNCATED:
        return FLPROG_ETHERNET_DNS_TRUNCATED;

    case FLPROG_DNS_ERROR_INVALID_NAME:
    case FLPROG_DNS_ERROR_RESPONSE:
    case FLPROG_DNS_ERROR_NXDOMAIN:
    case FLPROG_DNS_ERROR_NO_A_RECORD:
    default:
        return FLPROG_ETHERNET_DNS_INVALID_RESPONSE;
    }
}

int FLProgDNSClient::getHostByName(
    const char *hostname,
    uint8_t *result,
    uint16_t timeout)
{
    if (_sourse == nullptr || hostname == nullptr || result == nullptr)
    {
        _compatStatus = FLPROG_NOT_REDY_STATUS;
        _compatError = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    // Old DNS returned WAIT while the underlying interface was not ready; it did
    // not turn a temporary link startup into a DNS error.
    if (!_sourse->isReadyForDNS())
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_WAIT;
    }

    if (checkLegacyCache(hostname, result))
    {
        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_SUCCESS;
    }

    if (!ensureAttached())
    {
        _compatStatus = FLPROG_NOT_REDY_STATUS;
        _compatError = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    // Legacy timeout was applied per request attempt. Native resolver receives
    // the same timeout and the four-attempt policy configured in ensureAttached().
    _dns.setTimeout(timeout);

    // IMPORTANT: consume a terminal native result BEFORE calling resolve()
    // again.
    //
    // Native resolve() is intentionally reusable: when the resolver is no
    // longer busy it may start a new transaction. Old generated FLProg code,
    // on the other hand, polls getHostByName() every loop while it sees WAIT.
    // If the native resolver completed between two such calls, calling
    // resolve() first would replace SUCCESS by a new WAIT and the legacy layer
    // would never observe the answer. That exact failure produced hundreds of
    // DNS queries per second in Gate 0.4.2.
    if (_activeHost.equals(String(hostname)))
    {
        if (_dns.ready())
        {
            const IPAddress address = _dns.result();
            copyIpToArray(address, result);
            storeLegacyCache(hostname, address);
            _activeHost = "";

            _compatStatus = FLPROG_READY_STATUS;
            _compatError = FLPROG_NOT_ERROR;
            setFlags();
            return FLPROG_SUCCESS;
        }

        if (_dns.failed())
        {
            const uint8_t nativeError = _dns.error();
            _activeHost = "";

            _compatStatus = FLPROG_READY_STATUS;
            _compatError = translateNativeError(nativeError);
            setFlags();
            return FLPROG_ERROR;
        }

        // A legacy block polls getHostByName() every program cycle while it
        // receives FLPROG_WAIT. Once this adapter has started a native request,
        // that polling MUST NOT call resolve() again while the resolver is busy.
        //
        // Even though native resolve() is designed to recognise an identical
        // in-flight hostname, the compatibility boundary is safer when it owns
        // the "one old request = one native transaction" rule explicitly.
        // This also guarantees that queryCount can grow only because of the
        // native retry policy (maximum LEGACY_DNS_ATTEMPTS), never because the
        // legacy caller happens to execute hundreds of loops per second.
        if (_dns.busy())
        {
            _compatStatus = FLPROG_WAIT_ETHERNET_DNS_STATUS;
            _compatError = FLPROG_NOT_ERROR;
            setFlags();
            return FLPROG_WAIT;
        }

        // Reaching IDLE here means the native transaction was externally
        // cancelled/reset without producing SUCCESS or ERROR. Drop only the
        // compatibility latch and allow a clean new transaction below.
        _activeHost = "";
    }

    const uint8_t nativeResult =
        _dns.resolve(hostname, FLPROG_NETWORK_ROUTE_DEFAULT);

    if (nativeResult == FLPROG_DNS_RESULT_SUCCESS)
    {
        // Immediate SUCCESS is possible for an IPv4 literal or a native cache
        // hit. The legacy compatibility object still copies the answer into its
        // own one-entry millisecond cache.
        const IPAddress address = _dns.result();
        copyIpToArray(address, result);
        storeLegacyCache(hostname, address);
        _activeHost = "";

        _compatStatus = FLPROG_READY_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_SUCCESS;
    }

    if (nativeResult == FLPROG_DNS_RESULT_WAIT)
    {
        // WAIT means this exact hostname now owns the native transaction.
        // Remember it so a later terminal SUCCESS/ERROR can be harvested before
        // any new resolve() call is allowed to restart the resolver.
        _activeHost = String(hostname);

        _compatStatus = FLPROG_WAIT_ETHERNET_DNS_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_WAIT;
    }

    if (nativeResult == FLPROG_DNS_RESULT_BUSY)
    {
        // BUSY means another hostname is already in progress. Do not overwrite
        // _activeHost: it identifies the transaction that must finish first.
        _compatStatus = FLPROG_WAIT_ETHERNET_DNS_STATUS;
        _compatError = FLPROG_NOT_ERROR;
        setFlags();
        return FLPROG_WAIT;
    }

    _activeHost = "";
    _compatStatus = FLPROG_READY_STATUS;
    _compatError = translateNativeError(_dns.error());
    setFlags();
    return FLPROG_ERROR;
}

void FLProgDNSClient::stop()
{
    if (_dnsAttached)
    {
        // cancel() closes only the resolver's managed UDP lease and leaves the
        // service attached so a later getHostByName() can start immediately.
        _dns.cancel();
    }

    // A cancelled request must never be harvested by a later call.
    _activeHost = "";

    _compatStatus = FLPROG_NOT_REDY_STATUS;
    _compatError = FLPROG_NOT_ERROR;
    setFlags();
}

void FLProgDNSClient::setFlags()
{
    if (_compatStatus != _oldCompatStatus)
    {
        _oldCompatStatus = _compatStatus;
        _isChangeStatus = true;
    }

    if (_compatError != _oldCompatError)
    {
        _oldCompatError = _compatError;
        _isChangeError = true;
    }
}
