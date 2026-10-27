//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											CORE STM32
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//											  STM32
//=================================================================================================
#if !defined(RT_HW_CORE_CODE) && (defined(ARDUINO_ARCH_STM32))
    #define  RT_HW_CORE_CODE     (uint8_t)60
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_CORE_NAME     "STM32"	 		//--Наименование программной среды;
	#define  RT_HW_ARCH_NAME     "STM32"	 		//--Наименование архитектуры;
 	#define  RT_HW_ARCH_CODE     (uint8_t)60	 	//--Код архитектуры;
	#define  RT_HW_CORE_STM32   					//--МАКРОС программной среды;
	#define	 RT_HW_SUPPORTS_TASK_TIMER				//--Поддержка задач с прерыванием от таймера(RT_HW_Timers.hpp);
	#define  RT_HW_MAX_TASK		 (uint8_t)16			//--Максимальное кол-во задач по прерыванию от таймера.	
//=================================================================================================
//										1. Основные параметры
//=================================================================================================
    #define RT_HW_DEPTH_SYS  	(uint8_t)12			//--Системная разрядность;
    #define RT_HW_DEPTH_PWM  	(uint8_t)16			//--PWM разрядность;
    #define RT_HW_DEPTH_ADC  	(uint8_t)12			//--ADC разрядность;
    #define RT_HW_ADC_DRIFT  	(uint8_t)3			//--ADC дрейф показаний;
    #define RT_HW_ADC_NULL  	(uint8_t)4			//--ADC дрейф нуля;
    #define RT_HW_DEPTH_DAC  	(uint8_t)12			//--DAC разрядность;	
    #define RT_HW_PWM_FREQ      (uint16_t)1000     	//--Частота для PWM умолчанию;
//=================================================================================================
//										2. Дополнительные макросы
//=================================================================================================
    #define RT_HW_IRAM_ATTR							//--Атрибут для совместимости c ESP8266;		
//=================================================================================================
//										3. Интерфейсы
//=================================================================================================
    #define RT_HW_UART_SPEED	(uint32_t)115200	//--Скорость UART; 
    #define RT_HW_I2C_SPEED    	(uint32_t)400000UL	//--Cкоростm шины i2c (default 400kHz);
    #define RT_HW_I2C_SPEED_TFT	(uint32_t)400000UL	//--Скорость I2C для работы с TFT (default 800kHz); 
	#define RT_HW_SPI_SPEED	  	(uint32_t)4000000UL	//--Скорость SPI в Гц (default 4mHz); 
    #define RT_HW_SPI_SPEED_TFT	(uint32_t)4000000UL	//--Скорость SPI для работы с TFT (default 20mHz);
    #define RT_HW_SPN_DELAY		(uint8_t)0			//--Задержка для RT_HW_PIN_DIR_WRITE_HIGH->??? 0=0ns;1=600ns;2=650ns;3=700ns;4=850ns;5=900ns;
//=================================================================================================
//										4. Дополнительные библиотеки
//=================================================================================================
    #include <avr/pgmspace.h>						//--For PROGMEM; 
	#include "usbd_cdc_if.h"						//--For working with USB_CDC STM32duino
//-------------------------------------------------------------------------------------------------
    #include <Wire.h>
	#define    RT_HW_WIRE_LIBRARY
//-------------------------------------------------------------------------------------------------
    #include <SPI.h>
	#define    RT_HW_SPI_LIBRARY	
//=================================================================================================
//										5. Таймеры
//=================================================================================================



//=================================================================================================
//
//										10. МАКРОСЫ для функций WIRING 
//
//=================================================================================================
	#define RT_HW_PIN_MODE_INPUT  				    INPUT				//--Discrete input;
	#define RT_HW_PIN_MODE_OUTPUT  				    OUTPUT				//--Discrete output;
	#define RT_HW_PIN_MODE_INPUT_PULLUP  		    INPUT_PULLUP 		//--Discrete input with pull-up to +Vcc;;
	#define RT_HW_PIN_MODE_INPUT_FLOATING  		    INPUT				//--Discrete input - analogue of INPUT;
	#define RT_HW_PIN_MODE_INPUT_PULLDOWN  		    INPUT_PULLDOWN		//--Discrete input with pull-up to +Vcc;;
	#define RT_HW_PIN_MODE_INPUT_ANALOG  		    INPUT_ANALOG		//--Analog input;;
	#define RT_HW_PIN_MODE_OUTPUT_OPEN_DRAIN  	    OUTPUT_OPEN_DRAIN	//--Discrete output with open output
	#define RT_HW_PIN_MODE_OUTPUT_PWM			    OUTPUT				//--PWM output;
	#define RT_HW_PIN_MODE_OUTPUT_PWM_OPEN_DRAIN    OUTPUT_OPEN_DRAIN	//--PWM output with open output;	
