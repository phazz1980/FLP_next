/*
 * FLProg_Network — flprogNetwork.h
 * Назначение: Главный публичный интерфейс сети FLProg_Network.
 *
 * Архитектурные заметки:
 * - Объединяет Wi-Fi, Wiznet/W5500 и RMII за единым API.
 * - Содержит runtime-состояния интерфейса, конфигурацию, readiness/error и точки доступа к A3.
 * - Не переносите platform-specific детали выше этого слоя: они должны оставаться в backend/traits-подобных границах.
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

#include "flprogNetworkProfile.h"

#include <Arduino.h>
#include <IPAddress.h>
#include "flprogNetworkPlatform.h"
#include "device/flprogNetworkDeviceRegistry.h"
#include "hardware/flprogWiznetHardware.h"
#include "hardware/flprogRmiiEthernet.h"
#include "transport/flprogNetworkTransport.h"
#include "transport/flprogNetworkOpticalStub.h"
#include "socket/flprogNetworkSocket.h"
#include "socket/flprogSocketBackends.h"
#include "protocol/flprogNetworkUdp.h"
#include "protocol/flprogNetworkDhcp.h"
#include "protocol/flprogNetworkMacRaw.h"
#include "service/flprogNetworkService.h"
#include "protocol/flprogNetworkDns.h"
#include "service/flprogNetworkNtp.h"
#include "service/flprogNetworkHttpClient.h"
#include "service/flprogNetworkHttpServer.h"
#include "service/flprogNetworkMqttClient.h"
#include "service/flprogNetworkMqttBlockBridge.h"
#include "service/flprogNetworkOta.h"
#include "service/flprogNetworkOtaWiznet.h"
#include "service/flprogNetworkOtaStm32Wiznet.h"
#include "protocol/flprogNetworkTcpClient.h"
#include "protocol/flprogNetworkTcpServer.h"


// Public version/API markers.
// RC6-A dev.16-fix4 freezes the verified high-level legacy facade after
// API Compatibility Gate 0.6 PASS. The compatibility level remains 10006:
// freezing the existing contract does not create a new incompatible API level.
#define FLPROG_NETWORK_VERSION_MAJOR 1
#define FLPROG_NETWORK_VERSION_MINOR 0
#define FLPROG_NETWORK_VERSION_PATCH 0
#define FLPROG_NETWORK_VERSION_DEV   16
#define FLPROG_NETWORK_VERSION_STRING "1.0.0-rc.6a.dev.17-pr28"
#define FLPROG_NETWORK_API_LEVEL 10006UL
#define FLPROG_NETWORK_HIGH_LEVEL_API_FROZEN 1

// W5500 compile-gate profile: wired implementation is enabled.
// This branch exists only to verify W5500/Wiznet compilation and linking.
// No physical Ethernet hardware is required for the compile-only gate.

// -----------------------------------------------------------------------------
// FLProg_Network 1.0.0-rc.6a.dev.2-fix2 — interface-lifecycle isolation fix
// Standalone library. It does NOT include or depend on FLProg_Ethernet.
// Only the public API naming style was derived from FLProg_Ethernet.
// The implementation, state machine and transport handling are standalone.
//
// Design rule for FLProg user blocks:
//   SetupSection -> bind inputs/outputs once.
//   LoopSection  -> call only flpNetwork.pool();
// -----------------------------------------------------------------------------

#ifndef FLPROG_NETWORK_MODE_WIFI
#define FLPROG_NETWORK_MODE_WIFI 0
#endif
#ifndef FLPROG_NETWORK_MODE_ETHERNET
#define FLPROG_NETWORK_MODE_ETHERNET 1
#endif
#ifndef FLPROG_NETWORK_MODE_AUTO
#define FLPROG_NETWORK_MODE_AUTO 2
#endif
#ifndef FLPROG_NETWORK_MODE_BOTH
#define FLPROG_NETWORK_MODE_BOTH 3
#endif

#define FLPROG_NETWORK_HAS_ACTIVE_IP_INFO 1
#define FLPROG_NETWORK_HAS_DEVICE_IDENTITY 1

#ifndef FLPROG_NETWORK_ACTIVE_NONE
#define FLPROG_NETWORK_ACTIVE_NONE 0
#endif
#ifndef FLPROG_NETWORK_ACTIVE_WIFI
#define FLPROG_NETWORK_ACTIVE_WIFI 1
#endif
#ifndef FLPROG_NETWORK_ACTIVE_ETHERNET
#define FLPROG_NETWORK_ACTIVE_ETHERNET 2
#endif
#ifndef FLPROG_NETWORK_ACTIVE_BOTH
#define FLPROG_NETWORK_ACTIVE_BOTH 3
#endif
#ifndef FLPROG_NETWORK_ACTIVE_RMII
#define FLPROG_NETWORK_ACTIVE_RMII 4
#endif
#ifndef FLPROG_NETWORK_ACTIVE_MULTI
#define FLPROG_NETWORK_ACTIVE_MULTI 5
#endif
#ifndef FLPROG_NETWORK_ACTIVE_WIZNET
#define FLPROG_NETWORK_ACTIVE_WIZNET FLPROG_NETWORK_ACTIVE_ETHERNET
#endif
#ifndef FLPROG_NETWORK_ACTIVE_WIFI_AP
#define FLPROG_NETWORK_ACTIVE_WIFI_AP 6
#endif

#ifndef FLPROG_NETWORK_ROUTE_DEFAULT
#define FLPROG_NETWORK_ROUTE_DEFAULT 0
#endif
#ifndef FLPROG_NETWORK_ROUTE_WIFI
#define FLPROG_NETWORK_ROUTE_WIFI 1
#endif
#ifndef FLPROG_NETWORK_ROUTE_ETHERNET
#define FLPROG_NETWORK_ROUTE_ETHERNET 2
#endif
#ifndef FLPROG_NETWORK_ROUTE_AUTO
#define FLPROG_NETWORK_ROUTE_AUTO 3
#endif

#ifndef FLPROG_NETWORK_MAX_MANAGED_SOCKETS
#define FLPROG_NETWORK_MAX_MANAGED_SOCKETS 16
#endif
#ifndef FLPROG_NETWORK_MAX_MANAGED_SERVICES
#define FLPROG_NETWORK_MAX_MANAGED_SERVICES 8
#endif
#ifndef FLPROG_NETWORK_ROUTE_WIZNET
#define FLPROG_NETWORK_ROUTE_WIZNET 4
#endif
#ifndef FLPROG_NETWORK_ROUTE_RMII
#define FLPROG_NETWORK_ROUTE_RMII 5
#endif
#ifndef FLPROG_NETWORK_ROUTE_WIFI_AP
#define FLPROG_NETWORK_ROUTE_WIFI_AP 6
#endif


#ifndef FLPROG_WIFI_EVENT_NONE
#define FLPROG_WIFI_EVENT_NONE 0x0000U
#endif
#ifndef FLPROG_WIFI_EVENT_STA_UP
#define FLPROG_WIFI_EVENT_STA_UP 0x0001U
#endif
#ifndef FLPROG_WIFI_EVENT_STA_DOWN
#define FLPROG_WIFI_EVENT_STA_DOWN 0x0002U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_STARTED
#define FLPROG_WIFI_EVENT_AP_STARTED 0x0004U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_STOPPED
#define FLPROG_WIFI_EVENT_AP_STOPPED 0x0008U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_CLIENT_JOINED
#define FLPROG_WIFI_EVENT_AP_CLIENT_JOINED 0x0010U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_CLIENT_LEFT
#define FLPROG_WIFI_EVENT_AP_CLIENT_LEFT 0x0020U
#endif
#ifndef FLPROG_WIFI_EVENT_SCAN_DONE
#define FLPROG_WIFI_EVENT_SCAN_DONE 0x0040U
#endif
#ifndef FLPROG_WIFI_EVENT_SCAN_FAILED
#define FLPROG_WIFI_EVENT_SCAN_FAILED 0x0080U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_RESTARTED
#define FLPROG_WIFI_EVENT_AP_RESTARTED 0x0100U
#endif
#ifndef FLPROG_WIFI_EVENT_AP_FAILED
#define FLPROG_WIFI_EVENT_AP_FAILED 0x0200U
#endif
#ifndef FLPROG_WIFI_EVENT_SCAN_CANCELED
#define FLPROG_WIFI_EVENT_SCAN_CANCELED 0x0400U
#endif

#ifndef FLPROG_WIFI_SCAN_IDLE
#define FLPROG_WIFI_SCAN_IDLE 0
#endif
#ifndef FLPROG_WIFI_SCAN_RUNNING
#define FLPROG_WIFI_SCAN_RUNNING 1
#endif
#ifndef FLPROG_WIFI_SCAN_DONE
#define FLPROG_WIFI_SCAN_DONE 2
#endif
#ifndef FLPROG_WIFI_SCAN_ERROR
#define FLPROG_WIFI_SCAN_ERROR 3
#endif
#ifndef FLPROG_WIFI_SCAN_QUEUED
#define FLPROG_WIFI_SCAN_QUEUED 4
#endif
#ifndef FLPROG_WIFI_SCAN_CANCELED
#define FLPROG_WIFI_SCAN_CANCELED 5
#endif

#ifndef FLPROG_WIFI_SCAN_ERROR_NONE
#define FLPROG_WIFI_SCAN_ERROR_NONE 0
#endif
#ifndef FLPROG_WIFI_SCAN_ERROR_START
#define FLPROG_WIFI_SCAN_ERROR_START 1
#endif
#ifndef FLPROG_WIFI_SCAN_ERROR_RUNTIME
#define FLPROG_WIFI_SCAN_ERROR_RUNTIME 2
#endif
#ifndef FLPROG_WIFI_SCAN_ERROR_TIMEOUT
#define FLPROG_WIFI_SCAN_ERROR_TIMEOUT 3
#endif

#ifndef FLPROG_WIFI_AP_ERROR_NONE
#define FLPROG_WIFI_AP_ERROR_NONE 0
#endif
#ifndef FLPROG_WIFI_AP_ERROR_INVALID_SSID
#define FLPROG_WIFI_AP_ERROR_INVALID_SSID 1
#endif
#ifndef FLPROG_WIFI_AP_ERROR_INVALID_PASSWORD
#define FLPROG_WIFI_AP_ERROR_INVALID_PASSWORD 2
#endif
#ifndef FLPROG_WIFI_AP_ERROR_CONFIG_FAILED
#define FLPROG_WIFI_AP_ERROR_CONFIG_FAILED 3
#endif
#ifndef FLPROG_WIFI_AP_ERROR_START_FAILED
#define FLPROG_WIFI_AP_ERROR_START_FAILED 4
#endif

#ifndef FLPROG_WIFI_SCAN_TIMEOUT_MS
#define FLPROG_WIFI_SCAN_TIMEOUT_MS 30000UL
#endif

#ifndef FLPROG_NETWORK_WIRED_PRIORITY_WIZNET
#define FLPROG_NETWORK_WIRED_PRIORITY_WIZNET 0
#endif
#ifndef FLPROG_NETWORK_WIRED_PRIORITY_RMII
#define FLPROG_NETWORK_WIRED_PRIORITY_RMII 1
#endif

#ifndef FLPROG_NETWORK_DISABLED_STATUS
#define FLPROG_NETWORK_DISABLED_STATUS 0
#endif
#ifndef FLPROG_NETWORK_IDLE_STATUS
#define FLPROG_NETWORK_IDLE_STATUS 1
#endif
#ifndef FLPROG_NETWORK_CONNECTING_STATUS
#define FLPROG_NETWORK_CONNECTING_STATUS 2
#endif
#ifndef FLPROG_NETWORK_READY_STATUS
#define FLPROG_NETWORK_READY_STATUS 3
#endif
#ifndef FLPROG_NETWORK_RECONNECT_STATUS
#define FLPROG_NETWORK_RECONNECT_STATUS 4
#endif
#ifndef FLPROG_NETWORK_ERROR_STATUS
#define FLPROG_NETWORK_ERROR_STATUS 5
#endif

#ifndef FLPROG_NETWORK_NOT_ERROR
#define FLPROG_NETWORK_NOT_ERROR 0
#endif
#ifndef FLPROG_NETWORK_WIFI_TIMEOUT_ERROR
#define FLPROG_NETWORK_WIFI_TIMEOUT_ERROR 1
#endif
#ifndef FLPROG_NETWORK_WIFI_SSID_ERROR
#define FLPROG_NETWORK_WIFI_SSID_ERROR 2
#endif
#ifndef FLPROG_NETWORK_ETHERNET_HARDWARE_ERROR
#define FLPROG_NETWORK_ETHERNET_HARDWARE_ERROR 3
#endif
#ifndef FLPROG_NETWORK_ETHERNET_LINK_ERROR
#define FLPROG_NETWORK_ETHERNET_LINK_ERROR 4
#endif
#ifndef FLPROG_NETWORK_ETHERNET_DHCP_ERROR
#define FLPROG_NETWORK_ETHERNET_DHCP_ERROR 5
#endif

#ifndef FLPROG_NETWORK_ETHERNET_SPI_BUS_ERROR
#define FLPROG_NETWORK_ETHERNET_SPI_BUS_ERROR 6
#endif
#ifndef FLPROG_NETWORK_RMII_UNSUPPORTED_ERROR
#define FLPROG_NETWORK_RMII_UNSUPPORTED_ERROR 7
#endif
#ifndef FLPROG_NETWORK_RMII_BEGIN_ERROR
#define FLPROG_NETWORK_RMII_BEGIN_ERROR 8
#endif
#ifndef FLPROG_NETWORK_RMII_CONFIG_ERROR
#define FLPROG_NETWORK_RMII_CONFIG_ERROR 9
#endif

#ifndef FLPROG_NETWORK_SUCCESS
#define FLPROG_NETWORK_SUCCESS 0
#endif
#ifndef FLPROG_NETWORK_WAIT
#define FLPROG_NETWORK_WAIT 1
#endif
#ifndef FLPROG_NETWORK_ERROR
#define FLPROG_NETWORK_ERROR 2
#endif


struct FLProgNetworkCapacitySnapshot
{
    uint8_t logicalLeaseLimit = FLPROG_SOCKET_MAX_LEASES;
    uint8_t logicalLeasesUsed = 0;
    uint8_t logicalLeasesFree = FLPROG_SOCKET_MAX_LEASES;

    uint8_t managedSocketLimit = FLPROG_NETWORK_MAX_MANAGED_SOCKETS;
    uint8_t managedSocketsUsed = 0;
    uint8_t managedSocketsFree = FLPROG_NETWORK_MAX_MANAGED_SOCKETS;

    uint8_t managedServiceLimit = FLPROG_NETWORK_MAX_MANAGED_SERVICES;
    uint8_t managedServicesUsed = 0;
    uint8_t managedServicesFree = FLPROG_NETWORK_MAX_MANAGED_SERVICES;

    uint8_t wiznetSocketLimit = 0;
    uint8_t wiznetSocketsReserved = 0;
    uint8_t wiznetSocketsFree = 0;

    bool dnsResolverPresent = false;
    bool dnsResolverBusy = false;
    uint8_t dhcpState = 0;
    bool strictAcdActive = false;
    bool strictAcdSocketReserved = false;
};

class FLProgNetworkInterface
{
public:
    // Legacy service facades constructed without an interface (notably
    // FLProgOTA) use this only for lazy compatibility binding. It returns the
    // first live interface whose Wi-Fi STA is ready, or nullptr.
    // Native code must bind services explicitly with begin(network).
    static FLProgNetworkInterface *legacyDefaultWifiInterface();
    static const char *versionString() { return FLPROG_NETWORK_VERSION_STRING; }
    static uint32_t apiLevel() { return FLPROG_NETWORK_API_LEVEL; }
    static bool highLevelApiFrozen()
    {
        return FLPROG_NETWORK_HIGH_LEVEL_API_FROZEN != 0;
    }

    // Compile-time backend capability report for the current architecture.
    // This does not claim official-core or physical-hardware verification.
    static FLProgNetworkPlatformCapabilities platformCapabilities()
    {
        return FLProgNetworkPlatformCapabilities();
    }

    static const char *platformName()
    {
#if FLPROG_NETWORK_PLATFORM_ID == FLPROG_NETWORK_PLATFORM_ESP8266
        return "ESP8266";
#elif FLPROG_NETWORK_PLATFORM_ID == FLPROG_NETWORK_PLATFORM_ESP32
        return "ESP32";
#elif FLPROG_NETWORK_PLATFORM_ID == FLPROG_NETWORK_PLATFORM_RP
        return "RP2040/RP2350";
#elif FLPROG_NETWORK_PLATFORM_ID == FLPROG_NETWORK_PLATFORM_STM32
        return "STM32";
#else
        return "GENERIC";
#endif
    }

    static bool builtInTcpForTransport(uint8_t transportId)
    {
        if (transportId == FLPROG_TRANSPORT_WIZNET) return true;
        if (transportId == FLPROG_TRANSPORT_WIFI)
            return FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP;
        if (transportId == FLPROG_TRANSPORT_WIFI_AP)
            return (FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP) &&
                   (FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0);
        if (transportId == FLPROG_TRANSPORT_RMII)
            return (FLPROG_NETWORK_COMPILED_RMII != 0) &&
                   ((FLPROG_NETWORK_HAS_POSIX_TCP != 0) ||
                    (FLPROG_NETWORK_HAS_STM32_ETHERNET != 0));
        return false;
    }

    static bool builtInUdpForTransport(uint8_t transportId)
    {
        if (transportId == FLPROG_TRANSPORT_WIZNET) return true;
        if (transportId == FLPROG_TRANSPORT_WIFI)
            return FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP;
        if (transportId == FLPROG_TRANSPORT_WIFI_AP)
            return (FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP) &&
                   (FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0);
        if (transportId == FLPROG_TRANSPORT_RMII)
            return (FLPROG_NETWORK_COMPILED_RMII != 0) &&
                   ((FLPROG_NETWORK_HAS_POSIX_UDP != 0) ||
                    (FLPROG_NETWORK_HAS_STM32_ETHERNET != 0));
        return false;
    }
    FLProgNetworkInterface();
    FLProgNetworkInterface(int pinCs, uint8_t bus = 255);
    ~FLProgNetworkInterface();

    // Wi-Fi client API style compatible with FLProg_Ethernet naming.
    /**
     * Set the Wi-Fi STA SSID.
     * If the value changes while Wi-Fi is enabled, a new connection attempt
     * is scheduled by the library.
     */
    void setClientSsid(String ssid);
    /** Set the Wi-Fi STA password and schedule reconnect when required. */
    void setClientPassword(String password);
    String clientSsid() const { return _clientSsid; }
    String clientPassword() const { return _clientPassword; }

    /** Allow the Wi-Fi client interface and schedule connection if needed. */
    void clientOn();
    /** Disable the Wi-Fi client interface and clear Wi-Fi readiness state. */
    void clientOff();
    void clientMode(bool value);
    bool clientMode() const { return _wifiAllowed; }
    bool clientIsReady() const { return _wifiReady; }

    void setAutoConnect(bool value) { _autoConnect = value; }
    void setAutoReconnect(bool value) { _autoReconnect = value; }
    bool getAutoConnect() const { return _autoConnect; }
    bool getAutoReconnect() const { return _autoReconnect; }


    // Wi-Fi SoftAP / STA+AP API (ESP8266/ESP32).
    void setAccessPointSsid(String ssid);
    void setAccessPointPassword(String password);
    void setAccessPointChannel(uint8_t channel);
    void setAccessPointHidden(bool value);
    void setAccessPointMaxClients(uint8_t value);
    void setAccessPointIP(IPAddress value);
    void setAccessPointGateway(IPAddress value);
    void setAccessPointSubnet(IPAddress value);

    // Desired AP configuration. These getters report the requested values,
    // which may differ from the last successfully applied live configuration
    // while a cooperative restart is pending or blocked by validation.
    String accessPointSsid() const { return _apSsid; }
    String accessPointPassword() const { return _apPassword; }
    uint8_t accessPointChannel() const { return _apChannel; }
    bool accessPointHidden() const { return _apHidden; }
    uint8_t accessPointMaxClients() const { return _apMaxClients; }
    IPAddress accessPointConfiguredIP() const { return _apIp; }
    IPAddress accessPointConfiguredGateway() const { return _apGateway; }
    IPAddress accessPointConfiguredSubnet() const { return _apSubnet; }
    static uint8_t accessPointMaxClientsLimit()
    {
        return FLPROG_NETWORK_WIFI_AP_MAX_CLIENTS;
    }

    // Snapshot of the last AP configuration successfully applied by the
    // library. The password is intentionally not duplicated in this snapshot.
    bool accessPointAppliedConfigurationValid() const { return _apAppliedConfigValid; }
    bool accessPointConfigurationPending() const
    {
        return _apAllowed &&
               (!_apAppliedConfigValid ||
                _apConfigGeneration != _apAppliedConfigGeneration ||
                _apStartRequest);
    }
    uint32_t accessPointConfigurationGeneration() const { return _apConfigGeneration; }
    uint32_t accessPointAppliedConfigurationGeneration() const { return _apAppliedConfigGeneration; }
    String appliedAccessPointSsid() const { return _apAppliedSsid; }
    uint8_t appliedAccessPointChannel() const { return _apAppliedChannel; }
    bool appliedAccessPointHidden() const { return _apAppliedHidden; }
    uint8_t appliedAccessPointMaxClients() const { return _apAppliedMaxClients; }
    IPAddress appliedAccessPointIP() const { return _apAppliedIp; }
    IPAddress appliedAccessPointGateway() const { return _apAppliedGateway; }
    IPAddress appliedAccessPointSubnet() const { return _apAppliedSubnet; }

    void accessPointOn();
    void accessPointOff();
    void accessPointMode(bool value);
    bool accessPointMode() const { return _apAllowed; }
    bool accessPointIsReady() const { return _apReady; }
    IPAddress accessPointIP() const;
    String accessPointIPString() const { return accessPointIP().toString(); }
    uint8_t accessPointStationCount() const { return _apStationCount; }

    // LEGACY FLProg_Ethernet SoftAP facade. FLProg 9.8.1 generated
    // projects still emit these exact names. They delegate to the native
    // accessPoint* API; no second AP implementation is created.
    void setApSsid(String ssid) { setAccessPointSsid(ssid); }
    void setApPassword(String password) { setAccessPointPassword(password); }
    String apSsid() const { return accessPointSsid(); }
    String apPassworid() const { return accessPointPassword(); } // historical typo

    void apMac(uint8_t m0, uint8_t m1, uint8_t m2, uint8_t m3, uint8_t m4, uint8_t m5);
    void apMac(uint8_t *mac)
    {
        if (mac) apMac(mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    }
    uint8_t *apMac() { return _apMac; }

    void apLocalIP(IPAddress ip) { setAccessPointIP(ip); }
    void apLocalIP(uint32_t ip) { setAccessPointIP(IPAddress(ip)); }
    void apLocalIP(uint8_t a, uint8_t b, uint8_t c, uint8_t d)
    {
        setAccessPointIP(IPAddress(a, b, c, d));
    }
    IPAddress apLocalIP() const { return accessPointConfiguredIP(); }

    void apDns(IPAddress ip);
    void apDns(uint32_t ip) { apDns(IPAddress(ip)); }
    void apDns(uint8_t a, uint8_t b, uint8_t c, uint8_t d)
    {
        apDns(IPAddress(a, b, c, d));
    }
    IPAddress apDns() const { return _apDns; }

    void apSubnet(IPAddress ip) { setAccessPointSubnet(ip); }
    void apSubnet(uint32_t ip) { setAccessPointSubnet(IPAddress(ip)); }
    void apSubnet(uint8_t a, uint8_t b, uint8_t c, uint8_t d)
    {
        setAccessPointSubnet(IPAddress(a, b, c, d));
    }
    IPAddress apSubnet() const { return accessPointConfiguredSubnet(); }

    void apGateway(IPAddress ip) { setAccessPointGateway(ip); }
    void apGateway(uint32_t ip) { setAccessPointGateway(IPAddress(ip)); }
    void apGateway(uint8_t a, uint8_t b, uint8_t c, uint8_t d)
    {
        setAccessPointGateway(IPAddress(a, b, c, d));
    }
    IPAddress apGateway() const { return accessPointConfiguredGateway(); }

    void apOn() { accessPointOn(); }
    void apOff() { accessPointOff(); }
    void apMode(bool value) { accessPointMode(value); }
    bool apMode() const { return accessPointMode(); }
    bool apIsReady() const { return accessPointIsReady(); }
    bool staAndAccessPointReady() const { return _wifiReady && _apReady; }

    // Cooperative asynchronous Wi-Fi scan. Results remain valid until the
    // next scan or clearWifiScanResults().
    bool startWifiScan(bool showHidden = false);
    bool cancelWifiScan();
    void clearWifiScanResults();
    bool wifiScanRunning() const
    {
        return _wifiScanState == FLPROG_WIFI_SCAN_QUEUED ||
               _wifiScanState == FLPROG_WIFI_SCAN_RUNNING;
    }
    bool wifiScanPending() const { return wifiScanRunning(); }
    uint8_t wifiScanState() const { return _wifiScanState; }
    uint8_t wifiScanLastError() const { return _wifiScanLastError; }
    int16_t wifiScanCount() const { return _wifiScanCount; }
    String wifiScanSsid(uint8_t index) const;
    int32_t wifiScanRssi(uint8_t index) const;
    int32_t wifiScanChannel(uint8_t index) const;
    uint8_t wifiScanEncryption(uint8_t index) const;
    bool wifiScanHidden(uint8_t index) const;

    uint16_t wifiEvents() const { return _wifiEvents; }
    uint16_t takeWifiEvents()
    {
        const uint16_t value = _wifiEvents;
        _wifiEvents = FLPROG_WIFI_EVENT_NONE;
        return value;
    }
    uint32_t wifiScanStartCount() const { return _wifiScanStartCount; }
    uint32_t wifiScanCompleteCount() const { return _wifiScanCompleteCount; }
    uint32_t wifiScanFailureCount() const { return _wifiScanFailureCount; }
    uint32_t wifiScanCancelCount() const { return _wifiScanCancelCount; }
    uint32_t wifiScanTimeoutCount() const { return _wifiScanTimeoutCount; }
    uint32_t accessPointStartCount() const { return _apStartCount; }
    uint32_t accessPointRestartCount() const { return _apRestartCount; }
    uint32_t accessPointFailureCount() const { return _apFailureCount; }
    uint8_t accessPointLastError() const { return _apLastError; }
    bool accessPointConfigurationBlocked() const { return _apConfigBlocked; }
    uint32_t accessPointClientJoinCount() const { return _apClientJoinCount; }
    uint32_t accessPointClientLeaveCount() const { return _apClientLeaveCount; }

    // W5500 / Wiznet style API.
    /**
     * Set W5500/Wiznet chip-select pin.
     * Changing CS invalidates the current Ethernet state and schedules restart.
     */
    void setPinCs(int pinCs);
    int pinCs() const { return _pinCs; }
    void setSpiBus(uint8_t bus);
    uint8_t spiBus() const { return _spiBus; }
    uint8_t getSpiBus() const { return _spiBus; }
    bool spiBusAvailable() const { return wiznetRef().spiBusAvailable(); }
    bool spiPinsSupported() const { return wiznetRef().spiPinsSupported(); }
    SPIClass *spiObject() { return wiznetRef().spiObject(); }

    // SPI bus-management API. These calls affect only the Wiznet SPI backend;
    // RMII/LAN8720 does not use SPI.
    void setSpiObject(SPIClass *object);
    void clearSpiObject();
    void setSpiPins(int sck, int miso, int mosi);
    void clearSpiPins();

    // Wiznet SPI backend. The default is standalone Arduino SPI.
    void setWiznetSpiBackend(FLProgSpiBackend *backend);
    void useArduinoWiznetSpiBackend();
    uint8_t wiznetSpiBackendKind() const { return wiznetRef().spiBackendKind(); }
    uint8_t effectiveWiznetSpiBus() const { return wiznetRef().effectiveSpiBus(); }
    int effectiveWiznetPinCs() const { return wiznetRef().effectivePinCs(); }

    void setSharedSpiBusyFlag(volatile bool *flag)
    {
        wiznetRef().setSharedSpiBusyFlag(flag);
    }

    void setSpiLockCallbacks(
        FLProgSpiLockCallback lockCallback,
        FLProgSpiUnlockCallback unlockCallback,
        void *context = nullptr)
    {
        wiznetRef().setSpiLockCallbacks(
            lockCallback,
            unlockCallback,
            context
        );
    }

    bool tryLockSpi() { return wiznetRef().acquireSpi(); }
    void unlockSpi() { wiznetRef().releaseSpi(); }
    bool spiBusy() const { return wiznetRef().spiBusy(); }
    uint8_t spiStatus() const { return wiznetRef().lastSpiStatus(); }
    uint32_t spiConflictCount() const { return wiznetRef().spiConflictCount(); }
    uint32_t spiTransactionCount() const { return wiznetRef().spiTransactionCount(); }
    uint32_t spiDeferredCount() const { return wiznetRef().spiDeferredCount(); }
    void resetSpiStatistics() { wiznetRef().resetSpiStatistics(); }

    template <typename T>
    void bindSpiBus(T *value)
    {
        _spiBusPtr = value;
        _spiBusReader = &readByteValue<T>;
    }

    uint8_t chipCode() const { return wiznetRef().chipCode(); }
    const char *chipName() const { return wiznetRef().chipName(); }
    uint8_t maxSocketNum() const { return wiznetRef().maxSocketNum(); }
    uint16_t txMemoryTotal() const { return wiznetRef().txMemoryTotal(); }
    uint16_t rxMemoryTotal() const { return wiznetRef().rxMemoryTotal(); }

    // Independent wired backends ---------------------------------------------
    // Wiznet and RMII may both be enabled on ESP32. The old "Ethernet" API is
    // retained as a generic wired view for backward compatibility.
    void wiznetMode(bool value);
    bool wiznetMode() const { return _wiznetAllowed; }

    void rmiiMode(bool value);
    bool rmiiMode() const { return _rmiiAllowed; }

    void setWiredPriority(uint8_t value);
    uint8_t wiredPriority() const { return _wiredPriority; }

    FLProgRmiiEthernet &rmiiHardware() { return rmiiRef(); }
    const FLProgRmiiEthernet &rmiiHardware() const { return rmiiRef(); }

    void setRmiiPhyAddress(int32_t value) { rmiiRef().setPhyAddress(value); }
    void setRmiiMdcPin(int value) { rmiiRef().setMdcPin(value); }
    void setRmiiMdioPin(int value) { rmiiRef().setMdioPin(value); }
    void setRmiiPowerPin(int value) { rmiiRef().setPowerPin(value); }
    void setRmiiResetPin(int value) { rmiiRef().setResetPin(value); }
    void setRmiiClockMode(uint8_t value) { rmiiRef().setClockMode(value); }
    void setRmiiHostname(const String &value) { rmiiRef().setHostname(value); }

    void rmiiDhcpMode(bool value) { rmiiRef().dhcpMode(value); }
    bool rmiiIsDhcp() const { return rmiiRef().isDhcp(); }
    void rmiiLocalIP(IPAddress value) { rmiiRef().localIP(value); }
    void rmiiGateway(IPAddress value) { rmiiRef().gateway(value); }
    void rmiiSubnet(IPAddress value) { rmiiRef().subnet(value); }
    void rmiiDns(IPAddress value) { rmiiRef().dns(value); }
    void rmiiDns2(IPAddress value) { rmiiRef().dns2(value); }


    void setRetransmissionTime(uint16_t value)
    {
        _retransmissionTime = value;
        if (wiznetRef().isInit())
        {
            wiznetRef().setRetransmissionTime(value);
        }
    }

    uint16_t retransmissionTime() const { return _retransmissionTime; }

    void setRetransmissionCount(uint8_t value)
    {
        _retransmissionCount = value;
        if (wiznetRef().isInit())
        {
            wiznetRef().setRetransmissionCount(value);
        }
    }

    uint8_t retransmissionCount() const { return _retransmissionCount; }

    // W5500 Sn_MR bit 6: BCASTB (Broadcast Blocking). W5500 only.
    // Applies ONLY to UDP (P[3:0] = 0x2) and MACRAW (P[3:0] = 0x4).
    // 0 = accept broadcast packets
    // 1 = block broadcast packets
    /**
     * Configure W5500 Sn_MR.BCASTB policy.
     * false: broadcast packets are accepted.
     * true : broadcast packets are blocked for UDP and MACRAW sockets.
     */
    void setBroadcastBlocking(bool value);
    void broadcastBlocking(bool value) { setBroadcastBlocking(value); }
    bool broadcastBlocking() const { return _broadcastBlocking; }
    bool getBroadcastBlocking() const { return _broadcastBlocking; }

    // Bind a FLProg BOOL input. pool() will read it automatically.
    template <typename T>
    void bindBroadcastBlocking(T *value)
    {
        _broadcastBlockingPtr = value;
        _broadcastBlockingReader = &readBoolValue<T>;
    }

    // Low-level helpers for UDP/MACRAW implementations.
    // prepareW5500SocketMode() should be used BEFORE OPEN when this library
    // itself creates a W5500 socket. applyBroadcastBlocking() is also called
    // by pool() so sockets created by compatible external code are corrected.
    /**
     * Prepare an Sn_MR value before issuing the W5500 OPEN command.
     * BCASTB is changed only for UDP (0x02) and MACRAW (0x04) modes.
     */
    uint8_t prepareW5500SocketMode(uint8_t mode) const;
    /** Apply BCASTB to one already configured W5500 UDP/MACRAW socket. */
    bool setSocketBroadcastBlocking(uint8_t socket, bool value);
    /** Apply the configured BCASTB policy to all active W5500 sockets. */
    uint8_t applyBroadcastBlocking();

    void setReconnectionPeriod(uint32_t period) { _reconnectionPeriod = period; }
    uint32_t reconnectionPeriod() const { return _reconnectionPeriod; }

    void setCheckStatusPeriod(uint32_t period) { _checkStatusPeriod = period; }
    uint32_t checkStatusPeriod() const { return _checkStatusPeriod; }

    void setResponseDhcpTimeout(uint32_t period) { _responseDhcpTimeout = period; wiznetDhcpRef().setResponseTimeout(period); }
    uint32_t responseDhcpTimeout() const { return _responseDhcpTimeout; }

    void setDhcpTimeout(uint32_t period) { _dhcpTimeout = period; wiznetDhcpRef().setAcquireTimeout(period); }
    uint32_t dhcpTimeout() const { return _dhcpTimeout; }

    void setMaintainPeriod(uint32_t period) { _maintainPeriod = period; }
    uint32_t maintainPeriod() const { return _maintainPeriod; }

    // Network parameters API style from FLProg_Ethernet.
    void setDhcp() { dhcpMode(true); }
    void resetDhcp() { dhcpMode(false); }
    /**
     * Select DHCP (true) or static addressing (false).
     * A mode change schedules restart of the selected network interfaces.
     */
    void dhcpMode(bool value);
    bool isDhcp() const { return _dhcp; }

    // Native non-blocking Wiznet DHCP diagnostics.
    FLProgWiznetDhcp &wiznetDhcpClient() { return wiznetDhcpRef(); }
    const FLProgWiznetDhcp &wiznetDhcpClient() const { return wiznetDhcpRef(); }
    uint8_t dhcpState() const { return wiznetDhcpRef().state(); }
    uint8_t dhcpError() const { return wiznetDhcpRef().error(); }
    uint32_t dhcpLeaseTime() const { return wiznetDhcpRef().leaseTime(); }
    uint32_t dhcpRenewalTime() const { return wiznetDhcpRef().renewalTime(); }
    uint32_t dhcpRebindingTime() const { return wiznetDhcpRef().rebindingTime(); }
    IPAddress dhcpServer() const { return wiznetDhcpRef().serverIP(); }
    bool releaseDhcpLease() { return wiznetDhcpRef().release(); }
    uint32_t dhcpDeclineCount() const { return wiznetDhcpRef().declineCount(); }
    uint32_t dhcpReleaseCount() const { return wiznetDhcpRef().releaseCount(); }
    void setDhcpConflictProbe(FLProgDhcpConflictProbe probe, void *context = nullptr)
    { wiznetDhcpRef().setConflictProbe(probe, context); }
    void setDhcpConflictProbeTimeout(uint32_t value)
    { wiznetDhcpRef().setConflictProbeTimeout(value); }
    void setDhcpAddressConflictProbe(FLProgDhcpAddressConflictProbe probe, void *context = nullptr)
    { wiznetDhcpRef().setAddressConflictProbe(probe, context); }
    void setDhcpBuiltInArpProbeEnabled(bool value)
    { wiznetDhcpRef().setBuiltInArpProbeEnabled(value); }
    bool dhcpBuiltInArpProbeEnabled() const
    { return wiznetDhcpRef().builtInArpProbeEnabled(); }
    void setDhcpArpAnnouncementEnabled(bool value)
    { wiznetDhcpRef().setArpAnnouncementEnabled(value); }
    bool dhcpArpAnnouncementEnabled() const
    { return wiznetDhcpRef().arpAnnouncementEnabled(); }
    void setDhcpArpAddressDefenseEnabled(bool value)
    { wiznetDhcpRef().setArpAddressDefenseEnabled(value); }
    bool dhcpArpAddressDefenseEnabled() const
    { return wiznetDhcpRef().arpAddressDefenseEnabled(); }
    void setDhcpAcdMode(uint8_t value) { wiznetDhcpRef().setAcdMode(value); }
    uint8_t dhcpAcdMode() const { return wiznetDhcpRef().acdMode(); }
    bool dhcpStrictAcdActive() const { return wiznetDhcpRef().strictAcdActive(); }
    bool dhcpStrictAcdSocketReserved() const { return wiznetDhcpRef().strictAcdSocketReserved(); }
    bool dhcpStrictAcdResourceBlocked() const { return wiznetDhcpRef().strictAcdResourceBlocked(); }
    uint32_t dhcpStrictAcdResourceBlockedCount() const { return wiznetDhcpRef().strictAcdResourceBlockedCount(); }
    uint32_t dhcpStrictAcdConflictCount() const { return wiznetDhcpRef().strictAcdConflictCount(); }
    uint8_t dhcpArpProbesSent() const { return wiznetDhcpRef().arpProbesSent(); }
    uint32_t dhcpArpConflictCount() const { return wiznetDhcpRef().arpConflictCount(); }
    uint32_t dhcpArpAnnouncementCount() const { return wiznetDhcpRef().arpAnnouncementCount(); }
    uint32_t dhcpArpDefenseCount() const { return wiznetDhcpRef().arpDefenseCount(); }
    uint32_t dhcpArpDefenseConflictCount() const { return wiznetDhcpRef().arpDefenseConflictCount(); }
    uint32_t dhcpArpRateLimitCount() const { return wiznetDhcpRef().arpRateLimitCount(); }
    uint8_t dhcpArpConflictSeriesCount() const { return wiznetDhcpRef().arpConflictSeriesCount(); }
    bool dhcpHasArpConflictMac() const { return wiznetDhcpRef().hasArpConflictMac(); }
    const uint8_t *dhcpArpConflictMac() const { return wiznetDhcpRef().arpConflictMac(); }

    void localIP(IPAddress ip);
    void localIP(uint32_t ip) { localIP(IPAddress(ip)); }
    void localIP(uint8_t a, uint8_t b, uint8_t c, uint8_t d) { localIP(IPAddress(a, b, c, d)); }
    IPAddress localIP() const;
    // Effective addresses of the ready default route; zero while unavailable.
    // Existing gateway()/subnet() retain configured-value semantics.
    // Read the first attached identity provider; no copied identity storage.
    const char *deviceModel() const;
    const char *firmwareVersion() const;
    IPAddress activeGateway() const;
    IPAddress activeSubnet() const;

    void dns(IPAddress ip);
    void dns(uint32_t ip) { dns(IPAddress(ip)); }
    void dns(uint8_t a, uint8_t b, uint8_t c, uint8_t d) { dns(IPAddress(a, b, c, d)); }
    IPAddress dns() const { return _dnsIp; }

    void subnet(IPAddress ip);
    void subnet(uint32_t ip) { subnet(IPAddress(ip)); }
    void subnet(uint8_t a, uint8_t b, uint8_t c, uint8_t d) { subnet(IPAddress(a, b, c, d)); }
    IPAddress subnet() const { return _subnetIp; }

    void gateway(IPAddress ip);
    void gateway(uint32_t ip) { gateway(IPAddress(ip)); }
    void gateway(uint8_t a, uint8_t b, uint8_t c, uint8_t d) { gateway(IPAddress(a, b, c, d)); }
    IPAddress gateway() const { return _gatewayIp; }

    void mac(uint8_t m0, uint8_t m1, uint8_t m2, uint8_t m3, uint8_t m4, uint8_t m5);
    void mac(uint8_t *address);
    uint8_t *mac() { return _macAddress; }

    // Universal Transport Layer --------------------------------------------
    //
    // Transport objects expose a common state API. TCP/UDP implementations
    // added in later versions will use these objects instead of accessing
    // Wi-Fi, Wiznet or RMII hardware directly.
    FLProgNetworkTransport *transportById(uint8_t transportId);
    const FLProgNetworkTransport *transportById(uint8_t transportId) const;

    // RC6-A.3 A3-2: exact transport lookup inside this NetworkDevice.
    FLProgNetworkTransport *transportByIdentity(
        FLProgTransportKind kind,
        FLProgTransportInstanceId instanceId);

    const FLProgNetworkTransport *transportByIdentity(
        FLProgTransportKind kind,
        FLProgTransportInstanceId instanceId) const;

    FLProgNetworkTransport *transportByIdentity(
        const FLProgTransportIdentity &identity)
    {
        return transportByIdentity(identity.kind, identity.instanceId);
    }

    const FLProgNetworkTransport *transportByIdentity(
        const FLProgTransportIdentity &identity) const
    {
        return transportByIdentity(identity.kind, identity.instanceId);
    }

    // Resolve a route to the full transport identity. Existing built-in
    // routes currently select instance #0; later A3 gates may bind a route
    // to another instance without changing callers.
    FLProgTransportIdentity transportIdentityForRoute(
        uint8_t route,
        bool requireReady = true) const;

    // RC6-A.3 A3-6: optional per-NetworkDevice exact route binding.
    // When present, route resolution uses this exact kind + instanceId.
    // Existing built-in route behaviour is unchanged when no binding exists.
    bool setRouteTransportIdentity(
        uint8_t route,
        const FLProgTransportIdentity &identity);

    void clearRouteTransportIdentity(uint8_t route);

    FLProgTransportIdentity configuredRouteTransportIdentity(
        uint8_t route) const;

    FLProgNetworkTransport *transportForRoute(
        uint8_t route,
        bool requireReady = true);

    const FLProgNetworkTransport *transportForRoute(
        uint8_t route,
        bool requireReady = true) const;

    uint8_t transportIdForRoute(
        uint8_t route,
        bool requireReady = true) const;

    FLProgWiFiTransport &wifiTransport() { return _wifiTransport; }
    FLProgWiFiApTransport &wifiApTransport() { return _wifiApTransport; }
    FLProgWiznetTransport &wiznetTransport() { return wiznetTransportRef(); }
    FLProgRmiiTransport &rmiiTransport() { return rmiiTransportRef(); }

    const FLProgWiFiTransport &wifiTransport() const { return _wifiTransport; }
    const FLProgWiFiApTransport &wifiApTransport() const { return _wifiApTransport; }
    const FLProgWiznetTransport &wiznetTransport() const { return wiznetTransportRef(); }
    const FLProgRmiiTransport &rmiiTransport() const { return rmiiTransportRef(); }

    uint8_t wiznetReservedSocketCount() const { return wiznetTransportRef().nativeSocketReservedCount(); }
    uint8_t wiznetAvailableSocketCount() const { return wiznetTransportRef().nativeSocketAvailableCount(); }
    bool wiznetSocketReserved(int16_t socket) const { return wiznetTransportRef().nativeSocketReserved(socket); }

    // Route-aware DNS --------------------------------------------------------
    // The default resolver is managed by network.pool() and shared by
    // convenience APIs such as FLProgNetworkUdp::beginPacket(host, port).
    // Lazily creates and registers the default DNS resolver. The pointer can
    // be null only if allocation or service registration fails.
    FLProgNetworkDns *dnsResolver();
    const FLProgNetworkDns *dnsResolver() const { return _dnsResolver; }
    bool hasDnsResolver() const { return _dnsResolver != nullptr; }

    IPAddress dnsServerForTransport(uint8_t transportId) const;
    IPAddress dnsServerForRoute(
        uint8_t route,
        bool requireReady = true) const;

    FLProgSocketManager &socketManager() { return _socketManager; }
    const FLProgSocketManager &socketManager() const { return _socketManager; }

    // Common Socket API objects are serviced from network.pool().
    uint8_t managedSocketCount() const;
    uint8_t managedServiceCount() const;

    uint8_t managedSocketLimit() const { return FLPROG_NETWORK_MAX_MANAGED_SOCKETS; }
    uint8_t managedServiceLimit() const { return FLPROG_NETWORK_MAX_MANAGED_SERVICES; }
    uint8_t managedSocketFreeCount() const
    {
        const uint8_t used = managedSocketCount();
        return used >= FLPROG_NETWORK_MAX_MANAGED_SOCKETS
            ? 0
            : (uint8_t)(FLPROG_NETWORK_MAX_MANAGED_SOCKETS - used);
    }
    uint8_t managedServiceFreeCount() const
    {
        const uint8_t used = managedServiceCount();
        return used >= FLPROG_NETWORK_MAX_MANAGED_SERVICES
            ? 0
            : (uint8_t)(FLPROG_NETWORK_MAX_MANAGED_SERVICES - used);
    }
    uint8_t logicalLeaseCount() const { return _socketManager.activeLeaseCount(); }
    uint8_t logicalLeaseFreeCount() const { return _socketManager.freeLeaseCount(); }

    FLProgNetworkCapacitySnapshot capacitySnapshot() const;

    // Reserve a logical socket for a future TCP/UDP object.
    // The owner pointer should be the address of that protocol object.
    uint8_t reserveSocket(
        const void *owner,
        uint8_t route,
        bool requireReady = true);

    bool activateSocket(uint8_t handle, const void *owner)
    {
        return _socketManager.activate(handle, owner);
    }

    bool releaseSocket(
        uint8_t handle,
        const void *owner = nullptr)
    {
        return _socketManager.release(handle, owner);
    }

    uint8_t releaseOwnerSockets(const void *owner)
    {
        return _socketManager.releaseOwner(owner);
    }

    bool socketIsStale(uint8_t handle) const
    {
        return _socketManager.isStale(handle);
    }

    // Combined-interface API used by the new FLProg user block.
    /**
     * Select transport mode:
     * 0 = Wi-Fi, 1 = wired Ethernet, 2 = AUTO, 3 = BOTH.
     */
    void networkMode(uint8_t value);
    uint8_t networkMode() const { return _networkMode; }

    /** Globally enable or disable network processing. */
    void workMode(bool value);
    bool workMode() const { return _workMode; }

    // Inputs are processed on their positive edges INSIDE the library.
    /** Process the FLProg CON input; action is generated on a rising edge. */
    void connectInput(bool value);
    /** Process the FLProg DIS input; action is generated on a rising edge. */
    void disconnectInput(bool value);

    /** Request connection of every interface selected by networkMode(). */
    void connect();
    /** Stop automatic connection attempts until a new connect request. */
    void disconnect();

    // -----------------------------------------------------------------
    // FLProg block binding.
    //
    // Call ONCE from SetupSection. After binding, pool() itself reads
    // all inputs and writes all outputs. Therefore LoopSection contains
    // only:
    //
    //     flpNetwork.pool();
    //
    // Template parameters intentionally accept FLProg-generated scalar
    // types without requiring an exact int/byte typedef match.
    // -----------------------------------------------------------------
    template <
        typename TEN,
        typename TCON,
        typename TDIS,
        typename TNET,
        typename TSSID,
        typename TPASSWORD,
        typename TCS,
        typename TCONNECTED,
        typename TCONNECTING,
        typename TERROR,
        typename TWIFIOK,
        typename TETHERNETOK,
        typename TRSSI,
        typename TIP,
        typename TACTIVE,
        typename TSTATE>
    void bindBlockIO(
        TEN *en,
        TCON *con,
        TDIS *dis,
        TNET *net,
        TSSID *ssid,
        TPASSWORD *password,
        TCS *ethCs,
        TCONNECTED *connected,
        TCONNECTING *connecting,
        TERROR *error,
        TWIFIOK *wifiOk,
        TETHERNETOK *ethernetOk,
        TRSSI *rssiValue,
        TIP *ipValue,
        TACTIVE *active,
        TSTATE *state)
    {
        _enPtr = en;
        _enReader = &readBoolValue<TEN>;

        _conPtr = con;
        _conReader = &readBoolValue<TCON>;

        _disPtr = dis;
        _disReader = &readBoolValue<TDIS>;

        _netPtr = net;
        _netReader = &readByteValue<TNET>;

        _ssidPtr = ssid;
        _ssidReader = &readStringValue<TSSID>;

        _passwordPtr = password;
        _passwordReader = &readStringValue<TPASSWORD>;

        _ethCsPtr = ethCs;
        _ethCsReader = &readIntValue<TCS>;

        _connectedPtr = connected;
        _connectedWriter = &writeBoolValue<TCONNECTED>;

        _connectingPtr = connecting;
        _connectingWriter = &writeBoolValue<TCONNECTING>;

        _errorOutPtr = error;
        _errorWriter = &writeBoolValue<TERROR>;

        _wifiOkPtr = wifiOk;
        _wifiOkWriter = &writeBoolValue<TWIFIOK>;

        _ethernetOkPtr = ethernetOk;
        _ethernetOkWriter = &writeBoolValue<TETHERNETOK>;

        _rssiPtr = rssiValue;
        _rssiWriter = &writeInt32Value<TRSSI>;

        _ipPtr = ipValue;
        _ipWriter = &writeStringValue<TIP>;

        _activePtr = active;
        _activeWriter = &writeByteValue<TACTIVE>;

        _statePtr = state;
        _stateWriter = &writeByteValue<TSTATE>;

        _blockIoBound = true;
    }

    /**
     * Main cyclic service method.
     *
     * It reads all bound FLProg inputs, services Wi-Fi/Ethernet, performs
     * reconnection and W5500 socket policy maintenance, updates the state
     * machine, and writes all bound outputs.
     *
     * FLProg LoopSection should contain only:
     *     flpNetwork.pool();
     */
    FLProgWiznetHardware &wiznetHardware() { return wiznetRef(); }
    const FLProgWiznetHardware &wiznetHardware() const { return wiznetRef(); }

    template <typename TWIFIIP, typename TETHERNETIP>
    void bindInterfaceIPs(
        TWIFIIP *wifiIp,
        TETHERNETIP *ethernetIp)
    {
        _wifiIpPtr = wifiIp;
        _wifiIpWriter = &writeStringValue<TWIFIIP>;

        _ethernetIpPtr = ethernetIp;
        _ethernetIpWriter = &writeStringValue<TETHERNETIP>;
    }

    template <typename TCHIP, typename TSOCKETS, typename TLINK>
    void bindHardwareInfo(
        TCHIP *chip,
        TSOCKETS *sockets,
        TLINK *link)
    {
        _chipOutPtr = chip;
        _chipOutWriter = &writeByteValue<TCHIP>;

        _socketsOutPtr = sockets;
        _socketsOutWriter = &writeByteValue<TSOCKETS>;

        _linkOutPtr = link;
        _linkOutWriter = &writeByteValue<TLINK>;
    }

    uint8_t pool();

    bool isReady() const { return _wifiReady || _apReady || _ethernetReady || _rmiiReady; }

    // Legacy FLProg_Ethernet interface-level compatibility surface.
    // These names intentionally preserve source compatibility only at the
    // interface/configuration layer. Legacy raw socket methods are NOT exposed
    // here; TCP/UDP adapters map those operations to the A3 socket backend.
    bool isReadyForDNS() const { return isReady(); }
    bool isImitation() const { return false; }
    bool isInit() const { return true; }

    // Legacy FLProg_Ethernet compatibility. The old abstract interface
    // exposed this property to all interface types. OnBoardWifi stored it
    // but did not use it to throttle pool().
    void setSkippingEvents(uint16_t value) { _legacySkippingEvents = value; }
    uint16_t getSkippingEvents() const { return _legacySkippingEvents; }
    uint8_t maxSoketNum() const
    {
#if FLPROG_NETWORK_COMPILE_WIRED
        return maxSocketNum();
#else
        // WiFi-only compatibility path: do not create a link-time dependency
        // on FLProgWiznetHardware. Legacy adapters use the logical A3 socket
        // capacity until a transport-specific compatibility adapter is bound.
        return FLPROG_NETWORK_MAX_MANAGED_SOCKETS;
#endif
    }

    bool ethernetIsReady() const { return _ethernetReady || _rmiiReady; }
    bool wiznetIsReady() const { return _ethernetReady; }
    bool rmiiIsReady() const { return _rmiiReady; }
    bool wifiIsReady() const { return _wifiReady; }
    bool bothInterfacesReady() const { return _wifiReady && ethernetIsReady(); }

    uint8_t activeInterface() const { return _activeInterface; }
    uint8_t resolveRoute(uint8_t route) const;
    bool interfaceIsReady(uint8_t interfaceId) const;

    IPAddress wifiIP() const;
    IPAddress wiznetIP() const;
    IPAddress rmiiIP() const;
    IPAddress ethernetIP() const;
    String wifiIPString() const { return wifiIP().toString(); }
    String wiznetIPString() const { return wiznetIP().toString(); }
    String rmiiIPString() const { return rmiiIP().toString(); }
    String ethernetIPString() const { return ethernetIP().toString(); }

    uint16_t rmiiLinkSpeed() const { return rmiiRef().linkSpeed(); }
    bool rmiiFullDuplex() const { return rmiiRef().fullDuplex(); }
    uint8_t rmiiLinkStatus() const
    {
        if (!rmiiRef().started()) return 0;
        return rmiiRef().linkUp() ? 1 : 2;
    }
    uint8_t getStatus() const { return _status; }
    uint8_t getError() const { return _error; }
    bool isConnecting() const { return (_status == FLPROG_NETWORK_CONNECTING_STATUS) || (_status == FLPROG_NETWORK_RECONNECT_STATUS); }
    bool hasError() const { return _error != FLPROG_NETWORK_NOT_ERROR; }
    String ipString() const { return localIP().toString(); }

    /** Return Wi-Fi RSSI in dBm; returns 0 when Ethernet is active. */
    int32_t rssi() const;
    /** Return Wiznet hardware status using the underlying Ethernet driver. */
    uint8_t hardwareStatus() const;
    /** Return wired Ethernet link status. */
    uint8_t linkStatus() const;

private:
    static FLProgNetworkInterface *_legacyInterfaceList[FLPROG_NETWORK_MAX_DEVICES];
    static uint8_t _legacyInterfaceCount;
    void registerLegacyInterface();
    void unregisterLegacyInterface();
    template <typename T>
    static bool readBoolValue(void *ptr)
    {
        return static_cast<bool>(*static_cast<T *>(ptr));
    }

    template <typename T>
    static uint8_t readByteValue(void *ptr)
    {
        return static_cast<uint8_t>(*static_cast<T *>(ptr));
    }

    template <typename T>
    static int readIntValue(void *ptr)
    {
        return static_cast<int>(*static_cast<T *>(ptr));
    }

    template <typename T>
    static String readStringValue(void *ptr)
    {
        return String(*static_cast<T *>(ptr));
    }

    template <typename T>
    static void writeBoolValue(void *ptr, bool value)
    {
        *static_cast<T *>(ptr) = static_cast<T>(value);
    }

    template <typename T>
    static void writeByteValue(void *ptr, uint8_t value)
    {
        *static_cast<T *>(ptr) = static_cast<T>(value);
    }

    template <typename T>
    static void writeInt32Value(void *ptr, int32_t value)
    {
        *static_cast<T *>(ptr) = static_cast<T>(value);
    }

    template <typename T>
    static void writeStringValue(void *ptr, const String &value)
    {
        *static_cast<T *>(ptr) = value;
    }

    // FLProg binding layer. These methods convert the generated block
    // variables to/from the library's internal types.
    void syncBlockInputs();
    void syncBlockOutputs();

    // Runtime state-machine helpers.
    void resetRuntime();
    void requestSelectedInterfaces();
    void startWifi();
    void markAccessPointConfigurationChanged();
    void startAccessPoint();
    void updateWifiExtended();
    void applyWifiRadioMode();
    void startEthernet();
    void startRmii();
    bool configureStaticEthernet();
    void updateWifi();
    void updateEthernet();
    void updateRmii();
    void updateWiznetSocketPolicies();
    void updateActiveInterface();
    void updateState();
    bool usesWifi() const;
    bool usesEthernet() const;
    bool usesWiznet() const;
    bool usesRmii() const;
    bool macIsEmpty() const;
    void makeDefaultEthernetMac();

    // Universal transport adapters.
    friend class FLProgNetworkSocket;

    friend class FLProgNetworkService;

    bool registerManagedSocket(FLProgNetworkSocket *socket);
    void unregisterManagedSocket(FLProgNetworkSocket *socket);
    void updateSocketLayer();

    bool registerManagedService(FLProgNetworkService *service);
    void unregisterManagedService(FLProgNetworkService *service);
    void updateServiceLayer();

    void initTransportLayer();
    void updateTransportLayer();

    static bool transportWifiReady(void *context);
    static IPAddress transportWifiIp(void *context);
    static uint8_t transportWifiLink(void *context);

    static bool transportWifiApReady(void *context);
    static IPAddress transportWifiApIp(void *context);
    static uint8_t transportWifiApLink(void *context);

    static bool transportWiznetReady(void *context);
    static IPAddress transportWiznetIp(void *context);
    static uint8_t transportWiznetLink(void *context);
    static uint8_t transportWiznetSocketLimit(void *context);

    static bool transportRmiiReady(void *context);
    static IPAddress transportRmiiIp(void *context);
    static uint8_t transportRmiiLink(void *context);

    // EXP-1: wired storage is absent from Wi-Fi-only instances. These helpers
    // preserve source compatibility for wired API methods that are not used by
    // the Wi-Fi-only A1 resource gate. In the normal profile they return the
    // real inline members; in EXP-1 they resolve to lazy function-local stubs.
    FLProgWiznetHardware &wiznetRef();
    const FLProgWiznetHardware &wiznetRef() const;
    FLProgRmiiEthernet &rmiiRef();
    const FLProgRmiiEthernet &rmiiRef() const;
    FLProgWiznetDhcp &wiznetDhcpRef();
    const FLProgWiznetDhcp &wiznetDhcpRef() const;
    FLProgWiznetTransport &wiznetTransportRef();
    const FLProgWiznetTransport &wiznetTransportRef() const;
    FLProgRmiiTransport &rmiiTransportRef();
    const FLProgRmiiTransport &rmiiTransportRef() const;

    // -----------------------------------------------------------------
    // User configuration
    // -----------------------------------------------------------------
    String _clientSsid;
    String _clientPassword;

    String _apSsid = "FLProg";
    String _apPassword;
    uint8_t _apChannel = 1;
    bool _apHidden = false;
    uint8_t _apMaxClients = 4;
    IPAddress _apIp = IPAddress(192, 168, 4, 1);
    IPAddress _apGateway = IPAddress(192, 168, 4, 1);
    IPAddress _apSubnet = IPAddress(255, 255, 255, 0);
    uint8_t _apMac[6] = {0, 0, 0, 0, 0, 0};
    IPAddress _apDns = IPAddress(0, 0, 0, 0);

    // Last successfully applied AP configuration. Keep a snapshot instead of
    // querying platform-specific AP internals so ESP8266 and ESP32 expose the
    // same public semantics. The password is deliberately not duplicated.
    String _apAppliedSsid;
    uint8_t _apAppliedChannel = 1;
    bool _apAppliedHidden = false;
    uint8_t _apAppliedMaxClients = 0;
    IPAddress _apAppliedIp = IPAddress(0, 0, 0, 0);
    IPAddress _apAppliedGateway = IPAddress(0, 0, 0, 0);
    IPAddress _apAppliedSubnet = IPAddress(0, 0, 0, 0);
    uint32_t _apConfigGeneration = 1;
    uint32_t _apAppliedConfigGeneration = 0;
    bool _apAppliedConfigValid = false;

    int _pinCs = -1;
    uint8_t _spiBus = 255;

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgWiznetHardware _wiznet;
    FLProgRmiiEthernet _rmii;
    FLProgWiznetDhcp _wiznetDhcp;
#endif

    FLProgWiFiTransport _wifiTransport;
    FLProgWiFiApTransport _wifiApTransport;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    FLProgWiznetTransport _wiznetTransport;
    FLProgRmiiTransport _rmiiTransport;
#endif
    FLProgSocketManager _socketManager;

    FLProgNetworkSocket *_managedSockets[FLPROG_NETWORK_MAX_MANAGED_SOCKETS] =
        {nullptr};

    FLProgNetworkService *_managedServices[FLPROG_NETWORK_MAX_MANAGED_SERVICES] =
        {nullptr};

    FLProgNetworkDns *_dnsResolver = nullptr;

    bool _wiznetAllowed = true;
    bool _rmiiAllowed = false;
    uint8_t _wiredPriority = FLPROG_NETWORK_WIRED_PRIORITY_WIZNET;

    bool _ethernetConfigured = false;
    IPAddress _wiznetIpCache = IPAddress(0, 0, 0, 0);

    uint16_t _retransmissionTime = 2000;
    uint8_t _retransmissionCount = 8;

    bool _dhcp = true;
    IPAddress _configuredIp = IPAddress(0, 0, 0, 0);
    IPAddress _dnsIp = IPAddress(0, 0, 0, 0);
    IPAddress _subnetIp = IPAddress(255, 255, 255, 0);
    IPAddress _gatewayIp = IPAddress(0, 0, 0, 0);
    uint8_t _macAddress[6] = {0, 0, 0, 0, 0, 0};

    // A3-6 exact route overrides. An invalid identity means legacy route
    // resolution. Slots correspond to the public route constants 0..6.
    FLProgTransportIdentity _routeTransportIdentity[
        FLPROG_NETWORK_ROUTE_WIFI_AP + 1
    ];

    // -----------------------------------------------------------------
    // Runtime state
    // -----------------------------------------------------------------
    uint8_t _networkMode = FLPROG_NETWORK_MODE_WIFI;
    uint8_t _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;
    uint8_t _status = FLPROG_NETWORK_DISABLED_STATUS;
    uint8_t _error = FLPROG_NETWORK_NOT_ERROR;

    bool _workMode = false;
    bool _wifiAllowed = true;
    bool _apAllowed = false;
    bool _autoConnect = true;
    bool _autoReconnect = true;
    bool _manualStop = false;

    bool _prevConnectInput = false;
    bool _prevDisconnectInput = false;

    bool _wifiStartRequest = false;
    bool _apStartRequest = false;
    bool _ethernetStartRequest = false;
    bool _rmiiStartRequest = false;
    bool _wifiStarted = false;
    bool _apStarted = false;
    bool _ethernetStarted = false;
    bool _rmiiStarted = false;
    bool _wifiReady = false;
    bool _apReady = false;
    bool _ethernetReady = false;
    bool _rmiiReady = false;

    // RC6-A-dev.3-fix3: while a live STA restart is in progress, keep the
    // socket transport closed for new leases until the native Wi-Fi stack has
    // actually passed through a disconnected state. Public Wi-Fi READY remains
    // the real radio state; only socket-session admission is gated.
    bool _wifiSessionRestartGate = false;
    bool _wifiSessionRestartSawDown = false;
    uint32_t _wifiSessionRestartStarted = 0;

    uint8_t _apStationCount = 0;
    bool _prevApReady = false;
    bool _prevWifiReadyForEvents = false;
    uint8_t _wifiScanState = FLPROG_WIFI_SCAN_IDLE;
    int16_t _wifiScanCount = 0;
    bool _wifiScanShowHidden = false;
    bool _wifiScanStartRequest = false;
    bool _wifiScanCancelRequest = false;
    bool _wifiScanClearRequest = false;
    uint8_t _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_NONE;
    uint32_t _wifiScanStartedAt = 0;
    uint16_t _wifiEvents = FLPROG_WIFI_EVENT_NONE;
    uint32_t _wifiScanStartCount = 0;
    uint32_t _wifiScanCompleteCount = 0;
    uint32_t _wifiScanFailureCount = 0;
    uint32_t _wifiScanCancelCount = 0;
    uint32_t _wifiScanTimeoutCount = 0;
    uint32_t _apStartCount = 0;
    uint32_t _apRestartCount = 0;
    uint32_t _apFailureCount = 0;
    uint8_t _apLastError = FLPROG_WIFI_AP_ERROR_NONE;
    bool _apConfigBlocked = false;
    uint32_t _apClientJoinCount = 0;
    uint32_t _apClientLeaveCount = 0;
    uint8_t _wifiRadioMode = 0xFFU;

    // W5500 socket receive policy.
    bool _broadcastBlocking = false;
    bool _broadcastPolicyDirty = false;

    uint32_t _connectionStartTime = 0;
    uint32_t _lastWifiAttempt = 0;
    uint32_t _lastApAttempt = 0;
    uint32_t _lastEthernetAttempt = 0;
    uint32_t _lastRmiiAttempt = 0;
    uint32_t _lastEthernetCheck = 0;
    uint32_t _lastMaintainTime = 0;

    // -----------------------------------------------------------------
    // Timing parameters, milliseconds
    // -----------------------------------------------------------------
    uint32_t _connectTimeout = 15000;
    uint32_t _reconnectionPeriod = 5000;
    uint32_t _checkStatusPeriod = 1000;
    uint32_t _dhcpTimeout = 10000;
    uint32_t _responseDhcpTimeout = 1000;
    uint32_t _maintainPeriod = 60000;

    // Legacy FLProg_Ethernet interface property.
    uint16_t _legacySkippingEvents = 0;

    // FLProg block bindings ---------------------------------------------------
    bool _blockIoBound = false;

    void *_enPtr = nullptr;
    bool (*_enReader)(void *) = nullptr;

    void *_conPtr = nullptr;
    bool (*_conReader)(void *) = nullptr;

    void *_disPtr = nullptr;
    bool (*_disReader)(void *) = nullptr;

    void *_netPtr = nullptr;
    uint8_t (*_netReader)(void *) = nullptr;

    void *_ssidPtr = nullptr;
    String (*_ssidReader)(void *) = nullptr;

    void *_passwordPtr = nullptr;
    String (*_passwordReader)(void *) = nullptr;

    void *_ethCsPtr = nullptr;
    int (*_ethCsReader)(void *) = nullptr;

    void *_spiBusPtr = nullptr;
    uint8_t (*_spiBusReader)(void *) = nullptr;

    void *_broadcastBlockingPtr = nullptr;
    bool (*_broadcastBlockingReader)(void *) = nullptr;

    void *_connectedPtr = nullptr;
    void (*_connectedWriter)(void *, bool) = nullptr;

    void *_connectingPtr = nullptr;
    void (*_connectingWriter)(void *, bool) = nullptr;

    void *_errorOutPtr = nullptr;
    void (*_errorWriter)(void *, bool) = nullptr;

    void *_wifiOkPtr = nullptr;
    void (*_wifiOkWriter)(void *, bool) = nullptr;

    void *_ethernetOkPtr = nullptr;
    void (*_ethernetOkWriter)(void *, bool) = nullptr;

    void *_rssiPtr = nullptr;
    void (*_rssiWriter)(void *, int32_t) = nullptr;

    void *_ipPtr = nullptr;
    void (*_ipWriter)(void *, const String &) = nullptr;

    void *_wifiIpPtr = nullptr;
    void (*_wifiIpWriter)(void *, const String &) = nullptr;

    void *_ethernetIpPtr = nullptr;
    void (*_ethernetIpWriter)(void *, const String &) = nullptr;

    void *_chipOutPtr = nullptr;
    void (*_chipOutWriter)(void *, uint8_t) = nullptr;

    void *_socketsOutPtr = nullptr;
    void (*_socketsOutWriter)(void *, uint8_t) = nullptr;

    void *_linkOutPtr = nullptr;
    void (*_linkOutWriter)(void *, uint8_t) = nullptr;

    void *_activePtr = nullptr;
    void (*_activeWriter)(void *, uint8_t) = nullptr;

    void *_statePtr = nullptr;
    void (*_stateWriter)(void *, uint8_t) = nullptr;
};

