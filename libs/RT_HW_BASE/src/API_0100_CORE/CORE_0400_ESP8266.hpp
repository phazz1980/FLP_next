//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//												CORE ESP8266
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//													ESP8266
//=================================================================================================
#if !defined(RT_HW_CORE_CODE) && (defined(ESP8266) || defined(ARDUINO_ARCH_ESP8266))
    #define  RT_HW_CORE_CODE     (uint8_t)40
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_CORE_NAME     "ESP8266"	 		//--Наименование программной среды;
	#define  RT_HW_ARCH_NAME     "ESP8266"	 		//--Наименование архитектуры;
 	#define  RT_HW_ARCH_CODE     (uint8_t)40	 	//--Код архитектуры;
    #define  RT_HW_CORE_ESP8266   					//--МАКРОС программной среды;	
	#define	 RT_HW_SUPPORTS_TASK_TIMER				//--Поддержка задач с прерыванием от таймера(RT_HW_Timers.hpp);	
	#define  RT_HW_MAX_TASK		 (uint8_t)8			//--Максимальное кол-во задач по прерыванию от таймера.	
//=================================================================================================
//											1. Основные параметры
//=================================================================================================
    #define RT_HW_DEPTH_SYS  	 (uint8_t)12		//--Системная разрядность;
    #define RT_HW_DEPTH_PWM  	 (uint8_t)10		//--PWM разрядность;
    #define RT_HW_DEPTH_ADC  	 (uint8_t)10		//--ADC разрядность;
    #define RT_HW_ADC_DRIFT  	 (uint8_t)3			//--ADC дрейф показаний;
    #define RT_HW_ADC_NULL  	 (uint8_t)4			//--ADC дрейф нуля;
    #define RT_HW_PWM_FREQ      (uint16_t)1000      //--Частота для PWM умолчанию; 
//=================================================================================================
//											2. Дополнительные макросы
//=================================================================================================
    #define RT_HW_IRAM_ATTR		IRAM_ATTR			//--Атрибут для размещения кода в RAM;	
//=================================================================================================
//											3. Интерфейсы
//=================================================================================================
    #define RT_HW_UART_SPEED    (uint32_t)115200	//--Скорость UART; 
    #define RT_HW_I2C_SPEED     (uint32_t)400000UL	//--Код скорости шины i2c 		  (default 400kHz);
    #define RT_HW_I2C_SPEED_TFT	(uint32_t)400000UL	//--Скорость I2C для работы с TFT (default 800kHz);
    #define RT_HW_SPI_SPEED	    (uint32_t)4000000UL	//--Скорость SPI базовая  		  (default 4mHz); 
    #define RT_HW_SPI_SPEED_TFT	(uint32_t)4000000UL	//--Скорость SPI для работы с TFT (default 20mHz);
    #define RT_HW_SPN_DELAY		0					//--Задержка для RT_HW_PIN_DIR_WRITE_HIGH->??? 0=0ns;1=600ns;2=650ns;3=700ns;4=850ns;5=900ns;
//=================================================================================================
//											4. Дополнительные библиотеки
//=================================================================================================
    #include <pgmspace.h>					//--Работа с PROGMEM;
//-------------------------------------------------------------------------------------------------
    #include <Wire.h>
	#define    RT_HW_WIRE_LIBRARY
//-------------------------------------------------------------------------------------------------
    #include <SPI.h>
	#define    RT_HW_SPI_LIBRARY	
//=================================================================================================
//											5. Таймеры
//=================================================================================================

//=================================================================================================
//
//										10. МАКРОСЫ для функций WIRING 
//
//=================================================================================================
    #define RT_HW_PIN_MODE_INPUT  					INPUT			//--Дискретный вход;
    #define RT_HW_PIN_MODE_INPUT_PULLUP  			INPUT_PULLUP 	//--Дискретный вход с подтяжкой к +Vcc;
    #define RT_HW_PIN_MODE_INPUT_PULLDOWN  			INPUT			//--Дискретный вход с подтяжкой к GND;
    #define RT_HW_PIN_MODE_INPUT_FLOATING  			INPUT			//--Дискретный вход - аналог INPUT;
    #define RT_HW_PIN_MODE_INPUT_ANALOG  			INPUT			//--Аналоговый вход;
    #define RT_HW_PIN_MODE_OUTPUT  					OUTPUT			//--Дискретный выход;
    #define RT_HW_PIN_MODE_OUTPUT_OPEN_DRAIN  		OUTPUT			//--Дискретный выход с открытым выходом;
    #define RT_HW_PIN_MODE_OUTPUT_PWM				OUTPUT			//--ШИМ выход;
    #define RT_HW_PIN_MODE_OUTPUT_PWM_OPEN_DRAIN	OUTPUT			//--ШИМ выход с открытым коллектором;
