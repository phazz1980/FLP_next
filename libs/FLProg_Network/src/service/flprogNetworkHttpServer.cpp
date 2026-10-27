/*
 * FLProg_Network — flprogNetworkHttpServer.cpp
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
#include <new>
#include "flprogNetworkHttpServer.h"
#include "../flprogNetwork.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>

FLProgNetworkHttpServer *FLProgNetworkHttpServer::_legacyGroupMasters[4] = {nullptr, nullptr, nullptr, nullptr};

FLProgNetworkHttpServer::FLProgNetworkHttpServer()
    : _handlerCount(0),
      _notFoundCallback(nullptr),
      _running(false),
      _route(0),
      _port(80),
      _maxClients(FLPROG_HTTP_SERVER_MAX_CLIENTS),
      _currentSlot(-1),
      _error(FLPROG_HTTP_SERVER_ERROR_NONE),
      _events(FLPROG_HTTP_SERVER_EVENT_NONE),
      _requestTimeoutMs(FLPROG_HTTP_SERVER_REQUEST_TIMEOUT_MS),
      _readBudget(FLPROG_HTTP_SERVER_READ_BUDGET),
      _writeBudget(FLPROG_HTTP_SERVER_WRITE_BUDGET),
      _skippingEvents(0),
      _skipCounter(0),
      _requestCount(0),
      _responseCount(0),
      _notFoundCount(0),
      _badRequestCount(0),
      _timeoutCount(0),
      _rejectedBodyCount(0),
      _responseOverflowCount(0),
      _legacyNetwork(nullptr),
      _legacyAutoStart(false),
      _browserHeaderCompatibility(false),
      _legacyGroupMaster(nullptr),
      _legacyGroupAttached(false),
      _legacySecondaryTcpServer(nullptr),
      _legacySecondaryRoute(FLPROG_NETWORK_ROUTE_DEFAULT),
      _sharedListenerCount(0),
      _legacySharedContextOwner(0),
      _legacyProcessingClient(nullptr),
      _legacyDirectClient(nullptr),
      _legacyDirectResponse(false),
      _legacyDirectFailed(false),
      _legacyDirectBytes(0)
{
    for (uint8_t i = 0; i < FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS; ++i)
    {
        _sharedListeners[i] = nullptr;
    }
}

// WS02 compatibility constructor expected by FLProg 9.x generated projects.
// Historical behavior is preserved: do not start networking during global
// static construction; remember source+port and start later from pool().
FLProgNetworkHttpServer::FLProgNetworkHttpServer(
    FLProgNetworkInterface *legacyNetwork,
    uint16_t portValue)
    : FLProgNetworkHttpServer()
{
    _legacyNetwork = legacyNetwork;
    _port = portValue == 0 ? 80 : portValue;
    _maxClients = 1;
    _legacyAutoStart = legacyNetwork != nullptr;

#if FLPROG_HTTP_SERVER_LEGACY_AUTOSHARE_SAME_PORT
    if (_legacyAutoStart)
    {
        for (uint8_t i = 0; i < 4; ++i)
        {
            if (_legacyGroupMasters[i] != nullptr && _legacyGroupMasters[i]->_port == _port)
            {
                _legacyGroupMaster = _legacyGroupMasters[i];
                return;
            }
        }
        for (uint8_t i = 0; i < 4; ++i)
        {
            if (_legacyGroupMasters[i] == nullptr)
            {
                _legacyGroupMasters[i] = this;
                break;
            }
        }
    }
#endif
}

FLProgNetworkHttpServer::~FLProgNetworkHttpServer()
{
    for (uint8_t i = 0; i < 4; ++i)
    {
        if (_legacyGroupMasters[i] == this) _legacyGroupMasters[i] = nullptr;
    }
    end();
    delete[] _clients;
    _clients = nullptr;
    _clientCapacity = 0;
}

void FLProgNetworkHttpServer::poolLegacyGroupFollower()
{
    if (_legacyGroupMaster == nullptr || _legacyNetwork == nullptr) return;

    // The primary object starts on its own interface. Followers only add an
    // interface once it is actually ready, so an absent Wi-Fi module never
    // blocks working W5500 listeners.
    if (!_legacyGroupMaster->_running) return;

    const uint8_t mode = _legacyNetwork->networkMode();
    if (mode == FLPROG_NETWORK_MODE_WIFI)
    {
        if (!(_legacyGroupAttached & 0x01) && _legacyNetwork->wifiIsReady() &&
            _legacyGroupMaster->addSharedListener(*_legacyNetwork, FLPROG_NETWORK_ROUTE_WIFI))
        {
            _legacyGroupAttached |= 0x01;
        }
        if (!(_legacyGroupAttached & 0x02) && _legacyNetwork->accessPointIsReady() &&
            _legacyGroupMaster->addSharedListener(*_legacyNetwork, FLPROG_NETWORK_ROUTE_WIFI_AP))
        {
            _legacyGroupAttached |= 0x02;
        }
        return;
    }

    if (!(_legacyGroupAttached & 0x01) && _legacyNetwork->isReady() &&
        _legacyGroupMaster->addSharedListener(*_legacyNetwork, FLPROG_NETWORK_ROUTE_ETHERNET))
    {
        _legacyGroupAttached |= 0x01;
    }
}

bool FLProgNetworkHttpServer::ensureContextCapacity(uint8_t required)
{
    if (required == 0 || required > FLPROG_HTTP_SERVER_MAX_CLIENTS) return false;
    if (_clients != nullptr && _clientCapacity >= required) return true;

    ClientContext *replacement = new (std::nothrow) ClientContext[required];
    if (replacement == nullptr) return false;

    for (uint8_t i = 0; i < required; ++i)
    {
        resetContext(replacement[i], false);
    }

    delete[] _clients;
    _clients = replacement;
    _clientCapacity = required;
    return true;
}

bool FLProgNetworkHttpServer::equalsIgnoreCase(const char *left, const char *right)
{
    if (left == nullptr || right == nullptr) return left == right;
    while (*left != 0 && *right != 0)
    {
        char a = *left++;
        char b = *right++;
        if (a >= 'A' && a <= 'Z') a = (char)(a - 'A' + 'a');
        if (b >= 'A' && b <= 'Z') b = (char)(b - 'A' + 'a');
        if (a != b) return false;
    }
    return *left == 0 && *right == 0;
}

void FLProgNetworkHttpServer::copyBounded(char *destination, size_t capacity, const char *source)
{
    if (destination == nullptr || capacity == 0) return;
    if (source == nullptr) source = "";
    size_t length = std::strlen(source);
    if (length >= capacity) length = capacity - 1;
    if (length != 0) std::memcpy(destination, source, length);
    destination[length] = 0;
}

int FLProgNetworkHttpServer::hexValue(char value)
{
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    if (value >= 'A' && value <= 'F') return value - 'A' + 10;
    return -1;
}

bool FLProgNetworkHttpServer::parseUnsigned(const char *value, uint32_t &result)
{
    if (value == nullptr || *value == 0) return false;
    uint32_t parsed = 0;
    bool any = false;
    while (*value == ' ' || *value == '\t') ++value;
    while (*value >= '0' && *value <= '9')
    {
        const uint8_t digit = (uint8_t)(*value - '0');
        if (parsed > (0xFFFFFFFFUL - digit) / 10UL) return false;
        parsed = parsed * 10UL + digit;
        any = true;
        ++value;
    }
    while (*value == ' ' || *value == '\t') ++value;
    if (!any || *value != 0) return false;
    result = parsed;
    return true;
}

const char *FLProgNetworkHttpServer::reasonPhrase(int statusCode)
{
    switch (statusCode)
    {
        case 200: return "OK";
        case 201: return "Created";
        case 202: return "Accepted";
        case 204: return "No Content";
        case 301: return "Moved Permanently";
        case 302: return "Found";
        case 303: return "See Other";
        case 307: return "Temporary Redirect";
        case 308: return "Permanent Redirect";
        case 400: return "Bad Request";
        case 403: return "Forbidden";
        case 404: return "Not Found";
        case 405: return "Method Not Allowed";
        case 408: return "Request Timeout";
        case 411: return "Length Required";
        case 413: return "Payload Too Large";
        case 414: return "URI Too Long";
        case 431: return "Request Header Fields Too Large";
        case 500: return "Internal Server Error";
        case 501: return "Not Implemented";
        case 503: return "Service Unavailable";
        default: return "OK";
    }
}

bool FLProgNetworkHttpServer::begin(
    FLProgNetworkInterface &network,
    uint16_t portValue,
    uint8_t routeValue,
    uint8_t maxClientsValue)
{
    end();
    if (portValue == 0 || maxClientsValue == 0 || maxClientsValue > FLPROG_HTTP_SERVER_MAX_CLIENTS)
    {
        _error = FLPROG_HTTP_SERVER_ERROR_CONFIG;
        return false;
    }

    _route = routeValue;
    _port = portValue;
    _maxClients = maxClientsValue;
    _events = FLPROG_HTTP_SERVER_EVENT_NONE;
    _error = FLPROG_HTTP_SERVER_ERROR_NONE;

    if (!ensureContextCapacity(maxClientsValue))
    {
        _error = FLPROG_HTTP_SERVER_ERROR_SERVICE_LIMIT;
        return false;
    }

    // Register the TCP server first so its accept/cleanup work runs before the
    // HTTP parser in the same FLProgNetworkInterface::pool() pass.
    if (!_tcpServer.begin(network, portValue, routeValue, maxClientsValue))
    {
        _error = FLPROG_HTTP_SERVER_ERROR_TCP_SERVER;
        return false;
    }
    if (!attach(network))
    {
        _tcpServer.end();
        _error = FLPROG_HTTP_SERVER_ERROR_SERVICE_LIMIT;
        return false;
    }

    for (uint8_t i = 0; i < _clientCapacity; ++i)
    {
        resetContext(_clients[i], false);
    }
    _running = true;
    return true;
}

bool FLProgNetworkHttpServer::addSharedListener(
    FLProgNetworkInterface &network,
    uint8_t routeValue)
{
    // This is intentionally an opt-in low-level extension. Generated legacy
    // code continues to use its one source interface unchanged.
    if (!_running || _maxClients != 1 ||
        _sharedListenerCount >= FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS)
    {
        return false;
    }

    FLProgNetworkTcpServer *listener = new (std::nothrow) FLProgNetworkTcpServer();
    if (listener == nullptr)
    {
        return false;
    }

    if (!listener->beginChild(network, _port, routeValue, 1))
    {
        delete listener;
        return false;
    }

    _sharedListeners[_sharedListenerCount++] = listener;
    return true;
}

uint8_t FLProgNetworkHttpServer::sharedListenerSocketStatus(uint8_t index) const
{
    if (index >= _sharedListenerCount || _sharedListeners[index] == nullptr)
    {
        return FLPROG_WIZNET_SOCK_CLOSED;
    }
    return _sharedListeners[index]->wiznetSocketStatus();
}

bool FLProgNetworkHttpServer::sharedListenerSendPending(uint8_t index) const
{
    return index < _sharedListenerCount && _sharedListeners[index] != nullptr &&
           _sharedListeners[index]->wiznetSendPending();
}

uint8_t FLProgNetworkHttpServer::sharedListenerLastWiznetCloseEvent(uint8_t index) const
{
    if (index >= _sharedListenerCount || _sharedListeners[index] == nullptr)
    {
        return FLPROG_TCP_BACKEND_EVENT_NONE;
    }
    return _sharedListeners[index]->lastWiznetCloseEvent();
}

void FLProgNetworkHttpServer::end()
{
    _running = false;
    _currentSlot = -1;
    _legacySharedContextOwner = 0;
    _legacyProcessingClient = nullptr;
    _legacyDirectClient = nullptr;
    _legacyDirectResponse = false;
    _legacyDirectFailed = false;
    detach();
    _tcpServer.end();
    if (_legacySecondaryTcpServer != nullptr)
    {
        _legacySecondaryTcpServer->end();
        delete _legacySecondaryTcpServer;
        _legacySecondaryTcpServer = nullptr;
    }
    for (uint8_t i = 0; i < _sharedListenerCount; ++i)
    {
        if (_sharedListeners[i] != nullptr)
        {
            _sharedListeners[i]->end();
            delete _sharedListeners[i];
            _sharedListeners[i] = nullptr;
        }
    }
    _sharedListenerCount = 0;
    _legacySecondaryRoute = FLPROG_NETWORK_ROUTE_DEFAULT;
    for (uint8_t i = 0; i < _clientCapacity; ++i)
    {
        resetContext(_clients[i], false);
    }
}

void FLProgNetworkHttpServer::clearHandlers()
{
    _handlerCount = 0;
    _notFoundCallback = nullptr;
}

bool FLProgNetworkHttpServer::addHandler(
    const char *uriValue,
    FLProgNetworkHttpServerCallback callback,
    uint8_t methodValue)
{
    if (callback == nullptr || _handlerCount >= FLPROG_HTTP_SERVER_MAX_HANDLERS)
    {
        return false;
    }
    if (uriValue == nullptr) uriValue = "";
    char normalized[FLPROG_HTTP_SERVER_MAX_URI_LENGTH + 1];
    if (uriValue[0] == 0)
    {
        copyBounded(normalized, sizeof(normalized), "/");
    }
    else if (uriValue[0] == '/')
    {
        if (std::strlen(uriValue) > FLPROG_HTTP_SERVER_MAX_URI_LENGTH) return false;
        copyBounded(normalized, sizeof(normalized), uriValue);
    }
    else
    {
        const size_t length = std::strlen(uriValue);
        if (length + 1 > FLPROG_HTTP_SERVER_MAX_URI_LENGTH) return false;
        normalized[0] = '/';
        std::memcpy(normalized + 1, uriValue, length + 1);
    }

    Handler &handler = _handlers[_handlerCount++];
    copyBounded(handler.uri, sizeof(handler.uri), normalized);
    handler.method = methodValue;
    handler.callback = callback;
    return true;
}

bool FLProgNetworkHttpServer::addHandler(
    const char *uriValue,
    FLProgNetworkHttpServerCallback callback,
    const char *methodValue)
{
    const uint8_t code = getHttpMethodCode(methodValue);
    if (code == FLPROG_WEB_SERVER_UNKNOWN) return false;
    return addHandler(uriValue, callback, code);
}

void FLProgNetworkHttpServer::resetContext(ClientContext &ctx, bool usedValue)
{
    std::memset(&ctx, 0, sizeof(ctx));
    ctx.used = usedValue;
    ctx.state = usedValue ? FLPROG_HTTP_SERVER_STATE_REQUEST_LINE : FLPROG_HTTP_SERVER_STATE_IDLE;
    ctx.method = FLPROG_WEB_SERVER_UNKNOWN;
    if (usedValue)
    {
        ctx.startedAt = millis();
        ctx.lastActivityAt = ctx.startedAt;
    }
}

void FLProgNetworkHttpServer::pool()
{
    if (_legacyGroupMaster != nullptr)
    {
        poolLegacyGroupFollower();
        return;
    }

    // WS02 legacy generated-code auto-start.
    // FLProgWebServer(source, port) historically had no explicit begin()
    // call in generated sketches, so start only when source becomes ready.
    if (!_running && _legacyAutoStart)
    {
        if (_legacyNetwork == nullptr)
        {
            return;
        }

        // If the generated Wi-Fi interface is configured as a station as
        // well as SoftAP, the configured station address is the primary Web
        // endpoint.  Do not let SoftAP readiness start this one server first:
        // otherwise it remains bound to AP and the STA IP never listens.
        const uint8_t legacyModeBeforeStart = _legacyNetwork->networkMode();
        if (legacyModeBeforeStart == FLPROG_NETWORK_MODE_WIFI &&
            _legacyNetwork->clientMode() &&
            !_legacyNetwork->wifiIsReady())
        {
            return;
        }

        if (!_legacyNetwork->isReady())
        {
            return;
        }

        // WS09/AP runtime compatibility:
        // A generated project may have independent Wi-Fi and W5500 WebServer
        // objects, while one Wi-Fi interface can itself operate as STA+SoftAP.
        // For Wi-Fi+AP choose whichever Wi-Fi side is currently ready as the
        // primary listener; the other side is attached below as a lightweight
        // route-fixed TCP listener sharing the same HTTP parser context.
        uint8_t legacyRoute = FLPROG_NETWORK_ROUTE_DEFAULT;
        const uint8_t legacyMode = _legacyNetwork->networkMode();
        const bool legacyWifiAp =
            legacyMode == FLPROG_NETWORK_MODE_WIFI &&
            _legacyNetwork->accessPointMode();

        if (legacyMode == FLPROG_NETWORK_MODE_WIFI)
        {
            if (legacyWifiAp && !_legacyNetwork->clientMode())
            {
                legacyRoute = FLPROG_NETWORK_ROUTE_WIFI_AP;
            }
            else
            {
                legacyRoute = FLPROG_NETWORK_ROUTE_WIFI;
            }
        }
        else if (legacyMode == FLPROG_NETWORK_MODE_ETHERNET)
        {
            legacyRoute = FLPROG_NETWORK_ROUTE_ETHERNET;
        }

        if (!begin(
                *_legacyNetwork,
                _port,
                legacyRoute,
                1))
        {
            return;
        }

        // On ESP8266 keep one large legacy HTTP parsing context for STA+SoftAP.
        // The two TCP listeners are serialized through that context below.
    }

    if (!_running || _clients == nullptr) return;

    // Runtime SoftAP enable must not grow the legacy parser from one
    // ClientContext to two. Keep one parser context and serialize STA/AP.
    const bool legacyDualWifiNow =
        _legacyAutoStart &&
        _legacyNetwork != nullptr &&
        _legacyNetwork->networkMode() == FLPROG_NETWORK_MODE_WIFI &&
        _legacyNetwork->accessPointMode();

    // A legacy STA+AP WebServer can auto-start while only the AP personality
    // is reported ready. If that AP route has no TCP transport yet, do not
    // leave the single server permanently bound to a dead route once STA
    // becomes available. Move the primary listener to STA; the AP is attached
    // below as the existing secondary listener when it is usable.
    if (legacyDualWifiNow &&
        _route == FLPROG_NETWORK_ROUTE_WIFI_AP &&
        _legacyNetwork->wifiIsReady() &&
        _tcpServer.boundTransportId() == FLPROG_TRANSPORT_NONE)
    {
        _route = FLPROG_NETWORK_ROUTE_WIFI;
        _tcpServer.setRoute(FLPROG_NETWORK_ROUTE_WIFI);
    }

    // AP04-fix1: once the other Wi-Fi personality becomes ready, add a second
    // route-fixed TCP listener on the same port. The HTTP handlers/callbacks
    // remain shared by this single legacy WebServer object.
    if (legacyDualWifiNow && _clientCapacity >= 1)
    {
        uint8_t secondaryRoute = FLPROG_NETWORK_ROUTE_DEFAULT;
        bool secondaryReady = false;
        if (_route == FLPROG_NETWORK_ROUTE_WIFI)
        {
            secondaryRoute = FLPROG_NETWORK_ROUTE_WIFI_AP;
            secondaryReady = _legacyNetwork->accessPointIsReady();
        }
        else if (_route == FLPROG_NETWORK_ROUTE_WIFI_AP)
        {
            secondaryRoute = FLPROG_NETWORK_ROUTE_WIFI;
            secondaryReady = _legacyNetwork->wifiIsReady();
        }

        if (secondaryReady &&
            secondaryRoute != FLPROG_NETWORK_ROUTE_DEFAULT &&
            _legacySecondaryTcpServer == nullptr)
        {
            FLProgNetworkTcpServer *secondary = new (std::nothrow) FLProgNetworkTcpServer();
            if (secondary != nullptr)
            {
                // AP_SHARED_SAME_PORT_FIX1:
                // STA and SoftAP expose the same legacy Web interface on the same
                // configured TCP port. The listeners are route-fixed, so the local
                // endpoint is distinguished by interface/IP rather than by port.
                const uint16_t secondaryPort = _port;
                if (secondary->beginChild(*_legacyNetwork, secondaryPort, secondaryRoute, 1))
                {
                    _legacySecondaryTcpServer = secondary;
                    _legacySecondaryRoute = secondaryRoute;
                }
                else
                {
                    delete secondary;
                }
            }
        }
    }

    // The child listener is pooled by the parent HTTP service because it is
    // intentionally not registered as a separate managed service.
    if (_legacySecondaryTcpServer != nullptr)
    {
        _legacySecondaryTcpServer->pool();
    }
    for (uint8_t i = 0; i < _sharedListenerCount; ++i)
    {
        if (_sharedListeners[i] != nullptr)
        {
            _sharedListeners[i]->pool();
        }
    }


    if (_skipCounter < _skippingEvents)
    {
        ++_skipCounter;
        return;
    }
    _skipCounter = 0;

    // In the one-client legacy WebServer path every attached listener uses one
    // large HTTP parser context. Preserve ownership until the selected client
    // closes; requests accepted by other listeners wait in their TCP server.
    const bool legacySharedDual =
        (_legacySecondaryTcpServer != nullptr || _sharedListenerCount != 0) &&
        _maxClients == 1 &&
        _clientCapacity == 1;

    if (legacySharedDual)
    {
        FLProgNetworkTcpServer *listeners[
            2 + FLPROG_HTTP_SERVER_MAX_SHARED_LISTENERS
        ];
        uint8_t listenerCount = 0;
        listeners[listenerCount++] = &_tcpServer;
        if (_legacySecondaryTcpServer != nullptr)
        {
            listeners[listenerCount++] = _legacySecondaryTcpServer;
        }
        for (uint8_t i = 0; i < _sharedListenerCount; ++i)
        {
            if (_sharedListeners[i] != nullptr)
            {
                listeners[listenerCount++] = _sharedListeners[i];
            }
        }

        ClientContext &ctx = _clients[0];
        FLProgNetworkTcpServerClient *selected = nullptr;
        if (_legacySharedContextOwner != 0 &&
            _legacySharedContextOwner <= listenerCount)
        {
            selected = listeners[_legacySharedContextOwner - 1]->client(0);
            if (selected == nullptr || !selected->active())
            {
                if (ctx.used)
                {
                    resetContext(ctx, false);
                    _events |= FLPROG_HTTP_SERVER_EVENT_CLIENT_CLOSED;
                }
                _legacySharedContextOwner = 0;
                selected = nullptr;
            }
        }

        if (_legacySharedContextOwner == 0)
        {
            for (uint8_t i = 0; i < listenerCount; ++i)
            {
                FLProgNetworkTcpServerClient *candidate = listeners[i]->client(0);
                if (candidate != nullptr && candidate->active())
                {
                    _legacySharedContextOwner = (uint8_t)(i + 1);
                    selected = candidate;
                    resetContext(ctx, true);
                    break;
                }
            }
        }

        if (selected != nullptr && selected->active())
        {
            if (!ctx.used ||
                selected->rxBytes() < ctx.observedRxBytes ||
                selected->txBytes() < ctx.observedTxBytes)
            {
                resetContext(ctx, true);
            }
            updateClient(0, *selected);
            ctx.observedRxBytes = selected->rxBytes();
            ctx.observedTxBytes = selected->txBytes();
        }
        return;
    }

    // Primary listener contexts occupy [0 .. _maxClients-1].
    for (uint8_t slot = 0; slot < _maxClients; ++slot)
    {
        FLProgNetworkTcpServerClient *client = _tcpServer.client(slot);
        if (client == nullptr) continue;
        ClientContext &ctx = _clients[slot];

        if (client->active())
        {
            if (!ctx.used ||
                client->rxBytes() < ctx.observedRxBytes ||
                client->txBytes() < ctx.observedTxBytes)
            {
                resetContext(ctx, true);
            }
            updateClient(slot, *client);
            ctx.observedRxBytes = client->rxBytes();
            ctx.observedTxBytes = client->txBytes();
        }
        else if (ctx.used)
        {
            resetContext(ctx, false);
            _events |= FLPROG_HTTP_SERVER_EVENT_CLIENT_CLOSED;
        }
    }

    // Legacy AP+STA second listener uses the extra context immediately after
    // the primary client range. Each TCP server has maxClients=1 in this path.
    if (_legacySecondaryTcpServer != nullptr && _clientCapacity > _maxClients)
    {
        FLProgNetworkTcpServerClient *client = _legacySecondaryTcpServer->client(0);
        ClientContext &ctx = _clients[_maxClients];
        if (client != nullptr && client->active())
        {
            if (!ctx.used ||
                client->rxBytes() < ctx.observedRxBytes ||
                client->txBytes() < ctx.observedTxBytes)
            {
                resetContext(ctx, true);
            }
            updateClient(_maxClients, *client);
            ctx.observedRxBytes = client->rxBytes();
            ctx.observedTxBytes = client->txBytes();
        }
        else if (ctx.used)
        {
            resetContext(ctx, false);
            _events |= FLPROG_HTTP_SERVER_EVENT_CLIENT_CLOSED;
        }
    }
}

void FLProgNetworkHttpServer::updateClient(
    uint8_t slot,
    FLProgNetworkTcpServerClient &client)
{
    ClientContext &ctx = _clients[slot];

    if (ctx.state == FLPROG_HTTP_SERVER_STATE_SENDING)
    {
        // Bound a stalled response too. An error response cannot be queued on
        // a blocked writer; close the client so the shared STA/AP slot recovers.
        if (ctx.responseOffset < ctx.responseLength && _requestTimeoutMs != 0 &&
            (uint32_t)(millis() - ctx.lastActivityAt) >= _requestTimeoutMs)
        {
            _timeoutCount++;
            _events |= FLPROG_HTTP_SERVER_EVENT_TIMEOUT;
            _error = FLPROG_HTTP_SERVER_ERROR_TIMEOUT;
            ctx.error = FLPROG_HTTP_SERVER_ERROR_TIMEOUT;
            client.stop();
            ctx.state = FLPROG_HTTP_SERVER_STATE_CLOSING;
            return;
        }
        flushResponse(slot, client, ctx);
        return;
    }
    if (ctx.state == FLPROG_HTTP_SERVER_STATE_CLOSING || ctx.state == FLPROG_HTTP_SERVER_STATE_ERROR)
    {
        return;
    }

    if (
        _requestTimeoutMs != 0 &&
        (uint32_t)(millis() - ctx.lastActivityAt) >= _requestTimeoutMs)
    {
        _timeoutCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_TIMEOUT;
        queueProtocolError(slot, ctx, 408, FLPROG_HTTP_SERVER_ERROR_TIMEOUT);
        return;
    }

    _legacyProcessingClient = &client;
    processInput(slot, client, ctx);
    _legacyProcessingClient = nullptr;
}

void FLProgNetworkHttpServer::processInput(
    uint8_t slot,
    FLProgNetworkTcpServerClient &client,
    ClientContext &ctx)
{
    uint16_t work = 0;
    while (
        work < _readBudget &&
        client.available() > 0 &&
        ctx.state != FLPROG_HTTP_SERVER_STATE_SENDING &&
        ctx.state != FLPROG_HTTP_SERVER_STATE_CLOSING &&
        ctx.state != FLPROG_HTTP_SERVER_STATE_ERROR)
    {
        const int value = client.read();
        if (value < 0) break;
        ctx.lastActivityAt = millis();
        ++work;
        if (!feedByte(slot, ctx, (uint8_t)value)) break;
    }
}

bool FLProgNetworkHttpServer::feedByte(uint8_t slot, ClientContext &ctx, uint8_t value)
{
    if (ctx.state == FLPROG_HTTP_SERVER_STATE_BODY)
    {
        if (ctx.bodyReceived >= ctx.contentLength)
        {
            dispatch(slot, ctx);
            return true;
        }
        if (ctx.bodyReceived >= FLPROG_HTTP_SERVER_MAX_BODY_BYTES)
        {
            queueProtocolError(slot, ctx, 413, FLPROG_HTTP_SERVER_ERROR_BODY_TOO_LARGE);
            return false;
        }
        ctx.body[ctx.bodyReceived++] = value;
        if (ctx.bodyReceived == ctx.contentLength)
        {
            ctx.body[ctx.bodyReceived] = 0;
            dispatch(slot, ctx);
        }
        return true;
    }

    if (ctx.state != FLPROG_HTTP_SERVER_STATE_REQUEST_LINE &&
        ctx.state != FLPROG_HTTP_SERVER_STATE_HEADERS)
    {
        return true;
    }

    if (ctx.state == FLPROG_HTTP_SERVER_STATE_HEADERS)
    {
        ++ctx.headerBytes;
        if (ctx.headerBytes > FLPROG_HTTP_SERVER_MAX_HEADER_BYTES)
        {
            queueProtocolError(slot, ctx, 431, FLPROG_HTTP_SERVER_ERROR_HEADERS_TOO_LARGE);
            return false;
        }
    }

    if (value == '\r') return true;
    if (value == '\n')
    {
        ctx.line[ctx.lineLength] = 0;
        const bool result = consumeLine(slot, ctx);
        ctx.lineLength = 0;
        if (ctx.responseStructured != 2) ctx.line[0] = 0;
        return result;
    }

    if (ctx.lineLength >= FLPROG_HTTP_SERVER_MAX_REQUEST_LINE)
    {
        queueProtocolError(slot, ctx, 414, FLPROG_HTTP_SERVER_ERROR_LINE_TOO_LONG);
        return false;
    }
    ctx.line[ctx.lineLength++] = (char)value;
    return true;
}

bool FLProgNetworkHttpServer::consumeLine(uint8_t slot, ClientContext &ctx)
{
    if (ctx.state == FLPROG_HTTP_SERVER_STATE_REQUEST_LINE)
    {
        if (!parseRequestLine(ctx))
        {
            queueProtocolError(slot, ctx, 400, FLPROG_HTTP_SERVER_ERROR_REQUEST_LINE);
            return false;
        }
        ctx.state = FLPROG_HTTP_SERVER_STATE_HEADERS;
        return true;
    }

    if (ctx.state == FLPROG_HTTP_SERVER_STATE_HEADERS)
    {
        if (ctx.lineLength == 0)
        {
            return finishHeaders(slot, ctx);
        }
        if (!parseHeaderLine(ctx))
        {
            if (ctx.error == FLPROG_HTTP_SERVER_ERROR_UNSUPPORTED_TRANSFER)
            {
                _rejectedBodyCount++;
                _events |= FLPROG_HTTP_SERVER_EVENT_BODY_REJECTED;
                queueProtocolError(slot, ctx, 501, ctx.error);
            }
            else if (ctx.error == FLPROG_HTTP_SERVER_ERROR_TOO_MANY_HEADERS)
            {
                queueProtocolError(slot, ctx, 431, ctx.error);
            }
            else
            {
                const uint8_t errorCode = ctx.error == FLPROG_HTTP_SERVER_ERROR_BAD_LENGTH
                    ? FLPROG_HTTP_SERVER_ERROR_BAD_LENGTH
                    : FLPROG_HTTP_SERVER_ERROR_BAD_HEADER;
                queueProtocolError(slot, ctx, 400, errorCode);
            }
            return false;
        }
    }
    return true;
}

bool FLProgNetworkHttpServer::parseRequestLine(ClientContext &ctx)
{
    char *firstSpace = std::strchr(ctx.line, ' ');
    if (firstSpace == nullptr) return false;
    *firstSpace = 0;
    char *target = firstSpace + 1;
    char *secondSpace = std::strchr(target, ' ');
    if (secondSpace == nullptr) return false;
    *secondSpace = 0;
    const char *version = secondSpace + 1;

    // A request line has exactly one HTTP-version token. Do not let atoi()
    // silently accept a valid prefix such as "HTTP/1.1 trailing-data": that
    // would make malformed requests indistinguishable from HTTP/1.1.
    if (std::strncmp(version, "HTTP/", 5) != 0 ||
        version[5] < '0' || version[5] > '9' ||
        version[6] != '.' ||
        version[7] < '0' || version[7] > '9' ||
        version[8] != 0)
    {
        return false;
    }
    const int major = version[5] - '0';
    const int minor = version[7] - '0';

    copyBounded(ctx.methodName, sizeof(ctx.methodName), ctx.line);
    ctx.method = getHttpMethodCode(ctx.methodName);
    ctx.httpMajor = (uint8_t)major;
    ctx.httpMinor = (uint8_t)minor;

    const size_t targetLength = std::strlen(target);
    if (targetLength == 0 || targetLength > FLPROG_HTTP_SERVER_MAX_URI_LENGTH) return false;
    char targetCopy[FLPROG_HTTP_SERVER_MAX_URI_LENGTH + 1];
    copyBounded(targetCopy, sizeof(targetCopy), target);
    char *query = std::strchr(targetCopy, '?');
    if (query != nullptr)
    {
        *query++ = 0;
        if (!parseArguments(ctx, query)) return false;
    }
    if (targetCopy[0] == 0) copyBounded(ctx.uri, sizeof(ctx.uri), "/");
    else copyBounded(ctx.uri, sizeof(ctx.uri), targetCopy);
    return true;
}

bool FLProgNetworkHttpServer::parseHeaderLine(ClientContext &ctx)
{
    char *colon = std::strchr(ctx.line, ':');
    if (colon == nullptr)
    {
        ctx.error = FLPROG_HTTP_SERVER_ERROR_BAD_HEADER;
        return false;
    }
    *colon = 0;
    char *value = colon + 1;
    while (*value == ' ' || *value == '\t') ++value;

    size_t keyLength = std::strlen(ctx.line);
    while (keyLength > 0 && (ctx.line[keyLength - 1] == ' ' || ctx.line[keyLength - 1] == '\t'))
        ctx.line[--keyLength] = 0;

    size_t valueLength = std::strlen(value);
    while (valueLength > 0 && (value[valueLength - 1] == ' ' || value[valueLength - 1] == '\t'))
        value[--valueLength] = 0;

    if (keyLength == 0)
    {
        ctx.error = FLPROG_HTTP_SERVER_ERROR_BAD_HEADER;
        return false;
    }

    const bool isHost = equalsIgnoreCase(ctx.line, "Host");
    const bool isContentType = equalsIgnoreCase(ctx.line, "Content-Type");
    const bool isContentLength = equalsIgnoreCase(ctx.line, "Content-Length");
    const bool isTransferEncoding = equalsIgnoreCase(ctx.line, "Transfer-Encoding");
    const bool isProtocolCritical =
        isHost || isContentType || isContentLength || isTransferEncoding;

    // WS03B legacy browser compatibility:
    //
    // Modern Chromium/Edge sends many request headers and some values (most
    // notably Accept/User-Agent/client-hint headers) can exceed the bounded
    // storage limits chosen for ESP8266. Historical FLProgWebServer did not
    // require every browser header to be retained in RAM.
    //
    // Therefore, in legacy FLProgWebServer(source, port) mode:
    //   * protocol-critical headers are always parsed below;
    //   * optional headers are stored only while they fit the bounded table;
    //   * oversized or surplus OPTIONAL headers are ignored, not rejected.
    //
    // Native FLProgNetworkHttpServer mode remains strict and unchanged.
    if (!_legacyAutoStart && !_browserHeaderCompatibility)
    {
        if (keyLength > FLPROG_HTTP_SERVER_MAX_HEADER_KEY_LENGTH ||
            valueLength > FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH)
        {
            ctx.error = FLPROG_HTTP_SERVER_ERROR_BAD_HEADER;
            return false;
        }
        if (ctx.headerCount >= FLPROG_HTTP_SERVER_MAX_HEADERS)
        {
            ctx.error = FLPROG_HTTP_SERVER_ERROR_TOO_MANY_HEADERS;
            return false;
        }
    }

    // Parse headers that affect protocol/body handling even if the bounded
    // legacy header table is already full.
    if (isHost)
    {
        copyBounded(ctx.host, sizeof(ctx.host), value);
    }
    else if (isContentType)
    {
        copyBounded(ctx.contentType, sizeof(ctx.contentType), value);
    }
    else if (isContentLength)
    {
        uint32_t length = 0;
        if (!parseUnsigned(value, length) ||
            (ctx.contentLengthSeen && ctx.contentLength != length))
        {
            ctx.error = FLPROG_HTTP_SERVER_ERROR_BAD_LENGTH;
            return false;
        }
        ctx.contentLength = length;
        ctx.contentLengthSeen = true;
    }
    else if (isTransferEncoding)
    {
        if (!equalsIgnoreCase(value, "identity"))
        {
            ctx.error = FLPROG_HTTP_SERVER_ERROR_UNSUPPORTED_TRANSFER;
            return false;
        }
    }

    // Bounded header retention.
    //
    // In legacy mode an optional header that does not fit is deliberately
    // skipped to preserve RAM and old-browser compatibility. Critical fields
    // above have already been interpreted even if they cannot be retained.
    const bool fitsStorage =
        keyLength <= FLPROG_HTTP_SERVER_MAX_HEADER_KEY_LENGTH &&
        valueLength <= FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH &&
        ctx.headerCount < FLPROG_HTTP_SERVER_MAX_HEADERS;

    if (fitsStorage)
    {
        FLProgNetworkHttpServerPair &pair = ctx.headers[ctx.headerCount++];
        copyBounded(pair.key, sizeof(pair.key), ctx.line);
        copyBounded(pair.value, sizeof(pair.value), value);
    }
    else if (equalsIgnoreCase(ctx.line, "Accept-Encoding"))
    {
        // Keep content-negotiation even after Chromium fills the optional
        // table. Reuse the last slot; no extra per-client RAM is allocated.
        const uint8_t slot = ctx.headerCount < FLPROG_HTTP_SERVER_MAX_HEADERS ? ctx.headerCount++ : FLPROG_HTTP_SERVER_MAX_HEADERS - 1;
        FLProgNetworkHttpServerPair &pair = ctx.headers[slot];
        copyBounded(pair.key, sizeof(pair.key), ctx.line);
        copyBounded(pair.value, sizeof(pair.value),
            valueLength <= FLPROG_HTTP_SERVER_MAX_HEADER_VALUE_LENGTH ? value : "identity");
    }
    else if (!_legacyAutoStart && !_browserHeaderCompatibility)
    {
        // Defensive fallback; native strict checks above should already have
        // returned the precise error.
        ctx.error = ctx.headerCount >= FLPROG_HTTP_SERVER_MAX_HEADERS
            ? FLPROG_HTTP_SERVER_ERROR_TOO_MANY_HEADERS
            : FLPROG_HTTP_SERVER_ERROR_BAD_HEADER;
        return false;
    }
    else if (isProtocolCritical)
    {
        // Critical value was successfully interpreted above. Not retaining
        // it in the optional header table is acceptable for the legacy path.
    }

    return true;
}

bool FLProgNetworkHttpServer::finishHeaders(uint8_t slot, ClientContext &ctx)
{
    if (ctx.error == FLPROG_HTTP_SERVER_ERROR_UNSUPPORTED_TRANSFER)
    {
        _rejectedBodyCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_BODY_REJECTED;
        queueProtocolError(slot, ctx, 501, ctx.error);
        return false;
    }
    if (ctx.contentLength > FLPROG_HTTP_SERVER_MAX_BODY_BYTES)
    {
        _rejectedBodyCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_BODY_REJECTED;
        queueProtocolError(slot, ctx, 413, FLPROG_HTTP_SERVER_ERROR_BODY_TOO_LARGE);
        return false;
    }
    if (ctx.contentLength != 0)
    {
        ctx.state = FLPROG_HTTP_SERVER_STATE_BODY;
        return true;
    }
    dispatch(slot, ctx);
    return true;
}

void FLProgNetworkHttpServer::urlDecodeInto(
    const char *source,
    size_t length,
    char *destination,
    size_t capacity) const
{
    if (destination == nullptr || capacity == 0) return;
    size_t out = 0;
    for (size_t i = 0; i < length && out + 1 < capacity; ++i)
    {
        char value = source[i];
        if (value == '+' ) value = ' ';
        else if (value == '%' && i + 2 < length)
        {
            const int high = hexValue(source[i + 1]);
            const int low = hexValue(source[i + 2]);
            if (high >= 0 && low >= 0)
            {
                value = (char)((high << 4) | low);
                i += 2;
            }
        }
        destination[out++] = value;
    }
    destination[out] = 0;
}

bool FLProgNetworkHttpServer::parseArguments(ClientContext &ctx, const char *query)
{
    if (query == nullptr || *query == 0) return true;
    // Embedded NUL cannot be represented by the fixed C-string arguments.
    // Reject it before decoding instead of accepting a truncated value.
    for (const char *p = query; *p; ++p)
        if (p[0] == '%' && p[1] == '0' && p[2] == '0') {
            ctx.error = FLPROG_HTTP_SERVER_ERROR_REQUEST_LINE;
            return false;
        }
    const char *cursor = query;
    while (*cursor != 0)
    {
        const char *amp = std::strchr(cursor, '&');
        const char *end = amp == nullptr ? cursor + std::strlen(cursor) : amp;
        const char *equal = nullptr;
        for (const char *p = cursor; p < end; ++p)
        {
            if (*p == '=') { equal = p; break; }
        }
        if (equal != nullptr)
        {
            if (ctx.argumentCount >= FLPROG_HTTP_SERVER_MAX_ARGUMENTS)
            {
                ctx.error = FLPROG_HTTP_SERVER_ERROR_TOO_MANY_ARGUMENTS;
                return false;
            }
            FLProgNetworkHttpServerArgument &arg = ctx.arguments[ctx.argumentCount++];
            urlDecodeInto(cursor, (size_t)(equal - cursor), arg.key, sizeof(arg.key));
            urlDecodeInto(equal + 1, (size_t)(end - equal - 1), arg.value, sizeof(arg.value));
        }
        if (amp == nullptr) break;
        cursor = amp + 1;
    }
    return true;
}

void FLProgNetworkHttpServer::dispatch(uint8_t slot, ClientContext &ctx)
{
    ctx.state = FLPROG_HTTP_SERVER_STATE_DISPATCH;
    _requestCount++;
    _events |= FLPROG_HTTP_SERVER_EVENT_REQUEST;
    ctx.responseLength = 0;
    ctx.responseOffset = 0;
    ctx.responseOverflow = false;
    ctx.responseStructured = false;
    _currentSlot = (int8_t)slot;

    // WS03A legacy mode: old FLProgWebServer callbacks wrote through a small
    // rolling buffer directly to the accepted TCP connection.
    _legacyDirectClient = _legacyAutoStart ? _legacyProcessingClient : nullptr;
    _legacyDirectResponse = false;
    _legacyDirectFailed = false;
    _legacyDirectBytes = 0;

    FLProgNetworkHttpServerCallback callback = nullptr;
    for (uint8_t i = 0; i < _handlerCount; ++i)
    {
        if (_handlers[i].method == ctx.method && equalsIgnoreCase(_handlers[i].uri, ctx.uri))
        {
            callback = _handlers[i].callback;
            break;
        }
    }

    if (callback != nullptr)
    {
        callback(this);
    }
    else
    {
        _notFoundCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_NOT_FOUND;
        if (_notFoundCallback != nullptr) _notFoundCallback(this);
        else sendDefault404Page();
    }

    // If legacy print()/println() streamed the response directly, it is
    // already complete. Do not force it back through the modern 2-KB buffer.
    if (_legacyAutoStart && _legacyDirectResponse)
    {
        if (_legacyDirectClient != nullptr)
        {
            _legacyDirectClient->flush();
            _legacyDirectClient->stop();
        }
        _legacyDirectClient = nullptr;
        _currentSlot = -1;
        ctx.state = FLPROG_HTTP_SERVER_STATE_CLOSING;
        _responseCount++;
        return;
    }

    _legacyDirectClient = nullptr;

    if (ctx.responseOverflow)
    {
        _responseOverflowCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_OVERFLOW;
        ctx.responseLength = 0;
        ctx.responseOffset = 0;
        ctx.responseOverflow = false;
        (void)buildStructuredResponse(ctx, 500, "text/plain", "response too large", 18, false);
        _error = FLPROG_HTTP_SERVER_ERROR_RESPONSE_TOO_LARGE;
    }
    else if (ctx.responseLength == 0)
    {
        (void)buildStructuredResponse(ctx, 200, "text/plain", "", 0, false);
    }

    _currentSlot = -1;
    ctx.lastActivityAt = millis();
    ctx.state = FLPROG_HTTP_SERVER_STATE_SENDING;
}

void FLProgNetworkHttpServer::queueProtocolError(
    uint8_t,
    ClientContext &ctx,
    int statusCode,
    uint8_t errorCode)
{
    _error = errorCode;
    ctx.error = errorCode;
    _badRequestCount++;
    _events |= FLPROG_HTTP_SERVER_EVENT_BAD_REQUEST;
    ctx.responseLength = 0;
    ctx.responseOffset = 0;
    ctx.responseOverflow = false;
    const char *reason = reasonPhrase(statusCode);
    (void)buildStructuredResponse(ctx, statusCode, "text/plain", reason, (uint32_t)std::strlen(reason), false);
    // Give even a 408 response its own write-progress window.
    ctx.lastActivityAt = millis();
    ctx.state = FLPROG_HTTP_SERVER_STATE_SENDING;
}

void FLProgNetworkHttpServer::flushResponse(
    uint8_t,
    FLProgNetworkTcpServerClient &client,
    ClientContext &ctx)
{
    if (ctx.responseOffset >= ctx.responseLength && ctx.responseStructured == 2 &&
        ctx.flash.offset < ctx.flash.length)
    {
        // Refill only the existing queue, and perform at most one bounded
        // write per pool iteration. Partial/zero writes keep the same chunk.
        size_t amount = ctx.flash.length - ctx.flash.offset;
        if (amount > _writeBudget) amount = _writeBudget;
        if (amount > 256U) amount = 256U;
        if (amount > FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES) amount = FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES;
        ctx.responseLength = 0;
        ctx.responseOffset = 0;
        if (!appendResponseProgmem(ctx, reinterpret_cast<const char *>(ctx.flash.data + ctx.flash.offset), amount))
        {
            client.stop(); ctx.state = FLPROG_HTTP_SERVER_STATE_CLOSING; return;
        }
        ctx.flash.offset += amount;
    }
    if (ctx.responseOffset >= ctx.responseLength)
    {
        client.stop();
        ctx.state = FLPROG_HTTP_SERVER_STATE_CLOSING;
        _responseCount++;
        _events |= FLPROG_HTTP_SERVER_EVENT_RESPONSE;
        return;
    }

    size_t remaining = (size_t)(ctx.responseLength - ctx.responseOffset);
    if (remaining > _writeBudget) remaining = _writeBudget;
    const size_t written = client.write(
        reinterpret_cast<const uint8_t *>(ctx.response + ctx.responseOffset),
        remaining);
    if (written > 0)
    {
        ctx.responseOffset = (uint16_t)(ctx.responseOffset + written);
        ctx.lastActivityAt = millis();
    }
}

bool FLProgNetworkHttpServer::appendResponse(
    ClientContext &ctx,
    const uint8_t *data,
    size_t length)
{
    if (data == nullptr || length == 0) return true;
    if ((size_t)ctx.responseLength + length > FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES)
    {
        ctx.responseOverflow = true;
        return false;
    }
    std::memcpy(ctx.response + ctx.responseLength, data, length);
    ctx.responseLength = (uint16_t)(ctx.responseLength + length);
    ctx.response[ctx.responseLength] = 0;
    return true;
}

bool FLProgNetworkHttpServer::appendResponseProgmem(
    ClientContext &ctx,
    const char *data,
    size_t length)
{
    if (data == nullptr || length == 0) return true;
    if ((size_t)ctx.responseLength + length > FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES)
    {
        ctx.responseOverflow = true;
        return false;
    }
#if defined(ESP8266)
    for (size_t index = 0; index < length; ++index)
    {
        ctx.response[ctx.responseLength + index] = (char)pgm_read_byte(data + index);
    }
#else
    std::memcpy(ctx.response + ctx.responseLength, data, length);
#endif
    ctx.responseLength = (uint16_t)(ctx.responseLength + length);
    ctx.response[ctx.responseLength] = 0;
    return true;
}

bool FLProgNetworkHttpServer::buildStructuredResponse(
    ClientContext &ctx,
    int statusCode,
    const char *contentTypeValue,
    const char *bodyValue,
    uint32_t bodyLengthValue,
    bool headerOnly)
{
    if (contentTypeValue == nullptr || contentTypeValue[0] == 0) contentTypeValue = "text/plain";
    if (bodyValue == nullptr) { bodyValue = ""; bodyLengthValue = 0; }

    ctx.responseLength = 0;
    ctx.responseOffset = 0;
    ctx.responseOverflow = false;
    ctx.responseStructured = true;

    char header[320];
    const int headerLength = std::snprintf(
        header,
        sizeof(header),
        "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %lu\r\nConnection: close\r\n\r\n",
        statusCode,
        reasonPhrase(statusCode),
        contentTypeValue,
        (unsigned long)bodyLengthValue);
    if (headerLength <= 0 || (size_t)headerLength >= sizeof(header))
    {
        ctx.responseOverflow = true;
        return false;
    }
    if (!appendResponse(ctx, reinterpret_cast<const uint8_t *>(header), (size_t)headerLength)) return false;
    if (!headerOnly && bodyLengthValue != 0)
    {
        if (!appendResponse(ctx, reinterpret_cast<const uint8_t *>(bodyValue), bodyLengthValue)) return false;
    }
    return true;
}

bool FLProgNetworkHttpServer::buildStructuredResponseProgmem(
    ClientContext &ctx,
    int statusCode,
    const char *contentTypeValue,
    const char *bodyValue,
    uint32_t bodyLengthValue,
    bool headerOnly)
{
    if (contentTypeValue == nullptr || contentTypeValue[0] == 0) contentTypeValue = "text/plain";
    if (bodyValue == nullptr) { bodyValue = ""; bodyLengthValue = 0; }

    ctx.responseLength = 0;
    ctx.responseOffset = 0;
    ctx.responseOverflow = false;
    ctx.responseStructured = true;

    char header[320];
    const int headerLength = std::snprintf(
        header,
        sizeof(header),
        "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %lu\r\nConnection: close\r\n\r\n",
        statusCode,
        reasonPhrase(statusCode),
        contentTypeValue,
        (unsigned long)bodyLengthValue);
    if (headerLength <= 0 || (size_t)headerLength >= sizeof(header))
    {
        ctx.responseOverflow = true;
        return false;
    }
    if (!appendResponse(ctx, reinterpret_cast<const uint8_t *>(header), (size_t)headerLength)) return false;
    if (!headerOnly && bodyLengthValue != 0)
    {
        if (!appendResponseProgmem(ctx, bodyValue, bodyLengthValue)) return false;
    }
    return true;
}

FLProgNetworkHttpServer::ClientContext *FLProgNetworkHttpServer::currentContext()
{
    if (_clients == nullptr || _currentSlot < 0 || _currentSlot >= (int8_t)_clientCapacity) return nullptr;
    return &_clients[(uint8_t)_currentSlot];
}

const FLProgNetworkHttpServer::ClientContext *FLProgNetworkHttpServer::currentContext() const
{
    if (_clients == nullptr || _currentSlot < 0 || _currentSlot >= (int8_t)_clientCapacity) return nullptr;
    return &_clients[(uint8_t)_currentSlot];
}

uint8_t FLProgNetworkHttpServer::method() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? FLPROG_WEB_SERVER_UNKNOWN : ctx->method;
}

const char *FLProgNetworkHttpServer::methodName() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? "" : ctx->methodName;
}

String FLProgNetworkHttpServer::uri() const
{
    const ClientContext *ctx = currentContext();
    return String(ctx == nullptr ? "" : ctx->uri);
}

uint8_t FLProgNetworkHttpServer::methodVersion() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->httpMinor;
}

uint8_t FLProgNetworkHttpServer::httpMajor() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->httpMajor;
}

uint8_t FLProgNetworkHttpServer::httpMinor() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->httpMinor;
}

String FLProgNetworkHttpServer::host() const
{
    const ClientContext *ctx = currentContext();
    return String(ctx == nullptr ? "" : ctx->host);
}

uint16_t FLProgNetworkHttpServer::headersCount() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->headerCount;
}

String FLProgNetworkHttpServer::headerKeyAtIndex(uint16_t index) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr || index >= ctx->headerCount) return String("");
    return String(ctx->headers[index].key);
}

bool FLProgNetworkHttpServer::hasHeaderKey(const String &key) const
{
    return hasHeaderKey(key.c_str());
}

bool FLProgNetworkHttpServer::hasHeaderKey(const char *key) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr) return false;
    for (uint8_t i = 0; i < ctx->headerCount; ++i)
        if (equalsIgnoreCase(ctx->headers[i].key, key)) return true;
    return false;
}

String FLProgNetworkHttpServer::headerValueAtKey(const String &key) const
{
    return headerValueAtKey(key.c_str());
}

String FLProgNetworkHttpServer::headerValueAtKey(const char *key) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr) return String("");
    for (uint8_t i = 0; i < ctx->headerCount; ++i)
        if (equalsIgnoreCase(ctx->headers[i].key, key)) return String(ctx->headers[i].value);
    return String("");
}

uint16_t FLProgNetworkHttpServer::argumentsCount() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->argumentCount;
}

String FLProgNetworkHttpServer::argumentKeyAtIndex(uint16_t index) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr || index >= ctx->argumentCount) return String("");
    return String(ctx->arguments[index].key);
}

bool FLProgNetworkHttpServer::hasArgumentKey(const String &key) const
{
    return hasArgumentKey(key.c_str());
}

bool FLProgNetworkHttpServer::hasArgumentKey(const char *key) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr) return false;
    for (uint8_t i = 0; i < ctx->argumentCount; ++i)
        if (equalsIgnoreCase(ctx->arguments[i].key, key)) return true;
    return false;
}

String FLProgNetworkHttpServer::argumentValueAtKey(const String &key) const
{
    return argumentValueAtKey(key.c_str());
}

String FLProgNetworkHttpServer::argumentValueAtKey(const char *key) const
{
    const ClientContext *ctx = currentContext();
    if (ctx == nullptr) return String("");
    for (uint8_t i = 0; i < ctx->argumentCount; ++i)
        if (equalsIgnoreCase(ctx->arguments[i].key, key)) return String(ctx->arguments[i].value);
    return String("");
}

const uint8_t *FLProgNetworkHttpServer::bodyData() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? nullptr : ctx->body;
}

const char *FLProgNetworkHttpServer::body() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? "" : reinterpret_cast<const char *>(ctx->body);
}

uint32_t FLProgNetworkHttpServer::bodyLength() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->bodyReceived;
}

uint32_t FLProgNetworkHttpServer::contentLength() const
{
    const ClientContext *ctx = currentContext();
    return ctx == nullptr ? 0 : ctx->contentLength;
}

String FLProgNetworkHttpServer::contentType() const
{
    const ClientContext *ctx = currentContext();
    return String(ctx == nullptr ? "" : ctx->contentType);
}

bool FLProgNetworkHttpServer::send(
    int statusCode,
    const char *contentTypeValue,
    const char *bodyValue)
{
    ClientContext *ctx = currentContext();
    if (ctx == nullptr) return false;
    const uint32_t length = bodyValue == nullptr ? 0 : (uint32_t)std::strlen(bodyValue);
    const bool headRequest = ctx->method == FLPROG_WEB_SERVER_HEAD;
    return buildStructuredResponse(*ctx, statusCode, contentTypeValue, bodyValue, length, headRequest);
}

bool FLProgNetworkHttpServer::sendProgmem(
    int statusCode,
    const char *contentTypeValue,
    const char *bodyValue)
{
    ClientContext *ctx = currentContext();
    if (ctx == nullptr) return false;
#if defined(ESP8266)
    const uint32_t length = bodyValue == nullptr ? 0 : (uint32_t)strlen_P(bodyValue);
#else
    const uint32_t length = bodyValue == nullptr ? 0 : (uint32_t)std::strlen(bodyValue);
#endif
    const bool headRequest = ctx->method == FLPROG_WEB_SERVER_HEAD;
    return buildStructuredResponseProgmem(*ctx, statusCode, contentTypeValue, bodyValue, length, headRequest);
}

bool FLProgNetworkHttpServer::sendStreamProgmem(
    int statusCode, const char *type, const void *data, uint32_t length,
    const char *encoding, bool cacheImmutable)
{
    ClientContext *ctx = currentContext();
    if (!ctx || _legacyAutoStart || (length && !data)) return false;
    if (encoding && std::strcmp(encoding, "gzip") != 0) return false;
    if (!type || !*type) type = "application/octet-stream";
    ctx->responseLength = ctx->responseOffset = 0;
    ctx->responseOverflow = false;
    ctx->responseStructured = 1;
    const size_t headerCapacity = sizeof(ctx->response) < 320U ? sizeof(ctx->response) : 320U;
    const int count = std::snprintf(ctx->response, headerCapacity,
        "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %lu\r\n%sCache-Control: %s\r\nConnection: close\r\n\r\n",
        statusCode, reasonPhrase(statusCode), type, (unsigned long)length,
        encoding ? "Content-Encoding: gzip\r\nVary: Accept-Encoding\r\n" : "",
        cacheImmutable ? "public, max-age=31536000, immutable" : "no-store");
    if (count <= 0 || (size_t)count >= headerCapacity ||
        (size_t)count > FLPROG_HTTP_SERVER_MAX_RESPONSE_BYTES) return false;
    ctx->responseLength = (uint16_t)count;
    if (ctx->method != FLPROG_WEB_SERVER_HEAD && length) {
        ctx->flash.data = static_cast<const uint8_t *>(data);
        ctx->flash.length = length; ctx->flash.offset = 0;
        ctx->responseStructured = 2;
    }
    return true;
}

bool FLProgNetworkHttpServer::sendHeaderOnly(
    int statusCode,
    const char *contentTypeValue,
    uint32_t contentLengthValue)
{
    ClientContext *ctx = currentContext();
    if (ctx == nullptr) return false;
    return buildStructuredResponse(*ctx, statusCode, contentTypeValue, "", contentLengthValue, true);
}

void FLProgNetworkHttpServer::sendDefault200Page()
{
    if (_legacyAutoStart && _legacyDirectClient != nullptr)
    {
        print("HTTP/1.1 200 OK\r\n");
        print("Content-Type: text/html\r\n");
        print("Connection: close\r\n\r\n");
        return;
    }
    (void)send(200, "text/html", "");
}

void FLProgNetworkHttpServer::sendDefault404Page()
{
    if (_legacyAutoStart && _legacyDirectClient != nullptr)
    {
        print("HTTP/1.1 404 Not Found\r\n");
        print("Content-Type: text/html\r\n");
        print("Connection: close\r\n\r\n");
        return;
    }
    (void)send(404, "text/html", "");
}

void FLProgNetworkHttpServer::send403Page(String value)
{
    if (_legacyAutoStart && _legacyDirectClient != nullptr)
    {
        print("HTTP/1.1 403 Forbidden\r\n");
        print("Content-Type: text/htm\r\n");
        print("Connection: close\r\n\r\n");
        print(value);
        println();
        return;
    }
    (void)send(403, "text/html", value.c_str());
}

void FLProgNetworkHttpServer::sendJson(String value)
{
    if (_legacyAutoStart && _legacyDirectClient != nullptr)
    {
        print("HTTP/1.1 200 OK\r\n");
        print("Content-Type: text/json\r\n");
        print("Connection: close\r\n\r\n");
        print(value);
        println();
        return;
    }
    (void)send(200, "text/json", value.c_str());
}

size_t FLProgNetworkHttpServer::legacyWriteDirect(
    const uint8_t *data,
    size_t length)
{
    if (_legacyDirectClient == nullptr || data == nullptr || length == 0 || _legacyDirectFailed)
    {
        return 0;
    }

    const size_t maxChunk = 256U;

    // WEB-PORT01 accepted compatibility fix.
    // FLProg 9.x generated page callbacks often start by printing the HTML
    // body directly ("<!DOCTYPE ...") and do not emit an HTTP status line.
    // Historical clients were tolerant of that on port 80, but a proper
    // HTTP/1.1 response must not depend on the selected TCP port.
    //
    // Preserve callbacks which explicitly emit their own response headers
    // (sendDefault200Page(), sendJson(), send403Page(), ...). For a direct
    // legacy response whose FIRST write is not "HTTP/", prepend a minimal
    // 200 response header before streaming the generated page body.
    if (_legacyDirectBytes == 0)
    {
        const bool explicitHttpHeader =
            length >= 5 &&
            data[0] == 'H' &&
            data[1] == 'T' &&
            data[2] == 'T' &&
            data[3] == 'P' &&
            data[4] == '/';

        if (!explicitHttpHeader)
        {
            static const char implicitHeader[] =
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: text/html; charset=utf-8\r\n"
                "Connection: close\r\n\r\n";

            const uint8_t *headerData =
                reinterpret_cast<const uint8_t *>(implicitHeader);
            const size_t headerLength = sizeof(implicitHeader) - 1U;
            size_t headerOffset = 0;
            uint32_t headerWaitStarted = millis();

            while (headerOffset < headerLength)
            {
                size_t chunk = headerLength - headerOffset;
                if (chunk > maxChunk) chunk = maxChunk;

                const size_t written =
                    _legacyDirectClient->write(headerData + headerOffset, chunk);
                if (written > 0)
                {
                    headerOffset += written;
                    headerWaitStarted = millis();
                    _legacyDirectResponse = true;
                    _legacyDirectBytes += (uint32_t)written;
                    continue;
                }

                _legacyDirectClient->pool();
                yield();

                if (!_legacyDirectClient->connected() ||
                    (uint32_t)(millis() - headerWaitStarted) >=
                        FLPROG_HTTP_LEGACY_DIRECT_STALL_MS)
                {
                    _legacyDirectFailed = true;
                    _error = FLPROG_HTTP_SERVER_ERROR_TCP_SERVER;
                    return 0;
                }
            }
        }
    }
    size_t offset = 0;
    uint32_t waitStarted = millis();

    while (offset < length)
    {
        size_t chunk = length - offset;
        if (chunk > maxChunk) chunk = maxChunk;

        const size_t written = _legacyDirectClient->write(data + offset, chunk);
        if (written > 0)
        {
            offset += written;
            waitStarted = millis();
            _legacyDirectResponse = true;
            _legacyDirectBytes += (uint32_t)written;
            continue;
        }

        // W5500 can have SEND pending briefly. Poll cooperatively, but never
        // keep the whole FLProg loop blocked for seconds when the browser
        // abandons a refresh. The previous 3-second wait was repeated by many
        // generated print() calls and could stall both HTTP services.
        _legacyDirectClient->pool();
        yield();

        if (!_legacyDirectClient->connected())
        {
            _legacyDirectFailed = true;
            _error = FLPROG_HTTP_SERVER_ERROR_TCP_SERVER;
            break;
        }

        if ((uint32_t)(millis() - waitStarted) >= FLPROG_HTTP_LEGACY_DIRECT_STALL_MS)
        {
            _legacyDirectFailed = true;
            _error = FLPROG_HTTP_SERVER_ERROR_TCP_SERVER;
            break;
        }
    }

    return offset;
}

size_t FLProgNetworkHttpServer::write(const uint8_t *buffer, size_t size)
{
    if (_legacyAutoStart && _legacyDirectClient != nullptr)
    {
        return legacyWriteDirect(buffer, size);
    }

    ClientContext *ctx = currentContext();
    if (ctx == nullptr || buffer == nullptr || size == 0 || ctx->responseStructured == 2) return 0;
    return appendResponse(*ctx, buffer, size) ? size : 0;
}

size_t FLProgNetworkHttpServer::print(const char *value)
{
    if (value == nullptr) return 0;
    return write(reinterpret_cast<const uint8_t *>(value), std::strlen(value));
}

size_t FLProgNetworkHttpServer::print(int value)
{
    char buffer[24];
    const int count = std::snprintf(buffer, sizeof(buffer), "%d", value);
    return count > 0 ? print(buffer) : 0;
}

size_t FLProgNetworkHttpServer::print(unsigned int value)
{
    char buffer[24];
    const int count = std::snprintf(buffer, sizeof(buffer), "%u", value);
    return count > 0 ? print(buffer) : 0;
}

size_t FLProgNetworkHttpServer::print(long value)
{
    char buffer[32];
    const int count = std::snprintf(buffer, sizeof(buffer), "%ld", value);
    return count > 0 ? print(buffer) : 0;
}

size_t FLProgNetworkHttpServer::print(unsigned long value)
{
    char buffer[32];
    const int count = std::snprintf(buffer, sizeof(buffer), "%lu", value);
    return count > 0 ? print(buffer) : 0;
}

size_t FLProgNetworkHttpServer::println(const char *value)
{
    const size_t a = print(value);
    const size_t b = print("\r\n");
    return a + b;
}

size_t FLProgNetworkHttpServer::println()
{
    return print("\r\n");
}

bool FLProgNetworkHttpServer::responseQueued() const
{
    const ClientContext *ctx = currentContext();
    return ctx != nullptr && ctx->responseLength != 0;
}

bool FLProgNetworkHttpServer::responseOverflowed() const
{
    const ClientContext *ctx = currentContext();
    return ctx != nullptr && ctx->responseOverflow;
}

void FLProgNetworkHttpServer::resetStatistics()
{
    _requestCount = 0;
    _responseCount = 0;
    _notFoundCount = 0;
    _badRequestCount = 0;
    _timeoutCount = 0;
    _rejectedBodyCount = 0;
    _responseOverflowCount = 0;
}

uint8_t FLProgNetworkHttpServer::getHttpMethodCode(const char *methodValue)
{
    if (methodValue == nullptr) return FLPROG_WEB_SERVER_UNKNOWN;
    if (equalsIgnoreCase(methodValue, "DELETE")) return FLPROG_WEB_SERVER_DELETE;
    if (equalsIgnoreCase(methodValue, "GET")) return FLPROG_WEB_SERVER_GET;
    if (equalsIgnoreCase(methodValue, "HEAD")) return FLPROG_WEB_SERVER_HEAD;
    if (equalsIgnoreCase(methodValue, "POST")) return FLPROG_WEB_SERVER_POST;
    if (equalsIgnoreCase(methodValue, "PUT")) return FLPROG_WEB_SERVER_PUT;
    if (equalsIgnoreCase(methodValue, "CONNECT")) return FLPROG_WEB_SERVER_CONNECT;
    if (equalsIgnoreCase(methodValue, "OPTIONS")) return FLPROG_WEB_SERVER_OPTIONS;
    if (equalsIgnoreCase(methodValue, "TRACE")) return FLPROG_WEB_SERVER_TRACE;
    if (equalsIgnoreCase(methodValue, "COPY")) return FLPROG_WEB_SERVER_COPY;
    if (equalsIgnoreCase(methodValue, "LOCK")) return FLPROG_WEB_SERVER_LOCK;
    if (equalsIgnoreCase(methodValue, "MKCOL")) return FLPROG_WEB_SERVER_MKCOL;
    if (equalsIgnoreCase(methodValue, "MOVE")) return FLPROG_WEB_SERVER_MOVE;
    if (equalsIgnoreCase(methodValue, "PROPFIND")) return FLPROG_WEB_SERVER_PROPFIND;
    if (equalsIgnoreCase(methodValue, "PROPPATCH")) return FLPROG_WEB_SERVER_PROPPATCH;
    if (equalsIgnoreCase(methodValue, "SEARCH")) return FLPROG_WEB_SERVER_SEARCH;
    if (equalsIgnoreCase(methodValue, "UNLOCK")) return FLPROG_WEB_SERVER_UNLOCK;
    if (equalsIgnoreCase(methodValue, "BIND")) return FLPROG_WEB_SERVER_BIND;
    if (equalsIgnoreCase(methodValue, "REBIND")) return FLPROG_WEB_SERVER_REBIND;
    if (equalsIgnoreCase(methodValue, "UNBIND")) return FLPROG_WEB_SERVER_UNBIND;
    if (equalsIgnoreCase(methodValue, "ACL")) return FLPROG_WEB_SERVER_ACL;
    if (equalsIgnoreCase(methodValue, "REPORT")) return FLPROG_WEB_SERVER_REPORT;
    if (equalsIgnoreCase(methodValue, "MKACTIVITY")) return FLPROG_WEB_SERVER_MKACTIVITY;
    if (equalsIgnoreCase(methodValue, "CHECKOUT")) return FLPROG_WEB_SERVER_CHECKOUT;
    if (equalsIgnoreCase(methodValue, "MERGE")) return FLPROG_WEB_SERVER_MERGE;
    if (equalsIgnoreCase(methodValue, "MSEARCH")) return FLPROG_WEB_SERVER_MSEARCH;
    if (equalsIgnoreCase(methodValue, "NOTIFY")) return FLPROG_WEB_SERVER_NOTIFY;
    if (equalsIgnoreCase(methodValue, "SUBSCRIBE")) return FLPROG_WEB_SERVER_SUBSCRIBE;
    if (equalsIgnoreCase(methodValue, "UNSUBSCRIBE")) return FLPROG_WEB_SERVER_UNSUBSCRIBE;
    if (equalsIgnoreCase(methodValue, "PATCH")) return FLPROG_WEB_SERVER_PATCH;
    if (equalsIgnoreCase(methodValue, "PURGE")) return FLPROG_WEB_SERVER_PURGE;
    if (equalsIgnoreCase(methodValue, "MKCALENDAR")) return FLPROG_WEB_SERVER_MKCALENDAR;
    if (equalsIgnoreCase(methodValue, "LINK")) return FLPROG_WEB_SERVER_LINK;
    if (equalsIgnoreCase(methodValue, "UNLINK")) return FLPROG_WEB_SERVER_UNLINK;
    return FLPROG_WEB_SERVER_UNKNOWN;
}
