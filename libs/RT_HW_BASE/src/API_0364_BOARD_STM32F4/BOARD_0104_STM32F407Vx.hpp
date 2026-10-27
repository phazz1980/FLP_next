//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//									Контроллер STM32F407V
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================						
//									Контроллер STM32F407Vx 
//=================================================================================================
#if !defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_STM32) && \
    (defined(FLP_BOARD_BLACK_F407VE) || defined(FLP_BOARD_BLACK_F407VG) ||\
	 defined(ARDUINO_BLACK_F407VE)   || defined(ARDUINO_BLACK_F407VG))
//-------------------------------------------------------------------------------------------------
    #define	 RT_HW_BOARD_CODE  		(uint16_t)73  				//--Идентификатор выбора платы c кодом платы;
//-------------------------------------------------------------------------------------------------
//									0.1. BLACK_F407VE
//-------------------------------------------------------------------------------------------------
	#if   defined(FLP_BOARD_BLACK_F407VE)                            
		 #define  RT_HW_BOARD_NAME     	"FLProg: BLACK F407VE"		//--Наименование платы;		
		 #define  RT_HW_BOARD_MEMORY	"Flash=512Kb; SRAM=System(192+16+64)+Backup(4)Kb;"//--Параметры памяти;	
//-------------------------------------------------------------------------------------------------
	#elif defined(FLP_BOARD_BLACK_F407GE) 	
		 #define  RT_HW_BOARD_NAME     	"FLProg: BLACK F407VG"		//--Наименование платы;		
		 #define  RT_HW_BOARD_MEMORY	"Flash=1024Kb; SRAM=System(192+16+64)+Backup(4)Kb;"//--Параметры памяти;	
		 #define  RT_HW_BLOCK_USB
//-------------------------------------------------------------------------------------------------		
	#elif defined(ARDUINO_BLACK_F407VE)	                           
		 #define  RT_HW_BOARD_NAME     	"BLACK F407VE(DevEBox STM32F407VET6)"		//--Наименование платы;		
		 #define  RT_HW_BOARD_MEMORY	"Flash=512Kb; SRAM=System(192+16+64)+Backup(4)Kb;"//--Параметры памяти;	
		 #define  RT_HW_BLOCK_USB
//-------------------------------------------------------------------------------------------------		
	#elif defined(ARDUINO_BLACK_F407GE)	                           
		 #define  RT_HW_BOARD_NAME     	"BLACK F407GE(DevEBox STM32F407GET6)"		//--Наименование платы;		
		 #define  RT_HW_BOARD_MEMORY	"Flash=1024Kb; SRAM=System(192+16+64)+Backup(4)Kb;"//--Параметры памяти;	
//-------------------------------------------------------------------------------------------------
	#else							
		 #define  RT_HW_BOARD_NAME     	"BLACK F407Vx(DevEBox STM32F407VxT6)"		//--Наименование платы;		
		 #define  RT_HW_BOARD_MEMORY	"Flash=256-1024Kb; SRAM=System(192+16+64)+Backup(4)Kb;"//--Параметры памяти;	
	#endif
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_BOARD_FCPU       (uint16_t)160 		//--Частота процессора;	
    #define  RT_HW_BOARD_VCC      	(uint16_t)3300		//--Значение Vсс процессора; 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_CORE_STM32F4							//--Допол.идентификатор программной среды;
	#define  RT_HW_CORE_STM32F4V						//--Допол.идентификатор программной среды;
//=================================================================================================
#if !defined(PB2)
	#define  PB2 255
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(PA13)
	#define  PA13 255
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(PA14)
	#define  PA14 255
#endif
//=================================================================================================
//										1. Ограничение ресурсов контроллера
//=================================================================================================

//=================================================================================================
//										2. Системные пины
//=================================================================================================
	#define  RT_HW_BOARD_PIN_CONTROL   		PE5			//--Контроль цикла loop();
	#define  RT_HW_BOARD_PIN_LED       		PA1			//--Led на плате;
	#define  RT_HW_BOARD_PIN_BUTTON    		PA0			//--Button на плате;
	#define  RT_HW_BOARD_PIN_ADC       		PC0			//--ADC;
	#define  RT_HW_BOARD_PIN_PWM       		PB6			//--PWM;