//=================================================================================================
//
//										11. МАКРОСЫ/ФУНКЦИИ управления GPIO
//
//=================================================================================================
	#define RT_HW_IO_REG_TYPE   					uint32_t
	#define RT_HW_PIN_TO_BASEREG(pin)               ((volatile uint32_t*) GPO)
	#define RT_HW_PIN_TO_BITMASK(pin)               (1UL << (pin))
	#define RT_HW_IO_REG_BASE_ATTR
	#define RT_HW_IO_REG_MASK_ATTR
//--------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directModeInput(RT_HW_IO_REG_TYPE mask)
	{
		if(mask > 0x8000)
		{
			GP16FFS(GPFFS_GPIO(16));
			GPC16 = 0;
			GP16E &= ~1;
		}
		else
		{
        GPE &= ~(mask);
		}
	}
//--------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directModeOutput(RT_HW_IO_REG_TYPE mask)
	{
		if(mask > 0x8000)
		{
			GP16FFS(GPFFS_GPIO(16));
			GPC16 = 0; 
			GP16E |= 1;
		}
		else
		{
			GPE |= (mask);
		}
	}
//--------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	bool RT_HW_directRead(RT_HW_IO_REG_TYPE mask)
	{
		if(mask > 0x8000)
			return GP16I & 0x01;
		else
			return ((GPI & (mask)) ? true : false);
	}
//--------------------------------------------------------------------------------------------------
	#define  RT_HW_DIRECT_MODE_INPUT(base, mask)     RT_HW_directModeInput(mask)
	#define  RT_HW_DIRECT_MODE_OUTPUT(base, mask)    RT_HW_directModeOutput(mask)
	#define  RT_HW_DIRECT_READ(base, mask)           RT_HW_directRead(mask)
	#define  RT_HW_DIRECT_WRITE_HIGH(base, mask)    (mask > 0x8000) ? GP16O |= 1 : (GPOS = (mask))
	#define  RT_HW_DIRECT_WRITE_LOW(base, mask)     (mask > 0x8000) ? GP16O &= ~1 : (GPOC = (mask))
	#define  RT_HW_DIRECT_WRITE(base, mask, vr)     if(vr){RT_HW_DIRECT_WRITE_HIGH(base, mask);}\
	                                                else  {RT_HW_DIRECT_WRITE_LOW(base, mask);}
//=================================================================================================
//										20. DIR:: Дискретный ввод или вывод
//                     		 Для совместимости со старыми функциями
//=================================================================================================
  struct  RT_HW_PIN_DIR_ID {
	uint8_t pin=255; uint8_t dir=0; char mode='N'; uint8_t bitMask; volatile RT_HW_IO_REG_TYPE *baseReg;
	//uint32_t GPFFS_pin; uint32_t LEFT_PIN; uint32_t LEFT_GPCI; uint32_t LEFT_GPCD; uint32_t LEFT_GPFPU;
	uint8_t bit; 	
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
	void 	setPin  (uint8_t _pin){if(pin!=_pin) {dir=0; pin=_pin; }};			//--Изменение пина;
	void 	setInv  (uint8_t _inv){if(inv!=_inv) {dir=0; inv=_inv; }};			//--Изменение инверсии;	
//-------------------------------------------------------------------------------------------------  
  };	//--END STRUCT RT_HW_PIN_DOT_ID;
//=================================================================================================

//----------------------23.Встроенные функции управления GPIO--------------------------------------
	static inline __attribute__((always_inline))		//--Чтение дискретного пина
	bool RT_HW_BASE_digitalReadFast(RT_HW_PIN_DIR_ID &id) {
			 if(id.pin < 16){return GPIP(id.pin);} 
		else if(id.pin== 16){return GP16I & 0x01;} 
		return 0;
	};
