//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											Контроллеры ESP32S
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//											Контроллер ESP32S M5StackCore
//=================================================================================================
#if !defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_ESP32) &&(\
 	 defined(ARDUINO_M5Stack_Core_ESP32) ||\
	 defined(ARDUINO_M5Stack_Core_ESP32_EIS) ||\
	 defined(ARDUINO_M5Stack_Core_ESP32_EIS_BTH) \
    )
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_CODE     (uint8_t)50 							//--Идентификатор выбора платы;
	#define  RT_HW_CORE_ESP32S											//--Идентификатор ядра;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_ESP32S_DEV  									//--Идентификатор типа платы;
//-------------------------------------------------------------------------------------------------
 #if defined(ARDUINO_M5Stack_Core_ESP32_EIS_BTH)
	#define  RT_HW_BOARD_NAME  	 "FLProg: M5Stack Core with BTH"		//--Наименование платы;
	#define  RT_HW_BOARD_ESP32S_DEV_BTH  								//--Идентификатор типа платы;
	#define  RT_HW_ESP32_PERMIT_BTH										//--Подключить BlueTooth ESP32 (BluetoothSerial.h);	
//-------------------------------------------------------------------------------------------------
 #elif defined(ARDUINO_M5Stack_Core_ESP32_EIS_BTH) 
	#define  RT_HW_BOARD_NAME  	 "FLProg: M5Stack Core"					//--Наименование платы;	 
//-------------------------------------------------------------------------------------------------
 #else
	#define  RT_HW_BOARD_NAME  	 "M5Stack Core"							//--Наименование платы;	 
 #endif	 	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_MEMORY   "FLASH=4Mb; SRAM=512Kb;  ROM=448Kb;"//--Параметры памяти;
	#define  RT_HW_BOARD_FCPU     (uint16_t)160				//--Частота процессора;	
	#define  RT_HW_BOARD_VCC   	  3300						//--Значение Vсс процессора;
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
	#define  RT_HW_BOARD_PIN_CONTROL   		12		//--Контроль цикла loop();12,  2, 34,36,25,
	#define  RT_HW_BOARD_PIN_LED        	2		//--Led на плате;
	#define  RT_HW_BOARD_PIN_BUTTON    		34		//--Кнопка на плате;
	#define  RT_HW_BOARD_PIN_ADC       		36		//--ADC;
	#define  RT_HW_BOARD_PIN_DAC       		25		//--DAC0;
	#define  RT_HW_BOARD_PIN_DAC0      		25		//--DAC0;
	#define  RT_HW_BOARD_PIN_DAC1      		26		//--DAC1;	
	#define  RT_HW_BOARD_PIN_PWM       		26		//--PWM;	
	#define  RT_HW_BOARD_PIN_CS0        	5		//--SPI cs.0;
	#define  RT_HW_BOARD_PIN_CS1        	4		//--SPI cs.1;	
	#define  RT_HW_BOARD_PIN_CS2       		15		//--SPI cs.2;
	#define  RT_HW_BOARD_PIN_CS3        	2		//--SPI cs.3;	
//==================================================================================================
//								1.3. Тестовые устройства
//==================================================================================================
	#define  RT_HW_BOARD_PIN_DS1820    		33		//--Сенсор DS1820;33, 14,13,
	#define  RT_HW_BOARD_PIN_SR04      		13		//--Сенсор HC-SR04;
	#define  RT_HW_BOARD_PIN_SERVO0    		12		//--Серво-двигатель;
	#define  RT_HW_BOARD_PIN_SERVO1    		14		//--Серво-двигатель 1;
	#define  RT_HW_BOARD_PIN_TEST      		27		//--Пин тестовый;	
//=================================================================================================
//								1.4. Тестовые UART
//=================================================================================================		
	#define  RT_HW_BOARD_UART_CONSOLE   	0		//--Консоль через UART;	
	#define  RT_HW_BOARD_UART_MODBUS1  		1		//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_HMI  	   		2		//--Номер UART для HMI 1;
//==================================================================================================
//								1.5. Коммуникации
//==================================================================================================
    #define  RT_HW_PERMIT_UART0					//--Разрешение работы с UART0;
    #define  RT_HW_PERMIT_UART1					//--Разрешение работы с UART1;
    #define  RT_HW_PERMIT_UART2					//--Разрешение работы с UART2;			
    #define  RT_HW_PERMIT_I2C0					//--Разрешение работы с i2c0; 
    #define  RT_HW_PERMIT_I2C1					//--Разрешение работы с i2c0;
    #define  RT_HW_PERMIT_SPI0					//--Разрешение работы с SPI0; 
    #define  RT_HW_PERMIT_SPI1					//--Разрешение работы с SPI1;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART0_RX   				3
    #define  RT_HW_UART0_TX   				1	
    #define  RT_HW_UART1_RX   				14
    #define  RT_HW_UART1_TX   				27	 
	#define  RT_HW_UART2_RX   				16
    #define  RT_HW_UART2_TX   				17	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SCL  				22	
    #define  RT_HW_I2C0_SDA  				21
    #define  RT_HW_I2C1_SCL  				32	
    #define  RT_HW_I2C1_SDA  				33
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK  				18
    #define  RT_HW_SPI0_MISO 				19
    #define  RT_HW_SPI0_MOSI 				23
    #define  RT_HW_SPI0_SS    				5	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI1_SCK  				14
    #define  RT_HW_SPI1_MISO 				12
    #define  RT_HW_SPI1_MOSI 				13
    #define  RT_HW_SPI1_SS   				15	
//=================================================================================================
//										1.10. Доступные пины
//=================================================================================================  
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)39	
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)29	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)29	   
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)29
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)6		
    #define RT_HW_PERMIT_PIN_SRV	(uint8_t)2			
    #define RT_HW_PERMIT_PIN_INT	(uint8_t)33 
    #define RT_HW_PERMIT_PIN_N5V	(uint8_t)39 		
//--------------------------------Доступные пины контроллера---------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,39,	 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={ 0, 1, 2, 3, 4, 5,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,				RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={ 0, 1, 2, 3, 4, 5,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,			 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={ 0, 1, 2, 3, 4, 5,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,39, 	  	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={36,39,32,33,34,35, 	  		  							  	RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PIN_SRV_ID[] ={12,14,														RT_HW_PGM_END};		
	const uint8_t PROGMEM RT_HW_PGM_PIN_DAC_ID[] ={25,26,													  	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={ 0, 1, 2, 3, 4, 5,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,39,     	RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15,16,17,18,19,
												   20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,39,     	RT_HW_PGM_END};	
//-------------------------------------------------------------------------------------------------
#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++