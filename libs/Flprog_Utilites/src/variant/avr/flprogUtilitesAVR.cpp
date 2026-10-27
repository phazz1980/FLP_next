#include "flprogUtilitesAVR.h"

#ifdef RT_HW_CORE_AVR

int16_t flprog::freeMemory()
{
    int16_t free;
    if ((int16_t)__brkval == 0)
    {
        free = (int16_t)&freeMemory - (int16_t)&__heap_start;
    }
    else
    {
        free = (int16_t)&freeMemory - (int16_t)__brkval;
    }
    return free;
}
#endif