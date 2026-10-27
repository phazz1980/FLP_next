/*
 * FLProg_Network — flprogNetworkNtp.cpp
 * Назначение: NTP client/service поверх UDP.
 *
 * Архитектурные заметки:
 * - Запрос/ответ NTP использует native UDP и route выбранного NetworkDevice.
 * - Таймаут NTP не является transport failure.
 * - Повторный запрос должен быть безопасен после смены generation.
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
#include "flprogNetworkNtp.h"
#include "../flprogNetwork.h"
#include <string.h>

FLProgNetworkNtp::~FLProgNetworkNtp()
{
    end();
}

bool FLProgNetworkNtp::begin(
    FLProgNetworkInterface &network,
    uint8_t routeValue)
{
    _route = routeValue;

    if (!attach(network))
    {
        _error = FLPROG_NTP_ERROR_SERVICE_LIMIT;
        _state = FLPROG_NTP_STATE_ERROR;
        return false;
    }

    _enabled = true;
    _requestPending = true;
    _nextRequestAt = millis();
    _error = (_serverMode == FLPROG_NTP_SERVER_NONE)
        ? FLPROG_NTP_ERROR_NO_SERVER
        : FLPROG_NTP_ERROR_NONE;
    _state = FLPROG_NTP_STATE_IDLE;
    return true;
}

void FLProgNetworkNtp::end()
{
    cancelActive(false);

    if (_dnsAttached)
    {
        _dns.end();
        _dnsAttached = false;
    }

    detach();
    _state = FLPROG_NTP_STATE_DISABLED;
}

bool FLProgNetworkNtp::ipIsZero(IPAddress value)
{
    return value[0] == 0 && value[1] == 0 &&
           value[2] == 0 && value[3] == 0;
}

bool FLProgNetworkNtp::isLeapYear(uint16_t value)
{
    return (value % 4U == 0U) &&
           ((value % 100U != 0U) || (value % 400U == 0U));
}

uint8_t FLProgNetworkNtp::daysInMonth(uint16_t yearValue, uint8_t monthValue)
{
    static const uint8_t days[12] =
        {31,28,31,30,31,30,31,31,30,31,30,31};

    if (monthValue < 1 || monthValue > 12)
    {
        return 31;
    }

    if (monthValue == 2 && isLeapYear(yearValue))
    {
        return 29;
    }

    return days[monthValue - 1];
}

void FLProgNetworkNtp::setState(uint8_t value)
{
    _state = value;
}

void FLProgNetworkNtp::setEnabled(bool value)
{
    if (_enabled == value)
    {
        return;
    }

    _enabled = value;

    if (value)
    {
        _requestPending = true;
        _nextRequestAt = millis();
    }
    else
    {
        _restartRequested = true;
    }
}

void FLProgNetworkNtp::setRoute(uint8_t value)
{
    if (_route == value)
    {
        return;
    }

    _route = value;
    _restartRequested = true;
    _requestPending = true;

    if (_dnsAttached)
    {
        _dns.setRoute(value);
    }
}

void FLProgNetworkNtp::setServer(const char *value)
{
    if (value == nullptr || value[0] == 0)
    {
        clearServer();
        return;
    }

    char normalized[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1];
    size_t length = strlen(value);
    if (length > FLPROG_DNS_MAX_HOSTNAME_LENGTH)
    {
        length = FLPROG_DNS_MAX_HOSTNAME_LENGTH;
    }

    memcpy(normalized, value, length);
    normalized[length] = 0;

    if (
        _serverMode == FLPROG_NTP_SERVER_NAME &&
        strcmp(_serverName, normalized) == 0
    )
    {
        return;
    }

    memcpy(_serverName, normalized, length + 1);
    _serverMode = FLPROG_NTP_SERVER_NAME;
    _serverIp = IPAddress(0, 0, 0, 0);
    _resolvedIp = IPAddress(0, 0, 0, 0);
    _error = FLPROG_NTP_ERROR_NONE;
    _restartRequested = true;
    _requestPending = true;
    _nextRequestAt = millis();
}

void FLProgNetworkNtp::setServer(IPAddress value)
{
    if (ipIsZero(value))
    {
        clearServer();
        return;
    }

    if (
        _serverMode == FLPROG_NTP_SERVER_IP &&
        _serverIp == value
    )
    {
        return;
    }

    _serverMode = FLPROG_NTP_SERVER_IP;
    _serverIp = value;
    _resolvedIp = value;
    _serverName[0] = 0;
    _error = FLPROG_NTP_ERROR_NONE;
    _restartRequested = true;
    _requestPending = true;
    _nextRequestAt = millis();
}

void FLProgNetworkNtp::clearServer()
{
    if (_serverMode == FLPROG_NTP_SERVER_NONE)
    {
        return;
    }

    _serverMode = FLPROG_NTP_SERVER_NONE;
    _serverName[0] = 0;
    _serverIp = IPAddress(0, 0, 0, 0);
    _resolvedIp = IPAddress(0, 0, 0, 0);
    _error = FLPROG_NTP_ERROR_NO_SERVER;
    _restartRequested = true;
    _requestPending = false;
}


void FLProgNetworkNtp::setUseAsClock(bool value)
{
    if (_useAsClock == value)
    {
        return;
    }

    _useAsClock = value;
    _clockAnchorMillis = millis();
}

void FLProgNetworkNtp::setRequestPeriodMs(uint32_t value)
{
    _requestPeriodMs = value;

    if (!busy() && !_requestPending)
    {
        _nextRequestAt = millis() + _requestPeriodMs;
    }
}

bool FLProgNetworkNtp::syncNow()
{
    if (!isAttached() || !_enabled || _serverMode == FLPROG_NTP_SERVER_NONE)
    {
        return false;
    }

    if (busy())
    {
        return false;
    }

    _requestPending = true;
    _nextRequestAt = millis();
    _error = FLPROG_NTP_ERROR_NONE;
    return true;
}

void FLProgNetworkNtp::cancel()
{
    if (!isAttached())
    {
        return;
    }

    _cancelRequested = true;
    _restartRequested = false;
    _requestPending = false;
}

void FLProgNetworkNtp::cancelActive(bool countEvent)
{
    const bool wasBusy = busy();

    _udp.stop();

    if (_dnsAttached && _dns.busy())
    {
        _dns.cancel();
    }

    _resolvedIp = (_serverMode == FLPROG_NTP_SERVER_IP)
        ? _serverIp
        : IPAddress(0, 0, 0, 0);

    if (countEvent && wasBusy)
    {
        _cancelCount++;
        _events |= FLPROG_NTP_EVENT_CANCELED;
    }

    if (_enabled)
    {
        setState(FLPROG_NTP_STATE_IDLE);
    }
    else
    {
        setState(FLPROG_NTP_STATE_DISABLED);
    }
}

bool FLProgNetworkNtp::ensureDns()
{
    if (_dnsAttached)
    {
        _dns.setRoute(_route);
        return true;
    }

    FLProgNetworkInterface *network = serviceNetwork();
    if (network == nullptr)
    {
        return false;
    }

    if (!_dns.begin(*network, _route))
    {
        return false;
    }

    _dnsAttached = true;
    return true;
}

void FLProgNetworkNtp::scheduleNext()
{
    _requestPending = false;

    // Legacy FLProgNTP measured reqestPeriod from the packet-send moment.
    // Preserve that behavior: long response timeouts do not silently add an
    // extra full period after the transaction finishes.
    const uint32_t anchor = (_requestSentAt != 0)
        ? _requestSentAt
        : millis();
    _nextRequestAt = anchor + _requestPeriodMs;
}

void FLProgNetworkNtp::startTransaction()
{
    _requestPending = false;
    _error = FLPROG_NTP_ERROR_NONE;
    _requestStartedAt = millis();
    _requestSentAt = 0;
    _lastRoundTripMs = 0;
    _requestCount++;
    setState(FLPROG_NTP_STATE_WAIT_NETWORK);
}

void FLProgNetworkNtp::waitNetworkStep()
{
    FLProgNetworkInterface *network = serviceNetwork();

    if (network == nullptr)
    {
        finishError(FLPROG_NTP_ERROR_NO_NETWORK);
        return;
    }

    if (!network->workMode())
    {
        return;
    }

    const uint8_t transportId =
        network->transportIdForRoute(_route, true);

    if (transportId == FLPROG_TRANSPORT_NONE)
    {
        return;
    }

    if (_serverMode == FLPROG_NTP_SERVER_IP)
    {
        _resolvedIp = _serverIp;
        setState(FLPROG_NTP_STATE_OPENING_UDP);
        return;
    }

    if (_serverMode != FLPROG_NTP_SERVER_NAME)
    {
        finishError(FLPROG_NTP_ERROR_NO_SERVER);
        return;
    }

    if (!ensureDns())
    {
        _dnsErrorCount++;
        finishError(FLPROG_NTP_ERROR_SERVICE_LIMIT);
        return;
    }

    const uint8_t result = _dns.resolve(_serverName, _route);

    if (result == FLPROG_DNS_RESULT_SUCCESS)
    {
        _resolvedIp = _dns.result();
        setState(FLPROG_NTP_STATE_OPENING_UDP);
        return;
    }

    if (result == FLPROG_DNS_RESULT_ERROR)
    {
        _dnsErrorCount++;
        finishError(FLPROG_NTP_ERROR_DNS);
        return;
    }

    setState(FLPROG_NTP_STATE_RESOLVING);
}

void FLProgNetworkNtp::resolvingStep()
{
    if (!_dnsAttached)
    {
        _dnsErrorCount++;
        finishError(FLPROG_NTP_ERROR_DNS);
        return;
    }

    if (_dns.ready())
    {
        _resolvedIp = _dns.result();

        if (ipIsZero(_resolvedIp))
        {
            _dnsErrorCount++;
            finishError(FLPROG_NTP_ERROR_DNS);
            return;
        }

        setState(FLPROG_NTP_STATE_OPENING_UDP);
        return;
    }

    if (_dns.failed())
    {
        _dnsErrorCount++;
        finishError(FLPROG_NTP_ERROR_DNS);
    }
}

void FLProgNetworkNtp::openUdpStep()
{
    FLProgNetworkInterface *network = serviceNetwork();
    if (network == nullptr)
    {
        finishError(FLPROG_NTP_ERROR_NO_NETWORK);
        return;
    }

    if (!_udp.isAttached())
    {
        if (!_udp.begin(*network, _route, _localPort))
        {
            finishError(FLPROG_NTP_ERROR_UDP_OPEN);
            return;
        }
        return;
    }

    if (_udp.isOpen())
    {
        setState(FLPROG_NTP_STATE_SENDING);
        return;
    }

    if (
        _udp.state() == FLPROG_SOCKET_STATE_ERROR ||
        _udp.state() == FLPROG_SOCKET_STATE_UNSUPPORTED
    )
    {
        finishError(FLPROG_NTP_ERROR_UDP_OPEN);
    }
}

void FLProgNetworkNtp::buildRequestPacket()
{
    memset(_packet, 0, sizeof(_packet));

    // LI=0, VN=4, Mode=3 (client).
    _packet[0] = 0x23;
    _packet[2] = 6;
    _packet[3] = 0xEC;

    // The client transmit timestamp is used only as a transaction token.
    // Servers copy it verbatim into the response Originate Timestamp.
    const uint32_t a = millis();
    const uint32_t b = micros() ^ (_requestCount * 2654435761UL);

    _requestToken[0] = (uint8_t)(a >> 24);
    _requestToken[1] = (uint8_t)(a >> 16);
    _requestToken[2] = (uint8_t)(a >> 8);
    _requestToken[3] = (uint8_t)a;
    _requestToken[4] = (uint8_t)(b >> 24);
    _requestToken[5] = (uint8_t)(b >> 16);
    _requestToken[6] = (uint8_t)(b >> 8);
    _requestToken[7] = (uint8_t)b;

    memcpy(&_packet[40], _requestToken, sizeof(_requestToken));
}

void FLProgNetworkNtp::sendingStep()
{
    if (!_udp.isOpen())
    {
        setState(FLPROG_NTP_STATE_OPENING_UDP);
        return;
    }

    buildRequestPacket();

    if (_udp.beginPacket(_resolvedIp, FLPROG_NTP_SERVER_PORT) <= 0)
    {
        finishError(FLPROG_NTP_ERROR_SEND);
        return;
    }

    if (_udp.write(_packet, sizeof(_packet)) != sizeof(_packet))
    {
        finishError(FLPROG_NTP_ERROR_SEND);
        return;
    }

    if (_udp.endPacket() <= 0)
    {
        finishError(FLPROG_NTP_ERROR_SEND);
        return;
    }

    _requestSentAt = millis();
    setState(FLPROG_NTP_STATE_WAIT_RESPONSE);
}

bool FLProgNetworkNtp::validateResponse(
    uint8_t &responseError,
    uint32_t &unixTimeValue)
{
    responseError = FLPROG_NTP_ERROR_NONE;
    unixTimeValue = 0;

    const uint8_t li = (uint8_t)((_packet[0] >> 6) & 0x03U);
    const uint8_t version = (uint8_t)((_packet[0] >> 3) & 0x07U);
    const uint8_t mode = (uint8_t)(_packet[0] & 0x07U);
    const uint8_t stratum = _packet[1];

    if (li == 3)
    {
        responseError = FLPROG_NTP_ERROR_UNSYNCHRONIZED;
        return false;
    }

    if (version < 3 || version > 4 || mode != 4)
    {
        responseError = FLPROG_NTP_ERROR_INVALID_PACKET;
        return false;
    }

    if (stratum == 0)
    {
        responseError = FLPROG_NTP_ERROR_KISS_OF_DEATH;
        return false;
    }

    if (stratum > 15)
    {
        responseError = FLPROG_NTP_ERROR_INVALID_PACKET;
        return false;
    }

    if (
        _strictOriginateCheck &&
        memcmp(&_packet[24], _requestToken, sizeof(_requestToken)) != 0
    )
    {
        responseError = FLPROG_NTP_ERROR_INVALID_PACKET;
        return false;
    }

    const uint32_t seconds1900 =
        ((uint32_t)_packet[40] << 24) |
        ((uint32_t)_packet[41] << 16) |
        ((uint32_t)_packet[42] << 8) |
        (uint32_t)_packet[43];

    // Unfold the 32-bit NTP seconds field into the uint32_t Unix range.
    // Era 0 values >= 1970 map directly to Unix 1970..2036. After the NTP
    // rollover, era 1 starts at NTP seconds 0 and maps to Unix 2036..2106.
    // This is unambiguous for the library's uint32_t Unix API.
    if (seconds1900 >= FLPROG_NTP_EPOCH_DELTA)
    {
        unixTimeValue = seconds1900 - FLPROG_NTP_EPOCH_DELTA;
    }
    else
    {
        unixTimeValue = seconds1900 + FLPROG_NTP_ERA1_UNIX_BASE;
    }
    return true;
}

void FLProgNetworkNtp::waitResponseStep()
{
    if (
        _timeoutMs > 0 &&
        (uint32_t)(millis() - _requestSentAt) >= _timeoutMs
    )
    {
        _timeoutCount++;
        finishError(FLPROG_NTP_ERROR_TIMEOUT);
        return;
    }

    const int packetSize = _udp.parsePacket();
    if (packetSize <= 0)
    {
        return;
    }

    if (
        _strictSourceCheck &&
        (_udp.remoteIP() != _resolvedIp ||
         _udp.remotePort() != FLPROG_NTP_SERVER_PORT)
    )
    {
        _ignoredPacketCount++;
        _udp.flush();
        return;
    }

    if (packetSize < FLPROG_NTP_PACKET_SIZE)
    {
        _invalidPacketCount++;
        _udp.flush();
        return;
    }

    const int readLength = _udp.read(_packet, sizeof(_packet));
    _udp.flush();

    if (readLength < FLPROG_NTP_PACKET_SIZE)
    {
        _invalidPacketCount++;
        return;
    }

    uint8_t responseError = FLPROG_NTP_ERROR_NONE;
    uint32_t unixTimeValue = 0;

    if (!validateResponse(responseError, unixTimeValue))
    {
        _invalidPacketCount++;

        if (
            responseError == FLPROG_NTP_ERROR_KISS_OF_DEATH ||
            responseError == FLPROG_NTP_ERROR_UNSYNCHRONIZED ||
            responseError == FLPROG_NTP_ERROR_BAD_EPOCH
        )
        {
            finishError(responseError);
        }
        return;
    }

    finishSuccess(unixTimeValue);
}

void FLProgNetworkNtp::updateDateTime(uint32_t unixTimeValue)
{
    _dateTime.unixTime = unixTimeValue;

    uint32_t secondsOfDay = unixTimeValue % 86400UL;
    uint32_t days = unixTimeValue / 86400UL;

    _dateTime.hours = (uint8_t)(secondsOfDay / 3600UL);
    secondsOfDay %= 3600UL;
    _dateTime.minutes = (uint8_t)(secondsOfDay / 60UL);
    _dateTime.seconds = (uint8_t)(secondsOfDay % 60UL);

    uint16_t yearValue = 1970;
    while (true)
    {
        const uint16_t yearDays = isLeapYear(yearValue) ? 366U : 365U;
        if (days < yearDays)
        {
            break;
        }
        days -= yearDays;
        yearValue++;
    }

    uint8_t monthValue = 1;
    while (monthValue <= 12)
    {
        const uint8_t monthDays = daysInMonth(yearValue, monthValue);
        if (days < monthDays)
        {
            break;
        }
        days -= monthDays;
        monthValue++;
    }

    _dateTime.year = yearValue;
    _dateTime.month = monthValue;
    _dateTime.day = (uint8_t)(days + 1U);
}

void FLProgNetworkNtp::updateClock()
{
    if (!_hasTime || !_useAsClock)
    {
        return;
    }

    const uint32_t now = millis();
    const uint32_t elapsedMs = now - _clockAnchorMillis;
    const uint32_t elapsedSeconds = elapsedMs / 1000UL;

    if (elapsedSeconds == 0)
    {
        return;
    }

    updateDateTime(_dateTime.unixTime + elapsedSeconds);
    _clockAnchorMillis += elapsedSeconds * 1000UL;
}

void FLProgNetworkNtp::finishSuccess(uint32_t unixTimeValue)
{
    _lastRoundTripMs = millis() - _requestSentAt;
    _lastSyncMillis = millis();
    _clockAnchorMillis = _lastSyncMillis;
    updateDateTime(unixTimeValue);
    _hasTime = true;
    _successCount++;
    _error = FLPROG_NTP_ERROR_NONE;
    _events |= FLPROG_NTP_EVENT_SYNCED;

    closeTransactionResources();
    setState(FLPROG_NTP_STATE_IDLE);
    scheduleNext();

    if (_callback != nullptr)
    {
        _callback();
    }
}

void FLProgNetworkNtp::finishError(uint8_t value)
{
    _error = value;
    _events |= FLPROG_NTP_EVENT_FAILED;

    if (value == FLPROG_NTP_ERROR_TIMEOUT)
    {
        _events |= FLPROG_NTP_EVENT_TIMEOUT;
    }

    closeTransactionResources();
    setState(FLPROG_NTP_STATE_ERROR);
    scheduleNext();
}

void FLProgNetworkNtp::closeTransactionResources()
{
    _udp.stop();

    if (_dnsAttached && _dns.busy())
    {
        _dns.cancel();
    }
}

void FLProgNetworkNtp::pool()
{
    updateClock();

    if (!isAttached())
    {
        return;
    }

    if (_cancelRequested)
    {
        _cancelRequested = false;
        cancelActive(true);
        scheduleNext();
        return;
    }

    if (_restartRequested)
    {
        _restartRequested = false;
        cancelActive(false);

        if (_enabled && _serverMode != FLPROG_NTP_SERVER_NONE)
        {
            _requestPending = true;
            _nextRequestAt = millis();
        }
    }

    if (!_enabled)
    {
        if (busy())
        {
            cancelActive(false);
        }
        setState(FLPROG_NTP_STATE_DISABLED);
        return;
    }

    if (_serverMode == FLPROG_NTP_SERVER_NONE)
    {
        if (busy())
        {
            cancelActive(false);
        }
        _error = FLPROG_NTP_ERROR_NO_SERVER;
        setState(FLPROG_NTP_STATE_IDLE);
        return;
    }

    switch (_state)
    {
    case FLPROG_NTP_STATE_WAIT_NETWORK:
        waitNetworkStep();
        return;

    case FLPROG_NTP_STATE_RESOLVING:
        resolvingStep();
        return;

    case FLPROG_NTP_STATE_OPENING_UDP:
        openUdpStep();
        return;

    case FLPROG_NTP_STATE_SENDING:
        sendingStep();
        return;

    case FLPROG_NTP_STATE_WAIT_RESPONSE:
        waitResponseStep();
        return;

    default:
        break;
    }

    if (_requestPending || timeReached(millis(), _nextRequestAt))
    {
        startTransaction();
    }
}
