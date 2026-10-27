
/*
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									3.MCU GENERIC F401CXX
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_BOARD_SELECT) && defined(RT_HW_BOARD_GENERIC_F401CXXX)
//-------------------------------------------------------------------------------------------------
#define      RT_HW_BOARD_SELECT	    "BOARD GENERIC F401CXX"
//-------------------------------------------------------------------------------------------------
//							Permission to work with Link
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_UART0					//--Permission to work with UART0;
#define RT_HW_PERMIT_UART1					//--Permission to work with UART1;
#define RT_HW_PERMIT_UART2					//--Permission to work with UART2;
#define RT_HW_PERMIT_UART6					//--Permission to work with UART6;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_I2C0					//--Permission to work with i2c0;
#define RT_HW_PERMIT_I2C1					//--Permission to work with i2c1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_SPI0					//--Permission to work with SPI0;
//#define RT_HW_PERMIT_SPI1					//--Permission to work with SPI1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_CAN					//--Permission to work with CAN;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_DAC					//--Permission to work with DAC;
//-------------------------------------------------------------------------------------------------
//							Permission to cteate with Link
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_UART1					//--Permission to create Serail2 for UART1;
#define RT_HW_CREATE_UART6					//--Permission to create Serial6 for UART6;
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_I2C1					//--Permission to create Wire1 for i2c1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_SPI1					//--Permission to work with SPI1;
//-------------------------------------------------------------------------------------------------
//							Relabeling pins
//-------------------------------------------------------------------------------------------------
#if !defined(PB11)
#define PB11 PB_11
#endif
//=================================================================================================
//									Pins for Link
//=================================================================================================

//=================================================================================================
//--------------------------------Параметры UART---------------------------------------------------	
enum {RT_HW_UART0_RX=PA12, RT_HW_UART0_TX=PA11, RT_HW_UART0_DE=255,
	  RT_HW_UART1_RX=PA10, RT_HW_UART1_TX=PA9,  RT_HW_UART1_DE=255, 
	  RT_HW_UART2_RX=PA3,  RT_HW_UART2_TX=PA2,  RT_HW_UART2_DE=255,
	  RT_HW_UART3_RX=255,  RT_HW_UART3_TX=255,  RT_HW_UART3_DE=255,
	  RT_HW_UART4_RX=255,  RT_HW_UART4_TX=255,  RT_HW_UART4_DE=255,
	  RT_HW_UART5_RX=255,  RT_HW_UART5_TX=255,  RT_HW_UART5_DE=255,		
	  RT_HW_UART6_RX=PA12, RT_HW_UART6_TX=PA11, RT_HW_UART6_DE=255};
//--------------------------------Параметры I2C----------------------------------------------------
enum {RT_HW_I2C0_SDA=PB7,  RT_HW_I2C0_SCL=PB6,
      RT_HW_I2C1_SDA=PB3,  RT_HW_I2C1_SCL=PB10}; 
//--------------------------------Параметры SPI----------------------------------------------------	
enum {RT_HW_SPI0_SCK=PA5,  RT_HW_SPI0_MISO=PA6,  RT_HW_SPI0_MOSI=PA7,  RT_HW_SPI0_SS=PA4,  
      RT_HW_SPI1_SCK=PB13, RT_HW_SPI1_MISO=PB14, RT_HW_SPI1_MOSI=PB15, RT_HW_SPI1_SS=PB12};
//=================================================================================================
#define RT_HW_BOARD_CHECK_PIN				//--Разрешение проверки на допустимость использования пинов [функции checkPinXXX(uint8_t pin)];
//--------------------------------Доступные пины контроллера---------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ALL[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15,
										      PC13,PC14,PC15,	RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_DIN[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_ADC[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PB0,PB1,		  					 RT_HW_PGM_END};	
const uint8_t PROGMEM RT_HW_PGM_PIN_TCH[] 	={						 												 RT_HW_PGM_END};									
const uint8_t PROGMEM RT_HW_PGM_PIN_DOT[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15,
										      255,255,255,255,255,255,255,255,255,255, 255, 255, 255,PC13,PC14,PC15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_PWM[] 	={PA0,PA1,PA2,PA3,255,PA5,PA6,PA7,PA8,PA9,PA10,PA11, 255, 255, 255, 255,
										      PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255, 255,PB13,PB14,PB15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_DAC[] 	={																		 RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_INT[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12,PA13, 255, 255,
										      PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15, RT_HW_PGM_END};	
const uint8_t PROGMEM RT_HW_PGM_PIN_N5V[] 	={																		 RT_HW_PGM_END};
//=================================================================================================
//--------------------------------Коммуникационные пины-------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_UHS[] 	={RT_HW_UART0_RX, RT_HW_UART0_TX,  RT_HW_UART0_DE,	//--UART0;
										      RT_HW_UART1_RX, RT_HW_UART1_TX,  RT_HW_UART1_DE,	//--UART1; 
											  RT_HW_UART2_RX, RT_HW_UART2_TX,  RT_HW_UART2_DE,	//--UART2; 
										      RT_HW_UART3_RX, RT_HW_UART3_TX,  RT_HW_UART3_DE,	//--UART3;
										      RT_HW_UART4_RX, RT_HW_UART4_TX,  RT_HW_UART4_DE,	//--UART4; 
											  RT_HW_UART5_RX, RT_HW_UART5_TX,  RT_HW_UART5_DE,	//--UART5;	
											  RT_HW_UART6_RX, RT_HW_UART6_TX,  RT_HW_UART6_DE,	//--UART6;											  
																		       RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_USS[]	={                                 RT_HW_PGM_END};	//--UARTS; 
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_I2C[]	={RT_HW_I2C0_SDA, RT_HW_I2C0_SCL,					//--I2C0;
                                              RT_HW_I2C1_SDA, RT_HW_I2C1_SCL,  RT_HW_PGM_END};  //--i2C1;
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ICS[]	={									RT_HW_PGM_END}; //--I2CS;
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_SPI[]	={RT_HW_SPI0_SCK, RT_HW_SPI0_MISO, RT_HW_SPI0_MOSI, RT_HW_SPI0_SS, //--SPI0: sck,miso,mosi,ss; 	
											  RT_HW_SPI1_SCK, RT_HW_SPI1_MISO, RT_HW_SPI1_MOSI, RT_HW_SPI1_SS, //--SPI1: sck,miso,mosi,ss; 
																			   RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_SPF[]	={			         			   RT_HW_PGM_END};  //--SPF: sck,miso,mosi,cs;	
const uint8_t PROGMEM RT_HW_PGM_PIN_SDC[]	={						           RT_HW_PGM_END};  //--SDIO: DO,D1,D2,D3,SCK;
const uint8_t PROGMEM RT_HW_PGM_PIN_SPN[]	={          					   RT_HW_PGM_END};  //--SSPI: sck,miso,mosi,cs;		
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ETH[]	={				                   RT_HW_PGM_END};	//--numSPI; cs,int,rst;
const uint8_t PROGMEM RT_HW_PGM_PIN_CAN[]	={								   RT_HW_PGM_END};	//--CAN:  rx,tx;
//--------------------------------Системные пины---------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_BRD[]	={PB5,PC13,PA1,PA0,PB4,255,255,	   RT_HW_PGM_END};	//--Control,LED,Button,ADC,PWM,Debug,WS2812;
//--------------------------------SPI пины cs------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_CSX[]	={PB12,PB13,PB14,PB15,	    	   RT_HW_PGM_END};	//--SPI: cs0,cs1,cs2,cs3
//--------------------------------Дисплейные пины--------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_LC4[]	={								   RT_HW_PGM_END};	//--RS,E2(RW),E1,D4-D7;
const uint8_t PROGMEM RT_HW_PGM_PIN_LC6[]	={								   RT_HW_PGM_END};	//--RS,E2(RW),E1,D0-D7;
const uint8_t PROGMEM RT_HW_PGM_PIN_TFT[]	={	    	            		   RT_HW_PGM_END};	//--numSPI; cs,dc,rst,led; t.cs,t.int; sd.cs;
//--------------------------------Тестовые пины----------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_TST_DIS[]	={          	    			   RT_HW_PGM_END};	//--Дискретные сигналы;
const uint8_t PROGMEM RT_HW_PGM_TST_ANL[]	={       					       RT_HW_PGM_END};	//--Аналоговые сигналы: ADC1,PWM1;
const uint8_t PROGMEM RT_HW_PGM_TST_SRV[]	={            	   				   RT_HW_PGM_END};	//--Servo1,Servo2;
const uint8_t PROGMEM RT_HW_PGM_TST_DEV[]	={PB8, PB9, PA8,            	   RT_HW_PGM_END};	//--Устройства: 1-Wire,DHT22,HC-SR04.trig,HC-SR04.echo,
const uint8_t PROGMEM RT_HW_PGM_TST_URT[]	={1,2,            		   		   RT_HW_PGM_END};	//--UART: ModBus, Nextion;
//-------------------------------------------------------------------------------------------------
#endif
*/

