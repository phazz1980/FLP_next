//=================================================================================================
//
//                        				API.h для i2c
//ecoins@mail.ru 21.05.2023г.
//=================================================================================================
// AVR     - one  i2c(Wire)  bus, combined master/slave library;
// SAM3X   - one  i2c(Wire)  bus, combined master/slave library;
// ESP8266 - one  i2c(Wire)  bus, combined master/slave library;
// ESP32S  - two  i2c Master bus(Wire1, Wire2) buses,   Wire.h library.
//           two  i2c Slave  bus(WireSlave1,WireSlave2),ESP32_I2C_Slave library.
// Raspberry Pi Pico - two i2c default;
//			 Wire (SDA,SCL) - 0,1; 4,5;  8, 9; 12,13; 16,17; 20,21;
//			 Wire1(SDA,SCL) - 2,3; 6,7; 10,11; 14,15; 18,19; 26,27;
// STM32   - three i2c(Wire) bus, combined master/slave library;
//           Wire is defined by default 
//           Wire1 needs to define TwoWire Wire1(RT_HW_I2C1_SDA, RT_HW_I2C1_SCL),
//           Wire2 needs to define TwoWire Wire2(RT_HW_I2C2_SDA, RT_HW_I2C2_SCL);
// Seeeduino-six bus i2c(Wire,Wire1-5), combined master/slave library;
//=================================================================================================
// code Error: 0-ok,1-too much data; 2-Nack on address transfer; 3-Nack on data transfer; 4-another error i2c; 
//             5-time out waitting byte; 
//             50-invalid number bus; 51-invalid address device(>=254), 52-bus i2c no Master;
//			   60,61,62-error number sda,scl;
//			   65-No CORE;
//=================================================================================================+			 
//=================================================================================================
// 											0. Объекты c данными I2C
//=================================================================================================
	#if defined(RT_HW_PERMIT_I2C0) 
		RT_HW_STRUCT_I2C 				LinkI2C0; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C1) 
		RT_HW_STRUCT_I2C 				LinkI2C1; 	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C2) 
		RT_HW_STRUCT_I2C 				LinkI2C2; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C3) 
		RT_HW_STRUCT_I2C 				LinkI2C3; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C4) 
		RT_HW_STRUCT_I2C 				LinkI2C4; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C5) 
		RT_HW_STRUCT_I2C 				LinkI2C5; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C6) 
		RT_HW_STRUCT_I2C 				LinkI2C6; 
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C100) 
		RT_HW_STRUCT_I2C 				LinkI2C100; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_I2C101) 
		RT_HW_STRUCT_I2C 				LinkI2C101; 
	#endif

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											1. ФУНКЦИИ НАСТРОЙКИ i2c-CORE
//										FUNC_I2C_000x_CPP_COMMON_"CORE".hpp 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  uint8_t	i2cBegin	 (							uint8_t bus);		//--1.1.Инициализация как Master i2cx;
  uint8_t	i2cBeginSlave(			   uint8_t adr,	uint8_t bus);		//--1.2.Инициализация как Slave  i2cx;
  void 		i2cEnd  	 (							uint8_t bus);		//--1.3.Отключение    			 i2cx;
  void    	i2cSetClock  (uint32_t speed,  			uint8_t bus);		//--1.4.Установка скорости i2cx(аппаратная);
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											2. ФУНКЦИИ ОБМЕНА i2c-CORE
//										FUNC_I2C_000x_CPP_COMMON_"CORE".hpp 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  void  	i2cBeginTransmission(	   uint8_t adr,	uint8_t bus);		//--2.1.Начало     транзакции обмена i2cx;
  uint8_t 	i2cEndTransmission  (            		uint8_t bus);		//--2.2.Завершение транзакции обмена i2cx;
  uint8_t 	i2cFlush            (             		uint8_t bus);		//--2.3.Flush() i2cx;
  uint8_t  	i2cRequestFrom(uint8_t qnt,uint8_t adr,	uint8_t bus, uint8_t sendStop=1);//--1.13.Запрос на чтение qnt байт в  буфер  i2cx;
  uint8_t  	i2cAvailable        (             		uint8_t bus);		//--2.4.Проверка    количество      принятых байт в  буфере i2cx;
  uint8_t  	i2cRead             (             		uint8_t bus);		//--2.5.Чтение                       		 байт из буфера i2cx;
  void     	i2cWrite(uint8_t var,                 	uint8_t bus);		//--2.6.Запись одного байта в i2cx;
  void    	i2cWrite(const uint8_t *arr,uint8_t qnt,uint8_t bus);		//--2.7.Запись qnt    байт  в i2cx;
  void     	i2cWrite(const char    *arr,uint8_t qnt,uint8_t bus);		//--2.8.Запись qnt символов в i2cx;

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											3. ФУНКЦИИ НАСТРОЙКИ i2c LINKI2Cx
//										FUNC_I2C_0020_CPP_COMMON_MCU_1.hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  void		i2cSetPins   (uint8_t sda, uint8_t scl,	uint8_t bus);		//--2.1.Установка пинов RX,TX в объект LinkI2Cx; 
  uint8_t 	i2cGetPinSDA (							uint8_t bus);		//--2.2.Получение пина  SDA из объекта LinkI2Cx;
  uint8_t 	i2cGetPinSCL (							uint8_t bus);		//--2.3.Получение пина  SCL из объекта LinkI2Cx;
  void   	i2cInitParam (							uint8_t bus);		//--2.4.Иниц.параметров (и пинов) по умолчанию для      i2cx;
  void 		i2cInitParamAll();											//--2.5.Иниц.параметров (и пинов) по умолчанию для всех i2cAll;
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											4. ФУНКЦИИ РАБОТЫ С ПАРАМЕТРАМИ LINKI2Cx
//										FUNC_I2C_0020_CPP_COMMON_MCU_1.hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  uint8_t  	i2cCheckBus		(						uint8_t bus);		//--1.1.Проверка наличия 		шины i2cx;
  void    	i2cSetAddr  	(uint8_t adr,			uint8_t bus);		//--1.2.Установка адреса  		шины i2cx (Master =0; Slave >0;);
  uint8_t	i2cGetAddr  	(						uint8_t bus);		//--1.3.Получение адреса  		шины i2cx;
  void    	i2cSetStatus	(uint8_t v, 			uint8_t bus);		//--1.4.Установка статуса 		шины i2cx;
  uint8_t   i2cGetStatus	(						uint8_t bus);		//--1.5.Получение статуса 		шины i2cx; 
  void 		i2cSetCodeErr	(uint8_t v,				uint8_t bus);		//--1.6.Установка кода ошибки 	шины i2cx; 
  void 		i2cClearCodeErr	(						uint8_t bus);		//--1.7.Очистка   кода ошибки 	шины i2cx; 
  uint8_t 	i2cGetCodeErr	(						uint8_t bus);		//--1.8.Получение кода ошибки 	шины i2cx; 
  uint8_t  	i2cCheckBusy 	(						uint8_t bus);		//--1.9.Проверка  занятости  	шины i2cx;
  uint8_t  	i2cSetBusy 		(						uint8_t bus);		//--1.10.Установка занятости  	шины i2cx;
  void   	i2cClearBusy 	(						uint8_t bus);		//--1.11.Очистка   занятости  	шины i2cx;
  void 		i2cSetSpeed 	(uint32_t speed=0,		uint8_t bus=0);		//--1.12.Установка скорости 	шины i2cx;
  uint32_t 	i2cGetSpeed 	(						uint8_t bus);		//--1.13.Получение скорости 	шины i2cx;
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          				5. РАБОТА С УСТРОЙСТВАМИМ i2c
//										FUNC_I2C_0004_CPP_COMMON_DEVICE.hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
// 		    								5.1. Утилиты 
//=================================================================================================
  uint8_t 	i2cSetParam         (RT_HW_STRUCT_I2C_DEV &id); 												//--1.1.1. Установка параметров по умолчанию;  
  uint8_t 	i2cSetParam         (RT_HW_STRUCT_I2C_DEV &id, uint8_t addr, uint8_t bus, uint32_t speed); 	//--1.1.2. Установка параметров;
  uint8_t 	i2cSetParam         (RT_HW_STRUCT_I2C_DEV &id, uint8_t addr, uint8_t bus, uint32_t speed, uint8_t expander, uint8_t channel);
  void  	i2cInitDevice		(RT_HW_STRUCT_I2C_DEV &id);				//--1.2.Инициализация устройства 		
  uint8_t 	i2cBeginFunc		(RT_HW_STRUCT_I2C_DEV &id);				//--1.3.Утилита: начальные   действия перед функциями обмена;
  void 		i2cEndFunc			(RT_HW_STRUCT_I2C_DEV &id);				//--1.4.Утилита: завершающие действия перед функциями обмена;
  void  	i2cFindAddr			(RT_HW_STRUCT_I2C_DEV &id);				//--1.5.Утилита: проверка наличия на шине устройства с адресом id.adr;
  void		i2cFindAddr 		(RT_HW_STRUCT_I2C_DEV &id, uint8_t qnt, uint8_t period);//--проверка устройства qnt-кол-ва успешных проверок, period-период проверок;
 //------------------------------------------------------------------------------------------------
  void  	i2cFindAdr			(RT_HW_STRUCT_I2C_DEV &id){i2cFindAddr(id);};	//--1.5.УСТАРЕВШАЯ !!! Утилита: проверка наличия на шине устройства с адресом id.adr;
