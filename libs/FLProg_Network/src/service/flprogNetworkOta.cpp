/*
 * FLProg_Network — flprogNetworkOta.cpp
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
#include "flprogNetworkOta.h"
#include "../flprogNetwork.h"
#include <string.h>

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
#include <ArduinoOTA.h>
FLProgNetworkOta *FLProgNetworkOta::_platformOwner = nullptr;
bool FLProgNetworkOta::_platformConfiguredEver = false;
char FLProgNetworkOta::_platformLockedHostname[FLPROG_OTA_MAX_HOSTNAME_LENGTH + 1] = {0};
char FLProgNetworkOta::_platformLockedPassword[FLPROG_OTA_MAX_PASSWORD_LENGTH + 1] = {0};
#endif

FLProgNetworkOta::FLProgNetworkOta()
{
    _hostname[0] = 0;
    _password[0] = 0;
    _state = platformSupported()
        ? FLPROG_OTA_STATE_DISABLED
        : FLPROG_OTA_STATE_UNSUPPORTED;
    _error = platformSupported()
        ? FLPROG_OTA_ERROR_NONE
        : FLPROG_OTA_ERROR_UNSUPPORTED;
}

FLProgNetworkOta::~FLProgNetworkOta()
{
    end();
}

bool FLProgNetworkOta::platformSupported()
{
#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    return true;
#else
    return false;
#endif
}

bool FLProgNetworkOta::owner() const
{
#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    return _platformOwner == this;
#else
    return false;
#endif
}

bool FLProgNetworkOta::begin(FLProgNetworkInterface &network)
{
    if (!platformSupported())
    {
        _state = FLPROG_OTA_STATE_UNSUPPORTED;
        _error = FLPROG_OTA_ERROR_UNSUPPORTED;
        return false;
    }

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    if (_platformOwner != nullptr && _platformOwner != this)
    {
        _state = FLPROG_OTA_STATE_ERROR;
        _error = FLPROG_OTA_ERROR_OWNER_BUSY;
        return false;
    }

    // ArduinoOTA is a global singleton whose hostname/password storage survives
    // end(). A later service owner must therefore adopt the already-applied
    // configuration until the MCU reboots.
    if (_platformConfiguredEver)
    {
        memcpy(_hostname, _platformLockedHostname, sizeof(_hostname));
        memcpy(_password, _platformLockedPassword, sizeof(_password));
    }
#endif

    if (!attach(network))
    {
        _state = FLPROG_OTA_STATE_ERROR;
        _error = FLPROG_OTA_ERROR_SERVICE_LIMIT;
        return false;
    }

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    _platformOwner = this;
#endif

    _enabled = true;
    _state = FLPROG_OTA_STATE_WAIT_NETWORK;
    _error = FLPROG_OTA_ERROR_NONE;
    _networkWasAvailable = networkAvailable();
    _restartAfterNetworkRestore = false;
    return true;
}

void FLProgNetworkOta::end()
{
    if (_initialized || owner())
    {
        stopPlatform(true, true);
    }
    detach();

    if (platformSupported())
    {
        _state = FLPROG_OTA_STATE_DISABLED;
        _error = FLPROG_OTA_ERROR_NONE;
    }
    else
    {
        _state = FLPROG_OTA_STATE_UNSUPPORTED;
        _error = FLPROG_OTA_ERROR_UNSUPPORTED;
    }
}

void FLProgNetworkOta::setEnabled(bool value)
{
    if (_enabled == value)
    {
        return;
    }

    _enabled = value;
    if (!value)
    {
        _state = platformSupported()
            ? FLPROG_OTA_STATE_DISABLED
            : FLPROG_OTA_STATE_UNSUPPORTED;
        return;
    }

    _state = networkAvailable()
        ? (_initialized ? FLPROG_OTA_STATE_READY : FLPROG_OTA_STATE_STARTING)
        : FLPROG_OTA_STATE_WAIT_NETWORK;
}

bool FLProgNetworkOta::setHostname(const char *value)
{
    if (value == nullptr)
    {
        value = "";
    }

    const size_t length = strlen(value);
    if (length > FLPROG_OTA_MAX_HOSTNAME_LENGTH)
    {
        _error = FLPROG_OTA_ERROR_HOSTNAME_TOO_LONG;
        return false;
    }

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    if (_platformConfiguredEver && strcmp(_platformLockedHostname, value) != 0)
    {
        _error = FLPROG_OTA_ERROR_RESTART_REQUIRED;
        _events |= FLPROG_OTA_EVENT_RESTART_REQUIRED;
        _configurationRejectCount++;
        return false;
    }
#endif

    if (strcmp(_hostname, value) != 0)
    {
        memcpy(_hostname, value, length + 1U);
    }
    _error = FLPROG_OTA_ERROR_NONE;
    return true;
}

bool FLProgNetworkOta::setPasswordValue(const char *value)
{
    if (value == nullptr)
    {
        value = "";
    }

    const size_t length = strlen(value);
    if (length > FLPROG_OTA_MAX_PASSWORD_LENGTH)
    {
        _error = FLPROG_OTA_ERROR_PASSWORD_TOO_LONG;
        return false;
    }

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    if (_platformConfiguredEver && strcmp(_platformLockedPassword, value) != 0)
    {
        _error = FLPROG_OTA_ERROR_RESTART_REQUIRED;
        _events |= FLPROG_OTA_EVENT_RESTART_REQUIRED;
        _configurationRejectCount++;
        return false;
    }
#endif

    if (strcmp(_password, value) != 0)
    {
        memcpy(_password, value, length + 1U);
    }
    _error = FLPROG_OTA_ERROR_NONE;
    return true;
}

void FLProgNetworkOta::setRequirePassword(bool value)
{
    _requirePassword = value;
}

bool FLProgNetworkOta::networkAvailable() const
{
    const FLProgNetworkInterface *network = serviceNetwork();
    if (network == nullptr)
    {
        return false;
    }

    // ESP8266 ArduinoOTA uses WiFiUDP. ESP32 core 3.x uses unified lwIP
    // NetworkUDP/NetworkClient, which also binds through native RMII. Do not use
    // ethernetIsReady(): that includes SPI W5500, which this OTA backend cannot use.
    if (network->wifiIsReady())
    {
        return true;
    }
#if defined(ESP32)
    return network->rmiiIsReady();
#else
    return false;
#endif
}

void FLProgNetworkOta::fail(uint8_t code)
{
    _error = code;
    _state = FLPROG_OTA_STATE_ERROR;
}

bool FLProgNetworkOta::startPlatform()
{
#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    if (_platformOwner != this)
    {
        fail(FLPROG_OTA_ERROR_OWNER_BUSY);
        return false;
    }

    if (_requirePassword && _password[0] == 0)
    {
        _events |= FLPROG_OTA_EVENT_SECURITY_BLOCKED;
        fail(FLPROG_OTA_ERROR_PASSWORD_REQUIRED);
        return false;
    }

    _state = FLPROG_OTA_STATE_STARTING;

    // Hostname/password are set only before the first platform begin. Both
    // ESP8266 and ESP32 ArduinoOTA keep hostname in the singleton after end();
    // ESP8266 also keeps the password. Treat them as boot-time configuration.
    if (!_platformConfiguredEver)
    {
        if (_hostname[0] != 0)
        {
            ArduinoOTA.setHostname(_hostname);
        }
        if (_password[0] != 0)
        {
            ArduinoOTA.setPassword(_password);
        }
        memcpy(_platformLockedHostname, _hostname, sizeof(_hostname));
        memcpy(_platformLockedPassword, _password, sizeof(_password));
        _platformConfiguredEver = true;
    }

    ArduinoOTA.onStart([]() { FLProgNetworkOta::platformOnStart(); });
    ArduinoOTA.onEnd([]() { FLProgNetworkOta::platformOnEnd(); });
    ArduinoOTA.onProgress([](unsigned int progress, unsigned int total) {
        FLProgNetworkOta::platformOnProgress(progress, total);
    });
    ArduinoOTA.onError([](ota_error_t error) {
        FLProgNetworkOta::platformOnError((uint8_t)error);
    });

    ArduinoOTA.begin();
    _initialized = true;
    _beginCount++;
    _error = FLPROG_OTA_ERROR_NONE;
    _state = FLPROG_OTA_STATE_READY;
    _events |= FLPROG_OTA_EVENT_READY;
    return true;
#else
    fail(FLPROG_OTA_ERROR_UNSUPPORTED);
    return false;
#endif
}

void FLProgNetworkOta::stopPlatform(bool releaseOwner, bool emitEvent)
{
#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    if (_initialized && _platformOwner == this)
    {
        ArduinoOTA.end();
        _initialized = false;
        _endCount++;
        if (emitEvent)
        {
            _events |= FLPROG_OTA_EVENT_STOPPED;
        }
    }

    if (releaseOwner && _platformOwner == this)
    {
        _platformOwner = nullptr;
    }
#else
    (void)releaseOwner;
    (void)emitEvent;
#endif
}

void FLProgNetworkOta::pool()
{
    if (!platformSupported())
    {
        _state = FLPROG_OTA_STATE_UNSUPPORTED;
        _error = FLPROG_OTA_ERROR_UNSUPPORTED;
        return;
    }

    if (!owner())
    {
        fail(FLPROG_OTA_ERROR_OWNER_BUSY);
        return;
    }

    const bool networkReady = networkAvailable();

    if (_networkWasAvailable && !networkReady)
    {
        _events |= FLPROG_OTA_EVENT_NETWORK_LOST;
        if (_initialized)
        {
            _restartAfterNetworkRestore = true;
        }
    }
    else if (!_networkWasAvailable && networkReady)
    {
        _events |= FLPROG_OTA_EVENT_NETWORK_RESTORED;
    }
    _networkWasAvailable = networkReady;

    if (!_enabled)
    {
        _state = FLPROG_OTA_STATE_DISABLED;
        return;
    }

    if (_requirePassword && _password[0] == 0)
    {
        if (_initialized)
        {
            stopPlatform(false, true);
        }
        _events |= FLPROG_OTA_EVENT_SECURITY_BLOCKED;
        fail(FLPROG_OTA_ERROR_PASSWORD_REQUIRED);
        return;
    }

    if (!networkReady)
    {
        _state = FLPROG_OTA_STATE_WAIT_NETWORK;
        return;
    }

    if (_restartAfterNetworkRestore)
    {
        if (_initialized)
        {
            stopPlatform(false, false);
        }
        _restartAfterNetworkRestore = false;
        if (startPlatform())
        {
            _networkRestartCount++;
        }
        return;
    }

    if (!_initialized)
    {
        (void)startPlatform();
        return;
    }

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
    ArduinoOTA.handle();
    _handleCount++;
    if (_state != FLPROG_OTA_STATE_UPDATING)
    {
        _state = FLPROG_OTA_STATE_READY;
    }
#endif
}

#if FLPROG_NETWORK_COMPILE_OTA && (defined(ESP8266) || defined(ESP32))
void FLProgNetworkOta::platformOnStart()
{
    if (_platformOwner == nullptr)
    {
        return;
    }

    _platformOwner->_state = FLPROG_OTA_STATE_UPDATING;
    _platformOwner->_error = FLPROG_OTA_ERROR_NONE;
    _platformOwner->_progress = 0;
    _platformOwner->_progressTotal = 0;
    _platformOwner->_updateStartCount++;
    _platformOwner->_events |= FLPROG_OTA_EVENT_UPDATE_STARTED;
}

void FLProgNetworkOta::platformOnEnd()
{
    if (_platformOwner == nullptr)
    {
        return;
    }

    _platformOwner->_updateEndCount++;
    _platformOwner->_events |= FLPROG_OTA_EVENT_UPDATE_FINISHED;
    _platformOwner->_state = FLPROG_OTA_STATE_READY;
}

void FLProgNetworkOta::platformOnProgress(unsigned int progressValue, unsigned int totalValue)
{
    if (_platformOwner == nullptr)
    {
        return;
    }

    _platformOwner->_progress = progressValue;
    _platformOwner->_progressTotal = totalValue;
}

void FLProgNetworkOta::platformOnError(uint8_t errorValue)
{
    if (_platformOwner == nullptr)
    {
        return;
    }

    _platformOwner->_lastPlatformError = errorValue;
    _platformOwner->_error = FLPROG_OTA_ERROR_PLATFORM;
    _platformOwner->_updateErrorCount++;
    _platformOwner->_events |= FLPROG_OTA_EVENT_UPDATE_ERROR;
    _platformOwner->_state = FLPROG_OTA_STATE_READY;
}
#endif
