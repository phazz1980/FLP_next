/*
 * FLProg_Network — flprogDns.h
 * Назначение: legacy-адаптер FLProgDNSClient поверх native FLProgNetworkDns/A3.
 *
 * Почему существует отдельный adapter, а не typedef:
 * - старый Flprog_Ethernet возвращал FLPROG_ERROR/SUCCESS/WAIT с собственной
 *   нумерацией, отличающейся от native FLProgNetworkDns;
 * - старый DNS client имел отдельный однозаписный cache, время хранения которого
 *   задавалось в миллисекундах и не зависело от DNS TTL;
 * - старый getHostByName() был неблокирующим: его вызывали повторно, пока он не
 *   вернёт SUCCESS или ERROR.
 *
 * Архитектурная граница:
 * legacy FLProgDNSClient -> FLProgNetworkDns -> FLProgNetworkUdp -> A3 transport.
 * Старая raw-socket/DNS реализация сюда не возвращается.
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
#include "flprogUtilites.h"
#include "../abstract/flprogAbstractTcpInterface.h"
#include "../protocol/flprogNetworkDns.h"

class FLProgDNSClient
{
public:
    FLProgDNSClient() = default;

    explicit FLProgDNSClient(FLProgAbstractTcpInterface *sourse)
    {
        setSourse(sourse);
    }

    // Legacy spelling "Sourse" intentionally preserved for generated projects.
    void setSourse(FLProgAbstractTcpInterface *sourse);
    FLProgAbstractTcpInterface *getSourse() { return _sourse; }
    const FLProgAbstractTcpInterface *getSourse() const { return _sourse; }

    // Legacy asynchronous contract:
    //   FLPROG_WAIT    — request is still in progress;
    //   FLPROG_SUCCESS — aResult[0..3] contains IPv4;
    //   FLPROG_ERROR   — getError()/getErrorCode() explains the failure.
    int getHostByName(
        const char *hostname,
        uint8_t *result,
        uint16_t timeout = 5000);

    // Old API used milliseconds. We keep the same units and exact single-entry
    // cache semantics instead of exposing native DNS TTL behaviour to old blocks.
    void setDnsCacheStorageTime(uint32_t time)
    {
        _cacheStorageTime = time;
    }
    uint32_t getDnsCacheStorageTime() const { return _cacheStorageTime; }

    // stop() cancels only this DNS transaction. It is not a NET_RESET and does
    // not affect WiFi/Ethernet or other sockets/services on the same NET NUM.
    void stop();

    uint8_t getStatus() const { return _compatStatus; }
    uint8_t getError() const { return _compatError; }
    uint8_t getErrorCode() const { return _compatError; }

    // Legacy change flags are kept because generated diagnostic/control blocks
    // may poll them even when the DNS result itself is the only useful payload.
    bool getIsChangeStatus() const { return _isChangeStatus; }
    bool getIsChangeStatusWithReset()
    {
        const bool value = _isChangeStatus;
        _isChangeStatus = false;
        return value;
    }
    void setIsChangeStatus(bool value) { _isChangeStatus = value; }

    bool getIsChangeError() const { return _isChangeError; }
    bool getIsChangeErrorWithReset()
    {
        const bool value = _isChangeError;
        _isChangeError = false;
        return value;
    }
    void setIsChangeError(bool value) { _isChangeError = value; }

    uint32_t statusForExt() const { return _statusForExt; }
    bool statusForExtGetBit(uint8_t bit) const
    {
        return bitRead(_statusForExt, bit);
    }
    void statusForExtResetBit(uint8_t bit)
    {
        bitWrite(_statusForExt, bit, 0);
    }
    bool statusForExtGetBitWithReset(uint8_t bit)
    {
        const bool value = bitRead(_statusForExt, bit);
        bitWrite(_statusForExt, bit, 0);
        return value;
    }
    void statusForExtSetBit(uint8_t bit)
    {
        bitWrite(_statusForExt, bit, 1);
    }

    void setFlags();

    // Native resolver is exposed only for diagnostics/new code. Old generated
    // code should continue to use getHostByName().
    FLProgNetworkDns &nativeDns() { return _dns; }
    const FLProgNetworkDns &nativeDns() const { return _dns; }

private:
    bool ensureAttached();
    bool checkLegacyCache(const char *hostname, uint8_t *result);
    void storeLegacyCache(const char *hostname, IPAddress address);
    uint8_t translateNativeError(uint8_t nativeError) const;
    void copyIpToArray(IPAddress address, uint8_t *result) const;

    FLProgAbstractTcpInterface *_sourse = nullptr;
    FLProgNetworkDns _dns;
    bool _dnsAttached = false;

    // Name of the legacy request currently owned by the native resolver.
    //
    // Why this latch is required:
    // native FLProgNetworkDns keeps SUCCESS/ERROR as terminal states until the
    // caller consumes them. Its resolve() method is allowed to start a new
    // transaction whenever the resolver is no longer busy. The old
    // FLProgDNSClient contract, however, repeatedly calls getHostByName() while
    // waiting. Without this latch a successful native reply could be overwritten
    // by a brand-new WAIT before the compatibility layer copied the result.
    //
    // Keeping the requested host here lets getHostByName() harvest terminal
    // native SUCCESS/ERROR first and only then permit another resolve().
    String _activeHost = "";

    // Old Flprog_Ethernet DNS retried three times after the first request:
    // initial attempt + 3 retries = 4 total native attempts.
    static const uint8_t LEGACY_DNS_ATTEMPTS = 4;

    // Compatibility status/error values are deliberately independent of native
    // FLProgNetworkDns state numbers.
    uint8_t _compatStatus = FLPROG_NOT_REDY_STATUS;
    uint8_t _compatError = FLPROG_NOT_ERROR;

    uint8_t _oldCompatStatus = FLPROG_NOT_REDY_STATUS;
    uint8_t _oldCompatError = FLPROG_NOT_ERROR;
    bool _isChangeStatus = false;
    bool _isChangeError = false;
    uint32_t _statusForExt = 1;

    // Exact legacy cache model: one hostname + one IPv4 + fixed lifetime in ms.
    String _cachedHost = "";
    IPAddress _cachedIp = IPAddress(0, 0, 0, 0);
    uint32_t _cacheStarted = 0;
    uint32_t _cacheStorageTime = 60000UL;
};
