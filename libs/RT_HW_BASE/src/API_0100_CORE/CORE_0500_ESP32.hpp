//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											ARCH ESP32
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//											   ESP32S
//=================================================================================================
#if !defined(RT_HW_ARCH_IDF_ESP32_CODE) && (CONFIG_IDF_TARGET_ESP32)
    #define  RT_HW_ARCH_IDF_ESP32_CODE     (uint8_t)50
	#define  RT_HW_ARCH_IDF_ESP32S   
	#define  RT_HW_ARCH_IDF_NAME     "IDF_ESP32S"	 //--Наименование архитектуры ESP32 IDF;
#endif
//=================================================================================================
//											   ESP32C3
//=================================================================================================
#if !defined(RT_HW_ARCH_IDF_ESP32_CODE) && (CONFIG_IDF_TARGET_ESP32C3)
    #define  RT_HW_ARCH_IDF_ESP32_CODE     (uint8_t)51
	#define  RT_HW_ARCH_IDF_ESP32C3
	#define  RT_HW_ARCH_IDF_NAME     "IDF_ESP32C3"	 //--Наименование архитектуры ESP32 IDF;
#endif
//=================================================================================================
//											   ESP32C6
//=================================================================================================
#if !defined(RT_HW_ARCH_IDF_ESP32_CODE) && (CONFIG_IDF_TARGET_ESP32C6)
    #define  RT_HW_ARCH_IDF_ESP32_CODE     (uint8_t)52
	#define  RT_HW_ARCH_IDF_ESP32C6
	#define  RT_HW_ARCH_IDF_NAME     "IDF_ESP326"	 //--Наименование архитектуры ESP32 IDF;
#endif
//=================================================================================================
//											   ESP32S2
//=================================================================================================
#if !defined(RT_HW_ARCH_IDF_ESP32_CODE) && (CONFIG_IDF_TARGET_ESP32S2)
    #define  RT_HW_ARCH_IDF_ESP32_CODE     (uint8_t)55
	#define  RT_HW_ARCH_IDF_ESP32S2
	#define  RT_HW_ARCH_IDF_NAME     "IDF_ESP32S2"	 //--Наименование архитектуры ESP32 IDF;
#endif
//=================================================================================================
//											   ESP32S3
//=================================================================================================
#if !defined(RT_HW_ARCH_IDF_ESP32_CODE) && (CONFIG_IDF_TARGET_ESP32S3)
    #define  RT_HW_ARCH_IDF_ESP32_CODE     (uint8_t)56
	#define  RT_HW_ARCH_IDF_ESP32S3
	#define  RT_HW_ARCH_IDF_NAME     "IDF_ESP32S3"	 //--Наименование архитектуры ESP32 IDF;
#endif
//=================================================================================================




//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											 CORE ESP32
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//											   ESP32
//=================================================================================================
#if !defined(RT_HW_CORE_CODE) && (defined(ESP32) || defined(ARDUINO_ARCH_ESP32))
    #define  RT_HW_CORE_CODE     (uint8_t)50
//------------------------------------------------------------------------------------------------- 
	#define  RT_HW_CORE_NAME     "ESP32"	 		//--Наименование программной среды;
	#define  RT_HW_ARCH_NAME     "ESP32"	 		//--Наименование архитектуры;
	#define  RT_HW_ARCH_CODE     (uint8_t)50 		//--Код архитектуры;
	#define  RT_HW_CORE_ESP32   					//--МАКРОС программной среды;
	#define	 RT_HW_SUPPORTS_TASK_TIMER				//--Поддержка задач с прерыванием от таймера(RT_HW_Timers.hpp);
	#define  RT_HW_MAX_TASK		 (uint8_t)16		//--Максимальное кол-во задач по прерыванию от таймера.
//---------------------------------- Для ESP32С3 --------------------------------------------------
	#if defined(RT_HW_CORE_ESP32C3)
		#define RT_HW_USE_TIMER_NO        
		#define RT_HW_MAX_NUM_CHANNEL      6		//--Максимальное кол-во каналов LEDC
	#endif
//---------------------------------- Для ESP32С6 --------------------------------------------------
	#if defined(RT_HW_CORE_ESP32C6)
		#define RT_HW_USE_TIMER_NO        
		#define RT_HW_MAX_NUM_CHANNEL      6		//--Максимальное кол-во каналов LEDC
	#endif	
//---------------------------------  Для ESP32S2--------------------------------------------------
	#if defined(RT_HW_CORE_ESP32S2)
		#define RT_HW_USE_TIMER_NO        
		#define RT_HW_MAX_NUM_CHANNEL      6		//--Максимальное кол-во каналов LEDC
	#endif	
