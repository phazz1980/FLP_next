//#################################################################################################
//
//                        				API.h для SPI
//ecoins@mail.ru 18.11.2023г.
//=================================================================================================
// 							ДОСТУПНЫЕ ШИНЫ 													[РАЗДЕЛ  1]  (line 87)
// AVR  	- SPI;
// ESP8266  - SPI;
// ESP32S   - SPI,SPI1; 
// SAM3     - SPI;
// STM32    - SPI, дополнительно создаются шины SPI1x,SPI2x,SPI3x,SPI4x,SPI5x;
// RP2040   - SPI,SPI1;
//   Для каждой из доступной шины создается объект LinkSPIx [RT_HW_STRUCT_SPI LinkSPIx]	[РАЗДЕЛ  1].
//   (описание структуры ниже - line 91), в котором содержатся параметры шины (номера пинов, скорость и пр.).
//=================================================================================================
// 2.Для чтения/записи параметров LinkSPIx предусмотрены необходимые функции     			[РАЗДЕЛ  2]   (line 154)
//=================================================================================================
//                    		БАЗОВЫЕ ФУНКЦИИ ДЛЯ РАБОТЫ С ШИНОЙ SPIx.						[РАЗДЕЛЫ 3,4] (line 646,865)
//-------------------------------------------------------------------------------------------------
// 3.Для инициализации параметров предусмотрены  функции 									[Раздел 3]	  (line 646) 
// 3.1.  spiInitParam(										uint8_t bus=0)	//==Инициализация параметров          шины SPIx из описания контроллера  Board;
// 3.3.  spiInitParamAll()													//==Инициализация параметров для всех шины SPIx из описаний контроллеров Board;					
// 3.3.  spiSetPins(uint8_t sck, uint8_t miso, uint8_t mosi, uint8_t bus=0) //==Установка пинов sck,miso,mosi для выбранной шины SPI(bus);
//-------------------------------------------------------------------------------------------------
// 4.Базовые функции из библиотеки SPI.h(для разных контроллеров библиотеки SPI.h разные).	[Раздел 4]	  (line 865) 
// 4.1.spiBeginTransaction(uint32_t speed, uint8_t bitOrder, uint8_t dataMode, uint8_t bus=0);	//==Начало транзакции с параметрами speed,bitOrder,dataMode;
// 4.2.spiBeginTransaction(uint32_t speed, uint8_t bus=0);	//==Начало транзакции с параметрами speed         bitOrder=MSBFIRST,dataMode=MODE0;
// 4.3.spiBeginTransaction(uint8_t bus=0);					//==Начало транзакции с параметрами speed=default,bitOrder=MSBFIRST,dataMode=MODE0;
// 4.4.spiEndTransaction  (uint8_t bus=0);					//==Конец транзакции;
// 4.5.spiTransfer        (uint8_t  val,   uint8_t bus=0);	//==Передача байта. Возвращает принятый байт;

// 4.6.spiBegin           (uint8_t bus=0);					//==Инициализации шины. Функция использует параметры из объекта LinkSPIx(пины и др.) и выполняется при id.status=0. 
// 4.7.spiEnd			  (uint8_t bus=0);														//==Отключение шины SPIx;
//=================================================================================================
//                    		БАЗОВЫЕ ФУНКЦИИ ДЛЯ РАБОТЫ С УСТРОЙСТВАМИ НА ШИНЕ SPIx.					[РАЗДЕЛЫ 5-8] (line 646) 
//	Рекомендуется для использования именно этот набор функций, как более удобный. 
//  Устройство описывается структурой  src\BASE_002_Sruct\Struct_220_Link
//	struct RT_HW_STRUCT_SPI_DEV{
//	uint8_t  custom=0;				//--1.Флаг настройки параметров;
//	uint8_t  status=0;				//--2.Статус устройства: =0 не инициализировано; =1 Master; =2 Slave;
//	uint8_t  link=0;				//--3.Флаг успешной(=1) завершения транзакции;
//	uint8_t  codeErr=0,err=0;		//--4.Код ошибки;
//	uint8_t  bus=0;					//--5.Номер шины;
//	uint8_t  cs=255; 				//--6.Номер пина cs;
//	uint8_t  bitOrder=1;			//--7.Порядок следования битов  (=1 - впереди старший бит, в том числе и uint8_t,uint16_t,uint32_t);
//	uint8_t  dataMode=0;			//--8.Параметр dataMode для шины SPI;
//	uint8_t  qnt=1;					//--9.Кол-во передаваемых байт;
//	uint8_t  bf8,bfx;				//--11.Рабочий регистр;
//	uint32_t bf32;					//--12.Рабочий регистр;
//	int8_t   idx;					//--13.Рабочий индекс (-128: +127);
//	uint8_t  t1=0,t2=0,t3=0;		//--14.Задержки времени(mcs): t1- перед устновкой cs=0; t2-перед установкой cs=1;  t3-после установки cs=1;
//  uint32_t speed=RT_HW_SPI_SPEED;	//--15.Скорость шины в Гц;	
//};
//-------------------------------------------------------------------------------------------------
//  5.Функции настройки параметров устройства SPI.											[Раздел 5] (line 865) 
//    Перед примененем функций работы с устройствами на шине SPI, параметры устройство должны быть настроены функциями:
//  spiSetParam(   RT_HW_STRUCT_SPI_DEV &id, uint8_t cs, uint8_t bus, uint32_t speed=0)   - установка cs, номер шины,скорости. 
//                                                                     Возвращает параметр id.custom=1; Для повторной настройки нужно сбросить id.custom=0; 
//  spiSetParam(   RT_HW_STRUCT_SPI_DEV &id, uint8_t cs)                                  - аналогичная установка cs, номер шины=0, скорость=0 (при инициализации RT_HW_SPI_SPEED->id.speed); 
//-------------------------------------------------------------------------------------------------
//  6.Настройка устройства.																	[Раздел 6] (line 883) 
//  Перед примененем функций работы с устройствами на шине SPI, после настройки параметров устройства, устройство должно быть настроено функцией:
//  spiInitDevice( RT_HW_STRUCT_SPI_DEV &id)											  - настройка устройства;
//-------------------------------------------------------------------------------------------------
//	7.Функции обмена с устройствами на шине SPI:											[Раздел 7] (line 903) 
//  spiTransfer   (RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx, uint8_t qnt=1) - обмен  от 1 до 4 байтами;
//  spiWrite      (RT_HW_STRUCT_SPI_DEV &id, uint32_t tx,               uint8_t qnt=1) - запись от 1 до 4 байтов;
//  spiRead       (RT_HW_STRUCT_SPI_DEV &id,                            uint8_t qnt=1) - чтение от 1 до 4 байт;
//  spiREAD       (RT_HW_STRUCT_SPI_DEV &id, uint8_t reg)                              - чтение из регистра reg 
//                                                                   (отправляется номер регистра и возвращает результат в буфере id.bf32);
//  spiREADArr    (RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t *arr, uint8_t qnt=1) - чтение массива длиной qnt начиная с регистра reg 
//                                                                   (отправляется номер регистра и возвращает результат в массиве arr);
//  spiRead74HC165(RT_HW_STRUCT_SPI_DEV &id,                            uint8_t qnt=1) - чтение от 1 до 4 байт из каскадно-соединенных сдвиговых регистров 74HC165;
//  spiTransferArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx, uint8_t *rx,  uint8_t qnt=1) - обмен  массива tx,rx длиной qnt;
//  spiWriteArr   (RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx,               uint8_t qnt=1) - запись массива tx    длиной qnt;
//  spiReadArr    (RT_HW_STRUCT_SPI_DEV &id,              uint8_t *rx,  uint8_t qnt=1) - чтение массива    rx длиной qnt;
//
//-------------------------------------------------------------------------------------------------
//  8.Внутренние функции.																	[Раздел 8]  (line 1041) 
//  spiBeginTransaction(RT_HW_STRUCT_SPI_DEV &id) - начало    транзакции и установка пина CS LOW  после задержки=id.t1;
//  spiEndTransaction  (RT_HW_STRUCT_SPI_DEV &id) - окончание транзакции и установка пина CS HIGH после задержки=id.t2, далее дополнительная задержка id.t3;
//-------------------------------------------------------------------------------------------------

