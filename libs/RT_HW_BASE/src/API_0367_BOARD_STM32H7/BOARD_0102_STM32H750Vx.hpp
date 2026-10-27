//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										Контроллеры STM32H750Vx
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++


//=================================================================================================
//										Контроллер STM32H750VTX
//=================================================================================================
#if !defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_STM32) && defined(ARDUINO_WeActMiniH750VBTX)
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_CODE     (uint8_t)76 					//--Идентификатор выбора платы;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_BOARD_NAME  	  "WeActSTM32H750VBTX"     	//--Наименование платы;	
	#define  RT_HW_BOARD_MEMORY   "FLASH=128kB; RAM=264Kb;"	//--Параметры памяти;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_FCPU     (uint16_t)480			//--Частота процессора;	
	#define  RT_HW_BOARD_VCC   	  3300					//--Значение Vсс процессора;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_CORE_STM32H7							//--Допол.идентификатор программной среды;
	#define  RT_HW_CORE_STM32H7V						//--Допол.идентификатор программной среды;
    #define  RT_HW_BOARD_WeActSTM32H7XXVxx  			//--Идентификатор типа платы;
//=================================================================================================
//										1. Ограничение ресурсов контроллера
//=================================================================================================

//=================================================================================================
//										2. Системные пины
//=================================================================================================
	#define  RT_HW_BOARD_PIN_CONTROL   		PE2			//--Контроль цикла loop();
	#define  RT_HW_BOARD_PIN_LED       		PE3			//--Led на плате;
	#define  RT_HW_BOARD_PIN_BUTTON    		PC13		//--Кнопка на плате;
	#define  RT_HW_BOARD_PIN_ADC       		PC0			//--ADC;
	#define  RT_HW_BOARD_PIN_PWM       		PB5			//--PWM;
	#define  RT_HW_BOARD_PIN_WS2812    		PB6			//--WS2812;	
//=================================================================================================
//										3. Пины cs тестовые
//=================================================================================================
	#define  RT_HW_BOARD_PIN_CS0       		PD10		//--SPI cs.0;17,20,21,22
	#define  RT_HW_BOARD_PIN_CS1       		PD11		//--SPI cs.1;	
	#define  RT_HW_BOARD_PIN_CS2       		PD12		//--SPI cs.2;
//=================================================================================================
//										4. Пины тестовых устройств
//=================================================================================================
	#define  RT_HW_BOARD_PIN_DS1820    		PB6			//--Сенсор DS1820;
	#define  RT_HW_BOARD_PIN_SR04      		PB7			//--Сенсор HC-SR04;	
//=================================================================================================
//										5. Номера тестовых UART
//=================================================================================================	
	#define  RT_HW_BOARD_UART_CONSOLE   	210			//--Консоль через USB;
	#define  RT_HW_BOARD_UART_MODBUS1   	1			//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_MODBUS2   	2			//--Номер UART для MODBUS 2;
	#define  RT_HW_BOARD_UART_HMI1			3			//--Номер UART для панели HMI;
//=================================================================================================
//										5. FLASH память SPI
//=================================================================================================
	#define  RT_HW_PERMIT_FLASH_SPI	    	2			//--Разрешение FLASH через SPI;
	#define  RT_HW_BOARD_PIN_SPF_SPI   		0			//--SPF номер SPI;
	#define  RT_HW_BOARD_PIN_SPF_CS    		PD6			//--SPF CS;
//=================================================================================================
//										6. FLASH память QSPI
//=================================================================================================
	#define  RT_HW_PERMIT_FLASH_QSPI		6			//--Разрешение QSPI;	
	#define  RT_HW_BOARD_PIN_QSPI_D0   		PD11		//--QSPI D0;
	#define  RT_HW_BOARD_PIN_QSPI_D1   		PD12		//--QSPI D1;
	#define  RT_HW_BOARD_PIN_QSPI_D2   		PE2			//--QSPI D2;
	#define  RT_HW_BOARD_PIN_QSPI_D3   		PD13		//--QSPI D3;
	#define  RT_HW_BOARD_PIN_SDIO_CK   		PB2			//--QSPI CK;
	#define  RT_HW_BOARD_PIN_SDIO_NCS 		PB6			//--QSPI CMD;
//=================================================================================================
//										7. SD-карта
//=================================================================================================
	#define  RT_HW_PERMIT_SD_MICRO			7			//--Разрешение SD;	
	#define  RT_HW_BOARD_PIN_SD_D0   		PC8			//--SDIO D0;
	#define  RT_HW_BOARD_PIN_SD_D1   		PC9			//--SDIO D1;
	#define  RT_HW_BOARD_PIN_SD_D2   		PC10		//--SDIO D2;
	#define  RT_HW_BOARD_PIN_SD_D3   		PC11		//--SDIO D3;
	#define  RT_HW_BOARD_PIN_SD_CK   		PC12		//--SDIO CK;
	#define  RT_HW_BOARD_PIN_SD_CMD 		PD2			//--SDIO CMD;
	#define  RT_HW_BOARD_PIN_SD_SW			PD4			//--SDIO SW;
