//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
// 								1.СЕРВИСНЫЕ ФУНКЦИИ ДЛЯ РАБОТЫ С ПИНАМИ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
	#if !defined RT_HW_ADC_DRIFT	
		#define  RT_HW_ADC_DRIFT  3
	#endif	
	#if !defined RT_HW_ADC_NULL	
		#define  RT_HW_ADC_NULL   4
	#endif	
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
// 									2.SERVO CONTROL
//-------------------------------------------------------------------------------------------------
//struct RT_HW_PIN_SERVO_ID{
//	uint8_t  dir=0;				//--Stage number in the executive function; 
//	uint8_t  pin=255;			//--Pin number;
//	uint8_t  channel=4;			//--Channel number[ESP32]
//	uint8_t  depth=16;			//--Bit depthESP32];
//	uint16_t freq=50;			//--Frequency;
//	uint16_t maxAngle=180;		//--The maximum angle of rotation (-360,+360);
//	uint16_t minWidth=800;		//--Minimum pulse width(mcs);   
//	uint16_t maxWidth=2300;		//--Maximum pulse width(mcs);  
//	uint16_t maxDepth=0xFFFF;   //--Maximum value for the selected bit depth;
//	uint16_t period=20000;		//--Period of control pulses(calculated variable=1000000/freq); 
//-------------------------------
//	uint8_t  idx;				//--Working index;
//	uint16_t var;				//--External pulse duration setting;
//	uint16_t duration;			//--Estimated pulse duration;
//	uint16_t duty;				//--The value written to the device (taking into account the PWM bit depth);
//};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++


//=================================================================================================
//								  2.1. УСТАНОВКА ПАРАМЕТРОВ СЕРВО
//=================================================================================================
  void     pinServoSet_MG90S (RT_HW_PIN_SRV_ID &id){id.freq=50; id.minWidth=650; id.maxWidth=2200; id.maxAngle=180;}
  void     pinServoSet_MG996R(RT_HW_PIN_SRV_ID &id){id.freq=50; id.minWidth=650; id.maxWidth=2200; id.maxAngle=180;}
//=================================================================================================
//								  2.2. Нормализация значения ширины управляющего импульса(mcs)
//=================================================================================================
  uint16_t pinServoNormSYS   (RT_HW_PIN_SRV_ID &id, uint16_t var){return map(var,0,((1<<RT_HW_DEPTH_SYS)-1),id.minWidth,id.maxWidth);}
//=================================================================================================
//								  2.3. Нормализация угла поворота
//=================================================================================================
   int16_t pinServoNormAngle(RT_HW_PIN_SRV_ID &id, uint16_t var){return map(var,0,id.maxAngle,id.minWidth,id.maxWidth);}
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++