//=================================================================================================
//#################################################################################################

//=================================================================================================
//#define  RT_HW_LINK_SPI		//--Используется для отладки
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									0.СТРУКТУРА  ДАННЫХ ДЛЯ SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
struct RT_HW_STRUCT_SPI {
uint8_t  custom=0;				//--1.Флаг настройки: =1 -настроены параметры шины;
uint8_t  status=0;				//--2.Состояние шины: =0 -шина не иницирована; 
								//                    =1 -настроена как Master[SPIx.begin()]; =2 -Настроена 
								//                    =2 -настроена как Slave[для разных контроолерах по разному];
uint8_t  codeErr=0;				//--3.Код шины; 	
uint8_t  busy=0;  				//--4.Флаг занятости шины;
//-------------------------------------------------------------------------------------------------
uint8_t  sck=255; 				//--5.номер пина SCK;
uint8_t  mosi=255; 				//--6.номер пина MOSI;
uint8_t  miso=255; 				//--7.номер пина MISO;	
uint8_t  ss=0; 	    			//--8.номер пина SS/CS: для Slave=ss, для Master=сs;
//-------------------------------------------------------------------------------------------------
uint8_t  idx=0;					//--9.рабочий индекс;
uint8_t  step;					//--10.Номер шага для внутренних фукций(рабочая переменная);
uint8_t  bf8;					//--11.Рабочая байтовая переменная;
uint32_t timeBegin;				//--12.Рабочая переменная - для работы со временем во внутренних функциях;
//-------------------------------------------------------------------------------------------------
uint32_t speed=RT_HW_SPI_SPEED;	//--12.Скорость шины в гц.	
};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//				1.ОБЪЕКТЫ ДАННЫХ для SPI [со структурой RT_HW_STRUCT_SPI] c именем LinkSPIx.
// #define RT_HW_PERMITx - прописывается в описания контроллера Board_xxx_aaa
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									1.0. SPI0
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI0)			  
RT_HW_STRUCT_SPI 	LinkSPI0;
#endif
//=================================================================================================
//									1.1. SPI1
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI1)		  
RT_HW_STRUCT_SPI 	LinkSPI1;   //--for STM32=SPI2;
#endif
//=================================================================================================
//									1.2. SPI2
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI2)		  
RT_HW_STRUCT_SPI 	LinkSPI2;   //--for STM32=SPI3;						
#endif	
//=================================================================================================
//									1.3. SPI3
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI3)		  
RT_HW_STRUCT_SPI 	LinkSPI3;   //--for STM32=SPI4;						
#endif
//=================================================================================================
//									1.4. SPI4
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI4)		  
RT_HW_STRUCT_SPI 	LinkSPI4;   //--for STM32=SPI5;						
#endif
//=================================================================================================
//									1.5. SPI5
//=================================================================================================
#if defined(RT_HW_PERMIT_SPI5)		  
RT_HW_STRUCT_SPI 	LinkSPI5;   //--for STM32=SPI6;						
#endif
//----------------------------------------------------------------------------------------- 

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//			    2.ФУНКЦИИ для работы с данными LinkSPIx)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									2.1.Проверка наличия шины
//=================================================================================================
uint8_t	 spiCheckBus(uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return 1;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return 1;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return 1;}
#endif	
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return 1;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return 1;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return 1;}
#endif
return 0;
};
//=================================================================================================
//									2.2. Проверка занятости шины SPIx 
//=================================================================================================
uint8_t  spiCheckBusy(uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.busy;} 
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.busy;} 
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.busy;} 
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.busy;} 
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.busy;} 
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.busy;} 
#endif
return 0;
};
//=================================================================================================
//									2.3. Захват шины SPIx 
//=================================================================================================
uint8_t  spiSetBusy  (uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {if(LinkSPI0.busy==0){LinkSPI0.busy=1;} if(LinkSPI0.busy==0){LinkSPI0.busy=1;} return LinkSPI0.busy;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {if(LinkSPI1.busy==0){LinkSPI1.busy=1;} if(LinkSPI1.busy==0){LinkSPI1.busy=1;} return LinkSPI1.busy;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {if(LinkSPI2.busy==0){LinkSPI2.busy=1;} if(LinkSPI2.busy==0){LinkSPI2.busy=1;} return LinkSPI2.busy;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {if(LinkSPI3.busy==0){LinkSPI3.busy=1;} if(LinkSPI3.busy==0){LinkSPI3.busy=1;} return LinkSPI3.busy;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {if(LinkSPI4.busy==0){LinkSPI4.busy=1;} if(LinkSPI4.busy==0){LinkSPI4.busy=1;} return LinkSPI4.busy;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {if(LinkSPI5.busy==0){LinkSPI5.busy=1;} if(LinkSPI5.busy==0){LinkSPI5.busy=1;} return LinkSPI5.busy;}
#endif
return 0;
};
//=================================================================================================
//									2.4. Сброс занятости шины SPIx 
//=================================================================================================
void     spiClearBusy(uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.busy=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.busy=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.busy=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.busy=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.busy=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.busy=0; return;}
#endif
};
//=================================================================================================
//									2.5. Получение номера пина SCK
//=================================================================================================
uint8_t spiGetPinSCK(uint8_t bus=0){						
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.sck;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.sck;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.sck;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.sck;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.sck;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.sck;}
#endif
return 255;
};
//=================================================================================================
//									2.6. Установка номера пина SCK
//=================================================================================================
void 	spiSetPinSCK(uint8_t sck,   uint8_t bus=0){
(void)sck; (void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.sck=sck; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.sck=sck; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.sck=sck; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	 
if(bus==3) {LinkSPI3.sck=sck; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.sck=sck; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.sck=sck; return;}
#endif
};
//=================================================================================================
//									2.7. Получение номера пина MISO
//=================================================================================================
uint8_t spiGetPinMISO(              uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.miso;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.miso;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.miso;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.miso;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.miso;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.miso;}
#endif
return 255;
};
//=================================================================================================
//									2.8. Установка номера пина MISO
//=================================================================================================
void    spiSetPinMISO(uint8_t miso, uint8_t bus=0){
(void)miso; (void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.miso=miso; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.miso=miso; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.miso=miso; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.miso=miso; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.miso=miso; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.miso=miso; return;}
#endif
};
//=================================================================================================
//									2.9. Получение номера пина MOSI
//=================================================================================================
uint8_t spiGetPinMOSI(              uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.mosi;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.mosi;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.mosi;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.mosi;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.mosi;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.mosi;}
#endif
return 255;
};
//=================================================================================================
//									2.10. Установка номера пина MOSI
//=================================================================================================
void    spiSetPinMOSI(uint8_t mosi, uint8_t bus=0){
(void)mosi; (void)bus; 
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.mosi=mosi; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.mosi=mosi; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.mosi=mosi; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.mosi=mosi; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.mosi=mosi; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.mosi=mosi; return;}
#endif
};
//=================================================================================================
//									2.11. Получение номера пина SS
//=================================================================================================
uint8_t spiGetPinSS (uint8_t bus=0){	
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.ss;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.ss;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.ss;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.ss;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.ss;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.ss;}
#endif
return 255;
};
//=================================================================================================
//									2.12. Установка номера пина SS
//=================================================================================================
void    spiSetPinSS (uint8_t ss,    uint8_t bus=0){
(void)ss; (void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.ss=ss; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.ss=ss; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.ss=ss; return;}
#endif
};
//=================================================================================================
//									2.14. Получение номера скорости(Гц)
//=================================================================================================
uint32_t spiGetSpeed(                uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.speed;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.speed;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.speed;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.speed;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.speed;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.speed;}
#endif
return RT_HW_SPI_SPEED;
};
//=================================================================================================
//									2.15. Установка номера скорости(Гц)
//=================================================================================================
void  	 spiSetSpeed(uint32_t speed, uint8_t bus=0){
(void)bus;
if(speed==0){speed=RT_HW_SPI_SPEED;}
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.speed=speed; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.speed=speed; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.speed=speed; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.speed=speed; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.speed=speed; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.speed=speed; return;}
#endif
};
//=================================================================================================
//									2.16. Получение состояния шины SPIx
//=================================================================================================
uint8_t  spiGetStatus(              uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.status;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.status;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.status;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.status;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.status;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.status;}
#endif
//-------------------------------------
return 0;};
//=================================================================================================
//									2.17. Установка состояния шины SPIx
//=================================================================================================
void     spiSetStatus(uint8_t var,  uint8_t bus=0){
(void)var; (void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.status=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.status=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.status=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.status=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.status=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.status=var; return;}
#endif
};
//=================================================================================================
//									2.18. Получение кода ошибок
//=================================================================================================
uint8_t  spiGetCodeErr(             uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.codeErr;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.codeErr;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.codeErr;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.codeErr;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.codeErr;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.codeErr;}
#endif
return 90;
};
//=================================================================================================
//									2.19. Установка кода ошибок
//=================================================================================================
void     spiSetCodeErr(uint8_t var, uint8_t bus=0){
(void)bus; (void)var;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.codeErr=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.codeErr=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.codeErr=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.codeErr=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.codeErr=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.codeErr=var; return;}
#endif
};
//=================================================================================================
//									2.20. Очистка кода ошибок
//=================================================================================================
void     spiClearCodeErr(uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.codeErr=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.codeErr=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.codeErr=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.codeErr=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.codeErr=0; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.codeErr=0; return;}
#endif
}; 
//=================================================================================================
//									2.21. Получение рабочего буфера [uint8_t bf8]
//=================================================================================================
uint8_t  spiGetBuff (uint8_t bus=0){
(void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {return LinkSPI0.bf8;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {return LinkSPI1.bf8;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {return LinkSPI2.bf8;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {return LinkSPI3.bf8;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {return LinkSPI4.bf8;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {return LinkSPI5.bf8;}
#endif
return 0;
};
//=================================================================================================
//									2.22. Установка рабочего буфера [uint8_t bf8]
//=================================================================================================
void     spiSetBuff (uint8_t var,   uint8_t bus=0){
(void)var; (void)bus;
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0) {LinkSPI0.bf8=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI1)	
if(bus==1) {LinkSPI1.bf8=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI2)	
if(bus==2) {LinkSPI2.bf8=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI3)	
if(bus==3) {LinkSPI3.bf8=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI4)	
if(bus==4) {LinkSPI4.bf8=var; return;}
#endif
#if defined(RT_HW_PERMIT_SPI5)	
if(bus==5) {LinkSPI5.bf8=var; return;}
#endif
};

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          	3.ФУНКЦИИ ИНИЦИАЛИЗАЦИИ ПАРАМЕТРОВ В ОБЪЕКТАХ LinkSPIx
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
// 		    			3.1.Инициализация параметров шины SPIx из описания контроллера Board:
//             cбрасываются переменные, из описания контроллера Boards считываются номера пинов sck,miso,mosi,ss;  
//             устанавливается скорость для выбранного контроллера,                 
//=================================================================================================
void  spiInitParam(uint8_t bus=0){
(void)bus;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)
if(bus==0) {LinkSPI0.busy=LinkSPI0.status=LinkSPI0.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI0.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,0);	//--set pin SCK; 
			LinkSPI0.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,1);	//--set pin MISO;
			LinkSPI0.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,2);	//--set pin MOSI; 
			LinkSPI0.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,3);	//--set pin SS;
			LinkSPI0.speed=RT_HW_SPI_SPEED;}            	    //--set speed;
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)
if(bus==1) {LinkSPI1.busy=LinkSPI1.status=LinkSPI1.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI1.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,4);	//--set pin SCK; 
			LinkSPI1.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,5);	//--set pin MISO;
			LinkSPI1.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,6);	//--set pin MOSI; 
			LinkSPI1.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,7);	//--set pin SS;
			LinkSPI1.speed=RT_HW_SPI_SPEED;}					//--set speed;			
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)
if(bus==2) {LinkSPI2.busy=LinkSPI2.status=LinkSPI2.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI2.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,8);	//--set pin SCK; 
			LinkSPI2.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,9);	//--set pin MISO;
			LinkSPI2.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,10);	//--set pin MOSI; 
			LinkSPI2.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,11);	//--set pin SS;
			LinkSPI2.speed=RT_HW_SPI_SPEED;}				    //--set speed;
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)
if(bus==3) {LinkSPI3.busy=LinkSPI3.status=LinkSPI3.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI3.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,12);	//--set pin SCK; 
			LinkSPI3.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,13);	//--set pin MISO;
			LinkSPI3.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,14);	//--set pin MOSI; 
			LinkSPI3.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,15);	//--set pin SS;
			LinkSPI3.speed=RT_HW_SPI_SPEED;}				    //--set speed;
