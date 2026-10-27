/*
 * FLProg_Network — flprogNetworkMqttClient.h
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
#pragma once

#include <Arduino.h>
#include <IPAddress.h>
#include <stdint.h>
#include "flprogNetworkService.h"
#include "../protocol/flprogNetworkTcpClient.h"

class FLProgNetworkInterface;

#ifndef FLPROG_MQTT_DEFAULT_PORT
#define FLPROG_MQTT_DEFAULT_PORT 1883U
#endif
#ifndef FLPROG_MQTT_DEFAULT_KEEPALIVE_SECONDS
#define FLPROG_MQTT_DEFAULT_KEEPALIVE_SECONDS 60U
#endif
#ifndef FLPROG_MQTT_CONNECT_TIMEOUT_MS
#define FLPROG_MQTT_CONNECT_TIMEOUT_MS 10000UL
#endif
#ifndef FLPROG_MQTT_RESPONSE_TIMEOUT_MS
#define FLPROG_MQTT_RESPONSE_TIMEOUT_MS 5000UL
#endif
#ifndef FLPROG_MQTT_RECONNECT_INITIAL_MS
#define FLPROG_MQTT_RECONNECT_INITIAL_MS 1000UL
#endif
#ifndef FLPROG_MQTT_RECONNECT_MAX_MS
#define FLPROG_MQTT_RECONNECT_MAX_MS 30000UL
#endif
#ifndef FLPROG_MQTT_MAX_PACKET_SIZE
#define FLPROG_MQTT_MAX_PACKET_SIZE 768U
#endif
#ifndef FLPROG_MQTT_MAX_TOPIC_LENGTH
#define FLPROG_MQTT_MAX_TOPIC_LENGTH 127U
#endif
#ifndef FLPROG_MQTT_MAX_CLIENT_ID_LENGTH
#define FLPROG_MQTT_MAX_CLIENT_ID_LENGTH 63U
#endif
#ifndef FLPROG_MQTT_MAX_USERNAME_LENGTH
#define FLPROG_MQTT_MAX_USERNAME_LENGTH 95U
#endif
#ifndef FLPROG_MQTT_MAX_PASSWORD_LENGTH
#define FLPROG_MQTT_MAX_PASSWORD_LENGTH 95U
#endif
#ifndef FLPROG_MQTT_MAX_SUBSCRIPTIONS
#define FLPROG_MQTT_MAX_SUBSCRIPTIONS 4U
#endif
#ifndef FLPROG_MQTT_READ_BUDGET
#define FLPROG_MQTT_READ_BUDGET 256U
#endif
#ifndef FLPROG_MQTT_WRITE_BUDGET
#define FLPROG_MQTT_WRITE_BUDGET 256U
#endif

#define FLPROG_MQTT_HOST_NONE 0
#define FLPROG_MQTT_HOST_NAME 1
#define FLPROG_MQTT_HOST_IP   2

#define FLPROG_MQTT_STATE_IDLE           0
#define FLPROG_MQTT_STATE_TCP_CONNECTING 1
#define FLPROG_MQTT_STATE_SENDING        2
#define FLPROG_MQTT_STATE_WAIT_CONNACK   3
#define FLPROG_MQTT_STATE_CONNECTED      4
#define FLPROG_MQTT_STATE_WAIT_SUBACK    5
#define FLPROG_MQTT_STATE_WAIT_PINGRESP  6
#define FLPROG_MQTT_STATE_BACKOFF        7
#define FLPROG_MQTT_STATE_ERROR          8

#define FLPROG_MQTT_ERROR_NONE              0
#define FLPROG_MQTT_ERROR_BUSY              1
#define FLPROG_MQTT_ERROR_NO_BROKER         2
#define FLPROG_MQTT_ERROR_NO_CLIENT_ID      3
#define FLPROG_MQTT_ERROR_SERVICE_LIMIT     4
#define FLPROG_MQTT_ERROR_SOCKET_REGISTRY   5
#define FLPROG_MQTT_ERROR_CONNECT_TIMEOUT   6
#define FLPROG_MQTT_ERROR_TCP               7
#define FLPROG_MQTT_ERROR_PACKET_TOO_LARGE  8
#define FLPROG_MQTT_ERROR_PROTOCOL          9
#define FLPROG_MQTT_ERROR_CONNACK_REFUSED  10
#define FLPROG_MQTT_ERROR_SUBACK            11
#define FLPROG_MQTT_ERROR_TIMEOUT           12
#define FLPROG_MQTT_ERROR_UNSUPPORTED       13
#define FLPROG_MQTT_ERROR_BAD_TOPIC         14
#define FLPROG_MQTT_ERROR_NOT_CONNECTED     15
#define FLPROG_MQTT_ERROR_CREDENTIALS       16

#define FLPROG_MQTT_EVENT_NONE                0x0000U
#define FLPROG_MQTT_EVENT_CONNECTED           0x0001U
#define FLPROG_MQTT_EVENT_DISCONNECTED        0x0002U
#define FLPROG_MQTT_EVENT_MESSAGE             0x0004U
#define FLPROG_MQTT_EVENT_PUBLISHED           0x0008U
#define FLPROG_MQTT_EVENT_SUBSCRIBED          0x0010U
#define FLPROG_MQTT_EVENT_PINGRESP            0x0020U
#define FLPROG_MQTT_EVENT_FAILED              0x0040U
#define FLPROG_MQTT_EVENT_RECONNECT_SCHEDULED 0x0080U
#define FLPROG_MQTT_EVENT_CANCELED            0x0100U

#define FLPROG_MQTT_TX_NONE        0
#define FLPROG_MQTT_TX_CONNECT     1
#define FLPROG_MQTT_TX_PUBLISH     2
#define FLPROG_MQTT_TX_SUBSCRIBE   3
#define FLPROG_MQTT_TX_RESUBSCRIBE 4
#define FLPROG_MQTT_TX_PING        5
#define FLPROG_MQTT_TX_DISCONNECT  6

typedef void (*FLProgNetworkMqttMessageCallback)(
    const char *topic,
    const uint8_t *payload,
    size_t length,
    bool retained);

// MQTT 3.1.1 baseline client. One service object owns one route-aware TCP
// client. DNS/TCP/protocol progress happens only from network.pool().
class FLProgNetworkMqttClient : public FLProgNetworkService
{
public:
    FLProgNetworkMqttClient() {}
    ~FLProgNetworkMqttClient() override;

    bool begin(FLProgNetworkInterface &network, uint8_t route = 0);
    void end();
    void pool() override;

    void setRoute(uint8_t value);
    uint8_t route() const { return _route; }

    void setBroker(const char *value);
    void setBroker(const String &value) { setBroker(value.c_str()); }
    void setBroker(IPAddress value);
    void clearBroker();
    uint8_t brokerMode() const { return _brokerMode; }
    const char *brokerName() const { return _brokerName; }
    IPAddress brokerIP() const { return _brokerIp; }

    void setPort(uint16_t value) { _port = value; }
    uint16_t port() const { return _port; }

    bool setClientId(const char *value);
    bool setClientId(const String &value) { return setClientId(value.c_str()); }
    const char *clientId() const { return _clientId; }

    bool setCredentials(const char *username, const char *password);
    void clearCredentials();
    bool hasCredentials() const { return _username[0] != 0; }
    const char *username() const { return _username; }

    void setKeepAlive(uint16_t seconds) { _keepAliveSeconds = seconds; }
    uint16_t keepAlive() const { return _keepAliveSeconds; }

    void setConnectTimeout(uint32_t value)
    {
        _connectTimeoutMs = value;
        _tcp.setTimeout(value);
    }
    uint32_t connectTimeout() const { return _connectTimeoutMs; }

    void setResponseTimeout(uint32_t value) { _responseTimeoutMs = value; }
    uint32_t responseTimeout() const { return _responseTimeoutMs; }

    void setAutoReconnect(bool value) { _autoReconnect = value; }
    bool autoReconnect() const { return _autoReconnect; }
    void setReconnectBackoff(uint32_t initialMs, uint32_t maxMs);
    uint32_t reconnectInitialDelay() const { return _reconnectInitialMs; }
    uint32_t reconnectMaximumDelay() const { return _reconnectMaxMs; }
    uint32_t reconnectCurrentDelay() const { return _reconnectCurrentMs; }
    uint32_t reconnectIn() const;

    // connect()/disconnect()/cancel() only queue intent. No network I/O occurs
    // until the owning FLProgNetworkInterface is pooled.
    bool connect();
    void disconnect();
    void cancel();

    bool publish(
        const char *topic,
        const uint8_t *payload,
        size_t length,
        bool retained = false);
    bool publish(const char *topic, const char *payload, bool retained = false)
    {
        const char *value = payload == nullptr ? "" : payload;
        return publish(topic, reinterpret_cast<const uint8_t *>(value), strlen(value), retained);
    }
    bool publish(const String &topic, const String &payload, bool retained = false)
    {
        return publish(topic.c_str(), payload.c_str(), retained);
    }

    // Baseline subscription QoS is deliberately fixed to 0 in dev.4.
    bool subscribe(const char *topicFilter);
    bool subscribe(const String &topicFilter) { return subscribe(topicFilter.c_str()); }

    bool connected() const { return _sessionConnected; }
    bool busy() const
    {
        return _state == FLPROG_MQTT_STATE_TCP_CONNECTING ||
               _state == FLPROG_MQTT_STATE_SENDING ||
               _state == FLPROG_MQTT_STATE_WAIT_CONNACK ||
               _state == FLPROG_MQTT_STATE_WAIT_SUBACK ||
               _state == FLPROG_MQTT_STATE_WAIT_PINGRESP;
    }
    bool reconnectScheduled() const { return _state == FLPROG_MQTT_STATE_BACKOFF; }
    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t connackCode() const { return _connackCode; }
    uint8_t tcpClientError() const { return _tcp.clientError(); }
    uint8_t dnsError() const { return _tcp.dnsError(); }

    uint16_t events() const { return _events; }
    uint16_t takeEvents()
    {
        const uint16_t value = _events;
        _events = FLPROG_MQTT_EVENT_NONE;
        return value;
    }

    void setMessageCallback(FLProgNetworkMqttMessageCallback value)
    {
        _messageCallback = value;
    }

    uint8_t subscriptionCount() const { return _subscriptionCount; }
    const char *subscription(uint8_t index) const
    {
        return index < _subscriptionCount ? _subscriptions[index] : "";
    }

    const char *lastMessageTopic() const { return _lastMessageTopic; }
    size_t lastMessageLength() const { return _lastMessageLength; }
    bool lastMessageRetained() const { return _lastMessageRetained; }

    uint32_t connectAttemptCount() const { return _connectAttemptCount; }
    uint32_t connectSuccessCount() const { return _connectSuccessCount; }
    uint32_t disconnectCount() const { return _disconnectCount; }
    uint32_t publishCount() const { return _publishCount; }
    uint32_t subscribeCount() const { return _subscribeCount; }
    uint32_t subscribeFailureCount() const { return _subscribeFailureCount; }
    uint32_t messageCount() const { return _messageCount; }
    uint32_t pingRequestCount() const { return _pingRequestCount; }
    uint32_t pingResponseCount() const { return _pingResponseCount; }
    uint32_t reconnectScheduleCount() const { return _reconnectScheduleCount; }
    uint32_t protocolErrorCount() const { return _protocolErrorCount; }
    uint32_t oversizedPacketCount() const { return _oversizedPacketCount; }
    uint32_t unknownPacketCount() const { return _unknownPacketCount; }

    void resetStatistics();

    FLProgNetworkTcpClient *tcp() { return &_tcp; }

private:
    static bool ipIsZero(IPAddress value);
    static bool parseIpv4Literal(const char *value, IPAddress &result);
    static bool timeReached(uint32_t now, uint32_t target)
    {
        return (int32_t)(now - target) >= 0;
    }
    static bool validPublishTopic(const char *value);
    static bool validTopicFilter(const char *value);
    static size_t encodeRemainingLength(uint32_t value, uint8_t *out);

    bool writeUtf(size_t &offset, const char *value, size_t maxLength);
    bool finishPacket(uint8_t fixedHeader, size_t bodyLength, uint8_t purpose);
    bool buildConnectPacket();
    bool buildPublishPacket(
        const char *topic,
        const uint8_t *payload,
        size_t length,
        bool retained);
    bool buildSubscribePacket(const char *topicFilter, uint8_t purpose);
    bool buildPingPacket();
    bool buildDisconnectPacket();

    void startTcpConnect();
    void serviceTcpConnecting();
    void serviceSend();
    void serviceReceive();
    bool feedRxByte(uint8_t value);
    bool handlePacket(uint8_t header, const uint8_t *body, size_t length);
    bool handleConnack(const uint8_t *body, size_t length);
    bool handleSuback(const uint8_t *body, size_t length);
    bool handlePublish(uint8_t header, const uint8_t *body, size_t length);
    bool handlePingresp(const uint8_t *body, size_t length);

    void resetRxParser();
    void resetTransaction();
    void completeSubscribe();
    void addOrKeepSubscription(const char *topicFilter);
    void scheduleReconnect(uint8_t errorValue);
    void fail(uint8_t errorValue, bool reconnectAllowed = true);
    void processDisconnectRequest();
    void processCancelRequest();
    void serviceConnectedIdle();
    void resetReconnectBackoff();
    void advanceReconnectBackoff();

    FLProgNetworkTcpClient _tcp;
    bool _tcpAttached = false;

    uint8_t _route = 0;
    uint8_t _brokerMode = FLPROG_MQTT_HOST_NONE;
    char _brokerName[FLPROG_TCP_HOSTNAME_MAX_LENGTH + 1] = {0};
    IPAddress _brokerIp = IPAddress(0, 0, 0, 0);
    uint16_t _port = FLPROG_MQTT_DEFAULT_PORT;
    char _clientId[FLPROG_MQTT_MAX_CLIENT_ID_LENGTH + 1] = {0};
    char _username[FLPROG_MQTT_MAX_USERNAME_LENGTH + 1] = {0};
    char _password[FLPROG_MQTT_MAX_PASSWORD_LENGTH + 1] = {0};

    uint16_t _keepAliveSeconds = FLPROG_MQTT_DEFAULT_KEEPALIVE_SECONDS;
    uint32_t _connectTimeoutMs = FLPROG_MQTT_CONNECT_TIMEOUT_MS;
    uint32_t _responseTimeoutMs = FLPROG_MQTT_RESPONSE_TIMEOUT_MS;
    bool _autoReconnect = true;
    uint32_t _reconnectInitialMs = FLPROG_MQTT_RECONNECT_INITIAL_MS;
    uint32_t _reconnectMaxMs = FLPROG_MQTT_RECONNECT_MAX_MS;
    uint32_t _reconnectCurrentMs = FLPROG_MQTT_RECONNECT_INITIAL_MS;
    uint32_t _reconnectAt = 0;

    bool _wantConnected = false;
    bool _disconnectRequested = false;
    bool _cancelRequested = false;
    bool _connectIssued = false;
    bool _sessionConnected = false;
    uint8_t _state = FLPROG_MQTT_STATE_IDLE;
    uint8_t _error = FLPROG_MQTT_ERROR_NONE;
    uint8_t _connackCode = 0;
    uint16_t _events = FLPROG_MQTT_EVENT_NONE;

    uint32_t _stateStartedAt = 0;
    uint32_t _lastProgressAt = 0;
    uint32_t _lastTxAt = 0;
    uint32_t _lastRxAt = 0;

    uint8_t _txBuffer[FLPROG_MQTT_MAX_PACKET_SIZE];
    size_t _txLength = 0;
    size_t _txOffset = 0;
    uint8_t _txPurpose = FLPROG_MQTT_TX_NONE;

    uint8_t _rxBuffer[FLPROG_MQTT_MAX_PACKET_SIZE];
    uint8_t _rxHeader = 0;
    uint8_t _rxStage = 0;
    uint8_t _rxLengthBytes = 0;
    uint32_t _rxRemainingLength = 0;
    uint32_t _rxMultiplier = 1;
    size_t _rxBodyOffset = 0;

    uint16_t _nextPacketId = 1;
    uint16_t _waitingPacketId = 0;
    char _pendingSubscribe[FLPROG_MQTT_MAX_TOPIC_LENGTH + 1] = {0};
    bool _pendingSubscribeIsResubscribe = false;
    char _subscriptions[FLPROG_MQTT_MAX_SUBSCRIPTIONS][FLPROG_MQTT_MAX_TOPIC_LENGTH + 1] = {{0}};
    uint8_t _subscriptionCount = 0;
    uint8_t _resubscribeIndex = 0;
    bool _resubscribeActive = false;

    FLProgNetworkMqttMessageCallback _messageCallback = nullptr;
    char _lastMessageTopic[FLPROG_MQTT_MAX_TOPIC_LENGTH + 1] = {0};
    size_t _lastMessageLength = 0;
    bool _lastMessageRetained = false;

    uint32_t _connectAttemptCount = 0;
    uint32_t _connectSuccessCount = 0;
    uint32_t _disconnectCount = 0;
    uint32_t _publishCount = 0;
    uint32_t _subscribeCount = 0;
    uint32_t _subscribeFailureCount = 0;
    uint32_t _messageCount = 0;
    uint32_t _pingRequestCount = 0;
    uint32_t _pingResponseCount = 0;
    uint32_t _reconnectScheduleCount = 0;
    uint32_t _protocolErrorCount = 0;
    uint32_t _oversizedPacketCount = 0;
    uint32_t _unknownPacketCount = 0;
};
