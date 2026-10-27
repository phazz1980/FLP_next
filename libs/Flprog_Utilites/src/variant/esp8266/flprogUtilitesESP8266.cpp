#include "flprogUtilitesESP8266.h"
#ifdef RT_HW_CORE_ESP8266
uint32_t flprog::freeMemory()
{
    return ESP.getFreeHeap();
}
#endif