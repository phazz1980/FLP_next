/*
 * FLProg_Network — flprogStm32UdpBackend.cpp
 * Назначение: STM32-specific UDP backend adapter.
 *
 * Архитектурные заметки:
 * - Изолирует API UDP конкретного STM32 core.
 * - Packet/read/write семантика должна совпадать с остальными backend-ами.
 * - Platform-specific код не должен протекать в legacy или service слой.
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
#include "flprogNetworkUdp.h"

#if FLPROG_NETWORK_HAS_STM32_ETHERNET

bool FLProgStm32UdpBackend::configureMulticast(
    bool enabled,
    IPAddress group,
    uint16_t port)
{
    if (_open &&
        (enabled != _multicastEnabled ||
         group != _multicastGroup ||
         port != _multicastPort))
    {
        // Reopening is intentionally left to the managed socket state machine.
        return false;
    }

    _multicastEnabled = enabled;
    _multicastGroup = group;
    _multicastPort = port;
    return true;
}

uint8_t FLProgStm32UdpBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    (void)lease;
    (void)timeout;

    if (protocol != FLPROG_SOCKET_PROTOCOL_DATAGRAM)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_open)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    _localPort = endpoint.localPort;
    int result = 0;
    if (_multicastEnabled)
    {
        if (_multicastPort == 0 ||
            _multicastGroup == IPAddress(0, 0, 0, 0))
        {
            return FLPROG_SOCKET_RESULT_UNSUPPORTED;
        }
        _localPort = _multicastPort;
        result = _udp.beginMulticast(_multicastGroup, _multicastPort);
    }
    else
    {
        result = _udp.begin(_localPort);
    }

    if (result == 0)
    {
        _localPort = 0;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _open = true;
    _packetSize = 0;
    return FLPROG_SOCKET_RESULT_SUCCESS;
}

uint8_t FLProgStm32UdpBackend::pool()
{
    Ethernet.schedule();
    return _open ? FLPROG_SOCKET_RESULT_SUCCESS : FLPROG_SOCKET_RESULT_WAIT;
}

uint8_t FLProgStm32UdpBackend::close()
{
    if (_open)
    {
        _udp.stop();
    }
    _open = false;
    _localPort = 0;
    _packetSize = 0;
    return FLPROG_SOCKET_RESULT_SUCCESS;
}

int FLProgStm32UdpBackend::beginPacket(IPAddress ip, uint16_t port)
{
    return _open ? _udp.beginPacket(ip, port) : 0;
}

size_t FLProgStm32UdpBackend::packetWrite(
    const uint8_t *buffer,
    size_t length)
{
    return _open ? _udp.write(buffer, length) : 0;
}

int FLProgStm32UdpBackend::endPacket()
{
    return _open ? _udp.endPacket() : 0;
}

int FLProgStm32UdpBackend::parsePacket()
{
    _packetSize = _open ? _udp.parsePacket() : 0;
    return _packetSize;
}

int FLProgStm32UdpBackend::packetAvailable()
{
    return _open ? _udp.available() : 0;
}

int FLProgStm32UdpBackend::packetRead()
{
    return _open ? _udp.read() : -1;
}

int FLProgStm32UdpBackend::packetRead(uint8_t *buffer, size_t length)
{
    return _open ? _udp.read(buffer, length) : 0;
}

int FLProgStm32UdpBackend::packetPeek()
{
    return _open ? _udp.peek() : -1;
}

void FLProgStm32UdpBackend::packetFlush()
{
    if (_open)
    {
        _udp.flush();
    }
}

IPAddress FLProgStm32UdpBackend::packetRemoteIP() const
{
    return _open ? _udp.remoteIP() : IPAddress(0, 0, 0, 0);
}

uint16_t FLProgStm32UdpBackend::packetRemotePort() const
{
    return _open ? _udp.remotePort() : 0;
}

#endif // FLPROG_NETWORK_HAS_STM32_ETHERNET
