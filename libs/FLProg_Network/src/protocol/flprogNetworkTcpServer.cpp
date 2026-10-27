/*
 * FLProg_Network — flprogNetworkTcpServer.cpp
 * Назначение: Native TCP server FLProg_Network.
 *
 * Архитектурные заметки:
 * - Принимает клиентов через A3/socket backend выбранного transport.
 * - Сервер не должен владеть NetworkDevice: он получает интерфейс и route извне.
 * - Legacy FLProgEthernetServer является только фасадом над этим классом.
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
#include <new>
#include "flprogNetworkTcpServer.h"
#include "../flprogNetwork.h"

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
#include <new>
#endif

#if FLPROG_NETWORK_HAS_POSIX_TCP
#include <lwip/sockets.h>
#include <fcntl.h>
#include <errno.h>
#include <string.h>
#endif

#if FLPROG_NETWORK_HAS_POSIX_TCP
static uint32_t flprogTcpServerIpv4NetworkOrder(IPAddress ip)
{
    const uint32_t hostOrder =
        ((uint32_t)ip[0] << 24) |
        ((uint32_t)ip[1] << 16) |
        ((uint32_t)ip[2] << 8) |
        (uint32_t)ip[3];
    return htonl(hostOrder);
}

static IPAddress flprogTcpServerIpFromNetworkOrder(uint32_t value)
{
    const uint32_t hostOrder = ntohl(value);
    return IPAddress(
        (uint8_t)(hostOrder >> 24),
        (uint8_t)(hostOrder >> 16),
        (uint8_t)(hostOrder >> 8),
        (uint8_t)hostOrder);
}
#endif

FLProgNetworkTcpServerClient::FLProgNetworkTcpServerClient()
{
}

uint8_t FLProgNetworkTcpServerClient::fixedRouteForTransport(uint8_t transportIdValue)
{
    if (transportIdValue == FLPROG_TRANSPORT_WIFI)
    {
        return FLPROG_NETWORK_ROUTE_WIFI;
    }
    if (transportIdValue == FLPROG_TRANSPORT_WIFI_AP)
    {
        return FLPROG_NETWORK_ROUTE_WIFI_AP;
    }
    if (transportIdValue == FLPROG_TRANSPORT_WIZNET)
    {
        return FLPROG_NETWORK_ROUTE_WIZNET;
    }
    if (transportIdValue == FLPROG_TRANSPORT_RMII)
    {
        return FLPROG_NETWORK_ROUTE_RMII;
    }
    return FLPROG_NETWORK_ROUTE_DEFAULT;
}

bool FLProgNetworkTcpServerClient::startWiznetListener(
    FLProgNetworkInterface &network,
    uint16_t portValue,
    uint8_t slotIdValue,
    bool noDelayValue,
    bool keepAliveValue,
    uint32_t keepAliveIntervalMs)
{
    releaseSlot();

    _slotId = slotIdValue;
    _transportId = FLPROG_TRANSPORT_WIZNET;
    _used = true;
    _listener = true;
    _accepted = false;
    _pendingAccept = false;
    _cleanupRequested = false;
    _acceptedAt = 0;
    _lastActivityAt = 0;
    _rxBytes = 0;
    _txBytes = 0;

    _wiznetBackend.hardware(network.wiznetHardware());
    _wiznetBackend.setServerMode(true);
    _wiznetBackend.setNoDelayedAck(noDelayValue);
    _wiznetBackend.setKeepAlive(keepAliveValue, keepAliveIntervalMs);

    setBackend(FLPROG_TRANSPORT_WIZNET, &_wiznetBackend);
    endpoint(portValue, IPAddress(0, 0, 0, 0), 0);

    if (!FLProgNetworkSocket::begin(
            network,
            FLPROG_NETWORK_ROUTE_WIZNET,
            FLPROG_SOCKET_PROTOCOL_STREAM))
    {
        _used = false;
        _listener = false;
        return false;
    }

    localPort(portValue);
    requestOpen();
    return true;
}

bool FLProgNetworkTcpServerClient::adoptPosix(
    FLProgNetworkInterface &network,
    uint8_t transportIdValue,
    int socketFd,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue,
    uint8_t slotIdValue,
    bool noDelayValue,
    bool keepAliveValue,
    uint32_t keepAliveIdleSeconds,
    uint32_t keepAliveIntervalSeconds,
    uint8_t keepAliveProbeCount)
{
    releaseSlot();

    if (
        transportIdValue != FLPROG_TRANSPORT_WIFI &&
        transportIdValue != FLPROG_TRANSPORT_WIFI_AP &&
        transportIdValue != FLPROG_TRANSPORT_RMII)
    {
        return false;
    }

    _posixBackend.configure(transportIdValue);
    _posixBackend.setNoDelay(noDelayValue);
    _posixBackend.setKeepAlive(
        keepAliveValue,
        keepAliveIdleSeconds,
        keepAliveIntervalSeconds,
        keepAliveProbeCount);

    if (!_posixBackend.adoptAcceptedSocket(
            socketFd,
            localPortValue,
            remoteIpValue,
            remotePortValue))
    {
        return false;
    }

    _slotId = slotIdValue;
    _transportId = transportIdValue;
    _used = true;
    _listener = false;
    _accepted = false;
    _pendingAccept = true;
    _cleanupRequested = false;
    _acceptedAt = 0;
    _lastActivityAt = 0;
    _rxBytes = 0;
    _txBytes = 0;

    setBackend(transportIdValue, &_posixBackend);
    endpoint(localPortValue, remoteIpValue, remotePortValue);

    if (!FLProgNetworkSocket::begin(
            network,
            fixedRouteForTransport(transportIdValue),
            FLPROG_SOCKET_PROTOCOL_STREAM))
    {
        _posixBackend.close();
        _used = false;
        _pendingAccept = false;
        return false;
    }

    requestOpen();
    return true;
}

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
bool FLProgNetworkTcpServerClient::adoptEsp8266(
    FLProgNetworkInterface &network,
    uint8_t transportIdValue,
    const WiFiClient &client,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue,
    uint8_t slotIdValue,
    bool noDelayValue,
    bool keepAliveValue,
    uint32_t keepAliveIdleSeconds,
    uint32_t keepAliveIntervalSeconds,
    uint8_t keepAliveProbeCount)
{
    releaseSlot();

    if (
        transportIdValue != FLPROG_TRANSPORT_WIFI &&
        transportIdValue != FLPROG_TRANSPORT_WIFI_AP)
    {
        return false;
    }

    _posixBackend.configure(transportIdValue);
    _posixBackend.setNoDelay(noDelayValue);
    _posixBackend.setKeepAlive(
        keepAliveValue,
        keepAliveIdleSeconds,
        keepAliveIntervalSeconds,
        keepAliveProbeCount);

    if (!_posixBackend.adoptAcceptedClient(
            client,
            localPortValue,
            remoteIpValue,
            remotePortValue))
    {
        return false;
    }

    _slotId = slotIdValue;
    _transportId = transportIdValue;
    _used = true;
    _listener = false;
    _accepted = false;
    _pendingAccept = true;
    _cleanupRequested = false;
    _acceptedAt = 0;
    _lastActivityAt = 0;
    _rxBytes = 0;
    _txBytes = 0;

    setBackend(transportIdValue, &_posixBackend);
    endpoint(localPortValue, remoteIpValue, remotePortValue);

    if (!FLProgNetworkSocket::begin(
            network,
            fixedRouteForTransport(transportIdValue),
            FLPROG_SOCKET_PROTOCOL_STREAM))
    {
        _posixBackend.close();
        _used = false;
        _pendingAccept = false;
        return false;
    }

    requestOpen();
    return true;
}
#endif

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
bool FLProgNetworkTcpServerClient::adoptStm32(
    FLProgNetworkInterface &network,
    tcp_struct *client,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue,
    uint8_t slotIdValue,
    bool noDelayValue,
    bool keepAliveValue,
    uint32_t keepAliveIdleSeconds,
    uint32_t keepAliveIntervalSeconds,
    uint8_t keepAliveProbeCount)
{
    releaseSlot();

    _stm32Backend.setNoDelay(noDelayValue);
    _stm32Backend.setKeepAlive(
        keepAliveValue,
        keepAliveIdleSeconds,
        keepAliveIntervalSeconds,
        keepAliveProbeCount);

    if (!_stm32Backend.adoptAccepted(
            client,
            localPortValue,
            remoteIpValue,
            remotePortValue))
    {
        // From this point this method owns the accepted STM32 tcp_struct.
        FLProgStm32TcpClientBackend::destroyAccepted(client);
        return false;
    }

    _slotId = slotIdValue;
    _transportId = FLPROG_TRANSPORT_RMII;
    _used = true;
    _listener = false;
    _accepted = false;
    _pendingAccept = true;
    _cleanupRequested = false;
    _acceptedAt = 0;
    _lastActivityAt = 0;
    _rxBytes = 0;
    _txBytes = 0;

    setBackend(FLPROG_TRANSPORT_RMII, &_stm32Backend);
    endpoint(localPortValue, remoteIpValue, remotePortValue);

    if (!FLProgNetworkSocket::begin(
            network,
            FLPROG_NETWORK_ROUTE_RMII,
            FLPROG_SOCKET_PROTOCOL_STREAM))
    {
        _stm32Backend.close();
        _used = false;
        _pendingAccept = false;
        return false;
    }

    requestOpen();
    return true;
}
#endif

bool FLProgNetworkTcpServerClient::promoteAccepted()
{
    if (!_used || _accepted)
    {
        return false;
    }

    if (!connected())
    {
        return false;
    }

    _listener = false;
    _pendingAccept = false;
    _accepted = true;
    _acceptedAt = millis();
    _lastActivityAt = _acceptedAt;
    return true;
}

void FLProgNetworkTcpServerClient::requestCleanup()
{
    if (!_used || _cleanupRequested)
    {
        return;
    }

    _cleanupRequested = true;
    requestClose();
}

bool FLProgNetworkTcpServerClient::cleanupComplete() const
{
    return _cleanupRequested && state() == FLPROG_SOCKET_STATE_CLOSED;
}

void FLProgNetworkTcpServerClient::releaseSlot()
{
    if (isAttached())
    {
        FLProgNetworkSocket::end();
    }
    else
    {
        _posixBackend.close();
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
        _stm32Backend.close();
#endif
    }

    _wiznetBackend.setServerMode(false);
    _slotId = 0xFF;
    _transportId = FLPROG_TRANSPORT_NONE;
    _used = false;
    _listener = false;
    _accepted = false;
    _pendingAccept = false;
    _cleanupRequested = false;
    _acceptedAt = 0;
    _lastActivityAt = 0;
    _rxBytes = 0;
    _txBytes = 0;
}

void FLProgNetworkTcpServerClient::stop()
{
    requestCleanup();
}

int FLProgNetworkTcpServerClient::read()
{
    const int result = FLProgNetworkSocket::read();
    if (result >= 0)
    {
        _rxBytes++;
        _lastActivityAt = millis();
    }
    return result;
}

int FLProgNetworkTcpServerClient::read(uint8_t *buffer, size_t length)
{
    const int result = FLProgNetworkSocket::read(buffer, length);
    if (result > 0)
    {
        _rxBytes += (uint32_t)result;
        _lastActivityAt = millis();
    }
    return result;
}

size_t FLProgNetworkTcpServerClient::write(uint8_t value)
{
    return write(&value, 1);
}

size_t FLProgNetworkTcpServerClient::write(
    const uint8_t *buffer,
    size_t length)
{
    const size_t result = FLProgNetworkSocket::write(buffer, length);
    _txBytes += (uint32_t)result;
    if (result > 0)
    {
        _lastActivityAt = millis();
    }
    return result;
}

bool FLProgPosixTcpServerListener::begin(
    uint8_t transportIdValue,
    IPAddress localIp,
    uint16_t portValue,
    uint8_t backlogValue)
{
    close();

    if (portValue == 0)
    {
        return false;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (
        transportIdValue != FLPROG_TRANSPORT_WIFI &&
        transportIdValue != FLPROG_TRANSPORT_WIFI_AP)
    {
        return false;
    }

    _esp8266Server = new (_esp8266ServerStorage) WiFiServer(localIp, portValue);

    const uint8_t backlog = backlogValue == 0 ? 1 : backlogValue;
    _esp8266Server->begin(portValue, backlog);
    if (_esp8266Server->status() == CLOSED)
    {
        _lastNativeError = -2;
        close();
        return false;
    }

    _transportId = transportIdValue;
    _localPort = _esp8266Server->port();
    _listening = true;
    _lastNativeError = 0;
    return true;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (
        transportIdValue != FLPROG_TRANSPORT_WIFI &&
        transportIdValue != FLPROG_TRANSPORT_WIFI_AP &&
        transportIdValue != FLPROG_TRANSPORT_RMII)
    {
        return false;
    }

    _socketFd = ::socket(AF_INET, SOCK_STREAM, 0);
    if (_socketFd < 0)
    {
        _lastNativeError = errno;
        return false;
    }

    int reuse = 1;
    (void)setsockopt(
        _socketFd,
        SOL_SOCKET,
        SO_REUSEADDR,
        (const char *)&reuse,
        sizeof(reuse));

    const int flags = fcntl(_socketFd, F_GETFL, 0);
    if (flags >= 0)
    {
        fcntl(_socketFd, F_SETFL, flags | O_NONBLOCK);
    }

    struct sockaddr_in localAddress;
    memset(&localAddress, 0, sizeof(localAddress));
    localAddress.sin_family = AF_INET;
    localAddress.sin_port = htons(portValue);
    localAddress.sin_addr.s_addr = flprogTcpServerIpv4NetworkOrder(localIp);

    if (::bind(
            _socketFd,
            (struct sockaddr *)&localAddress,
            sizeof(localAddress)) < 0)
    {
        _lastNativeError = errno;
        close();
        return false;
    }

    const int backlog = backlogValue == 0 ? 1 : backlogValue;
    if (::listen(_socketFd, backlog) < 0)
    {
        _lastNativeError = errno;
        close();
        return false;
    }

    _transportId = transportIdValue;
    _localPort = portValue;
    _listening = true;
    _lastNativeError = 0;
    return true;
#else
    (void)transportIdValue;
    (void)localIp;
    (void)backlogValue;
    return false;
#endif
}

void FLProgPosixTcpServerListener::close()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (_esp8266Server != nullptr)
    {
        _esp8266Server->close();
        _esp8266Server->~WiFiServer();
        _esp8266Server = nullptr;
    }
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (_socketFd >= 0)
    {
        ::close(_socketFd);
    }
#endif
    _socketFd = -1;
    _transportId = FLPROG_TRANSPORT_NONE;
    _localPort = 0;
    _listening = false;
}

int FLProgPosixTcpServerListener::acceptOne(
    int &socketFd,
    IPAddress &remoteIp,
    uint16_t &remotePort)
{
    socketFd = -1;
    remoteIp = IPAddress(0, 0, 0, 0);
    remotePort = 0;

#if FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_listening || _socketFd < 0)
    {
        return -1;
    }

    struct sockaddr_in remoteAddress;
    socklen_t remoteLength = sizeof(remoteAddress);
    memset(&remoteAddress, 0, sizeof(remoteAddress));

    const int accepted = ::accept(
        _socketFd,
        (struct sockaddr *)&remoteAddress,
        &remoteLength);

    if (accepted < 0)
    {
        if (errno == EAGAIN
#ifdef EWOULDBLOCK
            || errno == EWOULDBLOCK
#endif
        )
        {
            return 0;
        }

        _lastNativeError = errno;
        return -1;
    }

    socketFd = accepted;
    remoteIp = flprogTcpServerIpFromNetworkOrder(remoteAddress.sin_addr.s_addr);
    remotePort = ntohs(remoteAddress.sin_port);
    _lastNativeError = 0;
    return 1;
#else
    return -1;
#endif
}

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
int FLProgPosixTcpServerListener::acceptOne(
    WiFiClient &client,
    IPAddress &remoteIp,
    uint16_t &remotePort)
{
    client = WiFiClient();
    remoteIp = IPAddress(0, 0, 0, 0);
    remotePort = 0;

    if (!_listening || _esp8266Server == nullptr)
    {
        return -1;
    }

    if (!_esp8266Server->hasClient())
    {
        return 0;
    }

    WiFiClient accepted = _esp8266Server->accept();
    if (!accepted)
    {
        return 0;
    }

    remoteIp = accepted.remoteIP();
    remotePort = accepted.remotePort();
    client = accepted;
    _lastNativeError = 0;
    return 1;
}
#endif

FLProgNetworkTcpServer::FLProgNetworkTcpServer()
{
    if (_maxClients > FLPROG_TCP_SERVER_MAX_CLIENTS)
    {
        _maxClients = FLPROG_TCP_SERVER_MAX_CLIENTS;
    }
}

FLProgNetworkTcpServer::~FLProgNetworkTcpServer()
{
    end();
    delete[] _clients;
    _clients = nullptr;
    _clientCapacity = 0;
}

bool FLProgNetworkTcpServer::ensureClientCapacity(uint8_t required)
{
    if (required == 0 || required > FLPROG_TCP_SERVER_MAX_CLIENTS)
    {
        return false;
    }
    if (_clients != nullptr && _clientCapacity >= required)
    {
        return true;
    }

    // Never relocate live socket backends. begin() calls end() before this
    // helper, and setMaxClients() may grow storage only while no slot is used.
    if (anyUsedSlot())
    {
        return false;
    }

    FLProgNetworkTcpServerClient *replacement =
        new (std::nothrow) FLProgNetworkTcpServerClient[required];
    if (replacement == nullptr)
    {
        return false;
    }

    delete[] _clients;
    _clients = replacement;
    _clientCapacity = required;
    return true;
}

bool FLProgNetworkTcpServer::begin(
    FLProgNetworkInterface &network,
    uint16_t portValue,
    uint8_t routeValue,
    uint8_t maxClientsValue)
{
    end();

    if (portValue == 0 || maxClientsValue == 0)
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_CONFIG;
        return false;
    }

    if (maxClientsValue > FLPROG_TCP_SERVER_MAX_CLIENTS)
    {
        maxClientsValue = FLPROG_TCP_SERVER_MAX_CLIENTS;
    }

    if (!ensureClientCapacity(maxClientsValue))
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
        return false;
    }

    _network = &network;
    _port = portValue;
    _route = routeValue;
    _maxClients = maxClientsValue;
    _boundTransportId = FLPROG_TRANSPORT_NONE;
    _boundGeneration = 0;
    _lastTransportId = FLPROG_TRANSPORT_NONE;
    _newClientMask = 0;
    _events = FLPROG_TCP_SERVER_EVENT_NONE;
    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
    _lastNativeError = 0;
    _lastWiznetCloseEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    _lastListeningState = false;
    _availableCursor = 0;
    _newClientCursor = 0;
    _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_RUNNING;

    if (!attach(network))
    {
        _network = nullptr;
        _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE;
        _serverError = FLPROG_TCP_SERVER_ERROR_ATTACH;
        return false;
    }

    _active = true;
    return true;
}

bool FLProgNetworkTcpServer::beginChild(
    FLProgNetworkInterface &network,
    uint16_t portValue,
    uint8_t routeValue,
    uint8_t maxClientsValue)
{
    // Composite-service variant of begin(). The parent owns this object and
    // calls pool() explicitly, so registering it again in the network managed
    // service table is both unnecessary and can exhaust the fixed service
    // registry in a loaded FLProg project.
    end();

    if (portValue == 0 || maxClientsValue == 0)
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_CONFIG;
        return false;
    }

    if (maxClientsValue > FLPROG_TCP_SERVER_MAX_CLIENTS)
    {
        maxClientsValue = FLPROG_TCP_SERVER_MAX_CLIENTS;
    }

    if (!ensureClientCapacity(maxClientsValue))
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
        return false;
    }

    _network = &network;
    _port = portValue;
    _route = routeValue;
    _maxClients = maxClientsValue;
    _boundTransportId = FLPROG_TRANSPORT_NONE;
    _boundGeneration = 0;
    _lastTransportId = FLPROG_TRANSPORT_NONE;
    _newClientMask = 0;
    _events = FLPROG_TCP_SERVER_EVENT_NONE;
    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
    _lastNativeError = 0;
    _lastWiznetCloseEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    _lastListeningState = false;
    _availableCursor = 0;
    _newClientCursor = 0;
    _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_RUNNING;
    _active = true;
    return true;
}

void FLProgNetworkTcpServer::end()
{
    _active = false;
    closePosixListener();

    for (uint8_t i = 0; i < _clientCapacity; i++)
    {
        _clients[i].releaseSlot();
    }

    detach();
    _network = nullptr;
    _boundTransportId = FLPROG_TRANSPORT_NONE;
    _boundGeneration = 0;
    _newClientMask = 0;
    _lastListeningState = false;
    _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE;
}

void FLProgNetworkTcpServer::setRoute(uint8_t value)
{
    if (_route == value)
    {
        return;
    }

    _route = value;
    if (_boundTransportId != FLPROG_TRANSPORT_NONE)
    {
        handleTransportLoss();
    }
}

bool FLProgNetworkTcpServer::setMaxClients(uint8_t value)
{
    if (value == 0 || value > FLPROG_TCP_SERVER_MAX_CLIENTS)
    {
        return false;
    }

    if (value < clientCount())
    {
        return false;
    }

    if (value > _clientCapacity && !ensureClientCapacity(value))
    {
        return false;
    }

    _maxClients = value;
    return true;
}

void FLProgNetworkTcpServer::setNoDelay(bool value)
{
    _noDelay = value;
    if (_clients == nullptr) return;

    // POSIX/lwIP supports live TCP_NODELAY reconfiguration on accepted fds.
    // Wiznet Sn_MR.ND is configured before OPEN and is not rewritten here.
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (
            _clients[i]._used &&
            (_clients[i]._transportId == FLPROG_TRANSPORT_WIFI ||
             _clients[i]._transportId == FLPROG_TRANSPORT_WIFI_AP ||
             _clients[i]._transportId == FLPROG_TRANSPORT_RMII))
        {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
            if (_clients[i]._transportId == FLPROG_TRANSPORT_RMII)
            {
                _clients[i]._stm32Backend.setNoDelay(value);
            }
            else
#endif
            {
                _clients[i]._posixBackend.setNoDelay(value);
            }
        }
    }
}

void FLProgNetworkTcpServer::setKeepAlive(
    bool enabled,
    uint32_t idleSeconds,
    uint32_t intervalSeconds,
    uint8_t probeCount)
{
    _keepAliveEnabled = enabled;
    _keepAliveIdleSeconds = idleSeconds;
    _keepAliveIntervalSeconds = intervalSeconds;
    _keepAliveProbeCount = probeCount == 0 ? 1 : probeCount;
    if (_clients == nullptr) return;

    // POSIX/lwIP can update keepalive on live accepted sockets. Wiznet
    // keepalive configuration remains an OPEN-time policy.
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (
            _clients[i]._used &&
            (_clients[i]._transportId == FLPROG_TRANSPORT_WIFI ||
             _clients[i]._transportId == FLPROG_TRANSPORT_WIFI_AP ||
             _clients[i]._transportId == FLPROG_TRANSPORT_RMII))
        {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
            if (_clients[i]._transportId == FLPROG_TRANSPORT_RMII)
            {
                _clients[i]._stm32Backend.setKeepAlive(
                    _keepAliveEnabled,
                    _keepAliveIdleSeconds,
                    _keepAliveIntervalSeconds,
                    _keepAliveProbeCount);
            }
            else
#endif
            {
                _clients[i]._posixBackend.setKeepAlive(
                    _keepAliveEnabled,
                    _keepAliveIdleSeconds,
                    _keepAliveIntervalSeconds,
                    _keepAliveProbeCount);
            }
        }
    }
}

bool FLProgNetworkTcpServer::listening() const
{
    if (_clients == nullptr) return false;
    if (_boundTransportId == FLPROG_TRANSPORT_WIZNET)
    {
        const int index = findWiznetListenerSlot();
        if (index < 0)
        {
            return false;
        }
        return _clients[index].wiznetBackend().listening();
    }

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    if (_boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        return _stm32Listener.listening();
    }
#endif

    if (
        _boundTransportId == FLPROG_TRANSPORT_WIFI ||
        _boundTransportId == FLPROG_TRANSPORT_WIFI_AP ||
        _boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        return _posixListener.listening();
    }

    return false;
}

uint8_t FLProgNetworkTcpServer::clientCount() const
{
    if (_clients == nullptr) return 0;
    uint8_t count = 0;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (_clients[i].active())
        {
            count++;
        }
    }
    return count;
}

uint8_t FLProgNetworkTcpServer::wiznetSocketStatus() const
{
    if (_clients == nullptr) return FLPROG_WIZNET_SOCK_CLOSED;
    for (uint8_t i = 0; i < _maxClients; ++i)
    {
        if (_clients[i]._used &&
            _clients[i]._transportId == FLPROG_TRANSPORT_WIZNET)
        {
            return _clients[i]._wiznetBackend.socketStatus();
        }
    }
    return FLPROG_WIZNET_SOCK_CLOSED;
}

bool FLProgNetworkTcpServer::wiznetSendPending() const
{
    if (_clients == nullptr) return false;
    for (uint8_t i = 0; i < _maxClients; ++i)
    {
        if (_clients[i]._used &&
            _clients[i]._transportId == FLPROG_TRANSPORT_WIZNET)
        {
            return _clients[i]._wiznetBackend.sendPending();
        }
    }
    return false;
}

FLProgNetworkTcpServerClient *FLProgNetworkTcpServer::client(uint8_t slot)
{
    if (_clients == nullptr) return nullptr;
    if (slot >= _maxClients || !_clients[slot].active())
    {
        return nullptr;
    }
    return &_clients[slot];
}

const FLProgNetworkTcpServerClient *FLProgNetworkTcpServer::client(uint8_t slot) const
{
    if (_clients == nullptr) return nullptr;
    if (slot >= _maxClients || !_clients[slot].active())
    {
        return nullptr;
    }
    return &_clients[slot];
}

FLProgNetworkTcpServerClient *FLProgNetworkTcpServer::takeNewClient()
{
    if (_clients == nullptr || _maxClients == 0) return nullptr;
    for (uint8_t offset = 0; offset < _maxClients; offset++)
    {
        const uint8_t i = (uint8_t)((_newClientCursor + offset) % _maxClients);
        const uint8_t bit = (uint8_t)(1U << i);
        if ((_newClientMask & bit) != 0)
        {
            _newClientMask &= (uint8_t)~bit;
            _newClientCursor = (uint8_t)((i + 1U) % _maxClients);
            if (_clients[i].active())
            {
                return &_clients[i];
            }
        }
    }
    return nullptr;
}

FLProgNetworkTcpServerClient *FLProgNetworkTcpServer::availableClient()
{
    if (_clients == nullptr || _maxClients == 0) return nullptr;
    for (uint8_t offset = 0; offset < _maxClients; offset++)
    {
        const uint8_t i = (uint8_t)((_availableCursor + offset) % _maxClients);
        if (_clients[i].active() && _clients[i].available() > 0)
        {
            _availableCursor = (uint8_t)((i + 1U) % _maxClients);
            return &_clients[i];
        }
    }
    return nullptr;
}

void FLProgNetworkTcpServer::disconnectAll()
{
    if (_clients == nullptr) return;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (_clients[i]._accepted || _clients[i]._pendingAccept)
        {
            _clients[i].requestCleanup();
        }
    }
}

void FLProgNetworkTcpServer::requestShutdown()
{
    if (!_active || _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE)
    {
        return;
    }

    if (_shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_RUNNING)
    {
        _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_REQUESTED;
    }
}

bool FLProgNetworkTcpServer::anyUsedSlot() const
{
    if (_clients == nullptr) return false;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (_clients[i]._used)
        {
            return true;
        }
    }
    return false;
}

void FLProgNetworkTcpServer::serviceShutdown()
{
    if (_shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_REQUESTED)
    {
        closePosixListener();
        for (uint8_t i = 0; i < _maxClients; i++)
        {
            if (_clients[i]._used)
            {
                _clients[i].requestCleanup();
            }
        }
        _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_DRAINING;
    }

    if (
        _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_DRAINING &&
        !anyUsedSlot())
    {
        closePosixListener();
        _active = false;
        detach();
        _network = nullptr;
        _boundTransportId = FLPROG_TRANSPORT_NONE;
        _boundGeneration = 0;
        _newClientMask = 0;
        _lastListeningState = false;
        _shutdownState = FLPROG_TCP_SERVER_SHUTDOWN_COMPLETE;
        _shutdownCount++;
        _events |= FLPROG_TCP_SERVER_EVENT_SHUTDOWN_COMPLETE;
    }
}

uint8_t FLProgNetworkTcpServer::serverOwnedWiznetSocketCount() const
{
    if (_clients == nullptr) return 0;
    uint8_t count = 0;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (
            _clients[i]._used &&
            _clients[i]._transportId == FLPROG_TRANSPORT_WIZNET &&
            _clients[i].nativeSocket() != FLPROG_SOCKET_NATIVE_NONE)
        {
            count++;
        }
    }
    return count;
}

uint8_t FLProgNetworkTcpServer::externalWiznetSocketReservationCount() const
{
    if (_network == nullptr)
    {
        return 0;
    }

    const uint8_t reserved = _network->wiznetReservedSocketCount();
    const uint8_t owned = serverOwnedWiznetSocketCount();
    return reserved > owned ? (uint8_t)(reserved - owned) : 0;
}

uint8_t FLProgNetworkTcpServer::effectiveClientCapacity() const
{
    if (_network == nullptr || _maxClients == 0)
    {
        return 0;
    }

    uint8_t transportIdValue = _boundTransportId;
    if (transportIdValue == FLPROG_TRANSPORT_NONE)
    {
        transportIdValue = _network->transportIdForRoute(_route, false);
    }

    uint8_t result = _maxClients;

    if (transportIdValue == FLPROG_TRANSPORT_WIZNET)
    {
        const FLProgNetworkTransport *base =
            _network->transportById(FLPROG_TRANSPORT_WIZNET);
        if (base == nullptr)
        {
            return 0;
        }

        const uint8_t limit = base->nativeSocketLimit();
        const uint8_t external = externalWiznetSocketReservationCount();
        if (limit <= external + 1U)
        {
            return 0;
        }

        const uint8_t nativeCapacity = (uint8_t)(limit - external - 1U);
        if (result > nativeCapacity)
        {
            result = nativeCapacity;
        }
        return result;
    }

    if (
        transportIdValue == FLPROG_TRANSPORT_WIFI ||
        transportIdValue == FLPROG_TRANSPORT_WIFI_AP ||
        transportIdValue == FLPROG_TRANSPORT_RMII)
    {
        uint8_t occupied = 0;
        for (uint8_t i = 0; i < _maxClients; i++)
        {
            if (_clients[i]._used)
            {
                occupied++;
            }
        }

        const uint8_t managedTotal =
            (uint8_t)(occupied + _network->managedSocketFreeCount());
        if (result > managedTotal)
        {
            result = managedTotal;
        }

        const uint8_t leaseTotal =
            (uint8_t)(occupied + _network->logicalLeaseFreeCount());
        if (result > leaseTotal)
        {
            result = leaseTotal;
        }
        return result;
    }

    return 0;
}

uint8_t FLProgNetworkTcpServer::acceptCapacityRemaining() const
{
    const uint8_t capacity = effectiveClientCapacity();
    const uint8_t active = clientCount();
    return active >= capacity ? 0 : (uint8_t)(capacity - active);
}

int FLProgNetworkTcpServer::findFreeSlot() const
{
    if (_clients == nullptr) return -1;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (!_clients[i]._used)
        {
            return i;
        }
    }
    return -1;
}

int FLProgNetworkTcpServer::findWiznetListenerSlot() const
{
    if (_clients == nullptr) return -1;
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (_clients[i].listenerSlot())
        {
            return i;
        }
    }
    return -1;
}

void FLProgNetworkTcpServer::reapSlots()
{
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        FLProgNetworkTcpServerClient &item = _clients[i];
        if (!item._used)
        {
            continue;
        }

        const uint8_t stateValue = item.state();

        if (item._accepted)
        {
            if (
                _clientIdleTimeoutMs > 0 &&
                item.connected() &&
                !item._cleanupRequested &&
                (uint32_t)(millis() - item._lastActivityAt) >= _clientIdleTimeoutMs)
            {
                _idleTimeoutCount++;
                _events |= FLPROG_TCP_SERVER_EVENT_CLIENT_IDLE;
                item.requestCleanup();
            }

            if (
                stateValue == FLPROG_SOCKET_STATE_ERROR ||
                stateValue == FLPROG_SOCKET_STATE_UNSUPPORTED ||
                stateValue == FLPROG_SOCKET_STATE_STALE ||
                stateValue == FLPROG_SOCKET_STATE_REOPEN_REQUIRED)
            {
                if (item._transportId == FLPROG_TRANSPORT_WIZNET)
                {
                    _lastWiznetCloseEvent = item._wiznetBackend.lastBackendEvent();
                }
                item.requestCleanup();
            }
        }
        else if (item._listener)
        {
            if (
                stateValue == FLPROG_SOCKET_STATE_ERROR ||
                stateValue == FLPROG_SOCKET_STATE_UNSUPPORTED)
            {
                if (item._transportId == FLPROG_TRANSPORT_WIZNET)
                {
                    _lastWiznetCloseEvent = item._wiznetBackend.lastBackendEvent();
                }
                _serverError = FLPROG_TCP_SERVER_ERROR_LISTEN;
                item.requestCleanup();
            }
        }
        else if (item._pendingAccept)
        {
            if (
                stateValue == FLPROG_SOCKET_STATE_ERROR ||
                stateValue == FLPROG_SOCKET_STATE_UNSUPPORTED ||
                stateValue == FLPROG_SOCKET_STATE_STALE ||
                stateValue == FLPROG_SOCKET_STATE_REOPEN_REQUIRED)
            {
                _rejectCount++;
                item.requestCleanup();
            }
        }

        if (item.cleanupComplete())
        {
            if (item._accepted)
            {
                _disconnectCount++;
                _events |= FLPROG_TCP_SERVER_EVENT_CLIENT_CLOSED;
            }
            _newClientMask &= (uint8_t)~(1U << i);
            item.releaseSlot();
        }
    }
}

void FLProgNetworkTcpServer::processPendingAccepts()
{
    for (uint8_t i = 0; i < _maxClients; i++)
    {
        FLProgNetworkTcpServerClient &item = _clients[i];
        if (!item._used || item._accepted)
        {
            continue;
        }

        if (item._listener && item.connected())
        {
            if (item.promoteAccepted())
            {
                _acceptCount++;
                _newClientMask |= (uint8_t)(1U << i);
                _events |= FLPROG_TCP_SERVER_EVENT_NEW_CLIENT;
            }
        }
        else if (item._pendingAccept && item.connected())
        {
            if (item.promoteAccepted())
            {
                _acceptCount++;
                _newClientMask |= (uint8_t)(1U << i);
                _events |= FLPROG_TCP_SERVER_EVENT_NEW_CLIENT;
            }
        }
    }
}

bool FLProgNetworkTcpServer::timeTransportUsable(
    FLProgNetworkInterface &network,
    uint8_t transportIdValue,
    uint32_t generationValue)
{
    FLProgNetworkTransport *transport = network.transportById(transportIdValue);
    return
        transport != nullptr &&
        transport->isReady() &&
        transport->generation() == generationValue;
}

void FLProgNetworkTcpServer::handleTransportLoss()
{
    if (_boundTransportId == FLPROG_TRANSPORT_NONE)
    {
        return;
    }

    closePosixListener();

    for (uint8_t i = 0; i < _maxClients; i++)
    {
        if (_clients[i]._used)
        {
            _clients[i].requestCleanup();
        }
    }

    _lastTransportId = _boundTransportId;
    _boundTransportId = FLPROG_TRANSPORT_NONE;
    _boundGeneration = 0;
}

void FLProgNetworkTcpServer::bindTransportIfNeeded()
{
    if (_network == nullptr)
    {
        return;
    }

    if (_boundTransportId != FLPROG_TRANSPORT_NONE)
    {
        if (timeTransportUsable(*_network, _boundTransportId, _boundGeneration))
        {
            return;
        }
        handleTransportLoss();
    }

    const uint8_t selected = _network->transportIdForRoute(_route, true);
    if (selected == FLPROG_TRANSPORT_NONE)
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_NO_TRANSPORT;
        return;
    }

    FLProgNetworkTransport *transport = _network->transportById(selected);
    if (transport == nullptr || !transport->isReady())
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_NO_TRANSPORT;
        return;
    }

    _boundTransportId = selected;
    _boundGeneration = transport->generation();
    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;

    if (_lastTransportId != FLPROG_TRANSPORT_NONE && _lastTransportId != selected)
    {
        _transportChangeCount++;
        _events |= FLPROG_TCP_SERVER_EVENT_TRANSPORT_CHANGED;
    }
    _lastTransportId = selected;
}

void FLProgNetworkTcpServer::noteResourceWait()
{
    _resourceWaitCount++;
    _events |= FLPROG_TCP_SERVER_EVENT_RESOURCE_WAIT;
    _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
}

void FLProgNetworkTcpServer::ensureWiznetListener()
{
    if (clientCount() >= _maxClients || findWiznetListenerSlot() >= 0)
    {
        return;
    }

    const int slot = findFreeSlot();
    if (slot < 0)
    {
        noteResourceWait();
        return;
    }

    FLProgWiznetTransport *transport =
        static_cast<FLProgWiznetTransport *>(
            _network->transportById(FLPROG_TRANSPORT_WIZNET));

    if (transport == nullptr || transport->nativeSocketAvailableCount() == 0)
    {
        noteResourceWait();
        return;
    }

    uint32_t keepAliveMs = _keepAliveIntervalSeconds * 1000UL;
    if (!_clients[slot].startWiznetListener(
            *_network,
            _port,
            (uint8_t)slot,
            _noDelay,
            _keepAliveEnabled,
            keepAliveMs))
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
        noteResourceWait();
        return;
    }

    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
}

bool FLProgNetworkTcpServer::reservePosixListenerLease()
{
    if (_posixListenerLease != FLPROG_SOCKET_INVALID_HANDLE)
    {
        return true;
    }

    _posixListenerLease = _network->socketManager().acquire(
        this,
        _boundTransportId,
        true);

    if (_posixListenerLease == FLPROG_SOCKET_INVALID_HANDLE)
    {
        noteResourceWait();
        return false;
    }

    if (!_network->socketManager().activate(_posixListenerLease, this))
    {
        _network->socketManager().release(_posixListenerLease, this);
        _posixListenerLease = FLPROG_SOCKET_INVALID_HANDLE;
        noteResourceWait();
        return false;
    }

    return true;
}

void FLProgNetworkTcpServer::releasePosixListenerLease()
{
    if (_network != nullptr && _posixListenerLease != FLPROG_SOCKET_INVALID_HANDLE)
    {
        _network->socketManager().release(_posixListenerLease, this);
    }
    _posixListenerLease = FLPROG_SOCKET_INVALID_HANDLE;
}

void FLProgNetworkTcpServer::closePosixListener()
{
    const bool wasListening = _posixListener.listening();
    _posixListener.close();
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    const bool wasStm32Listening = _stm32Listener.listening();
    _stm32Listener.close();
    (void)wasStm32Listening;
#endif
    releasePosixListenerLease();
    (void)wasListening;
}

void FLProgNetworkTcpServer::ensurePosixListener()
{
#if FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP
    if (_posixListener.listening())
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
        return;
    }

    if (!reservePosixListenerLease())
    {
        return;
    }

    FLProgNetworkTransport *transport = _network->transportById(_boundTransportId);
    if (transport == nullptr)
    {
        closePosixListener();
        return;
    }

    if (!_posixListener.begin(
            _boundTransportId,
            transport->localIP(),
            _port,
            _listenBacklog))
    {
        _lastNativeError = _posixListener.lastNativeError();
        _serverError = FLPROG_TCP_SERVER_ERROR_LISTEN;
        closePosixListener();
        return;
    }

    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
#else
    _serverError = FLPROG_TCP_SERVER_ERROR_LISTEN;
#endif
}

void FLProgNetworkTcpServer::acceptPosixClients()
{
#if FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP
    if (!_posixListener.listening())
    {
        return;
    }

    // Bound work per pool() call; no accept loop may run without a cap.
    for (uint8_t attempt = 0; attempt < _acceptWorkBudget; attempt++)
    {
        IPAddress remoteIp;
        uint16_t remotePort = 0;

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
        WiFiClient acceptedClient;
        const int result = _posixListener.acceptOne(
            acceptedClient,
            remoteIp,
            remotePort);
#else
        int fd = -1;
        const int result = _posixListener.acceptOne(fd, remoteIp, remotePort);
#endif

        if (result == 0)
        {
            return;
        }

        if (result < 0)
        {
            _lastNativeError = _posixListener.lastNativeError();
            _serverError = FLPROG_TCP_SERVER_ERROR_ACCEPT;
            return;
        }

        const int slot = findFreeSlot();
        const bool leaseCapacity =
            _network->socketManager().activeLeaseCount() < FLPROG_SOCKET_MAX_LEASES;

        if (slot < 0 || clientCount() >= _maxClients || !leaseCapacity)
        {
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
            (void)acceptedClient.stop(1);
#else
            FLProgPosixTcpClientBackend rejectBackend;
            rejectBackend.configure(_boundTransportId);
            if (rejectBackend.adoptAcceptedSocket(fd, _port, remoteIp, remotePort))
            {
                rejectBackend.close();
            }
            else
            {
                ::close(fd);
            }
#endif
            _rejectCount++;
            noteResourceWait();
            continue;
        }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
        const bool adopted = _clients[slot].adoptEsp8266(
            *_network,
            _boundTransportId,
            acceptedClient,
            _port,
            remoteIp,
            remotePort,
            (uint8_t)slot,
            _noDelay,
            _keepAliveEnabled,
            _keepAliveIdleSeconds,
            _keepAliveIntervalSeconds,
            _keepAliveProbeCount);
#else
        const bool adopted = _clients[slot].adoptPosix(
            *_network,
            _boundTransportId,
            fd,
            _port,
            remoteIp,
            remotePort,
            (uint8_t)slot,
            _noDelay,
            _keepAliveEnabled,
            _keepAliveIdleSeconds,
            _keepAliveIntervalSeconds,
            _keepAliveProbeCount);
#endif

        if (!adopted)
        {
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
            (void)acceptedClient.stop(1);
#else
            FLProgPosixTcpClientBackend rejectBackend;
            rejectBackend.configure(_boundTransportId);
            if (rejectBackend.adoptAcceptedSocket(fd, _port, remoteIp, remotePort))
            {
                rejectBackend.close();
            }
            else
            {
                ::close(fd);
            }
#endif
            _rejectCount++;
            _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
            continue;
        }
    }
#else
    return;
#endif
}

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
void FLProgNetworkTcpServer::ensureStm32Listener()
{
    if (_stm32Listener.listening())
    {
        _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
        return;
    }

    if (!reservePosixListenerLease())
    {
        return;
    }

    FLProgNetworkTransport *transport = _network->transportById(_boundTransportId);
    if (transport == nullptr)
    {
        closePosixListener();
        return;
    }

    if (!_stm32Listener.begin(
            transport->localIP(),
            _port,
            _listenBacklog))
    {
        _lastNativeError = _stm32Listener.lastNativeError();
        _serverError = FLPROG_TCP_SERVER_ERROR_LISTEN;
        closePosixListener();
        return;
    }

    _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
}

void FLProgNetworkTcpServer::acceptStm32Clients()
{
    if (!_stm32Listener.listening())
    {
        return;
    }

    for (uint8_t attempt = 0; attempt < _acceptWorkBudget; attempt++)
    {
        tcp_struct *client = nullptr;
        IPAddress remoteIp;
        uint16_t remotePort = 0;
        const int result = _stm32Listener.acceptOne(client, remoteIp, remotePort);

        if (result == 0)
        {
            return;
        }
        if (result < 0)
        {
            _lastNativeError = _stm32Listener.lastNativeError();
            _serverError = FLPROG_TCP_SERVER_ERROR_ACCEPT;
            return;
        }

        const int slot = findFreeSlot();
        const bool leaseCapacity =
            _network->socketManager().activeLeaseCount() < FLPROG_SOCKET_MAX_LEASES;

        if (slot < 0 || clientCount() >= _maxClients || !leaseCapacity)
        {
            FLProgStm32TcpClientBackend::destroyAccepted(client);
            _rejectCount++;
            noteResourceWait();
            continue;
        }

        if (!_clients[slot].adoptStm32(
                *_network,
                client,
                _port,
                remoteIp,
                remotePort,
                (uint8_t)slot,
                _noDelay,
                _keepAliveEnabled,
                _keepAliveIdleSeconds,
                _keepAliveIntervalSeconds,
                _keepAliveProbeCount))
        {
            // adoptStm32() consumes the accepted tcp_struct on every path once
            // ownership is handed to it. Do not free client a second time here.
            _rejectCount++;
            _serverError = FLPROG_TCP_SERVER_ERROR_RESOURCE;
            continue;
        }
    }
}
#endif

void FLProgNetworkTcpServer::ensureListener()
{
    if (_boundTransportId == FLPROG_TRANSPORT_WIZNET)
    {
        ensureWiznetListener();
        return;
    }

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    if (_boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        ensureStm32Listener();
        return;
    }
#endif

    if (
        _boundTransportId == FLPROG_TRANSPORT_WIFI ||
        _boundTransportId == FLPROG_TRANSPORT_WIFI_AP ||
        _boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        ensurePosixListener();
    }
}

void FLProgNetworkTcpServer::updateListenEventState()
{
    const bool current = listening();
    if (current == _lastListeningState)
    {
        return;
    }

    _lastListeningState = current;
    if (current)
    {
        _listenOpenCount++;
        _serverError = FLPROG_TCP_SERVER_ERROR_NONE;
        _events |= FLPROG_TCP_SERVER_EVENT_LISTEN_UP;
    }
    else
    {
        _events |= FLPROG_TCP_SERVER_EVENT_LISTEN_DOWN;
    }
}

void FLProgNetworkTcpServer::pool()
{
    if (!_active || _network == nullptr)
    {
        return;
    }

    reapSlots();
    processPendingAccepts();

    if (
        _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_REQUESTED ||
        _shutdownState == FLPROG_TCP_SERVER_SHUTDOWN_DRAINING)
    {
        serviceShutdown();
        updateListenEventState();
        return;
    }

    bindTransportIfNeeded();

    if (_boundTransportId == FLPROG_TRANSPORT_NONE)
    {
        updateListenEventState();
        return;
    }

    ensureListener();

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    if (_boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        acceptStm32Clients();
    }
    else
#endif
    if (
        _boundTransportId == FLPROG_TRANSPORT_WIFI ||
        _boundTransportId == FLPROG_TRANSPORT_WIFI_AP ||
        _boundTransportId == FLPROG_TRANSPORT_RMII)
    {
        acceptPosixClients();
    }

    updateListenEventState();
}

int16_t FLProgNetworkTcpServer::wiznetListenerSocket() const
{
    if (_clients == nullptr) return FLPROG_SOCKET_NATIVE_NONE;
    const int index = findWiznetListenerSlot();
    if (index < 0)
    {
        return FLPROG_SOCKET_NATIVE_NONE;
    }
    return _clients[index].nativeSocket();
}

uint8_t FLProgNetworkTcpServer::wiznetSocketsReserved() const
{
    if (_network == nullptr)
    {
        return 0;
    }
    const FLProgWiznetTransport *transport =
        static_cast<const FLProgWiznetTransport *>(
            _network->transportById(FLPROG_TRANSPORT_WIZNET));
    return transport == nullptr ? 0 : transport->nativeSocketReservedCount();
}

uint8_t FLProgNetworkTcpServer::wiznetSocketsAvailable() const
{
    if (_network == nullptr)
    {
        return 0;
    }
    const FLProgWiznetTransport *transport =
        static_cast<const FLProgWiznetTransport *>(
            _network->transportById(FLPROG_TRANSPORT_WIZNET));
    return transport == nullptr ? 0 : transport->nativeSocketAvailableCount();
}

void FLProgNetworkTcpServer::resetStatistics()
{
    _listenOpenCount = 0;
    _acceptCount = 0;
    _rejectCount = 0;
    _disconnectCount = 0;
    _resourceWaitCount = 0;
    _transportChangeCount = 0;
    _idleTimeoutCount = 0;
    _shutdownCount = 0;
}
