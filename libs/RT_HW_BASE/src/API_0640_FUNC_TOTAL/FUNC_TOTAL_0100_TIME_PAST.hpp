



//=================================================================================================
//							      1. ПОЛУЧЕНИЕ ПРОШЕДШЕГО ВРЕМЕНИ 
//  Перед вызовом необходимо установить параметры timeBegin, timeEnd;
//  К использованию рекомендуется getPastTime() вместо getPastMs(),getPastMcs().
//=================================================================================================
  uint32_t getPastTime(uint32_t &timeBegin, uint32_t &timeEnd) {	//==Возврат прошедшего времени по двум временным точкам; 
	if(timeEnd>=timeBegin)  {return (timeEnd-timeBegin);}  
	else 					{return (0xFFFFFFFF-timeBegin+timeEnd);}
  }; 
//=================================================================================================
  uint32_t getPastMs  (uint32_t &timeBegin){						//==Возврат прошед.времени в ms (millis()->timeBegin);
	if(millis()>=timeBegin) {return (millis()-timeBegin);} 
	else 					{return (0xFFFFFFFF-timeBegin+millis());}
  };	
//=================================================================================================
  uint32_t getPastUs (uint32_t &timeBegin){						//==Возврат прошед.времени в mcs(micros()->timeBegin);
	if(micros()>=timeBegin) {return (micros()-timeBegin);} 
	else 					{return (0xFFFFFFFF-timeBegin+micros());}
  };	
   uint32_t getPastMcs (uint32_t &timeBegin){return getPastUs(timeBegin);};	 
//=================================================================================================
//								2.ФУНКЦИИ ЗАДЕРЖКИ ПО ВРЕМЕНИ (возвращает событие 0/1)										 	
//=================================================================================================
  uint8_t timeDelayMs (uint32_t &timeDelay, uint8_t &stepDelay, uint32_t period){ //==delay в ms;
	if(stepDelay==0){timeDelay=millis();              stepDelay=1;}
	if(stepDelay==1){if(getPastMs(timeDelay)>period) {stepDelay=0; return 1;}}
	return 0;
  };	
//=================================================================================================
  uint8_t timeDelayUs(uint32_t &timeDelay, uint8_t &stepDelay, uint32_t period){ //==delay в mcs;
	if(stepDelay==0){timeDelay=micros();              stepDelay=1;}
	if(stepDelay==1){if(getPastUs(timeDelay)>period) {stepDelay=0; return 1;}}
	return 0;
  };
 //=================================================================================================
//								3. РАСЧЕТ ПРОШЕДШЕГО ВРЕМЕНИ внутри функций прерывания 
// Параметры следует определять следующим образом:
//   volatic uint32_t timeBegin, uint32_t timeEnd;
//=================================================================================================
  uint32_t calcPastTime(uint32_t timeBegin, uint32_t timeEnd){
	if(timeEnd>=timeBegin){return (timeEnd-timeBegin);} 	
	else				  {return (0xFFFFFFFF-timeBegin+timeEnd);}
  }; 
//=================================================================================================
//							    4. РАСЧЕТ МОМЕНТА 
//  Вход - время в миллисекундах на которое надо откатиться назад от текущего времени
//  Выход - значение функции millis() которое было указанное время назад.
//=================================================================================================
  uint32_t getTimeBack(uint32_t value){
    uint32_t current = millis();
    if (value < current){return current - value;}
    return (0xfFFFFFFF - value) + current;
  };
  
//=================================================================================================
//								5.ФУНКЦИИ КОНТРОЛЯ ПО ВРЕМЕНИ(возвращает событие 0/1)
//										(в стиле FLPROG)										 	
//=================================================================================================
  uint8_t  getIsTimerMs(uint32_t timeBegin, uint32_t period){
    uint32_t timeEnd=millis();
	if(getPastTime(timeBegin, timeEnd)>= period) {return 1;}
												  return 0;
  };
//-------------------------------------------------------------------------------------------------
  uint8_t  getIsTimerUs(uint32_t timeBegin, uint32_t period){
    uint32_t timeEnd=micros();
	if(getPastTime(timeBegin, timeEnd)>= period) {return 1;}
												  return 0;
  };   
  

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//									УСТАРЕВШИЕ ФУНКЦИИ !!!
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++ 
  
//=================================================================================================
//								10.ФУНКЦИИ ЗАДЕРЖКИ ПО ВРЕМЕНИ FLPROG(возвращает событие 0/1)	
//																	 	
//=================================================================================================  
  uint32_t difference32(uint32_t start, uint32_t end){
	if (end >= start){ 	return (end - start);}
						return ((0xfFFFFFFF - start) + end);
  };
//-------------------------------------------------------------------------------------------------
  uint8_t  isTimer(uint32_t startTime, uint32_t period){
    return (difference32(startTime, (millis()))) >= period;
  };
