//#################################################################################################
//
//                        				API.h для SPI
//ecoins@mail.ru 18.11.2023г.
//=================================================================================================
// 							ДОСТУПНЫЕ ШИНЫ 												
// AVR  	- SPI;
// SAM3X    - SPI;
// SAMD21   - SPI;
// ESP8266  - SPI;
// ESP32   -  SPI, дополнительно может быть создана шина SPI1x; 
// STM32    - SPI, дополнительно могут быть созданы шины SPI1x,SPI2x,SPI3x,SPI4x,SPI5x,SPI6x;
// RP2040   - SPI,SPI1;
//=================================================================================================
// 											0. Объекты SPI
//=================================================================================================
	#if         defined(RT_HW_PERMIT_SPI0) 
		RT_HW_STRUCT_SPI 		 LinkSPI0; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI1) 
		RT_HW_STRUCT_SPI 		 LinkSPI1; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI2) 
		RT_HW_STRUCT_SPI 		 LinkSPI2; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI3) 
		RT_HW_STRUCT_SPI 		 LinkSPI3; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI4) 
		RT_HW_STRUCT_SPI 		 LinkSPI4; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI5) 
		RT_HW_STRUCT_SPI 		 LinkSPI5; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if         defined(RT_HW_PERMIT_SPI6) 
		RT_HW_STRUCT_SPI 		 LinkSPI6; 
	#endif
//-------------------------------------------------------------------------------------------------

//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//										1. БАЗОВЫЕ ФУНКЦИИ SPI
//									FUNC_SPI_000x_CPP_COMMON_"CORE".hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  uint8_t  	spiBegin    (										  uint8_t bus);	//--1.1.Инициализация SPIn;		
  void     	spiEnd      (										  uint8_t bus);	//--1.2.Отключение    SPIn; 
  void 	 	spiBeginTransaction(uint32_t speed=4000000UL, uint8_t bitOrder=1, uint8_t dataMode=0, uint8_t bus=0);//--1.3.Начало транзакции с параметрами;
  void   	spiEndTransaction  (								  uint8_t bus);	//--1.4.Завершение транзакции;
  uint8_t   spiTransfer(                uint8_t tx, 		      uint8_t bus);	//--1.5.Отправка байта        tx, возвращает принятый байт;
  void  	spiTransferVar(             uint8_t tx,  uint8_t &rx, uint8_t bus);	//--1.6.Отправка байта        tx, прием байта     в  rx;  
  void      spiTransferArr(uint8_t qnt, uint8_t *arr, 			  uint8_t bus);	//--1.7.Отправка qnt байт из arr[],прием qnt байт в arr[];
  void      spiTransferArr(uint8_t qnt, uint8_t *tx, uint8_t *rx, uint8_t bus);	//--1.8.Отправка qnt байт из  tx[],прием qnt байт в  rx[];
  void		spiSetPins  (uint8_t sck, uint8_t miso, uint8_t mosi, uint8_t bus);	//--1.9.Установка пинов RX,TX в объект LinkSPIn; 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//										3. ФУНКЦИИ ОБМЕНА С SPI
//									FUNC_SPI_0020_CPP_COMMON_MCU_1.hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  uint8_t 	spiGetPinSCK (					uint8_t bus);		//--3.1.Получение пина SCK  из объекта 	LinkSPIn;
  uint8_t 	spiGetPinMISO(					uint8_t bus);		//--3.2.Получение пина MISO из объекта 	LinkSPIn;
  uint8_t 	spiGetPinMOSI(					uint8_t bus);		//--3.3.Получение пина MOSI из объекта 	LinkSPIn;
  uint8_t 	spiGetPinSS  (					uint8_t bus);		//--3.4.Получение пина SS 	из объекта 	LinkSPIn;   
  void   	spiInitParam(					uint8_t bus);		//--3.5.Иниц.параметров (и пинов) по умолчанию для SPIn;
  void 		spiInitParamAll();									//--3.6.Иниц.параметров (и пинов) по умолчанию для всех SPI;
//-------------------------------------------------------------------------------------------------
  uint8_t	spiCheckBus  (					uint8_t bus);		//--3.7.Проверка наличия на плате   	SPIn;
//-------------------------------------------------------------------------------------------------  
  uint8_t   spiGetBusy   (					uint8_t bus);		//--3.8.Проверка  флага занятости  		SPIn; 
  uint8_t   spiSetBusy   (					uint8_t bus);		//--3.9.Установка флага занятости  		SPIn;	
  void     	spiClearBusy (					uint8_t bus);		//--3.10.Очистка   флага занятости  	SPIn;
//-------------------------------------------------------------------------------------------------
  void     	spiSetStatus (uint8_t status,   uint8_t bus);		//--3.11.Установка статуса 				SPIn;
  uint8_t  	spiGetStatus (                  uint8_t bus);		//--3.12.Получение статуса 				SPIn;
