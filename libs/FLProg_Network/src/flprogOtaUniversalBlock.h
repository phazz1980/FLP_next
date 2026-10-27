/*
 * FLProg OTA universal user-block adapter.
 * One generated FBD contract, compile-time selected platform backend.
 */
#pragma once

#include <Arduino.h>
#include <flprogOtaBoardIdentity.h>

#if defined(ARDUINO_ARCH_STM32)
#include <service/flprogStm32OtaFlash.h>
static_assert(VECT_TAB_OFFSET == FlprogOtaFlash::app - FlprogOtaFlash::base,
    "FLProg OTA: wrong linker offset; select the matching OTA board profile");
#endif

#include <flprogNetwork.h>
#include <device/flprogNetworkDeviceRegistry.h>

#if defined(ESP8266) || defined(ESP32)
#define FLPROG_OTA_BLOCK_BACKEND_ESP 1
#include <service/flprogNetworkOta.h>
#elif defined(ARDUINO_ARCH_STM32)
#define FLPROG_OTA_BLOCK_BACKEND_FLE4D 1
#include <service/flprogNetworkOtaStm32Wiznet.h>
#else
#define FLPROG_OTA_BLOCK_BACKEND_UNSUPPORTED 1
#endif

class FLProgOtaUniversalBlock
{
public:
    enum : uint8_t
    {
        STATE_UNSUPPORTED = 0,
        STATE_DISABLED = 1,
        STATE_WAIT_NETWORK = 2,
        STATE_READY = 4,
        STATE_UPDATING = 5,
        STATE_ERROR = 6
    };

    enum : uint8_t
    {
        ERROR_NONE = 0,
        ERROR_UNSUPPORTED = 1,
        ERROR_NETWORK_NOT_FOUND = 2,
        ERROR_CONFIG_LOCKED = 3,
        ERROR_BACKEND = 4
    };

    FLProgOtaUniversalBlock()
        : _state(STATE_DISABLED), _error(ERROR_NONE), _netNum(0),
          _enabled(false), _started(false),
          _runtimeSupported(false),
          _lastRxBytes(0)
    {
        _password[0] = 0;
    }

    bool supported() const
    {
        if (_enabled && _state != STATE_WAIT_NETWORK) return _runtimeSupported;
        return platformAvailable();
    }

    static bool platformAvailable()
    {
#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        return FLProgNetworkOta::platformSupported();
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        return FLPROG_NETWORK_COMPILE_STM32_W5500_OTA != 0;
#else
        return false;
#endif
    }

    void control(bool enable, int32_t netNum, const String &password, const String &version = String())
    {
        if (!platformAvailable())
        {
            _state = STATE_UNSUPPORTED;
            _error = ERROR_UNSUPPORTED;
            _enabled = enable;
            return;
        }

        if (!enable)
        {
            stop();
            _state = STATE_DISABLED;
            _error = ERROR_NONE;
            return;
        }

        if (version.length() > 31)
        {
            _state = STATE_ERROR;
            _error = ERROR_BACKEND;
            return;
        }
#if defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        if (!_started || _version != version)
        {
            _version = version;
            _fle4d.setIdentity(FLPROG_OTA_MODEL, _version.c_str());
        }
#endif
        _enabled = true;
        _runtimeSupported = true;
        if (_started)
        {
            if (_netNum != netNum || _password != password)
            {
                _state = STATE_ERROR;
                _error = ERROR_CONFIG_LOCKED;
                return;
            }
            refreshState();
            return;
        }

        // NET NUM is a byte-sized logical device ID. Reject invalid values
        // before narrowing so negative values and 256+ cannot alias a valid ID.
        if (netNum < 0 || netNum >= (int32_t)FLPROG_NETWORK_DEVICE_INVALID)
        {
            _state = STATE_ERROR;
            _error = ERROR_NETWORK_NOT_FOUND;
            return;
        }

        FLProgNetworkInterface *network = flprogNetworkDevices.getDevice(
            (FLProgNetworkDeviceId)netNum);
        if (network == nullptr)
        {
            _state = STATE_WAIT_NETWORK;
            _error = ERROR_NETWORK_NOT_FOUND;
            return;
        }

#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        // ArduinoOTA is only verified through the ESP Wi-Fi path; it does not
        // route through an arbitrary registered Ethernet interface.
        if (network->networkMode() != FLPROG_NETWORK_MODE_WIFI)
        {
            _runtimeSupported = false;
            _state = STATE_UNSUPPORTED;
            _error = ERROR_UNSUPPORTED;
            return;
        }
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        if (network->networkMode() != FLPROG_NETWORK_MODE_ETHERNET ||
            !network->wiznetMode() || network->rmiiMode())
        {
            _runtimeSupported = false;
            _state = STATE_UNSUPPORTED;
            _error = ERROR_UNSUPPORTED;
            return;
        }
#endif

        _netNum = netNum;
        _password = password;
        if (_password.length() > 63)
        {
            _state = STATE_ERROR;
            _error = ERROR_BACKEND;
            return;
        }

#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        _esp.setPasswordValue(_password.c_str());
        _esp.setRequirePassword(_password.length() > 0);
        _started = _esp.begin(*network);
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        _started = _fle4d.begin(*network, _password.c_str(), 3233);
#endif
        refreshState();
    }

    bool ready() const
    {
#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        return _esp.ready();
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        return _fle4d.ready();
#else
        return false;
#endif
    }

    bool busy() const
    {
#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        return _esp.updating();
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        return _fle4d.updating();
#else
        return false;
#endif
    }

    uint8_t state() const { return _state; }
    uint8_t error() const { return _error; }
    uint32_t receivedBytes() const { return _lastRxBytes; }

private:
    void refreshState()
    {
        if (!_started)
        {
            _state = STATE_ERROR;
            _error = ERROR_BACKEND;
            return;
        }
#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        _state = _esp.state();
        _error = _esp.error() == 0 ? ERROR_NONE : ERROR_BACKEND;
        _lastRxBytes = _esp.progress();
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        _lastRxBytes = _fle4d.bytesReceived();
        _error = _fle4d.error() ? ERROR_BACKEND : ERROR_NONE;
        _state = _fle4d.updating() ? STATE_UPDATING
                : _fle4d.ready() ? STATE_READY
                : _error != ERROR_NONE ? STATE_ERROR : STATE_WAIT_NETWORK;
#endif
    }

    void stop()
    {
#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
        if (_started) _esp.end();
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
        if (_started) _fle4d.end();
#endif
        _started = false;
        _enabled = false;
        _lastRxBytes = 0;
    }

#if defined(FLPROG_OTA_BLOCK_BACKEND_ESP)
    FLProgNetworkOta _esp;
#elif defined(FLPROG_OTA_BLOCK_BACKEND_FLE4D)
    FLProgNetworkOtaStm32Wiznet _fle4d;
#endif
    uint8_t _state;
    uint8_t _error;
    int32_t _netNum;
    String _password;
    String _version;
    bool _enabled;
    bool _started;
    bool _runtimeSupported;
    uint32_t _lastRxBytes;
};

#undef FLPROG_OTA_BLOCK_BACKEND_ESP
#undef FLPROG_OTA_BLOCK_BACKEND_FLE4D
#undef FLPROG_OTA_BLOCK_BACKEND_UNSUPPORTED


