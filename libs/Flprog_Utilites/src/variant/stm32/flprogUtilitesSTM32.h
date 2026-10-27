#pragma once

#include <Arduino.h>
#include "RT_HW_BASE.h"

#ifdef RT_HW_CORE_STM32

namespace flprog
{
    size_t freeMemory();
};
#endif