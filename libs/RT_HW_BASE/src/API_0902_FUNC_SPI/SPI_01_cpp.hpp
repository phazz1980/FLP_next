#include "RT_HW_BASE.h"	 
//#################################################################################################
//
//							 					SPI [.cpp]
//
//#################################################################################################
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//							 	1.Создание объектов SPI для выбранного CORE (MCU)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//										1.STM32
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI) && defined(RT_HW_CORE_STM32)	
    #define  RT_HW_MCU_CREATE_SPI
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1) && defined(RT_HW_CREATE_SPI1)
   #define  RT_HW_CREATED_SPI1x
SPIClass SPI1x;
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2) && defined(RT_HW_CREATE_SPI2)
   #define  RT_HW_CREATED_SPI2x
SPIClass SPI2x;
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3) && defined(RT_HW_CREATE_SPI3)
   #define  RT_HW_CREATED_SPI3x
SPIClass SPI3x;
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//										2.RP2040
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI) && defined(RT_HW_CORE_RP2040)	
    #define  RT_HW_MCU_CREATE_SPI
//--------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//										3.ESP32
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI) &&  defined(RT_HW_CORE_ESP32)	
    #define  RT_HW_MCU_CREATE_SPI
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1) && defined(RT_HW_CREATE_SPI1)
   #define  RT_HW_CREATED_SPI1x
