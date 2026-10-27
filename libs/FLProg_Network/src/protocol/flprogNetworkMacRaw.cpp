/*
 * FLProg_Network — flprogNetworkMacRaw.cpp
 * Назначение: MACRAW helper для низкоуровневых Wiznet кадров.
 *
 * Архитектурные заметки:
 * - Применяется для L2-задач, недоступных обычному TCP/UDP socket API.
 * - Работает только через wired/Wiznet backend и должен быть profile-guarded.
 * - Не использовать MACRAW как обход A3 для обычного прикладного трафика.
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
#include "flprogNetworkMacRaw.h"
#include "../flprogNetwork.h"

bool FLProgWiznetMacRawBackend::validSocket() const
{
    return
        _hardware != nullptr &&
        _hardware->isInit() &&
        _socket == 0 &&
        _hardware->maxSocketNum() > 0;
}

bool FLProgWiznetMacRawBackend::spiWasDeferred()
{
    if (_hardware == nullptr)
    {
        return true;
    }

    const uint8_t status = _hardware->lastSpiStatus();
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

uint8_t FLProgWiznetMacRawBackend::read8(uint16_t address, bool &ok)
{
    ok = false;
    if (!validSocket()) return 0;
    const uint8_t value = _hardware->readSocket(0, address);
    if (spiWasDeferred()) return 0;
    ok = true;
    return value;
}

uint16_t FLProgWiznetMacRawBackend::read16(uint16_t address, bool &ok)
{
    ok = false;
    if (!validSocket()) return 0;
    uint8_t data[2] = {0, 0};
    _hardware->readSocket(0, address, data, 2);
    if (spiWasDeferred()) return 0;
    ok = true;
    return ((uint16_t)data[0] << 8) | data[1];
}

bool FLProgWiznetMacRawBackend::write8(uint16_t address, uint8_t value)
{
    if (!validSocket()) return false;
    _hardware->writeSocket(0, address, value);
    return !spiWasDeferred();
}

bool FLProgWiznetMacRawBackend::write16(uint16_t address, uint16_t value)
{
    if (!validSocket()) return false;
    uint8_t data[2] = {(uint8_t)(value >> 8), (uint8_t)value};
    _hardware->writeSocket(0, address, data, 2);
    return !spiWasDeferred();
}

bool FLProgWiznetMacRawBackend::issueCommand(uint8_t command)
{
    bool ok = false;
    const uint8_t current = read8(FLPROG_WIZNET_MACRAW_SN_CR, ok);
    if (!ok || current != 0) return false;
    return write8(FLPROG_WIZNET_MACRAW_SN_CR, command);
}

bool FLProgWiznetMacRawBackend::commandFinished(bool &ok)
{
    const uint8_t value = read8(FLPROG_WIZNET_MACRAW_SN_CR, ok);
    return ok && value == 0;
}

void FLProgWiznetMacRawBackend::resetFrameState()
{
    _txActive = false;
    _txWritePointer = 0;
    _txCapacity = 0;
    _txLength = 0;

    _rxActive = false;
    _rxFrameStart = 0;
    _rxReadPointer = 0;
    _rxFrameEnd = 0;
    _rxFrameSize = 0;
    _rxRemaining = 0;
}

void FLProgWiznetMacRawBackend::resetSocketState()
{
    _socketStatus = 0;
    _openConfigured = false;
    _openCommandSent = false;
    _closeCommandSent = false;
    _sendPending = false;
    _recvCommandPending = false;
    _started = 0;
    resetFrameState();
}

bool FLProgWiznetMacRawBackend::configureOpen()
{
    if (!validSocket()) return false;

    bool ok = false;
    const uint8_t status = read8(FLPROG_WIZNET_MACRAW_SN_SR, ok);
    if (!ok) return false;

    if (status != 0x00)
    {
        if (!issueCommand(FLPROG_WIZNET_MACRAW_CMD_CLOSE)) return false;
        return false;
    }

    const uint8_t mode = _hardware->prepareMacRawMode(
        _macFilterEnabled,
        _broadcastBlocking
    );

    if (!write8(FLPROG_WIZNET_MACRAW_SN_MR, mode)) return false;

    _openConfigured = true;
    return true;
}

uint8_t FLProgWiznetMacRawBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    (void)endpoint;

    if (
        _hardware == nullptr ||
        !_hardware->isInit() ||
        lease.transportId != FLPROG_TRANSPORT_WIZNET ||
        lease.nativeSocket != 0
    )
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (protocol != FLPROG_SOCKET_PROTOCOL_RAW)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_socket != 0)
    {
        _socket = 0;
        resetSocketState();
    }

    _timeout = timeout;
    if (_started == 0) _started = millis();

    if (!_openConfigured)
    {
        if (!configureOpen()) return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (!_openCommandSent)
    {
        if (!issueCommand(FLPROG_WIZNET_MACRAW_CMD_OPEN))
            return FLPROG_SOCKET_RESULT_WAIT;
        _openCommandSent = true;
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    return pollOpen()
        ? FLPROG_SOCKET_RESULT_SUCCESS
        : FLPROG_SOCKET_RESULT_WAIT;
}

bool FLProgWiznetMacRawBackend::pollOpen()
{
    bool ok = false;
    if (!commandFinished(ok) || !ok) return false;
    _socketStatus = read8(FLPROG_WIZNET_MACRAW_SN_SR, ok);
    return ok && _socketStatus == FLPROG_WIZNET_SOCK_MACRAW;
}

void FLProgWiznetMacRawBackend::pollInterrupts()
{
    if (!validSocket()) return;
    bool ok = false;
    const uint8_t interrupts = read8(FLPROG_WIZNET_MACRAW_SN_IR, ok);
    if (!ok) return;

    const uint8_t clearMask = interrupts &
        (FLPROG_WIZNET_MACRAW_IR_SEND_OK | FLPROG_WIZNET_MACRAW_IR_TIMEOUT);
    if (clearMask == 0) return;
    if (!write8(FLPROG_WIZNET_MACRAW_SN_IR, clearMask)) return;
    _sendPending = false;
}

uint8_t FLProgWiznetMacRawBackend::pool()
{
    if (!validSocket()) return FLPROG_SOCKET_RESULT_WAIT;

    if (_recvCommandPending)
    {
        if (issueCommand(FLPROG_WIZNET_MACRAW_CMD_RECV))
            _recvCommandPending = false;
    }

    bool ok = false;
    _socketStatus = read8(FLPROG_WIZNET_MACRAW_SN_SR, ok);
    if (!ok) return FLPROG_SOCKET_RESULT_WAIT;

    pollInterrupts();

    if (
        _timeout > 0 && _started > 0 && !isOpen() &&
        (uint32_t)(millis() - _started) >= _timeout
    )
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

uint8_t FLProgWiznetMacRawBackend::close()
{
    if (!validSocket())
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        resetSocketState();
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    bool ok = false;
    _socketStatus = read8(FLPROG_WIZNET_MACRAW_SN_SR, ok);
    if (!ok) return FLPROG_SOCKET_RESULT_WAIT;

    if (_socketStatus == 0x00 && !_closeCommandSent)
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        resetSocketState();
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (!_closeCommandSent)
    {
        if (!issueCommand(FLPROG_WIZNET_MACRAW_CMD_CLOSE))
            return FLPROG_SOCKET_RESULT_WAIT;
        _closeCommandSent = true;
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (!commandFinished(ok) || !ok) return FLPROG_SOCKET_RESULT_WAIT;
    _socketStatus = read8(FLPROG_WIZNET_MACRAW_SN_SR, ok);
    if (!ok || _socketStatus != 0x00) return FLPROG_SOCKET_RESULT_WAIT;

    _socket = FLPROG_SOCKET_NATIVE_NONE;
    resetSocketState();
    return FLPROG_SOCKET_RESULT_SUCCESS;
}

bool FLProgWiznetMacRawBackend::isOpen() const
{
    return _socket == 0 && _socketStatus == FLPROG_WIZNET_SOCK_MACRAW;
}

int FLProgWiznetMacRawBackend::beginFrame()
{
    if (!isOpen() || _sendPending || _txActive) return 0;

    bool ok = false;
    uint16_t freeSize = read16(FLPROG_WIZNET_MACRAW_SN_TX_FSR, ok);
    if (!ok || freeSize == 0) return 0;
    if (freeSize > FLPROG_MACRAW_MAX_FRAME_SIZE)
        freeSize = FLPROG_MACRAW_MAX_FRAME_SIZE;

    const uint16_t writePointer = read16(FLPROG_WIZNET_MACRAW_SN_TX_WR, ok);
    if (!ok) return 0;

    _txActive = true;
    _txWritePointer = writePointer;
    _txCapacity = freeSize;
    _txLength = 0;
    return 1;
}

size_t FLProgWiznetMacRawBackend::frameWrite(
    const uint8_t *buffer,
    size_t length)
{
    if (!_txActive || buffer == nullptr || length == 0) return 0;

    const uint16_t space = (uint16_t)(_txCapacity - _txLength);
    if (space == 0) return 0;

    uint16_t amount = (uint16_t)length;
    if (amount > space) amount = space;

    _hardware->writeTxBuffer(0, _txWritePointer, buffer, amount);
    if (spiWasDeferred()) return 0;

    _txWritePointer = (uint16_t)(_txWritePointer + amount);
    _txLength = (uint16_t)(_txLength + amount);
    return amount;
}

int FLProgWiznetMacRawBackend::endFrame()
{
    if (!_txActive || _sendPending || _txLength == 0) return 0;

    if (!write16(FLPROG_WIZNET_MACRAW_SN_TX_WR, _txWritePointer)) return 0;
    if (!issueCommand(FLPROG_WIZNET_MACRAW_CMD_SEND)) return 0;

    _txActive = false;
    _sendPending = true;
    return 1;
}

bool FLProgWiznetMacRawBackend::commitRxFrame()
{
    if (!_rxActive) return true;
    if (!write16(FLPROG_WIZNET_MACRAW_SN_RX_RD, _rxFrameEnd)) return false;

    _rxActive = false;
    _rxRemaining = 0;
    _recvCommandPending = true;
    return true;
}

int FLProgWiznetMacRawBackend::parseFrame()
{
    if (!isOpen()) return 0;

    if (_rxActive)
    {
        if (!commitRxFrame()) return 0;
    }

    if (_recvCommandPending) return 0;

    bool ok = false;
    const uint16_t received = read16(FLPROG_WIZNET_MACRAW_SN_RX_RSR, ok);
    if (!ok || received < 2) return 0;

    const uint16_t readPointer = read16(FLPROG_WIZNET_MACRAW_SN_RX_RD, ok);
    if (!ok) return 0;

    uint8_t header[2] = {0, 0};
    _hardware->readRxBuffer(0, readPointer, header, 2);
    if (spiWasDeferred()) return 0;

    const uint16_t packetLength = ((uint16_t)header[0] << 8) | header[1];
    if (packetLength > received) return 0;

    if (packetLength < 2 || packetLength > (FLPROG_MACRAW_MAX_FRAME_SIZE + 2U))
    {
        _invalidFrameCount++;
        const uint16_t skip = packetLength >= 2 ? packetLength : 2;
        if (!write16(FLPROG_WIZNET_MACRAW_SN_RX_RD, (uint16_t)(readPointer + skip)))
            return 0;
        _recvCommandPending = true;
        return 0;
    }

    _rxActive = true;
    _rxFrameStart = readPointer;
    _rxReadPointer = (uint16_t)(readPointer + 2U);
    _rxFrameEnd = (uint16_t)(readPointer + packetLength);
    _rxFrameSize = (uint16_t)(packetLength - 2U);
    _rxRemaining = _rxFrameSize;

    return _rxFrameSize;
}

int FLProgWiznetMacRawBackend::frameAvailable()
{
    return _rxActive ? _rxRemaining : 0;
}

int FLProgWiznetMacRawBackend::readCurrentFrame(
    uint8_t *buffer,
    size_t length,
    bool advance)
{
    if (!_rxActive || _rxRemaining == 0 || buffer == nullptr || length == 0)
        return 0;

    uint16_t amount = (uint16_t)length;
    if (amount > _rxRemaining) amount = _rxRemaining;

    _hardware->readRxBuffer(0, _rxReadPointer, buffer, amount);
    if (spiWasDeferred()) return 0;

    if (advance)
    {
        _rxReadPointer = (uint16_t)(_rxReadPointer + amount);
        _rxRemaining = (uint16_t)(_rxRemaining - amount);
    }
    return amount;
}

int FLProgWiznetMacRawBackend::frameRead()
{
    uint8_t value = 0;
    return readCurrentFrame(&value, 1, true) == 1 ? value : -1;
}

int FLProgWiznetMacRawBackend::frameRead(uint8_t *buffer, size_t length)
{
    return readCurrentFrame(buffer, length, true);
}

int FLProgWiznetMacRawBackend::framePeek()
{
    uint8_t value = 0;
    return readCurrentFrame(&value, 1, false) == 1 ? value : -1;
}

void FLProgWiznetMacRawBackend::frameFlush()
{
    if (_rxActive) _rxRemaining = 0;
}

FLProgNetworkMacRaw::FLProgNetworkMacRaw()
{
    preferredNativeSocket(0);
}

bool FLProgNetworkMacRaw::begin(FLProgNetworkInterface &network)
{
    _rawNetwork = &network;
    _wiznetBackend.hardware(network.wiznetHardware());
    _wiznetBackend.setMacFilterEnabled(_macFilterEnabled);

    if (!_broadcastBlockingExplicit)
        _broadcastBlocking = network.broadcastBlocking();
    _wiznetBackend.setBroadcastBlocking(_broadcastBlocking);

    setBackend(FLPROG_TRANSPORT_WIZNET, &_wiznetBackend);

    if (!FLProgNetworkSocket::begin(
        network,
        FLPROG_NETWORK_ROUTE_WIZNET,
        FLPROG_SOCKET_PROTOCOL_RAW))
    {
        _rawNetwork = nullptr;
        return false;
    }

    requestOpen();
    return true;
}

void FLProgNetworkMacRaw::stop()
{
    abort();
    end();
    _rawNetwork = nullptr;
}

void FLProgNetworkMacRaw::setMacFilterEnabled(bool value)
{
    if (_macFilterEnabled == value) return;
    _macFilterEnabled = value;
    _wiznetBackend.setMacFilterEnabled(value);
    requestRebind();
}

void FLProgNetworkMacRaw::pool()
{
    if (_rawNetwork != nullptr)
    {
        if (!_broadcastBlockingExplicit)
            _broadcastBlocking = _rawNetwork->broadcastBlocking();

        _wiznetBackend.setMacFilterEnabled(_macFilterEnabled);
        _wiznetBackend.setBroadcastBlocking(_broadcastBlocking);
    }

    FLProgNetworkSocket::pool();
}

int FLProgNetworkMacRaw::beginFrame()
{
    return isOpen() ? _wiznetBackend.beginFrame() : 0;
}

size_t FLProgNetworkMacRaw::write(uint8_t value)
{
    return write(&value, 1);
}

size_t FLProgNetworkMacRaw::write(const uint8_t *buffer, size_t length)
{
    return isOpen() ? _wiznetBackend.frameWrite(buffer, length) : 0;
}

int FLProgNetworkMacRaw::endFrame()
{
    return isOpen() ? _wiznetBackend.endFrame() : 0;
}

int FLProgNetworkMacRaw::parseFrame()
{
    return isOpen() ? _wiznetBackend.parseFrame() : 0;
}

int FLProgNetworkMacRaw::available()
{
    return isOpen() ? _wiznetBackend.frameAvailable() : 0;
}

int FLProgNetworkMacRaw::read()
{
    return isOpen() ? _wiznetBackend.frameRead() : -1;
}

int FLProgNetworkMacRaw::read(uint8_t *buffer, size_t length)
{
    return isOpen() ? _wiznetBackend.frameRead(buffer, length) : 0;
}

int FLProgNetworkMacRaw::peek()
{
    return isOpen() ? _wiznetBackend.framePeek() : -1;
}

void FLProgNetworkMacRaw::flush()
{
    if (isOpen()) _wiznetBackend.frameFlush();
}

uint16_t FLProgNetworkMacRaw::frameSize() const
{
    return isOpen() ? _wiznetBackend.frameSize() : 0;
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
