/*
 * FLProg_Network — flprogNetworkDhcp.cpp
 * Назначение: DHCP state machine для wired transport.
 *
 * Архитектурные заметки:
 * - DHCP не должен подтягиваться в WiFi-only профиль только из-за общего публичного API.
 * - Таймауты и повторы обрабатываются как state machine без блокирующих ожиданий.
 * - Полученные IP/GW/DNS/subnet публикуются через NetworkInterface.
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
#include "../flprogNetworkProfile.h"

#if FLPROG_NETWORK_COMPILE_WIRED
#include "flprogNetworkDhcp.h"
#include "../flprogNetwork.h"
#include <string.h>

static const uint8_t FLPROG_DHCP_MAGIC_COOKIE[4] = {99, 130, 83, 99};

static const uint8_t DHCP_OPT_SUBNET = 1;
static const uint8_t DHCP_OPT_ROUTER = 3;
static const uint8_t DHCP_OPT_DNS = 6;
static const uint8_t DHCP_OPT_REQUESTED_IP = 50;
static const uint8_t DHCP_OPT_LEASE = 51;
static const uint8_t DHCP_OPT_OVERLOAD = 52;
static const uint8_t DHCP_OPT_MESSAGE_TYPE = 53;
static const uint8_t DHCP_OPT_SERVER_ID = 54;
static const uint8_t DHCP_OPT_PARAMETER_LIST = 55;
static const uint8_t DHCP_OPT_MAX_MESSAGE = 57;
static const uint8_t DHCP_OPT_T1 = 58;
static const uint8_t DHCP_OPT_T2 = 59;
static const uint8_t DHCP_OPT_CLIENT_ID = 61;
static const uint8_t DHCP_OPT_END = 255;
static const uint8_t DHCP_OPT_PAD = 0;

FLProgWiznetDhcp::~FLProgWiznetDhcp()
{
    end(false);
}

bool FLProgWiznetDhcp::begin(FLProgNetworkInterface &network)
{
    if (_network != &network)
    {
        end(false);
        _network = &network;
        _wiznetUdp.hardware(network.wiznetHardware());
    }

    _udp = (_externalBackend != nullptr)
        ? _externalBackend
        : static_cast<FLProgUdpBackend *>(&_wiznetUdp);

    _error = FLPROG_DHCP_ERROR_NONE;
    clearLease(false);
    setState(FLPROG_DHCP_STATE_INIT);
    return true;
}

void FLProgWiznetDhcp::end(bool clearAddress)
{
    _strictArpMonitor.stop();
    _arpProbe.stop();
    _arpProbeStarted = false;
    _declinePending = false;
    closeSocket();

    // end() is a hard logical detach. If the hardware CLOSE command still
    // needs another poll cycle, release ownership now and reset the backend;
    // a later OPEN step will verify/close the physical socket again.
    if (_network != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        if (_externalBackend == nullptr)
        {
            _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
            _wiznetUdp = FLProgWiznetUdpBackend();
        }
        _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
        _lease = FLProgSocketLease();
        _socketReleasePending = false;
    }

    if (clearAddress)
    {
        applyZeroAddress();
    }

    _network = nullptr;
    _udp = nullptr;
    _exchange = EXCHANGE_NONE;
    setState(FLPROG_DHCP_STATE_DISABLED);
}

void FLProgWiznetDhcp::restart()
{
    if (_network == nullptr)
    {
        return;
    }

    _strictArpMonitor.stop();
    _arpProbe.stop();
    _arpProbeStarted = false;
    _declinePending = false;
    closeSocket();
    clearLease(true);
    _error = FLPROG_DHCP_ERROR_NONE;
    setState(FLPROG_DHCP_STATE_INIT);
}

bool FLProgWiznetDhcp::release()
{
    if (_network == nullptr || _state != FLPROG_DHCP_STATE_BOUND)
    {
        return false;
    }

    _strictArpMonitor.stop();
    if (!ensureHardware() || !ensureSocket())
    {
        return false;
    }

    _udp->pool();
    if (!sendRelease())
    {
        return false;
    }

    closeSocket();
    clearLease(true);
    _error = FLPROG_DHCP_ERROR_NONE;
    setState(FLPROG_DHCP_STATE_INIT);
    return true;
}

uint32_t FLProgWiznetDhcp::leaseAge() const
{
    if (_leaseStarted == 0)
    {
        return 0;
    }

    return (uint32_t)(millis() - _leaseStarted) / 1000UL;
}

void FLProgWiznetDhcp::setState(uint8_t value)
{
    _state = value;
}

bool FLProgWiznetDhcp::ipIsZero(IPAddress ip)
{
    return ip[0] == 0 && ip[1] == 0 && ip[2] == 0 && ip[3] == 0;
}

uint32_t FLProgWiznetDhcp::readU32(const uint8_t *data)
{
    return
        ((uint32_t)data[0] << 24) |
        ((uint32_t)data[1] << 16) |
        ((uint32_t)data[2] << 8) |
        (uint32_t)data[3];
}

void FLProgWiznetDhcp::writeU16(uint8_t *data, uint16_t value)
{
    data[0] = (uint8_t)(value >> 8);
    data[1] = (uint8_t)value;
}

void FLProgWiznetDhcp::writeU32(uint8_t *data, uint32_t value)
{
    data[0] = (uint8_t)(value >> 24);
    data[1] = (uint8_t)(value >> 16);
    data[2] = (uint8_t)(value >> 8);
    data[3] = (uint8_t)value;
}

IPAddress FLProgWiznetDhcp::readIp(const uint8_t *data)
{
    return IPAddress(data[0], data[1], data[2], data[3]);
}

uint32_t FLProgWiznetDhcp::makeTransactionId() const
{
    uint32_t value = micros() ^ millis();

    if (_network != nullptr)
    {
        const uint8_t *mac = const_cast<FLProgNetworkInterface *>(_network)->mac();
        for (uint8_t i = 0; i < 6; i++)
        {
            value = (value << 5) ^ (value >> 2) ^ mac[i];
        }
    }

    return value != 0 ? value : 0x464C5001UL;
}

bool FLProgWiznetDhcp::ensureHardware()
{
    if (_network == nullptr)
    {
        return false;
    }

    if (_externalBackend != nullptr)
    {
        return true;
    }

    FLProgWiznetHardware &hardware = _network->wiznetHardware();

    if (!hardware.isInit())
    {
        const uint8_t result = hardware.init();
        if (result == FLPROG_WIZNET_HW_WAIT)
        {
            return false;
        }
        if (result != FLPROG_WIZNET_HW_READY)
        {
            fail(FLPROG_DHCP_ERROR_HARDWARE);
            return false;
        }
    }

    if (_state == FLPROG_DHCP_STATE_INIT)
    {
        applyZeroAddress();
        hardware.setRetransmissionTime(_network->retransmissionTime());
        hardware.setRetransmissionCount(_network->retransmissionCount());
    }

    return true;
}

bool FLProgWiznetDhcp::ensureSocket()
{
    if (_network == nullptr || _udp == nullptr)
    {
        return false;
    }

    if (_udp->isOpen())
    {
        return true;
    }

    if (_socketReleasePending)
    {
        if (!closeSocket())
        {
            return false;
        }
    }

    if (_nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
    {
        if (_externalBackend != nullptr)
        {
            _nativeSocket = 0;
        }
        else
        {
            _nativeSocket = _network->wiznetTransport().reserveNativeSocket(this);
            if (_nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
            {
                return false;
            }
        }

        _lease = FLProgSocketLease();
        _lease.used = true;
        _lease.owner = this;
        _lease.transportId = FLPROG_TRANSPORT_WIZNET;
        _lease.nativeSocket = _nativeSocket;
        _lease.requestedNativeSocket = _nativeSocket;
        _lease.transportGeneration = _network->wiznetTransport().generation();

        _endpoint = FLProgSocketEndpoint();
        _endpoint.localPort = FLPROG_DHCP_CLIENT_PORT;

        _udp->setBroadcastBlocking(false);
    }

    const uint8_t openResult = _udp->open(
        _lease,
        FLPROG_SOCKET_PROTOCOL_DATAGRAM,
        _endpoint,
        _responseTimeout > 0 ? (_responseTimeout * 2UL) : 2000UL
    );

    if (openResult == FLPROG_SOCKET_RESULT_ERROR ||
        openResult == FLPROG_SOCKET_RESULT_UNSUPPORTED)
    {
        fail(FLPROG_DHCP_ERROR_SOCKET);
        return false;
    }

    _udp->pool();
    return _udp->isOpen();
}

bool FLProgWiznetDhcp::closeSocket()
{
    if (_udp != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        const uint8_t result = _udp->close();
        if (result != FLPROG_SOCKET_RESULT_SUCCESS)
        {
            _socketReleasePending = true;
            return false;
        }
    }

    if (
        _externalBackend == nullptr &&
        _network != nullptr &&
        _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
    }

    _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    _lease = FLProgSocketLease();
    _socketReleasePending = false;
    return true;
}

void FLProgWiznetDhcp::applyZeroAddress()
{
    if (_network == nullptr)
    {
        return;
    }

    if (_externalBackend == nullptr)
    {
        _network->wiznetHardware().setNetSettings(
            _network->mac(),
            IPAddress(0, 0, 0, 0),
            IPAddress(0, 0, 0, 0),
            IPAddress(0, 0, 0, 0)
        );
    }
}

void FLProgWiznetDhcp::applyLease()
{
    if (_network == nullptr)
    {
        return;
    }

    IPAddress gateway = _gatewayIp;
    IPAddress subnet = _subnetIp;

    if (ipIsZero(subnet))
    {
        subnet = IPAddress(255, 255, 255, 0);
    }

    if (_externalBackend == nullptr)
    {
        _network->wiznetHardware().setNetSettings(
            _network->mac(),
            _leasedIp,
            gateway,
            subnet
        );
    }
}

void FLProgWiznetDhcp::clearLease(bool clearHardware)
{
    _strictArpMonitor.stop();
    _offeredIp = IPAddress(0, 0, 0, 0);
    _leasedIp = IPAddress(0, 0, 0, 0);
    _serverIp = IPAddress(0, 0, 0, 0);
    _gatewayIp = IPAddress(0, 0, 0, 0);
    _subnetIp = IPAddress(0, 0, 0, 0);
    _dnsIp = IPAddress(0, 0, 0, 0);
    _leaseTime = 0;
    _renewalTime = 0;
    _rebindingTime = 0;
    _leaseStarted = 0;
    _exchangeAttempt = 0;
    _lastMessageType = 0;
    _arpProbeStarted = false;
    _declinePending = false;
    _initialAckPending = false;
    _announcementTargetState = FLPROG_DHCP_STATE_BOUND;
    _lastDefense = 0;

    if (clearHardware)
    {
        applyZeroAddress();
    }
}

bool FLProgWiznetDhcp::appendOption(uint8_t code, const uint8_t *data, uint8_t length, size_t &position)
{
    if (position + 2U + length >= FLPROG_DHCP_MAX_PACKET_SIZE)
    {
        return false;
    }

    _packet[position++] = code;
    _packet[position++] = length;
    if (length > 0 && data != nullptr)
    {
        memcpy(&_packet[position], data, length);
        position += length;
    }
    return true;
}

bool FLProgWiznetDhcp::appendOptionU32(uint8_t code, uint32_t value, size_t &position)
{
    uint8_t data[4];
    writeU32(data, value);
    return appendOption(code, data, 4, position);
}

bool FLProgWiznetDhcp::appendOptionIp(uint8_t code, IPAddress ip, size_t &position)
{
    uint8_t data[4] = {ip[0], ip[1], ip[2], ip[3]};
    return appendOption(code, data, 4, position);
}

size_t FLProgWiznetDhcp::buildPacket(
    uint8_t messageType,
    bool includeRequestedIp,
    bool includeServerId,
    bool ciaddrPresent,
    bool broadcastFlag)
{
    memset(_packet, 0, sizeof(_packet));

    _packet[0] = 1; // BOOTREQUEST
    _packet[1] = 1; // Ethernet
    _packet[2] = 6; // MAC length
    writeU32(&_packet[4], _xid);

    const uint32_t elapsedMs = (uint32_t)(millis() - _exchangeStarted);
    uint16_t seconds = (uint16_t)(elapsedMs / 1000UL);
    writeU16(&_packet[8], seconds);

    if (broadcastFlag)
    {
        _packet[10] = 0x80;
        _packet[11] = 0x00;
    }

    if (ciaddrPresent && !ipIsZero(_leasedIp))
    {
        _packet[12] = _leasedIp[0];
        _packet[13] = _leasedIp[1];
        _packet[14] = _leasedIp[2];
        _packet[15] = _leasedIp[3];
    }

    if (_network == nullptr)
    {
        return 0;
    }

    const uint8_t *mac = _network->mac();
    memcpy(&_packet[28], mac, 6);
    memcpy(&_packet[236], FLPROG_DHCP_MAGIC_COOKIE, 4);

    size_t position = 240;
    if (!appendOption(DHCP_OPT_MESSAGE_TYPE, &messageType, 1, position))
    {
        return 0;
    }

    uint8_t clientId[7] = {1, 0, 0, 0, 0, 0, 0};
    memcpy(&clientId[1], mac, 6);
    if (!appendOption(DHCP_OPT_CLIENT_ID, clientId, 7, position))
    {
        return 0;
    }

    if (includeRequestedIp && !ipIsZero(_offeredIp))
    {
        if (!appendOptionIp(DHCP_OPT_REQUESTED_IP, _offeredIp, position))
        {
            return 0;
        }
    }

    if (includeServerId && !ipIsZero(_serverIp))
    {
        if (!appendOptionIp(DHCP_OPT_SERVER_ID, _serverIp, position))
        {
            return 0;
        }
    }

    const uint8_t requestList[] = {
        DHCP_OPT_SUBNET,
        DHCP_OPT_ROUTER,
        DHCP_OPT_DNS,
        DHCP_OPT_LEASE,
        DHCP_OPT_T1,
        DHCP_OPT_T2
    };
    if (!appendOption(DHCP_OPT_PARAMETER_LIST, requestList, sizeof(requestList), position))
    {
        return 0;
    }

    uint8_t maxMessage[2] = {
        (uint8_t)(FLPROG_DHCP_MAX_PACKET_SIZE >> 8),
        (uint8_t)FLPROG_DHCP_MAX_PACKET_SIZE
    };
    if (!appendOption(DHCP_OPT_MAX_MESSAGE, maxMessage, 2, position))
    {
        return 0;
    }

    if (position >= FLPROG_DHCP_MAX_PACKET_SIZE)
    {
        return 0;
    }
    _packet[position++] = DHCP_OPT_END;

    if (position < 300)
    {
        position = 300;
    }

    return position;
}

bool FLProgWiznetDhcp::sendMessage(
    uint8_t messageType,
    IPAddress destination,
    bool includeRequestedIp,
    bool includeServerId,
    bool ciaddrPresent,
    bool broadcastFlag)
{
    if (_udp == nullptr || !_udp->isOpen())
    {
        return false;
    }

    const size_t length = buildPacket(
        messageType,
        includeRequestedIp,
        includeServerId,
        ciaddrPresent,
        broadcastFlag
    );

    if (length == 0)
    {
        return false;
    }

    if (_udp->beginPacket(destination, FLPROG_DHCP_SERVER_PORT) != 1)
    {
        return false;
    }

    if (_udp->packetWrite(_packet, length) != length)
    {
        _udp->packetFlush();
        return false;
    }

    if (_udp->endPacket() != 1)
    {
        return false;
    }

    _lastSend = millis();
    _exchangeAttempt++;
    if (_exchangeAttempt > 1)
    {
        _retryCount++;
    }
    return true;
}

bool FLProgWiznetDhcp::sendDiscover()
{
    if (sendMessage(
        FLPROG_DHCP_MESSAGE_DISCOVER,
        IPAddress(255, 255, 255, 255),
        false,
        false,
        false,
        true))
    {
        _discoverCount++;
        return true;
    }
    return false;
}

bool FLProgWiznetDhcp::sendSelectRequest()
{
    if (sendMessage(
        FLPROG_DHCP_MESSAGE_REQUEST,
        IPAddress(255, 255, 255, 255),
        true,
        true,
        false,
        true))
    {
        _requestCount++;
        return true;
    }
    return false;
}

bool FLProgWiznetDhcp::sendRenewRequest(bool rebind)
{
    const IPAddress destination = rebind
        ? IPAddress(255, 255, 255, 255)
        : _serverIp;

    if (ipIsZero(destination))
    {
        return false;
    }

    if (sendMessage(
        FLPROG_DHCP_MESSAGE_REQUEST,
        destination,
        false,
        false,
        true,
        rebind))
    {
        if (rebind)
        {
            _rebindCount++;
        }
        else
        {
            _renewCount++;
        }
        return true;
    }
    return false;
}

bool FLProgWiznetDhcp::sendDecline()
{
    if (ipIsZero(_offeredIp) || ipIsZero(_serverIp))
    {
        return false;
    }

    if (sendMessage(FLPROG_DHCP_MESSAGE_DECLINE,
                    IPAddress(255, 255, 255, 255),
                    true, true, false, true))
    {
        _declineCount++;
        return true;
    }
    return false;
}

bool FLProgWiznetDhcp::sendRelease()
{
    if (ipIsZero(_leasedIp) || ipIsZero(_serverIp))
    {
        return false;
    }

    if (sendMessage(FLPROG_DHCP_MESSAGE_RELEASE,
                    _serverIp,
                    false, true, true, false))
    {
        _releaseCount++;
        return true;
    }
    return false;
}

bool FLProgWiznetDhcp::optionIPv4(const uint8_t *data, uint8_t length, IPAddress &target) const
{
    if (data == nullptr || length < 4)
    {
        return false;
    }
    target = readIp(data);
    return true;
}

bool FLProgWiznetDhcp::parseOptions(
    const uint8_t *options,
    size_t length,
    Reply &reply,
    uint8_t &overload) const
{
    size_t position = 0;

    while (position < length)
    {
        const uint8_t code = options[position++];
        if (code == DHCP_OPT_PAD)
        {
            continue;
        }
        if (code == DHCP_OPT_END)
        {
            return true;
        }
        if (position >= length)
        {
            return false;
        }

        const uint8_t optionLength = options[position++];
        if (position + optionLength > length)
        {
            return false;
        }

        const uint8_t *data = &options[position];
        switch (code)
        {
            case DHCP_OPT_MESSAGE_TYPE:
                if (optionLength >= 1) reply.messageType = data[0];
                break;

            case DHCP_OPT_SERVER_ID:
                optionIPv4(data, optionLength, reply.server);
                break;

            case DHCP_OPT_SUBNET:
                optionIPv4(data, optionLength, reply.subnet);
                break;

            case DHCP_OPT_ROUTER:
                optionIPv4(data, optionLength, reply.gateway);
                break;

            case DHCP_OPT_DNS:
                optionIPv4(data, optionLength, reply.dns);
                break;

            case DHCP_OPT_LEASE:
                if (optionLength == 4) reply.lease = readU32(data);
                break;

            case DHCP_OPT_T1:
                if (optionLength == 4) reply.t1 = readU32(data);
                break;

            case DHCP_OPT_T2:
                if (optionLength == 4) reply.t2 = readU32(data);
                break;

            case DHCP_OPT_OVERLOAD:
                if (optionLength >= 1) overload = data[0];
                break;

            default:
                break;
        }

        position += optionLength;
    }

    return true;
}

bool FLProgWiznetDhcp::parseReply(const uint8_t *packet, size_t length, Reply &reply) const
{
    if (_network == nullptr || packet == nullptr || length < 240)
    {
        return false;
    }

    if (packet[0] != 2 || packet[1] != 1 || packet[2] != 6)
    {
        return false;
    }

    if (readU32(&packet[4]) != _xid)
    {
        return false;
    }

    if (memcmp(&packet[28], _network->mac(), 6) != 0)
    {
        return false;
    }

    if (memcmp(&packet[236], FLPROG_DHCP_MAGIC_COOKIE, 4) != 0)
    {
        return false;
    }

    reply.yiaddr = readIp(&packet[16]);

    uint8_t overload = 0;
    if (!parseOptions(&packet[240], length - 240, reply, overload))
    {
        return false;
    }

    // RFC 2132 option 52: bit 1 means file, bit 2 means sname.
    if ((overload & 1U) != 0)
    {
        uint8_t ignored = 0;
        if (!parseOptions(&packet[108], 128, reply, ignored))
        {
            return false;
        }
    }

    if ((overload & 2U) != 0)
    {
        uint8_t ignored = 0;
        if (!parseOptions(&packet[44], 64, reply, ignored))
        {
            return false;
        }
    }

    return reply.messageType != 0;
}

bool FLProgWiznetDhcp::receiveReply(Reply &reply)
{
    if (_udp == nullptr || !_udp->isOpen())
    {
        return false;
    }

    const int packetSize = _udp->parsePacket();
    if (packetSize <= 0 || _udp->packetRemotePort() != FLPROG_DHCP_SERVER_PORT)
    {
        return false;
    }

    const size_t amount = (packetSize > (int)sizeof(_packet))
        ? sizeof(_packet)
        : (size_t)packetSize;

    const int received = _udp->packetRead(_packet, amount);
    _udp->packetFlush();

    if (received <= 0)
    {
        return false;
    }

    Reply parsed;
    if (!parseReply(_packet, (size_t)received, parsed))
    {
        return false;
    }

    reply = parsed;
    _lastMessageType = parsed.messageType;
    return true;
}

void FLProgWiznetDhcp::acceptOffer(const Reply &reply)
{
    if (reply.messageType != FLPROG_DHCP_MESSAGE_OFFER ||
        ipIsZero(reply.yiaddr) ||
        ipIsZero(reply.server))
    {
        return;
    }

    _offeredIp = reply.yiaddr;
    _serverIp = reply.server;
    _gatewayIp = reply.gateway;
    _subnetIp = reply.subnet;
    _dnsIp = reply.dns;
    _leaseTime = reply.lease;
    _renewalTime = reply.t1;
    _rebindingTime = reply.t2;

    _exchange = EXCHANGE_SELECT_REQUEST;
    _exchangeAttempt = 0;
    _lastSend = 0;
    setState(FLPROG_DHCP_STATE_REQUESTING);
}

void FLProgWiznetDhcp::acceptAck(const Reply &reply)
{
    const bool initialLeaseAck = (_state == FLPROG_DHCP_STATE_REQUESTING);

    if (reply.messageType != FLPROG_DHCP_MESSAGE_ACK)
    {
        return;
    }

    IPAddress acceptedIp = reply.yiaddr;
    if (ipIsZero(acceptedIp))
    {
        acceptedIp = !ipIsZero(_leasedIp) ? _leasedIp : _offeredIp;
    }

    if (ipIsZero(acceptedIp))
    {
        fail(FLPROG_DHCP_ERROR_PROTOCOL);
        return;
    }

    _leasedIp = acceptedIp;
    if (!ipIsZero(reply.server)) _serverIp = reply.server;
    if (!ipIsZero(reply.gateway)) _gatewayIp = reply.gateway;
    if (!ipIsZero(reply.subnet)) _subnetIp = reply.subnet;
    if (!ipIsZero(reply.dns)) _dnsIp = reply.dns;
    if (reply.lease != 0) _leaseTime = reply.lease;
    if (reply.t1 != 0) _renewalTime = reply.t1;
    if (reply.t2 != 0) _rebindingTime = reply.t2;

    if (_leaseTime == 0)
    {
        _leaseTime = 3600UL;
    }

    if (_renewalTime == 0 || _renewalTime >= _leaseTime)
    {
        _renewalTime = _leaseTime / 2UL;
    }

    if (_rebindingTime == 0 ||
        _rebindingTime <= _renewalTime ||
        _rebindingTime >= _leaseTime)
    {
        _rebindingTime = (_leaseTime / 8UL) * 7UL;
    }

    _initialAckPending = initialLeaseAck;

    if (initialLeaseAck && _conflictProbe != nullptr)
    {
        _conflictProbeStarted = millis();
        setState(FLPROG_DHCP_STATE_CHECKING);
        return;
    }

    const bool canUseBuiltInProbe =
        initialLeaseAck &&
        _builtInArpProbeEnabled &&
        (_externalBackend == nullptr || _arpProbeBackendConfigured);

    if (canUseBuiltInProbe)
    {
        _conflictProbeStarted = millis();
        _arpProbeStarted = false;
        _declinePending = false;
        setState(FLPROG_DHCP_STATE_CHECKING);
        closeSocket();
        return;
    }

    finalizeAck();
}

void FLProgWiznetDhcp::finalizeAck()
{
    const bool initialBinding = _initialAckPending;

    _arpProbe.stop();
    _arpProbeStarted = false;
    _declinePending = false;

    applyLease();
    _leaseStarted = millis();
    _exchange = EXCHANGE_NONE;
    _exchangeAttempt = 0;
    _conflictProbeStarted = 0;
    _error = FLPROG_DHCP_ERROR_NONE;
    closeSocket();

    const bool canUseRawArp =
        (_externalBackend == nullptr || _arpProbeBackendConfigured);

    if (initialBinding && _arpAnnouncementEnabled && canUseRawArp)
    {
        _arpProbeStarted = false;
        _announcementTargetState = FLPROG_DHCP_STATE_BOUND;
        setState(FLPROG_DHCP_STATE_ANNOUNCING);
        return;
    }

    if (initialBinding)
    {
        completeBound();
    }
    else
    {
        _initialAckPending = false;
        setState(FLPROG_DHCP_STATE_BOUND);
    }
}

void FLProgWiznetDhcp::completeBound()
{
    _initialAckPending = false;
    _arpProbeStarted = false;
    _announcementTargetState = FLPROG_DHCP_STATE_BOUND;
    _arpConflictSeriesCount = 0;
    _lastDefense = 0;
    _rateLimitUntil = 0;

    // Discard a stale pre-binding CONFLICT flag. A new conflict after the
    // address becomes active will be observed by takeAddressConflict().
    if (_externalBackend == nullptr && _network != nullptr)
    {
        // Drain one stale pre-binding conflict indication if present.
        _network->wiznetHardware().takeIpConflictFlag();
    }

    setState(FLPROG_DHCP_STATE_BOUND);
    if (_acdMode == FLPROG_ACD_MODE_STRICT && _network != nullptr)
    {
        _strictArpMonitor.begin(*_network, _leasedIp);
    }
}

bool FLProgWiznetDhcp::takeAddressConflict()
{
    if (_addressConflictProbe != nullptr)
    {
        return _addressConflictProbe(_addressConflictProbeContext);
    }

    if (_acdMode == FLPROG_ACD_MODE_OFF)
    {
        return false;
    }

    if (_acdMode == FLPROG_ACD_MODE_STRICT)
    {
        return _strictArpMonitor.takeConflict();
    }

    if (_externalBackend != nullptr || _network == nullptr)
    {
        return false;
    }

    return _network->wiznetHardware().takeIpConflictFlag();
}

void FLProgWiznetDhcp::startAnnouncement(uint8_t count, uint8_t targetState)
{
    if (_network == nullptr || count == 0)
    {
        return;
    }

    _strictArpMonitor.stop();
    _arpProbe.stop();
    _arpProbeStarted = false;
    _announcementTargetState = targetState;

    if (!_arpProbe.beginAnnouncement(*_network, _leasedIp, count))
    {
        fail(FLPROG_DHCP_ERROR_ARP_COMPLETION);
        return;
    }

    _arpProbeStarted = true;
}

void FLProgWiznetDhcp::scheduleReacquireAfterConflict()
{
    if (_arpConflictSeriesCount < 0xFF)
    {
        _arpConflictSeriesCount++;
    }

    if (_arpConflictSeriesCount >= FLPROG_ARP_MAX_CONFLICTS)
    {
        _arpRateLimitCount++;
        _rateLimitUntil = millis() + FLPROG_ARP_RATE_LIMIT_MS;
        setState(FLPROG_DHCP_STATE_RATE_LIMIT);
    }
    else
    {
        setState(FLPROG_DHCP_STATE_INIT);
    }
}

void FLProgWiznetDhcp::handleNak()
{
    _arpProbe.stop();
    _arpProbeStarted = false;
    _declinePending = false;
    _initialAckPending = false;
    _nakCount++;
    _error = FLPROG_DHCP_ERROR_NAK;
    closeSocket();
    clearLease(true);
    setState(FLPROG_DHCP_STATE_INIT);
}

void FLProgWiznetDhcp::fail(uint8_t errorCode)
{
    _arpProbe.stop();
    _arpProbeStarted = false;
    _declinePending = false;
    _initialAckPending = false;
    _error = errorCode;
    closeSocket();
    setState(FLPROG_DHCP_STATE_FAILED);
}

uint32_t FLProgWiznetDhcp::nextRetryDelay() const
{
    uint32_t base = _responseTimeout > 0 ? _responseTimeout : 1000UL;
    uint8_t shift = _exchangeAttempt;
    if (shift > 3) shift = 3;
    uint32_t result = base << shift;
    return result > 8000UL ? 8000UL : result;
}

bool FLProgWiznetDhcp::responseExpired(uint32_t now) const
{
    return _lastSend != 0 && (uint32_t)(now - _lastSend) >= nextRetryDelay();
}

bool FLProgWiznetDhcp::acquireExpired(uint32_t now) const
{
    return _acquireTimeout > 0 &&
        _exchangeStarted != 0 &&
        (uint32_t)(now - _exchangeStarted) >= _acquireTimeout;
}

void FLProgWiznetDhcp::startInitialExchange()
{
    _error = FLPROG_DHCP_ERROR_NONE;
    _xid = makeTransactionId();
    _exchangeStarted = millis();
    _lastSend = 0;
    _exchangeAttempt = 0;
    _exchange = EXCHANGE_DISCOVER;
    setState(FLPROG_DHCP_STATE_OPENING);
}

void FLProgWiznetDhcp::startRenewExchange(bool rebind)
{
    _xid = makeTransactionId();
    _exchangeStarted = millis();
    _lastSend = 0;
    _exchangeAttempt = 0;
    _exchange = rebind ? EXCHANGE_REBIND : EXCHANGE_RENEW;
    setState(rebind ? FLPROG_DHCP_STATE_REBINDING : FLPROG_DHCP_STATE_RENEWING);
}

void FLProgWiznetDhcp::pool()
{
    if (_network == nullptr || _state == FLPROG_DHCP_STATE_DISABLED)
    {
        return;
    }

    const uint32_t now = millis();

    if (_socketReleasePending)
    {
        closeSocket();
    }

    if (_state == FLPROG_DHCP_STATE_FAILED)
    {
        return;
    }

    if (_state == FLPROG_DHCP_STATE_RATE_LIMIT)
    {
        if ((int32_t)(now - _rateLimitUntil) < 0)
        {
            return;
        }
        _rateLimitUntil = 0;
        setState(FLPROG_DHCP_STATE_INIT);
    }

    if (_state == FLPROG_DHCP_STATE_ANNOUNCING)
    {
        if (_arpAddressDefenseEnabled && takeAddressConflict())
        {
            _arpProbe.stop();
            _arpProbeStarted = false;
            _declinePending = true;
            _conflictProbeStarted = now;
            setState(FLPROG_DHCP_STATE_CHECKING);
            return;
        }

        if (!_arpProbeStarted)
        {
            startAnnouncement(2, FLPROG_DHCP_STATE_BOUND);
            if (_state == FLPROG_DHCP_STATE_FAILED)
            {
                return;
            }
        }

        const uint8_t result = _arpProbe.pool();
        if (result == FLPROG_ARP_PROBE_CLEAR)
        {
            _arpAnnouncementCount += _arpProbe.announcementsSent();
            _arpProbeStarted = false;
            completeBound();
            return;
        }
        if (result == FLPROG_ARP_PROBE_ERROR)
        {
            fail(FLPROG_DHCP_ERROR_ARP_COMPLETION);
        }
        return;
    }

    if (_state == FLPROG_DHCP_STATE_DEFENDING)
    {
        if (takeAddressConflict() &&
            _lastDefense != 0 &&
            (uint32_t)(now - _lastDefense) < FLPROG_ARP_DEFEND_INTERVAL_MS)
        {
            _arpDefenseConflictCount++;
            _arpProbe.stop();
            _arpProbeStarted = false;
            closeSocket();
            applyZeroAddress();
            _declinePending = true;
            _conflictProbeStarted = now == 0 ? 1 : now;
            _error = FLPROG_DHCP_ERROR_PROTOCOL;
            setState(FLPROG_DHCP_STATE_CHECKING);
            return;
        }

        if (!_arpProbeStarted)
        {
            startAnnouncement(1, FLPROG_DHCP_STATE_BOUND);
            if (_state == FLPROG_DHCP_STATE_FAILED)
            {
                return;
            }
        }

        const uint8_t result = _arpProbe.pool();
        if (result == FLPROG_ARP_PROBE_CLEAR)
        {
            const uint8_t sent = _arpProbe.announcementsSent();
            _arpAnnouncementCount += sent;
            _arpDefenseCount += sent;
            _arpProbeStarted = false;
            setState(FLPROG_DHCP_STATE_BOUND);
            return;
        }
        if (result == FLPROG_ARP_PROBE_ERROR)
        {
            fail(FLPROG_DHCP_ERROR_ARP_COMPLETION);
        }
        return;
    }

    if (_state == FLPROG_DHCP_STATE_CHECKING)
    {
        // DECLINE delivery has priority over any configured probe hook. This
        // state is also used after a late address-defense conflict.
        if (_declinePending)
        {
            if (!ensureHardware() || !ensureSocket())
            {
                if (_conflictProbeTimeout > 0 && _conflictProbeStarted != 0 &&
                    (uint32_t)(now - _conflictProbeStarted) >= _conflictProbeTimeout)
                {
                    fail(FLPROG_DHCP_ERROR_CONFLICT_PROBE);
                }
                return;
            }

            _udp->pool();
            if (sendDecline())
            {
                closeSocket();
                clearLease(true);
                _error = FLPROG_DHCP_ERROR_PROTOCOL;
                scheduleReacquireAfterConflict();
            }
            return;
        }

        if (_conflictProbe != nullptr)
        {
            const int8_t probeResult = _conflictProbe(_leasedIp, _conflictProbeContext);

            if (probeResult == FLPROG_DHCP_PROBE_CLEAR)
            {
                finalizeAck();
                return;
            }

            if (probeResult == FLPROG_DHCP_PROBE_CONFLICT)
            {
                sendDecline();
                closeSocket();
                clearLease(true);
                _error = FLPROG_DHCP_ERROR_PROTOCOL;
                scheduleReacquireAfterConflict();
                return;
            }
        }
        else
        {
            // The DHCP socket may still be finishing CLOSE. Socket 0 must be
            // free before the internal MACRAW probe can reserve it.
            if (_nativeSocket != FLPROG_SOCKET_NATIVE_NONE || _socketReleasePending)
            {
                if (!closeSocket())
                {
                    if (_conflictProbeTimeout > 0 && _conflictProbeStarted != 0 &&
                        (uint32_t)(now - _conflictProbeStarted) >= _conflictProbeTimeout)
                    {
                        fail(FLPROG_DHCP_ERROR_CONFLICT_PROBE);
                    }
                    return;
                }
            }

            if (!_arpProbeStarted)
            {
                if (!_arpProbe.begin(*_network, _leasedIp))
                {
                    fail(FLPROG_DHCP_ERROR_CONFLICT_PROBE);
                    return;
                }
                _arpProbeStarted = true;
            }

            const uint8_t arpResult = _arpProbe.pool();
            if (arpResult == FLPROG_ARP_PROBE_CLEAR)
            {
                finalizeAck();
                return;
            }
            if (arpResult == FLPROG_ARP_PROBE_CONFLICT)
            {
                _arpProbeStarted = false;
                _declinePending = true;
                return;
            }
            if (arpResult == FLPROG_ARP_PROBE_ERROR)
            {
                fail(FLPROG_DHCP_ERROR_CONFLICT_PROBE);
                return;
            }
        }

        if (_conflictProbeTimeout > 0 && _conflictProbeStarted != 0 &&
            (uint32_t)(now - _conflictProbeStarted) >= _conflictProbeTimeout)
        {
            fail(FLPROG_DHCP_ERROR_CONFLICT_PROBE);
        }
        return;
    }

    if (_state == FLPROG_DHCP_STATE_BOUND)
    {
        if (_acdMode == FLPROG_ACD_MODE_STRICT)
        {
            if (!_strictArpMonitor.active() && _network != nullptr)
                _strictArpMonitor.begin(*_network, _leasedIp);
            _strictArpMonitor.pool();
        }
        else if (_strictArpMonitor.active())
        {
            _strictArpMonitor.stop();
        }

        if (_arpAddressDefenseEnabled && takeAddressConflict())
        {
            _arpDefenseConflictCount++;

            if (_lastDefense != 0 &&
                (uint32_t)(now - _lastDefense) < FLPROG_ARP_DEFEND_INTERVAL_MS)
            {
                closeSocket();
                applyZeroAddress();
                _declinePending = true;
                _conflictProbeStarted = now == 0 ? 1 : now;
                _error = FLPROG_DHCP_ERROR_PROTOCOL;
                setState(FLPROG_DHCP_STATE_CHECKING);
                return;
            }

            _lastDefense = now == 0 ? 1 : now;
            _arpProbeStarted = false;
            setState(FLPROG_DHCP_STATE_DEFENDING);
            return;
        }

        const uint32_t age = leaseAge();
        if (_leaseTime > 0 && age >= _leaseTime)
        {
            clearLease(true);
            _error = FLPROG_DHCP_ERROR_TIMEOUT;
            setState(FLPROG_DHCP_STATE_INIT);
            return;
        }

        if (_renewalTime > 0 && age >= _renewalTime)
        {
            startRenewExchange(false);
        }
        else
        {
            return;
        }
    }

    if (_state == FLPROG_DHCP_STATE_RENEWING &&
        _rebindingTime > 0 && leaseAge() >= _rebindingTime)
    {
        closeSocket();
        startRenewExchange(true);
    }

    if (_state == FLPROG_DHCP_STATE_REBINDING &&
        _leaseTime > 0 && leaseAge() >= _leaseTime)
    {
        clearLease(true);
        _error = FLPROG_DHCP_ERROR_TIMEOUT;
        setState(FLPROG_DHCP_STATE_INIT);
        return;
    }

    if (!ensureHardware())
    {
        return;
    }

    if (_state == FLPROG_DHCP_STATE_INIT)
    {
        startInitialExchange();
    }

    if (!ensureSocket())
    {
        return;
    }

    _udp->pool();

    if ((_state == FLPROG_DHCP_STATE_RENEWING ||
         _state == FLPROG_DHCP_STATE_REBINDING) &&
        _lastSend == 0)
    {
        sendRenewRequest(_state == FLPROG_DHCP_STATE_REBINDING);
    }

    if (_state == FLPROG_DHCP_STATE_OPENING)
    {
        if (_exchange == EXCHANGE_DISCOVER)
        {
            if (sendDiscover())
            {
                setState(FLPROG_DHCP_STATE_SELECTING);
            }
        }
        else if (_exchange == EXCHANGE_RENEW)
        {
            sendRenewRequest(false);
        }
        else if (_exchange == EXCHANGE_REBIND)
        {
            sendRenewRequest(true);
        }
    }

    Reply reply;
    if (receiveReply(reply))
    {
        if (reply.messageType == FLPROG_DHCP_MESSAGE_NAK)
        {
            handleNak();
            return;
        }

        if (_state == FLPROG_DHCP_STATE_SELECTING &&
            reply.messageType == FLPROG_DHCP_MESSAGE_OFFER)
        {
            acceptOffer(reply);
            if (sendSelectRequest())
            {
                _lastSend = millis();
            }
            return;
        }

        if ((_state == FLPROG_DHCP_STATE_REQUESTING ||
             _state == FLPROG_DHCP_STATE_RENEWING ||
             _state == FLPROG_DHCP_STATE_REBINDING) &&
            reply.messageType == FLPROG_DHCP_MESSAGE_ACK)
        {
            acceptAck(reply);
            return;
        }
    }

    if ((_state == FLPROG_DHCP_STATE_SELECTING ||
         _state == FLPROG_DHCP_STATE_REQUESTING) &&
        acquireExpired(now))
    {
        fail(FLPROG_DHCP_ERROR_TIMEOUT);
        return;
    }

    if (!responseExpired(now))
    {
        return;
    }

    if (_state == FLPROG_DHCP_STATE_SELECTING)
    {
        sendDiscover();
    }
    else if (_state == FLPROG_DHCP_STATE_REQUESTING)
    {
        sendSelectRequest();
    }
    else if (_state == FLPROG_DHCP_STATE_RENEWING)
    {
        sendRenewRequest(false);
    }
    else if (_state == FLPROG_DHCP_STATE_REBINDING)
    {
        sendRenewRequest(true);
    }
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
