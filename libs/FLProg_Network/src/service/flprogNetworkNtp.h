/*
 * FLProg_Network — flprogNetworkNtp.h
 * Назначение: NTP client/service поверх UDP.
 *
 * Архитектурные заметки:
 * - Запрос/ответ NTP использует native UDP и route выбранного NetworkDevice.
 * - Таймаут NTP не является transport failure.
 * - Повторный запрос должен быть безопасен после смены generation.
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
#include "flprogNtpTimeZone.h"
#define FLPROG_NTP_TIMEZONE_API 1
#include "flprogNetworkService.h"
#include "../protocol/flprogNetworkUdp.h"
#include "../protocol/flprogNetworkDns.h"

class FLProgNetworkInterface;

#ifndef FLPROG_NTP_PACKET_SIZE
#define FLPROG_NTP_PACKET_SIZE 48
#endif

#ifndef FLPROG_NTP_DEFAULT_REQUEST_PERIOD_MS
#define FLPROG_NTP_DEFAULT_REQUEST_PERIOD_MS 3600000UL
#endif

#ifndef FLPROG_NTP_DEFAULT_TIMEOUT_MS
#define FLPROG_NTP_DEFAULT_TIMEOUT_MS 15000UL
#endif

#ifndef FLPROG_NTP_DEFAULT_LOCAL_PORT
#define FLPROG_NTP_DEFAULT_LOCAL_PORT 8888U
#endif

#ifndef FLPROG_NTP_SERVER_PORT
#define FLPROG_NTP_SERVER_PORT 123U
#endif

#ifndef FLPROG_NTP_EPOCH_DELTA
#define FLPROG_NTP_EPOCH_DELTA 2208988800UL
#endif

// NTP era 1 begins when the 32-bit NTP seconds field wraps in 2036.
// For the uint32_t Unix API, low NTP values map to Unix 2036..2106.
#ifndef FLPROG_NTP_ERA1_UNIX_BASE
#define FLPROG_NTP_ERA1_UNIX_BASE 2085978496UL
#endif

#define FLPROG_NTP_SERVER_NONE 0
#define FLPROG_NTP_SERVER_NAME 1
#define FLPROG_NTP_SERVER_IP   2

#define FLPROG_NTP_STATE_DISABLED       0
#define FLPROG_NTP_STATE_IDLE           1
#define FLPROG_NTP_STATE_WAIT_NETWORK   2
#define FLPROG_NTP_STATE_RESOLVING      3
#define FLPROG_NTP_STATE_OPENING_UDP    4
#define FLPROG_NTP_STATE_SENDING        5
#define FLPROG_NTP_STATE_WAIT_RESPONSE  6
#define FLPROG_NTP_STATE_ERROR          7

#define FLPROG_NTP_ERROR_NONE             0
#define FLPROG_NTP_ERROR_NO_NETWORK       1
#define FLPROG_NTP_ERROR_NO_SERVER        2
#define FLPROG_NTP_ERROR_DNS              3
#define FLPROG_NTP_ERROR_UDP_OPEN         4
#define FLPROG_NTP_ERROR_SEND             5
#define FLPROG_NTP_ERROR_TIMEOUT          6
#define FLPROG_NTP_ERROR_SHORT_PACKET     7
#define FLPROG_NTP_ERROR_INVALID_PACKET   8
#define FLPROG_NTP_ERROR_UNSYNCHRONIZED   9
#define FLPROG_NTP_ERROR_KISS_OF_DEATH   10
#define FLPROG_NTP_ERROR_WRONG_SOURCE    11
#define FLPROG_NTP_ERROR_BAD_EPOCH       12
#define FLPROG_NTP_ERROR_SERVICE_LIMIT   13

#define FLPROG_NTP_EVENT_NONE       0x00
#define FLPROG_NTP_EVENT_SYNCED     0x01
#define FLPROG_NTP_EVENT_FAILED     0x02
#define FLPROG_NTP_EVENT_TIMEOUT    0x04
#define FLPROG_NTP_EVENT_CANCELED   0x08

struct FLProgNtpDateTime
{
    uint32_t unixTime = 0;
    uint16_t year = 1970;
    uint8_t month = 1;
    uint8_t day = 1;
    uint8_t hours = 0;
    uint8_t minutes = 0;
    uint8_t seconds = 0;
};

typedef void (*FLProgNetworkNtpCallback)();

// Route-aware, cooperative NTP client. All protocol progress happens from
// FLProgNetworkInterface::pool(); no user ntp.pool() call is required.
// The uint32_t Unix clock unfolds NTP era 0/1 over 1970..2106. This baseline
// intentionally does not implement a four-timestamp clock-offset discipline.
class FLProgNetworkNtp : public FLProgNetworkService
{
public:
    FLProgNetworkNtp() {}
    ~FLProgNetworkNtp() override;

    bool begin(
        FLProgNetworkInterface &network,
        uint8_t route = 0);
    void end();
    void pool() override;

    void setEnabled(bool value);
    bool enabled() const { return _enabled; }

    void setRoute(uint8_t value);
    uint8_t route() const { return _route; }

    void setServer(const char *value);
    void setServer(const String &value) { setServer(value.c_str()); }
    void setServer(IPAddress value);
    void clearServer();

    // LEGACY API aliases retained for Flprog_Ethernet source compatibility.
    // New code/examples should use the correctly-spelled primary API above.
    // Generated FLProg sketches pass string literals here. On ESP32 a
    // const char* can also be converted to IPAddress, so retain an exact
    // overload to prevent ambiguity with timeServer(IPAddress).
    void timeServer(const char *value) { setServer(value); }
    void timeServer(const String &value) { setServer(value.c_str()); }
    void timeServer(IPAddress value) { setServer(value); }

    uint8_t serverMode() const { return _serverMode; }
    const char *serverName() const { return _serverName; }
    IPAddress serverIP() const { return _serverIp; }
    IPAddress lastResolvedIP() const { return _resolvedIp; }

    void setRequestPeriod(uint32_t seconds)
    {
        setRequestPeriodMs(
            seconds > 4294967UL
                ? 0xFFFFFFFFUL
                : seconds * 1000UL
        );
    }
    uint32_t requestPeriod() const { return _requestPeriodMs / 1000UL; }
    void setRequestPeriodMs(uint32_t value);
    uint32_t requestPeriodMs() const { return _requestPeriodMs; }

    void reqestPeriod(uint16_t value) { setRequestPeriod((uint32_t)value); }
    uint16_t reqestPeriod() const
    {
        const uint32_t value = requestPeriod();
        return value > 65535UL ? 65535U : (uint16_t)value;
    }

    void setTimeout(uint32_t value) { _timeoutMs = value; }
    uint32_t timeout() const { return _timeoutMs; }

    void setLocalPort(uint16_t value) { _localPort = value; }
    uint16_t localPort() const { return _localPort; }
    void localPort(uint32_t value)
    {
        _localPort = value > 65535UL ? 65535U : (uint16_t)value;
    }

    void setUseAsClock(bool value);
    bool useAsClock() const { return _useAsClock; }
    void isUseAsClock(bool value) { setUseAsClock(value); }
    bool isUseAsClock() const { return useAsClock(); }

    void setStrictSourceCheck(bool value) { _strictSourceCheck = value; }
    bool strictSourceCheck() const { return _strictSourceCheck; }

    void setStrictOriginateCheck(bool value) { _strictOriginateCheck = value; }
    bool strictOriginateCheck() const { return _strictOriginateCheck; }

    bool syncNow();
    void cancel();

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint8_t getStatus() const { return _state; }
    uint8_t getError() const { return _error; }

    bool busy() const
    {
        return _state == FLPROG_NTP_STATE_WAIT_NETWORK ||
               _state == FLPROG_NTP_STATE_RESOLVING ||
               _state == FLPROG_NTP_STATE_OPENING_UDP ||
               _state == FLPROG_NTP_STATE_SENDING ||
               _state == FLPROG_NTP_STATE_WAIT_RESPONSE;
    }

    bool hasTime() const { return _hasTime; }
    bool ready() const { return _hasTime && !busy(); }
    bool failed() const { return _state == FLPROG_NTP_STATE_ERROR; }

    uint8_t events() const { return _events; }
    uint8_t takeEvents()
    {
        const uint8_t value = _events;
        _events = FLPROG_NTP_EVENT_NONE;
        return value;
    }

    void setCallback(FLProgNetworkNtpCallback value) { _callback = value; }
    void setCallBack(FLProgNetworkNtpCallback value) { setCallback(value); }

    // Existing unixTime/dateTime/hour methods remain UTC for source compatibility.
    // mode0=fixed UTC offset, mode1=EU, mode2=US. Invalid settings leave the old zone.
    bool setTimeZone(int minutes, uint8_t mode=0) {
        if(mode==0)return _timeZone.setFixed(minutes);
        if(mode==1)return _timeZone.setEuropean(minutes);
        if(mode==2)return _timeZone.setUS(minutes);
        return false;
    }
    bool setDaylightRules(int standardMinutes,int summerMinutes,const FLProgNtpDstRule &start,const FLProgNtpDstRule &end) {
        return _timeZone.setRules(standardMinutes,summerMinutes,start,end);
    }
    FLProgNtpLocalDateTime localDateTime() const {return _timeZone.local(unixTime());}
    int16_t utcOffsetMinutes() const {return _timeZone.offsetMinutes(unixTime());}
    bool isDaylightSaving() const {return _timeZone.isDaylight(unixTime());}

    uint32_t unixTime() const { return _dateTime.unixTime; }
    uint32_t getUnixTime() const { return unixTime(); }
    uint8_t second() const { return _dateTime.seconds; }
    uint8_t minute() const { return _dateTime.minutes; }
    uint8_t hour() const { return _dateTime.hours; }
    uint8_t day() const { return _dateTime.day; }
    uint8_t month() const { return _dateTime.month; }
    uint16_t year() const { return _dateTime.year; }

    uint8_t getSecond() const { return second(); }
    uint8_t getMinutes() const { return minute(); }
    uint8_t getHours() const { return hour(); }
    uint8_t getDay() const { return day(); }
    uint8_t getMonth() const { return month(); }
    uint16_t getYear() const { return year(); }

    const FLProgNtpDateTime &dateTime() const { return _dateTime; }

    uint32_t requestCount() const { return _requestCount; }
    uint32_t successCount() const { return _successCount; }
    uint32_t getCounter() const { return _successCount; }
    uint32_t timeoutCount() const { return _timeoutCount; }
    uint32_t dnsErrorCount() const { return _dnsErrorCount; }
    uint32_t invalidPacketCount() const { return _invalidPacketCount; }
    uint32_t ignoredPacketCount() const { return _ignoredPacketCount; }
    uint32_t cancelCount() const { return _cancelCount; }
    uint32_t lastRoundTripMs() const { return _lastRoundTripMs; }
    uint32_t lastSyncMillis() const { return _lastSyncMillis; }

    void resetStatistics()
    {
        _requestCount = 0;
        _successCount = 0;
        _timeoutCount = 0;
        _dnsErrorCount = 0;
        _invalidPacketCount = 0;
        _ignoredPacketCount = 0;
        _cancelCount = 0;
    }

    // Exposed for diagnostics/testing/custom adapters. Ownership remains here.
    FLProgNetworkUdp *udp() { return &_udp; }
    FLProgNetworkDns *dns() { return &_dns; }

private:
    static bool ipIsZero(IPAddress value);
    static bool timeReached(uint32_t now, uint32_t target)
    {
        return (int32_t)(now - target) >= 0;
    }
    static bool isLeapYear(uint16_t year);
    static uint8_t daysInMonth(uint16_t year, uint8_t month);

    void setState(uint8_t value);
    void updateClock();
    void updateDateTime(uint32_t unixTime);
    bool ensureDns();
    void startTransaction();
    void waitNetworkStep();
    void resolvingStep();
    void openUdpStep();
    void sendingStep();
    void waitResponseStep();
    void buildRequestPacket();
    bool validateResponse(uint8_t &responseError, uint32_t &unixTime);
    void finishSuccess(uint32_t unixTime);
    void finishError(uint8_t value);
    void closeTransactionResources();
    void cancelActive(bool countEvent);
    void scheduleNext();

    FLProgNetworkUdp _udp;
    FLProgNetworkDns _dns;
    bool _dnsAttached = false;

    bool _enabled = true;
    bool _useAsClock = true;
    bool _strictSourceCheck = true;
    bool _strictOriginateCheck = true;
    bool _hasTime = false;
    bool _requestPending = true;
    bool _restartRequested = false;
    bool _cancelRequested = false;

    uint8_t _route = 0;
    uint8_t _serverMode = FLPROG_NTP_SERVER_NONE;
    char _serverName[FLPROG_DNS_MAX_HOSTNAME_LENGTH + 1] = {0};
    IPAddress _serverIp = IPAddress(0, 0, 0, 0);
    IPAddress _resolvedIp = IPAddress(0, 0, 0, 0);

    uint8_t _state = FLPROG_NTP_STATE_IDLE;
    uint8_t _error = FLPROG_NTP_ERROR_NO_SERVER;
    uint8_t _events = FLPROG_NTP_EVENT_NONE;

    uint16_t _localPort = FLPROG_NTP_DEFAULT_LOCAL_PORT;
    uint32_t _requestPeriodMs = FLPROG_NTP_DEFAULT_REQUEST_PERIOD_MS;
    uint32_t _timeoutMs = FLPROG_NTP_DEFAULT_TIMEOUT_MS;
    uint32_t _nextRequestAt = 0;
    uint32_t _requestStartedAt = 0;
    uint32_t _requestSentAt = 0;
    uint32_t _lastSyncMillis = 0;
    uint32_t _clockAnchorMillis = 0;
    uint32_t _lastRoundTripMs = 0;

    uint8_t _packet[FLPROG_NTP_PACKET_SIZE] = {0};
    uint8_t _requestToken[8] = {0};

    FLProgNtpTimeZone _timeZone;
    FLProgNtpDateTime _dateTime;
    FLProgNetworkNtpCallback _callback = nullptr;

    uint32_t _requestCount = 0;
    uint32_t _successCount = 0;
    uint32_t _timeoutCount = 0;
    uint32_t _dnsErrorCount = 0;
    uint32_t _invalidPacketCount = 0;
    uint32_t _ignoredPacketCount = 0;
    uint32_t _cancelCount = 0;
};
