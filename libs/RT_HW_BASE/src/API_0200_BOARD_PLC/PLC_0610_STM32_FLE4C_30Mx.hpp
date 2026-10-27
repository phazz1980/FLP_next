//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//							 			PLC FLE4-30Mx
//	 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

//=================================================================================================
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE3-30MT
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE4C_30MT)
	#define  RT_HW_PLC_STM32F407VGx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)3244			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE4C-30MT"		//--Наименование;
	#define  RT_HW_PLC_FLE4_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE4C_30 								//--Идентификатор PLC;
	#define  RT_HW_PLC_MODE8		 							//--Восьмеричный формат (Y00-Y07,Y10-Y17 и т.д.);
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		6			//--Консоль через USB;
	#define  RT_HW_PLC_UART_MODBUS1   		4			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		5			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1 (RS232);
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2 (RS232);	
//-------------------------------------------------------------------------------------------------	
//							Переопределение макросов пинов RX2,TX2
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_UART2_RX)
		#undefine  RT_HW_UART2_TX  		
	#endif
	#define  RT_HW_UART2_RX  		PD6
	//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_UART2_TX)
		#undefine  RT_HW_UART2_TX  		
	#endif	
	#define  RT_HW_UART2_TX  		PD5	
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)14			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)8			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)3			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
////-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={ PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, 255, 255, PE8, PE9,PE10,PE11,PE12,PE13,           RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={ PD3, PD4, PC2, PC3, PD7, PD10,PD11,PD12,255, 255,PD13,PD14,PD15,PC4,	PC5, PC8, PC9, PC13, RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={ PA0, PA1, PA2, PA3, PA6, PA7, PC0, PC1,         											 RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={ PA4, PA5,																				 RT_HW_PGM_END};	
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={ PA8, PA12, PA11,   																	 	 RT_HW_PGM_END}; 	//--YRUN,YERR,YDC;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={ PB11,PA15,					 													         RT_HW_PGM_END};	//--XSwitch, XVCC;	
#endif
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE3-30MR
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FLE4C_30MR)
	#define  RT_HW_PLC_STM32F407VGx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)3246			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE4C-30MR"		//--Наименование;
	#define  RT_HW_PLC_FLE4_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE4C_30 								//--Идентификатор PLC;
	#define  RT_HW_PLC_MODE8		 							//--Восьмеричный формат (Y00-Y07,Y10-Y17 и т.д.);
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		6			//--Консоль через USB;
	#define  RT_HW_PLC_UART_MODBUS1   		4			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		5			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1 (RS232);
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2 (RS232);	
//-------------------------------------------------------------------------------------------------	
//							Переопределение макросов пинов RX2,TX2
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_UART2_RX)
		#undefine  RT_HW_UART2_TX  		
	#endif
	#define  RT_HW_UART2_RX  		PD6
	//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_UART2_TX)
		#undefine  RT_HW_UART2_TX  		
	#endif	
	#define  RT_HW_UART2_TX  		PD5	
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)14			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)8			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)3			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
////-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={ PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, 255, 255, PE8, PE9,PE10,PE11,PE12,PE13,           RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={ PD3, PD4, PC2, PC3, PD7, PD10,PD11,PD12,255, 255,PD13,PD14,PD15,PC4,	PC5, PC8, PC9, PC13, RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={ PA0, PA1, PA2, PA3, PA6, PA7, PC0, PC1,         											 RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={ PA4, PA5,																				 RT_HW_PGM_END};	
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={ PA8, PA12, PA11,   																	 	 RT_HW_PGM_END}; 	//--YRUN,YERR,YDC;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={ PB11,PA15,					 													         RT_HW_PGM_END};	//--XSwitch, XVCC;	
#endif
//=================================================================================================


//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++					
//									 	PLC FLE3-30MGb
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  (defined(FLP_BOARD_PLC_FLE4_30MGb))
	#define  RT_HW_PLC_STM32F407VGx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)3242			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FLE4-30Mb"		//--Наименование;
	#define  RT_HW_PLC_FLE4_XX 									//--Группа;	
	#define  RT_HW_PLC_FLE4C_30 								//--Идентификатор PLC;
	#define  RT_HW_PLC_MODE8		 							//--Восьмеричный формат (Y00-Y07,Y10-Y17 и т.д.);
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE   		6			//--Консоль через USB;
	#define  RT_HW_PLC_UART_MODBUS1   		4			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		5			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1 (RS232);
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2 (RS232);	
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)14			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)8			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)1			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
////-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                  00   01   02   03   04   05   06   07   08   09   10   11   12   13   14   15   16   17   18   20
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={ PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, 255, 255, PE8, PE9,PE10,PE11,PE12,PE13,           RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={ PD3, PD4, PD5, PD6, PD7, PD10,PD11,PD12,255, 255,PD13,PD14,PD15,PC4,	PC5, PC8, PC9, PC13, RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={ PA0, PA1, PA2, PA3, PA6, PA7, PC0, PC1,         											 RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={ PA4, PA5,																				 RT_HW_PGM_END};	
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={ PA8, PB11, PC3, PA12,   																	 RT_HW_PGM_END}; 	//--YRUN,YERR,YDC,YRST;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={ PC2, PA15,					 													         RT_HW_PGM_END};	//--XSwitch, XVCC;	
#endif
//=================================================================================================