SPIClass SPI1x(HSPI);
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//										4.ESP8266
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI) && defined(RT_HW_CORE_ESP8266)	
    #define  RT_HW_MCU_CREATE_SPI
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//										5.AVR
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI) && defined(RT_HW_CORE_AVR)	
    #define  RT_HW_MCU_CREATE_SPI
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//										9.Другие CORE (MCU)
//=================================================================================================
#if !defined(RT_HW_MCU_CREATE_SPI)	
     #define RT_HW_MCU_CREATE_SPI
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									3.БАЗОВЫЕ ФУНКЦИИ SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									3.1.Начало транзакции с полными параметрами
//=================================================================================================
void    RT_HW_BASE:: spiBeginTransaction(uint32_t speed, uint8_t bitOrder, uint8_t dataMode, uint8_t bus){
(void)speed; (void)bitOrder; (void)dataMode; (void)bus;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------
if(bus==0) {if(bitOrder){SPI.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
else                    {SPI.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
return;}						 
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------
if(bus==1) {
#if defined RT_HW_CREATED_SPI1x
			if(bitOrder){SPI1x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI1x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
#else
			if(bitOrder){SPI1.beginTransaction  (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI1.beginTransaction ( SPISettings(speed,LSBFIRST,dataMode));}
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)	
//-------------------------------------------------------------------------------------------------
if(bus==2) {
#if defined RT_HW_CREATED_SPI2x
			if(bitOrder){SPI2x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI2x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
#else
			if(bitOrder){SPI2.beginTransaction  (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI2.beginTransaction  (SPISettings(speed,LSBFIRST,dataMode));}
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)	
//-------------------------------------------------------------------------------------------------
if(bus==3) {
#if defined RT_HW_CREATED_SPI3x
			if(bitOrder){SPI3x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI3x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
#else
			if(bitOrder){SPI3.beginTransaction  (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI3.beginTransaction  (SPISettings(speed,LSBFIRST,dataMode));}
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)	
//-------------------------------------------------------------------------------------------------
if(bus==4) {
#if defined RT_HW_CREATED_SPI4x
			if(bitOrder){SPI4x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI4x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
#else
			if(bitOrder){SPI4.beginTransaction  (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI4.beginTransaction  (SPISettings(speed,LSBFIRST,dataMode));}
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)	
//-------------------------------------------------------------------------------------------------
if(bus==5) {
#if defined RT_HW_CREATED_SPI5x
			if(bitOrder){SPI5x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI5x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
#else
			if(bitOrder){SPI5.beginTransaction  (SPISettings(speed,MSBFIRST,dataMode));}	
			else        {SPI5.beginTransaction  (SPISettings(speed,LSBFIRST,dataMode));}
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
};
//=================================================================================================
//									3.2. Начало транзакции с указанием параметров
//=================================================================================================
void    RT_HW_BASE:: spiBeginTransaction(uint32_t speed, uint8_t bus){spiBeginTransaction(speed,1,0,bus);};
//=================================================================================================
//									3.3. Начало транзакции с параметрами по умолчанию
//=================================================================================================
void    RT_HW_BASE:: spiBeginTransaction(                uint8_t bus){spiBeginTransaction(spiGetSpeed(bus),1,0,bus);};	

//=================================================================================================
//									3.4.spiEndTransaction()
//=================================================================================================
void    RT_HW_BASE:: spiEndTransaction  (uint8_t bus){
(void)bus;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------
if(bus==0) {SPI.endTransaction(); return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------
if(bus==1) {
#if defined RT_HW_CREATED_SPI1x
            SPI1x.endTransaction(); 
#else
            SPI1.endTransaction();
#endif
return;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)	
//-------------------------------------------------------------------------------------------------
if(bus==2) {
#if defined RT_HW_CREATED_SPI2x
            SPI2x.endTransaction(); 
#else
            SPI2.endTransaction();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)	
//-------------------------------------------------------------------------------------------------
if(bus==3) {
#if defined RT_HW_CREATED_SPI3x
            SPI3x.endTransaction(); 
#else
            SPI3.endTransaction();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)	
//-------------------------------------------------------------------------------------------------
if(bus==4) {
#if defined RT_HW_CREATED_SPI4x
            SPI4x.endTransaction(); 
#else
            SPI4.endTransaction();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)	
//-------------------------------------------------------------------------------------------------
if(bus==5) {
#if defined RT_HW_CREATED_SPI5x
            SPI5x.endTransaction(); 
#else
            SPI5.endTransaction();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
};	
//=================================================================================================
//									3.5.Отправка байта (возвращается принятый байт)
//=================================================================================================
uint8_t  RT_HW_BASE:: spiTransfer   (uint8_t  var, uint8_t bus){
(void)var; (void)bus;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------
if(bus==0) {return SPI.transfer(var);}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------
if(bus==1) {
#if defined RT_HW_CREATED_SPI1x
            return SPI1x.transfer(var); 
#else
            return SPI1.transfer(var);
#endif
}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)	
//-------------------------------------------------------------------------------------------------
if(bus==2) {
#if defined RT_HW_CREATED_SPI2x
            SPI2x.transfer(var); 
#else
            SPI2.transfer(var);
#endif
return 0;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)	
//-------------------------------------------------------------------------------------------------
if(bus==3) {
#if defined RT_HW_CREATED_SPI3x
            SPI3x.transfer(var); 
#else
            SPI3.transfer(var);
#endif
return 0;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)	
//-------------------------------------------------------------------------------------------------
if(bus==4) {
#if defined RT_HW_CREATED_SPI4x
            SPI4x.transfer(var); 
#else
            SPI4.transfer(var);
#endif
return 0;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)	
//-------------------------------------------------------------------------------------------------
if(bus==5) {
#if defined RT_HW_CREATED_SPI5x
            SPI5x.transfer(var); 
#else
            SPI5.transfer(var);
#endif
return 0;}	
#endif
//-------------------------------------------------------------------------------------------------
return 0;};
//=================================================================================================
//									3.6.Инициализации шины (с параметрами их LinSPIx);
//=================================================================================================
uint8_t  RT_HW_BASE:: spiBegin(uint8_t bus){
spiClearCodeErr(bus);	//--
if(!spiCheckBus(bus))	 {spiSetCodeErr(65,bus); return 1;}	//--Выход, если недопустимый номер шины;
if( spiGetStatus(bus)==1){            return 1;} 			//--Выход, если шина уже Master;
if(!spiSetBusy(bus))	 {            return 0;}			//--Захват шины или выход еcли шина занята;
    spiEnd(bus);											//--Отключение шины SPI (если вдруг раньше была инициализирована функцией SPIx.begin();
//=================================================================================================
//									4.1.1.AVR,SAM3X,ANON
//=================================================================================================
#if defined(RT_HW_CORE_AVR) || defined(RT_HW_CORE_SAM3X) || defined(RT_HW_CORE_ANON)
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0){
SPI.begin(); spiSetStatus(1,bus); spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//									4.1.2.ESP8266
// CORE->SPI.pins(sck=14,miso=12,mosi=13,ss=0)==>HSPI;
//=================================================================================================
#if defined(RT_HW_CORE_ESP8266) 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
if(bus==0){
SPI.pins(14,12,13,0); 							
SPI.begin(); spiSetStatus(1,bus); spiClearBusy(bus); return 1;};	
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//									4.1.3.ESP32
//=================================================================================================
#if defined(RT_HW_CORE_ESP32) 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------
if(bus==0){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(61,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(62,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(63,bus);  spiClearBusy(bus); return 1;}
SPI.begin(spiGetPinSCK(bus),spiGetPinMISO(bus),spiGetPinMOSI(bus)); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------
if(bus==1){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(61,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(62,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(63,bus);  spiClearBusy(bus); return 1;}
SPI1x.begin(spiGetPinSCK(bus),spiGetPinMISO(bus),spiGetPinMOSI(bus)); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//									4.1.4.RP2040
//=================================================================================================
#if defined(RT_HW_CORE_RP2040) 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------	
if(bus==0){
if(!SPI.setSCK(spiGetPinSCK (bus))) 							{spiSetCodeErr(61,bus);  spiClearBusy(bus); return 1;}							
if(!SPI.setRX (spiGetPinMISO(bus))) 							{spiSetCodeErr(62,bus);  spiClearBusy(bus); return 1;}	
if(!SPI.setTX (spiGetPinMOSI(bus))) 							{spiSetCodeErr(63,bus);  spiClearBusy(bus); return 1;}	
SPI.begin(); 					                                   spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------	
if(bus==1){
if(!SPI1.setSCK(spiGetPinSCK (bus))) 							{spiSetCodeErr(64,bus);  spiClearBusy(bus); return 1;}							
if(!SPI1.setRX (spiGetPinMISO(bus))) 							{spiSetCodeErr(65,bus);  spiClearBusy(bus); return 1;}	
if(!SPI1.setTX (spiGetPinMOSI(bus))) 							{spiSetCodeErr(66,bus);  spiClearBusy(bus); return 1;}	
SPI1.begin(); 					                                   spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//									4.1.5.STM32
//=================================================================================================
#if defined(RT_HW_CORE_STM32) 
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------	
if(bus==0){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(61,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(62,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(63,bus);  spiClearBusy(bus); return 1;}
SPI.setSCLK(spiGetPinSCK(bus));   SPI.setMISO(spiGetPinMISO(bus)); SPI.setMOSI(spiGetPinMOSI(bus));
SPI.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------	
if(bus==1){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(64,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(65,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(66,bus);  spiClearBusy(bus); return 1;}
SPI1x.setSCLK(spiGetPinSCK(bus)); SPI1x.setMISO(spiGetPinMISO(bus)); SPI1x.setMOSI(spiGetPinMOSI(bus));
SPI1x.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)	
//-------------------------------------------------------------------------------------------------	
if(bus==2){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(67,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(68,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(69,bus);  spiClearBusy(bus); return 1;}
SPI2x.setSCLK(spiGetPinSCK(bus)); SPI2x.setMISO(spiGetPinMISO(bus)); SPI2x.setMOSI(spiGetPinMOSI(bus));
SPI2x.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)	
//-------------------------------------------------------------------------------------------------	
if(bus==3){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(67,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(68,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(69,bus);  spiClearBusy(bus); return 1;}
SPI3x.setSCLK(spiGetPinSCK(bus)); SPI3x.setMISO(spiGetPinMISO(bus)); SPI3x.setMOSI(spiGetPinMOSI(bus));
SPI3x.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)	
//-------------------------------------------------------------------------------------------------	
if(bus==4){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(67,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(68,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(69,bus);  spiClearBusy(bus); return 1;}
SPI4x.setSCLK(spiGetPinSCK(bus)); SPI4x.setMISO(spiGetPinMISO(bus)); SPI4x.setMOSI(spiGetPinMOSI(bus));
SPI4x.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)	
//-------------------------------------------------------------------------------------------------	
if(bus==5){
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinSCK (bus))) 	     {spiSetCodeErr(67,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DIN_ID,spiGetPinMISO(bus)))        {spiSetCodeErr(68,bus);  spiClearBusy(bus); return 1;}
if(!checkPinPGM(RT_HW_PGM_PIN_DOT_ID,spiGetPinMOSI(bus)))	     {spiSetCodeErr(69,bus);  spiClearBusy(bus); return 1;}
SPI5x.setSCLK(spiGetPinSCK(bus)); SPI5x.setMISO(spiGetPinMISO(bus)); SPI5x.setMOSI(spiGetPinMOSI(bus));
SPI5x.begin(); spiSetStatus(1,bus);  spiClearBusy(bus); return 1;}	
#endif
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
//									4.1.9 MCU xxx
//=================================================================================================
																  spiSetCodeErr(60,bus);  spiClearBusy(bus); return 1;	
//=================================================================================================
return 0;}

//=================================================================================================
//									3.2.Отключение шины
//=================================================================================================
void    RT_HW_BASE:: spiEnd (uint8_t bus){
(void)bus;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI0)	
//-------------------------------------------------------------------------------------------------
if(bus==0) {SPI.end(); return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI1)	
//-------------------------------------------------------------------------------------------------
if(bus==1) {
#if defined RT_HW_CREATED_SPI1x
            SPI1x.end(); 
#else
            SPI1.end();
#endif
return;}	
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI2)	
//-------------------------------------------------------------------------------------------------
if(bus==2) {
#if defined RT_HW_CREATED_SPI2x
            SPI2x.end(); 
#else
            SPI2.end();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI3)	
//-------------------------------------------------------------------------------------------------
if(bus==3) {
#if defined RT_HW_CREATED_SPI3x
            SPI3x.end(); 
#else
            SPI3.end();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI4)	
//-------------------------------------------------------------------------------------------------
if(bus==4) {
#if defined RT_HW_CREATED_SPI4x
            SPI4x.end(); 
#else
            SPI4.end();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_PERMIT_SPI5)	
//-------------------------------------------------------------------------------------------------
if(bus==5) {
#if defined RT_HW_CREATED_SPI5x
            SPI5x.end(); 
#else
            SPI5.end();
#endif
return;}
#endif
//-------------------------------------------------------------------------------------------------
};	
//=================================================================================================
