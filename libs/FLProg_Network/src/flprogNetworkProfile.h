/*
 * FLProg_Network — flprogNetworkProfile.h
 *
 * RC6-A dev.16-fix6-profiles
 *
 * One code base, three compile-time profiles:
 *   UNIVERSAL  = built-in Wi-Fi + wired backends
 *   WIFI_ONLY  = built-in Wi-Fi, wired backends excluded
 *   WIRED_ONLY = wired backends, built-in Wi-Fi excluded
 *
 * IMPORTANT:
 * The profile is compile-time and therefore applies to every translation
 * unit of the library. Runtime clientOff()/wiznetMode() alone cannot remove
 * code/data from the firmware.
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

#define FLPROG_NETWORK_BUILD_UNIVERSAL  0
#define FLPROG_NETWORK_BUILD_WIFI_ONLY  1
#define FLPROG_NETWORK_BUILD_WIRED_ONLY 2

#ifndef FLPROG_NETWORK_BUILD_PROFILE
#define FLPROG_NETWORK_BUILD_PROFILE 0
#endif

#if (FLPROG_NETWORK_BUILD_PROFILE < FLPROG_NETWORK_BUILD_UNIVERSAL) || \
    (FLPROG_NETWORK_BUILD_PROFILE > FLPROG_NETWORK_BUILD_WIRED_ONLY)
#error "FLProg_Network: invalid FLPROG_NETWORK_BUILD_PROFILE"
#endif

#define FLPROG_NETWORK_PROFILE_WIFI_ONLY \
    (FLPROG_NETWORK_BUILD_PROFILE == FLPROG_NETWORK_BUILD_WIFI_ONLY)

#define FLPROG_NETWORK_PROFILE_WIRED_ONLY \
    (FLPROG_NETWORK_BUILD_PROFILE == FLPROG_NETWORK_BUILD_WIRED_ONLY)

#define FLPROG_NETWORK_PROFILE_UNIVERSAL \
    (FLPROG_NETWORK_BUILD_PROFILE == FLPROG_NETWORK_BUILD_UNIVERSAL)

#ifndef FLPROG_NETWORK_COMPILE_WIFI
  #if FLPROG_NETWORK_PROFILE_WIRED_ONLY
    #define FLPROG_NETWORK_COMPILE_WIFI 0
  #else
    #define FLPROG_NETWORK_COMPILE_WIFI 1
  #endif
#endif

#ifndef FLPROG_NETWORK_COMPILE_WIRED
  #if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    #define FLPROG_NETWORK_COMPILE_WIRED 0
  #else
    #define FLPROG_NETWORK_COMPILE_WIRED 1
  #endif
#endif

// ArduinoOTA is Wi-Fi-specific on ESP8266. ESP32 core 3.x uses the unified
// NetworkUDP/NetworkClient stack and can also serve native RMII. External
// SPI Ethernet (for example W5500) is not covered. Override to 0 to omit OTA.
#ifndef FLPROG_NETWORK_COMPILE_OTA
  #if defined(ESP8266)
    #define FLPROG_NETWORK_COMPILE_OTA FLPROG_NETWORK_COMPILE_WIFI
  #elif defined(ESP32)
    #define FLPROG_NETWORK_COMPILE_OTA (FLPROG_NETWORK_COMPILE_WIFI || FLPROG_NETWORK_COMPILE_WIRED)
  #else
    #define FLPROG_NETWORK_COMPILE_OTA 0
  #endif
#endif

#if !FLPROG_NETWORK_COMPILE_WIFI && !FLPROG_NETWORK_COMPILE_WIRED
#error "FLProg_Network: at least one backend family must be compiled"
#endif