#endif 
//--------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)
if(bus==4) {LinkSPI4.busy=LinkSPI4.status=LinkSPI4.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI4.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,16);	//--set pin SCK; 
			LinkSPI4.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,17);	//--set pin MISO;
			LinkSPI4.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,18);	//--set pin MOSI; 
			LinkSPI4.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,19);	//--set pin SS;
			LinkSPI4.speed=RT_HW_SPI_SPEED;}				    //--set speed;
#endif
//--------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)
if(bus==5) {LinkSPI5.busy=LinkSPI5.status=LinkSPI5.codeErr=0;	//--clear busy,status,codeErr;
			LinkSPI5.sck  =readArrPGM(RT_HW_PGM_PIN_SPI_ID,20);	//--set pin SCK; 
			LinkSPI5.miso =readArrPGM(RT_HW_PGM_PIN_SPI_ID,21);	//--set pin MISO;
			LinkSPI5.mosi =readArrPGM(RT_HW_PGM_PIN_SPI_ID,22);	//--set pin MOSI; 
			LinkSPI5.ss   =readArrPGM(RT_HW_PGM_PIN_SPI_ID,23);	//--set pin SS;
			LinkSPI5.speed=RT_HW_SPI_SPEED;}				    //--set speed;
#endif
};
//=================================================================================================
// 		    			3.2.Инициализация параметров всех шины SPIx из описания контроллера Board:
//=================================================================================================
void  spiInitParamAll(){for(uint8_t i=0; i<=6; i++){spiInitParam(i);}}; 
//=================================================================================================
//						3.3.Установка пинов sck,miso,mosi для выбранной шины SPI(bus)
//                                      Используется для ручной корректировки номеров пинов шины SPIx; 
//  1.Для AVR,SAM - конструктивно номера пинов не изменны, действияне выполняются;
//  2.Для ESP8266 - настройка выполняется только при условии не совпадении номеров пинов;
//  3.Для ESP32   - настройка выполняется только при условии не совпадении номеров пинов и номера пинов <34;
//  4.Для RP2040  - проверяетcя на допустимость номера пинов sck,miso,mosi:
//					для SPI0 -  2,0, 5;  6, 4, 7; 18,16,19; 22,20,23;
//					для SPI1 - 14,8,15; 10,12,11;
//  5.Для STM32   - настройка выполняется только при условии не совпадении номеров пинов. 
//                  Ответсвенность за правильный выбо комбинации пинов лежит на пользователе;
// SCK : 2, 6, 18, 22
// MISO: 0, 4, 16, 20
// MOSI: 3, 7, 19, 23 
//++++SPI (SCK,MISO,MOSI) -  2, 4, 5;  
//                           6, 8, 7;
//=================================================================================================
void  spiSetPins(uint8_t sck, uint8_t miso, uint8_t mosi, uint8_t bus=0){ 
(void)sck; (void)miso; (void)mosi; (void)bus; uint8_t bf; (void)bf;
//=================================================================================================
#if defined(RT_HW_CORE_AVR) || defined(RT_HW_CORE_SAM)
#if defined(RT_HW_PERMIT_SPI0)
if(bus==0){return;}
return;
#endif
#endif
//=================================================================================================
#if defined(RT_HW_CORE_ESP8266)
#if defined(RT_HW_PERMIT_SPI0)
if(bus==0){
if((sck==miso) || (sck==mosi) || (miso==mosi)) {spiSetCodeErr(60,bus);   return;}
if(miso==16)								   {spiSetCodeErr(61,bus);   return;}
spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus); return;
}
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
#if defined(RT_HW_CORE_ESP32)
#if defined(RT_HW_PERMIT_SPI0)
if(bus==0){
if((sck==miso) || (sck==mosi) || (miso==mosi)) {spiSetCodeErr(60,bus);   return;}
if(sck >=34) 								   {spiSetCodeErr(61,bus);   return;}
if(mosi>=34)                                   {spiSetCodeErr(62,bus);   return;}
spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus); return;
}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)
if(bus==1){
if((sck==miso) || (sck==mosi) || (miso==mosi)) {spiSetCodeErr(60,bus);   return;}
if(sck >=34) 								   {spiSetCodeErr(61,bus);   return;}
if(mosi>=34)                                   {spiSetCodeErr(62,bus);   return;}
spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus); return;
}
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
#if defined(RT_HW_CORE_RP2040)
spiSetBuff(0,bus);
//  SCK_SPI0 = 2,  6, 18, 22
// MISO_SPI0 = 0,  4, 16, 20
// MOSI_SPI0 = 3,  7, 19, 23

