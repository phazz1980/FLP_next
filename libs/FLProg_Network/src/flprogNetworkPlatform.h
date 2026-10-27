/*
 * FLProg_Network — flprogNetworkPlatform.h
 * Назначение: Определение платформы и platform capability guards.
 *
 * Архитектурные заметки:
 * - Здесь локализуется выбор ESP8266/ESP32/RP/STM32.
 * - Нельзя предполагать одинаковый SPI/API даже у близких Arduino-core: capability проверяется отдельно.
 * - Публичный FLProg API не должен зависеть от platform ID напрямую.
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
#include "flprogNetworkProfile.h"

// -----------------------------------------------------------------------------
// FLProg_Network v1.0.0-rc.6a.dev.2-fix2 - compile-time platform/backend capability map.
//
// This is deliberately a compile capability report, not a hardware-verification
// claim. A true flag means that the corresponding built-in backend is compiled
// by FLProg_Network for the current Arduino architecture. It does not mean that
// the board, PHY or external Wiznet chip has been physically verified.
// -----------------------------------------------------------------------------

#define FLPROG_NETWORK_PLATFORM_GENERIC 0
#define FLPROG_NETWORK_PLATFORM_ESP8266 1
#define FLPROG_NETWORK_PLATFORM_ESP32   2
#define FLPROG_NETWORK_PLATFORM_RP      3
#define FLPROG_NETWORK_PLATFORM_STM32   4

#if defined(ESP8266)
#define FLPROG_NETWORK_PLATFORM_ID FLPROG_NETWORK_PLATFORM_ESP8266
#if FLPROG_NETWORK_COMPILE_WIFI
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 1
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 1
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 1
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 1
#else
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 0
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#endif
#define FLPROG_NETWORK_COMPILED_RMII 0
#define FLPROG_NETWORK_HAS_POSIX_TCP 0
#define FLPROG_NETWORK_HAS_POSIX_UDP 0
#define FLPROG_NETWORK_PICO_RAW_LWIP_ONLY 0
#elif defined(ESP32)
#define FLPROG_NETWORK_PLATFORM_ID FLPROG_NETWORK_PLATFORM_ESP32
#if FLPROG_NETWORK_COMPILE_WIFI
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 1
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 1
#else
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 0
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 0
#endif
// RMII is available only on ESP32 SoCs that expose the internal EMAC,
// and only when the wired backend family is compiled by the selected profile.
// Arduino-ESP32 currently provides this path for classic ESP32 and ESP32-P4.
#if FLPROG_NETWORK_COMPILE_WIRED && \
    defined(CONFIG_ETH_ENABLED) && defined(CONFIG_ETH_USE_ESP32_EMAC) && \
    CONFIG_ETH_USE_ESP32_EMAC && \
    (defined(CONFIG_IDF_TARGET_ESP32) || defined(CONFIG_IDF_TARGET_ESP32P4))
#define FLPROG_NETWORK_COMPILED_RMII 1
#else
#define FLPROG_NETWORK_COMPILED_RMII 0
#endif

// ESP32 POSIX/lwIP sockets are a network-stack capability, not a Wi-Fi-only
// capability.  WIRED_ONLY + RMII must keep them compiled even when all Wi-Fi
// control/API code is excluded.
#if FLPROG_NETWORK_COMPILE_WIFI || FLPROG_NETWORK_COMPILED_RMII
#define FLPROG_NETWORK_HAS_POSIX_TCP 1
#define FLPROG_NETWORK_HAS_POSIX_UDP 1
#else
#define FLPROG_NETWORK_HAS_POSIX_TCP 0
#define FLPROG_NETWORK_HAS_POSIX_UDP 0
#endif
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#define FLPROG_NETWORK_PICO_RAW_LWIP_ONLY 0
#elif defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
#define FLPROG_NETWORK_PLATFORM_ID FLPROG_NETWORK_PLATFORM_RP
// Pico W high-level WiFi is intentionally not claimed as a built-in backend:
// route ownership / raw-lwIP adapter work is still explicit backlog.
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 0
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 0
#define FLPROG_NETWORK_COMPILED_RMII 0
#define FLPROG_NETWORK_HAS_POSIX_TCP 0
#define FLPROG_NETWORK_HAS_POSIX_UDP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#define FLPROG_NETWORK_PICO_RAW_LWIP_ONLY 1
#elif defined(ARDUINO_ARCH_STM32)
#define FLPROG_NETWORK_PLATFORM_ID FLPROG_NETWORK_PLATFORM_STM32
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 0
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 0
// STM32Ethernet supplies the internal-MAC Ethernet/lwIP stack. The header may
// be installed on STM32 targets that do not actually have an ETH peripheral, so
// runtime support remains board/MCU dependent. External-PHY compatibility is
// additionally controlled by the STM32 core/variant (stock STM32Ethernet is not
// interpreted here as a universal LAN8720 PHY driver).
#if defined(__has_include)
  #if __has_include(<STM32Ethernet.h>)
    #define FLPROG_NETWORK_STM32_ETHERNET_HEADER 1
  #else
    #define FLPROG_NETWORK_STM32_ETHERNET_HEADER 0
  #endif
#else
  #define FLPROG_NETWORK_STM32_ETHERNET_HEADER 0
#endif
#if FLPROG_NETWORK_STM32_ETHERNET_HEADER && \
    (defined(HAL_ETH_MODULE_ENABLED) || \
     (defined(USE_HAL_ETH_MODULE) && USE_HAL_ETH_MODULE) || \
     defined(ETH))
#define FLPROG_NETWORK_COMPILED_RMII 1
#define FLPROG_NETWORK_HAS_STM32_ETHERNET 1
#else
#define FLPROG_NETWORK_COMPILED_RMII 0
#define FLPROG_NETWORK_HAS_STM32_ETHERNET 0
#endif
#define FLPROG_NETWORK_HAS_POSIX_TCP 0
#define FLPROG_NETWORK_HAS_POSIX_UDP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#define FLPROG_NETWORK_PICO_RAW_LWIP_ONLY 0
#else
#define FLPROG_NETWORK_PLATFORM_ID FLPROG_NETWORK_PLATFORM_GENERIC
#define FLPROG_NETWORK_COMPILED_WIFI_CONTROL 0
#define FLPROG_NETWORK_COMPILED_WIFI_EXTENDED 0
#define FLPROG_NETWORK_COMPILED_RMII 0
#define FLPROG_NETWORK_HAS_POSIX_TCP 0
#define FLPROG_NETWORK_HAS_POSIX_UDP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#define FLPROG_NETWORK_PICO_RAW_LWIP_ONLY 0
#endif

#ifndef FLPROG_NETWORK_HAS_STM32_ETHERNET
#define FLPROG_NETWORK_HAS_STM32_ETHERNET 0
#endif

#ifndef FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP 0
#endif
#ifndef FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP
#define FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP 0
#endif

#define FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP \
    ((FLPROG_NETWORK_HAS_POSIX_TCP != 0) || (FLPROG_NETWORK_HAS_ESP8266_WIFI_TCP != 0))
#define FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP \
    ((FLPROG_NETWORK_HAS_POSIX_UDP != 0) || (FLPROG_NETWORK_HAS_ESP8266_WIFI_UDP != 0))



#if FLPROG_NETWORK_COMPILE_WIFI && (defined(ESP8266) || defined(ESP32))
#define FLPROG_NETWORK_COMPILED_ARDUINO_OTA 1
#else
#define FLPROG_NETWORK_COMPILED_ARDUINO_OTA 0
#endif

#if FLPROG_NETWORK_COMPILE_WIFI && defined(ESP8266)
#define FLPROG_NETWORK_WIFI_AP_MAX_CLIENTS 8
#elif FLPROG_NETWORK_COMPILE_WIFI && defined(ESP32)
#define FLPROG_NETWORK_WIFI_AP_MAX_CLIENTS 4
#else
#define FLPROG_NETWORK_WIFI_AP_MAX_CLIENTS 0
#endif

struct FLProgNetworkPlatformCapabilities
{
    uint8_t platformId = FLPROG_NETWORK_PLATFORM_ID;

    // Interface-management code compiled into this build.
    bool wifiControlCompiled = FLPROG_NETWORK_COMPILED_WIFI_CONTROL != 0;
    bool wifiExtendedCompiled = FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0;
    bool wifiApCompiled = FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0;
    bool wifiScanCompiled = FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0;
    bool rmiiCompiled = FLPROG_NETWORK_COMPILED_RMII != 0;

    // Direct Wiznet SPI stack is architecture-independent and always compiled.
    bool wiznetSpiCompiled = FLPROG_NETWORK_COMPILE_WIRED != 0;

    // Built-in packet/socket backends owned by FLProg_Network.
    bool tcpWifiBuiltIn = FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP;
    bool tcpWifiApBuiltIn =
        (FLPROG_NETWORK_HAS_BUILTIN_WIFI_TCP) &&
        (FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0);
    bool tcpRmiiBuiltIn =
        (FLPROG_NETWORK_COMPILED_RMII != 0) &&
        ((FLPROG_NETWORK_HAS_POSIX_TCP != 0) ||
         (FLPROG_NETWORK_HAS_STM32_ETHERNET != 0));
    bool udpWifiBuiltIn = FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP;
    bool udpWifiApBuiltIn =
        (FLPROG_NETWORK_HAS_BUILTIN_WIFI_UDP) &&
        (FLPROG_NETWORK_COMPILED_WIFI_EXTENDED != 0);
    bool udpRmiiBuiltIn =
        (FLPROG_NETWORK_COMPILED_RMII != 0) &&
        ((FLPROG_NETWORK_HAS_POSIX_UDP != 0) ||
         (FLPROG_NETWORK_HAS_STM32_ETHERNET != 0));

    // Arduino-Pico currently builds lwIP with LWIP_SOCKET=0. The ESP POSIX
    // backend is therefore intentionally disabled on RP2040/RP2350.
    bool rawLwipOnly = FLPROG_NETWORK_PICO_RAW_LWIP_ONLY != 0;

    // ArduinoOTA integration compiled only for ESP8266/ESP32. This is a
    // platform-global Wi-Fi OTA capability, not a route-aware socket backend.
    bool arduinoOtaCompiled = FLPROG_NETWORK_COMPILED_ARDUINO_OTA != 0;
};
