//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//						Предложения по изменениям FLProg.
//								01.02.2025г.
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//--1--------------------------------------------------------------------------
Блок init->CNS генерит следующий код (параметр скорости "В соответствии с интефейсом") 
    НЕВЕРНО устанавливается скорость.
	   if(! InitConsole_1_init) 
        {
            InitConsole_1_init = 1;
            RT_HW_console.setParam((5), (RT_HW_Base.uartGetSpeed((5))));
        }
//--2--------------------------------------------------------------------------	
Блок board->CNS.
	Похоже на выход блока сейчас ничего не подается.
		Хорошее решение вывод по выбору (оди или все) три выхода:
			run (консоль готова к выводу);
			head(событие вывода заголовков);
			ok  (готовности консоли после вывода заголовков);
//--3--------------------------------------------------------------------------
	В скетче после платы инициализации идут добавленные функциb FLProg- диспетчер, pool() и пр.
		Визуально их сложно отделить. Было бы хорошо отделить их горизонтальными линиями (комментарии),
		возможно с	заголовком, например:
	//-----------------------------BEGIN FLPprog FUNCTIONS-------------------------- 

	//-----------------------------END   FLPprog FUNCTIONS-------------------------- 	 

//--4--------------------------------------------------------------------------
	В LCD наверное можно сразу создавать экран 1. 
	В скетче блоки от LCD генерят дополнительные пустые строки. Неудобно смотреть.

//--7--------------------------------------------------------------------------
	По слову "отключено".
	При использовании Wi-Fi в нескольких местах лишняя "н" - "отключенно".
	Хорошо бы поправить.
	
//--8--------------------------------------------------------------------------	
	В описаниях блоков хорошо бы добавить плату, которая уже есть в описании
	ESP32S Dev Module BlueTooth.
	Эта плата будет мостиком в Bluetooth - протестировал на машинке, работает.
	
	Также лучше изменить название имеющегося модуля ESP32 WROOM DEVKIT V1 
	на название как в Arduino - ESP32 Dev Module.
	
	И видимо пока лучше убратьиз интерфейсов пункт =интегированный блютуз модуль.
	
//--9--------------------------------------------------------------------------	
	Вроде большое кол-во плат и контроллеров перегружают оперативную память.
	На мой взгляд многие платы и модули из ESP8266 могут быть убраны.
	Оставить/переименовать: 
		Generic 8266 Module
		Generic 8285 Module		
		NodeMCU 0.9 (ESP8266-12 Module)
		NodeMCU 1.0 (ESP8266-12E Module)
		LOLIN(WEMOS) D1 mini Pro
	Остальные можно будет добавлять по запросу.	
	
	Наверное и от экзотики пора избавляться - Intel Galileo.
	Тем более что она тянет за собой папку Intel 110 МБ.
	
//--10-------------------------------------------------------------------------		
	Было в планах заблокировать сообщения от ModBus 
	
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusSlaveRTU.cpp: In member function 'virtual uint8_t ModbusSlaveRTU::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusSlaveRTU.cpp:69:9: warning: unused variable 'msgCRC' [-Wunused-variable]
   69 |     int msgCRC =
      |         ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusMasterRTU.cpp: In member function 'virtual uint8_t ModbusMasterRTU::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusMasterRTU.cpp:118:13: warning: unused variable 'msgCRC' [-Wunused-variable]
  118 |     int16_t msgCRC = ((_buffer[_bufferSize - 2] << 8) | _buffer[_bufferSize - 1]);
      |             ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusMasterTCP.cpp: In member function 'virtual uint8_t ModbusMasterTCP::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusMasterTCP.cpp:585:17: warning: unused variable 'msgCRC' [-Wunused-variable]
  585 |         int16_t msgCRC = ((_buffer[_bufferSize - 2] << 8) | _buffer[_bufferSize - 1]);
      |                 ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusSlaveTCP.cpp: In member function 'virtual uint8_t ModbusSlaveTCP::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusSlaveTCP.cpp:271:13: warning: unused variable 'msgCRC' [-Wunused-variable]
  271 |         int msgCRC =
      |             ^~~~~~

//--13-------------------------------------------------------------------------	
   Это строка из скетча.
   uint8_t stm32_Plc_DisplayButtons_Fresh = 0; //============================================AHTxx.Declare=========================================
   
   И видим, что после объявления переменной отсутствут перевод строки.
   Хорошо бы поправить.

//--14-------------------------------------------------------------------------
	В версии RT_HW_BASE.h > 5.3.0 	прописан API диспетчера.
	Из FLProg убрать избыточные функции, которые усложняют тело скетча.
//=================================================================================================
//													API sheduler
//=================================================================================================
	uint8_t	 getShedReady()			{return (shed.ready);	   };	//--Готовность диспетчера. Устанавливается после первого вызова диспетчера;
