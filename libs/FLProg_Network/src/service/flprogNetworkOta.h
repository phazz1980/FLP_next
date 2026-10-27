/*
 * FLProg_Network — flprogNetworkOta.h
 * Назначение: OTA service, привязанный к состоянию NetworkDevice.
 *
 * Архитектурные заметки:
 * - OTA запускается только при готовой сети и не должен ломать основной pool().
 * - Platform-specific ArduinoOTA детали остаются внутри этого модуля.
 * - Multiple-library warning ArduinoOTA сам по себе не является ошибкой FLProg_Network.
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
#include <stdint.h>
#include "../flprogNetworkProfile.h"
#include "flprogNetworkService.h"

class FLProgNetworkInterface;

#ifndef FLPROG_OTA_MAX_HOSTNAME_LENGTH
#define FLPROG_OTA_MAX_HOSTNAME_LENGTH 63U
#endif
#ifndef FLPROG_OTA_MAX_PASSWORD_LENGTH
#define FLPROG_OTA_MAX_PASSWORD_LENGTH 63U
#endif

#define FLPROG_OTA_STATE_UNSUPPORTED   0
#define FLPROG_OTA_STATE_DISABLED      1
#define FLPROG_OTA_STATE_WAIT_NETWORK  2
#define FLPROG_OTA_STATE_STARTING      3
#define FLPROG_OTA_STATE_READY         4
#define FLPROG_OTA_STATE_UPDATING      5
#define FLPROG_OTA_STATE_ERROR         6

#define FLPROG_OTA_ERROR_NONE              0
#define FLPROG_OTA_ERROR_UNSUPPORTED       1
#define FLPROG_OTA_ERROR_SERVICE_LIMIT     2
#define FLPROG_OTA_ERROR_OWNER_BUSY        3
#define FLPROG_OTA_ERROR_HOSTNAME_TOO_LONG 4
#define FLPROG_OTA_ERROR_PASSWORD_TOO_LONG 5
#define FLPROG_OTA_ERROR_PASSWORD_REQUIRED 6
#define FLPROG_OTA_ERROR_PLATFORM          7
#define FLPROG_OTA_ERROR_RESTART_REQUIRED  8

#define FLPROG_OTA_EVENT_NONE              0x0000U
#define FLPROG_OTA_EVENT_READY             0x0001U
#define FLPROG_OTA_EVENT_STOPPED           0x0002U
#define FLPROG_OTA_EVENT_UPDATE_STARTED    0x0004U
#define FLPROG_OTA_EVENT_UPDATE_FINISHED   0x0008U
#define FLPROG_OTA_EVENT_UPDATE_ERROR      0x0010U
#define FLPROG_OTA_EVENT_NETWORK_LOST           0x0020U
#define FLPROG_OTA_EVENT_NETWORK_RESTORED       0x0040U
#define FLPROG_OTA_EVENT_SECURITY_BLOCKED       0x0080U
#define FLPROG_OTA_EVENT_RESTART_REQUIRED       0x0100U

// ESP8266/ESP32 ArduinoOTA wrapper managed by FLProgNetworkInterface::pool().
// ArduinoOTA is a platform-global singleton and therefore this service is
// intentionally NOT route-aware. Exactly one FLProgNetworkOta object may own
// ArduinoOTA at a time. RP/STM32/generic builds report explicit unsupported.
class FLProgNetworkOta : public FLProgNetworkService
{
public:
    FLProgNetworkOta();
    ~FLProgNetworkOta() override;

    bool begin(FLProgNetworkInterface &network);
    void end();
    void pool() override;

    static bool platformSupported();
    bool supported() const { return platformSupported(); }

    void setEnabled(bool value);
    bool enabled() const { return _enabled; }

    bool setHostname(const char *value);
    bool setHostname(const String &value) { return setHostname(value.c_str()); }
    const char *hostname() const { return _hostname; }

    bool setPasswordValue(const char *value);
    bool setPasswordValue(const String &value) { return setPasswordValue(value.c_str()); }
    const char *password() const { return _password; }
    bool passwordConfigured() const { return _password[0] != 0; }

    void setRequirePassword(bool value);
    bool requirePassword() const { return _requirePassword; }
    bool insecure() const { return !_requirePassword && !passwordConfigured(); }

    // LEGACY API retained for Flprog_Ethernet source compatibility.
    // New code should use setHostname()/setPasswordValue()/setEnabled().
    String getPasssword() const { return String(_password); }
    String getName() const { return String(_hostname); }
    bool getWorkStatus() const { return _enabled; }
    void setPassword(String value) { (void)setPasswordValue(value.c_str()); }
    void setName(String value) { (void)setHostname(value.c_str()); }
    void setWorkStatus(bool value) { setEnabled(value); }

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    bool ready() const { return _state == FLPROG_OTA_STATE_READY; }
    bool updating() const { return _state == FLPROG_OTA_STATE_UPDATING; }
    bool initialized() const { return _initialized; }
    bool owner() const;

    uint16_t events() const { return _events; }
    uint16_t takeEvents()
    {
        const uint16_t value = _events;
        _events = FLPROG_OTA_EVENT_NONE;
        return value;
    }

    uint32_t beginCount() const { return _beginCount; }
    uint32_t endCount() const { return _endCount; }
    uint32_t handleCount() const { return _handleCount; }
    uint32_t configurationRejectCount() const { return _configurationRejectCount; }
    uint32_t networkRestartCount() const { return _networkRestartCount; }
    uint32_t updateStartCount() const { return _updateStartCount; }
    uint32_t updateEndCount() const { return _updateEndCount; }
    uint32_t updateErrorCount() const { return _updateErrorCount; }
    uint8_t lastPlatformError() const { return _lastPlatformError; }
    uint32_t progress() const { return _progress; }
    uint32_t progressTotal() const { return _progressTotal; }

private:
    bool networkAvailable() const;
    bool startPlatform();
    void stopPlatform(bool releaseOwner, bool emitEvent);
    void fail(uint8_t code);

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    static FLProgNetworkOta *_platformOwner;
    static bool _platformConfiguredEver;
    static char _platformLockedHostname[FLPROG_OTA_MAX_HOSTNAME_LENGTH + 1];
    static char _platformLockedPassword[FLPROG_OTA_MAX_PASSWORD_LENGTH + 1];
    static void platformOnStart();
    static void platformOnEnd();
    static void platformOnProgress(unsigned int progress, unsigned int total);
    static void platformOnError(uint8_t error);
#endif

    char _hostname[FLPROG_OTA_MAX_HOSTNAME_LENGTH + 1];
    char _password[FLPROG_OTA_MAX_PASSWORD_LENGTH + 1];
    bool _enabled = true;
    bool _requirePassword = false;
    bool _initialized = false;
    bool _networkWasAvailable = false;
    bool _restartAfterNetworkRestore = false;
    uint8_t _state = FLPROG_OTA_STATE_DISABLED;
    uint8_t _error = FLPROG_OTA_ERROR_NONE;
    uint16_t _events = FLPROG_OTA_EVENT_NONE;
    uint8_t _lastPlatformError = 0;
    uint32_t _progress = 0;
    uint32_t _progressTotal = 0;
    uint32_t _beginCount = 0;
    uint32_t _endCount = 0;
    uint32_t _handleCount = 0;
    uint32_t _configurationRejectCount = 0;
    uint32_t _networkRestartCount = 0;
    uint32_t _updateStartCount = 0;
    uint32_t _updateEndCount = 0;
    uint32_t _updateErrorCount = 0;
};
