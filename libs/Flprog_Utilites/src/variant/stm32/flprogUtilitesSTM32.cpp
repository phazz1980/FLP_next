#include "flprogUtilitesSTM32.h"
#ifdef RT_HW_CORE_STM32
extern char _end;

size_t flprog::freeMemory()
{
    char top;
    return &top - &_end;
}
#endif