//  SCK_SPI1 =10, 14, 26
// MISO_SPI1 = 8, 12, 24, 28
// MOSI_SPI1 =11, 15, 27
//+++++++++++++++++++++++++++++++++++++++++++++
#if defined(RT_HW_PERMIT_SPI0)
if(bus==0){
bf=0;	
if(!bf){if((sck== 2) && (miso== 0) && (mosi== 3)){bf=1;}}
if(!bf){if((sck== 6) && (miso== 4) && (mosi== 7)){bf=1;}}
if(!bf){if((sck==18) && (miso==16) && (mosi==19)){bf=1;}}
if(!bf){if((sck==22) && (miso==20) && (mosi==23)){bf=1;}}
if( bf){spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus);}
return;
}
#endif
//++++SPI1(SCK,MISO,MOSI) - 14, 8,15; 10,12,11+++++++++++++++++++++++++++++++++++++++++++++++++++++
#if defined(RT_HW_PERMIT_SPI1)
if(bus==1){
if(!bf){if((sck==10) && (miso== 8) && (mosi==11)){bf=1;}}
if(!bf){if((sck==14) && (miso==12) && (mosi==15)){bf=1;}}
if(!bf){if((sck==26) && (miso==24) && (mosi==27)){bf=1;}}
if( bf){spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus);}
return;
}
#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#endif
//=================================================================================================
#if defined(RT_HW_CORE_STM32)
//++++SPI (SCK,MISO,MOSI) -  2, 4, 5;  6, 8, 7; 18,16;+++++++++++++++++++++++++++++++++++++++++++++
#if defined(RT_HW_PERMIT_SPI0)
#endif
//++++SPI1(SCK,MISO,MOSI) - 14, 8,15; 10,12,11+++++++++++++++++++++++++++++++++++++++++++++++++++++
#if defined(RT_HW_PERMIT_SPI1)
#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#endif
//=================================================================================================
if((sck==miso) || (sck==mosi) || (miso==mosi)) {spiSetCodeErr(60,bus);   return;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID, sck))    {spiSetCodeErr(61,bus);   return;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,miso))	   {spiSetCodeErr(62,bus);   return;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,mosi))	   {spiSetCodeErr(63,bus);   return;}
spiSetPinSCK(sck,bus); spiSetPinMISO(miso,bus); spiSetPinMOSI(mosi,bus); return;
};

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									4.БАЗОВЫЕ ФУНКЦИИ SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
void 	 spiBeginTransaction(uint32_t speed, uint8_t bitOrder, uint8_t dataMode, uint8_t bus=0);//==Начало транзакции с параметрами speed,        bitOrder,         dataMode;        bitOrder,         dataMode;
void 	 spiBeginTransaction(uint32_t speed, uint8_t bus=0);									//==Начало транзакции с параметрами speed         bitOrder=MSBFIRST,dataMode=MODE0;
void     spiBeginTransaction(uint8_t bus=0);													//==Начало транзакции с параметрами speed=default,bitOrder=MSBFIRST,dataMode=MODE0;
void 	 spiEndTransaction  (uint8_t bus=0);													//==Окончание транзакции;
uint8_t	 spiTransfer        (uint8_t val,               uint8_t bus=0);							//==Передача байта. Возвращает принятый байт;
void     spiTransfer        (uint8_t qnt, uint8_t *arr, uint8_t bus){						    //==Передача данных из массива и прием данных в массив длиной qnt;
		 (void)qnt; (void)bus; static uint8_t idx; for(idx=0; idx<qnt; idx++){arr[idx]=spiTransfer(arr[idx],bus);}};