//=================================================================================================
//										3. Пины cs тестовые
//=================================================================================================
	#define  RT_HW_BOARD_PIN_CS0       		PD3			//--SPI cs.0;
	#define  RT_HW_BOARD_PIN_CS1       		PD4			//--SPI cs.1;
	#define  RT_HW_BOARD_PIN_CS2       		PD5			//--SPI cs.2;	
	#define  RT_HW_BOARD_PIN_CS3       		PD6			//--SPI cs.3;
//=================================================================================================
//										4. Пины тестовых устройств
//=================================================================================================
	#define  RT_HW_BOARD_PIN_DS1820    		PE11		//--Сенсор DS1820;
	#define  RT_HW_BOARD_PIN_SR04      		PE13		//--Сенсор HC-SR04;
	#define  RT_HW_BOARD_PIN_TEST      		PE11		//--Пин тестовый;		
//=================================================================================================
//										5. Номера тестовых UART
//=================================================================================================	
	#define  RT_HW_BOARD_UART_CONSOLE   	210			//--Консоль через USB;
	#define  RT_HW_BOARD_UART_MODBUS1   	1			//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_MODBUS2   	2			//--Номер UART для MODBUS 2;
	#define  RT_HW_BOARD_UART_HMI1			3			//--Номер UART для панели HMI;	
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
//-------------------------------------------------------------------------------------------------
//	#define RT_HW_CREATE_UART1							//--Разрешение создания объекта Serial1 (создан в CORE);
	#define RT_HW_CREATE_UART2							//--Разрешение создания объекта Serial2;
	#define RT_HW_CREATE_UART3							//--Разрешение создания объекта Serial3;
	#define RT_HW_CREATE_UART4							//--Разрешение создания объекта Serial4;
	#define RT_HW_CREATE_UART5							//--Разрешение создания объекта Serial5;
	#define RT_HW_CREATE_UART6							//--Разрешение создания объекта Serial5;
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART0_RX)
    #define  RT_HW_UART0_RX  		PA12 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART0_TX)
    #define  RT_HW_UART0_TX  		PA11
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART1_RX)
    #define  RT_HW_UART1_RX  		PA10 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART1_TX)
    #define  RT_HW_UART1_TX  		PA9
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART2_RX)
    #define  RT_HW_UART2_RX  		PA3 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART2_TX)
    #define  RT_HW_UART2_TX  		PA2
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART3_RX)
    #define  RT_HW_UART3_RX  		PD9 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART3_TX)
    #define  RT_HW_UART3_TX  		PD8
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART4_RX)
    #define  RT_HW_UART4_RX  		PC11 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART4_TX)
    #define  RT_HW_UART4_TX  		PC10
#endif
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART5_RX)
    #define  RT_HW_UART5_RX  		PD2 
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART5_TX)
    #define  RT_HW_UART5_TX  		PC12
#endif	
//-------------------------------------------------------------------------------------------------
#if !defined(RT_HW_UART6_RX)
    #define  RT_HW_UART6_RX  		PC7
#endif
//---------------------------------------------------------
#if !defined(RT_HW_UART6_TX)
    #define  RT_HW_UART6_TX  		PC6
#endif	
//-------------------------------------------------------------------------------------------------
//    #define  RT_HW_UART0_RX   		PA12
//    #define  RT_HW_UART0_TX   		PA11	
//    #define  RT_HW_UART1_RX   		PA10
//    #define  RT_HW_UART1_TX   		PA9	
//    #define  RT_HW_UART2_RX   		PA3
//    #define  RT_HW_UART2_TX   		PA2
//    #define  RT_HW_UART3_RX  		PD9
//    #define  RT_HW_UART3_TX   		PD8
//    #define  RT_HW_UART4_RX  		PC11
//    #define  RT_HW_UART4_TX   		PC10
//    #define  RT_HW_UART5_RX  		PD2
//    #define  RT_HW_UART5_TX   		PC12	
//    #define  RT_HW_UART6_RX  		PC7
//    #define  RT_HW_UART6_TX   		PC6	
//=================================================================================================
//										10.2. i2c
//=================================================================================================
    #define  RT_HW_PERMIT_I2C0							//--Разрешение работы с i2c0; 

