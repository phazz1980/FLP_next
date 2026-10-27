/*
 * FLProg_Network — flprogNetworkHttpClient.h
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
#pragma once

#include <Arduino.h>
#include <IPAddress.h>
#include <stdint.h>
#include "flprogNetworkService.h"
#include "../protocol/flprogNetworkTcpClient.h"

class FLProgNetworkInterface;

#ifndef FLPROG_HTTP_DEFAULT_TIMEOUT_MS
#define FLPROG_HTTP_DEFAULT_TIMEOUT_MS 5000UL
#endif
#ifndef FLPROG_HTTP_DEFAULT_CONNECT_TIMEOUT_MS
#define FLPROG_HTTP_DEFAULT_CONNECT_TIMEOUT_MS 5000UL
#endif
#ifndef FLPROG_HTTP_MAX_REQUEST_SIZE
#define FLPROG_HTTP_MAX_REQUEST_SIZE 2048U
#endif
#ifndef FLPROG_HTTP_MAX_RESPONSE_BUFFER
#define FLPROG_HTTP_MAX_RESPONSE_BUFFER 2048U
#endif
#ifndef FLPROG_HTTP_MAX_HEADER_BYTES
#define FLPROG_HTTP_MAX_HEADER_BYTES 1536U
#endif
#ifndef FLPROG_HTTP_MAX_LINE_LENGTH
#define FLPROG_HTTP_MAX_LINE_LENGTH 255U
#endif
#ifndef FLPROG_HTTP_READ_BUDGET
#define FLPROG_HTTP_READ_BUDGET 256U
#endif
#ifndef FLPROG_HTTP_WRITE_BUDGET
#define FLPROG_HTTP_WRITE_BUDGET 256U
#endif
#ifndef FLPROG_HTTP_BODY_CALLBACK_CHUNK
#define FLPROG_HTTP_BODY_CALLBACK_CHUNK 64U
#endif
#ifndef FLPROG_HTTP_CONTENT_TYPE_MAX_LENGTH
#define FLPROG_HTTP_CONTENT_TYPE_MAX_LENGTH 95U
#endif
#ifndef FLPROG_HTTP_LOCATION_MAX_LENGTH
#define FLPROG_HTTP_LOCATION_MAX_LENGTH 191U
#endif

#define FLPROG_HTTP_HOST_NONE 0
#define FLPROG_HTTP_HOST_NAME 1
#define FLPROG_HTTP_HOST_IP   2

#define FLPROG_HTTP_STATE_IDLE          0
#define FLPROG_HTTP_STATE_CONNECTING    1
#define FLPROG_HTTP_STATE_SENDING       2
#define FLPROG_HTTP_STATE_RAW_RESPONSE  3
#define FLPROG_HTTP_STATE_STATUS_LINE   4
#define FLPROG_HTTP_STATE_HEADERS       5
#define FLPROG_HTTP_STATE_BODY          6
#define FLPROG_HTTP_STATE_DONE          7
#define FLPROG_HTTP_STATE_ERROR         8

#define FLPROG_HTTP_ERROR_NONE              0
#define FLPROG_HTTP_ERROR_BUSY              1
#define FLPROG_HTTP_ERROR_NO_HOST           2
#define FLPROG_HTTP_ERROR_NO_PORT           3
#define FLPROG_HTTP_ERROR_REQUEST_TOO_LARGE 4
#define FLPROG_HTTP_ERROR_SERVICE_LIMIT     5
#define FLPROG_HTTP_ERROR_SOCKET_REGISTRY   6
#define FLPROG_HTTP_ERROR_CONNECT           7
#define FLPROG_HTTP_ERROR_DNS               8
#define FLPROG_HTTP_ERROR_CONNECT_TIMEOUT   9
#define FLPROG_HTTP_ERROR_SEND             10
#define FLPROG_HTTP_ERROR_TIMEOUT          11
#define FLPROG_HTTP_ERROR_BAD_STATUS       12
#define FLPROG_HTTP_ERROR_HEADER_TOO_LARGE 13
#define FLPROG_HTTP_ERROR_LINE_TOO_LONG    14
#define FLPROG_HTTP_ERROR_BAD_LENGTH       15
#define FLPROG_HTTP_ERROR_BAD_CHUNK        16
#define FLPROG_HTTP_ERROR_UNEXPECTED_EOF   17
#define FLPROG_HTTP_ERROR_UNSUPPORTED      18
#define FLPROG_HTTP_ERROR_PROTOCOL         19

#define FLPROG_HTTP_EVENT_NONE       0x00U
#define FLPROG_HTTP_EVENT_COMPLETED  0x01U
#define FLPROG_HTTP_EVENT_FAILED     0x02U
#define FLPROG_HTTP_EVENT_TIMEOUT    0x04U
#define FLPROG_HTTP_EVENT_CANCELED   0x08U
#define FLPROG_HTTP_EVENT_HEADERS    0x10U
#define FLPROG_HTTP_EVENT_BODY_DATA  0x20U
#define FLPROG_HTTP_EVENT_TRUNCATED  0x40U
#define FLPROG_HTTP_EVENT_REDIRECT   0x80U

#define FLPROG_HTTP_BODY_NONE        0
#define FLPROG_HTTP_BODY_FIXED       1
#define FLPROG_HTTP_BODY_UNTIL_CLOSE 2
#define FLPROG_HTTP_BODY_CHUNKED     3

#define FLPROG_HTTP_CHUNK_SIZE_LINE  0
#define FLPROG_HTTP_CHUNK_DATA       1
#define FLPROG_HTTP_CHUNK_DATA_CR    2
#define FLPROG_HTTP_CHUNK_DATA_LF    3
#define FLPROG_HTTP_CHUNK_TRAILERS   4

typedef void (*FLProgNetworkHttpCallback)();
typedef void (*FLProgNetworkHttpByteCallback)(uint8_t);
typedef void (*FLProgNetworkHttpBodyCallback)(const uint8_t *, size_t);

// Route-aware cooperative HTTP/raw-TCP client service. Socket progress and
// response parsing are advanced only from FLProgNetworkInterface::pool().
class FLProgNetworkHttpClient : public FLProgNetworkService
{
public:
    FLProgNetworkHttpClient() {}
    ~FLProgNetworkHttpClient() override;

    bool begin(FLProgNetworkInterface &network, uint8_t route = 0);
    void end();
    void pool() override;

    void setRoute(uint8_t value);
    uint8_t route() const { return _route; }

    void setHost(const char *value);
    void setHost(const String &value) { setHost(value.c_str()); }
    void setHost(IPAddress value);
    void clearHost();
    uint8_t hostMode() const { return _hostMode; }
    const char *hostName() const { return _hostName; }
    IPAddress hostIP() const { return _hostIp; }

    void setPort(uint16_t value) { _port = value; }
    uint16_t port() const { return _port; }

    void setRequestTimeout(uint32_t value) { _requestTimeoutMs = value; }
    uint32_t requestTimeout() const { return _requestTimeoutMs; }
    // LEGACY API misspelled wrapper; new code uses setRequestTimeout().
    void setReqestTimeout(uint32_t value) { setRequestTimeout(value); }

    void setConnectTimeout(uint32_t value)
    {
        _connectTimeoutMs = value;
        _tcp.setTimeout(value);
    }
    uint32_t connectTimeout() const { return _connectTimeoutMs; }

    // Legacy value is milliseconds. New DNS uses server TTL with this value as
    // an upper bound; zero disables caching for this client's private resolver.
    void setDnsCacheStorageTime(uint32_t value);
    uint32_t dnsCacheStorageTime() const { return _dnsCacheStorageMs; }

    void setResponseBufferLimit(size_t value)
    {
        _responseBufferLimit = value > FLPROG_HTTP_MAX_RESPONSE_BUFFER
            ? FLPROG_HTTP_MAX_RESPONSE_BUFFER
            : value;
    }
    size_t responseBufferLimit() const { return _responseBufferLimit; }

    void setUserAgent(const char *value);
    const char *userAgent() const { return _userAgent; }

    // Primary structured API. Returns false only when the request cannot be
    // queued (busy/invalid/too large). Completion remains asynchronous.
    bool request(
        const char *method,
        const char *path,
        const char *body = nullptr,
        const char *contentType = nullptr);
    bool get(const char *path) { return request("GET", path, nullptr, nullptr); }
    bool head(const char *path) { return request("HEAD", path, nullptr, nullptr); }
    bool post(
        const char *path,
        const char *body,
        const char *contentType = "text/plain")
    {
        return request("POST", path, body, contentType);
    }

    // Raw compatibility path. sendReqest() is LEGACY API; new code should
    // call sendRawRequest() explicitly. It intentionally does not require an HTTP
    // response and completes after the first cooperatively drained RX burst,
    // matching the useful legacy FLProgWebClient behavior without a blocking
    // while(available()) loop.
    bool sendRawRequest(const String &value);
    void sendReqest(String value) { (void)sendRawRequest(value); }

    void cancel();
    bool busy() const
    {
        return _state == FLPROG_HTTP_STATE_CONNECTING ||
               _state == FLPROG_HTTP_STATE_SENDING ||
               _state == FLPROG_HTTP_STATE_RAW_RESPONSE ||
               _state == FLPROG_HTTP_STATE_STATUS_LINE ||
               _state == FLPROG_HTTP_STATE_HEADERS ||
               _state == FLPROG_HTTP_STATE_BODY;
    }
    bool done() const { return _state == FLPROG_HTTP_STATE_DONE; }
    bool failed() const { return _state == FLPROG_HTTP_STATE_ERROR; }

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t getStatus() const { return _state; }
    uint8_t getError() const { return _error; }
    uint8_t tcpClientError() const { return _tcp.clientError(); }
    uint8_t dnsError() const { return _tcp.dnsError(); }

    uint8_t events() const { return _events; }
    uint8_t takeEvents()
    {
        const uint8_t value = _events;
        _events = FLPROG_HTTP_EVENT_NONE;
        return value;
    }

    int statusCode() const { return _statusCode; }
    uint8_t httpMajor() const { return _httpMajor; }
    uint8_t httpMinor() const { return _httpMinor; }
    bool headersComplete() const { return _headersComplete; }
    bool chunked() const { return _chunked; }
    bool connectionClose() const { return _connectionClose; }
    bool contentLengthKnown() const { return _contentLengthKnown; }
    uint32_t contentLength() const { return _contentLength; }
    uint32_t bodyBytesReceived() const { return _bodyBytesReceived; }
    uint32_t rawBytesReceived() const { return _rawBytesReceived; }
    const char *contentType() const { return _contentType; }
    const char *location() const { return _location; }
    bool isRedirect() const { return _statusCode >= 300 && _statusCode < 400; }

    String getAnswerString() const { return _answerString; }
    bool hasAnswer() const { return _hasAnswer; }
    bool responseTruncated() const { return _answerTruncated; }
    uint32_t droppedResponseBytes() const { return _droppedResponseBytes; }

    void setCallback(FLProgNetworkHttpCallback value) { _callback = value; }
    void setCallBack(FLProgNetworkHttpCallback value) { setCallback(value); }
    void setBodyCallback(FLProgNetworkHttpBodyCallback value) { _bodyCallback = value; }
    void setByteCallback(FLProgNetworkHttpByteCallback value) { _byteCallback = value; }
    void setCallBackForEachByte(FLProgNetworkHttpByteCallback value)
    {
        setByteCallback(value);
    }

    uint32_t requestCount() const { return _requestCount; }
    uint32_t successCount() const { return _successCount; }
    uint32_t failureCount() const { return _failureCount; }
    uint32_t timeoutCount() const { return _timeoutCount; }
    uint32_t redirectCount() const { return _redirectCount; }
    uint32_t chunkedResponseCount() const { return _chunkedResponseCount; }
    uint32_t truncatedResponseCount() const { return _truncatedResponseCount; }
    uint32_t cancelCount() const { return _cancelCount; }
    uint32_t lastDurationMs() const { return _lastDurationMs; }

    void resetStatistics()
    {
        _requestCount = 0;
        _successCount = 0;
        _failureCount = 0;
        _timeoutCount = 0;
        _redirectCount = 0;
        _chunkedResponseCount = 0;
        _truncatedResponseCount = 0;
        _cancelCount = 0;
    }

    FLProgNetworkTcpClient *tcp() { return &_tcp; }

private:
    static bool ipIsZero(IPAddress value);
    static char asciiLower(char value);
    static bool startsWithIgnoreCase(const char *value, const char *prefix);
    static bool containsTokenIgnoreCase(const char *value, const char *token);
    static const char *skipSpaces(const char *value);
    static bool parseUnsigned(const char *value, uint32_t &result);
    static bool parseHexUnsigned(const char *value, uint32_t &result);
    static bool methodHasNoResponseBody(const char *method);
    static bool responseStatusHasNoBody(int statusCode);
    static bool timeExpired(uint32_t now, uint32_t started, uint32_t timeout)
    {
        return (uint32_t)(now - started) >= timeout;
    }

    bool queueRequest(const String &request, bool rawCompatibility, const char *method);
    bool buildStructuredRequest(
        String &out,
        const char *method,
        const char *path,
        const char *body,
        const char *contentType);
    void resetResponseState();
    void startConnect();
    void connectingStep();
    void sendingStep();
    void rawResponseStep();
    void httpResponseStep();
    void processHttpByte(uint8_t value);
    void processLineByte(uint8_t value, bool statusLine);
    bool parseStatusLine();
    bool parseHeaderLine();
    void finishHeaders();
    void processBodyByte(uint8_t value);
    void processChunkedByte(uint8_t value);
    void deliverBodyByte(uint8_t value);
    void flushBodyCallback();
    void consumeRawByte(uint8_t value);
    void finishSuccess();
    void finishError(uint8_t value);
    void processCancel();
    void stopTcp();
    void applyDnsCachePolicy();

    FLProgNetworkTcpClient _tcp;
    bool _tcpAttached = false;

    uint8_t _route = 0;
    uint8_t _hostMode = FLPROG_HTTP_HOST_NONE;
    char _hostName[FLPROG_TCP_HOSTNAME_MAX_LENGTH + 1] = {0};
    IPAddress _hostIp = IPAddress(0, 0, 0, 0);
    uint16_t _port = 0;

    uint32_t _requestTimeoutMs = FLPROG_HTTP_DEFAULT_TIMEOUT_MS;
    uint32_t _connectTimeoutMs = FLPROG_HTTP_DEFAULT_CONNECT_TIMEOUT_MS;
    uint32_t _dnsCacheStorageMs = 60000UL;
    size_t _responseBufferLimit = FLPROG_HTTP_MAX_RESPONSE_BUFFER;
    char _userAgent[64] = "FLProg_Network/1.0";

    String _request;
    size_t _requestOffset = 0;
    bool _rawCompatibility = false;
    bool _rawReceivedAny = false;
    bool _connectIssued = false;
    bool _cancelRequested = false;
    char _requestMethod[12] = {0};
    uint8_t _requestHostMode = FLPROG_HTTP_HOST_NONE;
    char _requestHostName[FLPROG_TCP_HOSTNAME_MAX_LENGTH + 1] = {0};
    IPAddress _requestHostIp = IPAddress(0, 0, 0, 0);
    uint16_t _requestPort = 0;
    uint8_t _requestRoute = 0;

    uint8_t _state = FLPROG_HTTP_STATE_IDLE;
    uint8_t _error = FLPROG_HTTP_ERROR_NONE;
    uint8_t _events = FLPROG_HTTP_EVENT_NONE;
    uint32_t _transactionStartedAt = 0;
    uint32_t _lastProgressAt = 0;
    uint32_t _lastDurationMs = 0;

    String _answerString;
    bool _hasAnswer = false;
    bool _answerTruncated = false;
    uint32_t _droppedResponseBytes = 0;
    uint32_t _rawBytesReceived = 0;

    char _line[FLPROG_HTTP_MAX_LINE_LENGTH + 1] = {0};
    size_t _lineLength = 0;
    uint32_t _headerBytes = 0;
    bool _headersComplete = false;
    int _statusCode = 0;
    uint8_t _httpMajor = 0;
    uint8_t _httpMinor = 0;
    bool _chunked = false;
    bool _connectionClose = false;
    bool _contentLengthKnown = false;
    uint32_t _contentLength = 0;
    uint8_t _bodyMode = FLPROG_HTTP_BODY_NONE;
    uint32_t _bodyBytesReceived = 0;
    char _contentType[FLPROG_HTTP_CONTENT_TYPE_MAX_LENGTH + 1] = {0};
    char _location[FLPROG_HTTP_LOCATION_MAX_LENGTH + 1] = {0};

    uint8_t _chunkState = FLPROG_HTTP_CHUNK_SIZE_LINE;
    uint32_t _chunkRemaining = 0;
    uint32_t _trailerBytes = 0;

    uint8_t _bodyCallbackBuffer[FLPROG_HTTP_BODY_CALLBACK_CHUNK] = {0};
    size_t _bodyCallbackLength = 0;

    FLProgNetworkHttpCallback _callback = nullptr;
    FLProgNetworkHttpByteCallback _byteCallback = nullptr;
    FLProgNetworkHttpBodyCallback _bodyCallback = nullptr;

    uint32_t _requestCount = 0;
    uint32_t _successCount = 0;
    uint32_t _failureCount = 0;
    uint32_t _timeoutCount = 0;
    uint32_t _redirectCount = 0;
    uint32_t _chunkedResponseCount = 0;
    uint32_t _truncatedResponseCount = 0;
    uint32_t _cancelCount = 0;
};