uint8_t  spiBegin           (uint8_t bus=0);													//==Инициализации шины как Master;													
void     spiEnd             (uint8_t bus=0);													//==Отключение инициализации шины;
void     spiInitCS 			(uint8_t cs){digitalWrite(cs, HIGH); pinMode (cs,OUTPUT);};			//==Настройка пина СS на вывод;
void     spiBeginCS  		(uint8_t cs){digitalWrite(cs, LOW);};								//==Установка пина CS в HIGH;
void     spiEndCS			(uint8_t cs){digitalWrite(cs, HIGH);};								//==Сброс     пина CS в LOW;
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//								ФУНКЦИИ ДЛЯ РАБОТЫ С УСТРОЙСТВАМИ НА ШИНЕ SPI
//==Для работы с устройствами на шине SPI рекомендуется использовать набор функций:
//  spiTransfer   (RT_HW_STRUCT_SPI_DEV &id, uint8_t tx, uint8_t &rx) 					- обмен байтами;
//  spiWrite      (RT_HW_STRUCT_SPI_DEV &id, uint8_t tx)								- запись байта;
//  spiRead       (RT_HW_STRUCT_SPI_DEV &id, 			  uint8_t &rx)					- чтение байта;
//  spiTransferArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx, uint8_t *rx, uint8_t qnt=1)	- обмен байтовыми массивами tx,rx длиной qnt;
//  spiTransferArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx,              uint8_t qnt=1)	- обмен байтовыми массивами, запись из tx, чтение в tx;
//  spiWriteArr   (RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx,              uint8_t qnt=1) 	- запись массива tx длиной qnt;
//  spiReadArr    (RT_HW_STRUCT_SPI_DEV &id,              uint8_t *rx, uint8_t qnt=1) 	- чтение массива rx длиной qnt;
//  spiWRITEArr   (RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t *tx, uint8_t qnt=1)   - запись массива tx длиной qnt начиная с регистра reg;
//  spiREADArr    (RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t *rx, uint8_t qnt=1)   - чтение массива rx длиной qnt начиная с регистра reg;
//
//==Для работы с устройствами на шине SPI рекомендуется функцию:
//  spiRead74HC165(RT_HW_STRUCT_SPI_DEV &id,                           uint8_t qnt=1)   - чтение от 1 до 4 байт из каскадно-соединенных сдвиговых регистров 74HC165;
//
//==Для работы со сдвиговыми регистрами типа 74HC595,DM13C и др.:
//  spiTransferSh (RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx, uint8_t qnt=1) - обмен  от 1 до 4 байтами;
//  spiWriteSh    (RT_HW_STRUCT_SPI_DEV &id, uint32_t tx,               uint8_t qnt=1) - запись от 1 до 4 байтов;
//  spiReadSh     (RT_HW_STRUCT_SPI_DEV &id,                            uint8_t qnt=1) - чтение от 1 до 4 байт;