//-------------------------------------------------------------------------------------------------
  uint8_t  isTimerMs(uint32_t startTime, uint32_t period){
    return (difference32(startTime, (millis()))) >= period;
  };
//-------------------------------------------------------------------------------------------------
  uint8_t  isTimerUs(uint32_t startTime, uint32_t period){
    return (difference32(startTime, (micros()))) >= period;
  };
//-------------------------------------------------------------------------------------------------
  uint8_t isTimerMicros(uint32_t startTime, uint32_t period){
    return (difference32(startTime, (micros()))) >= period;
  };
//-------------------------------------------------------------------------------------------------
  uint32_t timeBack(uint32_t value){
    uint32_t current = millis();
    if (value < current){return current - value;}
                         return (0xfFFFFFFF - value) + current;
 };
//=================================================================================================
//								11. УСТАРЕВШАЯ ФУНКЦИИ ЗАДЕРЖКИ ПО ВРЕМЕНИ (возвращает событие 0/1)										 	
//=================================================================================================
  uint8_t timeDelay (uint32_t &timeDelay, uint8_t &stepDelay, uint32_t period){return timeDelayMs(timeDelay, stepDelay, period);};

//=================================================================================================
//								12.ФУНКЦИИ проверки (возвращает событие 0/1)
// struct RT_HW_STRUCT_TIME{                    
//  uint32_t startTime,curTime;		//--Переменные для работы со временем;
//  uint16_t sizeDelay;				//--Время задержки;
//  uint8_t  state=0;				//--Состояние функции (1 - ожидание завершение задержки);
//  uint8_t  pointTime; 			//--Точка возрата из функции(для пользовательского использования);
//};										 
//=================================================================================================
  uint8_t checkDelay   (RT_HW_STRUCT_TIME &id, uint32_t sizeDelay, uint8_t vUnit=1){
	if(vUnit){id.curTime=millis();} else {id.curTime=micros();}
	if(id.state==0){id.startTime=id.curTime; id.state=1; id.sizeDelay=sizeDelay;}
	if(id.curTime >=id.startTime){if((id.curTime-id.startTime)           >id.sizeDelay){id.state=0; return 1;}}
	else                         {if((0xFFFFFFFF-id.startTime+id.curTime)>id.sizeDelay){id.state=0; return 1;}}	
	return 0;
  };
  uint8_t checkDelayMs(RT_HW_STRUCT_TIME &id, uint32_t sizeDelay){return checkDelay(id,    sizeDelay, (uint8_t)1);}; 
  uint8_t checkDelayUs(RT_HW_STRUCT_TIME &id, uint32_t sizeDelay){return checkDelay(id,    sizeDelay, (uint8_t)0);}; 
  uint8_t checkDelayMs(RT_HW_STRUCT_TIME &id)                    {return checkDelay(id, id.sizeDelay, (uint8_t)1);}; 
  uint8_t checkDelayUs(RT_HW_STRUCT_TIME &id)                    {return checkDelay(id, id.sizeDelay, (uint8_t)0);}; 


//=================================================================================================
//								СЕРВИСНЫЕ ФУНКЦИИ
//=================================================================================================
//-------------------------------------------------------------------------------------------------
//					Конвертирует время в тики (для ускорения работы таких библиотек как RT_HW_HD44780, RT_HW_NEXTION & etc)
//-------------------------------------------------------------------------------------------------
uint8_t timeToTick(uint16_t time,  uint8_t tick=50){
if(tick==0){tick=1;}	
if((tick*255)>=time){return time/tick;}	else {return 255;}};
//=================================================================================================

//=================================================================================================
//								УСТАРЕВШИЕ ФУНКЦИИ
//=================================================================================================
//-------------------------------------------------------------------------------------------------
/*
  uint8_t checkDelayMs (RT_HW_STRUCT_TIME &id){
	id.curTime=millis();
	if(id.state==0){id.startTime=id.curTime; id.state=1;}
	if(id.curTime >=id.startTime){if((id.curTime-id.startTime)           >id.sizeDelay){id.state=0;}}
	else                         {if((0xFFFFFFFF-id.startTime+id.curTime)>id.sizeDelay){id.state=0;}}	
	return id.state;
  };
  uint8_t checkDelayMS (RT_HW_STRUCT_TIME &id){return checkDelayMs(id);};
  
//-------------------------------------------------------------------------------------------------
  uint8_t checkDelayUs (RT_HW_STRUCT_TIME &id){id.curTime=micros();
	if(id.state==0){id.startTime=id.curTime; id.state=1;}
	if(id.curTime >=id.startTime){if((id.curTime-id.startTime)           >id.sizeDelay){id.state=0;}}
	else                         {if((0xFFFFFFFF-id.startTime+id.curTime)>id.sizeDelay){id.state=0;}}	
	return id.state;
  };
  uint8_t checkDelayUS (RT_HW_STRUCT_TIME &id){return checkDelayUs(id);};
*/  
//=================================================================================================
