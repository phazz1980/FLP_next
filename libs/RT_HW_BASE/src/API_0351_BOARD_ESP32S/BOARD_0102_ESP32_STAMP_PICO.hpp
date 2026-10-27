//=================================================================================================
//								1.Контроллеры ESP32 STAMP_PICO
//=================================================================================================
#if !defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_ESP32) && (\
	 defined(ARDUINO_ESP32S_STAMP_PICO) ||\
	 defined(ARDUINO_ESP32S_STAMP_PICO_EIS)\
    )	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_CODE     (uint8_t)50 							//--Идентификатор выбора платы;
	#define  RT_HW_CORE_ESP32S											//--Идентификатор ядра;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_ESP32S_DEV  									//--Идентификатор типа платы;									//--Идентификатор типа платы;
//-------------------------------------------------------------------------------------------------
 #if defined(ARDUINO_ESP32S_STAMP_PICO_EIS)
	#define  RT_HW_BOARD_NAME  	 "FLProg: ESP32S STAMP PICO"			//--Наименование платы;
//-------------------------------------------------------------------------------------------------
 #elif defined(ARDUINO_ESP32S_STAMP_PICO) 
	#define  RT_HW_BOARD_NAME  	 "ESP32S STAMP PICO"					//--Наименование платы;	 
//-------------------------------------------------------------------------------------------------
 #else
	#define  RT_HW_BOARD_NAME  	 "ESP32S STAMP PICO"					//--Наименование платы;	 
 #endif	 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_MEMORY   "FLASH=1310Kb; RAM=328Kb; Ext_FLASH=4/16Mb;"//--Параметры памяти;
	#define  RT_HW_BOARD_FCPU     (uint16_t)80					//--Частота процессора;	
	#define  RT_HW_BOARD_VCC   	  3300							//--Значение Vсс процессора;
//=================================================================================================
//								1.1. Ограничение ресурсов контроллера
//=================================================================================================
    #define RT_HW_SPECIFIC_USING_1  "Strapping pins 0,2,4,5,12,15"
    #define RT_HW_SPECIFIC_USING_2  "Connected to the integrated SPI flash pins 6,7,8,9,11"
    #define RT_HW_SPECIFIC_USING_3  "Boot fail if pulled high"	
    #define RT_HW_SPECIFIC_USING_4  "Input only pins >=34"	
	#define RT_HW_SPECIFIC_USING_5  "Download:: pin.0-pulled up; pin.1-debug output at boot"
	#define RT_HW_SPECIFIC_USING_6  "Download:: pin.2-connected to on-board Led; pin.3-HIGH at boot"
//=================================================================================================
//								1.2. Системные пины
//=================================================================================================
	#define  RT_HW_BOARD_PIN_BUTTON    		39		//--Кнопка на плате;
	#define  RT_HW_BOARD_PIN_BOOT      		0		//--Кнопка на плате BOOT;	
	#define  RT_HW_BOARD_PIN_WS2812    		27		//--ADC;
	#define  RT_HW_BOARD_PIN_IR        		12		//--IR;	
	#define  RT_HW_BOARD_PIN_DAC       		25		//--DAC0;
	#define  RT_HW_BOARD_PIN_DAC0      		25		//--DAC0;
	#define  RT_HW_BOARD_PIN_DAC1      		26		//--DAC1;	
	#define  RT_HW_BOARD_PIN_PWM       		26		//--PWM;			
//=================================================================================================
//								1.3. Тестовые устройства
//=================================================================================================
	#define  RT_HW_BOARD_PIN_DS1820  		26		//--Сенсор DS1820;33, 14,13,	
//=================================================================================================
//								1.4. Тестовые UART
//=================================================================================================	
	#define  RT_HW_BOARD_UART_CONSOLE   	0		//--Консоль через UART;	
	#define  RT_HW_BOARD_UART_MODBUS1  		1		//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_HMI  	   		2		//--Номер UART для HMI 1;
//=================================================================================================
//								1.5. Коммуникации
//=================================================================================================
    #define  RT_HW_PERMIT_UART0						//--Разрешение работы с UART0;
    #define  RT_HW_PERMIT_UART1						//--Разрешение работы с UART1;
    #define  RT_HW_PERMIT_UART2						//--Разрешение работы с UART2;	
    #define  RT_HW_PERMIT_I2C0						//--Разрешение работы с i2c0; 	
    #define  RT_HW_PERMIT_SPI0						//--Разрешение работы с SPI0; 	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART0_RX   				3
    #define  RT_HW_UART0_TX   				1	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART1_RX     			23
    #define  RT_HW_UART1_TX   				33	
//------------------------------------------------------------------------------------------------- 
	#define  RT_HW_UART2_RX   				26
    #define  RT_HW_UART2_TX   				32	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SDA  				26
	#define  RT_HW_I2C0_SCL  				32	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK  				32
    #define  RT_HW_SPI0_MISO 				19
    #define  RT_HW_SPI0_MOSI 				23
    #define  RT_HW_SPI0_SS   				33		
//=================================================================================================
//								1.10. Доступные пины
//=================================================================================================  
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)15	
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)15	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)15	   
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)15
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)4				
//  #define RT_HW_PERMIT_PIN_INT	(uint8_t)15 
//  #define RT_HW_PERMIT_PIN_N5V	(uint8_t)15 		
//--------------------------------Доступные пины контроллера---------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={ 0, 1, 3, 9,12,18,19,20,21,22,25,26,27,32,33,36,39,	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={ 0, 1, 3, 9,12,18,19,20,21,22,25,26,27,32,33,36,39,	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={ 0, 1, 3, 9,12,18,19,20,21,22,25,26,27,32,33,36,39,	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={ 0, 1, 3, 9,12,18,19,20,21,22,25,26,27,32,33,36,39, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={36,39,32,33, 	  		  						RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_DAC_ID[] ={25,26,											RT_HW_PGM_END};
//	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={ 0, 1, 3,18,19,20,21,22,25,26,27,32,33,36,39, 	RT_HW_PGM_END};	
//	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={ 0, 1, 3,18,19,20,21,22,25,26,27,32,33,36,39,    RT_HW_PGM_END};	
//-------------------------------------------------------------------------------------------------
#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++