//------------------------- Для ESP32S,ESP32S3---------------------------------------------------
	#if defined(RT_HW_CORE_ESP32S) || defined(RT_HW_CORE_ESP32S3) || defined(RT_HW_CORE_ESP32PICO) 
		#define RT_HW_USE_TIMER_NO        
		#define RT_HW_MAX_NUM_CHANNEL      16		//--Максимальное кол-во каналов LEDC
	#endif
//------------------------ Для неопознанных типов ESP32------------------------------------------
	#if !defined(RT_HW_USE_TIMER_NO)
		#define RT_HW_USE_TIMER_NO        
		#define RT_HW_MAX_NUM_CHANNEL      6		//--Максимальное кол-во каналов LEDC
	#endif
//-------------------------------------------------------------------------------------------------
//  Для ESP32C3 аппаратные PWM - 2,3,4,5,6,7	
//=================================================================================================
//										1. Основные параметры
//=================================================================================================
    #define RT_HW_DEPTH_SYS  	(uint8_t)12			//--Системная разрядность;
    #define RT_HW_DEPTH_PWM  	(uint8_t)12			//--PWM разрядность;
    #define RT_HW_DEPTH_DAC  	(uint8_t)12			//--DAC разрядность;
    #define RT_HW_DEPTH_ADC  	(uint8_t)12			//--ADC разрядность;
    #define RT_HW_ADC_DRIFT  	(uint8_t)20			//--ADC дрейф показаний;
    #define RT_HW_ADC_NULL  	(uint8_t)16			//--ADC дрейф нуля;	
    #define RT_HW_PWM_FREQ      (uint16_t)1000  	//--Частота для PWM умолчанию;
//-------------------------------------------------------------------------------------------------

//=================================================================================================
//										2. Дополнительные макросы
//=================================================================================================
    #define RT_HW_IRAM_ATTR		IRAM_ATTR			//--Атрибут для размещения кода в RAM;			
//=================================================================================================
//										3. Интерфейсы
//=================================================================================================
    #define RT_HW_UART_SPEED    (uint32_t)115200	//--Скорость UART; 
    #define RT_HW_I2C_SPEED     (uint32_t)400000UL	//--Код скорости шины i2c 		  (default 400kHz);
    #define RT_HW_I2C_SPEED_TFT	(uint32_t)400000UL	//--Скорость I2C для работы с TFT (default 800kHz);
    #define RT_HW_SPI_SPEED	    (uint32_t)4000000UL	//--Скорость SPI базовая  		  (default 4mHz); 
    #define RT_HW_SPI_SPEED_TFT	(uint32_t)4000000UL	//--Скорость SPI для работы с TFT (default 20mHz);
    #define RT_HW_SPN_DELAY		(uint8_t)0			//--Задержка для RT_HW_PIN_DIR_WRITE_HIGH->??? 0=0ns;1=600ns;2=650ns;3=700ns;4=850ns;5=900ns;
//=================================================================================================
//										4. Дополнительные библиотеки
//=================================================================================================
	#include <pgmspace.h>									//--For PROGMEM;;
	#include "esp32-hal-ledc.h"								//--SDK;	
	#include <driver/rtc_io.h>								//--SDK;	
//-------------------------------------------------------------------------------------------------
    #if !defined(RT_HW_BLOCK_I2C)
    #include <Wire.h>
	#define    RT_HW_WIRE_LIBRARY
	#endif  
//-------------------------------------------------------------------------------------------------
    #include <SPI.h>
	#define    RT_HW_SPI_LIBRARY
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_CORE_ESP32S)
	 // #include "BluetoothSerial.h"							//--Connecting the library BlueTooth;
	#endif   
//=================================================================================================
//										5. Таймеры
//=================================================================================================

//=================================================================================================
 
//=================================================================================================
//
//										10. МАКРОСЫ для функций WIRING 
//
//=================================================================================================
	#define RT_HW_PIN_MODE_INPUT  				    INPUT				//--Дискретный вход;
	#define RT_HW_PIN_MODE_OUTPUT  				    OUTPUT				//--Дискретный выход;
	#define RT_HW_PIN_MODE_INPUT_PULLUP  		    INPUT_PULLUP 		//--Дискретный вход с подтяжкой к +Vcc;
	#define RT_HW_PIN_MODE_INPUT_FLOATING  		    INPUT				//--Дискретный вход - аналог INPUT;
	#define RT_HW_PIN_MODE_INPUT_PULLDOWN  		    INPUT_PULLDOWN		//--Дискретный вход с подтяжкой к GND;
	#define RT_HW_PIN_MODE_INPUT_ANALOG  		    INPUT				//--Аналоговый вход;
	#define RT_HW_PIN_MODE_OUTPUT_OPEN_DRAIN  	    OUTPUT_OPEN_DRAIN	//--Дискретный выход с открытым выходом;
	#define RT_HW_PIN_MODE_OUTPUT_PWM			    OUTPUT				//--ШИМ выход;
	#define RT_HW_PIN_MODE_OUTPUT_PWM_OPEN_DRAIN    OUTPUT				//--ШИМ выход с открытым коллектором;