//=================================================================================================
//
//										11. МАКРОСЫ/ФУНКЦИИ управления GPIO
//
//=================================================================================================
//-------------------------------------------------------------------------------------------------
//										11.1. Базовые макросы 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_IO_REG_TYPE                      uint32_t
	#define  RT_HW_PIN_TO_BASEREG(pin)             (0)
	#define  RT_HW_PIN_TO_BITMASK(pin)             ((uint32_t)digitalPinToPinName(pin))
	#define  RT_HW_IO_REG_BASE_ATTR
	#define  RT_HW_IO_REG_MASK_ATTR
//-------------------------------------------------------------------------------------------------
//										11.2. Общие макросы 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_DIRECT_MODE_INPUT(base,  pin)    pin_function((PinName)pin, STM_PIN_DATA(STM_MODE_INPUT,     GPIO_NOPULL, 0))
	#define  RT_HW_DIRECT_MODE_OUTPUT(base, pin)    pin_function((PinName)pin, STM_PIN_DATA(STM_MODE_OUTPUT_PP, GPIO_NOPULL, 0))
	#define  RT_HW_DIRECT_READ(base, pin)           digitalReadFast((PinName)pin)
	#define  RT_HW_DIRECT_WRITE_HIGH(base,  pin)    digitalWriteFast((PinName)pin, HIGH)
	#define  RT_HW_DIRECT_WRITE_LOW(base,   pin)    digitalWriteFast((PinName)pin, LOW)
//-------------------------------------------------------------------------------------------------
//										11.3. Универсальные макросы для внешних функций
//-------------------------------------------------------------------------------------------------
//	#define RT_HW_PIN_DIR_SET_ID(id,p)	  id.pin=p; id.bitMask=((uint32_t)digitalPinToPinName(p)); id.baseReg=0;
//	#define RT_HW_PIN_DIR_SET_ID(id,p)	  id.pin=p; id.bitMask=RT_HW_PIN_TO_BITMASK(p); id.baseReg=RT_HW_PIN_TO_BASEREG(p);
    #define RT_HW_PIN_DIR_SET_ID(id,p)    id.pin=p; id.bitMask=RT_HW_PIN_TO_BITMASK(p); id.baseReg=RT_HW_PIN_TO_BASEREG(p);
	#define RT_HW_PIN_DIR_MODE_INPUT(id)  RT_HW_DIRECT_MODE_INPUT(id.base,  id.pin)
    #define RT_HW_PIN_DIR_MODE_PULLUP(id) pinMode(id.pin,INPUT_PULLUP);	
	#define RT_HW_PIN_DIR_MODE_OUTPUT(id) RT_HW_DIRECT_MODE_OUTPUT(id.base, id.pin)
	#define RT_HW_PIN_DIR_READ(id)        RT_HW_DIRECT_READ(       id.base, id.pin)	
	#define RT_HW_PIN_DIR_WRITE_HIGH(id)  RT_HW_DIRECT_WRITE_HIGH( id.base, id.pin)
	#define RT_HW_PIN_DIR_WRITE_LOW(id)   RT_HW_DIRECT_WRITE_LOW(  id.base, id.pin) 
	#define RT_HW_PIN_DIR_WRITE(id,v)     if(v){RT_HW_DIRECT_WRITE_HIGH(id.base, id.pin);} else{RT_HW_DIRECT_WRITE_LOW(id.base, id.pin);}