//-------------------------------------------------------------------------------------------------
  void     	spiSetCodeErr(uint8_t codeErr,  uint8_t bus);		//--3.13.Установка кода ошибки 			SPIn;
  uint8_t  	spiGetCodeErr(                  uint8_t bus);		//--3.14.Чтение    кода ошибки 			SPIn;
//-------------------------------------------------------------------------------------------------
  void     	spiSetSpeed  (uint32_t speed,   uint8_t bus);		//--3.15.Установка скорости 			SPIn;
  uint32_t	spiGetSpeed  (                  uint8_t bus);		//--3.16.Чтение    скорости 			SPIn;
//------------------------------------------------------------------------------------------------
  void     	spiInitCS 	 (uint8_t cs);							//--3.17.Настройка пина СS на вывод;
  void     	spiBeginCS   (uint8_t cs);							//--3.18.Установка пина CS в HIGH;
  void     	spiEndCS	 (uint8_t cs);							//--3.19.Сброс     пина CS в LOW;

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//										4. ФУНКЦИИ ОБМЕНА С УСТРОЙСТВАМИ SPI
//										FUNC_UART_0030_CPP_COMMON_DEVICE.hpp
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  uint8_t 	spiSetParam   	(RT_HW_STRUCT_SPI_DEV &id, uint8_t cs, uint8_t bus=0, uint32_t speed=0);//--4.1.Установка cs,bus,speed;
  void  	spiInitCS 		(RT_HW_STRUCT_SPI_DEV &id);											//--4.2.Настройка пина СS на вывод;
  void  	spiBeginCS		(RT_HW_STRUCT_SPI_DEV &id);											//--4.3.Установка пина СS в LOW;
  void  	spiEndCS  		(RT_HW_STRUCT_SPI_DEV &id); 										//--4.4.Установка пина СS в HIGH;
  void  	spiInitDevice 	(RT_HW_STRUCT_SPI_DEV &id);											//--4.5.Настройка устройства;
  void  	spiBeginTransaction(RT_HW_STRUCT_SPI_DEV &id);										//--4.6.Начало     транзакции;
  void  	spiEndTransaction  (RT_HW_STRUCT_SPI_DEV &id);										//--4.7.Завершение транзакции;
//---------------------------------------------------------------------------------------------------
  uint8_t   spiTransferVar	(RT_HW_STRUCT_SPI_DEV &id, uint8_t  tx);							//--4.10.Запись байта tx, возвращает принятый байт;
  void  	spiTransfer   	(RT_HW_STRUCT_SPI_DEV &id, uint8_t  tx, uint8_t &rx);				//--4.11.Запись байта tx, чтение байта в rx;
  void    	spiWrite	  	(RT_HW_STRUCT_SPI_DEV &id, uint8_t  tx); 							//--4.12.Запись байта   tx;
   void    	spiWriteReg	  	(RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t  tx); 				//--4.13.Запись регистра reg и байта tx; 
  void  	spiRead		  	(RT_HW_STRUCT_SPI_DEV &id,              uint8_t &rx);				//--4.15.Чтение байта в rx;
//-------------------------------------------------------------------------------------------------
  void  	spiTransferArr	(RT_HW_STRUCT_SPI_DEV &id, uint8_t *arr,             uint8_t qnt=1);//--4.21.Запись qnt байт из arr[] и чтение в arr[];
  void  	spiTransferArr	(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx, uint8_t *rx, uint8_t qnt=1);//--4.22.Запись qnt байт из  tx[] и чтение в  rx[];
  void      spiWriteArr   	(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx,              uint8_t qnt=1);//--4.23.Запись qnt байт из  tx[];
  void      spiWriteRegArr  (RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t *tx, uint8_t qnt=1);//--4.24.Запись регистра+qnt байт из  tx[];
  void  	spiReadArr    	(RT_HW_STRUCT_SPI_DEV &id, uint8_t *rx,              uint8_t qnt=1);//--4.25.Чтение qnt байт           в  rx[];
//-------------------------------------------------------------------------------------------------

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          			5.ФУНЦИИ ОБМЕНА СЛОВАМИ (qnt=1-4)
//										FUNC_UART_0030_CPP_COMMON_DEVICE.hpp	
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  void  	spiTransferShift(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx,uint8_t qnt=1);//--5.1.Обмен  словами длиной 1,2,3 или 4 байта;
  void  	spiWriteShift	(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, 			 uint8_t qnt=1);//--5.2.Запись словами длиной 1,2,3 или 4 байта;
  void  	spiReadShift	(RT_HW_STRUCT_SPI_DEV &id, 				uint32_t &rx,uint8_t qnt=1);//--5.3.Чтение словами длиной 1,2,3 или 4 байта;
//-------------------------------------------------------------------------------------------------  
//  void		spi74HC165		(RT_HW_STRUCT_SPI_DEV &id, 				uint32_t &rx,uint8_t qnt=1);//--5.10.Чтение из 74HC165;
//  void		spi74HC595		(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, 			 uint8_t qnt=1);//--5.11.Запись  в 74HC595;	 
  void		spiDM13C		(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx,uint8_t qnt=1);//--5.12.Запись  в DM13C;
 //+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++ 
 