//=================================================================================================
// 		    								5.2. Настройка параметров
//=================================================================================================
  void  	i2cBeginTransmission(RT_HW_STRUCT_I2C_DEV &id);				//--2.1.Начало     транзакции обмена с устройством i2cx;
  void  	i2cEndTransmission  (RT_HW_STRUCT_I2C_DEV &id);				//--2.2.Завершение транзакции обмена с устройством i2cx;
  void  	i2cSetChannel		(RT_HW_STRUCT_I2C_DEV &id);				//--2.3.Установка  канала расширителя i2cx(TCA9584A);
  void  	i2cClearChannel		(RT_HW_STRUCT_I2C_DEV &id);				//--2.4.Сброс      канала расширителя i2cx(TCA9584A);
//=================================================================================================
// 		    								5.3. Чтение из устройства
//=================================================================================================
  void 	  	i2cRequestFromFunc	(RT_HW_STRUCT_I2C_DEV &id, uint16_t qnt);							//--3.1.Утилита: Запрос и чтение в буфер i2c qnt байт;
  void 		i2cRead				(RT_HW_STRUCT_I2C_DEV &id);											//--3.2.Чтение    байта в регистр id.bf8;
  void 		i2cReadArr			(RT_HW_STRUCT_I2C_DEV &id, uint8_t *arr, uint16_t qnt);				//--3.3.Чтение qnt байт в массив  arr; 
  void 		i2cReadRegVar		(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg);							//--3.4.Чтение из регистра reg байта в регистр id.bf8;
  void 		i2cREAD				(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg);							//--3.4.1.УСТАРЕВШАЯ аналогичная функция;
  void 		i2cReadRegArr		(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint8_t *arr, uint16_t qnt); //--3.5.Чтение начиная с регистра reg в массив arr qnt байт; 
  void 		i2cREADArr			(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint8_t *arr, uint16_t qnt); //--3.5.1.УСТАРЕВШАЯ аналогичная функция;