//=================================================================================================
//										20. DIR:: Дискретный ввод или вывод
//=================================================================================================
  struct RT_HW_PIN_DIR_ID{
	uint8_t	dir=0, pin=255; char mode='N'; 		uint8_t bitMask; volatile RT_HW_IO_REG_TYPE *baseReg;
 	uint8_t	inv=0;							//--Флаг инверсии;
	uint8_t	cash=255;						//--Входные/выходные значения;
//-------------------------------------------------------------------------------------------------
	RT_HW_PIN_DIR_ID(){;};
	RT_HW_PIN_DIR_ID(uint8_t _pin, uint8_t _inv, char _mode){pin=_pin; inv=_inv; mode=_mode;};		
	RT_HW_PIN_DIR_ID(uint8_t _pin, uint8_t _inv)            {pin=_pin; inv=_inv;};		
	RT_HW_PIN_DIR_ID(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void	setParam(uint8_t _pin, uint8_t _inv, char _mode){pin=_pin; inv=_inv; mode=_mode;};		
	void    setParam(uint8_t _pin, uint8_t _inv)            {pin=_pin; inv=_inv;};		
	void    setParam(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin){if(pin!=_pin) {dir=0; pin=_pin; }};	//--Изменение пина;
	void 	setInv  (uint8_t _inv){if(inv!=_inv) {dir=0; inv=_inv; }}; 	//--Изменение инверсии;	
//-------------------------------------------------------------------------------------------------  
  };	//--END STRUCT RT_HW_PIN_DOT_ID;
//=================================================================================================

//=================================================================================================
//										25. DIB:: Дискретный ввод с защитой от дребезга
//=================================================================================================
  struct 	RT_HW_PIN_DIB_ID {
	uint8_t  dir=0, pin=255; char mode='N'; 		uint8_t bitMask; volatile RT_HW_IO_REG_TYPE *baseReg;
 	uint8_t	 inv=0;																//--Флаг инверсии; 
	uint8_t  cash=255;															//--Последнее значение;	
	uint32_t timeBegin; uint16_t period=50; 									//--Параметры дребезга;
//-------------------------------------------------------------------------------------------------
	RT_HW_PIN_DIB_ID(){;};
	RT_HW_PIN_DIB_ID(uint8_t _pin, uint16_t _period, uint8_t _inv, char _mode)	{pin=_pin; period=_period; inv=_inv; mode=_mode;};
	RT_HW_PIN_DIB_ID(uint8_t _pin, uint16_t _period, uint8_t _inv)            	{pin=_pin; period=_period; inv=_inv;};
	RT_HW_PIN_DIB_ID(uint8_t _pin, uint16_t _period)							{pin=_pin; period=_period;};	
	RT_HW_PIN_DIB_ID(uint8_t _pin)												{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t _pin, uint16_t _period, uint8_t _inv, char _mode)	{pin=_pin; period=_period; inv=_inv; mode=_mode;};
	void    setParam(uint8_t _pin, uint16_t _period, uint8_t _inv)            	{pin=_pin; period=_period; inv=_inv;};
	void    setParam(uint8_t _pin, uint16_t _period)							{pin=_pin; period=_period;};	
	void    setParam(uint8_t _pin)												{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin){if(pin!=_pin) {dir=0; pin=_pin;}};			//--Изменение пина;
	void 	setInv  (uint8_t _inv){if(inv!=_inv) {dir=0; inv=_inv;}}; 			//--Изменение инверсии;		
	void setPeriod(uint16_t _period){period=_period;};							//--Изменение периода;		
//-------------------------------------------------------------------------------------------------
  };	

//=================================================================================================
//										30. ADC:: Аналоговый ввод
//=================================================================================================
  struct 	RT_HW_PIN_ADC_ID {
	uint8_t  dir=0,  pin=255;  
	uint16_t vPin=0xFFFF, vDev=0,  vNorm=0; uint8_t vDigital=0;
	uint16_t cash=0;
	uint8_t  depth=12;
//-------------------------------------------------------------------------------------------------
	RT_HW_PIN_ADC_ID(){;};
	RT_HW_PIN_ADC_ID(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t _pin, uint8_t _depth)			{pin=_pin; depth=_depth;};
	void    setParam(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin)  {if(pin!=_pin) 	{dir=0; pin=_pin; }};		//--Изменение пина;
	void 	seDepth	(uint8_t _depth){if(depth!=_depth){dir=0; depth=_depth;}};	//--Изменение разрешения;	
//-------------------------------------------------------------------------------------------------
  };	//--END STRUCT RT_HW_PIN_ADC_ID; 	

//=================================================================================================
//										35. DAC:: Аналоговый вывод
//================================================================================================= 	
  struct 	RT_HW_PIN_DAC_ID {
	uint8_t  dir=0,  pin=255;  
	uint16_t cash=0;	
	uint8_t  depth=12;
//-------------------------------------------------------------------------------------------------	
	RT_HW_PIN_DAC_ID(){;};
	RT_HW_PIN_DAC_ID(uint8_t _pin)	{pin=_pin;};	
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t _pin, uint8_t _depth)			{pin=_pin; depth=_depth;};
	void    setParam(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin)	{if(pin!=_pin)    {dir=0; pin=_pin;}};		//--Изменение пина;
	void 	seDepth	(uint8_t _depth){if(depth!=_depth){dir=0; depth=_depth;}};	//--Изменение разрешения;
//-------------------------------------------------------------------------------------------------		
  };	//--END STRUCT RT_HW_PIN_DAC_ID;

