/*
 * FLProg_Network — flprogNetworkTcpClient.cpp
 * Назначение: Native TCP client FLProg_Network.
 *
 * Архитектурные заметки:
 * - Управляет connect/reconnect поверх socket lease и TransportIdentity.
 * - Разрыв удалённого TCP server не равен потере transport: generation остаётся прежней.
 * - Диагностические open/reopen/stale counters используются regression-gates.
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
#include "flprogNetworkTcpClient.h"
#include "../flprogNetwork.h"

#include <string.h>

#if FLPROG_NETWORK_HAS_POSIX_TCP
#include <lwip/sockets.h>
#include <lwip/tcp.h>
#include <fcntl.h>
#include <errno.h>
#endif

#if FLPROG_NETWORK_HAS_POSIX_TCP
static uint32_t flprogTcpIpv4NetworkOrder(IPAddress ip)
{
    const uint32_t hostOrder =
        ((uint32_t)ip[0] << 24) |
        ((uint32_t)ip[1] << 16) |
        ((uint32_t)ip[2] << 8) |
        (uint32_t)ip[3];
    return htonl(hostOrder);
}
#endif

bool FLProgPosixTcpClientBackend::ipIsZero(IPAddress ip)
{
    return ip[0] == 0 && ip[1] == 0 && ip[2] == 0 && ip[3] == 0;
}

void FLProgPosixTcpClientBackend::resetSocket()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (_esp8266Client || _connected)
    {
        (void)_esp8266Client.stop(1);
    }
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (_socketFd >= 0)
    {
        ::close(_socketFd);
    }
#endif
    _socketFd = -1;
    _connecting = false;
    _connected = false;
    _localPort = 0;
    _started = 0;
    _keepAliveApplied = false;
}

void FLProgPosixTcpClientBackend::applySocketOptions()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    _lastOptionError = 0;
    _keepAliveApplied = false;

    if (!_connected && !_esp8266Client)
    {
        return;
    }

    _esp8266Client.setNoDelay(_noDelay);

    _esp8266Client.setSync(false);

    if (_keepAliveEnabled)
    {
        const uint16_t idle = (uint16_t)(_keepAliveIdleSeconds == 0 ? 1 :
            (_keepAliveIdleSeconds > 65535UL ? 65535UL : _keepAliveIdleSeconds));
        const uint16_t interval = (uint16_t)(_keepAliveIntervalSeconds == 0 ? 1 :
            (_keepAliveIntervalSeconds > 65535UL ? 65535UL : _keepAliveIntervalSeconds));
        const uint8_t count = _keepAliveProbeCount == 0 ? 1 : _keepAliveProbeCount;
        _esp8266Client.keepAlive(idle, interval, count);
        _keepAliveApplied = _esp8266Client.isKeepAliveEnabled();
    }
    else
    {
        _esp8266Client.disableKeepAlive();
    }
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    _lastOptionError = 0;
    _keepAliveApplied = false;

#ifdef TCP_NODELAY
    {
        const int value = _noDelay ? 1 : 0;
        if (setsockopt(
                _socketFd,
                IPPROTO_TCP,
                TCP_NODELAY,
                (const char *)&value,
                sizeof(value)) < 0 &&
            _lastOptionError == 0)
        {
            _lastOptionError = errno;
        }
    }
#endif

    {
        const int value = _keepAliveEnabled ? 1 : 0;
        if (setsockopt(
                _socketFd,
                SOL_SOCKET,
                SO_KEEPALIVE,
                (const char *)&value,
                sizeof(value)) < 0)
        {
            if (_lastOptionError == 0) _lastOptionError = errno;
        }
        else
        {
            _keepAliveApplied = _keepAliveEnabled;
        }
    }

    if (_keepAliveEnabled)
    {
#ifdef TCP_KEEPIDLE
        int idle = (int)(_keepAliveIdleSeconds == 0 ? 1 : _keepAliveIdleSeconds);
        if (setsockopt(
                _socketFd,
                IPPROTO_TCP,
                TCP_KEEPIDLE,
                (const char *)&idle,
                sizeof(idle)) < 0 &&
            _lastOptionError == 0)
        {
            _lastOptionError = errno;
        }
#endif
#ifdef TCP_KEEPINTVL
        int interval = (int)(_keepAliveIntervalSeconds == 0 ? 1 : _keepAliveIntervalSeconds);
        if (setsockopt(
                _socketFd,
                IPPROTO_TCP,
                TCP_KEEPINTVL,
                (const char *)&interval,
                sizeof(interval)) < 0 &&
            _lastOptionError == 0)
        {
            _lastOptionError = errno;
        }
#endif
#ifdef TCP_KEEPCNT
        int count = (int)(_keepAliveProbeCount == 0 ? 1 : _keepAliveProbeCount);
        if (setsockopt(
                _socketFd,
                IPPROTO_TCP,
                TCP_KEEPCNT,
                (const char *)&count,
                sizeof(count)) < 0 &&
            _lastOptionError == 0)
        {
            _lastOptionError = errno;
        }
#endif
    }
#endif
}

bool FLProgPosixTcpClientBackend::adoptAcceptedSocket(
    int socketFd,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue)
{
#if FLPROG_NETWORK_HAS_POSIX_TCP
    if (socketFd < 0 || remotePortValue == 0 || ipIsZero(remoteIpValue))
    {
        return false;
    }

    resetSocket();

    _socketFd = socketFd;
    _localPort = localPortValue;
    _endpoint.localPort = localPortValue;
    _endpoint.remoteIP = remoteIpValue;
    _endpoint.remotePort = remotePortValue;
    _timeout = 0;
    _started = millis();
    _connecting = false;
    _connected = true;
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;

    const int flags = fcntl(_socketFd, F_GETFL, 0);
    if (flags >= 0)
    {
        fcntl(_socketFd, F_SETFL, flags | O_NONBLOCK);
    }

    applySocketOptions();
    return true;
#else
    (void)socketFd;
    (void)localPortValue;
    (void)remoteIpValue;
    (void)remotePortValue;
    return false;
#endif
}

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
bool FLProgPosixTcpClientBackend::adoptAcceptedClient(
    const WiFiClient &client,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue)
{
    if (remotePortValue == 0 || ipIsZero(remoteIpValue))
    {
        return false;
    }

    resetSocket();
    _esp8266Client = client;
    if (!_esp8266Client)
    {
        return false;
    }
    _localPort = localPortValue != 0 ? localPortValue : _esp8266Client.localPort();
    _endpoint.localPort = _localPort;
    _endpoint.remoteIP = remoteIpValue;
    _endpoint.remotePort = remotePortValue;
    _timeout = 0;
    _started = millis();
    _connecting = false;
    _connected = true;
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    applySocketOptions();
    return true;
}
#endif

uint8_t FLProgPosixTcpClientBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    (void)lease;

    if (
        protocol != FLPROG_SOCKET_PROTOCOL_STREAM ||
        (_transportId != FLPROG_TRANSPORT_WIFI &&
         _transportId != FLPROG_TRANSPORT_WIFI_AP &&
         _transportId != FLPROG_TRANSPORT_RMII) ||
        endpoint.remotePort == 0 || ipIsZero(endpoint.remoteIP))
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (_transportId != FLPROG_TRANSPORT_WIFI &&
        _transportId != FLPROG_TRANSPORT_WIFI_AP)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_connected && (_esp8266Client.connected() || _esp8266Client.available() > 0))
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_connected)
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
        resetSocket();
    }

    _endpoint = endpoint;
    _timeout = timeout;
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    _started = millis();
    _connecting = true;

    // ESP8266 core 3.1.2 exposes raw-lwIP through WiFiClient, not BSD sockets.
    // Its connect() call is synchronous inside the core. Keep the requested
    // timeout bounded and preserve the public FLProg_Network state machine.
    _esp8266Client.setTimeout(timeout == 0 ? 5000UL : timeout);
    if (endpoint.localPort != 0)
    {
        WiFiClient::setLocalPortStart(endpoint.localPort);
    }

    const int result = _esp8266Client.connect(endpoint.remoteIP, endpoint.remotePort);
    _connecting = false;

    if (result != 1)
    {
        _lastNativeError = -1;
        _lastBackendEvent =
            (timeout > 0 && (uint32_t)(millis() - _started) >= timeout)
                ? FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT
                : FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _connected = true;
    _localPort = _esp8266Client.localPort();
    applySocketOptions();
    return FLPROG_SOCKET_RESULT_SUCCESS;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (_connected)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_socketFd >= 0)
    {
        return pollConnect();
    }

    _endpoint = endpoint;
    _timeout = timeout;
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    _started = millis();

    _socketFd = ::socket(AF_INET, SOCK_STREAM, 0);
    if (_socketFd < 0)
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    applySocketOptions();

    const int flags = fcntl(_socketFd, F_GETFL, 0);
    if (flags >= 0)
    {
        fcntl(_socketFd, F_SETFL, flags | O_NONBLOCK);
    }

    struct sockaddr_in localAddress;
    memset(&localAddress, 0, sizeof(localAddress));
    localAddress.sin_family = AF_INET;
    localAddress.sin_port = htons(endpoint.localPort);
    localAddress.sin_addr.s_addr = ipIsZero(_localAddress)
        ? htonl(INADDR_ANY)
        : flprogTcpIpv4NetworkOrder(_localAddress);

    if (::bind(_socketFd, (struct sockaddr *)&localAddress, sizeof(localAddress)) < 0)
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    struct sockaddr_in actualAddress;
    socklen_t actualLength = sizeof(actualAddress);
    memset(&actualAddress, 0, sizeof(actualAddress));
    if (getsockname(_socketFd, (struct sockaddr *)&actualAddress, &actualLength) == 0)
    {
        _localPort = ntohs(actualAddress.sin_port);
    }
    else
    {
        _localPort = endpoint.localPort;
    }

    struct sockaddr_in remoteAddress;
    memset(&remoteAddress, 0, sizeof(remoteAddress));
    remoteAddress.sin_family = AF_INET;
    remoteAddress.sin_port = htons(endpoint.remotePort);
    remoteAddress.sin_addr.s_addr = flprogTcpIpv4NetworkOrder(endpoint.remoteIP);

    const int result = ::connect(
        _socketFd,
        (struct sockaddr *)&remoteAddress,
        sizeof(remoteAddress));

    if (result == 0)
    {
        _connected = true;
        _connecting = false;
        _lastNativeError = 0;
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    _lastNativeError = errno;
    if (
        errno == EINPROGRESS ||
        errno == EALREADY ||
#ifdef EWOULDBLOCK
        errno == EWOULDBLOCK ||
#endif
        errno == EAGAIN)
    {
        _connecting = true;
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
    resetSocket();
    return FLPROG_SOCKET_RESULT_ERROR;
#else
    (void)endpoint;
    (void)timeout;
    return FLPROG_SOCKET_RESULT_UNSUPPORTED;
#endif
}

uint8_t FLProgPosixTcpClientBackend::pollConnect()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (_connected && (_esp8266Client.connected() || _esp8266Client.available() > 0))
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }
    return FLPROG_SOCKET_RESULT_ERROR;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (_connected)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }
    if (_socketFd < 0 || !_connecting)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (_timeout > 0 && (uint32_t)(millis() - _started) >= _timeout)
    {
        _lastNativeError = ETIMEDOUT;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    fd_set writeSet;
    fd_set errorSet;
    FD_ZERO(&writeSet);
    FD_ZERO(&errorSet);
    FD_SET(_socketFd, &writeSet);
    FD_SET(_socketFd, &errorSet);
    struct timeval tv;
    tv.tv_sec = 0;
    tv.tv_usec = 0;

    const int ready = select(
        _socketFd + 1,
        nullptr,
        &writeSet,
        &errorSet,
        &tv);

    if (ready < 0)
    {
        if (errno == EINTR)
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (ready == 0)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    int socketError = 0;
    socklen_t optionLength = sizeof(socketError);
    if (getsockopt(
            _socketFd,
            SOL_SOCKET,
            SO_ERROR,
            (char *)&socketError,
            &optionLength) < 0)
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (socketError != 0)
    {
        _lastNativeError = socketError;
        _lastBackendEvent = socketError == ETIMEDOUT
            ? FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT
            : FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _connecting = false;
    _connected = true;
    _lastNativeError = 0;
    return FLPROG_SOCKET_RESULT_SUCCESS;
#else
    return FLPROG_SOCKET_RESULT_UNSUPPORTED;
#endif
}

bool FLProgPosixTcpClientBackend::peerClosed()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected)
    {
        return true;
    }
    if (_esp8266Client.connected() || _esp8266Client.available() > 0)
    {
        return false;
    }
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
    return true;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_connected || _socketFd < 0)
    {
        return true;
    }

    fd_set readSet;
    FD_ZERO(&readSet);
    FD_SET(_socketFd, &readSet);
    struct timeval tv;
    tv.tv_sec = 0;
    tv.tv_usec = 0;
    const int ready = select(_socketFd + 1, &readSet, nullptr, nullptr, &tv);
    if (ready <= 0 || !FD_ISSET(_socketFd, &readSet))
    {
        return false;
    }

    uint8_t value = 0;
    const int result = ::recv(_socketFd, &value, 1, MSG_PEEK);
    if (result == 0)
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
        return true;
    }
    if (result < 0 && errno != EAGAIN
#ifdef EWOULDBLOCK
        && errno != EWOULDBLOCK
#endif
    )
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        return true;
    }
#endif
    return false;
}

uint8_t FLProgPosixTcpClientBackend::pool()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }
    if (peerClosed())
    {
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }
    return FLPROG_SOCKET_RESULT_SUCCESS;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (_connecting)
    {
        return pollConnect();
    }
    if (!_connected)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }
    if (peerClosed())
    {
        resetSocket();
        return FLPROG_SOCKET_RESULT_ERROR;
    }
    return FLPROG_SOCKET_RESULT_SUCCESS;
#else
    return FLPROG_SOCKET_RESULT_UNSUPPORTED;
#endif
}

uint8_t FLProgPosixTcpClientBackend::close()
{
    resetSocket();
    return FLPROG_SOCKET_RESULT_SUCCESS;
}

int FLProgPosixTcpClientBackend::available()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected)
    {
        return 0;
    }
    return _esp8266Client.available();
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_connected || _socketFd < 0)
    {
        return 0;
    }
    int count = 0;
    if (lwip_ioctl(_socketFd, FIONREAD, &count) < 0)
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        return 0;
    }
    return count;
#else
    return 0;
#endif
}

int FLProgPosixTcpClientBackend::read()
{
    uint8_t value = 0;
    const int count = read(&value, 1);
    return count == 1 ? value : -1;
}

int FLProgPosixTcpClientBackend::read(uint8_t *buffer, size_t length)
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected || buffer == nullptr || length == 0)
    {
        return 0;
    }
    const int result = _esp8266Client.read(buffer, length);
    if (result > 0)
    {
        return result;
    }
    if (!_esp8266Client.connected() && _esp8266Client.available() <= 0)
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
        resetSocket();
    }
    return 0;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_connected || _socketFd < 0 || buffer == nullptr || length == 0)
    {
        return 0;
    }
    const int result = ::recv(_socketFd, buffer, length, 0);
    if (result > 0)
    {
        return result;
    }
    if (result == 0)
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
        resetSocket();
        return 0;
    }
    if (errno == EAGAIN
#ifdef EWOULDBLOCK
        || errno == EWOULDBLOCK
#endif
    )
    {
        return 0;
    }
    _lastNativeError = errno;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
    resetSocket();
#else
    (void)buffer;
    (void)length;
#endif
    return 0;
}

int FLProgPosixTcpClientBackend::peek()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected)
    {
        return -1;
    }
    return _esp8266Client.peek();
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_connected || _socketFd < 0)
    {
        return -1;
    }
    uint8_t value = 0;
    const int result = ::recv(_socketFd, &value, 1, MSG_PEEK);
    if (result == 0)
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
    }
    else if (result < 0 && errno != EAGAIN
#ifdef EWOULDBLOCK
        && errno != EWOULDBLOCK
#endif
    )
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
    }
    return result == 1 ? value : -1;
#else
    return -1;
#endif
}

size_t FLProgPosixTcpClientBackend::write(const uint8_t *buffer, size_t length)
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!_connected || buffer == nullptr || length == 0)
    {
        return 0;
    }
    const size_t result = _esp8266Client.write(buffer, length);
    if (result == 0 && !_esp8266Client.connected())
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        _lastNativeError = -1;
        resetSocket();
    }
    return result;
#elif FLPROG_NETWORK_HAS_POSIX_TCP
    if (!_connected || _socketFd < 0 || buffer == nullptr || length == 0)
    {
        return 0;
    }
    const int result = ::send(_socketFd, buffer, length, 0);
    if (result > 0)
    {
        return (size_t)result;
    }
    if (result < 0 && (errno == EAGAIN
#ifdef EWOULDBLOCK
        || errno == EWOULDBLOCK
#endif
    ))
    {
        return 0;
    }
    if (result < 0)
    {
        _lastNativeError = errno;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        resetSocket();
    }
#else
    (void)buffer;
    (void)length;
#endif
    return 0;
}

FLProgNetworkTcpClient::FLProgNetworkTcpClient()
{
    _wifiBackend.configure(FLPROG_TRANSPORT_WIFI);
    _wifiApBackend.configure(FLPROG_TRANSPORT_WIFI_AP);
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _rmiiBackend.configure(FLPROG_TRANSPORT_RMII);
#endif
    applyBackendOptions();
}

bool FLProgNetworkTcpClient::begin(
    FLProgNetworkInterface &network,
    uint8_t routeValue)
{
    _tcpNetwork = &network;
    _logicalRoute = routeValue;

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _wiznetBackend.hardware(network.wiznetHardware());
    setBackend(FLPROG_TRANSPORT_WIZNET, &_wiznetBackend);
#endif

#if FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP
    setBackend(FLPROG_TRANSPORT_WIFI, &_wifiBackend);
    setBackend(FLPROG_TRANSPORT_WIFI_AP, &_wifiApBackend);
#if defined(ESP32) && !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    setBackend(FLPROG_TRANSPORT_RMII, &_rmiiBackend);
#endif
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    setBackend(FLPROG_TRANSPORT_RMII, &_stm32RmiiBackend);
#endif

    applyBackendOptions();

    if (!FLProgNetworkSocket::begin(
            network,
            routeValue,
            FLPROG_SOCKET_PROTOCOL_STREAM))
    {
        _tcpNetwork = nullptr;
        return false;
    }

    // PRIVATE DNS is attached lazily only while a lookup is active. This
    // avoids permanently consuming a managed-service slot for idle clients.
    return true;
}

void FLProgNetworkTcpClient::end()
{
    stop();
    if (_privateDnsAttached)
    {
        _privateDns.end();
        _privateDnsAttached = false;
    }
    FLProgNetworkSocket::end();
    _tcpNetwork = nullptr;
}

bool FLProgNetworkTcpClient::validTarget(IPAddress ip, uint16_t port)
{
    return port != 0 && !(ip[0] == 0 && ip[1] == 0 && ip[2] == 0 && ip[3] == 0);
}

uint8_t FLProgNetworkTcpClient::currentResult() const
{
    if (connected()) return FLPROG_SOCKET_RESULT_SUCCESS;
    if (_clientError != FLPROG_TCP_CLIENT_ERROR_NONE || state() == FLPROG_SOCKET_STATE_ERROR)
        return FLPROG_SOCKET_RESULT_ERROR;
    return FLPROG_SOCKET_RESULT_WAIT;
}

void FLProgNetworkTcpClient::resetTargetState()
{
    _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
    _dnsError = 0;
    _resolvedIP = IPAddress(0, 0, 0, 0);
    _resolvedTransportId = FLPROG_TRANSPORT_NONE;
    _everConnected = false;
    _wasConnected = connected();
    _needResolve = false;
    _restartPending = false;
    _hostname[0] = 0;
    _lastNativeError = 0;
    _lastOptionError = 0;
    resetReconnectBackoff();
}

void FLProgNetworkTcpClient::startIpOpen()
{
    remoteIP(_resolvedIP);
    remotePort(_targetPort);
    FLProgNetworkSocket::setRoute(_logicalRoute);
    _connectAttemptCount++;
    requestOpen();
}

uint8_t FLProgNetworkTcpClient::connect(IPAddress ip, uint16_t port)
{
    if (_tcpNetwork == nullptr || !validTarget(ip, port))
    {
        _clientError = FLPROG_TCP_CLIENT_ERROR_TARGET;
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_INVALID_TARGET;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    const bool replacing = wantsOpen() || state() != FLPROG_SOCKET_STATE_CLOSED;
    resetTargetState();
    _connectRequested = true;
    _hostnameMode = false;
    _targetPort = port;
    _resolvedIP = ip;
    _logicalRoute = route();

    if (replacing)
    {
        scheduleRestart(FLPROG_TCP_DISCONNECT_USER_STOP, 0, false);
    }
    else
    {
        startIpOpen();
    }

    return currentResult();
}

uint8_t FLProgNetworkTcpClient::connect(const char *hostnameValue, uint16_t port)
{
    if (_tcpNetwork == nullptr || hostnameValue == nullptr || hostnameValue[0] == 0 || port == 0)
    {
        _clientError = FLPROG_TCP_CLIENT_ERROR_TARGET;
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_INVALID_TARGET;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    const size_t length = strlen(hostnameValue);
    if (length > FLPROG_TCP_HOSTNAME_MAX_LENGTH)
    {
        _clientError = FLPROG_TCP_CLIENT_ERROR_TARGET;
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_INVALID_TARGET;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    const bool replacing = wantsOpen() || state() != FLPROG_SOCKET_STATE_CLOSED;
    resetTargetState();
    _connectRequested = true;
    _hostnameMode = true;
    _needResolve = !replacing;
    _targetPort = port;
    _logicalRoute = route();
    memcpy(_hostname, hostnameValue, length + 1);

    if (replacing)
    {
        scheduleRestart(FLPROG_TCP_DISCONNECT_USER_STOP, 0, true);
    }

    return FLPROG_SOCKET_RESULT_WAIT;
}

void FLProgNetworkTcpClient::stop()
{
    if (_connectRequested || wantsOpen() || connected())
    {
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_USER_STOP;
    }

    _connectRequested = false;
    _needResolve = false;
    _restartPending = false;

    if (_privateDnsAttached)
    {
        _privateDns.cancel();
        _privateDns.end();
        _privateDnsAttached = false;
    }

    requestClose();
}

bool FLProgNetworkTcpClient::setDnsPolicy(uint8_t value)
{
    if (value != FLPROG_TCP_DNS_POLICY_SHARED && value != FLPROG_TCP_DNS_POLICY_PRIVATE)
    {
        return false;
    }
    if (_dnsPolicy == value)
    {
        return true;
    }

    if (value == FLPROG_TCP_DNS_POLICY_SHARED && _privateDnsAttached)
    {
        _privateDns.end();
        _privateDnsAttached = false;
    }

    _dnsPolicy = value;

    if (_connectRequested && _hostnameMode)
    {
        scheduleRestart(FLPROG_TCP_DISCONNECT_NONE, 0, true);
    }

    return true;
}

FLProgNetworkDns *FLProgNetworkTcpClient::activeDnsResolver()
{
    if (_tcpNetwork == nullptr)
    {
        return nullptr;
    }

    if (_dnsPolicy == FLPROG_TCP_DNS_POLICY_PRIVATE)
    {
        if (!_privateDnsAttached)
        {
            if (!_privateDns.begin(*_tcpNetwork, _logicalRoute))
            {
                return nullptr;
            }
            _privateDnsAttached = true;
        }
        return &_privateDns;
    }

    return _tcpNetwork->dnsResolver();
}

void FLProgNetworkTcpClient::setNoDelay(bool value)
{
    _noDelay = value;
    applyBackendOptions();
}

void FLProgNetworkTcpClient::setKeepAlive(
    bool enabled,
    uint32_t idleSeconds,
    uint32_t intervalSeconds,
    uint8_t probeCount)
{
    _keepAliveEnabled = enabled;
    _keepAliveIdleSeconds = idleSeconds == 0 ? 1 : idleSeconds;
    _keepAliveIntervalSeconds = intervalSeconds == 0 ? 1 : intervalSeconds;
    _keepAliveProbeCount = probeCount == 0 ? 1 : probeCount;
    applyBackendOptions();
}

void FLProgNetworkTcpClient::applyBackendOptions()
{
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _wiznetBackend.setNoDelayedAck(_noDelay);
    _wiznetBackend.setKeepAlive(
        _keepAliveEnabled,
        _keepAliveIntervalSeconds * 1000UL);
#endif

    _wifiBackend.setNoDelay(_noDelay);
    _wifiBackend.setKeepAlive(
        _keepAliveEnabled,
        _keepAliveIdleSeconds,
        _keepAliveIntervalSeconds,
        _keepAliveProbeCount);

    _wifiApBackend.setNoDelay(_noDelay);
    _wifiApBackend.setKeepAlive(
        _keepAliveEnabled,
        _keepAliveIdleSeconds,
        _keepAliveIntervalSeconds,
        _keepAliveProbeCount);

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _rmiiBackend.setNoDelay(_noDelay);
    _rmiiBackend.setKeepAlive(
        _keepAliveEnabled,
        _keepAliveIdleSeconds,
        _keepAliveIntervalSeconds,
        _keepAliveProbeCount);
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET && !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _stm32RmiiBackend.setNoDelay(_noDelay);
    _stm32RmiiBackend.setKeepAlive(
        _keepAliveEnabled,
        _keepAliveIdleSeconds,
        _keepAliveIntervalSeconds,
        _keepAliveProbeCount);
#endif
}

bool FLProgNetworkTcpClient::keepAliveApplied() const
{
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (boundTransportId() == FLPROG_TRANSPORT_WIZNET)
    {
        return _wiznetBackend.keepAliveApplied();
    }
#endif
    if (boundTransportId() == FLPROG_TRANSPORT_WIFI)
    {
        return _wifiBackend.keepAliveApplied();
    }
    if (boundTransportId() == FLPROG_TRANSPORT_WIFI_AP)
    {
        return _wifiApBackend.keepAliveApplied();
    }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (boundTransportId() == FLPROG_TRANSPORT_RMII)
    {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
        return _stm32RmiiBackend.keepAliveApplied();
#else
        return _rmiiBackend.keepAliveApplied();
#endif
    }
#endif
    return false;
}

void FLProgNetworkTcpClient::setReconnectBackoff(uint32_t initialMs, uint32_t maxMs)
{
    if (maxMs < initialMs)
    {
        maxMs = initialMs;
    }
    _reconnectInitialMs = initialMs;
    _reconnectMaxMs = maxMs;
    resetReconnectBackoff();
}

uint32_t FLProgNetworkTcpClient::reconnectIn() const
{
    if (!_restartPending)
    {
        return 0;
    }

    const uint32_t now = millis();
    if (timeReached(now, _restartAt))
    {
        return 0;
    }
    return _restartAt - now;
}

void FLProgNetworkTcpClient::resetReconnectBackoff()
{
    _reconnectCurrentMs = _reconnectInitialMs;
}

void FLProgNetworkTcpClient::advanceReconnectBackoff()
{
    if (_reconnectCurrentMs >= _reconnectMaxMs)
    {
        _reconnectCurrentMs = _reconnectMaxMs;
        return;
    }

    uint32_t next = _reconnectCurrentMs == 0 ? 1 : _reconnectCurrentMs * 2UL;
    if (next < _reconnectCurrentMs || next > _reconnectMaxMs)
    {
        next = _reconnectMaxMs;
    }
    _reconnectCurrentMs = next;
}

void FLProgNetworkTcpClient::setRoute(uint8_t value)
{
    if (_logicalRoute == value)
    {
        return;
    }

    _logicalRoute = value;
    FLProgNetworkSocket::setRoute(value);

    if (_connectRequested && _hostnameMode)
    {
        _resolvedIP = IPAddress(0, 0, 0, 0);
        _resolvedTransportId = FLPROG_TRANSPORT_NONE;
        _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
        _dnsError = 0;
        scheduleRestart(FLPROG_TCP_DISCONNECT_STALE, 0, true);
    }
}

void FLProgNetworkTcpClient::startResolvedTarget(IPAddress ip, uint8_t transportId)
{
    _resolvedIP = ip;
    _resolvedTransportId = transportId;
    _needResolve = false;
    _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
    remoteIP(ip);
    remotePort(_targetPort);
    startIpOpen();
}

void FLProgNetworkTcpClient::dnsStep()
{
    if (!_needResolve || _tcpNetwork == nullptr)
    {
        return;
    }

    FLProgNetworkDns *resolver = activeDnsResolver();
    if (resolver == nullptr)
    {
        _clientError = FLPROG_TCP_CLIENT_ERROR_NO_RESOLVER;
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_NO_RESOLVER;
        _needResolve = false;
        return;
    }

    const uint8_t result = resolver->resolve(_hostname, _logicalRoute);
    if (result == FLPROG_DNS_RESULT_SUCCESS)
    {
        const IPAddress address = resolver->result();
        const uint8_t transportIdValue = resolver->transportId();
        if (_dnsPolicy == FLPROG_TCP_DNS_POLICY_PRIVATE && _privateDnsAttached)
        {
            _privateDns.end();
            _privateDnsAttached = false;
        }
        startResolvedTarget(address, transportIdValue);
        return;
    }
    if (result == FLPROG_DNS_RESULT_BUSY)
    {
        _dnsBusyCount++;
        return;
    }
    if (result == FLPROG_DNS_RESULT_ERROR)
    {
        _dnsError = resolver->error();
        _clientError = FLPROG_TCP_CLIENT_ERROR_DNS;
        _lastDisconnectReason = FLPROG_TCP_DISCONNECT_DNS;
        _needResolve = false;
        if (_dnsPolicy == FLPROG_TCP_DNS_POLICY_PRIVATE && _privateDnsAttached)
        {
            _privateDns.end();
            _privateDnsAttached = false;
        }
    }
}

void FLProgNetworkTcpClient::scheduleRestart(
    uint8_t reason,
    uint32_t delayMs,
    bool reResolve)
{
    if (_tcpNetwork == nullptr)
    {
        return;
    }

    if (reason != FLPROG_TCP_DISCONNECT_NONE)
    {
        _lastDisconnectReason = reason;
    }

    _restartPending = true;
    _restartResolve = reResolve;
    _restartAt = millis() + delayMs;
    requestClose();

    if (_privateDnsAttached)
    {
        _privateDns.cancel();
        _privateDns.end();
        _privateDnsAttached = false;
    }
}

void FLProgNetworkTcpClient::serviceRestart()
{
    if (!_restartPending)
    {
        return;
    }

    // requestClose() set wantsOpen=false. Let the common socket state machine
    // finish DISCON/CLOSE and release the lease before any new attempt.
    FLProgNetworkSocket::pool();
    updateConnectionStatistics();

    if (state() != FLPROG_SOCKET_STATE_CLOSED)
    {
        return;
    }

    if (!timeReached(millis(), _restartAt))
    {
        return;
    }

    const bool resolveAgain = _restartResolve;
    _restartPending = false;
    _restartResolve = false;
    _clientError = FLPROG_TCP_CLIENT_ERROR_NONE;
    _dnsError = 0;
    clearError();

    if (!_connectRequested)
    {
        return;
    }

    if (resolveAgain && _hostnameMode)
    {
        _resolvedIP = IPAddress(0, 0, 0, 0);
        _resolvedTransportId = FLPROG_TRANSPORT_NONE;
        _needResolve = true;
        return;
    }

    _needResolve = false;
    startIpOpen();
}

void FLProgNetworkTcpClient::captureBackendDiagnostics()
{
    _lastNativeError = 0;
    _lastOptionError = 0;

    const uint8_t transport = boundTransportId();
    if (transport == FLPROG_TRANSPORT_WIFI)
    {
        _lastNativeError = _wifiBackend.lastNativeError();
        _lastOptionError = _wifiBackend.lastOptionError();
    }
    else if (transport == FLPROG_TRANSPORT_WIFI_AP)
    {
        _lastNativeError = _wifiApBackend.lastNativeError();
        _lastOptionError = _wifiApBackend.lastOptionError();
    }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    else if (transport == FLPROG_TRANSPORT_RMII)
    {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
        _lastNativeError = _stm32RmiiBackend.lastNativeError();
        _lastOptionError = _stm32RmiiBackend.lastOptionError();
#else
        _lastNativeError = _rmiiBackend.lastNativeError();
        _lastOptionError = _rmiiBackend.lastOptionError();
#endif
    }
#endif
}

uint8_t FLProgNetworkTcpClient::mapFailureReason() const
{
    uint8_t backendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (boundTransportId() == FLPROG_TRANSPORT_WIZNET)
    {
        backendEvent = _wiznetBackend.lastBackendEvent();
    }
    else
#endif
    if (boundTransportId() == FLPROG_TRANSPORT_WIFI)
    {
        backendEvent = _wifiBackend.lastBackendEvent();
    }
    else if (boundTransportId() == FLPROG_TRANSPORT_WIFI_AP)
    {
        backendEvent = _wifiApBackend.lastBackendEvent();
    }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    else if (boundTransportId() == FLPROG_TRANSPORT_RMII)
    {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
        backendEvent = _stm32RmiiBackend.lastBackendEvent();
#else
        backendEvent = _rmiiBackend.lastBackendEvent();
#endif
    }
#endif

    switch (backendEvent)
    {
        case FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT:
            return FLPROG_TCP_DISCONNECT_CONNECT_TIMEOUT;
        case FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED:
            return FLPROG_TCP_DISCONNECT_CONNECT_FAILED;
        case FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED:
            return FLPROG_TCP_DISCONNECT_PEER_CLOSED;
        case FLPROG_TCP_BACKEND_EVENT_IO_ERROR:
            return FLPROG_TCP_DISCONNECT_IO_ERROR;
        case FLPROG_TCP_BACKEND_EVENT_IO_TIMEOUT:
            return FLPROG_TCP_DISCONNECT_IO_TIMEOUT;
        default:
            break;
    }

    switch (error())
    {
        case FLPROG_SOCKET_ERROR_NO_TRANSPORT:
            return FLPROG_TCP_DISCONNECT_NO_TRANSPORT;
        case FLPROG_SOCKET_ERROR_NO_LEASE:
            return FLPROG_TCP_DISCONNECT_NO_LEASE;
        case FLPROG_SOCKET_ERROR_TIMEOUT:
            return FLPROG_TCP_DISCONNECT_CONNECT_TIMEOUT;
        case FLPROG_SOCKET_ERROR_STALE:
            return FLPROG_TCP_DISCONNECT_STALE;
        case FLPROG_SOCKET_ERROR_IO:
            return FLPROG_TCP_DISCONNECT_IO_ERROR;
        case FLPROG_SOCKET_ERROR_NO_BACKEND:
            return FLPROG_TCP_DISCONNECT_UNSUPPORTED;
        case FLPROG_SOCKET_ERROR_OPEN:
            return FLPROG_TCP_DISCONNECT_CONNECT_FAILED;
        default:
            return FLPROG_TCP_DISCONNECT_UNKNOWN;
    }
}

void FLProgNetworkTcpClient::scheduleReconnectFromFailure()
{
    if (_restartPending || !_connectRequested)
    {
        return;
    }

    if (_clientError == FLPROG_TCP_CLIENT_ERROR_DNS ||
        _clientError == FLPROG_TCP_CLIENT_ERROR_NO_RESOLVER)
    {
        const uint8_t reason = _clientError == FLPROG_TCP_CLIENT_ERROR_DNS
            ? FLPROG_TCP_DISCONNECT_DNS
            : FLPROG_TCP_DISCONNECT_NO_RESOLVER;

        if (!_autoReconnect || (!_everConnected && !_reconnectOnInitialFailure))
        {
            _lastDisconnectReason = reason;
            return;
        }

        const uint32_t delay = _reconnectCurrentMs;
        _reconnectScheduleCount++;
        scheduleRestart(reason, delay, true);
        advanceReconnectBackoff();
        return;
    }

    if (state() != FLPROG_SOCKET_STATE_ERROR)
    {
        return;
    }

    captureBackendDiagnostics();
    const uint8_t reason = mapFailureReason();
    _lastDisconnectReason = reason;

    if (!_autoReconnect || (!_everConnected && !_reconnectOnInitialFailure))
    {
        return;
    }

    const uint32_t delay = _reconnectCurrentMs;
    _reconnectScheduleCount++;
    scheduleRestart(reason, delay, _hostnameMode);
    advanceReconnectBackoff();
}

void FLProgNetworkTcpClient::updateConnectionStatistics()
{
    const bool nowConnected = connected();

    if (nowConnected && !_wasConnected)
    {
        _connectSuccessCount++;
        _everConnected = true;
        resetReconnectBackoff();

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        if (boundTransportId() == FLPROG_TRANSPORT_WIZNET)
        {
            _wiznetBackend.clearBackendEvent();
        }
        else
#endif
        if (boundTransportId() == FLPROG_TRANSPORT_WIFI)
        {
            _wifiBackend.clearBackendEvent();
        }
        else if (boundTransportId() == FLPROG_TRANSPORT_WIFI_AP)
        {
            _wifiApBackend.clearBackendEvent();
        }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        else if (boundTransportId() == FLPROG_TRANSPORT_RMII)
        {
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
            _stm32RmiiBackend.clearBackendEvent();
#else
            _rmiiBackend.clearBackendEvent();
#endif
        }
#endif
    }
    else if (!nowConnected && _wasConnected)
    {
        _disconnectCount++;
    }

    _wasConnected = nowConnected;
}

void FLProgNetworkTcpClient::pool()
{
    if (_tcpNetwork == nullptr)
    {
        return;
    }

    _wifiBackend.setLocalAddress(_tcpNetwork->wifiIP());
    _wifiApBackend.setLocalAddress(_tcpNetwork->accessPointIP());
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _rmiiBackend.setLocalAddress(_tcpNetwork->rmiiIP());
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    _stm32RmiiBackend.setLocalAddress(_tcpNetwork->rmiiIP());
#endif

    if (_restartPending)
    {
        serviceRestart();
        return;
    }

    if (!_connectRequested)
    {
        FLProgNetworkSocket::pool();
        updateConnectionStatistics();
        return;
    }

    if (_needResolve)
    {
        dnsStep();
        if (_needResolve)
        {
            return;
        }
        if (_clientError != FLPROG_TCP_CLIENT_ERROR_NONE)
        {
            scheduleReconnectFromFailure();
            return;
        }
    }

    FLProgNetworkSocket::pool();
    updateConnectionStatistics();

    // A dynamic route may have changed between DNS completion and socket bind,
    // or after stale rebind. Re-resolve a hostname on the actual transport.
    if (
        _hostnameMode &&
        _resolvedTransportId != FLPROG_TRANSPORT_NONE &&
        boundTransportId() != FLPROG_TRANSPORT_NONE &&
        boundTransportId() != _resolvedTransportId)
    {
        scheduleRestart(FLPROG_TCP_DISCONNECT_STALE, 0, true);
        return;
    }

    if (state() == FLPROG_SOCKET_STATE_ERROR)
    {
        scheduleReconnectFromFailure();
    }
}
