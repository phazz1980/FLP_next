/*
 * FLProg_Network — flprogNetworkDns.cpp
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
#include "flprogNetworkDns.h"
#include "../flprogNetwork.h"

#include <string.h>

static const uint16_t FLPROG_DNS_PORT = 53;

FLProgNetworkDns::FLProgNetworkDns()
{
    clearCache();
}

FLProgNetworkDns::~FLProgNetworkDns()
{
    end();
}

bool FLProgNetworkDns::begin(
    FLProgNetworkInterface &network,
    uint8_t routeValue)
{
    _route = routeValue;

    if (!attach(network))
    {
        _error = FLPROG_DNS_ERROR_NO_NETWORK;
        _state = FLPROG_DNS_STATE_ERROR;
        return false;
    }

    if (!busy())
    {
        _error = FLPROG_DNS_ERROR_NONE;
        _state = FLPROG_DNS_STATE_IDLE;
    }

    return true;
}

void FLProgNetworkDns::end()
{
    cancel();
    detach();
}

bool FLProgNetworkDns::ipIsZero(IPAddress ip)
{
    return
        ip[0] == 0 &&
        ip[1] == 0 &&
        ip[2] == 0 &&
        ip[3] == 0;
}

bool FLProgNetworkDns::normalizeHostname(
    const char *source,
    char *destination) const
{
    if (source == nullptr || destination == nullptr)
    {
        return false;
    }

    size_t length = strlen(source);

    if (length == 0)
    {
        return false;
    }

    // Конечная root-точка допустима, но в ключе cache она не нужна.
    if (length > 1 && source[length - 1] == '.')
    {
        length--;
    }

    if (
        length == 0 ||
        length > FLPROG_DNS_MAX_HOSTNAME_LENGTH
    )
    {
        return false;
    }

    size_t labelLength = 0;

    for (size_t i = 0; i < length; i++)
    {
        unsigned char value = (unsigned char)source[i];

        if (value == '.')
        {
            if (labelLength == 0 || labelLength > 63)
            {
                return false;
            }

            destination[i] = '.';
            labelLength = 0;
            continue;
        }

        if (value <= 0x20 || value >= 0x7F)
        {
            return false;
        }

        if (value >= 'A' && value <= 'Z')
        {
            value = (unsigned char)(value - 'A' + 'a');
        }

        destination[i] = (char)value;
        labelLength++;

        if (labelLength > 63)
        {
            return false;
        }
    }

    if (labelLength == 0 || labelLength > 63)
    {
        return false;
    }

    destination[length] = 0;
    return true;
}

bool FLProgNetworkDns::parseIpv4Literal(
    const char *text,
    IPAddress &address) const
{
    if (text == nullptr || *text == 0)
    {
        return false;
    }

    uint16_t parts[4] = {0, 0, 0, 0};
    uint8_t part = 0;
    uint8_t digits = 0;

    for (const char *cursor = text; ; cursor++)
    {
        const char value = *cursor;

        if (value >= '0' && value <= '9')
        {
            if (part >= 4)
            {
                return false;
            }

            parts[part] =
                (uint16_t)(parts[part] * 10U + (uint16_t)(value - '0'));

            digits++;

            if (digits > 3 || parts[part] > 255)
            {
                return false;
            }

            continue;
        }

        if (value == '.' || value == 0)
        {
            if (digits == 0 || part >= 4)
            {
                return false;
            }

            if (value == 0)
            {
                if (part != 3)
                {
                    return false;
                }

                address = IPAddress(
                    (uint8_t)parts[0],
                    (uint8_t)parts[1],
                    (uint8_t)parts[2],
                    (uint8_t)parts[3]
                );

                return true;
            }

            part++;
            digits = 0;
            continue;
        }

        return false;
    }
}

void FLProgNetworkDns::resetTransactionState()
{
    _queryServer = IPAddress(0, 0, 0, 0);
    _queryTransportId = FLPROG_TRANSPORT_NONE;
    _result = IPAddress(0, 0, 0, 0);
    _resultTtl = 0;
    _responseCode = 0;

    _queryLength = 0;
    _sendOffset = 0;
    _sendStage = 0;
    _responseLength = 0;
    _responseRead = 0;
    _attemptNumber = 0;
    _attemptStarted = 0;
}

void FLProgNetworkDns::startRequest(
    const char *normalized,
    uint8_t routeValue)
{
    _udp.stop();
    resetTransactionState();

    strncpy(
        _hostname,
        normalized,
        FLPROG_DNS_MAX_HOSTNAME_LENGTH
    );
    _hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH] = 0;

    _route = routeValue;
    _error = FLPROG_DNS_ERROR_NONE;
    _state = FLPROG_DNS_STATE_WAIT_TRANSPORT;

    _transactionId = (uint16_t)(_transactionId + 1U);
    if (_transactionId == 0)
    {
        _transactionId =
            (uint16_t)((millis() ^ 0xA53CU) & 0xFFFFU);
        if (_transactionId == 0)
        {
            _transactionId = 1;
        }
    }
}

uint8_t FLProgNetworkDns::resolve(const char *hostnameValue)
{
    return resolve(hostnameValue, _route);
}

uint8_t FLProgNetworkDns::resolve(
    const char *hostnameValue,
    uint8_t routeValue)
{
    if (serviceNetwork() == nullptr)
    {
        _error = FLPROG_DNS_ERROR_NO_NETWORK;
        _state = FLPROG_DNS_STATE_ERROR;
        return FLPROG_DNS_RESULT_ERROR;
    }

    char normalized[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1];

    if (!normalizeHostname(hostnameValue, normalized))
    {
        _error = FLPROG_DNS_ERROR_INVALID_NAME;
        _state = FLPROG_DNS_STATE_ERROR;
        return FLPROG_DNS_RESULT_ERROR;
    }

    IPAddress literal;
    if (parseIpv4Literal(normalized, literal))
    {
        _udp.stop();
        resetTransactionState();
        strncpy(
            _hostname,
            normalized,
            FLPROG_DNS_MAX_HOSTNAME_LENGTH
        );
        _hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH] = 0;
        _route = routeValue;
        _result = literal;
        _error = FLPROG_DNS_ERROR_NONE;
        _state = FLPROG_DNS_STATE_SUCCESS;
        return FLPROG_DNS_RESULT_SUCCESS;
    }

    if (busy())
    {
        if (
            _route == routeValue &&
            strcmp(_hostname, normalized) == 0
        )
        {
            return FLPROG_DNS_RESULT_WAIT;
        }

        return FLPROG_DNS_RESULT_BUSY;
    }

    FLProgNetworkInterface *network = serviceNetwork();
    const uint8_t transportId =
        network->transportIdForRoute(routeValue, true);

    if (transportId != FLPROG_TRANSPORT_NONE)
    {
        IPAddress serverValue = _serverOverrideEnabled
            ? _serverOverride
            : network->dnsServerForTransport(transportId);

        if (!ipIsZero(serverValue))
        {
            CacheEntry *entry =
                findCache(
                    normalized,
                    transportId,
                    serverValue,
                    true
                );

            if (entry != nullptr)
            {
                strncpy(
                    _hostname,
                    normalized,
                    FLPROG_DNS_MAX_HOSTNAME_LENGTH
                );
                _hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH] = 0;
                _route = routeValue;
                _queryTransportId = transportId;
                _queryServer = serverValue;
                _result = entry->address;
                _resultTtl = 0;
                _error = FLPROG_DNS_ERROR_NONE;
                _state = FLPROG_DNS_STATE_SUCCESS;
                return FLPROG_DNS_RESULT_SUCCESS;
            }

            _cacheMissCount++;
        }
    }

    startRequest(normalized, routeValue);
    return FLPROG_DNS_RESULT_WAIT;
}

void FLProgNetworkDns::cancel()
{
    _udp.stop();
    resetTransactionState();
    _hostname[0] = 0;
    _error = FLPROG_DNS_ERROR_NONE;
    _state = FLPROG_DNS_STATE_IDLE;
}

FLProgNetworkDns::CacheEntry *FLProgNetworkDns::findCache(
    const char *hostnameValue,
    uint8_t transportIdValue,
    IPAddress serverValue,
    bool countHit)
{
    if (hostnameValue == nullptr)
    {
        return nullptr;
    }

    const uint32_t now = millis();

    for (uint8_t i = 0; i < FLPROG_DNS_CACHE_SIZE; i++)
    {
        CacheEntry &entry = _cache[i];

        if (!entry.used)
        {
            continue;
        }

        if (timeReached(now, entry.expiresAt))
        {
            entry.used = false;
            continue;
        }

        if (
            entry.transportId == transportIdValue &&
            ipEqual(entry.server, serverValue) &&
            strcmp(entry.hostname, hostnameValue) == 0
        )
        {
            entry.lastUsed = now;
            if (countHit)
            {
                _cacheHitCount++;
            }
            return &entry;
        }
    }

    return nullptr;
}

bool FLProgNetworkDns::lookupCached(
    const char *hostnameValue,
    uint8_t transportIdValue,
    IPAddress serverValue,
    IPAddress &resultValue)
{
    char normalized[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1];

    if (!normalizeHostname(hostnameValue, normalized))
    {
        return false;
    }

    CacheEntry *entry =
        findCache(
            normalized,
            transportIdValue,
            serverValue,
            true
        );

    if (entry == nullptr)
    {
        return false;
    }

    resultValue = entry->address;
    return true;
}

void FLProgNetworkDns::storeCache(
    const char *hostnameValue,
    uint8_t transportIdValue,
    IPAddress serverValue,
    IPAddress addressValue,
    uint32_t ttl)
{
    if (
        hostnameValue == nullptr ||
        ttl == 0 ||
        _maxCacheTtl == 0 ||
        ipIsZero(addressValue)
    )
    {
        return;
    }

    if (ttl > _maxCacheTtl)
    {
        ttl = _maxCacheTtl;
    }

    // Сравнение millis() использует signed delta, поэтому expiry держим ниже 2^31 ms.
    if (ttl > 2147483UL)
    {
        ttl = 2147483UL;
    }

    const uint32_t now = millis();
    CacheEntry *slot = nullptr;
    uint32_t oldestAge = 0;

    for (uint8_t i = 0; i < FLPROG_DNS_CACHE_SIZE; i++)
    {
        CacheEntry &entry = _cache[i];

        if (
            entry.used &&
            timeReached(now, entry.expiresAt)
        )
        {
            entry.used = false;
        }

        if (!entry.used)
        {
            slot = &entry;
            break;
        }

        if (
            entry.transportId == transportIdValue &&
            ipEqual(entry.server, serverValue) &&
            strcmp(entry.hostname, hostnameValue) == 0
        )
        {
            slot = &entry;
            break;
        }

        const uint32_t age = now - entry.lastUsed;
        if (slot == nullptr || age > oldestAge)
        {
            oldestAge = age;
            slot = &entry;
        }
    }

    if (slot == nullptr)
    {
        return;
    }

    slot->used = true;
    strncpy(
        slot->hostname,
        hostnameValue,
        FLPROG_DNS_MAX_HOSTNAME_LENGTH
    );
    slot->hostname[FLPROG_DNS_MAX_HOSTNAME_LENGTH] = 0;
    slot->address = addressValue;
    slot->server = serverValue;
    slot->transportId = transportIdValue;
    slot->lastUsed = now;
    slot->expiresAt = now + ttl * 1000UL;
}

void FLProgNetworkDns::clearCache()
{
    for (uint8_t i = 0; i < FLPROG_DNS_CACHE_SIZE; i++)
    {
        _cache[i].used = false;
        _cache[i].hostname[0] = 0;
        _cache[i].address = IPAddress(0, 0, 0, 0);
        _cache[i].server = IPAddress(0, 0, 0, 0);
        _cache[i].transportId = FLPROG_TRANSPORT_NONE;
        _cache[i].expiresAt = 0;
        _cache[i].lastUsed = 0;
    }
}

uint8_t FLProgNetworkDns::cacheCount() const
{
    uint8_t count = 0;
    const uint32_t now = millis();

    for (uint8_t i = 0; i < FLPROG_DNS_CACHE_SIZE; i++)
    {
        if (
            _cache[i].used &&
            !timeReached(now, _cache[i].expiresAt)
        )
        {
            count++;
        }
    }

    return count;
}

bool FLProgNetworkDns::encodeQuestion(
    uint8_t *buffer,
    uint16_t &length) const
{
    if (buffer == nullptr || _hostname[0] == 0)
    {
        return false;
    }

    memset(buffer, 0, FLPROG_DNS_MAX_PACKET_SIZE);

    buffer[0] = (uint8_t)(_transactionId >> 8);
    buffer[1] = (uint8_t)_transactionId;
    buffer[2] = 0x01; // RD=1
    buffer[5] = 0x01; // QDCOUNT=1

    uint16_t position = 12;
    const char *label = _hostname;

    while (*label != 0)
    {
        const char *dot = strchr(label, '.');
        const size_t labelLength =
            (dot == nullptr)
                ? strlen(label)
                : (size_t)(dot - label);

        if (
            labelLength == 0 ||
            labelLength > 63 ||
            position + 1U + labelLength + 5U > FLPROG_DNS_MAX_PACKET_SIZE
        )
        {
            return false;
        }

        buffer[position++] = (uint8_t)labelLength;
        memcpy(buffer + position, label, labelLength);
        position = (uint16_t)(position + labelLength);

        if (dot == nullptr)
        {
            break;
        }

        label = dot + 1;
    }

    buffer[position++] = 0;

    // QTYPE=A, QCLASS=IN
    buffer[position++] = 0;
    buffer[position++] = 1;
    buffer[position++] = 0;
    buffer[position++] = 1;

    length = position;
    return true;
}

bool FLProgNetworkDns::prepareQuery()
{
    _queryLength = 0;
    _sendOffset = 0;
    _sendStage = 0;
    _responseLength = 0;
    _responseRead = 0;

    if (!encodeQuestion(_message, _queryLength))
    {
        finishError(FLPROG_DNS_ERROR_INVALID_NAME);
        return false;
    }

    _state = FLPROG_DNS_STATE_SENDING;
    return true;
}

bool FLProgNetworkDns::selectTransportAndServer()
{
    FLProgNetworkInterface *network = serviceNetwork();

    if (network == nullptr)
    {
        finishError(FLPROG_DNS_ERROR_NO_NETWORK);
        return false;
    }

    const uint8_t selected =
        network->transportIdForRoute(_route, true);

    if (selected == FLPROG_TRANSPORT_NONE)
    {
        return false;
    }

    const IPAddress serverValue = _serverOverrideEnabled
        ? _serverOverride
        : network->dnsServerForTransport(selected);

    if (ipIsZero(serverValue))
    {
        finishError(FLPROG_DNS_ERROR_NO_SERVER);
        return false;
    }

    _queryTransportId = selected;
    _queryServer = serverValue;

    CacheEntry *entry =
        findCache(
            _hostname,
            _queryTransportId,
            _queryServer,
            true
        );

    if (entry != nullptr)
    {
        finishSuccess(entry->address, 0, true);
        return false;
    }

    return true;
}

bool FLProgNetworkDns::startUdp()
{
    FLProgNetworkInterface *network = serviceNetwork();

    if (network == nullptr)
    {
        finishError(FLPROG_DNS_ERROR_NO_NETWORK);
        return false;
    }

    if (!_udp.begin(*network, _route, 0))
    {
        finishError(FLPROG_DNS_ERROR_UDP);
        return false;
    }

    _state = FLPROG_DNS_STATE_OPENING;
    return true;
}

bool FLProgNetworkDns::sendQueryStep()
{
    if (!_udp.isOpen())
    {
        return false;
    }

    if (_sendStage == 0)
    {
        if (_udp.beginPacket(_queryServer, FLPROG_DNS_PORT) == 0)
        {
            return false;
        }

        _sendStage = 1;
    }

    if (_sendStage == 1)
    {
        const size_t remaining =
            (size_t)(_queryLength - _sendOffset);

        if (remaining > 0)
        {
            const size_t written =
                _udp.write(
                    _message + _sendOffset,
                    remaining
                );

            if (written == 0)
            {
                return false;
            }

            _sendOffset =
                (uint16_t)(_sendOffset + written);

            if (_sendOffset < _queryLength)
            {
                return false;
            }
        }

        _sendStage = 2;
    }

    if (_sendStage == 2)
    {
        if (_udp.endPacket() == 0)
        {
            return false;
        }

        _sendStage = 3;
        _attemptNumber++;
        _attemptStarted = millis();
        _queryCount++;
        _state = FLPROG_DNS_STATE_WAIT_RESPONSE;
        return true;
    }

    return true;
}

void FLProgNetworkDns::retryOrTimeout()
{
    if (_attemptNumber >= _maxAttempts)
    {
        _timeoutCount++;
        finishError(FLPROG_DNS_ERROR_TIMEOUT);
        return;
    }

    _sendOffset = 0;
    _sendStage = 0;
    _responseLength = 0;
    _responseRead = 0;
    _state = FLPROG_DNS_STATE_SENDING;
}

void FLProgNetworkDns::waitResponseStep()
{
    const int packetSize = _udp.parsePacket();

    if (packetSize > 0)
    {
        if (
            _udp.remotePort() != FLPROG_DNS_PORT ||
            !ipEqual(_udp.remoteIP(), _queryServer)
        )
        {
            _udp.flush();
        }
        else if (
            packetSize < 12 ||
            packetSize > FLPROG_DNS_MAX_PACKET_SIZE
        )
        {
            _udp.flush();
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return;
        }
        else
        {
            _responseLength = (uint16_t)packetSize;
            _responseRead = 0;
            _state = FLPROG_DNS_STATE_READ_RESPONSE;
            return;
        }
    }

    if (
        _attemptStarted != 0 &&
        (uint32_t)(millis() - _attemptStarted) >= _timeout
    )
    {
        retryOrTimeout();
    }
}

void FLProgNetworkDns::readResponseStep()
{
    if (_responseRead < _responseLength)
    {
        const int received =
            _udp.read(
                _message + _responseRead,
                (size_t)(_responseLength - _responseRead)
            );

        if (received > 0)
        {
            _responseRead =
                (uint16_t)(_responseRead + (uint16_t)received);
        }
    }

    if (_responseRead < _responseLength)
    {
        if (
            _attemptStarted != 0 &&
            (uint32_t)(millis() - _attemptStarted) >= _timeout
        )
        {
            _udp.flush();
            retryOrTimeout();
        }
        return;
    }

    IPAddress address;
    uint32_t ttl = 0;

    if (parseResponse(address, ttl))
    {
        finishSuccess(address, ttl, false);
    }
}

uint16_t FLProgNetworkDns::read16(
    const uint8_t *buffer,
    uint16_t offset) const
{
    return
        ((uint16_t)buffer[offset] << 8) |
        buffer[offset + 1];
}

uint32_t FLProgNetworkDns::read32(
    const uint8_t *buffer,
    uint16_t offset) const
{
    return
        ((uint32_t)buffer[offset] << 24) |
        ((uint32_t)buffer[offset + 1] << 16) |
        ((uint32_t)buffer[offset + 2] << 8) |
        (uint32_t)buffer[offset + 3];
}

bool FLProgNetworkDns::skipName(
    const uint8_t *buffer,
    uint16_t length,
    uint16_t &offset) const
{
    while (offset < length)
    {
        const uint8_t value = buffer[offset++];

        if (value == 0)
        {
            return true;
        }

        if ((value & 0xC0U) == 0xC0U)
        {
            if (offset >= length)
            {
                return false;
            }

            offset++;
            return true;
        }

        if ((value & 0xC0U) != 0 || value > 63)
        {
            return false;
        }

        if ((uint32_t)offset + value > length)
        {
            return false;
        }

        offset = (uint16_t)(offset + value);
    }

    return false;
}

bool FLProgNetworkDns::parseResponse(
    IPAddress &address,
    uint32_t &ttl)
{
    if (_responseLength < 12)
    {
        _responseErrorCount++;
        finishError(FLPROG_DNS_ERROR_RESPONSE);
        return false;
    }

    if (read16(_message, 0) != _transactionId)
    {
        _responseErrorCount++;
        finishError(FLPROG_DNS_ERROR_RESPONSE);
        return false;
    }

    const uint16_t flags = read16(_message, 2);

    if ((flags & 0x8000U) == 0)
    {
        _responseErrorCount++;
        finishError(FLPROG_DNS_ERROR_RESPONSE);
        return false;
    }

    if ((flags & 0x0200U) != 0)
    {
        _responseErrorCount++;
        finishError(FLPROG_DNS_ERROR_TRUNCATED);
        return false;
    }

    _responseCode = (uint8_t)(flags & 0x000FU);

    if (_responseCode == 3)
    {
        finishError(FLPROG_DNS_ERROR_NXDOMAIN);
        return false;
    }

    if (_responseCode != 0)
    {
        _responseErrorCount++;
        finishError(FLPROG_DNS_ERROR_RESPONSE);
        return false;
    }

    const uint16_t questionCount = read16(_message, 4);
    const uint16_t answerCount = read16(_message, 6);
    uint16_t offset = 12;

    for (uint16_t i = 0; i < questionCount; i++)
    {
        if (!skipName(_message, _responseLength, offset))
        {
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return false;
        }

        if ((uint32_t)offset + 4U > _responseLength)
        {
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return false;
        }

        offset = (uint16_t)(offset + 4U);
    }

    for (uint16_t i = 0; i < answerCount; i++)
    {
        if (!skipName(_message, _responseLength, offset))
        {
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return false;
        }

        if ((uint32_t)offset + 10U > _responseLength)
        {
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return false;
        }

        const uint16_t type = read16(_message, offset);
        const uint16_t recordClass = read16(_message, (uint16_t)(offset + 2));
        const uint32_t recordTtl = read32(_message, (uint16_t)(offset + 4));
        const uint16_t dataLength = read16(_message, (uint16_t)(offset + 8));
        offset = (uint16_t)(offset + 10U);

        if ((uint32_t)offset + dataLength > _responseLength)
        {
            _responseErrorCount++;
            finishError(FLPROG_DNS_ERROR_RESPONSE);
            return false;
        }

        if (
            type == 1 &&
            recordClass == 1 &&
            dataLength == 4
        )
        {
            address = IPAddress(
                _message[offset],
                _message[offset + 1],
                _message[offset + 2],
                _message[offset + 3]
            );
            ttl = recordTtl;

            if (!ipIsZero(address))
            {
                return true;
            }
        }

        offset = (uint16_t)(offset + dataLength);
    }

    finishError(FLPROG_DNS_ERROR_NO_A_RECORD);
    return false;
}

void FLProgNetworkDns::finishSuccess(
    IPAddress address,
    uint32_t ttl,
    bool fromCache)
{
    _result = address;
    _resultTtl = ttl;
    _error = FLPROG_DNS_ERROR_NONE;
    _state = FLPROG_DNS_STATE_SUCCESS;

    if (!fromCache)
    {
        storeCache(
            _hostname,
            _queryTransportId,
            _queryServer,
            address,
            ttl
        );
    }

    _udp.stop();
}

void FLProgNetworkDns::finishError(uint8_t errorValue)
{
    _error = errorValue;
    _state = FLPROG_DNS_STATE_ERROR;
    _udp.stop();
}

void FLProgNetworkDns::pool()
{
    if (!busy())
    {
        return;
    }

    if (_state == FLPROG_DNS_STATE_WAIT_TRANSPORT)
    {
        if (!selectTransportAndServer())
        {
            return;
        }

        startUdp();
        return;
    }

    if (_state == FLPROG_DNS_STATE_OPENING)
    {
        if (_udp.state() == FLPROG_SOCKET_STATE_ERROR)
        {
            finishError(FLPROG_DNS_ERROR_UDP);
            return;
        }

        if (!_udp.isOpen())
        {
            return;
        }

        const uint8_t actualTransport = _udp.boundTransportId();

        if (
            actualTransport != FLPROG_TRANSPORT_NONE &&
            actualTransport != _queryTransportId
        )
        {
            _queryTransportId = actualTransport;
            _queryServer = _serverOverrideEnabled
                ? _serverOverride
                : serviceNetwork()->dnsServerForTransport(actualTransport);

            if (ipIsZero(_queryServer))
            {
                finishError(FLPROG_DNS_ERROR_NO_SERVER);
                return;
            }

            CacheEntry *entry =
                findCache(
                    _hostname,
                    _queryTransportId,
                    _queryServer,
                    true
                );

            if (entry != nullptr)
            {
                finishSuccess(entry->address, 0, true);
                return;
            }
        }

        prepareQuery();
        return;
    }

    if (_state == FLPROG_DNS_STATE_SENDING)
    {
        sendQueryStep();
        return;
    }

    if (_state == FLPROG_DNS_STATE_WAIT_RESPONSE)
    {
        waitResponseStep();
        return;
    }

    if (_state == FLPROG_DNS_STATE_READ_RESPONSE)
    {
        readResponseStep();
    }
}