//=================================================================================================
//										12. МАКРОСЫ/ФУНКЦИИ управления GPIO
//=================================================================================================
//-------------------------------------------------------------------------------------------------
	#include <driver/rtc_io.h>
	#include <soc/gpio_struct.h>
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_IO_REG_TYPE 		                uint32_t
	#define  RT_HW_PIN_TO_BASEREG(pin)              (0)
	#define  RT_HW_PIN_TO_BITMASK(pin)              (pin)
	#define  RT_HW_IO_REG_BASE_ATTR
	#define  RT_HW_IO_REG_MASK_ATTR
//-------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	RT_HW_IO_REG_TYPE 		RT_HW_directRead(RT_HW_IO_REG_TYPE pin){
	#if CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
		return (GPIO.in.val >> pin) & 0x1;
	#else // plain ESP32
			 if ( pin < 32 ) {return (GPIO.in >> pin) & 0x1;}
		else if ( pin < 46 ) {return (GPIO.in1.val >> (pin - 32)) & 0x1;}
	#endif
	return 0;
	};
//-------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directWriteLow(RT_HW_IO_REG_TYPE pin){
	#if CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
		GPIO.out_w1tc.val = ((uint32_t)1 << pin);
	#else // plain ESP32
			 if ( pin < 32 ) {GPIO.out_w1tc = ((uint32_t)1 << pin);}
		else if ( pin < 46 ) {GPIO.out1_w1tc.val = ((uint32_t)1 << (pin - 32));}
	#endif
	};
//-------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directWriteHigh(RT_HW_IO_REG_TYPE pin){
	#if CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
		GPIO.out_w1ts.val = ((uint32_t)1 << pin);
	#else // plain ESP32
			 if ( pin < 32 ){GPIO.out_w1ts = ((uint32_t)1 << pin);}
		else if ( pin < 46 ){GPIO.out1_w1ts.val = ((uint32_t)1 << (pin - 32));}
    #endif
	};
//-------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directModeInput(RT_HW_IO_REG_TYPE pin){
	#if CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
		GPIO.enable_w1tc.val = ((uint32_t)1 << (pin));
	#else
		if ( digitalPinIsValid(pin) ){
			#if ESP_IDF_VERSION_MAJOR < 4      // IDF 3.x ESP32/PICO-D4
				uint32_t rtc_reg(rtc_gpio_desc[pin].reg);

			if ( rtc_reg ) // RTC pins PULL settings
			{
            ESP_REG(rtc_reg) = ESP_REG(rtc_reg) & ~(rtc_gpio_desc[pin].mux);
            ESP_REG(rtc_reg) = ESP_REG(rtc_reg) & ~(rtc_gpio_desc[pin].pullup | rtc_gpio_desc[pin].pulldown);
			}
			#endif
	// Input
        if ( pin < 32 ) {GPIO.enable_w1tc = ((uint32_t)1 << pin);}
        else			{GPIO.enable1_w1tc.val = ((uint32_t)1 << (pin - 32));}
    }
	#endif
	};
