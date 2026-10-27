#pragma once

#include <Arduino.h>
#include "RT_HW_BASE.h"

#ifdef RT_HW_CORE_STM32
#define FLPROG_UTILITES_CORE_SELECT_FOR_ANON
#endif

#ifdef RT_HW_CORE_RP2040
#define FLPROG_UTILITES_CORE_SELECT_FOR_ANON
#endif

#ifdef RT_HW_CORE_ESP8266
#define FLPROG_UTILITES_CORE_SELECT_FOR_ANON
#endif

#ifdef RT_HW_CORE_ESP32
#define FLPROG_UTILITES_CORE_SELECT_FOR_ANON
#endif

#ifdef RT_HW_CORE_AVR
#define FLPROG_UTILITES_CORE_SELECT_FOR_ANON
#endif

#ifndef FLPROG_UTILITES_CORE_SELECT_FOR_ANON

namespace flprog
{
    uint32_t freeMemory();
};

#endif