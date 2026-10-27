#define FLPROG_HTTP_SERVER_HAS_CACHE_POLICY 1
/*
 * FLProg_Network — flprogNetworkHttpServer.h
 * Назначение: HTTP server поверх native TCP server.
 *
 * Архитектурные заметки:
 * - Приём клиента и разбор запроса выполняются неблокирующе.
 * - Сетевой lifecycle наследуется от выбранного NetworkDevice/A3.
 * - Не добавлять сюда platform-specific socket операции.
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
#define FLPROG_HTTP_SERVER_HAS_FLASH_STREAM 1

#include <Arduino.h>
#include <IPAddress.h>
#include <stdint.h>
#include "flprogNetworkService.h"
#include "../protocol/flprogNetworkTcpServer.h"

class FLProgNetworkInterface;
class FLProgNetworkHttpServer;

#ifndef FLPROG_HTTP_SERVER_MAX_CLIENTS
#define FLPROG_HTTP_SERVER_MAX_CLIENTS 2
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HANDLERS
#define FLPROG_HTTP_SERVER_MAX_HANDLERS 14
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HEADERS
#define FLPROG_HTTP_SERVER_MAX_HEADERS 8
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_ARGUMENTS
#define FLPROG_HTTP_SERVER_MAX_ARGUMENTS 8
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_URI_LENGTH
#define FLPROG_HTTP_SERVER_MAX_URI_LENGTH 191U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HOST_LENGTH
#define FLPROG_HTTP_SERVER_MAX_HOST_LENGTH 95U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HEADER_KEY_LENGTH
#define FLPROG_HTTP_SERVER_MAX_HEADER_KEY_LENGTH 31U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH
#define FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH 127U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_ARGUMENT_KEY_LENGTH
#define FLPROG_HTTP_SERVER_MAX_ARGUMENT_KEY_LENGTH 31U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_ARGUMENT_VALUE_LENGTH
#define FLPROG_HTTP_SERVER_MAX_ARGUMENT_VALUE_LENGTH 95U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_REQUEST_LINE
#define FLPROG_HTTP_SERVER_MAX_REQUEST_LINE 255U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_HEADER_BYTES
#define FLPROG_HTTP_SERVER_MAX_HEADER_BYTES 2048U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_BODY_BYTES
#define FLPROG_HTTP_SERVER_MAX_BODY_BYTES 1024U
#endif
#ifndef FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES
#define FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES 2048U
#endif
#ifndef FLPROG_HTTP_SERVER_READ_BUDGET
#define FLPROG_HTTP_SERVER_READ_BUDGET 256U
#endif
#ifndef FLPROG_HTTP_SERVER_WRITE_BUDGET
#define FLPROG_HTTP_SERVER_WRITE_BUDGET 256U
#endif

// Bound synchronous legacy streaming so an abandoned browser connection cannot
// monopolize the cooperative FLProg loop for seconds.
#ifndef FLPROG_HTTP_LEGACY_DIRECT_STALL_MS
#define FLPROG_HTTP_LEGACY_DIRECT_STALL_MS 1000UL
#endif
#ifndef FLPROG_HTTP_SERVER_REQUEST_TIMEOUT_MS
#define FLPROG_HTTP_SERVER_REQUEST_TIMEOUT_MS 5000UL
#endif

// Legacy FLProgWebServer method numbers are intentionally retained where they
// existed. Unknown methods are explicit instead of silently becoming GET.
#define FLPROG_WEB_SERVER_DELETE      0
#define FLPROG_WEB_SERVER_GET         1
#define FLPROG_WEB_SERVER_HEAD        2
#define FLPROG_WEB_SERVER_POST        3
#define FLPROG_WEB_SERVER_PUT         4
#define FLPROG_WEB_SERVER_CONNECT     5
#define FLPROG_WEB_SERVER_OPTIONS     6
#define FLPROG_WEB_SERVER_TRACE       7
#define FLPROG_WEB_SERVER_COPY        8
#define FLPROG_WEB_SERVER_LOCK        9
#define FLPROG_WEB_SERVER_MKCOL       10
#define FLPROG_WEB_SERVER_MOVE        11
#define FLPROG_WEB_SERVER_PROPFIND    12
#define FLPROG_WEB_SERVER_PROPPATCH   13
#define FLPROG_WEB_SERVER_SEARCH      14
#define FLPROG_WEB_SERVER_UNLOCK      15
#define FLPROG_WEB_SERVER_BIND        16
#define FLPROG_WEB_SERVER_REBIND      17
#define FLPROG_WEB_SERVER_UNBIND      18
#define FLPROG_WEB_SERVER_ACL         19
#define FLPROG_WEB_SERVER_REPORT      20
#define FLPROG_WEB_SERVER_MKACTIVITY  21
#define FLPROG_WEB_SERVER_CHECKOUT    22
#define FLPROG_WEB_SERVER_MERGE       23
#define FLPROG_WEB_SERVER_MSEARCH     24
#define FLPROG_WEB_SERVER_NOTIFY      25
#define FLPROG_WEB_SERVER_SUBSCRIBE   26
#define FLPROG_WEB_SERVER_UNSUBSCRIBE 27
#define FLPROG_WEB_SERVER_PATCH       28
#define FLPROG_WEB_SERVER_PURGE       29
#define FLPROG_WEB_SERVER_MKCALENDAR  30
#define FLPROG_WEB_SERVER_LINK        31
#define FLPROG_WEB_SERVER_UNLINK      33
#define FLPROG_WEB_SERVER_UNKNOWN     255

#define FLPROG_HTTP_SERVER_STATE_IDLE         0
#define FLPROG_HTTP_SERVER_STATE_REQUEST_LINE 1
#define FLPROG_HTTP_SERVER_STATE_HEADERS      2
#define FLPROG_HTTP_SERVER_STATE_BODY         3
#define FLPROG_HTTP_SERVER_STATE_DISPATCH     4
#define FLPROG_HTTP_SERVER_STATE_SENDING      5
#define FLPROG_HTTP_SERVER_STATE_CLOSING      6
#define FLPROG_HTTP_SERVER_STATE_ERROR        7

#define FLPROG_HTTP_SERVER_ERROR_NONE                0
#define FLPROG_HTTP_SERVER_ERROR_CONFIG              1
#define FLPROG_HTTP_SERVER_ERROR_SERVICE_LIMIT       2
#define FLPROG_HTTP_SERVER_ERROR_TCP_SERVER          3
#define FLPROG_HTTP_SERVER_ERROR_REQUEST_LINE        4
#define FLPROG_HTTP_SERVER_ERROR_LINE_TOO_LONG       5
#define FLPROG_HTTP_SERVER_ERROR_HEADERS_TOO_LARGE   6
#define FLPROG_HTTP_SERVER_ERROR_TOO_MANY_HEADERS    7
#define FLPROG_HTTP_SERVER_ERROR_BAD_HEADER          8
#define FLPROG_HTTP_SERVER_ERROR_BAD_LENGTH          9
#define FLPROG_HTTP_SERVER_ERROR_BODY_TOO_LARGE      10
#define FLPROG_HTTP_SERVER_ERROR_UNSUPPORTED_TRANSFER 11
#define FLPROG_HTTP_SERVER_ERROR_TIMEOUT             12
#define FLPROG_HTTP_SERVER_ERROR_RESPONSE_TOO_LARGE  13
#define FLPROG_HTTP_SERVER_ERROR_TOO_MANY_ARGUMENTS  14

#define FLPROG_HTTP_SERVER_EVENT_NONE          0x0000U
#define FLPROG_HTTP_SERVER_EVENT_REQUEST       0x0001U
#define FLPROG_HTTP_SERVER_EVENT_RESPONSE      0x0002U
#define FLPROG_HTTP_SERVER_EVENT_NOT_FOUND     0x0004U
#define FLPROG_HTTP_SERVER_EVENT_BAD_REQUEST   0x0008U
#define FLPROG_HTTP_SERVER_EVENT_TIMEOUT       0x0010U
#define FLPROG_HTTP_SERVER_EVENT_BODY_REJECTED 0x0020U
#define FLPROG_HTTP_SERVER_EVENT_CLIENT_CLOSED 0x0040U
#define FLPROG_HTTP_SERVER_EVENT_OVERFLOW      0x0080U

// A legacy/generated WebServer can expose one set of handlers through
// several independently configured FLProgNetworkInterface objects.  The
// primary listener lives in _tcpServer; this limit is for additional child
// listeners and deliberately costs no HTTP parser contexts.
#ifndef FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS
#define FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS 3
#endif

// FLProg generates one legacy WebServer object for each selected network
// interface, although all objects represent the same visual Web page. Merge
// equal-port legacy objects into one HTTP parser/context by default.
#ifndef FLPROG_HTTP_SERVER_LEGACY_AUTOSHARE_SAME_PORT
#define FLPROG_HTTP_SERVER_LEGACY_AUTOSHARE_SAME_PORT 1
#endif

struct FLProgNetworkHttpServerPair
{
    char key[FLPROG_HTTP_SERVER_MAX_HEADER_KEY_LENGTH + 1];
    char value[FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH + 1];
};

struct FLProgNetworkHttpServerArgument
{
    char key[FLPROG_HTTP_SERVER_MAX_ARGUMENT_KEY_LENGTH + 1];
    char value[FLPROG_HTTP_SERVER_MAX_ARGUMENT_VALUE_LENGTH + 1];
};

typedef void (*FLProgNetworkHttpServerCallback)(FLProgNetworkHttpServer *);

class FLProgNetworkHttpServer : public FLProgNetworkService
{
public:
    FLProgNetworkHttpServer();

    // Legacy FLProg generated-code constructor.
    // Stores source+port only; listener starts lazily from pool() when the
    // generated network interface is actually ready.
    FLProgNetworkHttpServer(
        FLProgNetworkInterface *legacyNetwork,
        uint16_t port = 80);

    ~FLProgNetworkHttpServer() override;

    // Parser contexts and child listeners must never be shallow-copied.
    FLProgNetworkHttpServer(const FLProgNetworkHttpServer &) = delete;
    FLProgNetworkHttpServer &operator=(const FLProgNetworkHttpServer &) = delete;

    bool begin(
        FLProgNetworkInterface &network,
        uint16_t port = 80,
        uint8_t route = 0,
        uint8_t maxClients = FLPROG_HTTP_SERVER_MAX_CLIENTS);

    // Add one route-fixed listener while retaining this server's handlers and
    // HTTP parser storage.  It is intended for a single generated WebServer
    // to serve Wi-Fi and several independent Wiznet interfaces on the same
    // TCP port. Call after the primary listener has started.
    bool addSharedListener(
        FLProgNetworkInterface &network,
        // This header is included before flprogNetwork.h defines the public
        // route constants; 0 is the stable DEFAULT route value.
        uint8_t route = 0);
    uint8_t sharedListenerCount() const { return _sharedListenerCount; }
    uint8_t sharedListenerSocketStatus(uint8_t index) const;
    bool sharedListenerSendPending(uint8_t index) const;
    uint8_t sharedListenerLastWiznetCloseEvent(uint8_t index) const;
    void end();
    void pool() override;

    // Browser clients routinely include optional client-hint and fetch
    // headers that exceed the compact retained-header table.  When enabled,
    // protocol-critical headers are still parsed, while surplus optional
    // headers are ignored instead of producing HTTP 431.
    void setBrowserHeaderCompatibility(bool value) { _browserHeaderCompatibility = value; }
    bool browserHeaderCompatibility() const { return _browserHeaderCompatibility; }

    bool addHandler(const char *uri, FLProgNetworkHttpServerCallback callback, uint8_t method);
    bool addHandler(const String &uri, FLProgNetworkHttpServerCallback callback, uint8_t method)
    {
        return addHandler(uri.c_str(), callback, method);
    }
    bool addHandler(const char *uri, FLProgNetworkHttpServerCallback callback)
    {
        return addHandler(uri, callback, FLPROG_WEB_SERVER_GET);
    }
    bool addHandler(const String &uri, FLProgNetworkHttpServerCallback callback)
    {
        return addHandler(uri.c_str(), callback, FLPROG_WEB_SERVER_GET);
    }
    bool addHandler(const char *uri, FLProgNetworkHttpServerCallback callback, const char *method);
    bool addHandler(const String &uri, FLProgNetworkHttpServerCallback callback, const String &method)
    {
        return addHandler(uri.c_str(), callback, method.c_str());
    }
    void add404Page(FLProgNetworkHttpServerCallback callback) { _notFoundCallback = callback; }
    void clearHandlers();

    uint8_t method() const;
    const char *methodName() const;
    String uri() const;
    uint8_t methodVersion() const;
    uint8_t httpMajor() const;
    uint8_t httpMinor() const;
    String host() const;

    uint16_t headersCount() const;
    String headerKeyAtIndex(uint16_t index) const;
    bool hasHeaderKey(const String &key) const;
    bool hasHeaderKey(const char *key) const;
    String headerValueAtKey(const String &key) const;
    String headerValueAtKey(const char *key) const;

    uint16_t argumentsCount() const;
    String argumentKeyAtIndex(uint16_t index) const;
    bool hasArgumentKey(const String &key) const;
    bool hasArgumentKey(const char *key) const;
    String argumentValueAtKey(const String &key) const;
    String argumentValueAtKey(const char *key) const;

    const uint8_t *bodyData() const;
    const char *body() const;
    uint32_t bodyLength() const;
    uint32_t contentLength() const;
    String contentType() const;

    bool send(int statusCode, const char *contentType, const char *body);
    // Copy an immutable flash-resident asset into the existing bounded
    // response queue. On ESP8266 this avoids retaining Web UI assets in DRAM.
    bool sendProgmem(int statusCode, const char *contentType, const char *body);
    // Native callbacks only. Data must remain immutable and valid until the
    // connection closes (normally static PROGMEM). No network I/O here.
    // Reuses request-line storage after parsing; no new per-client allocation.
    bool sendStreamProgmem(int statusCode, const char *contentType,
                          const void *data, uint32_t length,
                          const char *contentEncoding = nullptr, bool cacheImmutable = false);
    bool send(int statusCode, const String &contentType, const String &body)
    {
        return send(statusCode, contentType.c_str(), body.c_str());
    }
    bool sendHeaderOnly(int statusCode, const char *contentType = "text/plain", uint32_t contentLength = 0);
    void sendDefault200Page();
    void sendDefault404Page();
    void send403Page(String value);
    void sendJson(String value);

    // Legacy-compatible raw response builder for callbacks that used
    // server->print(...). Bytes are buffered and sent cooperatively after the
    // callback returns; no network I/O is performed by print().
    size_t write(const uint8_t *buffer, size_t size);
    size_t write(uint8_t value) { return write(&value, 1); }
    size_t print(const char *value);
    size_t print(const String &value) { return print(value.c_str()); }
    size_t print(char value) { return write((uint8_t)value); }
    size_t print(int value);
    size_t print(unsigned int value);
    size_t print(long value);
    size_t print(unsigned long value);
    size_t println(const char *value);
    size_t println(const String &value) { return println(value.c_str()); }
    size_t println();

    bool responseQueued() const;
    bool responseOverflowed() const;

    uint8_t getStatus() const { return _legacyGroupMaster ? _legacyGroupMaster->getStatus() : (_running ? 1 : 0); }
    uint8_t getError() const { return _error; }
    uint8_t error() const { return _error; }
    void clearError() { _error = FLPROG_HTTP_SERVER_ERROR_NONE; }

    uint16_t events() const { return _events; }
    uint16_t takeEvents()
    {
        const uint16_t value = _events;
        _events = FLPROG_HTTP_SERVER_EVENT_NONE;
        return value;
    }

    // Inactivity limit for request input and buffered response output; 0 disables it.
    void setRequestTimeout(uint32_t value) { _requestTimeoutMs = value; }
    uint32_t requestTimeout() const { return _requestTimeoutMs; }
    void setReadBudget(uint16_t value) { _readBudget = value == 0 ? 1 : value; }
    void setWriteBudget(uint16_t value) { _writeBudget = value == 0 ? 1 : value; }

    // LEGACY API scheduling compatibility. New code normally leaves this at 0.
    // A value of N processes HTTP parsing on
    // every (N+1)-th service pool pass; the underlying TCP server still remains
    // managed by network.pool(). New code should normally leave this at zero.
    uint16_t getSkippingEvents() const { return _skippingEvents; }
    void setSkippingEvents(uint16_t value) { _skippingEvents = value; }

    uint16_t port() const { return _port; }
    uint8_t route() const { return _route; }
    uint8_t maxClients() const { return _maxClients; }
    bool listening() const { return _legacyGroupMaster ? _legacyGroupMaster->listening() : _tcpServer.listening(); }
    uint8_t clientCount() const { return _tcpServer.clientCount(); }
    FLProgNetworkTcpServer *tcpServer() { return &_tcpServer; }

    uint32_t requestCount() const { return _requestCount; }
    uint32_t responseCount() const { return _responseCount; }
    uint32_t notFoundCount() const { return _notFoundCount; }
    uint32_t badRequestCount() const { return _badRequestCount; }
    uint32_t timeoutCount() const { return _timeoutCount; }
    uint32_t rejectedBodyCount() const { return _rejectedBodyCount; }
    uint32_t responseOverflowCount() const { return _responseOverflowCount; }
    void resetStatistics();

    static uint8_t getHttpMethodCode(const char *method);

private:
    struct Handler
    {
        char uri[FLPROG_HTTP_SERVER_MAX_URI_LENGTH + 1];
        uint8_t method;
        FLProgNetworkHttpServerCallback callback;
    };

    struct ClientContext
    {
        bool used;
        uint8_t state;
        uint8_t error;
        uint32_t startedAt;
        uint32_t lastActivityAt;
        uint32_t observedRxBytes;
        uint32_t observedTxBytes;

        union {
            char line[FLPROG_HTTP_SERVER_MAX_REQUEST_LINE + 1];
            struct {
                const uint8_t *data;
                uint32_t length;
                uint32_t offset;
            } flash;
        };
        static_assert(sizeof(flash) <= FLPROG_HTTP_SERVER_MAX_REQUEST_LINE + 1,
                      "Flash cursor must fit the existing request line");
        uint16_t lineLength;
        uint32_t headerBytes;

        uint8_t method;
        char methodName[16];
        uint8_t httpMajor;
        uint8_t httpMinor;
        char uri[FLPROG_HTTP_SERVER_MAX_URI_LENGTH + 1];
        char host[FLPROG_HTTP_SERVER_MAX_HOST_LENGTH + 1];

        FLProgNetworkHttpServerPair headers[FLPROG_HTTP_SERVER_MAX_HEADERS];
        uint8_t headerCount;
        FLProgNetworkHttpServerArgument arguments[FLPROG_HTTP_SERVER_MAX_ARGUMENTS];
        uint8_t argumentCount;

        uint32_t contentLength;
        bool contentLengthSeen;
        uint32_t bodyReceived;
        char contentType[FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH + 1];
        uint8_t body[FLPROG_HTTP_SERVER_MAX_BODY_BYTES + 1];

        char response[FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES + 1];
        uint16_t responseLength;
        uint16_t responseOffset;
        bool responseOverflow;
        uint8_t responseStructured; // 0 raw, 1 bounded structured, 2 Flash stream
    };

public:
    static constexpr size_t clientContextBytes() { return sizeof(ClientContext); }
private:
    static bool equalsIgnoreCase(const char *left, const char *right);
    static void copyBounded(char *destination, size_t capacity, const char *source);
    static int hexValue(char value);
    static bool parseUnsigned(const char *value, uint32_t &result);
    static const char *reasonPhrase(int statusCode);

    bool ensureContextCapacity(uint8_t required);
    void resetContext(ClientContext &ctx, bool used);
    void updateClient(uint8_t slot, FLProgNetworkTcpServerClient &client);
    void processInput(uint8_t slot, FLProgNetworkTcpServerClient &client, ClientContext &ctx);
    bool feedByte(uint8_t slot, ClientContext &ctx, uint8_t value);
    bool consumeLine(uint8_t slot, ClientContext &ctx);
    bool parseRequestLine(ClientContext &ctx);
    bool parseHeaderLine(ClientContext &ctx);
    bool finishHeaders(uint8_t slot, ClientContext &ctx);
    bool parseArguments(ClientContext &ctx, const char *query);
    void dispatch(uint8_t slot, ClientContext &ctx);
    void queueProtocolError(uint8_t slot, ClientContext &ctx, int statusCode, uint8_t errorCode);
    void flushResponse(uint8_t slot, FLProgNetworkTcpServerClient &client, ClientContext &ctx);
    bool appendResponse(ClientContext &ctx, const uint8_t *data, size_t length);
    bool appendResponseProgmem(ClientContext &ctx, const char *data, size_t length);
    bool buildStructuredResponse(ClientContext &ctx, int statusCode, const char *contentType, const char *body, uint32_t bodyLength, bool headerOnly);
    bool buildStructuredResponseProgmem(ClientContext &ctx, int statusCode, const char *contentType, const char *body, uint32_t bodyLength, bool headerOnly);

    // WS03A: legacy FLProg generated pages are streamed through the accepted
    // TCP client instead of buffering the entire HTML page in RAM.
    size_t legacyWriteDirect(const uint8_t *data, size_t length);
    ClientContext *currentContext();
    const ClientContext *currentContext() const;
    void urlDecodeInto(const char *source, size_t length, char *destination, size_t capacity) const;

    FLProgNetworkTcpServer _tcpServer;
    Handler _handlers[FLPROG_HTTP_SERVER_MAX_HANDLERS];
    uint8_t _handlerCount;
    FLProgNetworkHttpServerCallback _notFoundCallback;
    // Allocate only the contexts actually requested by begin(). Legacy FLProg
    // WebServer always asks for one client, so dual Wi-Fi+W5500 does not reserve
    // two full HTTP contexts per server in static DRAM.
    ClientContext *_clients = nullptr;
    uint8_t _clientCapacity = 0;

    bool _running;
    uint8_t _route;
    uint16_t _port;
    uint8_t _maxClients;
    int8_t _currentSlot;
    uint8_t _error;
    uint16_t _events;
    uint32_t _requestTimeoutMs;
    uint16_t _readBudget;
    uint16_t _writeBudget;
    uint16_t _skippingEvents;
    uint16_t _skipCounter;

    uint32_t _requestCount;
    uint32_t _responseCount;
    uint32_t _notFoundCount;
    uint32_t _badRequestCount;
    uint32_t _timeoutCount;
    uint32_t _rejectedBodyCount;
    uint32_t _responseOverflowCount;

    // WS02 legacy generated-code binding. The network object is externally
    // owned by the FLProg generated sketch.
    FLProgNetworkInterface *_legacyNetwork;
    bool _legacyAutoStart;
    bool _browserHeaderCompatibility;

    // A follower is a second/third FLProg-generated legacy WebServer on the
    // same port. It owns no parser and contributes only its TCP listener.
    FLProgNetworkHttpServer *_legacyGroupMaster;
    uint8_t _legacyGroupAttached;
    static FLProgNetworkHttpServer *_legacyGroupMasters[4];
    void poolLegacyGroupFollower();

    // AP04-fix1: one legacy FLProg Wi-Fi WebServer may need to listen on
    // both STA and SoftAP at the same time. Keep the second TCP listener
    // dynamic so normal/native and wired-only objects pay no static RAM cost.
    FLProgNetworkTcpServer *_legacySecondaryTcpServer;
    uint8_t _legacySecondaryRoute;

    // External child listeners for the same logical WebServer. They are
    // pooled by this parent and therefore do not consume managed-service
    // slots in their source interfaces.
    FLProgNetworkTcpServer *_sharedListeners[
        FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS
    ];
    uint8_t _sharedListenerCount;

    // Runtime STA+SoftAP compatibility: both route-fixed TCP listeners share
    // one large legacy HTTP parser context on ESP8266. 0=none, 1=primary,
    // 2=secondary. This avoids allocating a second ~6 KB ClientContext.
    uint8_t _legacySharedContextOwner;

    // Client currently being parsed. dispatch() uses this pointer instead of
    // assuming that every legacy request came from the primary TCP server.
    FLProgNetworkTcpServerClient *_legacyProcessingClient;

    // Valid only while a legacy request handler is executing.
    FLProgNetworkTcpServerClient *_legacyDirectClient;
    bool _legacyDirectResponse;
    bool _legacyDirectFailed;
    uint32_t _legacyDirectBytes;
};
