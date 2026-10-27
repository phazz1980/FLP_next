/*
 * FLProg_Network — flprogNetworkHttpClient.cpp
 * Назначение: HTTP client поверх native TCP/DNS.
 *
 * Архитектурные заметки:
 * - DNS, TCP connect, request send и response parse выполняются поэтапно из pool().
 * - Таймауты не должны блокировать основной цикл FLProg.
 * - Legacy WebClient позже должен быть фасадом над этим native API.
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
#include "flprogNetworkHttpClient.h"
#include "../flprogNetwork.h"
#include <string.h>
#include <stdio.h>

static bool flprogHttpParseIpv4Literal(const char *value, IPAddress &result)
{
    if (value == nullptr || value[0] == 0) return false;
    uint8_t parts[4] = {0, 0, 0, 0};
    const char *p = value;
    for (uint8_t part = 0; part < 4; ++part)
    {
        if (*p < '0' || *p > '9') return false;
        uint16_t number = 0;
        uint8_t digits = 0;
        while (*p >= '0' && *p <= '9')
        {
            number = (uint16_t)(number * 10U + (uint8_t)(*p - '0'));
            if (number > 255U || ++digits > 3U) return false;
            ++p;
        }
        parts[part] = (uint8_t)number;
        if (part < 3)
        {
            if (*p != '.') return false;
            ++p;
        }
    }
    if (*p != 0) return false;
    result = IPAddress(parts[0], parts[1], parts[2], parts[3]);
    return true;
}

FLProgNetworkHttpClient::~FLProgNetworkHttpClient()
{
    end();
}

bool FLProgNetworkHttpClient::ipIsZero(IPAddress value)
{
    return value[0] == 0 && value[1] == 0 && value[2] == 0 && value[3] == 0;
}

char FLProgNetworkHttpClient::asciiLower(char value)
{
    if (value >= 'A' && value <= 'Z') return (char)(value + ('a' - 'A'));
    return value;
}

bool FLProgNetworkHttpClient::startsWithIgnoreCase(const char *value, const char *prefix)
{
    if (value == nullptr || prefix == nullptr) return false;
    while (*prefix != 0)
    {
        if (*value == 0 || asciiLower(*value) != asciiLower(*prefix)) return false;
        ++value;
        ++prefix;
    }
    return true;
}

bool FLProgNetworkHttpClient::containsTokenIgnoreCase(const char *value, const char *token)
{
    if (value == nullptr || token == nullptr || token[0] == 0) return false;
    const size_t tokenLength = strlen(token);
    for (const char *p = value; *p != 0; ++p)
    {
        size_t i = 0;
        while (i < tokenLength && p[i] != 0 && asciiLower(p[i]) == asciiLower(token[i])) ++i;
        if (i == tokenLength) return true;
    }
    return false;
}

const char *FLProgNetworkHttpClient::skipSpaces(const char *value)
{
    if (value == nullptr) return "";
    while (*value == ' ' || *value == '\t') ++value;
    return value;
}

bool FLProgNetworkHttpClient::parseUnsigned(const char *value, uint32_t &result)
{
    value = skipSpaces(value);
    if (*value < '0' || *value > '9') return false;
    uint32_t parsed = 0;
    while (*value >= '0' && *value <= '9')
    {
        const uint8_t digit = (uint8_t)(*value - '0');
        if (parsed > (0xFFFFFFFFUL - digit) / 10UL) return false;
        parsed = parsed * 10UL + digit;
        ++value;
    }
    while (*value == ' ' || *value == '\t') ++value;
    if (*value != 0) return false;
    result = parsed;
    return true;
}

bool FLProgNetworkHttpClient::parseHexUnsigned(const char *value, uint32_t &result)
{
    value = skipSpaces(value);
    bool any = false;
    uint32_t parsed = 0;
    while (*value != 0 && *value != ';' && *value != ' ' && *value != '\t')
    {
        uint8_t digit = 0;
        if (*value >= '0' && *value <= '9') digit = (uint8_t)(*value - '0');
        else if (*value >= 'a' && *value <= 'f') digit = (uint8_t)(*value - 'a' + 10);
        else if (*value >= 'A' && *value <= 'F') digit = (uint8_t)(*value - 'A' + 10);
        else return false;
        if (parsed > (0xFFFFFFFFUL - digit) / 16UL) return false;
        parsed = parsed * 16UL + digit;
        any = true;
        ++value;
    }
    if (!any) return false;
    result = parsed;
    return true;
}

bool FLProgNetworkHttpClient::methodHasNoResponseBody(const char *method)
{
    return method != nullptr && startsWithIgnoreCase(method, "HEAD") && strlen(method) == 4;
}

bool FLProgNetworkHttpClient::responseStatusHasNoBody(int statusCode)
{
    return (statusCode >= 100 && statusCode < 200) || statusCode == 204 || statusCode == 304;
}

bool FLProgNetworkHttpClient::begin(FLProgNetworkInterface &network, uint8_t routeValue)
{
    end();
    _route = routeValue;
    if (!attach(network))
    {
        _state = FLPROG_HTTP_STATE_ERROR;
        _error = FLPROG_HTTP_ERROR_SERVICE_LIMIT;
        return false;
    }
    if (!_tcp.begin(network, routeValue))
    {
        detach();
        _state = FLPROG_HTTP_STATE_ERROR;
        _error = FLPROG_HTTP_ERROR_SOCKET_REGISTRY;
        return false;
    }
    _tcpAttached = true;
    _tcp.setAutoReconnect(false);
    _tcp.setReconnectOnInitialFailure(false);
    _tcp.setDnsPolicy(FLPROG_TCP_DNS_POLICY_PRIVATE);
    _tcp.setTimeout(_connectTimeoutMs);
    _state = FLPROG_HTTP_STATE_IDLE;
    _error = FLPROG_HTTP_ERROR_NONE;
    return true;
}

void FLProgNetworkHttpClient::end()
{
    if (_tcpAttached)
    {
        _tcp.end();
        _tcpAttached = false;
    }
    detach();
    _state = FLPROG_HTTP_STATE_IDLE;
    _error = FLPROG_HTTP_ERROR_NONE;
    _cancelRequested = false;
    _connectIssued = false;
}

void FLProgNetworkHttpClient::setRoute(uint8_t value)
{
    _route = value;
}

void FLProgNetworkHttpClient::setHost(const char *value)
{
    if (value == nullptr || value[0] == 0)
    {
        clearHost();
        return;
    }
    IPAddress parsed;
    if (flprogHttpParseIpv4Literal(value, parsed))
    {
        setHost(parsed);
        return;
    }
    const size_t length = strlen(value);
    if (length > FLPROG_TCP_HOSTNAME_MAX_LENGTH)
    {
        clearHost();
        return;
    }
    memcpy(_hostName, value, length + 1);
    _hostMode = FLPROG_HTTP_HOST_NAME;
}

void FLProgNetworkHttpClient::setHost(IPAddress value)
{
    if (ipIsZero(value))
    {
        clearHost();
        return;
    }
    _hostIp = value;
    _hostName[0] = 0;
    _hostMode = FLPROG_HTTP_HOST_IP;
}

void FLProgNetworkHttpClient::clearHost()
{
    _hostMode = FLPROG_HTTP_HOST_NONE;
    _hostName[0] = 0;
    _hostIp = IPAddress(0, 0, 0, 0);
}

void FLProgNetworkHttpClient::setDnsCacheStorageTime(uint32_t value)
{
    _dnsCacheStorageMs = value;
}

void FLProgNetworkHttpClient::setUserAgent(const char *value)
{
    if (value == nullptr) value = "";
    size_t length = strlen(value);
    if (length >= sizeof(_userAgent)) length = sizeof(_userAgent) - 1;
    memcpy(_userAgent, value, length);
    _userAgent[length] = 0;
}

bool FLProgNetworkHttpClient::buildStructuredRequest(
    String &out,
    const char *method,
    const char *path,
    const char *body,
    const char *contentTypeValue)
{
    if (method == nullptr || method[0] == 0) return false;
    if (path == nullptr || path[0] == 0) path = "/";

    out = "";
    out.reserve(256);
    out += method;
    out += " ";
    if (path[0] != '/') out += "/";
    out += path;
    out += " HTTP/1.1\r\nHost: ";

    if (_hostMode == FLPROG_HTTP_HOST_NAME)
    {
        out += _hostName;
    }
    else if (_hostMode == FLPROG_HTTP_HOST_IP)
    {
        out += _hostIp.toString();
    }
    else
    {
        return false;
    }

    if (_port != 80)
    {
        out += ":";
        out += String((unsigned int)_port);
    }
    out += "\r\n";

    if (_userAgent[0] != 0)
    {
        out += "User-Agent: ";
        out += _userAgent;
        out += "\r\n";
    }
    out += "Connection: close\r\n";

    const size_t bodyLength = body == nullptr ? 0 : strlen(body);
    if (body != nullptr)
    {
        if (contentTypeValue != nullptr && contentTypeValue[0] != 0)
        {
            out += "Content-Type: ";
            out += contentTypeValue;
            out += "\r\n";
        }
        out += "Content-Length: ";
        out += String((unsigned long)bodyLength);
        out += "\r\n";
    }
    out += "\r\n";
    if (body != nullptr && bodyLength != 0) out += body;

    return out.length() <= FLPROG_HTTP_MAX_REQUEST_SIZE;
}

bool FLProgNetworkHttpClient::request(
    const char *method,
    const char *path,
    const char *body,
    const char *contentTypeValue)
{
    String built;
    if (!buildStructuredRequest(built, method, path, body, contentTypeValue))
    {
        _error = _hostMode == FLPROG_HTTP_HOST_NONE
            ? FLPROG_HTTP_ERROR_NO_HOST
            : FLPROG_HTTP_ERROR_REQUEST_TOO_LARGE;
        return false;
    }
    return queueRequest(built, false, method);
}

bool FLProgNetworkHttpClient::sendRawRequest(const String &value)
{
    return queueRequest(value, true, "RAW");
}

bool FLProgNetworkHttpClient::queueRequest(
    const String &value,
    bool rawCompatibility,
    const char *method)
{
    if (!_tcpAttached || !isAttached())
    {
        _error = FLPROG_HTTP_ERROR_SERVICE_LIMIT;
        return false;
    }
    if (busy())
    {
        _error = FLPROG_HTTP_ERROR_BUSY;
        return false;
    }
    if (_hostMode == FLPROG_HTTP_HOST_NONE)
    {
        _error = FLPROG_HTTP_ERROR_NO_HOST;
        return false;
    }
    if (_port == 0)
    {
        _error = FLPROG_HTTP_ERROR_NO_PORT;
        return false;
    }
    if (value.length() == 0 || value.length() > FLPROG_HTTP_MAX_REQUEST_SIZE)
    {
        _error = FLPROG_HTTP_ERROR_REQUEST_TOO_LARGE;
        return false;
    }

    _request = value;
    _requestOffset = 0;
    _rawCompatibility = rawCompatibility;
    _rawReceivedAny = false;
    _connectIssued = false;
    _cancelRequested = false;

    size_t methodLength = method == nullptr ? 0 : strlen(method);
    if (methodLength >= sizeof(_requestMethod)) methodLength = sizeof(_requestMethod) - 1;
    if (methodLength != 0) memcpy(_requestMethod, method, methodLength);
    _requestMethod[methodLength] = 0;

    _requestHostMode = _hostMode;
    _requestHostIp = _hostIp;
    _requestPort = _port;
    _requestRoute = _route;
    if (_hostMode == FLPROG_HTTP_HOST_NAME)
    {
        const size_t hostLength = strlen(_hostName);
        memcpy(_requestHostName, _hostName, hostLength + 1);
    }
    else
    {
        _requestHostName[0] = 0;
    }

    resetResponseState();
    _state = FLPROG_HTTP_STATE_CONNECTING;
    _error = FLPROG_HTTP_ERROR_NONE;
    _transactionStartedAt = millis();
    _lastProgressAt = _transactionStartedAt;
    _requestCount++;
    return true;
}

void FLProgNetworkHttpClient::resetResponseState()
{
    _answerString = "";
    if (_responseBufferLimit != 0) _answerString.reserve(_responseBufferLimit);
    _hasAnswer = false;
    _answerTruncated = false;
    _droppedResponseBytes = 0;
    _rawBytesReceived = 0;

    _lineLength = 0;
    _line[0] = 0;
    _headerBytes = 0;
    _headersComplete = false;
    _statusCode = 0;
    _httpMajor = 0;
    _httpMinor = 0;
    _chunked = false;
    _connectionClose = false;
    _contentLengthKnown = false;
    _contentLength = 0;
    _bodyMode = FLPROG_HTTP_BODY_NONE;
    _bodyBytesReceived = 0;
    _contentType[0] = 0;
    _location[0] = 0;
    _chunkState = FLPROG_HTTP_CHUNK_SIZE_LINE;
    _chunkRemaining = 0;
    _trailerBytes = 0;
    _bodyCallbackLength = 0;
    _events = FLPROG_HTTP_EVENT_NONE;
}

void FLProgNetworkHttpClient::applyDnsCachePolicy()
{
    if (_requestHostMode != FLPROG_HTTP_HOST_NAME) return;
    FLProgNetworkDns *resolver = _tcp.activeDnsResolver();
    if (resolver == nullptr) return;
    uint32_t seconds = 0;
    if (_dnsCacheStorageMs != 0)
    {
        seconds = (_dnsCacheStorageMs + 999UL) / 1000UL;
        if (seconds == 0) seconds = 1;
    }
    resolver->setMaxCacheTtl(seconds);
}

void FLProgNetworkHttpClient::startConnect()
{
    _tcp.setRoute(_requestRoute);
    _tcp.setTimeout(_connectTimeoutMs);
    _tcp.clearClientError();
    _tcp.setAutoReconnect(false);
    _tcp.setReconnectOnInitialFailure(false);
    _tcp.setDnsPolicy(FLPROG_TCP_DNS_POLICY_PRIVATE);
    applyDnsCachePolicy();

    uint8_t result = FLPROG_SOCKET_RESULT_ERROR;
    if (_requestHostMode == FLPROG_HTTP_HOST_NAME)
    {
        result = _tcp.connect(_requestHostName, _requestPort);
    }
    else if (_requestHostMode == FLPROG_HTTP_HOST_IP)
    {
        result = _tcp.connect(_requestHostIp, _requestPort);
    }

    _connectIssued = true;
    _lastProgressAt = millis();
    if (result == FLPROG_SOCKET_RESULT_ERROR)
    {
        finishError(_tcp.clientError() == FLPROG_TCP_CLIENT_ERROR_DNS
            ? FLPROG_HTTP_ERROR_DNS
            : FLPROG_HTTP_ERROR_CONNECT);
    }
}

void FLProgNetworkHttpClient::connectingStep()
{
    const uint32_t now = millis();
    if (!_connectIssued)
    {
        startConnect();
        return;
    }
    if (_state != FLPROG_HTTP_STATE_CONNECTING) return;

    if (_tcp.connected())
    {
        _state = FLPROG_HTTP_STATE_SENDING;
        _lastProgressAt = now;
        return;
    }

    if (_tcp.clientError() == FLPROG_TCP_CLIENT_ERROR_DNS)
    {
        finishError(FLPROG_HTTP_ERROR_DNS);
        return;
    }
    if (_tcp.clientError() != FLPROG_TCP_CLIENT_ERROR_NONE ||
        _tcp.state() == FLPROG_SOCKET_STATE_ERROR)
    {
        const uint8_t value = _tcp.error() == FLPROG_SOCKET_ERROR_TIMEOUT
            ? FLPROG_HTTP_ERROR_CONNECT_TIMEOUT
            : FLPROG_HTTP_ERROR_CONNECT;
        finishError(value);
        return;
    }
    if (_tcp.state() == FLPROG_SOCKET_STATE_UNSUPPORTED)
    {
        finishError(FLPROG_HTTP_ERROR_UNSUPPORTED);
        return;
    }
    if (timeExpired(now, _transactionStartedAt, _connectTimeoutMs))
    {
        finishError(FLPROG_HTTP_ERROR_CONNECT_TIMEOUT);
    }
}

void FLProgNetworkHttpClient::sendingStep()
{
    const uint32_t now = millis();
    if (!_tcp.connected())
    {
        finishError(FLPROG_HTTP_ERROR_SEND);
        return;
    }
    if (timeExpired(now, _lastProgressAt, _requestTimeoutMs))
    {
        finishError(FLPROG_HTTP_ERROR_TIMEOUT);
        return;
    }

    const size_t total = _request.length();
    if (_requestOffset >= total)
    {
        _state = _rawCompatibility
            ? FLPROG_HTTP_STATE_RAW_RESPONSE
            : FLPROG_HTTP_STATE_STATUS_LINE;
        _lastProgressAt = now;
        return;
    }

    size_t remaining = total - _requestOffset;
    if (remaining > FLPROG_HTTP_WRITE_BUDGET) remaining = FLPROG_HTTP_WRITE_BUDGET;
    const uint8_t *data = reinterpret_cast<const uint8_t *>(_request.c_str() + _requestOffset);
    const size_t written = _tcp.write(data, remaining);
    if (written != 0)
    {
        _requestOffset += written;
        _lastProgressAt = now;
    }
}

void FLProgNetworkHttpClient::consumeRawByte(uint8_t value)
{
    _rawBytesReceived++;
    if (_byteCallback != nullptr) _byteCallback(value);

    if (_answerString.length() < _responseBufferLimit)
    {
        _answerString += (char)value;
    }
    else
    {
        _droppedResponseBytes++;
        if (!_answerTruncated)
        {
            _answerTruncated = true;
            _events |= FLPROG_HTTP_EVENT_TRUNCATED;
            _truncatedResponseCount++;
        }
    }
}

void FLProgNetworkHttpClient::rawResponseStep()
{
    const uint32_t now = millis();
    size_t budget = FLPROG_HTTP_READ_BUDGET;
    int available = _tcp.available();

    while (available > 0 && budget > 0)
    {
        int value = _tcp.read();
        if (value < 0) break;
        consumeRawByte((uint8_t)value);
        _rawReceivedAny = true;
        _lastProgressAt = now;
        --budget;
        available = _tcp.available();
    }

    if (_rawReceivedAny && _tcp.available() == 0)
    {
        finishSuccess();
        return;
    }
    if (!_tcp.connected() && _tcp.available() == 0)
    {
        if (_rawReceivedAny) finishSuccess();
        else finishError(FLPROG_HTTP_ERROR_UNEXPECTED_EOF);
        return;
    }
    if (timeExpired(now, _lastProgressAt, _requestTimeoutMs))
    {
        finishError(FLPROG_HTTP_ERROR_TIMEOUT);
    }
}

void FLProgNetworkHttpClient::processLineByte(uint8_t value, bool statusLine)
{
    _headerBytes++;
    if (_headerBytes > FLPROG_HTTP_MAX_HEADER_BYTES)
    {
        finishError(FLPROG_HTTP_ERROR_HEADER_TOO_LARGE);
        return;
    }

    if (value == '\n')
    {
        if (_lineLength > 0 && _line[_lineLength - 1] == '\r') --_lineLength;
        _line[_lineLength] = 0;
        bool ok = true;
        if (statusLine)
        {
            ok = parseStatusLine();
            if (ok) _state = FLPROG_HTTP_STATE_HEADERS;
        }
        else if (_lineLength == 0)
        {
            finishHeaders();
        }
        else
        {
            ok = parseHeaderLine();
        }
        _lineLength = 0;
        _line[0] = 0;
        if (!ok && _state != FLPROG_HTTP_STATE_ERROR)
        {
            finishError(statusLine ? FLPROG_HTTP_ERROR_BAD_STATUS : FLPROG_HTTP_ERROR_PROTOCOL);
        }
        return;
    }

    if (_lineLength >= FLPROG_HTTP_MAX_LINE_LENGTH)
    {
        finishError(FLPROG_HTTP_ERROR_LINE_TOO_LONG);
        return;
    }
    _line[_lineLength++] = (char)value;
}

bool FLProgNetworkHttpClient::parseStatusLine()
{
    if (!startsWithIgnoreCase(_line, "HTTP/")) return false;
    const char *p = _line + 5;
    if (p[0] < '0' || p[0] > '9' || p[1] != '.' || p[2] < '0' || p[2] > '9') return false;
    _httpMajor = (uint8_t)(p[0] - '0');
    _httpMinor = (uint8_t)(p[2] - '0');
    p += 3;
    while (*p == ' ') ++p;
    if (p[0] < '0' || p[0] > '9' || p[1] < '0' || p[1] > '9' || p[2] < '0' || p[2] > '9') return false;
    _statusCode = (p[0] - '0') * 100 + (p[1] - '0') * 10 + (p[2] - '0');
    return _statusCode >= 100 && _statusCode <= 999;
}

bool FLProgNetworkHttpClient::parseHeaderLine()
{
    char *colon = strchr(_line, ':');
    if (colon == nullptr) return false;
    *colon = 0;
    const char *value = skipSpaces(colon + 1);

    if (startsWithIgnoreCase(_line, "Content-Length") && strlen(_line) == strlen("Content-Length"))
    {
        uint32_t parsed = 0;
        if (!parseUnsigned(value, parsed)) return false;
        if (_contentLengthKnown && _contentLength != parsed)
        {
            finishError(FLPROG_HTTP_ERROR_BAD_LENGTH);
            return false;
        }
        _contentLengthKnown = true;
        _contentLength = parsed;
        return true;
    }
    if (startsWithIgnoreCase(_line, "Transfer-Encoding") && strlen(_line) == strlen("Transfer-Encoding"))
    {
        if (containsTokenIgnoreCase(value, "chunked")) _chunked = true;
        return true;
    }
    if (startsWithIgnoreCase(_line, "Connection") && strlen(_line) == strlen("Connection"))
    {
        if (containsTokenIgnoreCase(value, "close")) _connectionClose = true;
        return true;
    }
    if (startsWithIgnoreCase(_line, "Content-Type") && strlen(_line) == strlen("Content-Type"))
    {
        size_t length = strlen(value);
        if (length > FLPROG_HTTP_CONTENT_TYPE_MAX_LENGTH) length = FLPROG_HTTP_CONTENT_TYPE_MAX_LENGTH;
        memcpy(_contentType, value, length);
        _contentType[length] = 0;
        return true;
    }
    if (startsWithIgnoreCase(_line, "Location") && strlen(_line) == strlen("Location"))
    {
        size_t length = strlen(value);
        if (length > FLPROG_HTTP_LOCATION_MAX_LENGTH) length = FLPROG_HTTP_LOCATION_MAX_LENGTH;
        memcpy(_location, value, length);
        _location[length] = 0;
        return true;
    }
    return true;
}

void FLProgNetworkHttpClient::finishHeaders()
{
    if (_statusCode >= 100 && _statusCode < 200 && _statusCode != 101)
    {
        // Interim response. Parse the next status line as the final response.
        _statusCode = 0;
        _httpMajor = 0;
        _httpMinor = 0;
        _chunked = false;
        _connectionClose = false;
        _contentLengthKnown = false;
        _contentLength = 0;
        _contentType[0] = 0;
        _location[0] = 0;
        _state = FLPROG_HTTP_STATE_STATUS_LINE;
        return;
    }
    if (_statusCode == 101)
    {
        finishError(FLPROG_HTTP_ERROR_UNSUPPORTED);
        return;
    }

    _headersComplete = true;
    _events |= FLPROG_HTTP_EVENT_HEADERS;
    if (isRedirect())
    {
        _events |= FLPROG_HTTP_EVENT_REDIRECT;
        _redirectCount++;
    }

    if (methodHasNoResponseBody(_requestMethod) || responseStatusHasNoBody(_statusCode))
    {
        _bodyMode = FLPROG_HTTP_BODY_NONE;
        finishSuccess();
        return;
    }
    if (_chunked)
    {
        _bodyMode = FLPROG_HTTP_BODY_CHUNKED;
        _chunkState = FLPROG_HTTP_CHUNK_SIZE_LINE;
        _chunkRemaining = 0;
        _chunkedResponseCount++;
        _state = FLPROG_HTTP_STATE_BODY;
        return;
    }
    if (_contentLengthKnown)
    {
        _bodyMode = FLPROG_HTTP_BODY_FIXED;
        if (_contentLength == 0)
        {
            finishSuccess();
            return;
        }
        _state = FLPROG_HTTP_STATE_BODY;
        return;
    }

    _bodyMode = FLPROG_HTTP_BODY_UNTIL_CLOSE;
    _state = FLPROG_HTTP_STATE_BODY;
}

void FLProgNetworkHttpClient::deliverBodyByte(uint8_t value)
{
    _bodyBytesReceived++;
    if (_bodyCallback != nullptr)
    {
        _bodyCallbackBuffer[_bodyCallbackLength++] = value;
        if (_bodyCallbackLength >= FLPROG_HTTP_BODY_CALLBACK_CHUNK) flushBodyCallback();
    }
}

void FLProgNetworkHttpClient::flushBodyCallback()
{
    if (_bodyCallbackLength == 0) return;
    if (_bodyCallback != nullptr)
    {
        _bodyCallback(_bodyCallbackBuffer, _bodyCallbackLength);
        _events |= FLPROG_HTTP_EVENT_BODY_DATA;
    }
    _bodyCallbackLength = 0;
}

void FLProgNetworkHttpClient::processChunkedByte(uint8_t value)
{
    if (_chunkState == FLPROG_HTTP_CHUNK_DATA)
    {
        deliverBodyByte(value);
        if (_chunkRemaining == 0)
        {
            finishError(FLPROG_HTTP_ERROR_BAD_CHUNK);
            return;
        }
        _chunkRemaining--;
        if (_chunkRemaining == 0) _chunkState = FLPROG_HTTP_CHUNK_DATA_CR;
        return;
    }
    if (_chunkState == FLPROG_HTTP_CHUNK_DATA_CR)
    {
        if (value != '\r')
        {
            finishError(FLPROG_HTTP_ERROR_BAD_CHUNK);
            return;
        }
        _chunkState = FLPROG_HTTP_CHUNK_DATA_LF;
        return;
    }
    if (_chunkState == FLPROG_HTTP_CHUNK_DATA_LF)
    {
        if (value != '\n')
        {
            finishError(FLPROG_HTTP_ERROR_BAD_CHUNK);
            return;
        }
        _chunkState = FLPROG_HTTP_CHUNK_SIZE_LINE;
        _lineLength = 0;
        return;
    }

    // Chunk-size line or trailer line.
    // Chunk-size lines are individually bounded by _lineLength below.
    // Only actual trailers count towards the cumulative header limit.
    if (_chunkState == FLPROG_HTTP_CHUNK_TRAILERS &&
        ++_trailerBytes > FLPROG_HTTP_MAX_HEADER_BYTES)
    {
        finishError(FLPROG_HTTP_ERROR_HEADER_TOO_LARGE);
        return;
    }
    if (value == '\n')
    {
        if (_lineLength > 0 && _line[_lineLength - 1] == '\r') --_lineLength;
        _line[_lineLength] = 0;
        if (_chunkState == FLPROG_HTTP_CHUNK_SIZE_LINE)
        {
            uint32_t parsed = 0;
            if (!parseHexUnsigned(_line, parsed))
            {
                finishError(FLPROG_HTTP_ERROR_BAD_CHUNK);
                return;
            }
            _chunkRemaining = parsed;
            if (parsed == 0) _chunkState = FLPROG_HTTP_CHUNK_TRAILERS;
            else _chunkState = FLPROG_HTTP_CHUNK_DATA;
        }
        else if (_chunkState == FLPROG_HTTP_CHUNK_TRAILERS)
        {
            if (_lineLength == 0)
            {
                finishSuccess();
                return;
            }
            // Trailer syntax is bounded but otherwise ignored in dev.2.
            if (strchr(_line, ':') == nullptr)
            {
                finishError(FLPROG_HTTP_ERROR_BAD_CHUNK);
                return;
            }
        }
        _lineLength = 0;
        _line[0] = 0;
        return;
    }
    if (_lineLength >= FLPROG_HTTP_MAX_LINE_LENGTH)
    {
        finishError(FLPROG_HTTP_ERROR_LINE_TOO_LONG);
        return;
    }
    _line[_lineLength++] = (char)value;
}

void FLProgNetworkHttpClient::processBodyByte(uint8_t value)
{
    if (_bodyMode == FLPROG_HTTP_BODY_CHUNKED)
    {
        processChunkedByte(value);
        return;
    }

    deliverBodyByte(value);
    if (_bodyMode == FLPROG_HTTP_BODY_FIXED)
    {
        if (_bodyBytesReceived > _contentLength)
        {
            finishError(FLPROG_HTTP_ERROR_PROTOCOL);
            return;
        }
        if (_bodyBytesReceived == _contentLength) finishSuccess();
    }
}

void FLProgNetworkHttpClient::processHttpByte(uint8_t value)
{
    consumeRawByte(value);
    if (_state == FLPROG_HTTP_STATE_STATUS_LINE)
    {
        processLineByte(value, true);
        return;
    }
    if (_state == FLPROG_HTTP_STATE_HEADERS)
    {
        processLineByte(value, false);
        return;
    }
    if (_state == FLPROG_HTTP_STATE_BODY)
    {
        processBodyByte(value);
    }
}

void FLProgNetworkHttpClient::httpResponseStep()
{
    const uint32_t now = millis();
    size_t budget = FLPROG_HTTP_READ_BUDGET;
    int available = _tcp.available();

    while (available > 0 && budget > 0 && busy())
    {
        int value = _tcp.read();
        if (value < 0) break;
        processHttpByte((uint8_t)value);
        _lastProgressAt = now;
        --budget;
        available = _tcp.available();
    }
    flushBodyCallback();
    if (!busy()) return;

    if (!_tcp.connected() && _tcp.available() == 0)
    {
        if (_state == FLPROG_HTTP_STATE_BODY && _bodyMode == FLPROG_HTTP_BODY_UNTIL_CLOSE)
        {
            finishSuccess();
        }
        else
        {
            finishError(FLPROG_HTTP_ERROR_UNEXPECTED_EOF);
        }
        return;
    }

    if (timeExpired(now, _lastProgressAt, _requestTimeoutMs))
    {
        finishError(FLPROG_HTTP_ERROR_TIMEOUT);
    }
}

void FLProgNetworkHttpClient::stopTcp()
{
    if (_tcpAttached) _tcp.stop();
}

void FLProgNetworkHttpClient::finishSuccess()
{
    if (_state == FLPROG_HTTP_STATE_DONE) return;
    flushBodyCallback();
    _hasAnswer = _rawBytesReceived != 0;
    _state = FLPROG_HTTP_STATE_DONE;
    _error = FLPROG_HTTP_ERROR_NONE;
    _successCount++;
    _events |= FLPROG_HTTP_EVENT_COMPLETED;
    _lastDurationMs = millis() - _transactionStartedAt;
    stopTcp();
    if (_callback != nullptr) _callback();
}

void FLProgNetworkHttpClient::finishError(uint8_t value)
{
    if (_state == FLPROG_HTTP_STATE_ERROR) return;
    flushBodyCallback();
    _state = FLPROG_HTTP_STATE_ERROR;
    _error = value;
    _failureCount++;
    _events |= FLPROG_HTTP_EVENT_FAILED;
    if (value == FLPROG_HTTP_ERROR_TIMEOUT || value == FLPROG_HTTP_ERROR_CONNECT_TIMEOUT)
    {
        _timeoutCount++;
        _events |= FLPROG_HTTP_EVENT_TIMEOUT;
    }
    _lastDurationMs = millis() - _transactionStartedAt;
    stopTcp();
}

void FLProgNetworkHttpClient::cancel()
{
    if (busy()) _cancelRequested = true;
}

void FLProgNetworkHttpClient::processCancel()
{
    if (!_cancelRequested) return;
    _cancelRequested = false;
    if (!busy()) return;
    flushBodyCallback();
    stopTcp();
    _state = FLPROG_HTTP_STATE_IDLE;
    _error = FLPROG_HTTP_ERROR_NONE;
    _cancelCount++;
    _events |= FLPROG_HTTP_EVENT_CANCELED;
    _lastDurationMs = millis() - _transactionStartedAt;
}

void FLProgNetworkHttpClient::pool()
{
    if (!_tcpAttached || !isAttached()) return;
    processCancel();
    if (_state == FLPROG_HTTP_STATE_IDLE ||
        _state == FLPROG_HTTP_STATE_DONE ||
        _state == FLPROG_HTTP_STATE_ERROR)
    {
        return;
    }

    switch (_state)
    {
    case FLPROG_HTTP_STATE_CONNECTING:
        connectingStep();
        break;
    case FLPROG_HTTP_STATE_SENDING:
        sendingStep();
        break;
    case FLPROG_HTTP_STATE_RAW_RESPONSE:
        rawResponseStep();
        break;
    case FLPROG_HTTP_STATE_STATUS_LINE:
    case FLPROG_HTTP_STATE_HEADERS:
    case FLPROG_HTTP_STATE_BODY:
        httpResponseStep();
        break;
    default:
        break;
    }
}
