/*
 * FLProg_Network — flprogStm32TcpBackend.cpp
 * Назначение: STM32-specific TCP backend adapter.
 *
 * Архитектурные заметки:
 * - Изолирует различия STM32 networking core от native TCP API.
 * - Компиляция STM32 проверяется отдельно от аппаратного W5500 gate.
 * - Верхний слой не должен содержать условные STM32 ветки.
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
#include "flprogStm32TcpBackend.h"

#if FLPROG_NETWORK_HAS_STM32_ETHERNET

#include <string.h>

bool FLProgStm32TcpClientBackend::ipIsZero(IPAddress ip)
{
    return ip[0] == 0 && ip[1] == 0 && ip[2] == 0 && ip[3] == 0;
}

void FLProgStm32TcpClientBackend::ipToLwip(IPAddress ip, ip_addr_t &out)
{
    IP_ADDR4(&out, ip[0], ip[1], ip[2], ip[3]);
}

void FLProgStm32TcpClientBackend::destroyAccepted(tcp_struct *client)
{
    if (client == nullptr)
    {
        return;
    }

    if (client->pcb != nullptr)
    {
        tcp_connection_close(client->pcb, client);
    }

    if (client->data.p != nullptr)
    {
        client->data.p = stm32_free_data(client->data.p);
        client->data.available = 0;
    }

    mem_free(client);
}

void FLProgStm32TcpClientBackend::resetConnection()
{
    if (_tcp != nullptr)
    {
        destroyAccepted(_tcp);
        _tcp = nullptr;
    }

    _connecting = false;
    _connected = false;
    _localPort = 0;
    _started = 0;
    _keepAliveApplied = false;
}

void FLProgStm32TcpClientBackend::applyTcpOptions()
{
    _lastOptionError = 0;
    _keepAliveApplied = false;

    if (_tcp == nullptr || _tcp->pcb == nullptr)
    {
        return;
    }

    if (_noDelay)
    {
        tcp_nagle_disable(_tcp->pcb);
    }
    else
    {
        tcp_nagle_enable(_tcp->pcb);
    }

#if LWIP_TCP_KEEPALIVE
    if (_keepAliveEnabled)
    {
        ip_set_option(_tcp->pcb, SOF_KEEPALIVE);
        _tcp->pcb->keep_idle =
            (_keepAliveIdleSeconds == 0 ? 1UL : _keepAliveIdleSeconds) * 1000UL;
        _tcp->pcb->keep_intvl =
            (_keepAliveIntervalSeconds == 0 ? 1UL : _keepAliveIntervalSeconds) * 1000UL;
        _tcp->pcb->keep_cnt =
            _keepAliveProbeCount == 0 ? 1 : _keepAliveProbeCount;
        _keepAliveApplied = true;
    }
    else
    {
        ip_reset_option(_tcp->pcb, SOF_KEEPALIVE);
    }
#else
    if (_keepAliveEnabled)
    {
        _lastOptionError = ERR_VAL;
    }
#endif
}

void FLProgStm32TcpClientBackend::setNoDelay(bool value)
{
    _noDelay = value;
    applyTcpOptions();
}

void FLProgStm32TcpClientBackend::setKeepAlive(
    bool enabled,
    uint32_t idleSeconds,
    uint32_t intervalSeconds,
    uint8_t probeCount)
{
    _keepAliveEnabled = enabled;
    _keepAliveIdleSeconds = idleSeconds;
    _keepAliveIntervalSeconds = intervalSeconds;
    _keepAliveProbeCount = probeCount == 0 ? 1 : probeCount;
    applyTcpOptions();
}

bool FLProgStm32TcpClientBackend::adoptAccepted(
    tcp_struct *client,
    uint16_t localPortValue,
    IPAddress remoteIpValue,
    uint16_t remotePortValue)
{
    if (
        client == nullptr ||
        client->pcb == nullptr ||
        remotePortValue == 0 ||
        ipIsZero(remoteIpValue))
    {
        return false;
    }

    resetConnection();

    _tcp = client;
    _endpoint.localPort = localPortValue;
    _endpoint.remoteIP = remoteIpValue;
    _endpoint.remotePort = remotePortValue;
    _localPort = localPortValue;
    _timeout = 0;
    _started = millis();
    _connecting = false;
    _connected =
        client->state == TCP_ACCEPTED || client->state == TCP_CONNECTED;
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
    applyTcpOptions();
    return _connected;
}

uint8_t FLProgStm32TcpClientBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    (void)lease;

    if (
        protocol != FLPROG_SOCKET_PROTOCOL_STREAM ||
        endpoint.remotePort == 0 ||
        ipIsZero(endpoint.remoteIP))
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_connected)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_tcp != nullptr)
    {
        return pool();
    }

    _endpoint = endpoint;
    _timeout = timeout;
    _started = millis();
    _lastNativeError = 0;
    _lastOptionError = 0;
    _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;

    _tcp = (tcp_struct *)mem_malloc(sizeof(tcp_struct));
    if (_tcp == nullptr)
    {
        _lastNativeError = ERR_MEM;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    memset(_tcp, 0, sizeof(tcp_struct));
    _tcp->pcb = tcp_new();
    _tcp->state = TCP_NONE;
    if (_tcp->pcb == nullptr)
    {
        _lastNativeError = ERR_MEM;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetConnection();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    ip_addr_t localAddress;
    const ip_addr_t *localPtr = IP_ADDR_ANY;
    if (!ipIsZero(_localAddress))
    {
        ipToLwip(_localAddress, localAddress);
        localPtr = &localAddress;
    }

    err_t err = tcp_bind(_tcp->pcb, localPtr, endpoint.localPort);
    if (err != ERR_OK)
    {
        _lastNativeError = err;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetConnection();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _localPort = _tcp->pcb->local_port;
    tcp_arg(_tcp->pcb, _tcp);
    applyTcpOptions();

    ip_addr_t remoteAddress;
    ipToLwip(endpoint.remoteIP, remoteAddress);
    err = tcp_connect(
        _tcp->pcb,
        &remoteAddress,
        endpoint.remotePort,
        tcp_connected_callback);

    if (err != ERR_OK)
    {
        _lastNativeError = err;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetConnection();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _connecting = true;
    return FLPROG_SOCKET_RESULT_WAIT;
}

uint8_t FLProgStm32TcpClientBackend::pool()
{
    Ethernet.schedule();

    if (_tcp == nullptr)
    {
        return _connected
            ? FLPROG_SOCKET_RESULT_SUCCESS
            : FLPROG_SOCKET_RESULT_WAIT;
    }

    if (_tcp->state == TCP_CONNECTED || _tcp->state == TCP_ACCEPTED)
    {
        _connecting = false;
        _connected = true;
        if (_tcp->pcb != nullptr)
        {
            _localPort = _tcp->pcb->local_port;
        }
        applyTcpOptions();
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_tcp->state == TCP_CLOSING || _tcp->pcb == nullptr)
    {
        _lastNativeError = ERR_CLSD;
        _lastBackendEvent = _connected
            ? FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED
            : FLPROG_TCP_BACKEND_EVENT_CONNECT_FAILED;
        resetConnection();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (
        _connecting &&
        _timeout > 0 &&
        (uint32_t)(millis() - _started) >= _timeout)
    {
        _lastNativeError = ERR_TIMEOUT;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT;
        resetConnection();
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    return FLPROG_SOCKET_RESULT_WAIT;
}

uint8_t FLProgStm32TcpClientBackend::close()
{
    resetConnection();
    return FLPROG_SOCKET_RESULT_SUCCESS;
}

int FLProgStm32TcpClientBackend::available()
{
    if (!_connected || _tcp == nullptr)
    {
        return 0;
    }
    return (int)_tcp->data.available;
}

int FLProgStm32TcpClientBackend::read()
{
    uint8_t value = 0;
    const int count = read(&value, 1);
    return count == 1 ? value : -1;
}

int FLProgStm32TcpClientBackend::read(uint8_t *buffer, size_t length)
{
    if (
        !_connected ||
        _tcp == nullptr ||
        buffer == nullptr ||
        length == 0)
    {
        return 0;
    }

    return (int)stm32_get_data(&_tcp->data, buffer, length);
}

int FLProgStm32TcpClientBackend::peek()
{
    if (
        !_connected ||
        _tcp == nullptr ||
        _tcp->data.p == nullptr ||
        _tcp->data.available == 0)
    {
        return -1;
    }

    const uint16_t offset =
        (uint16_t)(_tcp->data.p->tot_len - _tcp->data.available);
    return (int)pbuf_get_at(_tcp->data.p, offset);
}

size_t FLProgStm32TcpClientBackend::write(
    const uint8_t *buffer,
    size_t length)
{
    if (
        !_connected ||
        _tcp == nullptr ||
        _tcp->pcb == nullptr ||
        buffer == nullptr ||
        length == 0)
    {
        return 0;
    }

    const u16_t room = tcp_sndbuf(_tcp->pcb);
    if (room == 0)
    {
        return 0;
    }

    size_t chunk = length;
    if (chunk > room) chunk = room;
    if (chunk > 0xFFFFU) chunk = 0xFFFFU;

    const err_t err = tcp_write(
        _tcp->pcb,
        buffer,
        (u16_t)chunk,
        TCP_WRITE_FLAG_COPY);

    if (err == ERR_MEM)
    {
        return 0;
    }

    if (err != ERR_OK)
    {
        _lastNativeError = err;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        resetConnection();
        return 0;
    }

    const err_t out = tcp_output(_tcp->pcb);
    if (out != ERR_OK && out != ERR_MEM)
    {
        _lastNativeError = out;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_IO_ERROR;
        resetConnection();
        return 0;
    }

    return chunk;
}

void FLProgStm32TcpClientBackend::flush()
{
    if (_tcp != nullptr && _tcp->pcb != nullptr)
    {
        tcp_output(_tcp->pcb);
    }
}

FLProgStm32TcpServerListener::FLProgStm32TcpServerListener()
{
    for (uint8_t i = 0; i < FLPROG_TCP_SERVER_MAX_CLIENTS; i++)
    {
        _pending[i] = nullptr;
    }
}

IPAddress FLProgStm32TcpServerListener::lwipToIp(const ip_addr_t &ip)
{
    const ip4_addr_t *v4 = ip_2_ip4(&ip);
    return IPAddress(
        (uint8_t)ip4_addr1(v4),
        (uint8_t)ip4_addr2(v4),
        (uint8_t)ip4_addr3(v4),
        (uint8_t)ip4_addr4(v4));
}

err_t FLProgStm32TcpServerListener::acceptCallback(
    void *arg,
    tcp_pcb *newpcb,
    err_t err)
{
    FLProgStm32TcpServerListener *self =
        static_cast<FLProgStm32TcpServerListener *>(arg);

    if (self == nullptr || newpcb == nullptr || err != ERR_OK)
    {
        if (newpcb != nullptr) tcp_abort(newpcb);
        return ERR_ABRT;
    }

    int slot = -1;
    for (uint8_t i = 0; i < FLPROG_TCP_SERVER_MAX_CLIENTS; i++)
    {
        if (self->_pending[i] == nullptr)
        {
            slot = i;
            break;
        }
    }

    if (slot < 0)
    {
        tcp_abort(newpcb);
        return ERR_ABRT;
    }

    tcp_struct *client = (tcp_struct *)mem_malloc(sizeof(tcp_struct));
    if (client == nullptr)
    {
        tcp_abort(newpcb);
        return ERR_ABRT;
    }

    memset(client, 0, sizeof(tcp_struct));
    client->pcb = newpcb;
    client->state = TCP_CONNECTED;
    tcp_arg(newpcb, client);

    // Reuse STM32Ethernet's callback installer. It keeps incoming pbuf data in
    // tcp_struct::data, exactly the representation consumed by our backend.
    const err_t installed = tcp_connected_callback(client, newpcb, ERR_OK);
    if (installed != ERR_OK)
    {
        FLProgStm32TcpClientBackend::destroyAccepted(client);
        return installed;
    }

    client->state = TCP_ACCEPTED;
    self->_pending[slot] = client;
    return ERR_OK;
}

bool FLProgStm32TcpServerListener::begin(
    IPAddress localIp,
    uint16_t port,
    uint8_t backlog)
{
    if (port == 0)
    {
        _lastNativeError = ERR_ARG;
        return false;
    }

    if (_listener != nullptr)
    {
        return _localPort == port;
    }

    tcp_pcb *pcb = tcp_new();
    if (pcb == nullptr)
    {
        _lastNativeError = ERR_MEM;
        return false;
    }

    ip_addr_t localAddress;
    const ip_addr_t *localPtr = IP_ADDR_ANY;
    if (!(localIp == IPAddress(0, 0, 0, 0)))
    {
        IP_ADDR4(
            &localAddress,
            localIp[0], localIp[1], localIp[2], localIp[3]);
        localPtr = &localAddress;
    }

    err_t err = tcp_bind(pcb, localPtr, port);
    if (err != ERR_OK)
    {
        tcp_close(pcb);
        _lastNativeError = err;
        return false;
    }

    // STM32Ethernet itself uses tcp_listen() and its supported LwIP versions
    // do not expose a stable backlog acknowledgement API through Arduino.
    // FLProg_Network still bounds pending accepts with its own fixed queue.
    (void)backlog;
    tcp_pcb *listener = tcp_listen(pcb);
    if (listener == nullptr)
    {
        tcp_abort(pcb);
        _lastNativeError = ERR_MEM;
        return false;
    }

    _listener = listener;
    _localPort = port;
    _lastNativeError = 0;
    tcp_arg(_listener, this);
    tcp_accept(_listener, &FLProgStm32TcpServerListener::acceptCallback);
    return true;
}

void FLProgStm32TcpServerListener::clearPending()
{
    for (uint8_t i = 0; i < FLPROG_TCP_SERVER_MAX_CLIENTS; i++)
    {
        if (_pending[i] != nullptr)
        {
            FLProgStm32TcpClientBackend::destroyAccepted(_pending[i]);
            _pending[i] = nullptr;
        }
    }
}

void FLProgStm32TcpServerListener::close()
{
    clearPending();

    if (_listener != nullptr)
    {
        tcp_arg(_listener, nullptr);
        tcp_accept(_listener, nullptr);
        if (tcp_close(_listener) != ERR_OK)
        {
            tcp_abort(_listener);
        }
        _listener = nullptr;
    }

    _localPort = 0;
}

int FLProgStm32TcpServerListener::acceptOne(
    tcp_struct *&client,
    IPAddress &remoteIp,
    uint16_t &remotePort)
{
    client = nullptr;
    remoteIp = IPAddress(0, 0, 0, 0);
    remotePort = 0;

    if (_listener == nullptr)
    {
        _lastNativeError = ERR_CLSD;
        return -1;
    }

    Ethernet.schedule();

    for (uint8_t i = 0; i < FLPROG_TCP_SERVER_MAX_CLIENTS; i++)
    {
        tcp_struct *candidate = _pending[i];
        if (candidate == nullptr)
        {
            continue;
        }

        _pending[i] = nullptr;

        if (candidate->pcb == nullptr)
        {
            FLProgStm32TcpClientBackend::destroyAccepted(candidate);
            continue;
        }

        client = candidate;
        remoteIp = lwipToIp(candidate->pcb->remote_ip);
        remotePort = candidate->pcb->remote_port;
        return 1;
    }

    return 0;
}

#endif // FLPROG_NETWORK_HAS_STM32_ETHERNET
