/*
 * FLProg_Network — flprogRmiiEthernet.h
 * Назначение: RMII/LAN8720 transport backend.
 *
 * Архитектурные заметки:
 * - Инкапсулирует особенности встроенного Ethernet MAC/PHY конкретной платформы.
 * - RMII должен оставаться отдельным transport от Wiznet даже при общем публичном Ethernet API.
 * - Compile guards обязаны позволять полностью исключить RMII из неподдерживаемых сборок.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
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

#include <Arduino.h>
#include <IPAddress.h>

// -----------------------------------------------------------------------------
// FLProg_Network v1.0.0-rc.4 - native MAC / RMII backend.
//
// LAN8720 is an Ethernet PHY, not a hardwired TCP/IP controller. A direct RMII
// connection therefore requires an MCU with an internal Ethernet MAC. rc.2 uses
// LAN8720 through the native EMAC API on supported ESP32 targets. On STM32 it
// provides the native MAC/RMII transport through STM32Ethernet/raw lwIP; the
// exact external PHY (LAN8742A, LAN8720, etc.) is controlled by the STM32
// core/variant/PHY profile. The stock STM32Ethernet release is not treated as a
// universal LAN8720 driver. Controllers without an internal MAC use Wiznet SPI.
// -----------------------------------------------------------------------------

#define FLPROG_RMII_PHY_LAN8720 1

#define FLPROG_RMII_CLOCK_GPIO0_IN   0
#define FLPROG_RMII_CLOCK_GPIO0_OUT  1
#define FLPROG_RMII_CLOCK_GPIO16_OUT 2
#define FLPROG_RMII_CLOCK_GPIO17_OUT 3

#define FLPROG_RMII_WAIT        0
#define FLPROG_RMII_READY       1
#define FLPROG_RMII_ERROR       2
#define FLPROG_RMII_UNSUPPORTED 3

#define FLPROG_RMII_ERROR_NONE        0
#define FLPROG_RMII_ERROR_UNSUPPORTED 1
#define FLPROG_RMII_ERROR_BEGIN       2
#define FLPROG_RMII_ERROR_CONFIG      3

class FLProgRmiiEthernet
{
public:
    FLProgRmiiEthernet();

    // Capability --------------------------------------------------------------
    bool supported() const;

    // LAN8720 / RMII hardware configuration ----------------------------------
    void setPhyAddress(int32_t value);
    int32_t phyAddress() const { return _phyAddress; }

    void setMdcPin(int value);
    int mdcPin() const { return _mdcPin; }

    void setMdioPin(int value);
    int mdioPin() const { return _mdioPin; }

    // Optional PHY power/enable pin. Applied by ESP32; STM32 variants own RMII pins.
    void setPowerPin(int value);
    int powerPin() const { return _powerPin; }

    // Optional dedicated reset pin. When used, reset is pulsed by the library
    // without delay(); the state machine advances from pool().
    void setResetPin(int value);
    int resetPin() const { return _resetPin; }

    void setClockMode(uint8_t value);
    uint8_t clockMode() const { return _clockMode; }

    void setHostname(const String &value) { _hostname = value; }
    String hostname() const { return _hostname; }

    // IP configuration --------------------------------------------------------
    void dhcpMode(bool value);
    bool isDhcp() const { return _dhcp; }

    void localIP(IPAddress value) { _configuredIp = value; _ipConfigDirty = true; }
    void gateway(IPAddress value) { _gateway = value; _ipConfigDirty = true; }
    void subnet(IPAddress value) { _subnet = value; _ipConfigDirty = true; }
    void dns(IPAddress value) { _dns = value; _ipConfigDirty = true; }
    void dns2(IPAddress value) { _dns2 = value; _ipConfigDirty = true; }

    IPAddress configuredIP() const { return _configuredIp; }
    IPAddress configuredGateway() const { return _gateway; }
    IPAddress configuredSubnet() const { return _subnet; }
    IPAddress configuredDns() const { return _dns; }
    IPAddress configuredDns2() const { return _dns2; }

    // Lifecycle ---------------------------------------------------------------
    // begin() only requests startup. pool() performs reset/start/status work.
    uint8_t begin();
    void end();
    uint8_t pool();

    bool started() const { return _started; }
    bool linkUp() const { return _linkUp; }
    bool isReady() const { return _ready; }
    uint8_t error() const { return _error; }

    // Runtime information -----------------------------------------------------
    IPAddress currentIP() const;
    IPAddress currentGateway() const;
    IPAddress currentSubnet() const;
    IPAddress currentDns() const;
    String macAddress() const;
    uint16_t linkSpeed() const;
    bool fullDuplex() const;

private:
    enum InitState : uint8_t
    {
        INIT_IDLE = 0,
        INIT_RESET_LOW = 1,
        INIT_RESET_HIGH_WAIT = 2,
        INIT_DRIVER = 3,
        INIT_RUNNING = 4
    };

    void markRestartRequired();
    uint8_t startDriver();
    bool applyIpConfiguration();
    void updateRuntimeState();

    int32_t _phyAddress = 0;
    int _mdcPin = 23;
    int _mdioPin = 18;
    int _powerPin = -1;
    int _resetPin = -1;
    uint8_t _clockMode = FLPROG_RMII_CLOCK_GPIO0_IN;
    String _hostname;

    bool _dhcp = true;
    IPAddress _configuredIp = IPAddress(0, 0, 0, 0);
    IPAddress _gateway = IPAddress(0, 0, 0, 0);
    IPAddress _subnet = IPAddress(255, 255, 255, 0);
    IPAddress _dns = IPAddress(0, 0, 0, 0);
    IPAddress _dns2 = IPAddress(0, 0, 0, 0);

    bool _startRequested = false;
    bool _restartRequired = false;
    bool _ipConfigDirty = false;
    bool _started = false;
    bool _linkUp = false;
    bool _ready = false;
    uint8_t _error = FLPROG_RMII_ERROR_NONE;

    InitState _state = INIT_IDLE;
    uint32_t _stateTime = 0;
};
