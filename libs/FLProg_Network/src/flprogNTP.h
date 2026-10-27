/*
 * Legacy public include facade for FLProg-generated NTP projects.
 * The implementation lives in service/flprogNetworkNtp.h; keep this stable
 * filename because generated sketches include "flprogNTP.h" directly.
 * Расширенный контракт сопровождения:
 * - Это публичная или межмодульная декларация: комментарии описывают владельца состояния и допустимый порядок вызовов.
 * - Конфигурация применяется до начала работы; runtime-методы обслуживаются из общего network.pool().
 * - При изменении API учитывать legacy generated-код и frozen API level 10006.
 * Extended maintenance contract:
 * - This declaration defines a public or cross-module boundary; document state ownership and call order here.
 * - Apply configuration before start; service runtime through the shared network.pool() lifecycle.
 * - Any API change must account for legacy generated code and frozen API level 10006.
 */
#pragma once

#include "flprogNetwork.h"

struct FLProgLegacyUnixId
{
    int8_t zone = 0;
    uint16_t year = 1970;
    uint8_t month = 1, day = 1, weekDay = 0;
    uint8_t hours = 0, minutes = 0, seconds = 0;
    uint32_t timeUNIX = 0;
};

class FLProgNTP : public FLProgNetworkNtp
{
public:
    explicit FLProgNTP(FLProgNetworkInterface *network = nullptr)
        : _legacyNetwork(network)
    {
    }

    FLProgLegacyUnixId unixID;

    void pool() override
    {
        const uint32_t now = millis();
        // The legacy FLProg block calls pool() explicitly, while begin()
        // also attaches this service to the selected interface. Generated
        // schedules therefore invoke the same NTP state machine twice during
        // one quick-scheduler sweep. NTP does not need sub-25 ms polling;
        // coalesce both calls to one safe state-machine advance.
        if (_lastPoolAt != 0xFFFFFFFFUL &&
            static_cast<uint32_t>(now - _lastPoolAt) < 25UL)
        {
            return;
        }
        _lastPoolAt = now;

        // FLProg constructs this object globally, before Setup section has
        // applied STA IP/SSID settings.  Starting NTP in the constructor can
        // therefore create UDP activity against an incompletely configured
        // Wi-Fi stack.  Bind lazily from the generated loop instead.
        if (!_legacyStarted && _legacyNetwork != nullptr)
        {
            _legacyStarted = begin(*_legacyNetwork);
        }
        FLProgNetworkNtp::pool();
        const FLProgNtpDateTime &value = dateTime();
        unixID.timeUNIX = value.unixTime;
        unixID.year = value.year;
        unixID.month = value.month;
        unixID.day = value.day;
        unixID.hours = value.hours;
        unixID.minutes = value.minutes;
        unixID.seconds = value.seconds;
    }

private:
    FLProgNetworkInterface *_legacyNetwork = nullptr;
    bool _legacyStarted = false;
    uint32_t _lastPoolAt = 0xFFFFFFFFUL;
};

using FLProgNtp = FLProgNTP;
