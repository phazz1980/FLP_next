#pragma once

#include <Arduino.h>
#include "RT_HW_BASE.h"

#ifdef RT_HW_CORE_AVR

extern unsigned int __heap_start, *__brkval;

namespace flprog
{
    int16_t freeMemory();
};
#endif