//=================================================================================================
//										40. PWM:: Широтно-импульсный вывод
// mode:  не имеет значения (='N') 
//=================================================================================================
  struct	RT_HW_PIN_PWM_ID {	
	uint8_t  dir=0, pin=255; char mode='N';
 	uint8_t  inv=0;																//--Флаг инверсии;
	uint16_t vPin=0;    
	uint16_t cash=0;
	uint16_t var=0;
	uint16_t freq=500;   
	uint8_t  depth=12;	
//-------------------------------------------------------------------------------------------------		
	RT_HW_PIN_PWM_ID(){;};
	RT_HW_PIN_PWM_ID(uint8_t _pin, uint16_t _freq)	{pin=_pin;	freq=_freq;};
	RT_HW_PIN_PWM_ID(uint8_t _pin)					{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t  _pin, uint16_t _freq, uint8_t _depth, uint8_t _inv){pin=_pin;	freq=_freq; depth=_depth; inv=_inv;};	
	void    setParam(uint8_t  _pin, uint16_t _freq, uint8_t _depth)	{pin=_pin;	freq=_freq; depth=_depth;};	
	void    setParam(uint8_t  _pin, uint16_t _freq)					{pin=_pin;	freq=_freq;				 };	
	void    setParam(uint8_t  _pin, uint8_t _depth)					{pin=_pin;	 			depth=_depth;};		
	void    setParam(uint8_t  _pin)            	  					{pin=_pin;							 };		
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin)   {if(pin!=_pin)    {dir=0; pin=_pin;}};		//--Изменение пина;
	void 	setInv  (uint8_t _inv)   {if(inv!=_inv)    {dir=0; inv=_inv;}}; 	//--Изменение инверсии;	
	void 	setFreq (uint16_t _freq) {if(freq!=_freq)  {dir=0; freq=_freq;}};	//--Изменение частоты;
	void 	seDepth	(uint8_t  _depth){if(depth!=_depth){dir=0; depth=_depth;}};	//--Изменение разрешения;
//-------------------------------------------------------------------------------------------------  
  };	//--END STRUCT RT_HW_PIN_PWM_ID;

