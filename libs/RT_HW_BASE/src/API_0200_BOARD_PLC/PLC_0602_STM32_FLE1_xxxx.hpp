//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//							 		PLC STM32F103V FLE1-xxxx
//	 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE1-32Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  (defined(FLP_BOARD_PLC_FLE1_32MX) || defined(FLP_BOARD_PLC_FX2_32MX))  
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)5320			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE1-32Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE1_XX 									//--Группа PLC;	
	#define  RT_HW_PLC_FLE1_32Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART2_RX  		PA3
    #define  RT_HW_UART2_TX  		PA2
    #define  RT_HW_UART3_RX  		PB11 
    #define  RT_HW_UART3_TX  		PB10
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PLC_TEXT1 	"UART2(PA2-TX2, PA3-RX2) доступен при условии не использования входов AD2,AD3"
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		2			//--Консоль через UART 2;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		3			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2;
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)14			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)13			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)2			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PA8, PA11,PA12,PE14,PE13,PE12,PE11,255, 255, PE10, PE9, PE8, PE7, PB1, PB0, PC5, PC4, 255, 255, RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB5, PB4, PB7, PB3, PA15,PB6, PB8, PB9, 255, 255, PE0,  PE1, PC10,PC11,PC12,PD0, PD1, PD2,  	RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA0, PA1, PA2, PA3, PC0, PC1,	PC2, PC3,														RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																						RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9,  							RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD12,PD13,   	RT_HW_PGM_END}; 	//--YRUN, YERR;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PE2, 	RT_HW_PGM_END};		//--XSW,  XVCC;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------	
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE1-40Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  (defined(FLP_BOARD_PLC_FLE1_40MX) || defined(FLP_BOARD_PLC_FX2_40MX))  
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)5400			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE1-40Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE1_XX 									//--Группа PLC;	
	#define  RT_HW_PLC_FLE1_40Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART2_RX  			PA3
    #define  RT_HW_UART2_TX  			PA2
    #define  RT_HW_UART3_RX  			PB11 
    #define  RT_HW_UART3_TX  			PB10
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PLC_TEXT1 	"UART2(PA2-TX2, PA3-RX2) доступен при условии не использования входов AD2,AD3"
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		2			//--Консоль через UART 2;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		3			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2;
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)16			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)24			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)11			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)2			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PA8, PA11,PA12,PE14,PE13,PE12,PE11, 255, 255,PE10, PE9, PE8, PE7, PB1, PB0, PC5, PC4,   	RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB5, PB4, PB7, PB3, PA15,PB6, PB8, PB9,  255, 255,PE0,  PE1, PC10,PC11,PC12,PD0, PD1, PD2,  	
	                                              PD3, PD4, PD5, PD6, PD7, PE3, PE4, PE5,                                                       RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA0, PA1, PA2, PA3, PC0, PC1,	PC2, PC3,														RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																						RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={ 255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255,  				RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD12,PD13,   	RT_HW_PGM_END}; 	//--YRUN, YERR;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PE2, 	RT_HW_PGM_END};		//--XSW,  XVCC;
#endif
//=================================================================================================


//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FLE1_28Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE1_28MX)  
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)4320			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE1-28Mx"		//--Наименование;
	#define  RT_HW_PLC_FLE1_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE1_28Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		1			//--Консоль через USB;
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)16			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
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
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD10,    		RT_HW_PGM_END}; 	//--LedRUN;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--Switch,Vcc;
#endif
//=================================================================================================