// -----------------------------------------------------------------------------
// Thin compatibility wrappers for the two configuration APIs used most often
// by FLProg_Ethernet. They are intentionally limited to interface management;
// TCP/UDP/Web/NTP classes from the old library are not copied here.
// -----------------------------------------------------------------------------

class FLProgOnBoardWifiInterface : public FLProgNetworkInterface
{
public:
    FLProgOnBoardWifiInterface()
    {
        networkMode(FLPROG_NETWORK_MODE_WIFI);
        workMode(true);
    }
};

class FLProgWiznetInterface : public FLProgNetworkInterface
{
public:
    FLProgWiznetInterface()
    {
        networkMode(FLPROG_NETWORK_MODE_ETHERNET);
        workMode(true);
    }

    FLProgWiznetInterface(int pinCs, uint8_t bus = 255) : FLProgNetworkInterface(pinCs, bus)
    {
        networkMode(FLPROG_NETWORK_MODE_ETHERNET);
        workMode(true);
    }

    uint8_t begin()
    {
        connect();
        return pool();
    }

    uint8_t begin(IPAddress ip)
    {
        resetDhcp();
        localIP(ip);
        connect();
        return pool();
    }

    uint8_t begin(IPAddress ip, IPAddress dnsIp)
    {
        resetDhcp();
        localIP(ip);
        dns(dnsIp);
        connect();
        return pool();
    }

    uint8_t begin(IPAddress ip, IPAddress dnsIp, IPAddress gatewayIp)
    {
        resetDhcp();
        localIP(ip);
        dns(dnsIp);
        gateway(gatewayIp);
        connect();
        return pool();
    }

    uint8_t begin(IPAddress ip, IPAddress dnsIp, IPAddress gatewayIp, IPAddress subnetIp)
    {
        resetDhcp();
        localIP(ip);
        dns(dnsIp);
        gateway(gatewayIp);
        subnet(subnetIp);
        connect();
        return pool();
    }
};
