/*
 * FLProg_Network — flprogSocketBackends.cpp
 * Назначение: Адаптеры socket-операций к конкретным transport backend.
 *
 * Архитектурные заметки:
 * - Маршрутизация выбирается A3, а не legacy-классом клиента.
 * - Backend обязан одинаково трактовать open/close/read/write и ошибки для верхнего socket manager.
 * - Compile-time profile должен исключать недоступные backend-ы из линковки.
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
#include "flprogSocketBackends.h"

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
// -----------------------------------------------------------------------------
// FLProgWiznetSocketBackend
// -----------------------------------------------------------------------------

bool FLProgWiznetSocketBackend::validSocket() const
{
    return
        _hardware != nullptr &&
        _hardware->isInit() &&
        _socket >= 0 &&
        _socket < _hardware->maxSocketNum();
}

bool FLProgWiznetSocketBackend::spiWasDeferred()
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

uint8_t FLProgWiznetSocketBackend::read8(
    uint16_t address,
    bool &ok)
{
    ok = false;

    if (!validSocket())
    {
        return 0;
    }

    const uint8_t value =
        _hardware->readSocket((uint8_t)_socket, address);

    if (spiWasDeferred())
    {
        return 0;
    }

    ok = true;
    return value;
}

uint16_t FLProgWiznetSocketBackend::read16(
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
    return ((uint16_t)data[0] << 8) | data[1];
}

bool FLProgWiznetSocketBackend::write8(
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

bool FLProgWiznetSocketBackend::write16(
    uint16_t address,
    uint16_t value)
{
    uint8_t data[2] = {
        (uint8_t)(value >> 8),
        (uint8_t)(value & 0xFF)
    };

    return writeBlock(address, data, 2);
}

bool FLProgWiznetSocketBackend::writeBlock(
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

bool FLProgWiznetSocketBackend::readBlock(
    uint16_t address,
    uint8_t *buffer,
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

    _hardware->readSocket(
        (uint8_t)_socket,
        address,
        buffer,
        length
    );

    return !spiWasDeferred();
}

bool FLProgWiznetSocketBackend::issueCommand(uint8_t command)
{
    bool ok = false;
    const uint8_t current = read8(FLPROG_WIZNET_SN_CR, ok);

    if (!ok || current != 0)
    {
        return false;
    }

    return write8(FLPROG_WIZNET_SN_CR, command);
}

bool FLProgWiznetSocketBackend::commandFinished(bool &ok)
{
    const uint8_t current = read8(FLPROG_WIZNET_SN_CR, ok);
    return ok && current == 0;
}


bool FLProgWiznetSocketBackend::keepAliveSupported() const
{
    if (_hardware == nullptr)
    {
        return false;
    }

    const uint8_t chip = _hardware->chipCode();
    return
        chip == FLPROG_WIZNET_CHIP_W5100S ||
        chip == FLPROG_WIZNET_CHIP_W5200 ||
        chip == FLPROG_WIZNET_CHIP_W5500;
}

bool FLProgWiznetSocketBackend::automaticKeepAliveSupported() const
{
    if (_hardware == nullptr)
    {
        return false;
    }

    const uint8_t chip = _hardware->chipCode();
    return
        chip == FLPROG_WIZNET_CHIP_W5100S ||
        chip == FLPROG_WIZNET_CHIP_W5500;
}

bool FLProgWiznetSocketBackend::manualKeepAliveSupported() const
{
    if (_hardware == nullptr)
    {
        return false;
    }

    const uint8_t chip = _hardware->chipCode();
    return
        chip == FLPROG_WIZNET_CHIP_W5100S ||
        chip == FLPROG_WIZNET_CHIP_W5200 ||
        chip == FLPROG_WIZNET_CHIP_W5500;
}

bool FLProgWiznetSocketBackend::configureTcpOptions()
{
    if (!validSocket())
    {
        return false;
    }

    _keepAliveApplied = false;

    const uint8_t chip = _hardware->chipCode();

    if (chip == FLPROG_WIZNET_CHIP_W5500 ||
        chip == FLPROG_WIZNET_CHIP_W5100S)
    {
        uint8_t units = 0;
        if (_keepAliveEnabled)
        {
            uint32_t interval = _keepAliveIntervalMs;
            if (interval < 5000UL) interval = 5000UL;
            uint32_t rounded = (interval + 4999UL) / 5000UL;
            if (rounded > 255UL) rounded = 255UL;
            units = (uint8_t)rounded;
        }

        const uint16_t address =
            chip == FLPROG_WIZNET_CHIP_W5500
                ? FLPROG_WIZNET_SN_KPALVTR_W5500
                : FLPROG_WIZNET_SN_KPALVTR_W5100S;

        if (!write8(address, units))
        {
            return false;
        }

        _keepAliveApplied = _keepAliveEnabled;
        return true;
    }

    if (chip == FLPROG_WIZNET_CHIP_W5200)
    {
        // W5200 has SEND_KEEP but no automatic keepalive timer register.
        // pool() therefore schedules SEND_KEEP cooperatively.
        _keepAliveApplied = _keepAliveEnabled;
        return true;
    }

    // W5100 has neither auto keepalive nor SEND_KEEP in the ioLibrary API.
    return true;
}

void FLProgWiznetSocketBackend::keepAliveStep()
{
    if (
        !_keepAliveEnabled ||
        !_keepAliveApplied ||
        _hardware == nullptr ||
        _hardware->chipCode() != FLPROG_WIZNET_CHIP_W5200 ||
        !connected())
    {
        return;
    }

    uint32_t interval = _keepAliveIntervalMs;
    if (interval < 1000UL) interval = 1000UL;

    const uint32_t now = millis();
    if (_lastKeepAliveAt == 0)
    {
        _lastKeepAliveAt = now;
        return;
    }

    if ((uint32_t)(now - _lastKeepAliveAt) < interval)
    {
        return;
    }

    if (_sendPrepared || _sendPending || _recvCommandPending)
    {
        return;
    }

    if (issueCommand(FLPROG_WIZNET_CMD_SEND_KEEP))
    {
        _lastKeepAliveAt = now;
    }
}

bool FLProgWiznetSocketBackend::captureServerPeer()
{
    if (!_serverMode || _serverPeerCaptured || !validSocket())
    {
        return true;
    }

    if (
        _socketStatus != FLPROG_WIZNET_SOCK_ESTABLISHED &&
        _socketStatus != FLPROG_WIZNET_SOCK_CLOSE_WAIT
    )
    {
        return true;
    }

    uint8_t ip[4] = {0, 0, 0, 0};
    if (!readBlock(FLPROG_WIZNET_SN_DIPR, ip, 4))
    {
        return false;
    }

    bool ok = false;
    const uint16_t port = read16(FLPROG_WIZNET_SN_DPORT, ok);
    if (!ok)
    {
        return false;
    }

    _endpoint.remoteIP = IPAddress(ip[0], ip[1], ip[2], ip[3]);
    _endpoint.remotePort = port;
    _serverPeerCaptured = true;
    return true;
}

bool FLProgWiznetSocketBackend::configureOpen()
{
    if (!validSocket())
    {
        return false;
    }

    // v0.8.1 uses this stream backend as a real TCP client. Destination IP
    // and port are configured before OPEN; CONNECT is issued after SOCK_INIT.
    if (_protocol != FLPROG_SOCKET_PROTOCOL_STREAM)
    {
        return false;
    }

    bool ok = false;
    const uint8_t status = read8(FLPROG_WIZNET_SN_SR, ok);

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

    uint8_t mode = FLPROG_WIZNET_MODE_TCP;
    if (_noDelayedAck)
    {
        mode |= FLPROG_WIZNET_MODE_ND;
    }

    if (!write8(FLPROG_WIZNET_SN_MR, mode))
    {
        return false;
    }

    if (!write16(FLPROG_WIZNET_SN_PORT, _effectiveLocalPort))
    {
        return false;
    }

    if (!_serverMode)
    {
        const uint8_t destinationIp[4] = {
            _endpoint.remoteIP[0],
            _endpoint.remoteIP[1],
            _endpoint.remoteIP[2],
            _endpoint.remoteIP[3]
        };

        if (!writeBlock(FLPROG_WIZNET_SN_DIPR, destinationIp, 4))
        {
            return false;
        }

        if (!write16(FLPROG_WIZNET_SN_DPORT, _endpoint.remotePort))
        {
            return false;
        }
    }

    if (!configureTcpOptions())
    {
        return false;
    }

    _openConfigured = true;
    return true;
}

uint8_t FLProgWiznetSocketBackend::open(
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

    if (protocol != FLPROG_SOCKET_PROTOCOL_STREAM)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_socket != lease.nativeSocket)
    {
        _socket = lease.nativeSocket;
        _openConfigured = false;
        _openCommandSent = false;
        _listenCommandSent = false;
        _connectCommandSent = false;
        _closeCommandSent = false;
        _disconnectCommandSent = false;
        _disconnectStartedAt = 0;
        _sendPrepared = false;
        _sendPending = false;
        _recvCommandPending = false;
        _timeoutSeen = false;
        _everConnected = false;
        _serverPeerCaptured = false;
        _keepAliveApplied = false;
        _lastKeepAliveAt = 0;
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_NONE;
        _socketStatus = FLPROG_WIZNET_SOCK_CLOSED;
        _interruptStatus = 0;
        _started = 0;
    }

    _protocol = protocol;
    _endpoint = endpoint;
    _effectiveLocalPort = endpoint.localPort;

    if (_effectiveLocalPort == 0)
    {
        _effectiveLocalPort = (uint16_t)(49152U + (uint16_t)_socket);
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

    bool ok = false;
    if (!commandFinished(ok) || !ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    _socketStatus = read8(FLPROG_WIZNET_SN_SR, ok);
    if (!ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    pollInterrupts();

    if (_timeoutSeen)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (_serverMode && _socketStatus == FLPROG_WIZNET_SOCK_LISTEN)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_socketStatus == FLPROG_WIZNET_SOCK_ESTABLISHED ||
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT)
    {
        _everConnected = true;
        if (!captureServerPeer())
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_socketStatus == FLPROG_WIZNET_SOCK_INIT)
    {
        if (_serverMode)
        {
            if (!_listenCommandSent)
            {
                if (!issueCommand(FLPROG_WIZNET_CMD_LISTEN))
                {
                    return FLPROG_SOCKET_RESULT_WAIT;
                }
                _listenCommandSent = true;
            }
        }
        else if (!_connectCommandSent)
        {
            if (!issueCommand(FLPROG_WIZNET_CMD_CONNECT))
            {
                return FLPROG_SOCKET_RESULT_WAIT;
            }
            _connectCommandSent = true;
        }
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (_socketStatus == FLPROG_WIZNET_SOCK_SYNSENT ||
        _socketStatus == FLPROG_WIZNET_SOCK_SYNRECV)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (((_serverMode && _listenCommandSent) ||
         (!_serverMode && _connectCommandSent)) &&
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSED)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    return FLPROG_SOCKET_RESULT_WAIT;
}

bool FLProgWiznetSocketBackend::pollOpen()
{
    bool ok = false;

    if (!commandFinished(ok))
    {
        return false;
    }

    if (!ok)
    {
        return false;
    }

    _socketStatus = read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return false;
    }

    return
        _protocol == FLPROG_SOCKET_PROTOCOL_STREAM &&
        _socketStatus == FLPROG_WIZNET_SOCK_INIT;
}

void FLProgWiznetSocketBackend::pollInterrupts()
{
    if (!validSocket())
    {
        return;
    }

    bool ok = false;
    _interruptStatus = read8(FLPROG_WIZNET_SN_IR, ok);

    if (!ok)
    {
        return;
    }

    const uint8_t clearMask =
        _interruptStatus &
        (FLPROG_WIZNET_IR_SEND_OK |
         FLPROG_WIZNET_IR_TIMEOUT |
         FLPROG_WIZNET_IR_CONNECT |
         FLPROG_WIZNET_IR_DISCON);

    if (clearMask != 0)
    {
        if (write8(FLPROG_WIZNET_SN_IR, clearMask))
        {
            if (clearMask & FLPROG_WIZNET_IR_SEND_OK)
            {
                _sendPending = false;
            }

            if (clearMask & FLPROG_WIZNET_IR_TIMEOUT)
            {
                _sendPending = false;
                _timeoutSeen = true;
                _lastBackendEvent = _everConnected
                    ? FLPROG_TCP_BACKEND_EVENT_IO_TIMEOUT
                    : FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT;
            }

            if ((clearMask & FLPROG_WIZNET_IR_DISCON) && _everConnected)
            {
                _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
            }
        }
    }
}

uint8_t FLProgWiznetSocketBackend::pool()
{
    if (!validSocket())
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (_recvCommandPending)
    {
        if (issueCommand(FLPROG_WIZNET_CMD_RECV))
        {
            _recvCommandPending = false;
        }
    }

    if (_sendPrepared && !_sendPending)
    {
        if (issueCommand(FLPROG_WIZNET_CMD_SEND))
        {
            _sendPrepared = false;
            _sendPending = true;
        }
    }

    bool ok = false;
    _socketStatus = read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    pollInterrupts();

    if (_serverMode && _socketStatus == FLPROG_WIZNET_SOCK_LISTEN)
    {
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (_socketStatus == FLPROG_WIZNET_SOCK_ESTABLISHED ||
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT)
    {
        if (!_everConnected)
        {
            _lastKeepAliveAt = millis();
        }
        _everConnected = true;
        if (!captureServerPeer())
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }
    }

    keepAliveStep();

    if (_socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT)
    {
        bool rxOk = false;
        const uint16_t pendingRx = read16(FLPROG_WIZNET_SN_RX_RSR, rxOk);
        if (rxOk && pendingRx == 0)
        {
            _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
            return FLPROG_SOCKET_RESULT_ERROR;
        }
    }

    if (_timeoutSeen)
    {
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (_everConnected && _socketStatus == FLPROG_WIZNET_SOCK_CLOSED)
    {
        if (_lastBackendEvent == FLPROG_TCP_BACKEND_EVENT_NONE)
        {
            _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_PEER_CLOSED;
        }
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    if (
        _timeout > 0 &&
        _started > 0 &&
        !isOpen() &&
        (uint32_t)(millis() - _started) >= _timeout
    )
    {
        _lastBackendEvent = FLPROG_TCP_BACKEND_EVENT_CONNECT_TIMEOUT;
        return FLPROG_SOCKET_RESULT_ERROR;
    }

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

uint8_t FLProgWiznetSocketBackend::close()
{
    if (!validSocket())
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    bool ok = false;
    _socketStatus = read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    // RC6-A dev.16-fix7:
    // A graceful TCP DISCON needs a live physical link so the FIN exchange
    // can complete. If the Ethernet cable is already down, W5500 may keep
    // Sn_SR in ESTABLISHED/CLOSE_WAIT and DISCON never reaches CLOSED.
    //
    // In that condition this close belongs to a dead transport generation:
    // skip/bypass graceful DISCON and use the local hardware CLOSE command.
    // Normal user-initiated close with LINK UP keeps the graceful path.
    bool physicalLinkUp = true;
    if (_hardware != nullptr && _hardware->isInit())
    {
        physicalLinkUp = _hardware->linkIsUp();
    }

    if (
        physicalLinkUp &&
        (_socketStatus == FLPROG_WIZNET_SOCK_ESTABLISHED ||
         _socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT) &&
        !_disconnectCommandSent
    )
    {
        if (!issueCommand(FLPROG_WIZNET_CMD_DISCON))
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }

        _disconnectCommandSent = true;
        _disconnectStartedAt = millis();
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    if (
        physicalLinkUp &&
        _disconnectCommandSent &&
        !_closeCommandSent &&
        _socketStatus != FLPROG_WIZNET_SOCK_CLOSED
    )
    {
        // Accepted server sockets must not keep their only reusable hardware
        // slot tied up through a long FIN/TIME_WAIT after an abandoned refresh.
        if (
            _serverMode &&
            _disconnectStartedAt != 0 &&
            (uint32_t)(millis() - _disconnectStartedAt) >=
                FLPROG_WIZNET_SERVER_GRACEFUL_CLOSE_MS
        )
        {
            if (!issueCommand(FLPROG_WIZNET_CMD_CLOSE))
            {
                return FLPROG_SOCKET_RESULT_WAIT;
            }
            _closeCommandSent = true;
        }
        else
        {
            return FLPROG_SOCKET_RESULT_WAIT;
        }
    }

    // If DISCON had already been issued immediately before LINK loss,
    // do not remain trapped waiting for a peer that is no longer reachable.
    if (!physicalLinkUp)
    {
        _disconnectCommandSent = false;
        _disconnectStartedAt = 0;
    }

    if (
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSED &&
        !_closeCommandSent
    )
    {
        _socket = FLPROG_SOCKET_NATIVE_NONE;
        _openConfigured = false;
        _openCommandSent = false;
        _listenCommandSent = false;
        _connectCommandSent = false;
        _disconnectCommandSent = false;
        _disconnectStartedAt = 0;
        _timeoutSeen = false;
        _everConnected = false;
        _serverPeerCaptured = false;
        _keepAliveApplied = false;
        _lastKeepAliveAt = 0;
        _sendPrepared = false;
        _sendPending = false;
        _recvCommandPending = false;
        _started = 0;
        return FLPROG_SOCKET_RESULT_SUCCESS;
    }

    if (!_closeCommandSent)
    {
        if (!issueCommand(FLPROG_WIZNET_CMD_CLOSE))
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

    _socketStatus = read8(FLPROG_WIZNET_SN_SR, ok);

    if (!ok || _socketStatus != FLPROG_WIZNET_SOCK_CLOSED)
    {
        return FLPROG_SOCKET_RESULT_WAIT;
    }

    _socket = FLPROG_SOCKET_NATIVE_NONE;
    _openConfigured = false;
    _openCommandSent = false;
    _listenCommandSent = false;
    _connectCommandSent = false;
    _closeCommandSent = false;
    _disconnectCommandSent = false;
    _disconnectStartedAt = 0;
    _timeoutSeen = false;
    _everConnected = false;
    _serverPeerCaptured = false;
    _keepAliveApplied = false;
    _lastKeepAliveAt = 0;
    _sendPrepared = false;
    _sendPending = false;
    _recvCommandPending = false;
    _started = 0;

    return FLPROG_SOCKET_RESULT_SUCCESS;
}

bool FLProgWiznetSocketBackend::isOpen() const
{
    if (_protocol != FLPROG_SOCKET_PROTOCOL_STREAM)
    {
        return false;
    }

    return
        // SYN_RECV is a live server-side TCP handshake. Treating it as closed
        // made the generic socket state machine invoke open() again while a
        // browser was completing its first SYN, which reset that connection.
        (_serverMode &&
         (_socketStatus == FLPROG_WIZNET_SOCK_LISTEN ||
          _socketStatus == FLPROG_WIZNET_SOCK_SYNRECV)) ||
        _socketStatus == FLPROG_WIZNET_SOCK_ESTABLISHED ||
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT;
}

bool FLProgWiznetSocketBackend::connected() const
{
    return
        _socketStatus == FLPROG_WIZNET_SOCK_ESTABLISHED ||
        _socketStatus == FLPROG_WIZNET_SOCK_CLOSE_WAIT;
}

int FLProgWiznetSocketBackend::available()
{
    if (!connected())
    {
        return 0;
    }

    bool ok = false;
    const uint16_t value = read16(FLPROG_WIZNET_SN_RX_RSR, ok);

    return ok ? (int)value : 0;
}

int FLProgWiznetSocketBackend::readStream(
    uint8_t *buffer,
    size_t length,
    bool advance)
{
    if (
        !connected() ||
        buffer == nullptr ||
        length == 0 ||
        _recvCommandPending
    )
    {
        return 0;
    }

    bool ok = false;
    const uint16_t received = read16(FLPROG_WIZNET_SN_RX_RSR, ok);

    if (!ok || received == 0)
    {
        return 0;
    }

    uint16_t amount = (uint16_t)length;
    if (amount > received)
    {
        amount = received;
    }

    const uint16_t rd = read16(FLPROG_WIZNET_SN_RX_RD, ok);
    if (!ok)
    {
        return 0;
    }

    _hardware->readRxBuffer(
        (uint8_t)_socket,
        rd,
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

    if (!write16(FLPROG_WIZNET_SN_RX_RD, (uint16_t)(rd + amount)))
    {
        return 0;
    }

    _lastKeepAliveAt = millis();
    _recvCommandPending = true;

    if (issueCommand(FLPROG_WIZNET_CMD_RECV))
    {
        _recvCommandPending = false;
    }

    // RX_RD is already committed, therefore the bytes are logically consumed
    // even if RECV itself must be deferred to a later pool() cycle.
    return amount;
}

int FLProgWiznetSocketBackend::read()
{
    uint8_t value = 0;
    const int count = readStream(&value, 1, true);
    return count == 1 ? value : -1;
}

int FLProgWiznetSocketBackend::read(
    uint8_t *buffer,
    size_t length)
{
    return readStream(buffer, length, true);
}

int FLProgWiznetSocketBackend::peek()
{
    uint8_t value = 0;
    const int count = readStream(&value, 1, false);
    return count == 1 ? value : -1;
}

size_t FLProgWiznetSocketBackend::write(
    const uint8_t *buffer,
    size_t length)
{
    if (
        !connected() ||
        buffer == nullptr ||
        length == 0 ||
        _sendPrepared ||
        _sendPending
    )
    {
        return 0;
    }

    bool ok = false;
    const uint16_t freeSize = read16(FLPROG_WIZNET_SN_TX_FSR, ok);

    if (!ok || freeSize == 0)
    {
        return 0;
    }

    uint16_t amount = length > 65535U
        ? 65535U
        : (uint16_t)length;
    if (amount > freeSize)
    {
        amount = freeSize;
    }

    const uint16_t wr = read16(FLPROG_WIZNET_SN_TX_WR, ok);
    if (!ok)
    {
        return 0;
    }

    _hardware->writeTxBuffer(
        (uint8_t)_socket,
        wr,
        buffer,
        amount
    );

    if (spiWasDeferred())
    {
        return 0;
    }

    if (!write16(FLPROG_WIZNET_SN_TX_WR, (uint16_t)(wr + amount)))
    {
        return 0;
    }

    _sendPrepared = true;

    if (issueCommand(FLPROG_WIZNET_CMD_SEND))
    {
        _sendPrepared = false;
        _sendPending = true;
    }

    _lastKeepAliveAt = millis();

    // TX_WR is already committed. The backend owns these bytes now and pool()
    // will issue SEND later if the command could not be written immediately.
    return amount;
}

#endif // !FLPROG_NETWORK_PROFILE_WIFI_ONLY

// -----------------------------------------------------------------------------
// FLProgLwipSocketBackend
// -----------------------------------------------------------------------------

uint8_t FLProgLwipSocketBackend::open(
    const FLProgSocketLease &lease,
    uint8_t protocol,
    const FLProgSocketEndpoint &endpoint,
    uint32_t timeout)
{
    _endpoint = endpoint;

    if (
        _transportId != FLPROG_TRANSPORT_WIFI &&
        _transportId != FLPROG_TRANSPORT_WIFI_AP &&
        _transportId != FLPROG_TRANSPORT_RMII
    )
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    if (_callbacks.open == nullptr)
    {
        return FLPROG_SOCKET_RESULT_UNSUPPORTED;
    }

    return _callbacks.open(
        _context,
        _transportId,
        lease,
        protocol,
        endpoint,
        timeout
    );
}

uint8_t FLProgLwipSocketBackend::pool()
{
    return (_callbacks.pool != nullptr)
        ? _callbacks.pool(_context)
        : static_cast<uint8_t>(FLPROG_SOCKET_RESULT_SUCCESS);
}

uint8_t FLProgLwipSocketBackend::close()
{
    return (_callbacks.close != nullptr)
        ? _callbacks.close(_context)
        : static_cast<uint8_t>(FLPROG_SOCKET_RESULT_SUCCESS);
}

bool FLProgLwipSocketBackend::isOpen() const
{
    return
        _callbacks.isOpen != nullptr &&
        _callbacks.isOpen(_context);
}

bool FLProgLwipSocketBackend::connected() const
{
    return
        _callbacks.connected != nullptr &&
        _callbacks.connected(_context);
}

int FLProgLwipSocketBackend::available()
{
    return (_callbacks.available != nullptr)
        ? _callbacks.available(_context)
        : 0;
}

int FLProgLwipSocketBackend::read()
{
    return (_callbacks.readOne != nullptr)
        ? _callbacks.readOne(_context)
        : -1;
}

int FLProgLwipSocketBackend::read(
    uint8_t *buffer,
    size_t length)
{
    return (_callbacks.read != nullptr)
        ? _callbacks.read(_context, buffer, length)
        : 0;
}

int FLProgLwipSocketBackend::peek()
{
    return (_callbacks.peek != nullptr)
        ? _callbacks.peek(_context)
        : -1;
}

size_t FLProgLwipSocketBackend::write(
    const uint8_t *buffer,
    size_t length)
{
    return (_callbacks.write != nullptr)
        ? _callbacks.write(_context, buffer, length)
        : 0;
}

void FLProgLwipSocketBackend::flush()
{
    if (_callbacks.flush != nullptr)
    {
        _callbacks.flush(_context);
    }
}

uint16_t FLProgLwipSocketBackend::localPort() const
{
    return (_callbacks.localPort != nullptr)
        ? _callbacks.localPort(_context)
        : _endpoint.localPort;
}

IPAddress FLProgLwipSocketBackend::remoteIP() const
{
    return (_callbacks.remoteIP != nullptr)
        ? _callbacks.remoteIP(_context)
        : _endpoint.remoteIP;
}

uint16_t FLProgLwipSocketBackend::remotePort() const
{
    return (_callbacks.remotePort != nullptr)
        ? _callbacks.remotePort(_context)
        : _endpoint.remotePort;
}
