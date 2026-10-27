#include "RT_HW_BASE.h"	 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_LINK_I2C)
//=================================================================================================
//										1. СОЗДАНИЕ ОБЪЕКТОВ И ФУНКЦИИ
//=================================================================================================
  #include "API_0901_FUNC_I2C/FUNC_I2C_0001_CPP_COMMON_AVR.hpp"			//--Функции для I2C AVR;
  #include "API_0901_FUNC_I2C/FUNC_I2C_0002_CPP_COMMON_SAM3X.hpp"		//--Функции для I2C SAM3X;
  #include "API_0901_FUNC_I2C/FUNC_I2C_0004_CPP_COMMON_ESP8266.hpp"		//--Функции для I2C ESP8266;
  #include "API_0901_FUNC_I2C/FUNC_I2C_0005_CPP_COMMON_ESP32.hpp"		//--Функции для I2C ESP32;
  #include "API_0901_FUNC_I2C/FUNC_I2C_0006_CPP_COMMON_STM32.hpp"		//--Функции для I2C STM32;  
  #include "API_0901_FUNC_I2C/FUNC_I2C_0007_CPP_COMMON_RP2040.hpp"		//--Функции для I2C RP2040;  
  #include "API_0901_FUNC_I2C/FUNC_I2C_0019_CPP_COMMON_ANON.hpp"		//--Функции для i2C ANON;    
  #include "API_0901_FUNC_I2C/FUNC_I2C_0020_CPP_COMMON_MCU_1.hpp"		//--Функции общие 1 для всех MCU;
  #include "API_0901_FUNC_I2C/FUNC_I2C_0030_CPP_COMMON_DEVICE.hpp" 		//--Функции обмена с устройствами i2c(RT_HW_STRUCT_I2C_DEV); 
//-------------------------------------------------------------------------------------------------
#endif
//-------------------------------------------------------------------------------------------------