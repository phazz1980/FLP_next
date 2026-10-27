/*
 * FLProg_Network — flprogNetworkDhcp.h
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
#pragma once

#include <Arduino.h>
#include <IPAddress.h>
#include "flprogNetworkUdp.h"
#include "flprogNetworkArpProbe.h"

class FLProgNetworkInterface;

#ifndef FLPROG_DHCP_CLIENT_PORT
#define FLPROG_DHCP_CLIENT_PORT 68
#endif
#ifndef FLPROG_DHCP_SERVER_PORT
#define FLPROG_DHCP_SERVER_PORT 67
#endif
#ifndef FLPROG_DHCP_MAX_PACKET_SIZE
#define FLPROG_DHCP_MAX_PACKET_SIZE 576
#endif

#ifndef FLPROG_DHCP_STATE_DISABLED
#define FLPROG_DHCP_STATE_DISABLED 0
#endif
#ifndef FLPROG_DHCP_STATE_INIT
#define FLPROG_DHCP_STATE_INIT 1
#endif
#ifndef FLPROG_DHCP_STATE_OPENING
#define FLPROG_DHCP_STATE_OPENING 2
#endif
#ifndef FLPROG_DHCP_STATE_SELECTING
#define FLPROG_DHCP_STATE_SELECTING 3
#endif
#ifndef FLPROG_DHCP_STATE_REQUESTING
#define FLPROG_DHCP_STATE_REQUESTING 4
#endif
#ifndef FLPROG_DHCP_STATE_BOUND
#define FLPROG_DHCP_STATE_BOUND 5
#endif
#ifndef FLPROG_DHCP_STATE_RENEWING
#define FLPROG_DHCP_STATE_RENEWING 6
#endif
#ifndef FLPROG_DHCP_STATE_REBINDING
#define FLPROG_DHCP_STATE_REBINDING 7
#endif
#ifndef FLPROG_DHCP_STATE_FAILED
#define FLPROG_DHCP_STATE_FAILED 8
#endif
#ifndef FLPROG_DHCP_STATE_CHECKING
#define FLPROG_DHCP_STATE_CHECKING 9
#endif
#ifndef FLPROG_DHCP_STATE_ANNOUNCING
#define FLPROG_DHCP_STATE_ANNOUNCING 10
#endif
#ifndef FLPROG_DHCP_STATE_DEFENDING
#define FLPROG_DHCP_STATE_DEFENDING 11
#endif
#ifndef FLPROG_DHCP_STATE_RATE_LIMIT
#define FLPROG_DHCP_STATE_RATE_LIMIT 12
#endif

#ifndef FLPROG_DHCP_ERROR_NONE
#define FLPROG_DHCP_ERROR_NONE 0
#endif
#ifndef FLPROG_DHCP_ERROR_HARDWARE
#define FLPROG_DHCP_ERROR_HARDWARE 1
#endif
#ifndef FLPROG_DHCP_ERROR_SOCKET
#define FLPROG_DHCP_ERROR_SOCKET 2
#endif
#ifndef FLPROG_DHCP_ERROR_TIMEOUT
#define FLPROG_DHCP_ERROR_TIMEOUT 3
#endif
#ifndef FLPROG_DHCP_ERROR_PROTOCOL
#define FLPROG_DHCP_ERROR_PROTOCOL 4
#endif
#ifndef FLPROG_DHCP_ERROR_NAK
#define FLPROG_DHCP_ERROR_NAK 5
#endif
#ifndef FLPROG_DHCP_ERROR_CONFLICT_PROBE
#define FLPROG_DHCP_ERROR_CONFLICT_PROBE 6
#endif
#ifndef FLPROG_DHCP_ERROR_ARP_COMPLETION
#define FLPROG_DHCP_ERROR_ARP_COMPLETION 7
#endif

#ifndef FLPROG_DHCP_MESSAGE_DISCOVER
#define FLPROG_DHCP_MESSAGE_DISCOVER 1
#endif
#ifndef FLPROG_DHCP_MESSAGE_OFFER
#define FLPROG_DHCP_MESSAGE_OFFER 2
#endif
#ifndef FLPROG_DHCP_MESSAGE_REQUEST
#define FLPROG_DHCP_MESSAGE_REQUEST 3
#endif
#ifndef FLPROG_DHCP_MESSAGE_DECLINE
#define FLPROG_DHCP_MESSAGE_DECLINE 4
#endif
#ifndef FLPROG_DHCP_MESSAGE_ACK
#define FLPROG_DHCP_MESSAGE_ACK 5
#endif
#ifndef FLPROG_DHCP_MESSAGE_NAK
#define FLPROG_DHCP_MESSAGE_NAK 6
#endif
#ifndef FLPROG_DHCP_MESSAGE_RELEASE
#define FLPROG_DHCP_MESSAGE_RELEASE 7
#endif

typedef int8_t (*FLProgDhcpConflictProbe)(IPAddress address, void *context);
typedef bool (*FLProgDhcpAddressConflictProbe)(void *context);


#ifndef FLPROG_ACD_MODE_OFF
#define FLPROG_ACD_MODE_OFF 0
#endif
#ifndef FLPROG_ACD_MODE_LIGHTWEIGHT
#define FLPROG_ACD_MODE_LIGHTWEIGHT 1
#endif
#ifndef FLPROG_ACD_MODE_STRICT
#define FLPROG_ACD_MODE_STRICT 2
#endif

#ifndef FLPROG_ARP_MAX_CONFLICTS
#define FLPROG_ARP_MAX_CONFLICTS 10
#endif
#ifndef FLPROG_ARP_RATE_LIMIT_MS
#define FLPROG_ARP_RATE_LIMIT_MS 60000UL
#endif
#ifndef FLPROG_ARP_DEFEND_INTERVAL_MS
#define FLPROG_ARP_DEFEND_INTERVAL_MS 10000UL
#endif

#ifndef FLPROG_DHCP_PROBE_PENDING
#define FLPROG_DHCP_PROBE_PENDING  (-1)
#endif
#ifndef FLPROG_DHCP_PROBE_CLEAR
#define FLPROG_DHCP_PROBE_CLEAR    0
#endif
#ifndef FLPROG_DHCP_PROBE_CONFLICT
#define FLPROG_DHCP_PROBE_CONFLICT 1
#endif

// Standalone non-blocking DHCP client for the Wiznet transport.
// It is deliberately driven by FLProgNetworkInterface::pool() before the
// Wiznet transport becomes READY, so it does not use FLProgNetworkSocket.
class FLProgWiznetDhcp
{
public:
    FLProgWiznetDhcp() {}
    ~FLProgWiznetDhcp();

    bool begin(FLProgNetworkInterface &network);
    void end(bool clearAddress = false);
    void restart();
    bool release();
    void pool();

    bool active() const { return _network != nullptr && _state != FLPROG_DHCP_STATE_DISABLED; }
    bool bound() const { return _state == FLPROG_DHCP_STATE_BOUND; }
    bool failed() const { return _state == FLPROG_DHCP_STATE_FAILED; }

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t lastMessageType() const { return _lastMessageType; }

    IPAddress leasedIP() const { return _leasedIp; }
    IPAddress serverIP() const { return _serverIp; }
    IPAddress gatewayIP() const { return _gatewayIp; }
    IPAddress subnetMask() const { return _subnetIp; }
    IPAddress dnsIP() const { return _dnsIp; }

    uint32_t leaseTime() const { return _leaseTime; }
    uint32_t renewalTime() const { return _renewalTime; }
    uint32_t rebindingTime() const { return _rebindingTime; }
    uint32_t leaseAge() const;

    uint32_t discoverCount() const { return _discoverCount; }
    uint32_t requestCount() const { return _requestCount; }
    uint32_t renewCount() const { return _renewCount; }
    uint32_t rebindCount() const { return _rebindCount; }
    uint32_t nakCount() const { return _nakCount; }
    uint32_t retryCount() const { return _retryCount; }
    uint32_t declineCount() const { return _declineCount; }
    uint32_t releaseCount() const { return _releaseCount; }

    void setAcquireTimeout(uint32_t value) { _acquireTimeout = value; }
    uint32_t acquireTimeout() const { return _acquireTimeout; }

    void setResponseTimeout(uint32_t value) { _responseTimeout = value; }
    uint32_t responseTimeout() const { return _responseTimeout; }

    // Advanced test/porting hook. nullptr restores the built-in direct Wiznet
    // UDP backend. Production code normally never needs this method.
    void setUdpBackend(FLProgUdpBackend *backend) { _externalBackend = backend; }

    // Optional non-blocking duplicate-address probe. Return PENDING while the
    // check is in progress, CLEAR when the address is safe, or CONFLICT when
    // another host owns it. CONFLICT causes DHCPDECLINE and a fresh acquire.
    void setConflictProbe(FLProgDhcpConflictProbe probe, void *context = nullptr)
    { _conflictProbe = probe; _conflictProbeContext = context; }
    void setConflictProbeTimeout(uint32_t value) { _conflictProbeTimeout = value; }
    uint32_t conflictProbeTimeout() const { return _conflictProbeTimeout; }

    // Native Wiznet RFC 5227 path. The initial probe, announcements and
    // address defense are independent switches so resource policy stays clear.
    void setBuiltInArpProbeEnabled(bool value) { _builtInArpProbeEnabled = value; }
    bool builtInArpProbeEnabled() const { return _builtInArpProbeEnabled; }
    void setArpAnnouncementEnabled(bool value) { _arpAnnouncementEnabled = value; }
    bool arpAnnouncementEnabled() const { return _arpAnnouncementEnabled; }
    void setArpAddressDefenseEnabled(bool value) { _arpAddressDefenseEnabled = value; }
    bool arpAddressDefenseEnabled() const { return _arpAddressDefenseEnabled; }

    void setAcdMode(uint8_t value)
    {
        if (value > FLPROG_ACD_MODE_STRICT) value = FLPROG_ACD_MODE_LIGHTWEIGHT;
        if (_acdMode != value) { _strictArpMonitor.stop(); _acdMode = value; }
    }
    uint8_t acdMode() const { return _acdMode; }
    bool strictAcdActive() const { return _acdMode == FLPROG_ACD_MODE_STRICT && _strictArpMonitor.socketOpen(); }
    bool strictAcdSocketReserved() const { return _strictArpMonitor.socketReserved(); }
    bool strictAcdResourceBlocked() const { return _acdMode == FLPROG_ACD_MODE_STRICT && _strictArpMonitor.resourceBlocked(); }
    uint32_t strictAcdResourceBlockedCount() const { return _strictArpMonitor.resourceBlockedCount(); }
    uint32_t strictAcdConflictCount() const { return _strictArpMonitor.conflictCount(); }

    uint8_t arpProbesSent() const { return _arpProbe.probesSent(); }
    uint32_t arpConflictCount() const { return _arpProbe.conflictCount(); }
    uint32_t arpAnnouncementCount() const { return _arpAnnouncementCount; }
    uint32_t arpDefenseCount() const { return _arpDefenseCount; }
    uint32_t arpDefenseConflictCount() const { return _arpDefenseConflictCount; }
    uint32_t arpRateLimitCount() const { return _arpRateLimitCount; }
    uint8_t arpConflictSeriesCount() const { return _arpConflictSeriesCount; }
    bool hasArpConflictMac() const { return _arpProbe.hasConflictMac(); }
    const uint8_t *arpConflictMac() const { return _arpProbe.conflictMac(); }

    // Advanced host-test/porting hooks. Production Wiznet code normally uses
    // the direct MACRAW backend and the common IR[CONFLICT] hardware flag.
    void setArpProbeBackend(FLProgMacRawBackend *backend)
    { _arpProbe.setBackend(backend); _arpProbeBackendConfigured = backend != nullptr; }
    void setStrictAcdBackend(FLProgMacRawBackend *backend)
    { _strictArpMonitor.setBackend(backend); }
    void setAddressConflictProbe(FLProgDhcpAddressConflictProbe probe, void *context = nullptr)
    { _addressConflictProbe = probe; _addressConflictProbeContext = context; }

private:
    struct Reply
    {
        uint8_t messageType = 0;
        IPAddress yiaddr = IPAddress(0, 0, 0, 0);
        IPAddress server = IPAddress(0, 0, 0, 0);
        IPAddress gateway = IPAddress(0, 0, 0, 0);
        IPAddress subnet = IPAddress(0, 0, 0, 0);
        IPAddress dns = IPAddress(0, 0, 0, 0);
        uint32_t lease = 0;
        uint32_t t1 = 0;
        uint32_t t2 = 0;
    };

    enum Exchange
    {
        EXCHANGE_NONE = 0,
        EXCHANGE_DISCOVER = 1,
        EXCHANGE_SELECT_REQUEST = 2,
        EXCHANGE_RENEW = 3,
        EXCHANGE_REBIND = 4
    };

    bool ensureHardware();
    bool ensureSocket();
    bool closeSocket();

    bool sendDiscover();
    bool sendSelectRequest();
    bool sendRenewRequest(bool rebind);
    bool sendDecline();
    bool sendRelease();
    bool sendMessage(uint8_t messageType, IPAddress destination, bool includeRequestedIp, bool includeServerId, bool ciaddrPresent, bool broadcastFlag);

    bool receiveReply(Reply &reply);
    bool parseReply(const uint8_t *packet, size_t length, Reply &reply) const;
    bool parseOptions(const uint8_t *options, size_t length, Reply &reply, uint8_t &overload) const;
    bool optionIPv4(const uint8_t *data, uint8_t length, IPAddress &target) const;

    size_t buildPacket(uint8_t messageType, bool includeRequestedIp, bool includeServerId, bool ciaddrPresent, bool broadcastFlag);
    bool appendOption(uint8_t code, const uint8_t *data, uint8_t length, size_t &position);
    bool appendOptionU32(uint8_t code, uint32_t value, size_t &position);
    bool appendOptionIp(uint8_t code, IPAddress ip, size_t &position);

    void setState(uint8_t value);
    void startInitialExchange();
    void startRenewExchange(bool rebind);
    void acceptOffer(const Reply &reply);
    void acceptAck(const Reply &reply);
    void finalizeAck();
    void completeBound();
    void handleNak();
    void fail(uint8_t errorCode);
    bool takeAddressConflict();
    void startAnnouncement(uint8_t count, uint8_t targetState);
    void scheduleReacquireAfterConflict();
    void clearLease(bool clearHardware);
    void applyZeroAddress();
    void applyLease();

    bool responseExpired(uint32_t now) const;
    bool acquireExpired(uint32_t now) const;
    uint32_t nextRetryDelay() const;
    uint32_t makeTransactionId() const;

    static uint32_t readU32(const uint8_t *data);
    static void writeU16(uint8_t *data, uint16_t value);
    static void writeU32(uint8_t *data, uint32_t value);
    static IPAddress readIp(const uint8_t *data);
    static bool ipIsZero(IPAddress ip);

    FLProgNetworkInterface *_network = nullptr;
    FLProgWiznetUdpBackend _wiznetUdp;
    FLProgUdpBackend *_externalBackend = nullptr;
    FLProgUdpBackend *_udp = nullptr;

    int16_t _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    bool _socketReleasePending = false;
    FLProgSocketLease _lease;
    FLProgSocketEndpoint _endpoint;

    uint8_t _state = FLPROG_DHCP_STATE_DISABLED;
    uint8_t _error = FLPROG_DHCP_ERROR_NONE;
    uint8_t _lastMessageType = 0;
    uint8_t _exchange = EXCHANGE_NONE;

    uint32_t _xid = 0;
    uint32_t _exchangeStarted = 0;
    uint32_t _lastSend = 0;
    uint32_t _leaseStarted = 0;
    uint8_t _exchangeAttempt = 0;

    IPAddress _offeredIp = IPAddress(0, 0, 0, 0);
    IPAddress _leasedIp = IPAddress(0, 0, 0, 0);
    IPAddress _serverIp = IPAddress(0, 0, 0, 0);
    IPAddress _gatewayIp = IPAddress(0, 0, 0, 0);
    IPAddress _subnetIp = IPAddress(0, 0, 0, 0);
    IPAddress _dnsIp = IPAddress(0, 0, 0, 0);

    uint32_t _leaseTime = 0;
    uint32_t _renewalTime = 0;
    uint32_t _rebindingTime = 0;

    uint32_t _acquireTimeout = 15000;
    uint32_t _responseTimeout = 1000;

    uint32_t _discoverCount = 0;
    uint32_t _requestCount = 0;
    uint32_t _renewCount = 0;
    uint32_t _rebindCount = 0;
    uint32_t _nakCount = 0;
    uint32_t _retryCount = 0;
    uint32_t _declineCount = 0;
    uint32_t _releaseCount = 0;

    FLProgDhcpConflictProbe _conflictProbe = nullptr;
    void *_conflictProbeContext = nullptr;
    uint32_t _conflictProbeStarted = 0;
    uint32_t _conflictProbeTimeout = 10000;

    FLProgWiznetArpProbe _arpProbe;
    FLProgWiznetArpMonitor _strictArpMonitor;
    uint8_t _acdMode = FLPROG_ACD_MODE_LIGHTWEIGHT;
    bool _builtInArpProbeEnabled = true;
    bool _arpAnnouncementEnabled = true;
    bool _arpAddressDefenseEnabled = true;
    bool _arpProbeBackendConfigured = false;
    bool _arpProbeStarted = false;
    bool _declinePending = false;
    bool _initialAckPending = false;
    uint8_t _announcementTargetState = FLPROG_DHCP_STATE_BOUND;

    FLProgDhcpAddressConflictProbe _addressConflictProbe = nullptr;
    void *_addressConflictProbeContext = nullptr;
    uint32_t _lastDefense = 0;
    uint32_t _rateLimitUntil = 0;
    uint32_t _arpAnnouncementCount = 0;
    uint32_t _arpDefenseCount = 0;
    uint32_t _arpDefenseConflictCount = 0;
    uint32_t _arpRateLimitCount = 0;
    uint8_t _arpConflictSeriesCount = 0;

    uint8_t _packet[FLPROG_DHCP_MAX_PACKET_SIZE] = {0};
};
