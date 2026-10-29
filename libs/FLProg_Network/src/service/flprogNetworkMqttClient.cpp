/*
 * FLProg_Network — flprogNetworkMqttClient.cpp
 * Назначение: MQTT client core поверх native TCP.
 *
 * Архитектурные заметки:
 * - Разделяет TCP lifecycle и MQTT session lifecycle.
 * - Reconnect MQTT не должен искусственно сбрасывать transport generation.
 * - Keepalive, packet parsing и retry выполняются без блокирующих задержек.
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
#include "flprogNetworkMqttClient.h"
#include "../flprogNetwork.h"

#include <string.h>
#include <stdlib.h>

#define FLPROG_MQTT_BODY_OFFSET 5U

FLProgNetworkMqttClient::~FLProgNetworkMqttClient()
{
    end();
    free(_subscriptions);
}

bool FLProgNetworkMqttClient::ipIsZero(IPAddress value)
{
    return value[0] == 0 && value[1] == 0 && value[2] == 0 && value[3] == 0;
}

bool FLProgNetworkMqttClient::parseIpv4Literal(const char *value, IPAddress &result)
{
    if (value == nullptr || *value == 0) return false;
    uint8_t octets[4] = {0, 0, 0, 0};
    uint8_t index = 0;
    uint16_t current = 0;
    bool digit = false;
    for (const char *p = value;; ++p)
    {
        const char c = *p;
        if (c >= '0' && c <= '9')
        {
            digit = true;
            current = (uint16_t)(current * 10U + (uint16_t)(c - '0'));
            if (current > 255U) return false;
            continue;
        }
        if (c == '.' || c == 0)
        {
            if (!digit || index >= 4) return false;
            octets[index++] = (uint8_t)current;
            current = 0;
            digit = false;
            if (c == 0) break;
            continue;
        }
        return false;
    }
    if (index != 4) return false;
    result = IPAddress(octets[0], octets[1], octets[2], octets[3]);
    return true;
}

bool FLProgNetworkMqttClient::validPublishTopic(const char *value)
{
    if (value == nullptr || value[0] == 0) return false;
    const size_t length = strlen(value);
    if (length > FLPROG_MQTT_MAX_TOPIC_LENGTH) return false;
    for (size_t i = 0; i < length; ++i)
    {
        if (value[i] == '+' || value[i] == '#') return false;
    }
    return true;
}

bool FLProgNetworkMqttClient::validTopicFilter(const char *value)
{
    if (value == nullptr || value[0] == 0) return false;
    const size_t length = strlen(value);
    if (length > FLPROG_MQTT_MAX_TOPIC_LENGTH) return false;
    for (size_t i = 0; i < length; ++i)
    {
        if (value[i] == '#')
        {
            if (i + 1 != length) return false;
            if (i != 0 && value[i - 1] != '/') return false;
        }
        else if (value[i] == '+')
        {
            if (i != 0 && value[i - 1] != '/') return false;
            if (i + 1 != length && value[i + 1] != '/') return false;
        }
    }
    return true;
}

size_t FLProgNetworkMqttClient::encodeRemainingLength(uint32_t value, uint8_t *out)
{
    size_t count = 0;
    do
    {
        uint8_t encoded = (uint8_t)(value % 128UL);
        value /= 128UL;
        if (value != 0) encoded |= 0x80U;
        if (out != nullptr) out[count] = encoded;
        count++;
    } while (value != 0 && count < 4);
    return value == 0 ? count : 0;
}

bool FLProgNetworkMqttClient::begin(FLProgNetworkInterface &network, uint8_t routeValue)
{
    end();
    _route = routeValue;
    if (!attach(network))
    {
        _state = FLPROG_MQTT_STATE_ERROR;
        _error = FLPROG_MQTT_ERROR_SERVICE_LIMIT;
        return false;
    }
    if (!_tcp.begin(network, routeValue))
    {
        detach();
        _state = FLPROG_MQTT_STATE_ERROR;
        _error = FLPROG_MQTT_ERROR_SOCKET_REGISTRY;
        return false;
    }
    _tcpAttached = true;
    _tcp.setAutoReconnect(false);
    _tcp.setReconnectOnInitialFailure(false);
    _tcp.setDnsPolicy(FLPROG_TCP_DNS_POLICY_PRIVATE);
    _tcp.setNoDelay(true);
    _tcp.setTimeout(_connectTimeoutMs);
    _state = FLPROG_MQTT_STATE_IDLE;
    _error = FLPROG_MQTT_ERROR_NONE;
    resetRxParser();
    return true;
}

void FLProgNetworkMqttClient::end()
{
    _wantConnected = false;
    _disconnectRequested = false;
    _cancelRequested = false;
    _sessionConnected = false;
    if (_tcpAttached)
    {
        _tcp.end();
        _tcpAttached = false;
    }
    detach();
    _pollStarted = false;
    _pollRequested = false;
    _rxBacklog = false;
    _state = FLPROG_MQTT_STATE_IDLE;
    _error = FLPROG_MQTT_ERROR_NONE;
    resetTransaction();
    resetRxParser();
}

void FLProgNetworkMqttClient::setRoute(uint8_t value)
{
    if (_route == value) return;
    _route = value;
    _tcp.setRoute(value);
    if (_wantConnected)
    {
        scheduleReconnect(FLPROG_MQTT_ERROR_TCP);
    }
}

void FLProgNetworkMqttClient::setBroker(const char *value)
{
    if (value == nullptr || value[0] == 0)
    {
        clearBroker();
        return;
    }
    IPAddress parsed;
    if (parseIpv4Literal(value, parsed))
    {
        setBroker(parsed);
        return;
    }
    const size_t length = strlen(value);
    if (length > FLPROG_TCP_HOSTNAME_MAX_LENGTH)
    {
        clearBroker();
        return;
    }
    memcpy(_brokerName, value, length + 1);
    _brokerIp = IPAddress(0, 0, 0, 0);
    _brokerMode = FLPROG_MQTT_HOST_NAME;
}

void FLProgNetworkMqttClient::setBroker(IPAddress value)
{
    if (ipIsZero(value))
    {
        clearBroker();
        return;
    }
    _brokerIp = value;
    _brokerName[0] = 0;
    _brokerMode = FLPROG_MQTT_HOST_IP;
}

void FLProgNetworkMqttClient::clearBroker()
{
    _brokerMode = FLPROG_MQTT_HOST_NONE;
    _brokerName[0] = 0;
    _brokerIp = IPAddress(0, 0, 0, 0);
}

bool FLProgNetworkMqttClient::setClientId(const char *value)
{
    if (value == nullptr) value = "";
    const size_t length = strlen(value);
    if (length == 0 || length > FLPROG_MQTT_MAX_CLIENT_ID_LENGTH)
    {
        _error = FLPROG_MQTT_ERROR_NO_CLIENT_ID;
        return false;
    }
    memcpy(_clientId, value, length + 1);
    return true;
}

bool FLProgNetworkMqttClient::setCredentials(const char *usernameValue, const char *passwordValue)
{
    if (usernameValue == nullptr || usernameValue[0] == 0)
    {
        clearCredentials();
        return true;
    }
    if (passwordValue == nullptr) passwordValue = "";
    const size_t userLength = strlen(usernameValue);
    const size_t passLength = strlen(passwordValue);
    if (userLength > FLPROG_MQTT_MAX_USERNAME_LENGTH ||
        passLength > FLPROG_MQTT_MAX_PASSWORD_LENGTH)
    {
        _error = FLPROG_MQTT_ERROR_CREDENTIALS;
        return false;
    }
    memcpy(_username, usernameValue, userLength + 1);
    memcpy(_password, passwordValue, passLength + 1);
    return true;
}

void FLProgNetworkMqttClient::clearCredentials()
{
    _username[0] = 0;
    _password[0] = 0;
}

void FLProgNetworkMqttClient::setReconnectBackoff(uint32_t initialMs, uint32_t maxMs)
{
    if (maxMs < initialMs) maxMs = initialMs;
    _reconnectInitialMs = initialMs;
    _reconnectMaxMs = maxMs;
    resetReconnectBackoff();
}

uint32_t FLProgNetworkMqttClient::reconnectIn() const
{
    if (_state != FLPROG_MQTT_STATE_BACKOFF) return 0;
    const uint32_t now = millis();
    return timeReached(now, _reconnectAt) ? 0 : _reconnectAt - now;
}

bool FLProgNetworkMqttClient::connect()
{
    if (!_tcpAttached || !isAttached())
    {
        _error = FLPROG_MQTT_ERROR_SERVICE_LIMIT;
        return false;
    }
    if (_brokerMode == FLPROG_MQTT_HOST_NONE || _port == 0)
    {
        _error = FLPROG_MQTT_ERROR_NO_BROKER;
        return false;
    }
    if (_clientId[0] == 0)
    {
        _error = FLPROG_MQTT_ERROR_NO_CLIENT_ID;
        return false;
    }
    if (!_wantConnected || _state == FLPROG_MQTT_STATE_IDLE ||
        _state == FLPROG_MQTT_STATE_ERROR || _state == FLPROG_MQTT_STATE_BACKOFF)
        _pollRequested = true;
    _wantConnected = true;
    _disconnectRequested = false;
    _cancelRequested = false;
    _error = FLPROG_MQTT_ERROR_NONE;
    if (_state == FLPROG_MQTT_STATE_ERROR || _state == FLPROG_MQTT_STATE_BACKOFF)
    {
        _state = FLPROG_MQTT_STATE_IDLE;
    }
    return true;
}

void FLProgNetworkMqttClient::disconnect()
{
    _wantConnected = false;
    _disconnectRequested = true;
    _pollRequested = true;
}

void FLProgNetworkMqttClient::cancel()
{
    _wantConnected = false;
    _cancelRequested = true;
    _pollRequested = true;
}

bool FLProgNetworkMqttClient::writeUtf(size_t &offset, const char *value, size_t maxLength)
{
    if (value == nullptr) value = "";
    const size_t length = strlen(value);
    if (length > maxLength || length > 65535U) return false;
    if (offset + 2U + length > FLPROG_MQTT_MAX_PACKET_SIZE) return false;
    _txBuffer[offset++] = (uint8_t)(length >> 8);
    _txBuffer[offset++] = (uint8_t)(length & 0xFFU);
    if (length != 0)
    {
        memcpy(_txBuffer + offset, value, length);
        offset += length;
    }
    return true;
}

bool FLProgNetworkMqttClient::finishPacket(uint8_t fixedHeader, size_t bodyLength, uint8_t purpose)
{
    uint8_t encoded[4];
    const size_t encodedLength = encodeRemainingLength((uint32_t)bodyLength, encoded);
    if (encodedLength == 0) return false;
    const size_t headerLength = 1U + encodedLength;
    if (headerLength + bodyLength > FLPROG_MQTT_MAX_PACKET_SIZE)
    {
        _oversizedPacketCount++;
        _error = FLPROG_MQTT_ERROR_PACKET_TOO_LARGE;
        return false;
    }
    memmove(_txBuffer + headerLength, _txBuffer + FLPROG_MQTT_BODY_OFFSET, bodyLength);
    _txBuffer[0] = fixedHeader;
    memcpy(_txBuffer + 1, encoded, encodedLength);
    _txLength = headerLength + bodyLength;
    _txOffset = 0;
    _txPurpose = purpose;
    _state = FLPROG_MQTT_STATE_SENDING;
    _lastProgressAt = millis();
    return true;
}

bool FLProgNetworkMqttClient::buildConnectPacket()
{
    size_t offset = FLPROG_MQTT_BODY_OFFSET;
    if (offset + 10U > FLPROG_MQTT_MAX_PACKET_SIZE) return false;
    _txBuffer[offset++] = 0;
    _txBuffer[offset++] = 4;
    _txBuffer[offset++] = 'M';
    _txBuffer[offset++] = 'Q';
    _txBuffer[offset++] = 'T';
    _txBuffer[offset++] = 'T';
    _txBuffer[offset++] = 4; // MQTT 3.1.1
    uint8_t flags = 0x02U; // Clean Session baseline.
    if (_username[0] != 0)
    {
        flags |= 0x80U;
        if (_password[0] != 0) flags |= 0x40U;
    }
    _txBuffer[offset++] = flags;
    _txBuffer[offset++] = (uint8_t)(_keepAliveSeconds >> 8);
    _txBuffer[offset++] = (uint8_t)(_keepAliveSeconds & 0xFFU);
    if (!writeUtf(offset, _clientId, FLPROG_MQTT_MAX_CLIENT_ID_LENGTH)) return false;
    if (_username[0] != 0)
    {
        if (!writeUtf(offset, _username, FLPROG_MQTT_MAX_USERNAME_LENGTH)) return false;
        if (_password[0] != 0 &&
            !writeUtf(offset, _password, FLPROG_MQTT_MAX_PASSWORD_LENGTH)) return false;
    }
    return finishPacket(0x10U, offset - FLPROG_MQTT_BODY_OFFSET, FLPROG_MQTT_TX_CONNECT);
}

bool FLProgNetworkMqttClient::buildPublishPacket(
    const char *topic,
    const uint8_t *payload,
    size_t length,
    bool retained)
{
    if (!validPublishTopic(topic))
    {
        _error = FLPROG_MQTT_ERROR_BAD_TOPIC;
        return false;
    }
    if (payload == nullptr && length != 0)
    {
        _error = FLPROG_MQTT_ERROR_PROTOCOL;
        return false;
    }
    size_t offset = FLPROG_MQTT_BODY_OFFSET;
    if (!writeUtf(offset, topic, FLPROG_MQTT_MAX_TOPIC_LENGTH)) return false;
    if (offset + length > FLPROG_MQTT_MAX_PACKET_SIZE)
    {
        _oversizedPacketCount++;
        _error = FLPROG_MQTT_ERROR_PACKET_TOO_LARGE;
        return false;
    }
    if (length != 0)
    {
        memcpy(_txBuffer + offset, payload, length);
        offset += length;
    }
    return finishPacket(
        retained ? 0x31U : 0x30U,
        offset - FLPROG_MQTT_BODY_OFFSET,
        FLPROG_MQTT_TX_PUBLISH);
}

bool FLProgNetworkMqttClient::buildSubscribePacket(const char *topicFilter, uint8_t purpose)
{
    if (!validTopicFilter(topicFilter))
    {
        _error = FLPROG_MQTT_ERROR_BAD_TOPIC;
        return false;
    }
    uint16_t packetId = _nextPacketId++;
    if (packetId == 0) packetId = _nextPacketId++;
    size_t offset = FLPROG_MQTT_BODY_OFFSET;
    _txBuffer[offset++] = (uint8_t)(packetId >> 8);
    _txBuffer[offset++] = (uint8_t)(packetId & 0xFFU);
    if (!writeUtf(offset, topicFilter, FLPROG_MQTT_MAX_TOPIC_LENGTH)) return false;
    if (offset >= FLPROG_MQTT_MAX_PACKET_SIZE) return false;
    _txBuffer[offset++] = 0; // requested QoS 0
    _waitingPacketId = packetId;
    const size_t topicLength = strlen(topicFilter);
    memcpy(_pendingSubscribe, topicFilter, topicLength + 1);
    _pendingSubscribeIsResubscribe = purpose == FLPROG_MQTT_TX_RESUBSCRIBE;
    return finishPacket(0x82U, offset - FLPROG_MQTT_BODY_OFFSET, purpose);
}

bool FLProgNetworkMqttClient::buildPingPacket()
{
    return finishPacket(0xC0U, 0, FLPROG_MQTT_TX_PING);
}

bool FLProgNetworkMqttClient::buildDisconnectPacket()
{
    return finishPacket(0xE0U, 0, FLPROG_MQTT_TX_DISCONNECT);
}

bool FLProgNetworkMqttClient::publish(
    const char *topic,
    const uint8_t *payload,
    size_t length,
    bool retained)
{
    if (!_sessionConnected)
    {
        _error = FLPROG_MQTT_ERROR_NOT_CONNECTED;
        return false;
    }
    if (_state != FLPROG_MQTT_STATE_CONNECTED || _resubscribeActive)
    {
        _error = FLPROG_MQTT_ERROR_BUSY;
        return false;
    }
    _error = FLPROG_MQTT_ERROR_NONE;
    const bool accepted = buildPublishPacket(topic, payload, length, retained);
    if (accepted) _pollRequested = true;
    return accepted;
}

bool FLProgNetworkMqttClient::subscribe(const char *topicFilter)
{
    if (!_sessionConnected)
    {
        _error = FLPROG_MQTT_ERROR_NOT_CONNECTED;
        return false;
    }
    if (_state != FLPROG_MQTT_STATE_CONNECTED || _resubscribeActive)
    {
        _error = FLPROG_MQTT_ERROR_BUSY;
        return false;
    }
    if (!validTopicFilter(topicFilter))
    {
        _error = FLPROG_MQTT_ERROR_BAD_TOPIC;
        return false;
    }
    if (!hasSubscription(topicFilter))
    {
        if (_subscriptionLimit != 0 && _subscriptionCount >= _subscriptionLimit)
        {
            _error = FLPROG_MQTT_ERROR_SUBSCRIPTION_LIMIT;
            return false;
        }
        // Reserve before transmission: a successful SUBACK must always fit.
        if (!reserveSubscriptions(_subscriptionCount + 1)) return false;
    }
    _error = FLPROG_MQTT_ERROR_NONE;
    const bool accepted = buildSubscribePacket(topicFilter, FLPROG_MQTT_TX_SUBSCRIBE);
    if (accepted) _pollRequested = true;
    return accepted;
}

void FLProgNetworkMqttClient::resetRxParser()
{
    _rxBacklog = false;
    _rxHeader = 0;
    _rxStage = 0;
    _rxLengthBytes = 0;
    _rxRemainingLength = 0;
    _rxMultiplier = 1;
    _rxBodyOffset = 0;
}

void FLProgNetworkMqttClient::resetTransaction()
{
    _txLength = 0;
    _txOffset = 0;
    _txPurpose = FLPROG_MQTT_TX_NONE;
    _waitingPacketId = 0;
    _pendingSubscribe[0] = 0;
    _pendingSubscribeIsResubscribe = false;
    _connectIssued = false;
}

void FLProgNetworkMqttClient::resetReconnectBackoff()
{
    _reconnectCurrentMs = _reconnectInitialMs;
}

void FLProgNetworkMqttClient::advanceReconnectBackoff()
{
    if (_reconnectCurrentMs >= _reconnectMaxMs)
    {
        _reconnectCurrentMs = _reconnectMaxMs;
        return;
    }
    uint32_t next = _reconnectCurrentMs == 0 ? 1UL : _reconnectCurrentMs * 2UL;
    if (next < _reconnectCurrentMs || next > _reconnectMaxMs) next = _reconnectMaxMs;
    _reconnectCurrentMs = next;
}

void FLProgNetworkMqttClient::scheduleReconnect(uint8_t errorValue)
{
    const bool wasConnected = _sessionConnected;
    _sessionConnected = false;
    resetTransaction();
    resetRxParser();
    if (_tcpAttached) _tcp.stop();
    if (wasConnected)
    {
        _disconnectCount++;
        _events |= FLPROG_MQTT_EVENT_DISCONNECTED;
    }
    _error = errorValue;
    if (!_wantConnected || !_autoReconnect)
    {
        _state = FLPROG_MQTT_STATE_ERROR;
        _events |= FLPROG_MQTT_EVENT_FAILED;
        return;
    }
    const uint32_t delay = _reconnectCurrentMs;
    _reconnectAt = millis() + delay;
    _state = FLPROG_MQTT_STATE_BACKOFF;
    _reconnectScheduleCount++;
    _events |= FLPROG_MQTT_EVENT_FAILED | FLPROG_MQTT_EVENT_RECONNECT_SCHEDULED;
    advanceReconnectBackoff();
}

void FLProgNetworkMqttClient::fail(uint8_t errorValue, bool reconnectAllowed)
{
    if (errorValue == FLPROG_MQTT_ERROR_PROTOCOL || errorValue == FLPROG_MQTT_ERROR_UNSUPPORTED)
    {
        _protocolErrorCount++;
    }
    if (reconnectAllowed)
    {
        scheduleReconnect(errorValue);
    }
    else
    {
        _error = errorValue;
        _state = FLPROG_MQTT_STATE_ERROR;
        _sessionConnected = false;
        if (_tcpAttached) _tcp.stop();
        _events |= FLPROG_MQTT_EVENT_FAILED;
    }
}

void FLProgNetworkMqttClient::startTcpConnect()
{
    if (_brokerMode == FLPROG_MQTT_HOST_NONE || _port == 0 || _clientId[0] == 0)
    {
        fail(_brokerMode == FLPROG_MQTT_HOST_NONE
                 ? FLPROG_MQTT_ERROR_NO_BROKER
                 : FLPROG_MQTT_ERROR_NO_CLIENT_ID,
             false);
        return;
    }
    _tcp.stop();
    _tcp.clearClientError();
    _tcp.setRoute(_route);
    _tcp.setTimeout(_connectTimeoutMs);
    _connectIssued = false;
    _sessionConnected = false;
    _state = FLPROG_MQTT_STATE_TCP_CONNECTING;
    _stateStartedAt = millis();
    _lastProgressAt = _stateStartedAt;
    _connectAttemptCount++;
}

void FLProgNetworkMqttClient::serviceTcpConnecting()
{
    if (!_connectIssued)
    {
        if (_brokerMode == FLPROG_MQTT_HOST_IP)
        {
            (void)_tcp.connect(_brokerIp, _port);
        }
        else
        {
            (void)_tcp.connect(_brokerName, _port);
        }
        _connectIssued = true;
        _lastProgressAt = millis();
    }

    if (_tcp.connected())
    {
        _connectIssued = false;
        if (!buildConnectPacket())
        {
            fail(FLPROG_MQTT_ERROR_PACKET_TOO_LARGE);
        }
        return;
    }

    if (_tcp.clientError() != FLPROG_TCP_CLIENT_ERROR_NONE ||
        _tcp.state() == FLPROG_SOCKET_STATE_ERROR ||
        _tcp.state() == FLPROG_SOCKET_STATE_UNSUPPORTED)
    {
        fail(FLPROG_MQTT_ERROR_TCP);
        return;
    }

    if (_connectTimeoutMs != 0 &&
        (uint32_t)(millis() - _stateStartedAt) >= _connectTimeoutMs)
    {
        fail(FLPROG_MQTT_ERROR_CONNECT_TIMEOUT);
    }
}

void FLProgNetworkMqttClient::serviceSend()
{
    if (!_tcp.connected())
    {
        fail(FLPROG_MQTT_ERROR_TCP);
        return;
    }
    if (_txOffset < _txLength && FLPROG_MQTT_WRITE_BUDGET != 0)
    {
        size_t count = _txLength - _txOffset;
        if (count > FLPROG_MQTT_WRITE_BUDGET) count = FLPROG_MQTT_WRITE_BUDGET;
        if (count > FLPROG_MQTT_IO_CHUNK_SIZE) count = FLPROG_MQTT_IO_CHUNK_SIZE;
        const size_t written = _tcp.write(_txBuffer + _txOffset, count);
        if (written != 0)
        {
            _txOffset += written;
            _lastProgressAt = millis();
        }
    }
    if (_txOffset < _txLength)
    {
        if (_responseTimeoutMs != 0 &&
            (uint32_t)(millis() - _lastProgressAt) >= _responseTimeoutMs)
        {
            fail(FLPROG_MQTT_ERROR_TIMEOUT);
        }
        return;
    }

    const uint8_t purpose = _txPurpose;
    _lastTxAt = millis();
    _txLength = 0;
    _txOffset = 0;
    _txPurpose = FLPROG_MQTT_TX_NONE;
    _stateStartedAt = millis();
    _lastProgressAt = _stateStartedAt;

    if (purpose == FLPROG_MQTT_TX_CONNECT)
    {
        _state = FLPROG_MQTT_STATE_WAIT_CONNACK;
    }
    else if (purpose == FLPROG_MQTT_TX_PUBLISH)
    {
        _publishCount++;
        _events |= FLPROG_MQTT_EVENT_PUBLISHED;
        _state = FLPROG_MQTT_STATE_CONNECTED;
    }
    else if (purpose == FLPROG_MQTT_TX_SUBSCRIBE || purpose == FLPROG_MQTT_TX_RESUBSCRIBE)
    {
        _state = FLPROG_MQTT_STATE_WAIT_SUBACK;
    }
    else if (purpose == FLPROG_MQTT_TX_PING)
    {
        _pingRequestCount++;
        _state = FLPROG_MQTT_STATE_WAIT_PINGRESP;
    }
    else if (purpose == FLPROG_MQTT_TX_DISCONNECT)
    {
        _tcp.stop();
        if (_sessionConnected)
        {
            _sessionConnected = false;
            _disconnectCount++;
            _events |= FLPROG_MQTT_EVENT_DISCONNECTED;
        }
        _state = FLPROG_MQTT_STATE_IDLE;
        _disconnectRequested = false;
    }
    else
    {
        fail(FLPROG_MQTT_ERROR_PROTOCOL);
    }
}

bool FLProgNetworkMqttClient::feedRxByte(uint8_t value)
{
    if (_rxStage == 0)
    {
        _rxHeader = value;
        _rxStage = 1;
        _rxLengthBytes = 0;
        _rxRemainingLength = 0;
        _rxMultiplier = 1;
        _rxBodyOffset = 0;
        return true;
    }
    if (_rxStage == 1)
    {
        _rxLengthBytes++;
        if (_rxLengthBytes > 4) return false;
        _rxRemainingLength += (uint32_t)(value & 0x7FU) * _rxMultiplier;
        if (_rxMultiplier > 128UL * 128UL * 128UL) return false;
        if ((value & 0x80U) != 0)
        {
            _rxMultiplier *= 128UL;
            return true;
        }
        if (_rxRemainingLength > FLPROG_MQTT_MAX_PACKET_SIZE)
        {
            _oversizedPacketCount++;
            _error = FLPROG_MQTT_ERROR_PACKET_TOO_LARGE;
            return false;
        }
        if (_rxRemainingLength == 0)
        {
            const bool ok = handlePacket(_rxHeader, _rxBuffer, 0);
            resetRxParser();
            return ok;
        }
        _rxStage = 2;
        return true;
    }
    if (_rxBodyOffset >= FLPROG_MQTT_MAX_PACKET_SIZE) return false;
    _rxBuffer[_rxBodyOffset++] = value;
    if (_rxBodyOffset == _rxRemainingLength)
    {
        const bool ok = handlePacket(_rxHeader, _rxBuffer, _rxBodyOffset);
        resetRxParser();
        return ok;
    }
    return true;
}

bool FLProgNetworkMqttClient::handleConnack(const uint8_t *body, size_t length)
{
    if (_state != FLPROG_MQTT_STATE_WAIT_CONNACK || length != 2) return false;
    if ((body[0] & 0xFEU) != 0 || (body[0] & 0x01U) != 0) return false;
    _connackCode = body[1];
    if (_connackCode != 0)
    {
        // MQTT 3.1.1 code 3 (Server unavailable) is transient. The other
        // refusal codes normally require configuration/user action and are
        // not retried automatically to avoid an endless broker hammer loop.
        fail(FLPROG_MQTT_ERROR_CONNACK_REFUSED, _connackCode == 3);
        return true;
    }
    _sessionConnected = true;
    _state = FLPROG_MQTT_STATE_CONNECTED;
    _error = FLPROG_MQTT_ERROR_NONE;
    _connectSuccessCount++;
    _events |= FLPROG_MQTT_EVENT_CONNECTED;
    _lastRxAt = millis();
    _lastTxAt = millis();
    resetReconnectBackoff();
    _resubscribeIndex = 0;
    _resubscribeActive = _subscriptionCount != 0;
    return true;
}

bool FLProgNetworkMqttClient::hasSubscription(const char *topicFilter) const
{
    for (size_t i = 0; i < _subscriptionCount; ++i)
        if (strcmp(_subscriptions[i], topicFilter) == 0) return true;
    return false;
}

bool FLProgNetworkMqttClient::reserveSubscriptions(size_t count)
{
    if (count <= _subscriptionCapacity) return true;
    if (count > ((size_t)-1) / sizeof(*_subscriptions))
    {
        _error = FLPROG_MQTT_ERROR_NO_MEMORY;
        return false;
    }
    void *storage = realloc(_subscriptions, count * sizeof(*_subscriptions));
    if (storage == nullptr)
    {
        _error = FLPROG_MQTT_ERROR_NO_MEMORY;
        return false;
    }
    _subscriptions = static_cast<char (*)[FLPROG_MQTT_MAX_TOPIC_LENGTH + 1]>(storage);
    _subscriptionCapacity = count;
    return true;
}

bool FLProgNetworkMqttClient::setSubscriptionLimit(size_t value)
{
    const size_t pending = _pendingSubscribe[0] != 0 &&
        !_pendingSubscribeIsResubscribe && !hasSubscription(_pendingSubscribe) ? 1 : 0;
    if (value != 0 && value < _subscriptionCount + pending)
    {
        _error = FLPROG_MQTT_ERROR_SUBSCRIPTION_LIMIT;
        return false;
    }
    _subscriptionLimit = value;
    _error = FLPROG_MQTT_ERROR_NONE;
    return true;
}

bool FLProgNetworkMqttClient::clearSubscriptions()
{
    if (_wantConnected || _sessionConnected || _state != FLPROG_MQTT_STATE_IDLE)
    {
        _error = FLPROG_MQTT_ERROR_BUSY;
        return false;
    }
    free(_subscriptions);
    _subscriptions = nullptr;
    _subscriptionCount = _subscriptionCapacity = _resubscribeIndex = 0;
    _resubscribeActive = false;
    _error = FLPROG_MQTT_ERROR_NONE;
    return true;
}

void FLProgNetworkMqttClient::addOrKeepSubscription(const char *topicFilter)
{
    if (hasSubscription(topicFilter)) return;
    // subscribe() has already reserved this slot before sending the packet.
    if (_subscriptionCount >= _subscriptionCapacity) return;
    strcpy(_subscriptions[_subscriptionCount++], topicFilter);
}

void FLProgNetworkMqttClient::completeSubscribe()
{
    if (!_pendingSubscribeIsResubscribe)
    {
        addOrKeepSubscription(_pendingSubscribe);
        _subscribeCount++;
        _events |= FLPROG_MQTT_EVENT_SUBSCRIBED;
    }
    else
    {
        if (_resubscribeIndex < _subscriptionCount) _resubscribeIndex++;
        if (_resubscribeIndex >= _subscriptionCount) _resubscribeActive = false;
    }
    _pendingSubscribe[0] = 0;
    _pendingSubscribeIsResubscribe = false;
    _waitingPacketId = 0;
    _state = FLPROG_MQTT_STATE_CONNECTED;
}

bool FLProgNetworkMqttClient::handleSuback(const uint8_t *body, size_t length)
{
    if (_state != FLPROG_MQTT_STATE_WAIT_SUBACK || length != 3) return false;
    const uint16_t packetId = (uint16_t)(((uint16_t)body[0] << 8) | body[1]);
    if (packetId != _waitingPacketId) return false;
    if (body[2] != 0x00U)
    {
        // One filter is sent per SUBSCRIBE packet in dev.4, and only QoS 0 is
        // requested. 0x80 means the broker rejected the filter. Keep the MQTT
        // connection alive and report the application-level failure explicitly.
        _error = FLPROG_MQTT_ERROR_SUBACK;
        _subscribeFailureCount++;
        _events |= FLPROG_MQTT_EVENT_FAILED;
        if (_pendingSubscribeIsResubscribe)
        {
            if (_resubscribeIndex < _subscriptionCount) _resubscribeIndex++;
            if (_resubscribeIndex >= _subscriptionCount) _resubscribeActive = false;
        }
        _pendingSubscribe[0] = 0;
        _pendingSubscribeIsResubscribe = false;
        _waitingPacketId = 0;
        _state = FLPROG_MQTT_STATE_CONNECTED;
        return true;
    }
    completeSubscribe();
    return true;
}

bool FLProgNetworkMqttClient::handlePublish(uint8_t header, const uint8_t *body, size_t length)
{
    const uint8_t qos = (uint8_t)((header >> 1) & 0x03U);
    if (qos != 0)
    {
        fail(FLPROG_MQTT_ERROR_UNSUPPORTED);
        return true;
    }
    if (length < 2) return false;
    const size_t topicLength = (size_t)(((uint16_t)body[0] << 8) | body[1]);
    if (topicLength == 0 || topicLength > FLPROG_MQTT_MAX_TOPIC_LENGTH || 2U + topicLength > length)
    {
        return false;
    }
    memcpy(_lastMessageTopic, body + 2, topicLength);
    _lastMessageTopic[topicLength] = 0;
    const uint8_t *payload = body + 2U + topicLength;
    const size_t payloadLength = length - 2U - topicLength;
    _lastMessageLength = payloadLength;
    _lastMessageRetained = (header & 0x01U) != 0;
    _messageCount++;
    _events |= FLPROG_MQTT_EVENT_MESSAGE;
    if (_messageCallback != nullptr)
    {
        _messageCallback(
            _lastMessageTopic,
            payload,
            payloadLength,
            _lastMessageRetained);
    }
    return true;
}

bool FLProgNetworkMqttClient::handlePingresp(const uint8_t *body, size_t length)
{
    (void)body;
    if (length != 0) return false;
    if (_state == FLPROG_MQTT_STATE_WAIT_PINGRESP)
    {
        _state = FLPROG_MQTT_STATE_CONNECTED;
        _pingResponseCount++;
        _events |= FLPROG_MQTT_EVENT_PINGRESP;
    }
    return true;
}

bool FLProgNetworkMqttClient::handlePacket(uint8_t header, const uint8_t *body, size_t length)
{
    const uint8_t type = (uint8_t)(header >> 4);
    const uint8_t flags = (uint8_t)(header & 0x0FU);
    _lastRxAt = millis();
    _lastProgressAt = _lastRxAt;
    if (type == 2)
    {
        if (flags != 0) return false;
        return handleConnack(body, length);
    }
    if (type == 3) return handlePublish(header, body, length);
    if (type == 9)
    {
        if (flags != 0) return false;
        return handleSuback(body, length);
    }
    if (type == 13)
    {
        if (flags != 0) return false;
        return handlePingresp(body, length);
    }
    _unknownPacketCount++;
    return true;
}

void FLProgNetworkMqttClient::serviceReceive()
{
    _rxBacklog = false;
    if (!_tcp.connected() || FLPROG_MQTT_READ_BUDGET == 0) return;
    const int available = _tcp.available();
    if (available <= 0) return;
    uint8_t bytes[FLPROG_MQTT_IO_CHUNK_SIZE];
    size_t count = (size_t)available;
    if (count > sizeof(bytes)) count = sizeof(bytes);
    if (count > FLPROG_MQTT_READ_BUDGET) count = FLPROG_MQTT_READ_BUDGET;
    const int received = _tcp.read(bytes, count);
    if (received <= 0)
    {
        // W5500 may still be completing the previous RECV command.
        _rxBacklog = true;
        return;
    }
    for (int i = 0; i < received; ++i)
    {
        if (!feedRxByte(bytes[i]))
        {
            fail(_error == FLPROG_MQTT_ERROR_PACKET_TOO_LARGE
                     ? FLPROG_MQTT_ERROR_PACKET_TOO_LARGE : FLPROG_MQTT_ERROR_PROTOCOL);
            return;
        }
        if (!isAttached() || _cancelRequested ||
            _state == FLPROG_MQTT_STATE_IDLE ||
            _state == FLPROG_MQTT_STATE_BACKOFF || _state == FLPROG_MQTT_STATE_ERROR) return;
    }
    // resetRxParser() may have run at a packet boundary inside this chunk.
    _rxBacklog = available > received;
}

void FLProgNetworkMqttClient::serviceConnectedIdle()
{
    if (!_sessionConnected) return;
    if (_resubscribeActive)
    {
        if (_resubscribeIndex < _subscriptionCount)
        {
            (void)buildSubscribePacket(
                _subscriptions[_resubscribeIndex],
                FLPROG_MQTT_TX_RESUBSCRIBE);
        }
        else
        {
            _resubscribeActive = false;
        }
        return;
    }
    if (_keepAliveSeconds == 0) return;
    const uint32_t interval = (uint32_t)_keepAliveSeconds * 1000UL;
    if ((uint32_t)(millis() - _lastTxAt) >= interval)
    {
        (void)buildPingPacket();
    }
}

void FLProgNetworkMqttClient::processDisconnectRequest()
{
    if (!_disconnectRequested) return;
    if (!_sessionConnected || !_tcp.connected())
    {
        _tcp.stop();
        _sessionConnected = false;
        _disconnectRequested = false;
        _state = FLPROG_MQTT_STATE_IDLE;
        return;
    }
    if (_state == FLPROG_MQTT_STATE_CONNECTED && _txPurpose == FLPROG_MQTT_TX_NONE)
    {
        if (!buildDisconnectPacket())
        {
            _tcp.stop();
            _sessionConnected = false;
            _disconnectRequested = false;
            _state = FLPROG_MQTT_STATE_IDLE;
        }
    }
}

void FLProgNetworkMqttClient::processCancelRequest()
{
    if (!_cancelRequested) return;
    const bool wasConnected = _sessionConnected;
    _tcp.stop();
    _sessionConnected = false;
    _cancelRequested = false;
    _disconnectRequested = false;
    _state = FLPROG_MQTT_STATE_IDLE;
    resetTransaction();
    resetRxParser();
    if (wasConnected)
    {
        _disconnectCount++;
        _events |= FLPROG_MQTT_EVENT_DISCONNECTED;
    }
    _events |= FLPROG_MQTT_EVENT_CANCELED;
}

bool FLProgNetworkMqttClient::pollDue(uint32_t now) const
{
    if (!_pollStarted || _pollRequested || _pollIntervalMs == 0) return true;
    // Protocol deadlines must not be extended by a slow idle poll setting.
    if (_state == FLPROG_MQTT_STATE_BACKOFF && _wantConnected &&
        timeReached(now, _reconnectAt)) return true;
    if (_state == FLPROG_MQTT_STATE_CONNECTED && _keepAliveSeconds != 0 &&
        (uint32_t)(now - _lastTxAt) >= (uint32_t)_keepAliveSeconds * 1000UL) return true;
    const bool active = busy() || _rxStage != 0 || _rxBacklog ||
                        _resubscribeActive || _disconnectRequested;
    const uint16_t interval = active ? 1U : _pollIntervalMs;
    return (uint32_t)(now - _lastPollAt) >= interval;
}

void FLProgNetworkMqttClient::pool()
{
    if (!isAttached() || !_tcpAttached) return;
    const uint32_t now = millis();
    if (!pollDue(now)) return;
    _lastPollAt = now;
    _pollStarted = true;
    _pollRequested = false;

    if (_cancelRequested)
    {
        processCancelRequest();
        return;
    }

    _tcp.service();
    processDisconnectRequest();

    if (_sessionConnected && !_tcp.connected() &&
        _state != FLPROG_MQTT_STATE_IDLE &&
        _state != FLPROG_MQTT_STATE_BACKOFF)
    {
        scheduleReconnect(FLPROG_MQTT_ERROR_TCP);
        return;
    }

    if (_state == FLPROG_MQTT_STATE_IDLE)
    {
        if (_wantConnected) startTcpConnect();
        return;
    }

    if (_state == FLPROG_MQTT_STATE_BACKOFF)
    {
        if (!_wantConnected)
        {
            _state = FLPROG_MQTT_STATE_IDLE;
            return;
        }
        if (timeReached(millis(), _reconnectAt)) startTcpConnect();
        return;
    }

    if (_state == FLPROG_MQTT_STATE_TCP_CONNECTING)
    {
        serviceTcpConnecting();
        return;
    }

    if (_state == FLPROG_MQTT_STATE_SENDING)
    {
        serviceSend();
        return;
    }

    serviceReceive();
    if (_state == FLPROG_MQTT_STATE_BACKOFF || _state == FLPROG_MQTT_STATE_ERROR) return;

    if ((_state == FLPROG_MQTT_STATE_WAIT_CONNACK ||
         _state == FLPROG_MQTT_STATE_WAIT_SUBACK ||
         _state == FLPROG_MQTT_STATE_WAIT_PINGRESP) &&
        _responseTimeoutMs != 0 &&
        // serviceSend() starts the ACK window after sending the full packet.
        // Unrelated incoming PUBLISH packets must not extend that window.
        (uint32_t)(millis() - _stateStartedAt) >= _responseTimeoutMs)
    {
        fail(FLPROG_MQTT_ERROR_TIMEOUT);
        return;
    }

    if (_state == FLPROG_MQTT_STATE_CONNECTED)
    {
        serviceConnectedIdle();
    }
}

void FLProgNetworkMqttClient::resetStatistics()
{
    _connectAttemptCount = 0;
    _connectSuccessCount = 0;
    _disconnectCount = 0;
    _publishCount = 0;
    _subscribeCount = 0;
    _subscribeFailureCount = 0;
    _messageCount = 0;
    _pingRequestCount = 0;
    _pingResponseCount = 0;
    _reconnectScheduleCount = 0;
    _protocolErrorCount = 0;
    _oversizedPacketCount = 0;
    _unknownPacketCount = 0;
}
