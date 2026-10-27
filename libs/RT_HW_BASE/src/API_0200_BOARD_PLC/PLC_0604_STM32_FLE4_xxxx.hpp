//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//							 			PLC FLE4
//	 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE4_24_1 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE4_24_1)  
	#define  RT_HW_PLC_STM32F407VEx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)3241			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE4-24-1"		//--Наименование;
	#define  RT_HW_PLC_FLE4_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE4_24 									//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
#if !defined(PB2)
	#define  PB2 PB_2
#endif
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		0			//--Консоль через USB;
	#define  RT_HW_PLC_UART_CONSOLE1   		5			//--Консоль через программатор WeAct;
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)12			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)12			//--Доступные пины DIN;
//    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
//    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
//    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
//    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//   #define RT_HW_PERMIT_PLC_ADX	(uint8_t)2			//--Доступные пины ADX;
////-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PA6, PA7, PA8, PB0, PB6, PB7, PC2, PC3, PC4, PC1, PD7, PD10,  RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, PE8, PE9, PE10,PE11,	RT_HW_PGM_END};	
//    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PC0, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
//    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
//    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PA1,    		RT_HW_PGM_END}; 	//--YRUN;
//    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PC7, 	RT_HW_PGM_END};		//--XSW,  XVCC;
//    const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={PC0, 			RT_HW_PGM_END};		//--AUST1,AUST2;	
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE4_20_1 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE4_20_1)  
	#define  RT_HW_PLC_STM32F407VEx 					//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)3201	//--Код;	
    #define  RT_HW_PLC_NAME		"FLProg PLC FLE4-20-1"	//--Наименование;
	#define  RT_HW_PLC_FLE4_XX 							//--Группа;	
	#define  RT_HW_PLC_FLE4_20 							//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------
#if !defined(PB2)
	#define  PB2 PB_2
#endif
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		0			//--Консоль через USB;
	#define  RT_HW_PLC_UART_CONSOLE2   		5			//--Консоль через программатор WeAct;
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		2			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			3			//--Номер UART для панели HMI 1;		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)4			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)6			//--Доступные пины DIN;
//  #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
//  #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
//  #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//  #define RT_HW_PERMIT_PLC_ADX	(uint8_t)2			//--Доступные пины ADX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PA6, PA7, PA8, PB0, PB6, PB7, PC2, PC3,   					RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, PE8, PE9, PE10,PE11,	RT_HW_PGM_END};		
//  const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1,															RT_HW_PGM_END};																																		
//  const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
//  const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PA1, PC13,  	RT_HW_PGM_END}; 	//--YRUN, YETH;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={255, PC0, 	RT_HW_PGM_END};		//--XSW,  XVCC;
//  const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={PC2, PC3,		RT_HW_PGM_END};		//--AUST1,AUST2;	
#endif
//=================================================================================================