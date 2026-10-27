/*
 * FLProg_Network — flprogRmiiEthernet.cpp
 * Назначение: RMII/LAN8720 transport backend.
 *
 * Архитектурные заметки:
 * - Инкапсулирует особенности встроенного Ethernet MAC/PHY конкретной платформы.
 * - RMII должен оставаться отдельным transport от Wiznet даже при общем публичном Ethernet API.
 * - Compile guards обязаны позволять полностью исключить RMII из неподдерживаемых сборок.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Реализация вызывается из cooperative lifecycle и не должна добавлять блокирующие ожидания или скрытые delay().
 * - Смена состояния обязана оставаться согласованной с transport identity, generation и диагностическими счетчиками.
 * - Platform-specific ветки остаются локальными: верхние слои не получают прямой доступ к hardware API.
 * Extended maintenance contract:
 * - The implementation runs in the cooperative lifecycle and must not introduce blocking waits or hidden delay().
 * - State changes must remain consistent with transport identity, generation and diagnostic counters.
 * - Keep platform-specific branches local; upper layers must not access hardware APIs directly.
 */
#include "../flprogNetworkProfile.h"

#if FLPROG_NETWORK_COMPILE_WIRED
#include "flprogRmiiEthernet.h"
#include "../flprogNetworkPlatform.h"

#if defined(ESP32)
    #include "sdkconfig.h"
    #if __has_include(<esp_arduino_version.h>)
        #include <esp_arduino_version.h>
    #endif
    #include <ETH.h>
#endif

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
    #include <STM32Ethernet.h>
    #include <utility/stm32_eth.h>
    #include <stdio.h>
#endif

#ifndef ESP_ARDUINO_VERSION_MAJOR
#define ESP_ARDUINO_VERSION_MAJOR 2
#endif

#if defined(ESP32) && defined(CONFIG_ETH_ENABLED) && CONFIG_ETH_ENABLED && \
    defined(CONFIG_ETH_USE_ESP32_EMAC) && CONFIG_ETH_USE_ESP32_EMAC && \
    (defined(CONFIG_IDF_TARGET_ESP32) || defined(CONFIG_IDF_TARGET_ESP32P4))
#define FLPROG_RMII_RUNTIME_ESP32 1
#else
#define FLPROG_RMII_RUNTIME_ESP32 0
#endif

#if FLPROG_NETWORK_HAS_STM32_ETHERNET
#define FLPROG_RMII_RUNTIME_STM32 1
#else
#define FLPROG_RMII_RUNTIME_STM32 0
#endif

#define FLPROG_RMII_RUNTIME_SUPPORTED \
    (FLPROG_RMII_RUNTIME_ESP32 || FLPROG_RMII_RUNTIME_STM32)

FLProgRmiiEthernet::FLProgRmiiEthernet()
{
#if defined(ESP32) && defined(CONFIG_IDF_TARGET_ESP32P4)
    // Current Arduino-ESP32 P4 Ethernet example defaults.
    _mdcPin = 31;
    _mdioPin = 52;
    _powerPin = 51;
#elif FLPROG_RMII_RUNTIME_STM32
    // STM32 RMII data/MDC/MDIO pins are owned by the Arduino STM32 variant's
    // PinMap_Ethernet. -1 makes it explicit that ESP32 pin defaults do not apply.
    _mdcPin = -1;
    _mdioPin = -1;
    _powerPin = -1;
#endif
}

bool FLProgRmiiEthernet::supported() const
{
    return FLPROG_RMII_RUNTIME_SUPPORTED != 0;
}

void FLProgRmiiEthernet::markRestartRequired()
{
    if (_started || _state != INIT_IDLE)
    {
        _restartRequired = true;
    }
}