/*
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									4.MCU GENERIC F411CXXXX
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_BOARD_SELECT) && defined(RT_HW_BOARD_GENERIC_F411CXXX)
//-------------------------------------------------------------------------------------------------
#define      RT_HW_BOARD_SELECT	    "BOARD GENERIC F411CXXX)"
//-------------------------------------------------------------------------------------------------
//							Permission to work with Link
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_UART0					//--Permission to work with UART0;
#define RT_HW_PERMIT_UART1					//--Permission to work with UART1;
#define RT_HW_PERMIT_UART2					//--Permission to work with UART2;
#define RT_HW_PERMIT_UART6					//--Permission to work with UART6;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_I2C0					//--Permission to work with i2c0;
#define RT_HW_PERMIT_I2C1					//--Permission to work with i2c1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_SPI0					//--Permission to work with SPI0;
//#define RT_HW_PERMIT_SPI1					//--Permission to work with SPI1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_CAN					//--Permission to work with CAN;
//-------------------------------------------------------------------------------------------------
#define RT_HW_PERMIT_DAC					//--Permission to work with DAC;
//-------------------------------------------------------------------------------------------------
//							Permission to cteate with Link
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_UART1					//--Permission to create Serail2 for UART1;
#define RT_HW_CREATE_UART6					//--Permission to create Serial6 for UART6;
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_I2C1					//--Permission to create Wire1 for i2c1;
//-------------------------------------------------------------------------------------------------
#define RT_HW_CREATE_SPI1					//--Permission to work with SPI1;
//-------------------------------------------------------------------------------------------------
//							Relabeling pins
//-------------------------------------------------------------------------------------------------
#if !defined(PB11)
#define PB11 PB_11
#endif
//=================================================================================================
//--------------------------------Параметры UART---------------------------------------------------	
enum {RT_HW_UART0_RX=PA12, RT_HW_UART0_TX=PA11, RT_HW_UART0_DE=255,
	  RT_HW_UART1_RX=PA10, RT_HW_UART1_TX=PA9,  RT_HW_UART1_DE=255, 
	  RT_HW_UART2_RX=PA3,  RT_HW_UART2_TX=PA2,  RT_HW_UART2_DE=255,
	  RT_HW_UART3_RX=255,  RT_HW_UART3_TX=255,  RT_HW_UART3_DE=255,
	  RT_HW_UART4_RX=255,  RT_HW_UART4_TX=255,  RT_HW_UART4_DE=255,
	  RT_HW_UART5_RX=255,  RT_HW_UART5_TX=255,  RT_HW_UART5_DE=255,		
	  RT_HW_UART6_RX=PA12, RT_HW_UART6_TX=PA11, RT_HW_UART6_DE=255};
//--------------------------------Параметры I2C----------------------------------------------------
enum {RT_HW_I2C0_SDA=PB7,  RT_HW_I2C0_SCL=PB6,
      RT_HW_I2C1_SDA=PB3,  RT_HW_I2C1_SCL=PB10}; 
//--------------------------------Параметры SPI----------------------------------------------------	
enum {RT_HW_SPI0_SCK=PA5,  RT_HW_SPI0_MISO=PA6,  RT_HW_SPI0_MOSI=PA7,  RT_HW_SPI0_SS=PA4,  
      RT_HW_SPI1_SCK=PB13, RT_HW_SPI1_MISO=PB14, RT_HW_SPI1_MOSI=PB15, RT_HW_SPI1_SS=PB12};
//=================================================================================================
#define RT_HW_BOARD_CHECK_PIN				//--Разрешение проверки на допустимость использования пинов [функции checkPinXXX(uint8_t pin)];
//--------------------------------Доступные пины контроллера---------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ALL[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15,
										      PC13,PC14,PC15,	RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_DIN[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_ADC[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PB0,PB1,		  					 RT_HW_PGM_END};	
const uint8_t PROGMEM RT_HW_PGM_PIN_TCH[] 	={						 												 RT_HW_PGM_END};									
const uint8_t PROGMEM RT_HW_PGM_PIN_DOT[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12, 255, 255,PA15,
										      PB0,PB1,PB2,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15,
										      255,255,255,255,255,255,255,255,255,255, 255, 255, 255,PC13,PC14,PC15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_PWM[] 	={PA0,PA1,PA2,PA3,255,PA5,PA6,PA7,PA8,PA9,PA10,PA11, 255, 255, 255, 255,
										      PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255, 255,PB13,PB14,PB15, RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_DAC[] 	={																		 RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_INT[] 	={PA0,PA1,PA2,PA3,PA4,PA5,PA6,PA7,PA8,PA9,PA10,PA11,PA12,PA13, 255, 255,
										      PB0,PB1,255,PB3,PB4,PB5,PB6,PB7,PB8,PB9,PB10, 255,PB12,PB13,PB14,PB15, RT_HW_PGM_END};	
const uint8_t PROGMEM RT_HW_PGM_PIN_N5V[] 	={PA0,PB5,																 RT_HW_PGM_END};
//=================================================================================================
//--------------------------------Коммуникационные пины-------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_UHS[] 	={RT_HW_UART0_RX, RT_HW_UART0_TX,  RT_HW_UART0_DE,	//--UART0;
										      RT_HW_UART1_RX, RT_HW_UART1_TX,  RT_HW_UART1_DE,	//--UART1; 
											  RT_HW_UART2_RX, RT_HW_UART2_TX,  RT_HW_UART2_DE,	//--UART2; 
										      RT_HW_UART3_RX, RT_HW_UART3_TX,  RT_HW_UART3_DE,	//--UART3;
										      RT_HW_UART4_RX, RT_HW_UART4_TX,  RT_HW_UART4_DE,	//--UART4; 
											  RT_HW_UART5_RX, RT_HW_UART5_TX,  RT_HW_UART5_DE,	//--UART5;	
											  RT_HW_UART6_RX, RT_HW_UART6_TX,  RT_HW_UART6_DE,	//--UART6;											  
																		       RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_USS[]	={                                 RT_HW_PGM_END};	//--UARTS; 
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_I2C[]	={RT_HW_I2C0_SDA, RT_HW_I2C0_SCL,					//--I2C0;
                                              RT_HW_I2C1_SDA, RT_HW_I2C1_SCL,  RT_HW_PGM_END};  //--i2C1;
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ICS[]	={									RT_HW_PGM_END}; //--I2CS;
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_SPI[]	={RT_HW_SPI0_SCK, RT_HW_SPI0_MISO, RT_HW_SPI0_MOSI, RT_HW_SPI0_SS, //--SPI0: sck,miso,mosi,ss; 	
											  RT_HW_SPI1_SCK, RT_HW_SPI1_MISO, RT_HW_SPI1_MOSI, RT_HW_SPI1_SS, //--SPI1: sck,miso,mosi,ss; 
																			   RT_HW_PGM_END};
const uint8_t PROGMEM RT_HW_PGM_PIN_SPF[]	={			         			   RT_HW_PGM_END};  //--SPF: sck,miso,mosi,cs;	
const uint8_t PROGMEM RT_HW_PGM_PIN_SDC[]	={						           RT_HW_PGM_END};  //--SDIO: DO,D1,D2,D3,SCK;																			   RT_HW_PGM_END};	//--SPI1:  sck,miso,mosi,ss
const uint8_t PROGMEM RT_HW_PGM_PIN_SPN[]	={          					   RT_HW_PGM_END};  //--SSPI: sck,miso,mosi,cs;	
//-------------------------------------------------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_ETH[]	={				                   RT_HW_PGM_END};	//--numSPI; cs,int,rst;
const uint8_t PROGMEM RT_HW_PGM_PIN_CAN[]	={								   RT_HW_PGM_END};	//--CAN:  rx,tx;
//--------------------------------Системные пины---------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_BRD[]	={PB5,PC13,PA1,PA0,PB4,255,255,	   RT_HW_PGM_END};	//--Control,LED,Button,ADC,PWM,Debug,WS2812;
//--------------------------------SPI пины cs------------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_CSX[]	={PB12,PB13,PB14,PB15,	    	   RT_HW_PGM_END};	//--SPI: cs0,cs1,cs2,cs3
//--------------------------------Дисплейные пины--------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_PIN_LC4[]	={								   RT_HW_PGM_END};	//--RS,E2(RW),E1,D4-D7;
const uint8_t PROGMEM RT_HW_PGM_PIN_LC6[]	={								   RT_HW_PGM_END};	//--RS,E2(RW),E1,D0-D7;
const uint8_t PROGMEM RT_HW_PGM_PIN_TFT[]	={	    	            		   RT_HW_PGM_END};	//--numSPI; cs,dc,rst,led; t.cs,t.int; sd.cs;
//--------------------------------Тестовые пины----------------------------------------------------
const uint8_t PROGMEM RT_HW_PGM_TST_DIS[]	={          	    			   RT_HW_PGM_END};	//--Дискретные сигналы;
const uint8_t PROGMEM RT_HW_PGM_TST_ANL[]	={       					       RT_HW_PGM_END};	//--Аналоговые сигналы: ADC1,PWM1;
const uint8_t PROGMEM RT_HW_PGM_TST_SRV[]	={            	   				   RT_HW_PGM_END};	//--Servo1,Servo2;
const uint8_t PROGMEM RT_HW_PGM_TST_DEV[]	={PB8, PB9, PA8,            		RT_HW_PGM_END};	//--Устройства: 1-Wire,DHT22,HC-SR04.trig,HC-SR04.echo,
const uint8_t PROGMEM RT_HW_PGM_TST_URT[]	={1,2,            		   		   RT_HW_PGM_END};	//--UART: ModBus, Nextion;
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================
*/