#include "RT_HW_BASE.h"	 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_LINK_UART)
//=================================================================================================
//										1. СОЗДАНИЕ ОБЪЕКТОВ И ФУНКЦИИ
//=================================================================================================
  #include "API_0900_FUNC_UART/FUNC_UART_0001_CPP_COMMON_AVR.hpp"		//--Функции для UART AVR;
  #include "API_0900_FUNC_UART/FUNC_UART_0002_CPP_COMMON_SAM3X.hpp"		//--Функции для UART SAM3X;
  #include "API_0900_FUNC_UART/FUNC_UART_0004_CPP_COMMON_ESP8266.hpp"	//--Функции для UART ESP8266;
  #include "API_0900_FUNC_UART/FUNC_UART_0005_CPP_COMMON_ESP32.hpp"		//--Функции для UART ESP32;
  #include "API_0900_FUNC_UART/FUNC_UART_0006_CPP_COMMON_STM32.hpp"		//--Функции для UART STM32;  
  #include "API_0900_FUNC_UART/FUNC_UART_0007_CPP_COMMON_RP2040.hpp"	//--Функции для UART RP2040;  
  #include "API_0900_FUNC_UART/FUNC_UART_0019_CPP_COMMON_ANON.hpp"		//--Функции для UART ANON;    
  #include "API_0900_FUNC_UART/FUNC_UART_0020_CPP_COMMON_MCU_1.hpp"		//--Функции общие 1 для всех MCU;
  #include "API_0900_FUNC_UART/FUNC_UART_0030_CPP_COMMON_DEVICE.hpp" 	//--Функции для работы с устройствами UART; 
//-------------------------------------------------------------------------------------------------
#endif
//-------------------------------------------------------------------------------------------------