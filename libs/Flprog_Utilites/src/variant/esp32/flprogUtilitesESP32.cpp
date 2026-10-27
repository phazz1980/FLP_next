#include "flprogUtilitesESP32.h"
#ifdef RT_HW_CORE_ESP32
uint32_t flprog::freeMemory()
{
    return ESP.getFreeHeap();
}
#endif