//-------------------------------------------------------------------------------------------------
//			Возвращают события 10,25,50,100,250,500,1000 мс.
//			Только для внутренних функций - в FLProg не рекомендуется к использованию
//-------------------------------------------------------------------------------------------------
	uint8_t  getShedEvent10()    	{return (shed.event)    &1;};	//--Событие 10 мс.
	uint8_t  getShedEvent25()    	{return (shed.event>>1) &1;};	//--Событие 25 мс.
	uint8_t  getShedEvent50()    	{return (shed.event>>2) &1;};	//--Событие 50 мс.
	uint8_t  getShedEvent100()   	{return (shed.event>>3) &1;};	//--Событие 100 мс.
	uint8_t  getShedEvent250()   	{return (shed.event>>4) &1;};	//--Событие 250 мс.
	uint8_t  getShedEvent500()   	{return (shed.event>>5) &1;};	//--Событие 500 мс.
	uint8_t  getShedEvent1000()  	{return (shed.event>>6) &1;};	//--Событие 1000 мс.
//-------------------------------------------------------------------------------------------------
//			Возвращают меандры 10,25,50,100,250,500,1000 мс. Длительность соответствует полупериоду.
//-------------------------------------------------------------------------------------------------
	uint8_t  getShedBlink10()    	{return  shed.blink10;};	//--Меандр 10   мс.
	uint8_t  getShedBlink25()    	{return  shed.blink25;};	//--Меандр 25   мс.
	uint8_t  getShedBlink50()    	{return  shed.blink50;};	//--Меандр 50   мс.
	uint8_t  getShedBlink100()    	{return  shed.blink100;};	//--Меандр 100  мс.
	uint8_t  getShedBlink250()    	{return  shed.blink250;};	//--Меандр 250  мс.
	uint8_t  getShedBlink500()    	{return  shed.blink500;};	//--Меандр 500  мс.
	uint8_t  getShedBlink1000()    	{return  shed.blink1000;};	//--Меандр 1000 мс.	
//-------------------------------------------------------------------------------------------------
//			Возвращают быстродействие CORE0, CORE1  циклах loop() в сек
//-------------------------------------------------------------------------------------------------
	uint32_t getShedSpeed()      	{return shed.cntCycle;};	//--Быстродействие Core0;
	uint32_t getShedSpeed0()     	{return shed.cntCycle;};	//--Быстродействие Core0;		
	uint16_t getShedSpeed_small() 	{return shed.smallCycle;};  //--Быстродействие Core0/1000;
	uint16_t getShedSpeed0_small()	{return shed.smallCycle;};  //--Быстродействие Core0/1000;	
	uint32_t getShedSpeed1()      	{return shed.cntCycle1;};	//--Быстродействие Core0;	
	uint16_t getShedSpeed1_small() 	{return shed.smallCycle1;}; //--Быстродействие Core0/1000;	
//-------------------------------------------------------------------------------------------------
//			Возвращает мото-секунды (время работы процессора в сек)
//-------------------------------------------------------------------------------------------------
	uint32_t getShedMotoSec()   	{return shed.cntSec;};		//--Мотосекунды;		
//=================================================================================================


//--14-------------------------------------------------------------------------
		ПРИГОТОВИТЬСЯ!!! Вроде удалось существенно сократить описание PLC через привязку к базовому описанию.
			Наверное уберется и раздел 20.
			Готовлю API для доступа к пинам через номер пина, 
			что-то вроде getPinDOT(n),getPinDIN(n),getPinAC(n),getPinDAC(n),getPinPWM(n),getPinDOX(n),getPinDIX(n),getPinADX(n).
			Обращение к регистрам 74HC595,74HC165 пока наверное будет через костыли.

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++						
//									 	PLC FX2_32Mx 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_PLC_CODE) &&  defined(FLP_BOARD_PLC_FX2_32MX)  
	#define  RT_HW_PLC_STM32F103VCx 							//--Идентификатор для привязки к базовому MCU;   
	#define  RT_HW_PLC_CODE             (uint16_t)5320			//--Код;	
    #define  RT_HW_PLC_NAME			"FLProg PLC FX2-32Mx"		//--Наименование;
	#define  RT_HW_PLC_FX2_XX 									//--Группа PLC;	
	#define  RT_HW_PLC_FX2_32Mx 								//--Идентификатор PLC;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------	
    #define RT_HW_PLC_TEXT1 	"UART2(PA2-TX2, PA3-RX2) доступен при условии не использования входов AD2,AD3"
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_PLC_UART_CONSOLE1   		1			//--Консоль через UART 1;
	#define  RT_HW_PLC_UART_CONSOLE2   		2			//--Консоль через UART 2;	
	#define  RT_HW_PLC_UART_MODBUS1   		1			//--Номер UART для MODBUS 1;
	#define  RT_HW_PLC_UART_MODBUS2   		4			//--Номер UART для MODBUS 2;
	#define  RT_HW_PLC_UART_HMI1			1			//--Номер UART для панели HMI 1;
	#define  RT_HW_PLC_UART_HMI2			2			//--Номер UART для панели HMI 2;
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOT 	(uint8_t)14			//--Доступные пины DOT;
    #define RT_HW_PERMIT_PLC_DIN	(uint8_t)16			//--Доступные пины DIN;
    #define RT_HW_PERMIT_PLC_ADC	(uint8_t)8			//--Доступные пины ADC; 
    #define RT_HW_PERMIT_PLC_DAC	(uint8_t)2			//--Доступные пины DAC; 	
    #define RT_HW_PERMIT_PLC_PWM	(uint8_t)13			//--Доступные пины PWM; 