//-------------------------------------------------------------------------------------------------
	static inline __attribute__((always_inline))
	void RT_HW_directModeOutput(RT_HW_IO_REG_TYPE pin){
	#if CONFIG_IDF_TARGET_ESP32C3 || CONFIG_IDF_TARGET_ESP32C6
		GPIO.enable_w1ts.val = ((uint32_t)1 << (pin));
	#else
		if ( digitalPinIsValid(pin)) 			//--&& pin <= 33  Use Any In/Out pins 
		{
		#if ESP_IDF_VERSION_MAJOR < 4      		//--IDF 3.x ESP32/PICO-D4
		  uint32_t rtc_reg(rtc_gpio_desc[pin].reg);
          if ( rtc_reg ) // RTC pins PULL settings
          {
            ESP_REG(rtc_reg) = ESP_REG(rtc_reg) & ~(rtc_gpio_desc[pin].mux);
            ESP_REG(rtc_reg) = ESP_REG(rtc_reg) & ~(rtc_gpio_desc[pin].pullup | rtc_gpio_desc[pin].pulldown);
          }
		#endif
        // Output
          if ( pin < 32 ) {GPIO.enable_w1ts      = ((uint32_t)1 << pin);}
          else 			  {GPIO.enable1_w1ts.val = ((uint32_t)1 << (pin - 32));} // already validated to pins <= 33
		}
	#endif
	}
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_DIRECT_MODE_INPUT(base,  pin)    RT_HW_directModeInput(pin)
	#define  RT_HW_DIRECT_MODE_OUTPUT(base, pin)    RT_HW_directModeOutput(pin)
	#define  RT_HW_DIRECT_READ(base, pin)           RT_HW_directRead(pin)
	#define  RT_HW_DIRECT_WRITE_LOW(base,   pin)    RT_HW_directWriteLow(pin)
	#define  RT_HW_DIRECT_WRITE_HIGH(base,  pin)    RT_HW_directWriteHigh(pin)
//--------------------------------------------------------------------------------------------------
/*
	#ifdef    interrupts
	  #undef  interrupts
	#endif
	#ifdef    noInterrupts
	  #undef  noInterrupts
    #endif
    #define noInterrupts() {static portMUX_TYPE mux = portMUX_INITIALIZER_UNLOCKED; portENTER_CRITICAL(&mux)
    #define interrupts()    portEXIT_CRITICAL(&mux);}
*/
#ifdef noInterrupts
  #undef noInterrupts
#endif
#ifdef interrupts
  #undef interrupts
#endif

#define noInterrupts() do { static portMUX_TYPE mux = portMUX_INITIALIZER_UNLOCKED; portENTER_CRITICAL(&mux);
#define interrupts()   portEXIT_CRITICAL(&mux); } while(0)

//=================================================================================================

//=================================================================================================
//										20. DIR:: Дискретный ввод или вывод
//                     		 Для совместимости со старыми функциями
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
	void 	setPeriod(uint16_t _period){period=_period;};						//--Изменение периода;		
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
	uint16_t cash=0, vPin, var;	
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
//=================================================================================================
  struct	RT_HW_PIN_PWM_ID {	
	uint8_t  dir=0, pin=255; char mode='N';
 	uint8_t  inv=0;																//--Флаг инверсии;
	uint16_t vPin=0;    
	uint16_t cash=0;
	uint16_t var=0;
	uint16_t freq=500, cashFreq=0;   
	uint8_t  depth=12;
	uint8_t  channel=0;
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
	uint16_t freq=500, cashFreq=0; 	
	uint8_t  depth=12;
	uint8_t  channel=0;	
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
	uint8_t  dir=0; char mode='N';
	uint16_t pin=0;    
	uint16_t cash=0;
	uint16_t var=0;
	uint16_t freq=100, cashFreq=0;   
	uint8_t  depth=12;
	uint8_t  channel=0;
//-------------------------------------------------------------------------------------------------
	uint16_t period;
	uint8_t  maxDepth;
	uint16_t duration;
	uint16_t duty;			//-- Длительность с учетом разрядности depth;
	uint16_t maxAngle=180, minWidth=800, maxWidth=2300;
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

//=================================================================================================
//
//										100. МАКРОСЫ/ФУНКЦИИ управления GPIO через структуру
//
//=================================================================================================
	#define RT_HW_PIN_DIR_SET_ID(id,p) 	        	id.pin=p;
	#define RT_HW_PIN_DIR_MODE_INPUT(id)        	RT_HW_directModeInput(id.pin);
	#define RT_HW_PIN_DIR_MODE_PULLUP(id) 	    	pinMode(id.pin,INPUT_PULLUP);
	#define RT_HW_PIN_DIR_MODE_OUTPUT(id) 	    	RT_HW_directModeOutput(id.pin); 	
	#define RT_HW_PIN_DIR_READ(id)              	RT_HW_directRead(id.pin) 			
	#define RT_HW_PIN_DIR_WRITE_HIGH(id)        	RT_HW_directWriteHigh(id.pin);	
	#define RT_HW_PIN_DIR_WRITE_LOW(id)         	RT_HW_directWriteLow(id.pin);
	#define RT_HW_PIN_DIR_WRITE(id,vr) 				if(vr){RT_HW_directWriteHigh(id.pin);}\
	                                                else  {RT_HW_directWriteLow(id.pin);}
//=================================================================================================
#endif   //--END #if !defined(RT_HW_CORE_CODE) 
//=================================================================================================