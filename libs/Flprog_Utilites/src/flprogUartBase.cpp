#include "flprogUartBase.h"

uint32_t flprog::speedFromCode(uint8_t code)
{
    switch (code)
    {
    case FLPROG_SPEED_300:
        return 300;
        break;
    case FLPROG_SPEED_600:
        return 600;
        break;
    case FLPROG_SPEED_1200:
        return 1200;
        break;
    case FLPROG_SPEED_2400:
        return 2400;
        break;
    case FLPROG_SPEED_4800:
        return 4800;
        break;
    case FLPROG_SPEED_9600:
        return 9600;
        break;
    case FLPROG_SPEED_14400:
        return 14400;
        break;
    case FLPROG_SPEED_19200:
        return 19200;
        break;
    case FLPROG_SPEED_28800:
        return 28800;
        break;
    case FLPROG_SPEED_38400:
        return 38400;
        break;
    case FLPROG_SPEED_57600:
        return 57600;
        break;
    case FLPROG_SPEED_115200:
        return 115200;
        break;
    default:
        return 9600;
        break;
    }
}

uint8_t flprog::codeFromSpeed(int32_t speed)
{
    switch (speed)
    {
    case 300:
        return FLPROG_SPEED_300;
        break;
    case 600:
        return FLPROG_SPEED_600;
        break;
    case 1200:
        return FLPROG_SPEED_1200;
        break;
    case 2400:
        return FLPROG_SPEED_2400;
        break;
    case 4800:
        return FLPROG_SPEED_4800;
        break;
    case 9600:
        return FLPROG_SPEED_9600;
        break;
    case 14400:
        return FLPROG_SPEED_14400;
        break;
    case 19200:
        return FLPROG_SPEED_19200;
        break;
    case 28800:
        return FLPROG_SPEED_28800;
        break;
    case 38400:
        return FLPROG_SPEED_38400;
        break;
    case 57600:
        return FLPROG_SPEED_57600;
        break;
    case 115200:
        return FLPROG_SPEED_115200;
        break;
    }
    return FLPROG_SPEED_9600;
}

int flprog::serialCodeForParametrs(byte portDataBits, byte portStopBits, byte portParity)
{
    int code = 0;
    if (portStopBits == 2)
    {
        code = code + 8;
    }
    if (portDataBits == 6)
    {
        code = code + 2;
    }
    if (portDataBits == 7)
    {
        code = code + 4;
    }
    if (portDataBits == 8)
    {
        code = code + 6;
    }
    if (portParity == 1)
    {
        code = code + 32;
    }
    if (portParity == 2)
    {
        code = code + 48;
    }
    return code;
}