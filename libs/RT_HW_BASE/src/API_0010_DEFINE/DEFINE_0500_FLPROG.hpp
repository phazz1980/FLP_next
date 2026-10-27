//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//						1. МАКРОСЫ ДЛЯ СОВМЕСТИМОСТИ С FLPROG
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
	#define	RT_HW_SENSOR_NOT_ERROR 0
	#define RT_HW_SENSOR_NOT_READY_ERROR 1
	#define RT_HW_SENSOR_DEVICE_NOT_FOUND_ERROR 2
	#define RT_HW_SENSOR_DEVICE_NOT_CORRECT_BUS_NUMBER_ERROR 65
	#define RT_HW_SENSOR_DEVICE_NOT_CORRECT_DEVICE_ADDRESS_ERROR 61
	#define RT_HW_SENSOR_DEVICE_NOT_DEFINED_ERROR 5
	#define RT_HW_SENSOR_DEVICE_NOT_CORRECT_DATA_ERROR 6
	#define RT_HW_SENSOR_CRC_ERROR 72
	#define RT_HW_SENSOR_WAITING_READ_STEP 0
	#define RT_HW_SENSOR_WAITING_DELAY 1
//=================================================================================================
	#define RT_HW_MODE_DOT_NORMAL 			'N'  	//--DOT::Дискретный выход->push-pull;
	#define RT_HW_MODE_DOT_OPEN 			'O'  	//--DOT::Дискретный выход->open collector;
	#define RT_HW_MODE_DIN_INPUT 			'I'  	//--DIN::Дискретный  вход->без подтяжки;
	#define RT_HW_MODE_DIN_INPUT_PULLUP 	'U'  	//--DIN::Дискретный  вход->c подтяжкой к Vcc;
	#define RT_HW_MODE_DIN_INPUT_PULLDOWN 	'D'  	//--DIN::Дискретный  вход->c подтяжкой к GND;
	#define RT_HW_MODE_DIN_INPUT_FLOAT 	    'F'  	//--DIN::Дискретный  вход->плавающий вход (STM32);	
	

