#include "flprogUtilitesRP2040.h"
#ifdef RT_HW_CORE_RP2040

int16_t flprog::freeMemory()
{
    struct mallinfo mi = mallinfo();
    return mi.fordblks;
}
#endif