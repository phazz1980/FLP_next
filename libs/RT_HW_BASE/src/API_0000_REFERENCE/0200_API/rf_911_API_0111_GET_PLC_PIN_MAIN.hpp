//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										API PARAM PLC
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
	uint8_t PLC_checkPinDOT(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDOT  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDOT (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDOT();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDOT();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinDIN(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDIN  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDIN (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDIN();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDIN();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinADC(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinADC  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameADC (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinADC();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinADC();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinDAC(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDAC  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDAC (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDAC();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDAC();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinPWM(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinPWM  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNamePWM (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinPWM();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinPWM();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================		
	uint8_t PLC_checkPinDOX(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDOX  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDOX (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDOX();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDOX();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinDIX(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDIX  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDIX (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDIX();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDIX();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinADX(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinADX  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameADX (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinADX();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinADX();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinDAX(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinDAX  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNameDAX (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinDAX();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinDAX();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================
	uint8_t PLC_checkPinPWX(uint8_t pin);	//--Проверка наличия пина;
	uint8_t PLC_getPinPWX  (uint8_t idx);	//--Возвращает номер пина;
	String  PLC_getNamePWX (uint8_t idx);	//--Возвращает имя выхода;	
	uint8_t PLC_getFullPinPWX();			//--Возвращает полный размер массива;
	uint8_t PLC_getSizePinPWX();			//--Возвращает фактическое кол-во пинов; 
//=================================================================================================	























//-------------------------------------------------------------------------------------------------
//			1.ДОСТУП К ПАРАМЕТРАМ PLC 				[FUNC_DEFINE_PGM_0130_GET_PARAM_PLC.hpp]
//-------------------------------------------------------------------------------------------------
	uint16_t 	getPLCcode();					//--Получение кода PLC;
	String 		getPLCname();					//--Получение имени PLC;

	uint8_t 	getPLCnumConsole();				//--Получение значения номера консоли PLC;
	uint8_t 	getPLCpinLed();					//--Получение значения пина Led;
	uint8_t 	getPLCpinButton();				//--Получение значения пина Button;
	uint8_t 	getPLCpinSwitch();				//--Получение значения пина Switch;
	uint8_t 	getPLCpinVcc();					//--Получение значения пина контроль Vcc;
	uint8_t 	getPLCpinADC_ust1();			//--Получение значения пина ADC уставка 1;
	uint8_t 	getPLCpinADC_ust2();			//--Получение значения пина ADC уставка 2;
//-------------------------------------------------------------------------------------------------
	uint8_t 	getPLCbusETH0();				//--Получение номера шины Ethernet0;
	uint8_t 	getPLCbusETH1();				//--Получение номера шины Ethernet1;
	uint8_t 	getPLCbusETH2();				//--Получение номера шины Ethernet2;
	uint8_t 	getPLCbusETH3();				//--Получение номера шины Ethernet3;
//-------------------------------------------------------------------------------------------------
	uint8_t 	getPLCcsETH0();					//--Получение cs Ethernet0;
	uint8_t 	getPLCcsETH1();					//--Получение cs Ethernet1;
	uint8_t 	getPLCcsETH2();					//--Получение cs Ethernet2;
	uint8_t 	getPLCcsETH3();					//--Получение cs Ethernet3;
//-------------------------------------------------------------------------------------------------
	uint8_t 	getPLCoeETH0();					//--Получение oe Ethernet0;
	uint8_t 	getPLCoeETH1();					//--Получение oe Ethernet1;	
	uint8_t 	getPLCoeETH2();					//--Получение oe Ethernet2;
	uint8_t 	getPLCoeETH3();					//--Получение oe Ethernet3;		
//------------------------------------------------------------------------------------------------- 
	uint32_t 	getPLCspeedETH0()				//--Получение speed SPI Ethernet0;
	uint32_t 	getPLCspeedETH1()				//--Получение speed SPI Ethernet1;	
	uint32_t 	getPLCspeedETH2()				//--Получение speed SPI Ethernet2;
	uint32_t 	getPLCspeedETH3()				//--Получение speed SPI Ethernet3;
//-------------------------------------------------------------------------------------------------
	uint8_t 	getPLCpinSpnSCK();				//--Получение значения пина SPN	SCK;
	uint8_t 	getPLCpinSpnMISO();				//--Получение значения пина SPN MISO;
	uint8_t 	getPLCpinSpnMOSI();				//--Получение значения пина SPN_MOSI;
	uint8_t 	getPLCspeenSpn();				//--Получение значения SPN Speed(0-100%)
//-------------------------------------------------------------------------------------------------
	uint8_t 	getPLCpinSpnCS_HC165();			//--Получение значения пина SPN CS  HC165;
	uint8_t 	getPLCpinSpnQN_HC165();			//--Получение значения кол-ва чипов HC165;
	uint8_t 	getPLCpinSpnCS_HC595();			//--Получение значения пина SPN CS  HC595;
	uint8_t 	getPLCpinSpnOE_HC595();			//--Получение значения пина SPN OE  HC595;
	uint8_t 	getPLCpinSpnQN_HC595();			//--Получение значения кол-ва чипов HC595;
//=================================================================================================