//=================================================================================================
//										25. DIB:: Дискретный ввод с защитой от дребезга
//=================================================================================================
  struct 	RT_HW_PIN_DIB_ID {
	uint8_t pin=255; uint8_t dir=0; char mode='N'; uint8_t bitMask; volatile RT_HW_IO_REG_TYPE *baseReg;
	//uint32_t GPFFS_pin; uint32_t LEFT_PIN; uint32_t LEFT_GPCI; uint32_t LEFT_GPCD; uint32_t LEFT_GPFPU;
	uint8_t bit; 	
 	uint8_t	inv=0;							//--Флаг инверсии;
	uint8_t	cash=255;						//--Входные/выходные значения;	
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
	uint8_t  depth=10;
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
	uint8_t  depth=8;
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
	uint8_t  depth=8;	
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
	void 	setFreq (uint16_t _freq) {(void)_freq;};							//--Изменение частоты;
	void 	seDepth	(uint8_t  _depth){(void)_depth;};							//--Изменение разрешения;
//-------------------------------------------------------------------------------------------------  
  };	//--END STRUCT RT_HW_PIN_PWM_ID;
  
//=================================================================================================
//										50. SRV:: Серводвигатели типа SRG90
//=================================================================================================
  struct 	RT_HW_PIN_SRV_ID {
	uint8_t  dir=0, pin=255; 
	uint16_t freq=50;  	uint16_t maxAngle=180, minWidth=800, maxWidth=2300;
    uint16_t cash, vDev;	
//-------------------------------------------------------------------------------------------------	
	RT_HW_PIN_SRV_ID(){;};
	RT_HW_PIN_SRV_ID(uint8_t _pin){pin=_pin;};
//-------------------------------------------------------------------------------------------------	
	void 	setPin  (uint8_t  _pin)  {if(pin!=_pin)   {dir=0; pin=_pin;}}; 		//--Изменение пина;
	void 	setFreq (uint16_t _freq) {(void)_freq;};							//--Изменение частоты;
	void 	setDepth(uint8_t  _depth){(void)_depth;};							//--Изменение разрешения;	
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
//=================================================================================================
//
//										21. МАКРОСЫ/ФУНКЦИИ управления GPIO через структуру
//
//=================================================================================================	
	#define RT_HW_PIN_DIR_SET_ID(id,p)        id.pin=p; id.GPFFS_pin=GPFFS(GPFFS_GPIO(id.pin)); id.LEFT_PIN=(1<<id.pin); id.LEFT_GPCI=(0xF<<GPCI);\
                                                        id.LEFT_GPCD=(1<<GPCD); id.LEFT_GPFPU=(1<<GPFPU);
	#define RT_HW_PIN_DIR_MODE_INPUT(id)      GPF(id.pin) =id.GPFFS_pin; GPEC=id.LEFT_PIN; GPC(id.pin)=(GPC(id.pin) & id.LEFT_GPCI) | (id.LEFT_GPCD);
	#define RT_HW_PIN_DIR_MODE_PULLUP(id)     GPF(id.pin) =id.GPFFS_pin; GPEC=id.LEFT_PIN; GPC(id.pin)=(GPC(id.pin) & id.LEFT_GPCI) | (id.LEFT_GPCD); GPF(id.pin)|=id.LEFT_GPFPU;
	#define RT_HW_PIN_DIR_MODE_OUTPUT(id)     GPF(id.pin)=id.GPFFS_pin; GPC(id.pin)=GPC(id.pin) & id.LEFT_GPCI; GPES = id.LEFT_PIN;
	#define RT_HW_PIN_DIR_READ(id)            RT_HW_BASE_digitalReadFast(id)
	#define RT_HW_PIN_DIR_WRITE_HIGH(id)      if(id.pin<16){GPOS=id.LEFT_PIN;}\
	                                          else         {if(id.pin==16){GP16O &= ~1;}}
    #define RT_HW_PIN_DIR_WRITE_LOW(id)       if(id.pin<16){GPOC=id.LEFT_PIN;}\
	                                          else         {if(id.pin==16){GP16O |=  1;}}
    #define RT_HW_PIN_DIR_WRITE(id,vr)        if(id.pin<16){if(vr){GPOS=id.LEFT_PIN;} else{GPOC=id.LEFT_PIN;}}\
	                                          else         {if(id.pin==16){if(vr){GP16O |= 1;} else{GP16O &= ~1;}}}
//=================================================================================================
#endif   //--END #if !defined(RT_HW_CORE_CODE)
//=================================================================================================