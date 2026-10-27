/*
 * FLProg_Network — flprogNetworkArpProbe.cpp
 * Назначение: ARP probe/проверка адреса для wired Ethernet.
 *
 * Архитектурные заметки:
 * - Используется только там, где backend действительно поддерживает L2/ARP.
 * - Проверка не должна блокировать основной pool().
 * - WiFi-only профиль обязан исключать этот код из линковки.
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
#include "flprogNetworkArpProbe.h"
#include "../flprogNetwork.h"
#include <string.h>

// RFC 5227 sections 2.1 and 2.3 timing constants.
static const uint32_t FLPROG_ARP_PROBE_WAIT_MS = 1000UL;
static const uint8_t FLPROG_ARP_PROBE_NUM = 3;
static const uint32_t FLPROG_ARP_PROBE_MIN_MS = 1000UL;
static const uint32_t FLPROG_ARP_PROBE_MAX_MS = 2000UL;
static const uint32_t FLPROG_ARP_ANNOUNCE_WAIT_MS = 2000UL;
static const uint32_t FLPROG_ARP_ANNOUNCE_INTERVAL_MS = 2000UL;

FLProgWiznetArpProbe::~FLProgWiznetArpProbe()
{
    stop();
}

bool FLProgWiznetArpProbe::begin(FLProgNetworkInterface &network, IPAddress address)
{
    return beginOperation(network, address, false, 0);
}

bool FLProgWiznetArpProbe::beginAnnouncement(
    FLProgNetworkInterface &network,
    IPAddress address,
    uint8_t count)
{
    if (count == 0)
    {
        return false;
    }
    return beginOperation(network, address, true, count);
}

bool FLProgWiznetArpProbe::beginOperation(
    FLProgNetworkInterface &network,
    IPAddress address,
    bool announcement,
    uint8_t announcementCount)
{
    stop();

    if (address == IPAddress(0, 0, 0, 0))
    {
        return false;
    }

    _network = &network;
    _address = address;
    memcpy(_mac, network.mac(), sizeof(_mac));
    _probesSent = 0;
    _announcementsSent = 0;
    _announcementTargetCount = announcementCount;
    _announcementMode = announcement;
    _result = FLPROG_ARP_PROBE_PENDING;
    _active = true;
    _state = STATE_OPENING;
    _nextAction = 0;
    _txOffset = 0;
    _txBuilding = false;

    _wiznetBackend.hardware(network.wiznetHardware());
    _wiznetBackend.setMacFilterEnabled(false);
    _wiznetBackend.setBroadcastBlocking(false);
    _backend = (_externalBackend != nullptr)
        ? _externalBackend
        : static_cast<FLProgMacRawBackend *>(&_wiznetBackend);

    return true;
}

void FLProgWiznetArpProbe::stop()
{
    if (_backend != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        _backend->close();
    }

    if (
        _externalBackend == nullptr &&
        _network != nullptr &&
        _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
        _wiznetBackend = FLProgWiznetMacRawBackend();
    }

    _network = nullptr;
    _backend = nullptr;
    _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    _lease = FLProgSocketLease();
    _endpoint = FLProgSocketEndpoint();
    _state = STATE_IDLE;
    _result = FLPROG_ARP_PROBE_PENDING;
    _active = false;
    _nextAction = 0;
    _txOffset = 0;
    _txBuilding = false;
    _announcementMode = false;
    _announcementTargetCount = 0;
}

bool FLProgWiznetArpProbe::ensureSocket()
{
    if (_network == nullptr || _backend == nullptr)
    {
        return false;
    }

    if (_backend->isOpen())
    {
        return true;
    }

    if (_nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
    {
        if (_externalBackend != nullptr)
        {
            _nativeSocket = 0;
        }
        else
        {
            _nativeSocket = _network->wiznetTransport().reserveNativeSocket(this, 0);
            if (_nativeSocket != 0)
            {
                if (_nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
                {
                    _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
                }
                _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
                return false;
            }
        }

        _lease = FLProgSocketLease();
        _lease.used = true;
        _lease.owner = this;
        _lease.transportId = FLPROG_TRANSPORT_WIZNET;
        _lease.nativeSocket = 0;
        _lease.requestedNativeSocket = 0;
        _lease.transportGeneration = _network->wiznetTransport().generation();
        _endpoint = FLProgSocketEndpoint();
    }

    const uint8_t result = _backend->open(
        _lease,
        FLPROG_SOCKET_PROTOCOL_RAW,
        _endpoint,
        2500UL
    );

    if (result == FLPROG_SOCKET_RESULT_ERROR ||
        result == FLPROG_SOCKET_RESULT_UNSUPPORTED)
    {
        beginClosing(FLPROG_ARP_PROBE_ERROR);
        return false;
    }

    _backend->pool();
    return _backend->isOpen();
}

bool FLProgWiznetArpProbe::closeSocket()
{
    if (_backend != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        const uint8_t result = _backend->close();
        if (result != FLPROG_SOCKET_RESULT_SUCCESS)
        {
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
    return true;
}

uint32_t FLProgWiznetArpProbe::initialDelay() const
{
    uint32_t seed = micros() ^ millis() ^ (uint32_t)_address;
    for (uint8_t i = 0; i < 6; i++)
    {
        seed = (seed * 33UL) ^ _mac[i];
    }
    return seed % (FLPROG_ARP_PROBE_WAIT_MS + 1UL);
}

uint32_t FLProgWiznetArpProbe::repeatDelay() const
{
    uint32_t seed = micros() ^ millis() ^ ((uint32_t)_probesSent << 24) ^ (uint32_t)_address;
    const uint32_t span = FLPROG_ARP_PROBE_MAX_MS - FLPROG_ARP_PROBE_MIN_MS;
    return FLPROG_ARP_PROBE_MIN_MS + (seed % (span + 1UL));
}

bool FLProgWiznetArpProbe::sendProbe()
{
    if (!sendArpRequest(false))
    {
        return false;
    }
    _probesSent++;
    return true;
}

bool FLProgWiznetArpProbe::sendAnnouncement()
{
    if (!sendArpRequest(true))
    {
        return false;
    }
    _announcementsSent++;
    return true;
}

bool FLProgWiznetArpProbe::sendArpRequest(bool announcement)
{
    if (_backend == nullptr || !_backend->isOpen())
    {
        return false;
    }

    if (!_txBuilding)
    {
        if (_backend->beginFrame() != 1)
        {
            return false;
        }

        memset(_txFrame, 0, sizeof(_txFrame));

        // Ethernet broadcast header.
        memset(&_txFrame[0], 0xFF, 6);
        memcpy(&_txFrame[6], _mac, 6);
        _txFrame[12] = 0x08;
        _txFrame[13] = 0x06;

        // ARP REQUEST, Ethernet/IPv4.
        _txFrame[14] = 0x00;
        _txFrame[15] = 0x01;
        _txFrame[16] = 0x08;
        _txFrame[17] = 0x00;
        _txFrame[18] = 6;
        _txFrame[19] = 4;
        _txFrame[20] = 0x00;
        _txFrame[21] = 0x01;
        memcpy(&_txFrame[22], _mac, 6);

        // Probe: SPA=0.0.0.0, TPA=candidate.
        // Announcement/defense: SPA=TPA=assigned address.
        if (announcement)
        {
            _txFrame[28] = _address[0];
            _txFrame[29] = _address[1];
            _txFrame[30] = _address[2];
            _txFrame[31] = _address[3];
        }
        _txFrame[38] = _address[0];
        _txFrame[39] = _address[1];
        _txFrame[40] = _address[2];
        _txFrame[41] = _address[3];

        _txOffset = 0;
        _txBuilding = true;
    }

    if (_txOffset < sizeof(_txFrame))
    {
        const size_t written = _backend->frameWrite(
            &_txFrame[_txOffset],
            sizeof(_txFrame) - _txOffset
        );
        _txOffset = (uint8_t)(_txOffset + written);
        if (_txOffset < sizeof(_txFrame))
        {
            return false;
        }
    }

    if (_backend->endFrame() != 1)
    {
        return false;
    }

    _txOffset = 0;
    _txBuilding = false;
    return true;
}

bool FLProgWiznetArpProbe::ipEquals(const uint8_t *bytes, IPAddress ip)
{
    return bytes != nullptr &&
        bytes[0] == ip[0] && bytes[1] == ip[1] &&
        bytes[2] == ip[2] && bytes[3] == ip[3];
}

bool FLProgWiznetArpProbe::bytesAllZero(const uint8_t *bytes, size_t length)
{
    if (bytes == nullptr) return false;
    for (size_t i = 0; i < length; i++)
    {
        if (bytes[i] != 0) return false;
    }
    return true;
}

bool FLProgWiznetArpProbe::isConflictFrame(const uint8_t *frame, size_t length)
{
    if (frame == nullptr || length < 42)
    {
        return false;
    }

    if (frame[12] != 0x08 || frame[13] != 0x06)
    {
        return false;
    }

    if (frame[14] != 0x00 || frame[15] != 0x01 ||
        frame[16] != 0x08 || frame[17] != 0x00 ||
        frame[18] != 6 || frame[19] != 4)
    {
        return false;
    }

    const uint16_t operation = ((uint16_t)frame[20] << 8) | frame[21];
    if (operation != 1 && operation != 2)
    {
        return false;
    }

    const uint8_t *senderMac = &frame[22];
    if (memcmp(senderMac, _mac, 6) == 0)
    {
        return false;
    }

    const uint8_t *senderIp = &frame[28];
    const uint8_t *targetIp = &frame[38];

    bool conflict = ipEquals(senderIp, _address);
    if (!conflict && bytesAllZero(senderIp, 4) && ipEquals(targetIp, _address))
    {
        conflict = true;
    }

    if (conflict)
    {
        memcpy(_conflictMac, senderMac, 6);
        _hasConflictMac = true;
        _conflictCount++;
    }

    return conflict;
}

bool FLProgWiznetArpProbe::receiveConflict()
{
    if (_backend == nullptr || !_backend->isOpen())
    {
        return false;
    }

    const int frameSize = _backend->parseFrame();
    if (frameSize <= 0)
    {
        return false;
    }

    uint8_t frame[64];
    const size_t amount = frameSize < (int)sizeof(frame)
        ? (size_t)frameSize
        : sizeof(frame);
    const int received = _backend->frameRead(frame, amount);
    _backend->frameFlush();

    return received > 0 && isConflictFrame(frame, (size_t)received);
}

void FLProgWiznetArpProbe::beginClosing(uint8_t result)
{
    _result = result;
    _state = STATE_CLOSING;
}

uint8_t FLProgWiznetArpProbe::pool()
{
    if (!_active || _network == nullptr || _backend == nullptr)
    {
        return FLPROG_ARP_PROBE_ERROR;
    }

    if (_state == STATE_CLOSING)
    {
        if (!closeSocket())
        {
            return FLPROG_ARP_PROBE_PENDING;
        }
        _active = false;
        _state = STATE_IDLE;
        return _result;
    }

    if (_state == STATE_OPENING)
    {
        if (!ensureSocket())
        {
            return FLPROG_ARP_PROBE_PENDING;
        }

        if (_announcementMode)
        {
            _state = STATE_ANNOUNCING;
            _nextAction = millis();
        }
        else
        {
            _state = STATE_PROBING;
            _nextAction = millis() + initialDelay();
        }
    }

    _backend->pool();

    if (_state == STATE_PROBING && receiveConflict())
    {
        beginClosing(FLPROG_ARP_PROBE_CONFLICT);
        return FLPROG_ARP_PROBE_PENDING;
    }

    const uint32_t now = millis();
    if ((int32_t)(now - _nextAction) < 0)
    {
        return FLPROG_ARP_PROBE_PENDING;
    }

    if (_state == STATE_ANNOUNCING)
    {
        if (_announcementsSent < _announcementTargetCount)
        {
            if (!sendAnnouncement())
            {
                return FLPROG_ARP_PROBE_PENDING;
            }

            if (_announcementsSent < _announcementTargetCount)
            {
                _nextAction = millis() + FLPROG_ARP_ANNOUNCE_INTERVAL_MS;
            }
            else
            {
                beginClosing(FLPROG_ARP_PROBE_CLEAR);
            }
            return FLPROG_ARP_PROBE_PENDING;
        }

        beginClosing(FLPROG_ARP_PROBE_CLEAR);
        return FLPROG_ARP_PROBE_PENDING;
    }

    if (_state == STATE_PROBING && _probesSent < FLPROG_ARP_PROBE_NUM)
    {
        if (!sendProbe())
        {
            return FLPROG_ARP_PROBE_PENDING;
        }

        if (_probesSent < FLPROG_ARP_PROBE_NUM)
        {
            _nextAction = millis() + repeatDelay();
        }
        else
        {
            _nextAction = millis() + FLPROG_ARP_ANNOUNCE_WAIT_MS;
        }
        return FLPROG_ARP_PROBE_PENDING;
    }

    if (_state == STATE_PROBING)
    {
        beginClosing(FLPROG_ARP_PROBE_CLEAR);
    }
    return FLPROG_ARP_PROBE_PENDING;
}

// -----------------------------------------------------------------------------
// FLProgWiznetArpMonitor
// -----------------------------------------------------------------------------
FLProgWiznetArpMonitor::~FLProgWiznetArpMonitor()
{
    stop();
}

bool FLProgWiznetArpMonitor::begin(FLProgNetworkInterface &network, IPAddress address)
{
    stop();
    if (address == IPAddress(0, 0, 0, 0)) return false;

    _network = &network;
    _address = address;
    memcpy(_mac, network.mac(), sizeof(_mac));
    _wiznetBackend.hardware(network.wiznetHardware());
    _wiznetBackend.setMacFilterEnabled(false);
    _wiznetBackend.setBroadcastBlocking(false);
    _backend = (_externalBackend != nullptr)
        ? _externalBackend
        : static_cast<FLProgMacRawBackend *>(&_wiznetBackend);
    _active = true;
    _resourceBlocked = false;
    _conflictPending = false;
    _hasConflictMac = false;
    return true;
}

void FLProgWiznetArpMonitor::stop()
{
    if (_backend != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        _backend->close();
    }
    if (_externalBackend == nullptr && _network != nullptr && _nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
    {
        _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
        _wiznetBackend = FLProgWiznetMacRawBackend();
    }
    _network = nullptr;
    _backend = nullptr;
    _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    _lease = FLProgSocketLease();
    _endpoint = FLProgSocketEndpoint();
    _active = false;
    _resourceBlocked = false;
    _conflictPending = false;
}

bool FLProgWiznetArpMonitor::ensureSocket()
{
    if (!_active || _network == nullptr || _backend == nullptr) return false;
    if (_backend->isOpen())
    {
        _resourceBlocked = false;
        return true;
    }

    if (_nativeSocket == FLPROG_SOCKET_NATIVE_NONE)
    {
        if (_externalBackend != nullptr)
        {
            _nativeSocket = 0;
        }
        else
        {
            _nativeSocket = _network->wiznetTransport().reserveNativeSocket(this, 0);
            if (_nativeSocket != 0)
            {
                if (_nativeSocket != FLPROG_SOCKET_NATIVE_NONE)
                    _network->wiznetTransport().releaseNativeSocket(_nativeSocket, this);
                _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
                if (!_resourceBlocked) _resourceBlockedCount++;
                _resourceBlocked = true;
                return false;
            }
        }

        _lease = FLProgSocketLease();
        _lease.used = true;
        _lease.owner = this;
        _lease.transportId = FLPROG_TRANSPORT_WIZNET;
        _lease.nativeSocket = 0;
        _lease.requestedNativeSocket = 0;
        _lease.transportGeneration = _network->wiznetTransport().generation();
        _endpoint = FLProgSocketEndpoint();
    }

    const uint8_t result = _backend->open(_lease, FLPROG_SOCKET_PROTOCOL_RAW, _endpoint, 2500UL);
    if (result == FLPROG_SOCKET_RESULT_ERROR || result == FLPROG_SOCKET_RESULT_UNSUPPORTED)
    {
        _resourceBlocked = true;
        return false;
    }
    _backend->pool();
    _resourceBlocked = !_backend->isOpen();
    return _backend->isOpen();
}

bool FLProgWiznetArpMonitor::ipEquals(const uint8_t *bytes, IPAddress ip)
{
    return bytes != nullptr && bytes[0] == ip[0] && bytes[1] == ip[1] && bytes[2] == ip[2] && bytes[3] == ip[3];
}

bool FLProgWiznetArpMonitor::bytesAllZero(const uint8_t *bytes, size_t length)
{
    if (bytes == nullptr) return false;
    for (size_t i = 0; i < length; i++) if (bytes[i] != 0) return false;
    return true;
}

bool FLProgWiznetArpMonitor::isConflictFrame(const uint8_t *frame, size_t length)
{
    if (frame == nullptr || length < 42) return false;
    if (frame[12] != 0x08 || frame[13] != 0x06) return false;
    if (frame[14] != 0x00 || frame[15] != 0x01 || frame[16] != 0x08 || frame[17] != 0x00 || frame[18] != 6 || frame[19] != 4) return false;
    const uint16_t operation = ((uint16_t)frame[20] << 8) | frame[21];
    if (operation != 1 && operation != 2) return false;
    const uint8_t *senderMac = &frame[22];
    if (memcmp(senderMac, _mac, 6) == 0) return false;
    const uint8_t *senderIp = &frame[28];
    const uint8_t *targetIp = &frame[38];
    const bool conflict = ipEquals(senderIp, _address) || (bytesAllZero(senderIp, 4) && ipEquals(targetIp, _address));
    if (conflict)
    {
        memcpy(_conflictMac, senderMac, 6);
        _hasConflictMac = true;
        _conflictCount++;
    }
    return conflict;
}

bool FLProgWiznetArpMonitor::receiveConflict()
{
    if (_backend == nullptr || !_backend->isOpen()) return false;
    const int frameSize = _backend->parseFrame();
    if (frameSize <= 0) return false;
    uint8_t frame[64];
    const size_t amount = frameSize < (int)sizeof(frame) ? (size_t)frameSize : sizeof(frame);
    const int received = _backend->frameRead(frame, amount);
    _backend->frameFlush();
    return received > 0 && isConflictFrame(frame, (size_t)received);
}

void FLProgWiznetArpMonitor::pool()
{
    if (!_active || _network == nullptr || _backend == nullptr) return;
    if (!ensureSocket()) return;
    _backend->pool();
    if (receiveConflict()) _conflictPending = true;
}

bool FLProgWiznetArpMonitor::takeConflict()
{
    const bool value = _conflictPending;
    _conflictPending = false;
    return value;
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
