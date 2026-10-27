/*
 * FLProg_Network — flprogNetworkUdp.cpp
 * Назначение: Native UDP transport FLProg_Network.
 *
 * Архитектурные заметки:
 * - Поддерживает unicast/multicast и пакетную семантику beginPacket/write/endPacket.
 * - Маршрут и socket lifecycle делегируются A3; legacy UDP adapter не должен обходить этот слой.
 * - Multicast MAC/helper код должен компилироваться только там, где он реально нужен.
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
#include "../flprogNetwork.h"

#include <stdlib.h>
#include <string.h>

#if FLPROG_NETWORK_HAS_POSIX_UDP
#include <lwip/sockets.h>
#include <fcntl.h>
#include <errno.h>
#endif

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

static bool flprogUdpIpIsZero(IPAddress ip)
{
    return
        ip[0] == 0 &&
        ip[1] == 0 &&
        ip[2] == 0 &&
        ip[3] == 0;
}

static bool flprogUdpIpIsMulticast(IPAddress ip)
{
    return ip[0] >= 224 && ip[0] <= 239;
}

static void flprogUdpMulticastMac(IPAddress ip, uint8_t *mac)
{
    mac[0] = 0x01;
    mac[1] = 0x00;
    mac[2] = 0x5E;
    mac[3] = ip[1] & 0x7F;
    mac[4] = ip[2];
    mac[5] = ip[3];
}

#if FLPROG_NETWORK_HAS_POSIX_UDP
static uint32_t flprogUdpIpv4NetworkOrder(IPAddress ip)
{
    const uint32_t hostOrder =
        ((uint32_t)ip[0] << 24) |
        ((uint32_t)ip[1] << 16) |
        ((uint32_t)ip[2] << 8) |
        (uint32_t)ip[3];

    return htonl(hostOrder);
}

static IPAddress flprogUdpFromNetworkOrder(uint32_t value)
{
    const uint32_t hostOrder = ntohl(value);

    return IPAddress(
        (uint8_t)(hostOrder >> 24),
        (uint8_t)(hostOrder >> 16),
        (uint8_t)(hostOrder >> 8),
        (uint8_t)hostOrder
    );
}
#endif

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
// -----------------------------------------------------------------------------
// FLProgWiznetUdpBackend
// -----------------------------------------------------------------------------

bool FLProgWiznetUdpBackend::validSocket() const
{
    return
        _hardware != nullptr &&
        _hardware->isInit() &&
        _socket >= 0 &&
        _socket < _hardware->maxSocketNum();
}

bool FLProgWiznetUdpBackend::spiWasDeferred()
{
    if (_hardware == nullptr)
    {
        return true;
    }

    const uint8_t status =
        _hardware->lastSpiStatus();

    if (status == FLPROG_SPI_STATUS_OK)
    {
        return false;
    }

    if (status == FLPROG_SPI_STATUS_BUSY)
    {
        _deferredCount++;
    }

    return true;
}

uint8_t FLProgWiznetUdpBackend::read8(
    uint16_t address,
    bool &ok)
{
    ok = false;

    if (!validSocket())
    {
        return 0;
    }

    const uint8_t value =
        _hardware->readSocket(
            (uint8_t)_socket,
            address
        );

    if (spiWasDeferred())
    {
        return 0;
    }

    ok = true;
    return value;
}

uint16_t FLProgWiznetUdpBackend::read16(
    uint16_t address,
    bool &ok)
{
    ok = false;

    if (!validSocket())
    {
        return 0;
    }

    uint8_t data[2] = {0, 0};

    _hardware->readSocket(
        (uint8_t)_socket,
        address,
        data,
        2
    );

    if (spiWasDeferred())
    {
        return 0;
    }

    ok = true;

    return
        ((uint16_t)data[0] << 8) |
        data[1];
}

bool FLProgWiznetUdpBackend::write8(
    uint16_t address,
    uint8_t value)
{
    if (!validSocket())
    {
        return false;
    }

    _hardware->writeSocket(
        (uint8_t)_socket,
        address,
        value
    );

    return !spiWasDeferred();
}

bool FLProgWiznetUdpBackend::write16(
    uint16_t address,
    uint16_t value)
{
    uint8_t data[2] = {
        (uint8_t)(value >> 8),
        (uint8_t)value
    };

    return writeBlock(address, data, 2);
}

bool FLProgWiznetUdpBackend::writeBlock(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (
        !validSocket() ||
        buffer == nullptr ||
        length == 0
    )
    {
        return false;
    }

    _hardware->writeSocket(
        (uint8_t)_socket,
        address,
        buffer,
        length
    );

    return !spiWasDeferred();
}

bool FLProgWiznetUdpBackend::issueCommand(
    uint8_t command)
{
    bool ok = false;

    const uint8_t current =
        read8(FLPROG_WIZNET_SN_CR, ok);

    if (!ok || current != 0)
    {
        return false;
    }

    return write8(
        FLPROG_WIZNET_SN_CR,
        command
    );
}

bool FLProgWiznetUdpBackend::commandFinished(
    bool &ok)
{
    const uint8_t value =
        read8(FLPROG_WIZNET_SN_CR, ok);

    return ok && value == 0;
}

void FLProgWiznetUdpBackend::resetPacketState()
{
    _txActive = false;
    _txWritePointer = 0;
    _txCapacity = 0;
    _txLength = 0;
    _txRemoteIP = IPAddress(0, 0, 0, 0);
    _txRemotePort = 0;

    _rxActive = false;
    _rxPacketStart = 0;
    _rxReadPointer = 0;
    _rxPacketEnd = 0;
    _rxPacketSize = 0;
    _rxRemaining = 0;
    _rxRemoteIP = IPAddress(0, 0, 0, 0);
    _rxRemotePort = 0;
}

void FLProgWiznetUdpBackend::resetSocketState()
{
    _openConfigured = false;
    _openCommandSent = false;
    _closeCommandSent = false;
    _sendPending = false;
    _recvCommandPending = false;

    _socketStatus = FLPROG_WIZNET_SOCK_CLOSED;

    _effectiveLocalPort = 0;
    _started = 0;

    resetPacketState();
}

bool FLProgWiznetUdpBackend::configureOpen()
{
    if (!validSocket())
    {
        return false;
    }

    bool ok = false;

    const uint8_t status =
        read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return false;
    }

    if (status != FLPROG_WIZNET_SOCK_CLOSED)
    {
        if (!issueCommand(FLPROG_WIZNET_CMD_CLOSE))
        {
            return false;
        }

        return false;
    }

    // BCASTB must be present in Sn_MR before OPEN. The hardware helper changes
    // bit 6 only for W5500 UDP/MACRAW; W5100/W5100S/W5200 remain untouched.
    uint8_t mode = FLPROG_WIZNET_MODE_UDP;

    if (_multicastEnabled)
    {
        if (
            !flprogUdpIpIsMulticast(_multicastGroup) ||
            _multicastPort == 0
        )
        {
            return false;
        }

        uint8_t multicastMac[6];
        flprogUdpMulticastMac(_multicastGroup, multicastMac);

        const uint8_t multicastIp[4] = {
            _multicastGroup[0], _multicastGroup[1],
            _multicastGroup[2], _multicastGroup[3]
        };

        if (
            !writeBlock(FLPROG_WIZNET_SN_DHAR, multicastMac, 6) ||
            !writeBlock(FLPROG_WIZNET_SN_DIPR, multicastIp, 4) ||
            !write16(FLPROG_WIZNET_SN_DPORT, _multicastPort)
        )
        {
            return false;
        }

        // Sn_MR.MULTI must be set before OPEN. Keep IGMPv2 (bit 5 clear).
        mode |= 0x80;
    }

    mode =
        _hardware->prepareSocketMode(
            mode,
            _broadcastBlocking
        );

    if (!write8(FLPROG_WIZNET_SN_MR, mode))
    {
        return false;
    }

    if (!write16(
        FLPROG_WIZNET_SN_PORT,
        _effectiveLocalPort))
    {
        return false;
    }

    _openConfigured = true;
    return true;
}

uint8_t FLProgWiznetUdpBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    if (
        _hardware == nullptr ||
        !_hardware->isInit() ||
        lease.transportId != FLPROG_TRANSPORT_WIZNET ||
        lease.nativeSocket == FLPROG_SOCKET_NATIVE_NONE
    )
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (protocol != FLPROG_SOCKET_PROTOCOL_DATAGRAM)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_socket != lease.nativeSocket)
    {
        _socket = lease.nativeSocket;
        resetSocketState();
    }

    _endpoint = endpoint;

    _effectiveLocalPort = endpoint.localPort;

    if (_effectiveLocalPort == 0)
    {
        _effectiveLocalPort =
            (uint16_t)(
                49152U +
                (uint16_t)_socket
            );
    }

    _timeout = timeout;

    if (_started == 0)
    {
        _started = millis();
    }

    if (!_openConfigured)
    {
        if (!configureOpen())
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }
    }

    if (!_openCommandSent)
    {
        if (!issueCommand(FLPROG_WIZNET_CMD_OPEN))
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }

        _openCommandSent = true;
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    return pollOpen()
        ? FLPROG_SOCKET_RESULT_SUCCESS
        : FLPROG_SOCKET_RESULT_WAIT;
}

bool FLProgWiznetUdpBackend::pollOpen()
{
    bool ok = false;

    if (!commandFinished(ok) || !ok)
    {
        return false;
    }

    _socketStatus =
        read8(FLPROG_WIZNET_SN_SR, ok);

    return
        ok &&
        _socketStatus == FLPROG_WIZNET_SOCK_UDP;
}

void FLProgWiznetUdpBackend::pollInterrupts()
{
    if (!validSocket())
    {
        return;
    }

    bool ok = false;

    const uint8_t interrupts =
        read8(FLPROG_WIZNET_SN_IR, ok);

    if (!ok)
    {
        return;
    }

    const uint8_t clearMask =
        interrupts &
        (
            FLPROG_WIZNET_IR_SEND_OK |
            FLPROG_WIZNET_IR_TIMEOUT
        );

    if (clearMask == 0)
    {
        return;
    }

    if (!write8(
        FLPROG_WIZNET_SN_IR,
        clearMask))
    {
        return;
    }

    if (
        clearMask &
        FLPROG_WIZNET_IR_SEND_OK
    )
    {
        _sendPending = false;
    }

    if (
        clearMask &
        FLPROG_WIZNET_IR_TIMEOUT
    )
    {
        _sendPending = false;
    }
}

uint8_t FLProgWiznetUdpBackend::pool()
{
    if (!validSocket())
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (_recvCommandPending)
    {
        if (issueCommand(
            FLPROG_WIZNET_CMD_RECV))
        {
            _recvCommandPending = false;
        }
    }

    bool ok = false;

    _socketStatus =
        read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    pollInterrupts();

    if (
        _timeout > 0 &&
        _started > 0 &&
        !isOpen() &&
        (uint32_t)(millis() - _started) >=
            _timeout
    )
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

uint8_t FLProgWiznetUdpBackend::close()
{
    if (!validSocket())
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        resetSocketState();
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    bool ok = false;

    _socketStatus =
        read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSED &&
        !_closeCommandSent
    )
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        resetSocketState();
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (!_closeCommandSent)
    {
        if (!issueCommand(
            FLPROG_WIZNET_CMD_CLOSE))
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }

        _closeCommandSent = true;
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (!commandFinished(ok) || !ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    _socketStatus =
        read8(FLPROG_WIZNET_SN_SR, ok);

    if (
        !ok ||
        _socketStatus !=
            FLPROG_WIZNET_SOCK_CLOSED
    )
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    _socket = FLPROG_SOCKET_NATIVE_NONE;
    resetSocketState();

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

bool FLProgWiznetUdpBackend::isOpen() const
{
    return
        _socket >= 0 &&
        _socketStatus ==
            FLPROG_WIZNET_SOCK_UDP;
}

int FLProgWiznetUdpBackend::beginPacket(
    IPAddress ip,
    uint16_t port)
{
    if (
        !isOpen() ||
        _sendPending ||
        _txActive ||
        flprogUdpIpIsZero(ip) ||
        port == 0 ||
        (
            _multicastEnabled &&
            (ip != _multicastGroup || port != _multicastPort)
        )
    )
    {
        return 0;
    }

    bool ok = false;

    uint16_t freeSize =
        read16(
            FLPROG_WIZNET_SN_TX_FSR,
            ok
        );

    if (!ok || freeSize == 0)
    {
        return 0;
    }

    if (freeSize > FLPROG_UDP_MAX_PACKET_SIZE)
    {
        freeSize =
            FLPROG_UDP_MAX_PACKET_SIZE;
    }

    const uint16_t writePointer =
        read16(
            FLPROG_WIZNET_SN_TX_WR,
            ok
        );

    if (!ok)
    {
        return 0;
    }

    _txActive = true;
    _txWritePointer = writePointer;
    _txCapacity = freeSize;
    _txLength = 0;
    _txRemoteIP = ip;
    _txRemotePort = port;

    return 1;
}

size_t FLProgWiznetUdpBackend::packetWrite(
    const uint8_t *buffer,
    size_t length)
{
    if (
        !_txActive ||
        buffer == nullptr ||
        length == 0
    )
    {
        return 0;
    }

    uint16_t availableSpace =
        (uint16_t)(
            _txCapacity -
            _txLength
        );

    if (availableSpace == 0)
    {
        return 0;
    }

    uint16_t amount = (uint16_t)length;

    if (amount > availableSpace)
    {
        amount = availableSpace;
    }

    _hardware->writeTxBuffer(
        (uint8_t)_socket,
        _txWritePointer,
        buffer,
        amount
    );

    if (spiWasDeferred())
    {
        return 0;
    }

    _txWritePointer =
        (uint16_t)(
            _txWritePointer +
            amount
        );

    _txLength =
        (uint16_t)(
            _txLength +
            amount
        );

    return amount;
}

int FLProgWiznetUdpBackend::endPacket()
{
    if (
        !_txActive ||
        _sendPending
    )
    {
        return 0;
    }

    if (!_multicastEnabled)
    {
        const uint8_t destinationIp[4] = {
            _txRemoteIP[0],
            _txRemoteIP[1],
            _txRemoteIP[2],
            _txRemoteIP[3]
        };

        if (!writeBlock(
            FLPROG_WIZNET_SN_DIPR,
            destinationIp,
            4))
        {
            return 0;
        }

        if (!write16(
            FLPROG_WIZNET_SN_DPORT,
            _txRemotePort))
        {
            return 0;
        }
    }

    if (!write16(
        FLPROG_WIZNET_SN_TX_WR,
        _txWritePointer))
    {
        return 0;
    }

    if (!issueCommand(
        FLPROG_WIZNET_CMD_SEND))
    {
        return 0;
    }

    _txActive = false;
    _sendPending = true;

    return 1;
}

bool FLProgWiznetUdpBackend::commitRxPacket()
{
    if (!_rxActive)
    {
        return true;
    }

    if (!write16(
        FLPROG_WIZNET_SN_RX_RD,
        _rxPacketEnd))
    {
        return false;
    }

    _rxActive = false;
    _rxRemaining = 0;

    if (issueCommand(
        FLPROG_WIZNET_CMD_RECV))
    {
        _recvCommandPending = false;
    }
    else
    {
        _recvCommandPending = true;
    }

    return true;
}

int FLProgWiznetUdpBackend::parsePacket()
{
    if (!isOpen())
    {
        return 0;
    }

    if (_recvCommandPending)
    {
        if (issueCommand(
            FLPROG_WIZNET_CMD_RECV))
        {
            _recvCommandPending = false;
        }
        else
        {
            return 0;
        }
    }

    // Arduino-style parsePacket discards unread bytes of the previous packet.
    if (_rxActive)
    {
        if (!commitRxPacket())
        {
            return 0;
        }
    }

    bool ok = false;

    const uint16_t received =
        read16(
            FLPROG_WIZNET_SN_RX_RSR,
            ok
        );

    if (!ok || received < 8)
    {
        return 0;
    }

    const uint16_t readPointer =
        read16(
            FLPROG_WIZNET_SN_RX_RD,
            ok
        );

    if (!ok)
    {
        return 0;
    }

    uint8_t header[8] =
        {0, 0, 0, 0, 0, 0, 0, 0};

    _hardware->readRxBuffer(
        (uint8_t)_socket,
        readPointer,
        header,
        8
    );

    if (spiWasDeferred())
    {
        return 0;
    }

    const uint16_t payloadLength =
        ((uint16_t)header[6] << 8) |
        header[7];

    if (
        received <
        (uint16_t)(8U + payloadLength)
    )
    {
        return 0;
    }

    _rxRemoteIP =
        IPAddress(
            header[0],
            header[1],
            header[2],
            header[3]
        );

    _rxRemotePort =
        ((uint16_t)header[4] << 8) |
        header[5];

    _rxPacketStart =
        (uint16_t)(
            readPointer + 8U
        );

    _rxReadPointer = _rxPacketStart;

    _rxPacketEnd =
        (uint16_t)(
            _rxPacketStart +
            payloadLength
        );

    _rxPacketSize = payloadLength;
    _rxRemaining = payloadLength;
    _rxActive = true;

    // Zero-length UDP datagrams are valid. The Arduino-style return value is
    // still zero, but remoteIP()/remotePort() are updated.
    if (payloadLength == 0)
    {
        commitRxPacket();
    }

    return payloadLength;
}

int FLProgWiznetUdpBackend::packetAvailable()
{
    return _rxActive
        ? (int)_rxRemaining
        : 0;
}

int FLProgWiznetUdpBackend::readCurrentPacket(
    uint8_t *buffer,
    size_t length,
    bool advance)
{
    if (
        !_rxActive ||
        buffer == nullptr ||
        length == 0 ||
        _rxRemaining == 0
    )
    {
        return 0;
    }

    uint16_t amount = (uint16_t)length;

    if (amount > _rxRemaining)
    {
        amount = _rxRemaining;
    }

    _hardware->readRxBuffer(
        (uint8_t)_socket,
        _rxReadPointer,
        buffer,
        amount
    );

    if (spiWasDeferred())
    {
        return 0;
    }

    if (!advance)
    {
        return amount;
    }

    _rxReadPointer =
        (uint16_t)(
            _rxReadPointer +
            amount
        );

    _rxRemaining =
        (uint16_t)(
            _rxRemaining -
            amount
        );

    if (_rxRemaining == 0)
    {
        commitRxPacket();
    }

    return amount;
}

int FLProgWiznetUdpBackend::packetRead()
{
    uint8_t value = 0;

    const int count =
        readCurrentPacket(
            &value,
            1,
            true
        );

    return count == 1
        ? value
        : -1;
}

int FLProgWiznetUdpBackend::packetRead(
    uint8_t *buffer,
    size_t length)
{
    return readCurrentPacket(
        buffer,
        length,
        true
    );
}

int FLProgWiznetUdpBackend::packetPeek()
{
    uint8_t value = 0;

    const int count =
        readCurrentPacket(
            &value,
            1,
            false
        );

    return count == 1
        ? value
        : -1;
}

void FLProgWiznetUdpBackend::packetFlush()
{
    if (_rxActive)
    {
        commitRxPacket();
    }
}

#endif // !FLPROG_NETWORK_PROFILE_WIFI_ONLY

// -----------------------------------------------------------------------------
// FLProgPosixUdpBackend
// -----------------------------------------------------------------------------

FLProgPosixUdpBackend::~FLProgPosixUdpBackend()
{
    close();
}

bool FLProgPosixUdpBackend::ensureTxBuffer()
{
    if (_txBuffer != nullptr)
    {
        return true;
    }

    _txBuffer =
        (uint8_t *)malloc(
            FLPROG_UDP_MAX_PACKET_SIZE
        );

    return _txBuffer != nullptr;
}

bool FLProgPosixUdpBackend::ensureRxBuffer()
{
    if (_rxBuffer != nullptr)
    {
        return true;
    }

    _rxBuffer =
        (uint8_t *)malloc(
            FLPROG_UDP_MAX_PACKET_SIZE
        );

    return _rxBuffer != nullptr;
}

void FLProgPosixUdpBackend::freeBuffers()
{
    if (_txBuffer != nullptr)
    {
        free(_txBuffer);
        _txBuffer = nullptr;
    }

    if (_rxBuffer != nullptr)
    {
        free(_rxBuffer);
        _rxBuffer = nullptr;
    }

    _txLength = 0;
    _txActive = false;
    clearRxPacket();
}

void FLProgPosixUdpBackend::clearRxPacket()
{
    _rxLength = 0;
    _rxPosition = 0;
    _rxRemoteIP =
        IPAddress(0, 0, 0, 0);
    _rxRemotePort = 0;
}

uint8_t FLProgPosixUdpBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    (void)lease;
    (void)timeout;

    if (
        protocol != FLPROG_SOCKET_PROTOCOL_DATAGRAM ||
        (_transportId != FLPROG_TRANSPORT_WIFI &&
         _transportId != FLPROG_TRANSPORT_WIFI_AP &&
         _transportId != FLPROG_TRANSPORT_RMII))
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (
        _transportId != FLPROG_TRANSPORT_WIFI &&
        _transportId != FLPROG_TRANSPORT_WIFI_AP)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_esp8266Open)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    uint8_t opened = 0;
    if (_multicastEnabled)
    {
        if (
            !flprogUdpIpIsMulticast(_multicastGroup) ||
            _multicastPort == 0 ||
            flprogUdpIpIsZero(_localAddress))
        {
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        opened = _esp8266Udp.beginMulticast(
            _localAddress,
            _multicastGroup,
            _multicastPort);
        if (opened)
        {
            _multicastJoined = true;
            _joinedMulticastGroup = _multicastGroup;
            _joinedMulticastInterface = _localAddress;
        }
    }
    else
    {
        opened = _esp8266Udp.begin(endpoint.localPort);
    }

    if (!opened)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    _esp8266Open = true;
    _localPort = _esp8266Udp.localPort();
    return FLPROG_SOCKET_RESULT_SUCCESS;
#elif FLPROG_NETWORK_HAS_POSIX_UDP
    if (_socketFd >= 0)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    _socketFd = ::socket(AF_INET, SOCK_DGRAM, 0);
    if (_socketFd < 0)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    const int flags = fcntl(_socketFd, F_GETFL, 0);
    if (flags >= 0)
    {
        fcntl(_socketFd, F_SETFL, flags | O_NONBLOCK);
    }

    int reuse = 1;
    setsockopt(
        _socketFd,
        SOL_SOCKET,
        SO_REUSEADDR,
        (const char *)&reuse,
        sizeof(reuse));

    struct sockaddr_in localAddress;
    memset(&localAddress, 0, sizeof(localAddress));
    localAddress.sin_family = AF_INET;
    localAddress.sin_port = htons(endpoint.localPort);
    localAddress.sin_addr.s_addr =
        _multicastEnabled
            ? htonl(INADDR_ANY)
            : (flprogUdpIpIsZero(_localAddress)
                   ? htonl(INADDR_ANY)
                   : flprogUdpIpv4NetworkOrder(_localAddress));

    if (::bind(
            _socketFd,
            (struct sockaddr *)&localAddress,
            sizeof(localAddress)) < 0)
    {
        ::close(_socketFd);
        _socketFd = -1;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (_multicastEnabled)
    {
        if (
            !flprogUdpIpIsMulticast(_multicastGroup) ||
            _multicastPort == 0 ||
            flprogUdpIpIsZero(_localAddress))
        {
            ::close(_socketFd);
            _socketFd = -1;
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        struct ip_mreq membership;
        memset(&membership, 0, sizeof(membership));
        membership.imr_multiaddr.s_addr =
            flprogUdpIpv4NetworkOrder(_multicastGroup);
        membership.imr_interface.s_addr =
            flprogUdpIpv4NetworkOrder(_localAddress);

        if (setsockopt(
                _socketFd,
                IPPROTO_IP,
                IP_ADD_MEMBERSHIP,
                (const char *)&membership,
                sizeof(membership)) < 0)
        {
            ::close(_socketFd);
            _socketFd = -1;
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        const uint32_t outgoingInterface =
            flprogUdpIpv4NetworkOrder(_localAddress);
        if (setsockopt(
                _socketFd,
                IPPROTO_IP,
                IP_MULTICAST_IF,
                (const char *)&outgoingInterface,
                sizeof(outgoingInterface)) < 0)
        {
            setsockopt(
                _socketFd,
                IPPROTO_IP,
                IP_DROP_MEMBERSHIP,
                (const char *)&membership,
                sizeof(membership));
            ::close(_socketFd);
            _socketFd = -1;
            return FLPROG_SOCKET_RESULT_ERROR;
        }

        _multicastJoined = true;
        _joinedMulticastGroup = _multicastGroup;
        _joinedMulticastInterface = _localAddress;
    }

    struct sockaddr_in actualAddress;
    socklen_t actualLength = sizeof(actualAddress);
    memset(&actualAddress, 0, sizeof(actualAddress));
    if (getsockname(
            _socketFd,
            (struct sockaddr *)&actualAddress,
            &actualLength) == 0)
    {
        _localPort = ntohs(actualAddress.sin_port);
    }
    else
    {
        _localPort = endpoint.localPort;
    }

    return FLPROG_SOCKET_RESULT_SUCCESS;
#else
    (void)endpoint;
    return FLPROG_SOCKET_RESULT_UNSUPPORTED;
#endif
}

uint8_t FLProgPosixUdpBackend::pool()
{
    return isOpen()
        ? FLPROG_SOCKET_RESULT_SUCCESS
        : FLPROG_SOCKET_RESULT_WAIT;
}

uint8_t FLProgPosixUdpBackend::close()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (_esp8266Open)
    {
        _esp8266Udp.stop();
        _esp8266Open = false;
    }
#elif FLPROG_NETWORK_HAS_POSIX_UDP
    if (_socketFd >= 0)
    {
        if (_multicastJoined)
        {
            struct ip_mreq membership;
            memset(&membership, 0, sizeof(membership));
            membership.imr_multiaddr.s_addr =
                flprogUdpIpv4NetworkOrder(_joinedMulticastGroup);
            membership.imr_interface.s_addr =
                flprogUdpIpv4NetworkOrder(_joinedMulticastInterface);

            setsockopt(
                _socketFd,
                IPPROTO_IP,
                IP_DROP_MEMBERSHIP,
                (const char *)&membership,
                sizeof(membership));
        }

        ::close(_socketFd);
        _socketFd = -1;
    }
#endif

    _multicastJoined = false;
    _joinedMulticastGroup = IPAddress(0, 0, 0, 0);
    _joinedMulticastInterface = IPAddress(0, 0, 0, 0);
    _localPort = 0;
    _txActive = false;
    _txLength = 0;
    clearRxPacket();
    freeBuffers();

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

int FLProgPosixUdpBackend::beginPacket(
    IPAddress ip,
    uint16_t port)
{
    if (!isOpen() || flprogUdpIpIsZero(ip) || port == 0)
    {
        return 0;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    int opened = 0;
    if (flprogUdpIpIsMulticast(ip) && !flprogUdpIpIsZero(_localAddress))
    {
        opened = _esp8266Udp.beginPacketMulticast(
            ip,
            port,
            _localAddress);
    }
    else
    {
        opened = _esp8266Udp.beginPacket(ip, port);
    }
    if (!opened)
    {
        return 0;
    }
#else
    if (!ensureTxBuffer())
    {
        return 0;
    }
#endif

    _txRemoteIP = ip;
    _txRemotePort = port;
    _txLength = 0;
    _txActive = true;
    return 1;
}

size_t FLProgPosixUdpBackend::packetWrite(
    const uint8_t *buffer,
    size_t length)
{
    if (!_txActive || buffer == nullptr || length == 0)
    {
        return 0;
    }

    const size_t space = FLPROG_UDP_MAX_PACKET_SIZE - _txLength;
    if (space == 0)
    {
        return 0;
    }
    if (length > space)
    {
        length = space;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    const size_t written = _esp8266Udp.write(buffer, length);
    _txLength += written;
    return written;
#else
    if (_txBuffer == nullptr)
    {
        return 0;
    }
    memcpy(_txBuffer + _txLength, buffer, length);
    _txLength += length;
    return length;
#endif
}

int FLProgPosixUdpBackend::endPacket()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!isOpen() || !_txActive)
    {
        return 0;
    }
    const int result = _esp8266Udp.endPacket();
    if (result)
    {
        _txLength = 0;
        _txActive = false;
    }
    return result ? 1 : 0;
#elif FLPROG_NETWORK_HAS_POSIX_UDP
    if (
        !isOpen() ||
        !_txActive ||
        _txBuffer == nullptr ||
        flprogUdpIpIsZero(_txRemoteIP) ||
        _txRemotePort == 0)
    {
        return 0;
    }

    struct sockaddr_in remoteAddress;
    memset(&remoteAddress, 0, sizeof(remoteAddress));
    remoteAddress.sin_family = AF_INET;
    remoteAddress.sin_port = htons(_txRemotePort);
    remoteAddress.sin_addr.s_addr =
        flprogUdpIpv4NetworkOrder(_txRemoteIP);

    const int sent = ::sendto(
        _socketFd,
        (const char *)_txBuffer,
        _txLength,
        0,
        (struct sockaddr *)&remoteAddress,
        sizeof(remoteAddress));

    if (sent < 0)
    {
        if (errno == EWOULDBLOCK || errno == EAGAIN)
        {
            return 0;
        }
        return 0;
    }

    const bool allSent = (size_t)sent == _txLength;
    if (allSent)
    {
        _txLength = 0;
        _txActive = false;
    }
    return allSent ? 1 : 0;
#else
    return 0;
#endif
}

int FLProgPosixUdpBackend::parsePacket()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!isOpen())
    {
        return 0;
    }

    clearRxPacket();
    const int received = _esp8266Udp.parsePacket();
    if (received <= 0)
    {
        return 0;
    }

    _rxLength = (size_t)received;
    _rxPosition = 0;
    _rxRemoteIP = _esp8266Udp.remoteIP();
    _rxRemotePort = _esp8266Udp.remotePort();
    return received;
#elif FLPROG_NETWORK_HAS_POSIX_UDP
    if (!isOpen() || !ensureRxBuffer())
    {
        return 0;
    }

    clearRxPacket();
    struct sockaddr_in remoteAddress;
    socklen_t remoteLength = sizeof(remoteAddress);
    memset(&remoteAddress, 0, sizeof(remoteAddress));

    const int received = ::recvfrom(
        _socketFd,
        (char *)_rxBuffer,
        FLPROG_UDP_MAX_PACKET_SIZE,
        MSG_DONTWAIT,
        (struct sockaddr *)&remoteAddress,
        &remoteLength);

    if (received < 0)
    {
        if (errno == EWOULDBLOCK || errno == EAGAIN)
        {
            return 0;
        }
        return 0;
    }

    _rxLength = (size_t)received;
    _rxPosition = 0;
    _rxRemoteIP = flprogUdpFromNetworkOrder(remoteAddress.sin_addr.s_addr);
    _rxRemotePort = ntohs(remoteAddress.sin_port);
    return received;
#else
    return 0;
#endif
}

int FLProgPosixUdpBackend::packetAvailable()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    if (!isOpen())
    {
        return 0;
    }
    return _esp8266Udp.available();
#else
    if (_rxPosition >= _rxLength)
    {
        return 0;
    }
    return (int)(_rxLength - _rxPosition);
#endif
}

int FLProgPosixUdpBackend::packetRead()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    const int value = _esp8266Udp.read();
    if (value >= 0 && _rxPosition < _rxLength)
    {
        _rxPosition++;
    }
    return value;
#else
    if (_rxPosition >= _rxLength)
    {
        return -1;
    }
    return _rxBuffer[_rxPosition++];
#endif
}

int FLProgPosixUdpBackend::packetRead(
    uint8_t *buffer,
    size_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return 0;
    }

#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    const int result = _esp8266Udp.read(buffer, length);
    if (result > 0)
    {
        _rxPosition += (size_t)result;
        if (_rxPosition > _rxLength) _rxPosition = _rxLength;
    }
    return result;
#else
    if (_rxPosition >= _rxLength)
    {
        return 0;
    }

    size_t amount = _rxLength - _rxPosition;
    if (amount > length) amount = length;
    memcpy(buffer, _rxBuffer + _rxPosition, amount);
    _rxPosition += amount;
    return (int)amount;
#endif
}

int FLProgPosixUdpBackend::packetPeek()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    return _esp8266Udp.peek();
#else
    if (_rxPosition >= _rxLength)
    {
        return -1;
    }
    return _rxBuffer[_rxPosition];
#endif
}

void FLProgPosixUdpBackend::packetFlush()
{
#if defined(ESP8266) && FLPROG_NETWORK_COMPILE_WIFI
    // WiFiUDP::flush() is a TX operation in ESP8266 core. Drain unread RX bytes
    // explicitly instead of calling it here.
    while (_esp8266Udp.available() > 0)
    {
        (void)_esp8266Udp.read();
    }
#endif
    _rxPosition = _rxLength;
}

// -----------------------------------------------------------------------------
// FLProgNetworkUdp
// -----------------------------------------------------------------------------

FLProgNetworkUdp::FLProgNetworkUdp()
{
    for (uint8_t i = 0; i < FLPROG_TRANSPORT_COUNT; i++)
    {
        _udpBackends[i] = nullptr;
    }

    _wifiPosixBackend.configure(
        FLPROG_TRANSPORT_WIFI
    );

    _wifiApPosixBackend.configure(
        FLPROG_TRANSPORT_WIFI_AP
    );

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _rmiiPosixBackend.configure(
        FLPROG_TRANSPORT_RMII
    );
#endif
}

bool FLProgNetworkUdp::begin(
    FLProgNetworkInterface &network,
    uint8_t routeValue,
    uint16_t localPortValue)
{
    _udpNetwork = &network;

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _wiznetBackend.hardware(
        network.wiznetHardware()
    );

    if (
        _udpBackends[
            FLPROG_TRANSPORT_WIZNET
        ] == nullptr ||
        _udpBackends[
            FLPROG_TRANSPORT_WIZNET
        ] == &_wiznetBackend
    )
    {
        setUdpBackend(
            FLPROG_TRANSPORT_WIZNET,
            &_wiznetBackend
        );
    }

#endif

#if FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP
    if (
        _udpBackends[
            FLPROG_TRANSPORT_WIFI
        ] == nullptr ||
        _udpBackends[
            FLPROG_TRANSPORT_WIFI
        ] == &_wifiPosixBackend
    )
    {
        setUdpBackend(
            FLPROG_TRANSPORT_WIFI,
            &_wifiPosixBackend
        );
    }

    if (
        _udpBackends[
            FLPROG_TRANSPORT_WIFI_AP
        ] == nullptr ||
        _udpBackends[
            FLPROG_TRANSPORT_WIFI_AP
        ] == &_wifiApPosixBackend
    )
    {
        setUdpBackend(
            FLPROG_TRANSPORT_WIFI_AP,
            &_wifiApPosixBackend
        );
    }

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
#if defined(ESP32)
    if (
        _udpBackends[
            FLPROG_TRANSPORT_RMII
        ] == nullptr ||
        _udpBackends[
            FLPROG_TRANSPORT_RMII
        ] == &_rmiiPosixBackend
    )
    {
        setUdpBackend(
            FLPROG_TRANSPORT_RMII,
            &_rmiiPosixBackend
        );
    }
#endif
#endif
#endif

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    if (
        _udpBackends[
            FLPROG_TRANSPORT_RMII
        ] == nullptr ||
        _udpBackends[
            FLPROG_TRANSPORT_RMII
        ] == &_stm32RmiiBackend)
    {
        setUdpBackend(
            FLPROG_TRANSPORT_RMII,
            &_stm32RmiiBackend
        );
    }
#endif

    FLProgNetworkSocket::localPort(
        localPortValue
    );

    for (uint8_t i = 1; i < FLPROG_TRANSPORT_COUNT; i++)
    {
        if (_udpBackends[i] != nullptr)
        {
            _udpBackends[i]->configureMulticast(
                _multicastEnabled,
                _multicastGroup,
                _multicastPort
            );
        }
    }

    if (
        !FLProgNetworkSocket::begin(
            network,
            routeValue,
            FLPROG_SOCKET_PROTOCOL_DATAGRAM
        )
    )
    {
        return false;
    }

    requestOpen();
    return true;
}

bool FLProgNetworkUdp::beginMulticast(
    FLProgNetworkInterface &network,
    uint8_t routeValue,
    IPAddress group,
    uint16_t port)
{
    if (!flprogUdpIpIsMulticast(group) || port == 0)
    {
        return false;
    }

    _multicastEnabled = true;
    _multicastGroup = group;
    _multicastPort = port;

    return begin(network, routeValue, port);
}

bool FLProgNetworkUdp::joinMulticast(
    IPAddress group,
    uint16_t port)
{
    if (
        _udpNetwork == nullptr ||
        !flprogUdpIpIsMulticast(group) ||
        port == 0
    )
    {
        return false;
    }

    _multicastEnabled = true;
    _multicastGroup = group;
    _multicastPort = port;
    FLProgNetworkSocket::localPort(port);

    bool supported = false;
    for (uint8_t i = 1; i < FLPROG_TRANSPORT_COUNT; i++)
    {
        if (_udpBackends[i] != nullptr)
        {
            supported =
                _udpBackends[i]->configureMulticast(true, group, port) ||
                supported;
        }
    }

    requestRebind();
    return supported;
}

bool FLProgNetworkUdp::leaveMulticast()
{
    if (_udpNetwork == nullptr)
    {
        return false;
    }

    _multicastEnabled = false;
    _multicastGroup = IPAddress(0, 0, 0, 0);
    _multicastPort = 0;

    for (uint8_t i = 1; i < FLPROG_TRANSPORT_COUNT; i++)
    {
        if (_udpBackends[i] != nullptr)
        {
            _udpBackends[i]->configureMulticast(
                false,
                IPAddress(0, 0, 0, 0),
                0
            );
        }
    }

    requestRebind();
    return true;
}

void FLProgNetworkUdp::stop()
{
    abort();
    end();
    _udpNetwork = nullptr;
    _multicastEnabled = false;
    _multicastGroup = IPAddress(0, 0, 0, 0);
    _multicastPort = 0;
}

void FLProgNetworkUdp::setUdpBackend(
    uint8_t transportIdValue,
    FLProgUdpBackend *value)
{
    if (
        transportIdValue ==
            FLPROG_TRANSPORT_NONE ||
        transportIdValue >=
            FLPROG_TRANSPORT_COUNT
    )
    {
        return;
    }

    _udpBackends[
        transportIdValue
    ] = value;

    setBackend(
        transportIdValue,
        value
    );
}

FLProgUdpBackend *FLProgNetworkUdp::udpBackend(
    uint8_t transportIdValue)
{
    if (
        transportIdValue >=
        FLPROG_TRANSPORT_COUNT
    )
    {
        return nullptr;
    }

    return _udpBackends[
        transportIdValue
    ];
}

const FLProgUdpBackend *FLProgNetworkUdp::udpBackend(
    uint8_t transportIdValue) const
{
    if (
        transportIdValue >=
        FLPROG_TRANSPORT_COUNT
    )
    {
        return nullptr;
    }

    return _udpBackends[
        transportIdValue
    ];
}

FLProgUdpBackend *FLProgNetworkUdp::currentUdpBackend()
{
    return udpBackend(
        boundTransportId()
    );
}

const FLProgUdpBackend *FLProgNetworkUdp::currentUdpBackend() const
{
    return udpBackend(
        boundTransportId()
    );
}

void FLProgNetworkUdp::pool()
{
    if (_udpNetwork != nullptr)
    {
        _wifiPosixBackend.setLocalAddress(
            _udpNetwork->wifiIP()
        );

        _wifiApPosixBackend.setLocalAddress(
            _udpNetwork->accessPointIP()
        );

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _rmiiPosixBackend.setLocalAddress(
            _udpNetwork->rmiiIP()
        );
#endif

        if (!_broadcastBlockingExplicit)
        {
            _broadcastBlocking =
                _udpNetwork->broadcastBlocking();
        }

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _wiznetBackend.setBroadcastBlocking(
            _broadcastBlocking
        );

        _wiznetBackend.configureMulticast(
            _multicastEnabled,
            _multicastGroup,
            _multicastPort
        );
#endif

        _wifiPosixBackend.configureMulticast(
            _multicastEnabled,
            _multicastGroup,
            _multicastPort
        );

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _rmiiPosixBackend.configureMulticast(
            _multicastEnabled,
            _multicastGroup,
            _multicastPort
        );
#endif
#if FLPROG_NETWORK_HAS_STM32_ETHERNET && !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _stm32RmiiBackend.configureMulticast(
            _multicastEnabled,
            _multicastGroup,
            _multicastPort
        );
#endif
    }

    FLProgNetworkSocket::pool();
}

int FLProgNetworkUdp::beginPacket(
    IPAddress ip,
    uint16_t port)
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    if (
        !isOpen() ||
        backendValue == nullptr
    )
    {
        return 0;
    }

    return backendValue->beginPacket(
        ip,
        port
    );
}

int FLProgNetworkUdp::beginPacket(
    const char *hostname,
    uint16_t port)
{
    if (
        _udpNetwork == nullptr ||
        hostname == nullptr ||
        hostname[0] == 0 ||
        port == 0
    )
    {
        return 0;
    }

    FLProgNetworkDns *resolver =
        _udpNetwork->dnsResolver();

    if (resolver == nullptr)
    {
        return 0;
    }

    const uint8_t result =
        resolver->resolve(hostname, route());

    if (result != FLPROG_DNS_RESULT_SUCCESS)
    {
        return 0;
    }

    return beginPacket(resolver->result(), port);
}

size_t FLProgNetworkUdp::write(
    uint8_t value)
{
    return write(&value, 1);
}

size_t FLProgNetworkUdp::write(
    const uint8_t *buffer,
    size_t length)
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetWrite(
            buffer,
            length
        )
        : 0;
}

int FLProgNetworkUdp::endPacket()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->endPacket()
        : 0;
}

int FLProgNetworkUdp::parsePacket()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->parsePacket()
        : 0;
}

int FLProgNetworkUdp::available()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetAvailable()
        : 0;
}

int FLProgNetworkUdp::read()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetRead()
        : -1;
}

int FLProgNetworkUdp::read(
    uint8_t *buffer,
    size_t length)
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetRead(
            buffer,
            length
        )
        : 0;
}

int FLProgNetworkUdp::peek()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetPeek()
        : -1;
}

void FLProgNetworkUdp::flush()
{
    FLProgUdpBackend *backendValue =
        currentUdpBackend();

    if (
        isOpen() &&
        backendValue != nullptr
    )
    {
        backendValue->packetFlush();
    }
}

IPAddress FLProgNetworkUdp::remoteIP() const
{
    const FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetRemoteIP()
        : IPAddress(0, 0, 0, 0);
}

uint16_t FLProgNetworkUdp::remotePort() const
{
    const FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetRemotePort()
        : 0;
}

uint16_t FLProgNetworkUdp::packetSize() const
{
    const FLProgUdpBackend *backendValue =
        currentUdpBackend();

    return (
        isOpen() &&
        backendValue != nullptr
    )
        ? backendValue->packetSize()
        : 0;
}
