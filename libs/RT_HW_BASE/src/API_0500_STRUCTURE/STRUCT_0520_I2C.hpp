////#################################################################################################
//
//                        				СТРУКТУРЫ для i2c
//ecoins@mail.ru 21.05.2023г.
//=================================================================================================
// AVR     - one  i2c(Wire)  bus, combined master/slave library;
// SAM3X   - one  i2c(Wire)  bus, combined master/slave library;
// ESP8266 - one  i2c(Wire)  bus, combined master/slave library;
// ESP32S  - two  i2c Master bus(Wire1, Wire2) buses,   Wire.h library.
//           two  i2c Slave  bus(WireSlave1,WireSlave2),ESP32_I2C_Slave library.
// Raspberry Pi Pico - two i2c default;
//			 Wire (SDA,SCL) - 0,1; 4,5;  8, 9; 12,13; 16,17; 20,21;
//			 Wire1(SDA,SCL) - 2,3; 6,7; 10,11; 14,15; 18,19; 26,27;
// STM32   - three i2c(Wire) bus, combined master/slave library;
//           Wire is defined by default 
//           Wire1 needs to define TwoWire Wire1(RT_HW_I2C1_SDA, RT_HW_I2C1_SCL),
//           Wire2 needs to define TwoWire Wire2(RT_HW_I2C2_SDA, RT_HW_I2C2_SCL);
// Seeeduino-six bus i2c(Wire,Wire1-5), combined master/slave library;
//=================================================================================================
// code Err0r: 0-ok,1-too much data; 2-Nack on address transfer; 3-Nack on data transfer; 4-another error i2c; 
//             5-time out waitting byte; 
//             50-invalid number bus; 51-invalid address device(>=254), 52-bus i2c no Master;
//			   60,61,62-error number sda,scl;
//			   65-No CORE;
//=================================================================================================
//									1. Данные для i2c 
//=================================================================================================	
  struct RT_HW_STRUCT_I2C {
	uint8_t  custom=0;				//--1.Состояние настройки;
	uint8_t  status=0;				//--2.Статус: no init=0; Master=1; Slave=2;
	uint8_t  codeErr=0;				//--3.Код ошибки; 	
	uint8_t  busy=0;  				//--4.Флаг занятости;
//-------------------------------------------------------------------------------------------------
	uint8_t  sda=255; 				//--5.Номер пина SDA;
	uint8_t  scl=255;				//--6.Номер пина SCL;  
	uint8_t  addr=0; 				//--7.Адрес Slave шины (>0) или =0 для Master;	
//-------------------------------------------------------------------------------------------------
	uint8_t  bf8;   				//--10.Буфер для чтения байта;
	uint16_t bf16;					//--11.Буфер для чтения uint16_t
//-------------------------------------------------------------------------------------------------
	uint32_t speed=RT_HW_I2C_SPEED; //--Скорость I2C;;
  };
//=================================================================================================
//									5. Данные для работы как с устройством через i2c
//=================================================================================================	
  struct RT_HW_STRUCT_I2C_DEV{
	uint8_t  custom=0;				//--1.Состояние настройки;
	uint8_t  status=0;				//--2.Статус: no init=0; Master=1; Slave=2;
	uint8_t  link=0;				//--3.Флаг завершения транзакции;
	uint8_t  codeErr=0,err=0;		//--4.Код ошибки;
	uint8_t  bus=0;					//--5.Номер шины;
	uint8_t	 addr=255; 				//--6.Адрес Slave шины (>0) или =0 для Master;;
	uint8_t  expander=0,  agoExpander=255;  //--7.1.Адрес расширителя i2c expander address;
	uint8_t  channel=255, agoChannel=255;	//--7.2.Номер канала в расширители i2c (1-8);	
	uint8_t  cnt,cntErr;
	uint8_t  step=0;
//-------------------------------------------------------------------------------------------------
	uint8_t  stop=1;				//--8.Stop bit;
//-------------------------------------------------------------------------------------------------
	uint8_t  bf8;					//--Working register	
//-------------------------------------------------------------------------------------------------
	uint8_t  ok=0;					//--=0-операция продолжается; =1-операция успешно завершена; >1 - зваершение с кодом ошибки=ok;
	uint16_t period;				//--Период (для i2cFindAdr);
	uint32_t timeBegin;				//--Рабочий регистр для отсчета времени;
	uint32_t speed=RT_HW_I2C_SPEED;	//--Speed;	
  };
 //=================================================================================================	 