//=================================================================================================
//										10. Коммуникации
//=================================================================================================
//=================================================================================================
//										10.1. UART
//=================================================================================================
    #define  RT_HW_PERMIT_UART0							//--Разрешение работы с UART0; 
    #define  RT_HW_PERMIT_UART1							//--Разрешение работы с UART1; 
    #define  RT_HW_PERMIT_UART2							//--Разрешение работы с UART2; 
    #define  RT_HW_PERMIT_UART3							//--Разрешение работы с UART3;
    #define  RT_HW_PERMIT_UART4							//--Разрешение работы с UART4; 
    #define  RT_HW_PERMIT_UART5							//--Разрешение работы с UART5; 	
    #define  RT_HW_PERMIT_UART6							//--Разрешение работы с UART6; 
    #define  RT_HW_PERMIT_UART7							//--Разрешение работы с UART7; 	
    #define  RT_HW_PERMIT_UART8							//--Разрешение работы с UART8;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_CREATE_UART1							//--Разрешение создания объекта UART1;
	#define  RT_HW_CREATE_UART2							//--Разрешение создания объекта UART2;
	#define  RT_HW_CREATE_UART3							//--Разрешение создания объекта UART3;
	#define  RT_HW_CREATE_UART4							//--Разрешение создания объекта UART4;
	#define  RT_HW_CREATE_UART5							//--Разрешение создания объекта UART5;
	#define  RT_HW_CREATE_UART6							//--Разрешение создания объекта UART6;
	#define  RT_HW_CREATE_UART7							//--Разрешение создания объекта UART7;
	#define  RT_HW_CREATE_UART8							//--Разрешение создания объекта UART8;	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART0_RX  		PA12
    #define  RT_HW_UART0_TX  		PA11	
    #define  RT_HW_UART1_RX   		PA10
    #define  RT_HW_UART1_TX   		PA9	
    #define  RT_HW_UART2_RX   		PA3
    #define  RT_HW_UART2_TX   		PA2	
    #define  RT_HW_UART3_RX 		PD9
    #define  RT_HW_UART3_TX 		PD8
    #define  RT_HW_UART3_DE 		PB14	
    #define  RT_HW_UART4_RX  		PA1
    #define  RT_HW_UART4_TX  		PA0
    #define  RT_HW_UART4_DE 		PB15	
    #define  RT_HW_UART5_RX   		PB12
    #define  RT_HW_UART5_TX  		PB13	
    #define  RT_HW_UART6_RX  		PC7
    #define  RT_HW_UART6_TX  		PC6
    #define  RT_HW_UART7_RX  		PE7
    #define  RT_HW_UART7_TX  		PE8	
    #define  RT_HW_UART8_RX  		PE0
    #define  RT_HW_UART8_TX  		PE1	
//=================================================================================================
//								1.5.2. Коммуникации i2c
//=================================================================================================
    #define  RT_HW_PERMIT_I2C0					//--Разрешение работы с i2c0; 
    #define  RT_HW_PERMIT_I2C1					//--Разрешение работы с i2c1;	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_CREATE_I2C1					//--Разрешение создания объекта i2c1;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SCL   		PB8	
    #define  RT_HW_I2C0_SDA   		PB9
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C1_SCL  		PB10
    #define  RT_HW_I2C1_SDA  		PB11
//=================================================================================================
//								1.5.3. Коммуникации SPI
//=================================================================================================
    #define  RT_HW_PERMIT_SPI0					//--Разрешение работы с SPI0; 
    #define  RT_HW_PERMIT_SPI3					//--Разрешение работы с SPI3; 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_CREATE_SPI3					//--Разрешение создания объекта SPI3;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK  		PB3
    #define  RT_HW_SPI0_MISO 		PB4
    #define  RT_HW_SPI0_MOSI 		PD7
    #define  RT_HW_SPI0_SS   		PD6	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI3_SCK  		PE12
    #define  RT_HW_SPI3_MISO 		PE5
    #define  RT_HW_SPI3_MOSI 		PE14
    #define  RT_HW_SPI3_SS   		PE11	
//=================================================================================================
//								1.5.4. Коммуникации Ethernet
//=================================================================================================
	#define RT_HW_PERMIT_ETH				//--Разрешение работы с ETH;
//=================================================================================================
//								1.5.5. Коммуникации CAN
//=================================================================================================
	#define RT_HW_PERMIT_CAN				//--Разрешение работы с CAN;
//=================================================================================================
//								1.5.6. Коммуникации дисплей TFT
//=================================================================================================
	#define RT_HW_PERMIT_TFT				//--Разрешение работы с TFT;
//=================================================================================================
//								1.10. Доступные пины
//=================================================================================================  
	#ifndef PC2
		#define PC2 255
	#endif
	#ifndef PC3
		#define PC3 255
	#endif
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)75	 	//--Доступные
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)75		//--Дискретный выход;	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)26		//--PWM;  
	#define RT_HW_PERMIT_PIN_DAC	(uint8_t)2		//--DAC;	
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)75		//--Дискретный
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)16		//--ADC;	
    #define RT_HW_PERMIT_PIN_INT	(uint8_t)75		//--Прерывания;
	#define RT_HW_PERMIT_PIN_SRV	(uint8_t)2		//--Серво;
//=================================================================================================
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12,PA13,PA14,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15,	RT_HW_PGM_END}; 
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={PA0,PA1,PA2,PA3,255,PA5,PA6,PA7,PA8,PA9,PA10,PA11, 255, 255, 255, 255,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255, 255,PB13,PB14,PB15,	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DAC_ID[] ={PA4,PA5, 																	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PB0,PB1,PC0, PC1, PC2, PC3, PC4, PC5,  	RT_HW_PGM_END};										
	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15,	RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PC0,PC1,PC2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
												   PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15,	RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_SRV_ID[] ={PB14,PB15,  																RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
//=================================================================================================
#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
