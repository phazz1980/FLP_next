//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//							 			PLC STM32F103V FLE-xxxx
//	 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

//=================================================================================================
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE_10Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_10MX)  
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6110			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-10Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_10Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)4			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)6			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
    #define RT_HW_PERMIT_PLC_ADX	(uint8_t)2			//--Доступные пины ADX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, 			 															RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,															RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--YRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={PC2, PC3,		RT_HW_PGM_END};		//--AUST1,AUST2;	
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE_14Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_14MX) 
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6114			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-14Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_14Mx 								//--Идентификатор PLC;	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)6			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)8			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
    #define RT_HW_PERMIT_PLC_ADX	(uint8_t)2			//--Доступные пины ADX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12, 															RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,													RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--YRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={PC2, PC3,		RT_HW_PGM_END};		//--AUST1,AUST2;
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE_24Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_24MX) 
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6240			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-24Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_24Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;	
//-------------------------------------------------------------------------------------------------	
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)10			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)14			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
    #define RT_HW_PERMIT_PLC_ADX	(uint8_t)0			//--Доступные пины ADX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12,PB15,PA7, 255, 255, PA6, PA2, 								RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,255, 255, PE11,PE12,PE9, PE10,PE7, PE8,			RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,      	RT_HW_PGM_END}; 	//--LedRUN,LedERR,;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--Switch,Vcc;
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={PC2, PC3,	    RT_HW_PGM_END};		//--UST1,AUST2;
#endif
//=================================================================================================



//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE_32Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_32MX) 

	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6320			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-32Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_32Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)24			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)32			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12,PB15,PA7, 255, 255, PA6, PA2, PB8, PB9, PE0, PE1,PC13,PE3, 	RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,255, 255, PE11,PE12,PE9, PE10,PE7, PE8,PB0, PB1,  RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--YRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE_48Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_48MX) 
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6480			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-48Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_48Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//------------------------------------------------------------------------------------------------
    #define  RT_HW_SPN0_HC595_OE 			PE4			//--Пин блокировки	74HC595;
    #define  RT_HW_SPN0_HC595_CS 			PB5			//--Пин CS   		74HC595;	
    #define  RT_HW_SPN0_HC595_QNT 			1			//--Кол-во чипов   	74HC595;	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPN0_HC165_CS 			PB6			//--Кол-во чипов   	74HC165;
    #define  RT_HW_SPN0_HC165_QNT 			1			//--Кол-во чипов   	74HC165;	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_SPN0_SPEED				100			//--Скорость (0-100%);
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)24			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)24			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------
	#define RT_HW_PERMIT_PLC_74HCxx5					//--Доступные расширители 74HC165 и 74HC595; 
	#define RT_HW_PERMIT_PLC_74HCxx5_LINK   'N'			//--'S' - Интерфейс SPI, 'N' - Интерфейс SPN;	
	#define RT_HW_PERMIT_PLC_74HC165_QNT     1			//--Кол-во чипов 74HC165;
	#define RT_HW_PERMIT_PLC_74HC595_QNT     1			//--Кол-во чипов 74HC595;
	#define RT_HW_PERMIT_PLC_74HC165_I2C     			//--Разрешение использовать для строба 74HC165 запрос i2c0;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12,PB15,PA7, 255, 255, PA6, PA2, PB8, PB9, PE0, PE1,PC13,PE3, 	RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,255, 255, PE11,PE12,PE9, PE10,PE7, PE8,PB0, PB1,  RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--YRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_74HCxx5_SPN[]={PD6, PD5, PE5, PB6, PB5, PE4, RT_HW_PGM_END};		//--SPN: sck,mosi,miso,cs165,cs595,oe;

//-------------------------------------------------------------------------------------------------------------------------------------------------------------
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE_56Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE_56MX) 
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)6560			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE-56Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE_56Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через UART 5;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;2	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPN0_HC595_OE 			PE4			//--Пин блокировки	74HC595;
    #define  RT_HW_SPN0_HC595_CS 			PB5			//--Пин CS   		74HC595;	
    #define  RT_HW_SPN0_HC595_QNT 			1			//--Кол-во чипов   	74HC595;	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPN0_HC165_CS 			PB6			//--Кол-во чипов   	74HC165;
    #define  RT_HW_SPN0_HC165_QNT 			2			//--Кол-во чипов   	74HC165;	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_SPN0_SPEED				100			//--Скорость (0-100%);
//------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)24			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)32			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12,PB15,PA7, 255, 255, PA6, PA2, PB8, PB9, PE0, PE1,PC13,PE3, 254,	RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,255, 255, PE11,PE12,PE9, PE10,PE7, PE8,PB0, PB1,  RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--YRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
#endif
//=================================================================================================