//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SCL   		PB8
    #define  RT_HW_I2C0_SDA   		PB9
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_PERMIT_I2C1							//--Разрешение работы с i2c1;
	#define  RT_HW_CREATE_I2C1							//--Разрешение создания объекта i2c1;
    #define  RT_HW_I2C1_SCL  		PB10
    #define  RT_HW_I2C1_SDA  		PB11
//-------------------------------------------------------------------------------------------------	
//	#define RT_HW_PERMIT_I2C100	
//	#define RT_HW_CREATE_I2C100	
//	#define RT_HW_I2C100_SDA		PC0
//	#define	RT_HW_I2C100_SCL		PC1	
//=================================================================================================
//										10.3. SPI
//=================================================================================================
    #define  RT_HW_PERMIT_SPI0							//--Разрешение работы с SPI0; 
    #define  RT_HW_PERMIT_SPI1							//--Разрешение работы с SPI1; 
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_CREATE_SPI1							//--Разрешение создания объекта SPI1
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK   		PB13
    #define  RT_HW_SPI0_MISO  		PB14
    #define  RT_HW_SPI0_MOSI  		PB15
    #define  RT_HW_SPI0_SS   		PB12	
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI1_SCK   		PB3
    #define  RT_HW_SPI1_MISO  		PB4
    #define  RT_HW_SPI1_MOSI  		PB5
    #define  RT_HW_SPI1_SS   		PA15
//=================================================================================================
//									10.4. CAN
//================================================================================================
	#define  RT_HW_PERMIT_CAN					//--Разрешение работы с CAN;
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_CAN_RX			PD0
    #define  RT_HW_CAN_TX			PD1
//=================================================================================================
//									10.5. ETHERNET
//=================================================================================================
    #define RT_HW_PERMIT_ETH0					//--Разрешение работы с ETH0;
    #define RT_HW_PERMIT_ETH1					//--Разрешение работы с ETH1;		
	const uint8_t PROGMEM RT_HW_PGM_ETH0_ID[] ={0,PD13,PD14,255,255, RT_HW_PGM_END};//--Ethernet.0->bus SPI,pin: CS,INI,RST,OE;
	const uint8_t PROGMEM RT_HW_PGM_ETH1_ID[] ={0,PC9,  255,255,255, RT_HW_PGM_END};//--Ethernet.1->bus SPI,pin: CS,INI,RST,OE;
//==================================================================================================
//									20. Доступные пины
//==================================================================================================  
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)76	 		//--Доступные
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)76			//--Дискретный выход;	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)32			//--PWM;  
	#define RT_HW_PERMIT_PIN_DAC	(uint8_t)2			//--DAC;	
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)78			//--Дискретный
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)17			//--ADC;	
    #define RT_HW_PERMIT_PIN_INT	(uint8_t)78			//--Прерывания;
    #define RT_HW_PERMIT_PIN_N5V	(uint8_t)16			//--Несовместимые с 5V;	
	#define RT_HW_PERMIT_PIN_SRV	(uint8_t)2			//--Серво;
//--------------------------------Доступные пины контроллера---------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12,PA13,PA14,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
										           PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END};									
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
										           PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
										           PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
                                                   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END}; 
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={PA0,PA1,PA2,PA3,255,PA5,PA6,PA7,PA8,PA9,PA10,PA11, 255, 255, 255, 255,
												   PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255, 255,PB13,PB14,PB15, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DAC_ID[] ={PA4,PA5, 																RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
										           PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
										           PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
										           PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PB0,PB1,PC0, PC1, PC2, PC3, PC4, PC5,  	RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
												   PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10,PB11,PB12,PB13,PB14,PB15,
										           PC0,PC1,PC2,PC3,PC4,PC5,PC6,PC7,PC8,PC9,PC10,PC11,PC12,PC13, 255, 255,
												   PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PD8,PD9,PD10,PD11,PD12,PD13,PD14,PD15,
												   PE0,PE1,PE2,PE3,PE4,PE5,PE6,PE7,PE8,PE9,PE10,PE11,PE12,PE13,PE14,PE15, 	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PB0,PB1,PC0, PC1, PC2, PC3, PC4, PC5,  	RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_SRV_ID[] ={PA4,PA5, 																RT_HW_PGM_END};
#endif
//=================================================================================================