void FLProgRmiiEthernet::setPhyAddress(int32_t value)
{
    if (_phyAddress == value) return;
    _phyAddress = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::setMdcPin(int value)
{
    if (_mdcPin == value) return;
    _mdcPin = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::setMdioPin(int value)
{
    if (_mdioPin == value) return;
    _mdioPin = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::setPowerPin(int value)
{
    if (_powerPin == value) return;
    _powerPin = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::setResetPin(int value)
{
    if (_resetPin == value) return;
    _resetPin = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::setClockMode(uint8_t value)
{
    if (value > FLPROG_RMII_CLOCK_GPIO17_OUT)
    {
        value = FLPROG_RMII_CLOCK_GPIO0_IN;
    }
    if (_clockMode == value) return;
    _clockMode = value;
    markRestartRequired();
}

void FLProgRmiiEthernet::dhcpMode(bool value)
{
    if (_dhcp == value) return;
    _dhcp = value;
    _ipConfigDirty = true;
}

uint8_t FLProgRmiiEthernet::begin()
{
    if (!supported())
    {
        _error = FLPROG_RMII_ERROR_UNSUPPORTED;
        return FLPROG_RMII_UNSUPPORTED;
    }

    _startRequested = true;
    _error = FLPROG_RMII_ERROR_NONE;
    return pool();
}

void FLProgRmiiEthernet::end()
{
#if FLPROG_RMII_RUNTIME_ESP32
    #if ESP_ARDUINO_VERSION_MAJOR >= 3
        if (_started)
        {
            ETH.end();
        }
    #endif
#elif FLPROG_RMII_RUNTIME_STM32
    if (_started && _dhcp)
    {
        stm32_set_DHCP_state(DHCP_ASK_RELEASE);
        Ethernet.schedule();
    }
#endif

    _startRequested = false;
    _restartRequired = false;
    _started = false;
    _linkUp = false;
    _ready = false;
    _state = INIT_IDLE;
    _error = FLPROG_RMII_ERROR_NONE;
}

#if FLPROG_RMII_RUNTIME_ESP32
static eth_clock_mode_t flprogRmiiClockMode(uint8_t value)
{
#if defined(CONFIG_IDF_TARGET_ESP32P4)
    // P4 uses the generic EMAC clock enum. The public FLProg clock constants
    // remain API-compatible: *_IN means external 50 MHz; any *_OUT requests
    // EMAC-generated RMII clock. ETH_RMII_CLK selects the actual P4 clock pin.
    return value == FLPROG_RMII_CLOCK_GPIO0_IN
        ? EMAC_CLK_EXT_IN
        : EMAC_CLK_OUT;
#else
    switch (value)
    {
        case FLPROG_RMII_CLOCK_GPIO0_OUT:
            return ETH_CLOCK_GPIO0_OUT;
        case FLPROG_RMII_CLOCK_GPIO16_OUT:
            return ETH_CLOCK_GPIO16_OUT;
        case FLPROG_RMII_CLOCK_GPIO17_OUT:
            return ETH_CLOCK_GPIO17_OUT;
        default:
            return ETH_CLOCK_GPIO0_IN;
    }
#endif
}
#endif

#if FLPROG_RMII_RUNTIME_STM32
static int32_t flprogStm32PhyAddress()
{
#ifdef LAN8742A_PHY_ADDRESS
    return (int32_t)LAN8742A_PHY_ADDRESS;
#else
    // STM32Ethernet owns the external-PHY implementation. Address 0 is its
    // historical default when the core does not expose LAN8742A_PHY_ADDRESS.
    // This address fallback is not a claim that the stock driver supports every
    // RMII PHY; LAN8720 requires a compatible STM32 PHY profile/variant.
    return 0;
#endif
}

static void flprogIpToBytes(IPAddress ip, uint8_t out[4])
{
    out[0] = ip[0];
    out[1] = ip[1];
    out[2] = ip[2];
    out[3] = ip[3];
}
#endif

uint8_t FLProgRmiiEthernet::startDriver()
{
#if FLPROG_RMII_RUNTIME_ESP32
    bool ok = false;

    #if ESP_ARDUINO_VERSION_MAJOR >= 3
        ok = ETH.begin(
            ETH_PHY_LAN8720,
            _phyAddress,
            _mdcPin,
            _mdioPin,
            _powerPin,
            flprogRmiiClockMode(_clockMode)
        );
    #else
        ok = ETH.begin(
            (uint8_t)_phyAddress,
            _powerPin,
            _mdcPin,
            _mdioPin,
            ETH_PHY_LAN8720,
            flprogRmiiClockMode(_clockMode),
            false
        );
    #endif

    if (!ok)
    {
        _error = FLPROG_RMII_ERROR_BEGIN;
        _started = false;
        _state = INIT_IDLE;
        return FLPROG_RMII_ERROR;
    }

    _started = true;
    _state = INIT_RUNNING;

    if (_hostname.length() > 0)
    {
        ETH.setHostname(_hostname.c_str());
    }

    if (!applyIpConfiguration())
    {
        _error = FLPROG_RMII_ERROR_CONFIG;
        return FLPROG_RMII_ERROR;
    }

    return FLPROG_RMII_WAIT;
#elif FLPROG_RMII_RUNTIME_STM32
    // STM32Ethernet fixes the PHY address/type in its low-level PHY profile.
    // Reject a mismatched address instead of pretending the runtime setter can
    // reconfigure a PHY that the installed STM32 core/library does not support.
    if (_phyAddress != flprogStm32PhyAddress())
    {
        _error = FLPROG_RMII_ERROR_CONFIG;
        _state = INIT_IDLE;
        return FLPROG_RMII_ERROR;
    }

    if (!applyIpConfiguration())
    {
        _error = FLPROG_RMII_ERROR_CONFIG;
        _started = false;
        _state = INIT_IDLE;
        return FLPROG_RMII_ERROR;
    }

    _started = stm32_eth_is_init() != 0;
    if (!_started)
    {
        _error = FLPROG_RMII_ERROR_BEGIN;
        _state = INIT_IDLE;
        return FLPROG_RMII_ERROR;
    }

    _state = INIT_RUNNING;
    return FLPROG_RMII_WAIT;
#else
    _error = FLPROG_RMII_ERROR_UNSUPPORTED;
    return FLPROG_RMII_UNSUPPORTED;
#endif
}

bool FLProgRmiiEthernet::applyIpConfiguration()
{
#if FLPROG_RMII_RUNTIME_ESP32
    IPAddress zero(0, 0, 0, 0);

    if (_dhcp)
    {
        if (!ETH.config(zero, zero, zero, zero, zero))
        {
            return false;
        }
    }
    else
    {
        if (_configuredIp == zero)
        {
            return false;
        }

        IPAddress gw = _gateway;
        if (gw == zero)
        {
            gw = _configuredIp;
            gw[3] = 1;
        }

        IPAddress dns1 = _dns;
        if (dns1 == zero)
        {
            dns1 = gw;
        }

        if (!ETH.config(_configuredIp, gw, _subnet, dns1, _dns2))
        {
            return false;
        }
    }

    _ipConfigDirty = false;
    return true;
#elif FLPROG_RMII_RUNTIME_STM32
    IPAddress zero(0, 0, 0, 0);

    if (_dhcp)
    {
        // Direct low-level start keeps DHCP cooperative: unlike
        // Ethernet.begin(), it does not wait synchronously for a lease.
        stm32_eth_init(nullptr, nullptr, nullptr, nullptr);
        if (!stm32_eth_is_init())
        {
            return false;
        }
        stm32_set_DHCP_state(DHCP_START);
    }
    else
    {
        if (_configuredIp == zero)
        {
            return false;
        }

        IPAddress gw = _gateway;
        if (gw == zero)
        {
            gw = _configuredIp;
            gw[3] = 1;
        }

        IPAddress dns1 = _dns;
        if (dns1 == zero)
        {
            dns1 = gw;
        }

        uint8_t ip[4], gateway[4], subnet[4], dns[4];
        flprogIpToBytes(_configuredIp, ip);
        flprogIpToBytes(gw, gateway);
        flprogIpToBytes(_subnet, subnet);
        flprogIpToBytes(dns1, dns);
        stm32_eth_init(nullptr, ip, gateway, subnet);
        if (!stm32_eth_is_init())
        {
            return false;
        }
        stm32_dns_init(dns);
    }

    _ipConfigDirty = false;
    return true;
#else
    return false;
#endif
}

void FLProgRmiiEthernet::updateRuntimeState()
{
#if FLPROG_RMII_RUNTIME_ESP32
    if (!_started)
    {
        _linkUp = false;
        _ready = false;
        return;
    }

    _linkUp = ETH.linkUp();
    _ready = _linkUp && (ETH.localIP() != IPAddress(0, 0, 0, 0));
#elif FLPROG_RMII_RUNTIME_STM32
    if (!_started)
    {
        _linkUp = false;
        _ready = false;
        return;
    }

    Ethernet.schedule();
    _linkUp = stm32_eth_link_up() != 0;
    _ready = _linkUp && (Ethernet.localIP() != IPAddress(0, 0, 0, 0));
#else
    _linkUp = false;
    _ready = false;
#endif
}

uint8_t FLProgRmiiEthernet::pool()
{
    if (!supported())
    {
        _error = FLPROG_RMII_ERROR_UNSUPPORTED;
        _linkUp = false;
        _ready = false;
        return FLPROG_RMII_UNSUPPORTED;
    }

    if (_restartRequired)
    {
        end();
        _restartRequired = false;
        _startRequested = true;
    }

    if (_started && _ipConfigDirty)
    {
        if (!applyIpConfiguration())
        {
            _error = FLPROG_RMII_ERROR_CONFIG;
            return FLPROG_RMII_ERROR;
        }
    }

    if (_startRequested && !_started)
    {
        if (_state == INIT_IDLE)
        {
            if (_resetPin >= 0)
            {
                pinMode(_resetPin, OUTPUT);
                digitalWrite(_resetPin, LOW);
                _state = INIT_RESET_LOW;
                _stateTime = millis();
                return FLPROG_RMII_WAIT;
            }

            _state = INIT_DRIVER;
        }

        if (_state == INIT_RESET_LOW)
        {
            if ((uint32_t)(millis() - _stateTime) < 2)
            {
                return FLPROG_RMII_WAIT;
            }

            digitalWrite(_resetPin, HIGH);
            _state = INIT_RESET_HIGH_WAIT;
            _stateTime = millis();
            return FLPROG_RMII_WAIT;
        }

        if (_state == INIT_RESET_HIGH_WAIT)
        {
            if ((uint32_t)(millis() - _stateTime) < 10)
            {
                return FLPROG_RMII_WAIT;
            }
            _state = INIT_DRIVER;
        }

        if (_state == INIT_DRIVER)
        {
            const uint8_t result = startDriver();
            if (result == FLPROG_RMII_ERROR || result == FLPROG_RMII_UNSUPPORTED)
            {
                return result;
            }
            _startRequested = false;
        }
    }

    updateRuntimeState();

    if (_ready)
    {
        _error = FLPROG_RMII_ERROR_NONE;
        return FLPROG_RMII_READY;
    }

    return FLPROG_RMII_WAIT;
}

IPAddress FLProgRmiiEthernet::currentIP() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? ETH.localIP() : IPAddress(0, 0, 0, 0);
#elif FLPROG_RMII_RUNTIME_STM32
    return _started ? Ethernet.localIP() : IPAddress(0, 0, 0, 0);
#else
    return IPAddress(0, 0, 0, 0);
#endif
}

IPAddress FLProgRmiiEthernet::currentGateway() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? ETH.gatewayIP() : IPAddress(0, 0, 0, 0);
#elif FLPROG_RMII_RUNTIME_STM32
    return _started ? Ethernet.gatewayIP() : IPAddress(0, 0, 0, 0);
#else
    return IPAddress(0, 0, 0, 0);
#endif
}

IPAddress FLProgRmiiEthernet::currentSubnet() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? ETH.subnetMask() : IPAddress(0, 0, 0, 0);
#elif FLPROG_RMII_RUNTIME_STM32
    return _started ? Ethernet.subnetMask() : IPAddress(0, 0, 0, 0);
#else
    return IPAddress(0, 0, 0, 0);
#endif
}

IPAddress FLProgRmiiEthernet::currentDns() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? ETH.dnsIP() : IPAddress(0, 0, 0, 0);
#elif FLPROG_RMII_RUNTIME_STM32
    return _started ? Ethernet.dnsServerIP() : IPAddress(0, 0, 0, 0);
#else
    return IPAddress(0, 0, 0, 0);
#endif
}

String FLProgRmiiEthernet::macAddress() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? ETH.macAddress() : String();
#elif FLPROG_RMII_RUNTIME_STM32
    if (!_started) return String();
    uint8_t mac[6] = {0, 0, 0, 0, 0, 0};
    stm32_eth_get_macaddr(mac);
    char text[18];
    snprintf(
        text,
        sizeof(text),
        "%02X:%02X:%02X:%02X:%02X:%02X",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    return String(text);
#else
    return String();
#endif
}

uint16_t FLProgRmiiEthernet::linkSpeed() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started ? (uint16_t)ETH.linkSpeed() : 0;
#elif FLPROG_RMII_RUNTIME_STM32
    // Current STM32Ethernet public/raw API does not expose negotiated speed.
    return 0;
#else
    return 0;
#endif
}

bool FLProgRmiiEthernet::fullDuplex() const
{
#if FLPROG_RMII_RUNTIME_ESP32
    return _started && ETH.fullDuplex();
#else
    // Current STM32Ethernet public/raw API does not expose negotiated duplex.
    return false;
#endif
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