//=================================================================================================
//										45. FRQ:: Частотный вывод
// mode:  не имеет значения (='N') 
//=================================================================================================
  struct	RT_HW_PIN_FRQ_ID {	
	uint8_t  dir=0, pin=255; char mode='N';
 	uint8_t  inv=0;																//--Флаг инверсии;	
	uint16_t vPin=0;	     
	uint16_t cash;
	uint16_t freq=500; 	
	uint8_t  depth=12;
//-------------------------------------------------------------------------------------------------		
	RT_HW_PIN_FRQ_ID(){;};
	RT_HW_PIN_FRQ_ID(uint8_t _pin, uint16_t _freq)	{pin=_pin;	freq=_freq;};
	RT_HW_PIN_FRQ_ID(uint8_t _pin)					{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t  _pin, uint16_t _freq, uint8_t _depth, uint8_t _inv){pin=_pin;	freq=_freq; depth=_depth; inv=_inv;};	
	void    setParam(uint8_t  _pin, uint16_t _freq, uint8_t _depth)	{pin=_pin;	freq=_freq; depth=_depth;};	
	void    setParam(uint8_t  _pin, uint16_t _freq)					{pin=_pin;	freq=_freq;				 };	
	void    setParam(uint8_t  _pin, uint8_t _depth)					{pin=_pin;	 			depth=_depth;};		
	void    setParam(uint8_t  _pin)            	  					{pin=_pin;							 };		
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin)   {if(pin!=_pin)    {dir=0; pin=_pin;}};		//--Изменение пина;
	void 	setInv  (uint8_t _inv)   {if(inv!=_inv)    {dir=0; inv=_inv;}}; 	//--Изменение инверсии;	
	void 	setFreq (uint16_t _freq) {if(freq!=_freq)  {dir=0; freq=_freq;}};	//--Изменение частоты;
	void 	seDepth	(uint8_t  _depth){if(depth!=_depth){dir=0; depth=_depth;}};	//--Изменение разрешения;
//-------------------------------------------------------------------------------------------------  
  };	//--END STRUCT RT_HW_PIN_PWM_ID;
  
//=================================================================================================
//										50. SRV:: Серводвигатели типа SRG90
//=================================================================================================
  struct 	RT_HW_PIN_SRV_ID {
	uint8_t  dir=0, pin=255; 
	uint16_t maxAngle=180, minWidth=800, maxWidth=2300;
    uint16_t cash, vDev;
	uint16_t freq=50; 	
	uint8_t  depth=12;	
//-------------------------------------------------------------------------------------------------	
	RT_HW_PIN_SRV_ID(){;};
	RT_HW_PIN_SRV_ID(uint8_t _pin){pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t  _pin, uint16_t _freq, uint8_t _depth)	{pin=_pin;	freq=_freq; depth=_depth;};	
	void    setParam(uint8_t  _pin, uint16_t _freq)					{pin=_pin;	freq=_freq;				 };	
	void    setParam(uint8_t  _pin, uint8_t _depth)					{pin=_pin;	 			depth=_depth;};		
	void    setParam(uint8_t  _pin)            	  					{pin=_pin;							 };	
//-------------------------------------------------------------------------------------------------
	void 	setPin  (uint8_t _pin)   {if(pin!=_pin)    {dir=0; pin=_pin;}};		//--Изменение пина;
	void 	setFreq (uint16_t _freq) {if(freq!=_freq)  {dir=0; freq=_freq;}};	//--Изменение частоты;
	void 	seDepth	(uint8_t  _depth){if(depth!=_depth){dir=0; depth=_depth;}};	//--Изменение разрешения;	
//-------------------------------------------------------------------------------------------------
  };  
   
//=================================================================================================
//										60. TCH:: Сенсорный ввод
//=================================================================================================
  struct 	RT_HW_PIN_TCH_ID {
	uint8_t dir=0, pin=255; uint16_t cash=0, vPin=0, vDev=0, vNorm=0;
	RT_HW_PIN_TCH_ID(uint8_t _pin)							{pin=_pin;};
//-------------------------------------------------------------------------------------------------
	void    setParam(uint8_t _pin)							{pin=_pin;};
  };	//--END STRUCT RT_HW_PIN_TCH_ID;

//=================================================================================================
//										65. HAL:: Ввод с датчика Холла (ESP32)
//=================================================================================================
  struct 	RT_HW_PIN_HAL_ID { 
	uint16_t dev; uint16_t var; 
	RT_HW_PIN_HAL_ID();
//-------------------------------------------------------------------------------------------------	
  };	//--END STRUCT RT_HW_PIN_HAL_ID;
//=================================================================================================