//-------------------------------------------------------------------------------------------------
//  Перед примененем функций работы с устройствами на шине SPI, параметры устройство должны быть настроены функциями:
//  spiSetParam(   RT_HW_STRUCT_SPI_DEV &id, uint8_t cs, uint8_t bus, uint32_t speed=0)   - установка cs, номер шины,скорости. 
//                                                                     Возвращает параметр id.custom=1; Для повторной настройки нужно сбросить id.custom=0; 
//  spiSetParam(   RT_HW_STRUCT_SPI_DEV &id, uint8_t cs)                                  - аналогичная установка cs, номер шины=0, скорость=0 (при инициализации RT_HW_SPI_SPEED->id.speed); 
//-------------------------------------------------------------------------------------------------
//  Перед применением функций работы с устройствами на шине SPI, после настройки параметров устройства, устройство должно быть настроено функцией:
//  spiInitDevice( RT_HW_STRUCT_SPI_DEV &id)											  - настройка устройства;
//-------------------------------------------------------------------------------------------------
//  Функции для работы с устройствами на шине SPI используют внутренние функции:
//  spiBeginTransaction(RT_HW_STRUCT_SPI_DEV &id) - начало    транзакции и установка пина CS LOW после задержки=id.t1;
//  spiEndTransaction  (RT_HW_STRUCT_SPI_DEV &id) - окончание транзакции и установка пина CS HIGH после задержки=id.t2, далее дополнительная задержка id.t3;
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++


//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          		5.ФУНКЦИИ ДЛЯ НАСТРОЙКИ УСТРОЙСТВ SPI  (DEVICE)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
// 		    						5.1.Настройка параметров SPI 
//               для устройства c CS=cs, на шине 0,со стандартной скоростью(default=0->RT_HW_SPI_SPEED)                             
//=================================================================================================
uint8_t spiSetParam(RT_HW_STRUCT_SPI_DEV &id, uint8_t cs){
if(id.custom==0){id.link=id.status=id.codeErr=id.err=0; id.cs=cs; id.custom=1;}
return id.custom;};
//=================================================================================================
// 		    						5.2.Настройка параметров SPI 
//               для устройства c CS=cs, на шине bus,со скоростью(speed)                               
//=================================================================================================
uint8_t spiSetParam(RT_HW_STRUCT_SPI_DEV &id, uint8_t cs, uint8_t bus, uint32_t speed=0){
if(id.custom==0){id.link=id.status=id.codeErr=id.err=0; id.cs=cs; id.bus=bus; id.speed=speed; id.custom=1;}
return id.custom;};

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          		6.ФУНКЦИИ ДЛЯ НАСТРОЙКИ УСТРОЙСТВ SPI  (DEVICE)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									6.1.НАСТРОЙКА УСТРОЙСТВА
//				Обрабатываются конфликты доступа к шине при одновременном доступе(RTOS,многопроцессорность и т.п.)
//=================================================================================================
void  spiInitDevice(RT_HW_STRUCT_SPI_DEV &id){
  id.link=0;
  if(id.status)   {return;}		//--Выход, если устройство настроено;
  if(id.err==1)   {return;}		//--Выход, предыдущая настройка выявила ошибку;
  id.status=id.codeErr=0;
//------------------------------------------------------------------------------------------------
  if(!spiCheckBus(id.bus))	                  {id.codeErr=52; id.err=1; id.link=1; return;} //--Проверка номера шины;
  if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,id.cs)){id.codeErr=53; id.err=1; id.link=1; return;} //--Проверка допустимости номера пина cs;
