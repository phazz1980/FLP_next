//=================================================================================================
//						PLC FLE_56Mx:: 20. Системные пины
//=================================================================================================
	#define  RT_HW_BOARD_PIN_LED      		PD10	//--Led (Run) на плате;
//	#define  RT_HW_BOARD_PIN_ERR			255		//--Err       на платe
//	#define  RT_HW_BOARD_PIN_Button   		255		//--Кноака    на плате;	
	#define  RT_HW_BOARD_PIN_SWITCH   		PB2		//--Переключатель на плате;
	#define  RT_HW_BOARD_PIN_VCC     		PC7		//--Контроль питания;
//	#define  RT_HW_BOARD_PIN_ADC_UST1    	PC2		//--Уставка ADC.1;	
//	#define  RT_HW_BOARD_PIN_ADC_UST2    	PC3		//--Уставка ADC.2;		
//=================================================================================================
//						PLC FLE_56Mx:: 30. Доступные пины MCU
//=================================================================================================
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)81			//--Дискретный выход;	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)15			//--PWM;  
	#define RT_HW_PERMIT_PIN_DAC	(uint8_t)2			//--DAC;	
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)81			//--Дискретный
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)17			//--ADC;	
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={PA0, PA1, PA2, PA3, PA4, PA5, PA6, PA7, PA8, PA9, PA10,PA11,PA12, 255, 255,PA15,
												   PB0, PB1, PB2, PB3, PB4, PB5, PB6, PB7, PB8, PB9, PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0, PC1, PC2, PC3, PC4, PC5, PC6, PC7, PC8, PC9, PC10,PC11,PC12,PC13,PC14,PC15,
												   PD0, PD1, PD2, PD3, PD4, PD5, PD6, PD7, PD8, PD9, PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, PE8, PE9, PE10,PE11,PE12,PE13,PE14,PE15,	RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={PA0, PA1, PA2, PA3, PA6, PA7, PA8, PA9, PA10,PB0, PB1, PB6, PB7, PB8, PB9,		RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_DAC_ID[] ={PA4, PA5,																		RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={PA0, PA1, PA2, PA3, PA4, PA5, PA6, PA7, PA8, PA9, PA10,PA11,PA12, 255, 255,PA15,
												   PB0, PB1, PB2, PB3, PB4, PB5, PB6, PB7, PB8, PB9, PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0, PC1, PC2, PC3, PC4, PC5, PC6, PC7, PC8, PC9, PC10,PC11,PC12,PC13,PC14,PC15,
												   PD0, PD1, PD2, PD3, PD4, PD5, PD6, PD7, PD8, PD9, PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7, PE8, PE9, PE10,PE11,PE12,PE13,PE14,PE15,	RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={PA0, PA1, PA2, PA3, PA4, PA5, PA6, PA7, PA8, PB0, PB1, PC0,PC1,PC2,PC3,PC4,PC5,	RT_HW_PGM_END};	
//=================================================================================================
//						PLC FLE_56Mx:: 40. Доступные входа/выходы PLC
//=================================================================================================
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)24			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)32			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)6			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)4			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                X00  X01  X02  X03  X04  X05  X06  X07  X08  X09  X10  X11  X12  X13  X14  X15  X16  X17
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PC8, PA8, PA0, PB3, PD12,PB15,PA7, 255, 255, PA6, PA2, PB8, PB9, PE0, PE1,PC13,PE3, 	RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB13,PB14,PB11,PB12,PE15,PB10,PE13,PE14,255, 255, PE11,PE12,PE9, PE10,PE7, PE8,PB0, PB1,  RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA1, PA3, PC4, PC5, PC0, PC1,																RT_HW_PGM_END};																																		
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																					RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9, 255, 255,255,   			RT_HW_PGM_END};
//=================================================================================================
//						PLC FLE_56Mx:: 50. Описание входов/выходов PLC (для вывода на консоль)
//=================================================================================================
    #define RT_HW_PERMIT_TEXT_PIN_DOT0 	"DOT: Y00,Y01,Y02,Y03,Y04,Y05,Y06,Y07"		
    #define RT_HW_PERMIT_TEXT_PIN_DOT1 	"DOT: Y10,Y11,Y12,Y13,Y14,Y15,Y16,Y17"
    #define RT_HW_PERMIT_TEXT_PIN_DOT2 	"DOT: Y20,Y21,Y22,Y23,Y24,Y25,Y26,Y27"			
//  #define RT_HW_PERMIT_TEXT_PIN_DOT3 	"DOT: Y30,Y31,Y32,Y33,Y34,Y35,Y36,Y37"		
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_TEXT_PIN_DIN0 	"DIN: X00,X01,X02,X03,X04,X05,X06,X07"	
    #define RT_HW_PERMIT_TEXT_PIN_DIN1 	"DIN: X10,X11,X12,X13,X14,X15,X16,X17"	
    #define RT_HW_PERMIT_TEXT_PIN_DIN2 	"DIN: X20,X21,X22,X23,X24,X25,X26,X27"	
    #define RT_HW_PERMIT_TEXT_PIN_DIN3 	"DIN: X30,X31,X32,X33,X34,X35,X36,X37"	
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_TEXT_PIN_ADC0 	"ADC: AD0,AD1,AD2,AD3,AD4,AD5"				
//  #define RT_HW_PERMIT_TEXT_PIN_ADC1 	"ADC: Set1,Set2"								
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_TEXT_PIN_DAC0 	"DAC: DAC0,DAC1"								
//=================================================================================================