/*
//=================================================================================================
//
//										11. МАКРОСЫ/ФУНКЦИИ управления GPIO
//
//=================================================================================================
//-------------------------------------------------------------------------------------------------
//										11.1. Базовые макросы 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_IO_REG_TYPE                      uint32_t
	#define  RT_HW_PIN_TO_BASEREG(pin)             (0)
	#define  RT_HW_PIN_TO_BITMASK(pin)             ((uint32_t)digitalPinToPinName(pin))
	#define  RT_HW_IO_REG_BASE_ATTR
	#define  RT_HW_IO_REG_MASK_ATTR
//-------------------------------------------------------------------------------------------------
//										11.2. Общие макросы 
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_DIRECT_MODE_INPUT(base,  pin)    pin_function((PinName)pin, STM_PIN_DATA(STM_MODE_INPUT,     GPIO_NOPULL, 0))
	#define  RT_HW_DIRECT_MODE_OUTPUT(base, pin)    pin_function((PinName)pin, STM_PIN_DATA(STM_MODE_OUTPUT_PP, GPIO_NOPULL, 0))
	#define  RT_HW_DIRECT_READ(base, pin)           digitalReadFast((PinName)pin)
	#define  RT_HW_DIRECT_WRITE_HIGH(base,  pin)    digitalWriteFast((PinName)pin, HIGH)
	#define  RT_HW_DIRECT_WRITE_LOW(base,   pin)    digitalWriteFast((PinName)pin, LOW)
//-------------------------------------------------------------------------------------------------
//										11.3. Универсальные макросы для внешних функций
//-------------------------------------------------------------------------------------------------
//	#define RT_HW_PIN_DIR_SET_ID(id,p)	  id.pin=p; id.bitMask=((uint32_t)digitalPinToPinName(p)); id.baseReg=0;
//	#define RT_HW_PIN_DIR_SET_ID(id,p)	  id.pin=p; id.bitMask=RT_HW_PIN_TO_BITMASK(p); id.baseReg=RT_HW_PIN_TO_BASEREG(p);
    #define RT_HW_PIN_DIR_SET_ID(id,p)    id.pin=p; id.bitMask=RT_HW_PIN_TO_BITMASK(p); id.baseReg=RT_HW_PIN_TO_BASEREG(p);
	#define RT_HW_PIN_DIR_MODE_INPUT(id)  RT_HW_DIRECT_MODE_INPUT(id.base,  id.pin)
    #define RT_HW_PIN_DIR_MODE_PULLUP(id) pinMode(id.pin,INPUT_PULLUP);	
	#define RT_HW_PIN_DIR_MODE_OUTPUT(id) RT_HW_DIRECT_MODE_OUTPUT(id.base, id.pin)
	#define RT_HW_PIN_DIR_READ(id)        RT_HW_DIRECT_READ(       id.base, id.pin)	
	#define RT_HW_PIN_DIR_WRITE_HIGH(id)  RT_HW_DIRECT_WRITE_HIGH( id.base, id.pin)
	#define RT_HW_PIN_DIR_WRITE_LOW(id)   RT_HW_DIRECT_WRITE_LOW(  id.base, id.pin) 
	#define RT_HW_PIN_DIR_WRITE(id,v)     if(v){RT_HW_DIRECT_WRITE_HIGH(id.base, id.pin);} else{RT_HW_DIRECT_WRITE_LOW(id.base, id.pin);}

*/





//=================================================================================================
//
//										150. STM32 Config SERIAL
//
//=================================================================================================
    #if !defined(SERIAL_5N1)					
		#define SERIAL_5N1 0x00
		#define SERIAL_6N1 0x02
		#define SERIAL_7N1 0x04
		#define SERIAL_8N1 0x06
		#define SERIAL_5N2 0x08
		#define SERIAL_6N2 0x0A
		#define SERIAL_7N2 0x0C
		#define SERIAL_8N2 0x0E
		#define SERIAL_5E1 0x20
		#define SERIAL_6E1 0x22
		#define SERIAL_7E1 0x24
		#define SERIAL_8E1 0x26
		#define SERIAL_5E2 0x28
		#define SERIAL_6E2 0x2A
		#define SERIAL_7E2 0x2C
		#define SERIAL_8E2 0x2E
		#define SERIAL_5O1 0x30
		#define SERIAL_6O1 0x32
		#define SERIAL_7O1 0x34
		#define SERIAL_8O1 0x36
		#define SERIAL_5O2 0x38
		#define SERIAL_6O2 0x3A
		#define SERIAL_7O2 0x3C
		#define SERIAL_8O2 0x3E
	#endif
//=================================================================================================
#endif
//=================================================================================================