//-------------------------------------------------------------------------------------------------																								
  if(!spiBegin(id.bus))		  {											 		   return;} //--Инициализация      шины SPI как Master. Если шина занята, будет ждать освобождения шины; 
  if( spiGetStatus(id.bus)!=1){                id.codeErr=54; id.err=1; id.link=1; return;} //--Проверка настройки шины SPI как Master;
//-------------------------------------------------------------------------------------------------
  if(id.speed==0){id.speed=RT_HW_SPI_SPEED;}												//--Нормализация скорости;
  digitalWrite(id.cs,1); pinMode(id.cs,OUTPUT);												//--Настройка пина CS на вывод																				//--Сохранение для макросов быстрого ввода-вывода;
  id.custom=id.status=id.link=1;															//--Установка выходных параметров;	
};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          7.ФУНЦИИ БАЙТОВОГО ОБМЕНА С УСТРОЙСТВАМИ на шине SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									7.1.spiTransfer(id) - обмен байтами
//=================================================================================================
void  spiTransfer(RT_HW_STRUCT_SPI_DEV &id, uint8_t tx, uint8_t &rx){
id.link=0; rx=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;   
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
rx=spiTransfer(tx,id.bus);	  			//--Обмен байтами;
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.2.spiWrite(id) - запись байта
//=================================================================================================
void  spiWrite(RT_HW_STRUCT_SPI_DEV &id, uint8_t tx){
id.link=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;   
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
spiTransfer(tx,id.bus);	  				//--Запись байта;
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.3.spiRead(id) - чтение байта
//=================================================================================================
void  spiRead(RT_HW_STRUCT_SPI_DEV &id, uint8_t &rx){
id.link=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;   
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
rx=spiTransfer(0,id.bus);	  			//--Обмен байтами;
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.4.spiTransferArr(id) обмен qnt байтами 
//=================================================================================================
void  spiTransferArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx, uint8_t *rx, uint8_t qnt=1){
id.link=0;
if(qnt==0)      {id.link=1; return;}	//--Выход, если qnt=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины; 
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
for(id.idx=0; id.idx<qnt; id.idx++){rx[id.idx]=spiTransfer(tx[id.idx],id.bus);} //--В начале старший байт;
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.5.spiTransferArr(id) запись из массива arr со чтением в массив arr qnt байт
//=================================================================================================
void  spiTransferArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *arr, uint8_t qnt=1){
id.link=0;
if(qnt==0)      {id.link=1; return;}	//--Выход, если qnt=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины; 
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
for(id.idx=0; id.idx<qnt; id.idx++){arr[id.idx]=spiTransfer(arr[id.idx],id.bus);} //--В начале старший байт;
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}					
//=================================================================================================
//									7.6.spiWriteArr(id)
//=================================================================================================
void  spiWriteArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *tx, uint8_t qnt=1){
id.link=0;
if(qnt==0){      id.link=1; return;}	//--Выход, если qnt=0;
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;   
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;;
spiBeginCS(id);							//--Установка CS в LOW;
for(id.idx=0; id.idx<qnt; id.idx++){ spiTransfer(tx[id.idx],id.bus);} //--Отправка qnt байт;
spiEndCS(id);							//--Установка CS в LOW;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.9.spiReadArr(id)
//=================================================================================================
void  spiReadArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t *rx, uint8_t qnt=1){
id.link=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если qnt=0;
if(id.status!=1){id.link=1; return;}	//--Checking device and bus setup;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;  
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
for(id.idx=0; id.idx<qnt; id.idx++){rx[id.idx]=spiTransfer(0,id.bus);}
spiEndCS(id);							//--Установка CS в LOW;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}		

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          8.ФУНЦИИ ОБМЕНА СЛОВАМИ (qnt=1-4) С УСТРОЙСТВАМИ на шине SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									8.1.spiTransferSh(id) обмен словами длиной 1,2,3 или 4 байта
//=================================================================================================
void  spiTransferShift(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx, uint8_t qnt){
id.link=0; rx=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если количество байтов qnt=0;
if(qnt>4) {qnt=4;} 						//--Ограничение кол-во байт (qnt<=4);
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;   
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
rx=0;												 
if(id.bitOrder){for(id.idx=qnt-1; id.idx>=0;       id.idx--){rx|=((uint32_t)(spiTransfer((tx>>(id.idx*8))&0xFF,id.bus)))<<(id.idx*8);}}
else           {for(id.idx=0;     id.idx<=(qnt-1); id.idx++){rx|=((uint32_t)(spiTransfer((tx>>(id.idx*8))&0xFF,id.bus)))<<(id.idx*8);}}
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									8.2.spiWriteSh(id) обмен словами длиной 1,2,3 или 4 байта
//=================================================================================================
void  spiWriteShift(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint8_t qnt){
id.link=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если количество байтов qnt=0;
if(qnt>4) {qnt=4;} 						//--Ограничение кол-во байт (qnt<=4);
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;    
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;												 
if(id.bitOrder){for(id.idx=qnt-1; id.idx>=0;       id.idx--){spiTransfer((tx>>(id.idx*8))&0xFF,id.bus);}}
else           {for(id.idx=0;     id.idx<=(qnt-1); id.idx++){spiTransfer((tx>>(id.idx*8))&0xFF,id.bus);}}
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									8.3.spiTransfer(id) qnt	(DM13Cx2)
//=================================================================================================
void  spiTransfer(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint32_t &rx, uint8_t qnt){
id.link=0; rx=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если количество байтов qnt=0;
if(qnt>4){qnt=4;} 						//--Ограничение кол-во байт (qnt<=4);
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;    
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
spiBeginCS(id);							//--Установка CS в LOW;
rx=0;
for(id.idx=qnt-1; id.idx>=0; id.idx--){rx|=spiTransfer((tx>>(id.idx*8))&0xFF,id.bus)<<(id.idx*8);}
spiEndCS(id);							//--Установка CS в HIGH;
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}				
//=================================================================================================
//									8.4.spiWrite(id) запись слова (1,2,3,4 байт) из tx (74HC595x4)
//=================================================================================================
void  spiWrite(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint8_t qnt=1){spiTransfer(id,tx,id.bf32,qnt);};
//=================================================================================================
//									7.3.spiRead(id)  чтение слова (1,2,3,4 байт) в id.bf32 (MAX6675 и др.)
//=================================================================================================
void  spiRead (RT_HW_STRUCT_SPI_DEV &id,              uint8_t qnt=1){spiTransfer(id, 0,id.bf32,qnt);};
//=================================================================================================
//									7.4.spiREAD(id) [Gag - read register]
//=================================================================================================
void  spiREAD(RT_HW_STRUCT_SPI_DEV &id,  uint8_t reg){spiTransfer(id,(uint32_t)reg,id.bf32,1);};
//=================================================================================================
//									7.5.spiReadRegs(id) [Gag - read qnt register]
//=================================================================================================
void  spiREADArr(RT_HW_STRUCT_SPI_DEV &id, uint8_t reg, uint8_t *arr, uint8_t qnt=1){id.bf32=reg; id.idx=qnt;  arr[0]=id.link=1;};
//=================================================================================================
//									7.6.spiRead74HC165(id)
//=================================================================================================
void  spiRead74HC165(RT_HW_STRUCT_SPI_DEV &id, uint8_t qnt=1){
id.link=0; id.bf32=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если количество байтов qnt=0;
if(qnt>4){qnt=4;} 						//--Ограничение кол-во байт (qnt<=4);
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;    
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id.speed,id.bitOrder,id.dataMode,id.bus);
//-------------------------------------------------------------------------------------------------
spiBeginCS(id);							//--Установка CS в LOW;
spiEndCS(id);							//--Установка CS в HIGH;
//-------------------------------------------------------------------------------------------------
for(id.idx=qnt-1; id.idx>=0; id.idx--){id.bf32|=(spiTransfer(0,id.bus))<<(id.idx*8);} //--gJMFQN
//-------------------------------------------------------------------------------------------------
spiEndTransaction(id.bus);				//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//=================================================================================================
//									7.7.spiRead74HC595(id)
//=================================================================================================
void  spiWrite74HC595(RT_HW_STRUCT_SPI_DEV &id, uint32_t tx, uint8_t qnt=1){
id.link=0;
if(qnt==0){id.link=1;    	return;}	//--Выход, если количество байтов qnt=0;
if(qnt>4) {qnt=4;} 						//--Ограничение кол-во байт (qnt<=4);
if(id.status!=1){id.link=1; return;}	//--Выход, если устройство не настроено;
if(!spiSetBusy(id.bus))    {return;};	//--Ожидание освобождение шины и захват шины;    
//-------------------------------------------------------------------------------------------------
spiBeginTransaction(id);				//--Начало транзакции;
//-------------------------------------------------------------------------------------------------											 
if(id.bitOrder){for(id.idx=qnt-1; id.idx>=0;       id.idx--){spiTransfer((tx>>(id.idx*8))&0xFF,id.bus);}}
else           {for(id.idx=0;     id.idx<=(qnt-1); id.idx++){spiTransfer((tx>>(id.idx*8))&0xFF,id.bus);}}
//-------------------------------------------------------------------------------------------------
spiBeginCS(id);							//--Установка CS в LOW;	
spiEndCS(id);							//--Установка CS в HIGH;
//-------------------------------------------------------------------------------------------------
spiEndTransaction(id);					//--Окончание транзакции;
//-------------------------------------------------------------------------------------------------
spiClearBusy(id.bus);					//--Освобождение шины;
id.link=1;}	
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//                          		 8.ФУНЦИИ ДЛЯ РАБОТЫ С УСТРОЙСТВАМИ на шине SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									8.1.Начало транзакции для шины SPI для устройства с ID=id 
//                                      и установка пина CS LOW с задержкой id.t1            
//=================================================================================================
void  spiBeginTransaction(RT_HW_STRUCT_SPI_DEV &id){spiBeginTransaction(id.speed,id.bitOrder,id.dataMode,id.bus);};
//=================================================================================================
//									8.2.Завершение транзакции для шины SPI для устройства с ID=id 
//                                      и установка пина CS HIGH с задержками id.t2,id.t3
//=================================================================================================
void  spiEndTransaction(RT_HW_STRUCT_SPI_DEV &id){spiEndTransaction(id.bus);};
//=================================================================================================
//									8.3.Настройка пина SS на вывод
//=================================================================================================
void  spiInitCS (RT_HW_STRUCT_SPI_DEV &id){digitalWrite(id.cs, HIGH);           pinMode(id.cs,OUTPUT);};			//==Настройка СS на вывод;
void  spiBeginCS(RT_HW_STRUCT_SPI_DEV &id){if(id.t1){delayMicroseconds(id.t1);} pinDigitalWrite(id.ddCS,id.cs,0);};	//==Перевод   CS в LOW; 
void  spiEndCS  (RT_HW_STRUCT_SPI_DEV &id){if(id.t2){delayMicroseconds(id.t2);} pinDigitalWrite(id.ddCS,id.cs,1);	//==Перевод   CS в HIGH;  
										   if(id.t3){delayMicroseconds(id.t3);}};	
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
