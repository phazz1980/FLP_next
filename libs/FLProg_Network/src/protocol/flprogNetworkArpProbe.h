/*
 * FLProg_Network — flprogNetworkArpProbe.h
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
#pragma once

#include <Arduino.h>
#include <IPAddress.h>
#include "flprogNetworkMacRaw.h"

class FLProgNetworkInterface;

#ifndef FLPROG_ARP_PROBE_PENDING
#define FLPROG_ARP_PROBE_PENDING 0
#endif
#ifndef FLPROG_ARP_PROBE_CLEAR
#define FLPROG_ARP_PROBE_CLEAR 1
#endif
#ifndef FLPROG_ARP_PROBE_CONFLICT
#define FLPROG_ARP_PROBE_CONFLICT 2
#endif
#ifndef FLPROG_ARP_PROBE_ERROR
#define FLPROG_ARP_PROBE_ERROR 3
#endif

// RFC 5227 IPv4 duplicate-address probe / announcement helper for Wiznet.
// MACRAW socket 0 is reserved only while probing or transmitting an ARP
// announcement; it is released again as soon as the operation completes.
class FLProgWiznetArpProbe
{
public:
    FLProgWiznetArpProbe() {}
    ~FLProgWiznetArpProbe();

    bool begin(FLProgNetworkInterface &network, IPAddress address);
    bool beginAnnouncement(FLProgNetworkInterface &network, IPAddress address, uint8_t count = 2);
    void stop();
    uint8_t pool();

    bool active() const { return _active; }
    IPAddress address() const { return _address; }
    uint8_t probesSent() const { return _probesSent; }
    uint8_t announcementsSent() const { return _announcementsSent; }
    uint32_t conflictCount() const { return _conflictCount; }
    uint32_t deferredCount() const { return _wiznetBackend.deferredCount(); }

    bool hasConflictMac() const { return _hasConflictMac; }
    const uint8_t *conflictMac() const { return _conflictMac; }

    // Advanced test/porting hook. nullptr restores the direct Wiznet MACRAW
    // backend. Production code normally does not need this method.
    void setBackend(FLProgMacRawBackend *backend) { _externalBackend = backend; }

private:
    enum State
    {
        STATE_IDLE = 0,
        STATE_OPENING = 1,
        STATE_PROBING = 2,
        STATE_ANNOUNCING = 3,
        STATE_CLOSING = 4
    };

    bool beginOperation(FLProgNetworkInterface &network, IPAddress address, bool announcement, uint8_t announcementCount);
    bool ensureSocket();
    bool closeSocket();
    bool sendProbe();
    bool sendAnnouncement();
    bool sendArpRequest(bool announcement);
    bool receiveConflict();
    bool isConflictFrame(const uint8_t *frame, size_t length);
    void beginClosing(uint8_t result);
    uint32_t initialDelay() const;
    uint32_t repeatDelay() const;
    static bool ipEquals(const uint8_t *bytes, IPAddress ip);
    static bool bytesAllZero(const uint8_t *bytes, size_t length);

    FLProgNetworkInterface *_network = nullptr;
    FLProgWiznetMacRawBackend _wiznetBackend;
    FLProgMacRawBackend *_externalBackend = nullptr;
    FLProgMacRawBackend *_backend = nullptr;

    int16_t _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    FLProgSocketLease _lease;
    FLProgSocketEndpoint _endpoint;

    IPAddress _address = IPAddress(0, 0, 0, 0);
    uint8_t _mac[6] = {0, 0, 0, 0, 0, 0};
    uint8_t _conflictMac[6] = {0, 0, 0, 0, 0, 0};

    uint8_t _state = STATE_IDLE;
    uint8_t _result = FLPROG_ARP_PROBE_PENDING;
    uint8_t _probesSent = 0;
    uint8_t _announcementsSent = 0;
    uint8_t _announcementTargetCount = 0;
    uint32_t _nextAction = 0;
    uint32_t _conflictCount = 0;
    bool _active = false;
    bool _hasConflictMac = false;
    bool _announcementMode = false;

    uint8_t _txFrame[60] = {0};
    uint8_t _txOffset = 0;
    bool _txBuilding = false;
};

// Passive strict RFC 5227 observer. Unlike FLProgWiznetArpProbe this object
// keeps Wiznet MACRAW socket 0 reserved while active and watches both ARP
// Requests and ARP Replies for the configured IPv4 address.
class FLProgWiznetArpMonitor
{
public:
    FLProgWiznetArpMonitor() {}
    ~FLProgWiznetArpMonitor();

    bool begin(FLProgNetworkInterface &network, IPAddress address);
    void stop();
    void pool();

    bool active() const { return _active; }
    bool socketReserved() const { return _nativeSocket == 0; }
    bool socketOpen() const { return _backend != nullptr && _backend->isOpen(); }
    bool resourceBlocked() const { return _resourceBlocked; }
    bool takeConflict();
    uint32_t conflictCount() const { return _conflictCount; }
    uint32_t resourceBlockedCount() const { return _resourceBlockedCount; }
    uint32_t deferredCount() const { return _wiznetBackend.deferredCount(); }
    bool hasConflictMac() const { return _hasConflictMac; }
    const uint8_t *conflictMac() const { return _conflictMac; }

    void setBackend(FLProgMacRawBackend *backend) { _externalBackend = backend; }

private:
    bool ensureSocket();
    bool receiveConflict();
    bool isConflictFrame(const uint8_t *frame, size_t length);
    static bool ipEquals(const uint8_t *bytes, IPAddress ip);
    static bool bytesAllZero(const uint8_t *bytes, size_t length);

    FLProgNetworkInterface *_network = nullptr;
    FLProgWiznetMacRawBackend _wiznetBackend;
    FLProgMacRawBackend *_externalBackend = nullptr;
    FLProgMacRawBackend *_backend = nullptr;
    int16_t _nativeSocket = FLPROG_SOCKET_NATIVE_NONE;
    FLProgSocketLease _lease;
    FLProgSocketEndpoint _endpoint;
    IPAddress _address = IPAddress(0, 0, 0, 0);
    uint8_t _mac[6] = {0, 0, 0, 0, 0, 0};
    uint8_t _conflictMac[6] = {0, 0, 0, 0, 0, 0};
    uint32_t _conflictCount = 0;
    uint32_t _resourceBlockedCount = 0;
    bool _active = false;
    bool _resourceBlocked = false;
    bool _conflictPending = false;
    bool _hasConflictMac = false;
};
