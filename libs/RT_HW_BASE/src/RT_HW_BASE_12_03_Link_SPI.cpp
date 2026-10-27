#include "RT_HW_BASE.h"
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_LINK_SPI)
//=================================================================================================
//										1. СОЗДАНИЕ ОБЪЕКТОВ И ФУНКЦИИ
//=================================================================================================
  #include "API_0902_FUNC_SPI/FUNC_SPI_0001_CPP_COMMON_AVR.hpp"			//--Функции для SPI AVR;
  #include "API_0902_FUNC_SPI/FUNC_SPI_0002_CPP_COMMON_SAM3X.hpp"		//--Функции для SPI SAM3X;
  #include "API_0902_FUNC_SPI/FUNC_SPI_0004_CPP_COMMON_ESP8266.hpp"		//--Функции для SPI ESP8266;
  #include "API_0902_FUNC_SPI/FUNC_SPI_0005_CPP_COMMON_ESP32.hpp"		//--Функции для SPI ESP32;
  #include "API_0902_FUNC_SPI/FUNC_SPI_0006_CPP_COMMON_STM32.hpp"		//--Функции для SPI STM32;  
  #include "API_0902_FUNC_SPI/FUNC_SPI_0007_CPP_COMMON_RP2040.hpp"		//--Функции для SPI RP2040;  
  #include "API_0902_FUNC_SPI/FUNC_SPI_0019_CPP_COMMON_ANON.hpp"		//--Функции для SPI ANON;    
  #include "API_0902_FUNC_SPI/FUNC_SPI_0020_CPP_COMMON_MCU_1.hpp"		//--Функции общие 1 для всех MCU;
  #include "API_0902_FUNC_SPI/FUNC_SPI_0030_CPP_COMMON_DEVICE.hpp" 		//--Функции обмена с устройствами SPI(RT_HW_STRUCT_SPI_DEV); 
//-------------------------------------------------------------------------------------------------
#endif
//-------------------------------------------------------------------------------------------------