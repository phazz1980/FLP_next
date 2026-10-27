
#include <Arduino.h>	

 
//=================================================================================================
//							      1. ПОЛУЧЕНИЕ ПРОШЕДШЕГО ВРЕМЕНИ 
//  Перед вызовом необходимо установить параметры timeBegin, timeEnd;
//  К использованию рекомендуется getPastTime() вместо getPastMs(),getPastMcs().
//=================================================================================================
  uint32_t RT_HW_getPastTime(uint32_t &timeBegin, uint32_t &timeEnd) {	//==Возврат прошедшего времени по двум временным точкам; 
	if(timeEnd>=timeBegin)  {return (timeEnd-timeBegin);}  
	else 					{return (0xFFFFFFFF-timeBegin+timeEnd);}
  }; 
/*
//=================================================================================================
  uint32_t RT_HW_getPastMs  (uint32_t &timeBegin){						//==Возврат прошед.времени в ms (millis()->timeBegin);
	if(millis()>=timeBegin) {return (millis()-timeBegin);} 
	else 					{return (0xFFFFFFFF-timeBegin+millis());}
  };	
//=================================================================================================
  uint32_t RT_HW_getPastUs (uint32_t &timeBegin){						//==Возврат прошед.времени в mcs(micros()->timeBegin);
	if(micros()>=timeBegin) {return (micros()-timeBegin);} 
	else 					{return (0xFFFFFFFF-timeBegin+micros());}
  };	
*/
  
/*
//=================================================================================================
//								2.ФУНКЦИИ ЗАДЕРЖКИ ПО ВРЕМЕНИ (возвращает событие 0/1)										 	
//=================================================================================================
  uint8_t RT_HW_timeDelayMs (uint32_t &timeDelay, uint8_t &stepDelay, uint32_t period){ //==delay в ms;
	if(stepDelay==0){timeDelay=millis();              stepDelay=1;}
	if(stepDelay==1){if(RT_HW_getPastMs(timeDelay)>period) {stepDelay=0; return 1;}}
	return 0;
  };	
//=================================================================================================
  uint8_t RT_HW_timeDelayUs(uint32_t &timeDelay, uint8_t &stepDelay, uint32_t period){ //==delay в mcs;
	if(stepDelay==0){timeDelay=micros();              stepDelay=1;}
	if(stepDelay==1){if(RT_HW_getPastUs(timeDelay)>period) {stepDelay=0; return 1;}}
	return 0;
  };
//=================================================================================================
//								5.ФУНКЦИИ КОНТРОЛЯ ПО ВРЕМЕНИ(возвращает событие 0/1)
//										(в стиле FLPROG)										 	
//=================================================================================================
  uint8_t  RT_HW_getIsTimerMs(uint32_t timeBegin, uint32_t period){
    uint32_t timeEnd=millis();
	if(RT_HW_getPastTime(timeBegin, timeEnd)>= period) {return 1;}
														return 0;
  };
//-------------------------------------------------------------------------------------------------
  uint8_t  RT_HW_getIsTimerUs(uint32_t timeBegin, uint32_t period){
    uint32_t timeEnd=micros();
	if(RT_HW_getPastTime(timeBegin, timeEnd)>= period) {return 1;}
														return 0;
  };     
//=================================================================================================
*/