//=================================================================================================
// 		    								5.4. Запись в устройство 
//=================================================================================================
  void 		i2cWrite 			(RT_HW_STRUCT_I2C_DEV &id, 				uint8_t v);					//--4.1.Запись одного байта;
  void 		i2cWrite2  			(RT_HW_STRUCT_I2C_DEV &id,              uint8_t v1,   uint8_t v2);	//--4.2.Запись 2-х байт;
  void 		i2cWriteReg			(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint8_t v);					//--4.3.Запись двух байт начиная с регистра reg
  void 		i2cWriteReg2		(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint8_t v1,   uint8_t v2);	//--4.4.Запись двух байт начиная с регистра reg
  void 		i2cWriteV16			(RT_HW_STRUCT_I2C_DEV &id, uint16_t v);								//--4.5.Запись одного 16-ти разрядного слова;
  void 		i2cWriteRegV16		(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint16_t v);				//--4.6.Запись uint16_t в регистр reg;
  void 		i2cWriteArr			(RT_HW_STRUCT_I2C_DEV &id,              uint8_t *arr, uint16_t qnt);//--4.7.Запись qnt байт;
  void 		i2cWriteRegArr		(RT_HW_STRUCT_I2C_DEV &id, uint8_t reg, uint8_t *arr, uint16_t qnt);//--4.8.Запись qnt байт  начиная с регистра qnt;
//=================================================================================================

