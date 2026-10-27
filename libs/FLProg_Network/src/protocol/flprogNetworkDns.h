/*
 * FLProg_Network — flprogNetworkDns.h
 * Назначение: DNS resolver с route-aware кэшированием.
 *
 * Архитектурные заметки:
 * - DNS выполняется через тот же NetworkDevice/route, который будет использовать клиент.
 * - WAIT/BUSY/SUCCESS/ERROR должны однозначно преобразовываться в legacy-коды адаптеров.
 * - TTL хранится в native единицах; legacy миллисекунды преобразуются только в compatibility-слое.
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
#include <stdint.h>
#include <stddef.h>
#include "../service/flprogNetworkService.h"
#include "flprogNetworkUdp.h"

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// FLProg_Network v0.6.2 - неблокирующий IPv4 DNS resolver.
//
// Resolver отправляет обычные RFC 1035 запросы A/IN через FLProgNetworkUdp,
// поэтому DNS-транзакция следует той же route/transport политике, что и другие
// сокеты. На ESP32 global default interface не переключается.
//
// Один resolver обрабатывает одно активное имя. Network Manager содержит
// resolver по умолчанию, используемый также UDP beginPacket(host). При
// необходимости параллельных запросов можно подключить дополнительные объекты.
// -----------------------------------------------------------------------------

#ifndef FLPROG_DNS_MAX_HOSTNAME_LENGTH
#define FLPROG_DNS_MAX_HOSTNAME_LENGTH 127
#endif

#ifndef FLPROG_DNS_MAX_PACKET_SIZE
#define FLPROG_DNS_MAX_PACKET_SIZE 512
#endif

#ifndef FLPROG_DNS_CACHE_SIZE
#define FLPROG_DNS_CACHE_SIZE 4
#endif

#ifndef FLPROG_DNS_DEFAULT_TIMEOUT
#define FLPROG_DNS_DEFAULT_TIMEOUT 1500UL
#endif

#ifndef FLPROG_DNS_DEFAULT_ATTEMPTS
#define FLPROG_DNS_DEFAULT_ATTEMPTS 3
#endif

#ifndef FLPROG_DNS_MAX_CACHE_TTL
#define FLPROG_DNS_MAX_CACHE_TTL 86400UL
#endif

#define FLPROG_DNS_RESULT_SUCCESS 0
#define FLPROG_DNS_RESULT_WAIT    1
#define FLPROG_DNS_RESULT_ERROR   2
#define FLPROG_DNS_RESULT_BUSY    3

#define FLPROG_DNS_STATE_IDLE           0
#define FLPROG_DNS_STATE_WAIT_TRANSPORT 1
#define FLPROG_DNS_STATE_OPENING        2
#define FLPROG_DNS_STATE_SENDING        3
#define FLPROG_DNS_STATE_WAIT_RESPONSE  4
#define FLPROG_DNS_STATE_READ_RESPONSE  5
#define FLPROG_DNS_STATE_SUCCESS        6
#define FLPROG_DNS_STATE_ERROR          7

#define FLPROG_DNS_ERROR_NONE          0
#define FLPROG_DNS_ERROR_INVALID_NAME  1
#define FLPROG_DNS_ERROR_NO_NETWORK    2
#define FLPROG_DNS_ERROR_NO_TRANSPORT  3
#define FLPROG_DNS_ERROR_NO_SERVER     4
#define FLPROG_DNS_ERROR_UDP           5
#define FLPROG_DNS_ERROR_SEND          6
#define FLPROG_DNS_ERROR_TIMEOUT       7
#define FLPROG_DNS_ERROR_RESPONSE      8
#define FLPROG_DNS_ERROR_NXDOMAIN      9
#define FLPROG_DNS_ERROR_TRUNCATED    10
#define FLPROG_DNS_ERROR_NO_A_RECORD  11

class FLProgNetworkDns : public FLProgNetworkService
{
public:
    FLProgNetworkDns();
    ~FLProgNetworkDns() override;

    bool begin(
        FLProgNetworkInterface &network,
        uint8_t route = 0);

    void end();

    // Запустить или опросить разрешение имени. Для уже активного того же запроса
    // возвращается WAIT. Cache hit и IPv4 literal дают SUCCESS сразу.
    uint8_t resolve(const char *hostname);
    uint8_t resolve(const String &hostname)
    {
        return resolve(hostname.c_str());
    }

    uint8_t resolve(const char *hostname, uint8_t route);
    uint8_t resolve(const String &hostname, uint8_t route)
    {
        return resolve(hostname.c_str(), route);
    }

    void cancel();

    void pool() override;

    void setRoute(uint8_t value) { _route = value; }
    uint8_t route() const { return _route; }

    void setServer(IPAddress value)
    {
        _serverOverride = value;
        _serverOverrideEnabled = !ipIsZero(value);
    }

    void clearServerOverride()
    {
        _serverOverrideEnabled = false;
        _serverOverride = IPAddress(0, 0, 0, 0);
    }

    bool serverOverrideEnabled() const { return _serverOverrideEnabled; }
    IPAddress server() const { return _queryServer; }

    void setTimeout(uint32_t value) { _timeout = value; }
    uint32_t timeout() const { return _timeout; }

    void setAttempts(uint8_t value)
    {
        _maxAttempts = (value == 0) ? 1 : value;
    }
    uint8_t attempts() const { return _maxAttempts; }

    void setMaxCacheTtl(uint32_t seconds)
    {
        _maxCacheTtl = seconds;
    }
    uint32_t maxCacheTtl() const { return _maxCacheTtl; }

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t responseCode() const { return _responseCode; }
    bool busy() const
    {
        return
            _state == FLPROG_DNS_STATE_WAIT_TRANSPORT ||
            _state == FLPROG_DNS_STATE_OPENING ||
            _state == FLPROG_DNS_STATE_SENDING ||
            _state == FLPROG_DNS_STATE_WAIT_RESPONSE ||
            _state == FLPROG_DNS_STATE_READ_RESPONSE;
    }
    bool ready() const { return _state == FLPROG_DNS_STATE_SUCCESS; }
    bool failed() const { return _state == FLPROG_DNS_STATE_ERROR; }

    IPAddress result() const { return _result; }
    uint8_t transportId() const { return _queryTransportId; }
    const char *hostname() const { return _hostname; }
    uint32_t resultTtl() const { return _resultTtl; }

    bool lookupCached(
        const char *hostname,
        uint8_t transportId,
        IPAddress server,
        IPAddress &result);

    void clearCache();
    uint8_t cacheCount() const;

    uint32_t queryCount() const { return _queryCount; }
    uint32_t cacheHitCount() const { return _cacheHitCount; }
    uint32_t cacheMissCount() const { return _cacheMissCount; }
    uint32_t timeoutCount() const { return _timeoutCount; }
    uint32_t responseErrorCount() const { return _responseErrorCount; }

    void resetStatistics()
    {
        _queryCount = 0;
        _cacheHitCount = 0;
        _cacheMissCount = 0;
        _timeoutCount = 0;
        _responseErrorCount = 0;
    }

private:
    struct CacheEntry
    {
        bool used = false;
        char hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1] = {0};
        IPAddress address = IPAddress(0, 0, 0, 0);
        IPAddress server = IPAddress(0, 0, 0, 0);
        uint8_t transportId = 0;
        uint32_t expiresAt = 0;
        uint32_t lastUsed = 0;
    };

    static bool ipIsZero(IPAddress ip);
    static bool ipEqual(IPAddress a, IPAddress b) { return a == b; }
    static bool timeReached(uint32_t now, uint32_t target)
    {
        return (int32_t)(now - target) >= 0;
    }

    bool normalizeHostname(const char *source, char *destination) const;
    bool parseIpv4Literal(const char *text, IPAddress &address) const;
    bool encodeQuestion(uint8_t *buffer, uint16_t &length) const;

    uint16_t read16(const uint8_t *buffer, uint16_t offset) const;
    uint32_t read32(const uint8_t *buffer, uint16_t offset) const;
    bool skipName(const uint8_t *buffer, uint16_t length, uint16_t &offset) const;
    bool parseResponse(IPAddress &address, uint32_t &ttl);

    void startRequest(const char *normalized, uint8_t route);
    void resetTransactionState();
    void finishSuccess(IPAddress address, uint32_t ttl, bool fromCache);
    void finishError(uint8_t error);

    bool selectTransportAndServer();
    bool startUdp();
    bool prepareQuery();
    bool sendQueryStep();
    void retryOrTimeout();
    void waitResponseStep();
    void readResponseStep();

    CacheEntry *findCache(
        const char *hostname,
        uint8_t transportId,
        IPAddress server,
        bool countHit);

    void storeCache(
        const char *hostname,
        uint8_t transportId,
        IPAddress server,
        IPAddress address,
        uint32_t ttl);

    FLProgNetworkUdp _udp;

    uint8_t _route = 0;
    uint8_t _state = FLPROG_DNS_STATE_IDLE;
    uint8_t _error = FLPROG_DNS_ERROR_NONE;
    uint8_t _responseCode = 0;

    char _hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1] = {0};

    IPAddress _result = IPAddress(0, 0, 0, 0);
    IPAddress _queryServer = IPAddress(0, 0, 0, 0);
    IPAddress _serverOverride = IPAddress(0, 0, 0, 0);
    bool _serverOverrideEnabled = false;

    uint8_t _queryTransportId = 0;
    uint16_t _transactionId = 0;
    uint32_t _resultTtl = 0;

    uint8_t _message[FLPROG_DNS_MAX_PACKET_SIZE];
    uint16_t _queryLength = 0;
    uint16_t _sendOffset = 0;
    uint8_t _sendStage = 0;

    uint16_t _responseLength = 0;
    uint16_t _responseRead = 0;

    uint32_t _timeout = FLPROG_DNS_DEFAULT_TIMEOUT;
    uint8_t _maxAttempts = FLPROG_DNS_DEFAULT_ATTEMPTS;
    uint8_t _attemptNumber = 0;
    uint32_t _attemptStarted = 0;
    uint32_t _maxCacheTtl = FLPROG_DNS_MAX_CACHE_TTL;

    CacheEntry _cache[FLPROG_DNS_CACHE_SIZE];

    uint32_t _queryCount = 0;
    uint32_t _cacheHitCount = 0;
    uint32_t _cacheMissCount = 0;
    uint32_t _timeoutCount = 0;
    uint32_t _responseErrorCount = 0;
};