//-------------------------------------------------------------------------------------------------
    #define RT_HW_PERMIT_PLC_DOX 	(uint8_t)2			//--Доступные пины DOX;
    #define RT_HW_PERMIT_PLC_DIX	(uint8_t)2			//--Доступные пины DIX;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
//                                                X00  X01  X02  X03  X04  X05  X06  X07  X08  X09  X10  X11  X12  X13  X14  X15  X16  X17
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOT_ID[]={PC9, PA8, PA11,PA12,PE14,PE13,PE12,PE11,255, 255, PE10, PE9, PE8, PE7,  						RT_HW_PGM_END};
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIN_ID[]={PB5, PB4, PB7, PB3, PA15,PB6, PB8, PB9, 255, 255, PE0,  PE1, PC10,PC11,PC12,PD0, PD1, PD2,  	RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADC_ID[]={PA0, PA1, PA2, PA3, PC0, PC1,	PC2, PC3,														RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_DAC_ID[]={PA4, PA5,																						RT_HW_PGM_END};	
    const uint8_t PROGMEM RT_HW_PGM_PLC_PWM_ID[]={255, 255, PA8, PA0, 255, 255, 255, 255, 255, 255, 255, PB8, PB9,  							RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------------------------------------------------------------------
    const uint8_t PROGMEM RT_HW_PGM_PLC_DOX_ID[]={PD12,PD13,   	RT_HW_PGM_END}; 	//--YRUN, YERR;
    const uint8_t PROGMEM RT_HW_PGM_PLC_DIX_ID[]={PB2, PE2, 	RT_HW_PGM_END};		//--XSW,  XVCC;
    const uint8_t PROGMEM RT_HW_PGM_PLC_ADX_ID[]={			 	RT_HW_PGM_END};		//--AUST1,AUST2;
//-------------------------------------------------------------------------------------------------------------------------------------------------------------	
#endif
//=================================================================================================

//--15-------------------------------------------------------------------------
	А можно ли в FLProg отображать версию в которой создан проект.
	И если где в начале в комментах отразить эту информацию в скетче, то будет проще понимать проблемы пользователей, 
	которые часто выкладывают ошибки и непонятно в какой версии создан проект.
	
//--16-------------------------------------------------------------------------
	Убрать бы предупреждения от ModBus
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusSlaveRTU.cpp: In member function 'virtual uint8_t ModbusSlaveRTU::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusSlaveRTU.cpp:69:9: warning: unused variable 'msgCRC' [-Wunused-variable]
   69 |     int msgCRC =
      |         ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusMasterRTU.cpp: In member function 'virtual uint8_t ModbusMasterRTU::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\rtu\flprogModbusMasterRTU.cpp:118:13: warning: unused variable 'msgCRC' [-Wunused-variable]
  118 |     int16_t msgCRC = ((_buffer[_bufferSize - 2] << 8) | _buffer[_bufferSize - 1]);
      |             ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusMasterTCP.cpp: In member function 'virtual uint8_t ModbusMasterTCP::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusMasterTCP.cpp:585:17: warning: unused variable 'msgCRC' [-Wunused-variable]
  585 |         int16_t msgCRC = ((_buffer[_bufferSize - 2] << 8) | _buffer[_bufferSize - 1]);
      |                 ^~~~~~
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusSlaveTCP.cpp: In member function 'virtual uint8_t ModbusSlaveTCP::validateRequest()':
D:\0127_FLProg 9.2.7\ideV8\portable\sketchbook\libraries\Flprog_Modbus\src\tcp\flprogModbusSlaveTCP.cpp:271:13: warning: unused variable 'msgCRC' [-Wunused-variable]
  271 |         int msgCRC =
      |             ^~~~~~
//--17-------------------------------------------------------------------------
	MODBUS - При создании регистров в таблице не увидел где можно задавать начальный адрес.
	Это возможно делать/сделать?
	Приемы работы вроде подобрал, наверное можно оставить как есть.

	MODBUS!!! Записываю в регистр 0 с установкой записи по детектору изменения. В регистре мусор.
	Убираю по изменению - нормально.
	Возможно, что первый раз не пишет, ва в регистре по умолчанию не 0.


//--15-------------------------------------------------------------------------
	Имеет ли смысл в меню справа - Система-Чтение и запись систеиного параметра?
	При записи в системный параметр blink1000 - вылет  с карандашом....
	
	