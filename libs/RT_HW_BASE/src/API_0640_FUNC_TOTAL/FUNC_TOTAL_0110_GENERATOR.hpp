//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//							      1. ГЕНЕРАТОРЫ СОБЫТИЙ С ТОЧНЫМ ПЕРИОДОМ
// Идея от https://alexgyver.ru/arduino-algorithms/
// timeBegin -время начала отсчета (определяется вне функции); 
// timeEnd   -время текущее;
// timeMax   -тайм-аут;
// Функция генерит событие с точным поддержанием периода и защитой от пропуска шага.
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						1.1.Генератор периодических событий в ms
//=================================================================================================
uint8_t periodMs (uint32_t &timeBegin, uint32_t period){ //==Точный генератор событий ms;
if(!period){return 1;}   //--Защита от нулевого значения периода;
if((millis()-timeBegin)>=period){do{timeBegin+=period; if(timeBegin<period) {break;}
                   } while(timeBegin<(millis()-period));   return 1;} 
return 0;
};
//=================================================================================================
//						1.2.Генератор периодических событий в mcs
//=================================================================================================
uint8_t periodUs (uint32_t &timeBegin, uint32_t period){ //==Точный генератор событий mсs;
if(!period){return 1;}   //--Защита от нулевого значения периода;
if((micros()-timeBegin)>=period){do{timeBegin+=period; if(timeBegin<period) {break;}
                   } while(timeBegin<(micros()-period));   return 1;} 
return 0;
};
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//									2. ГЕНЕРАТОРЫ СОБЫТИЙ (МЕАНДРОВ)
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

//=================================================================================================
//							2.1. Генератор события:
// по переднему фронту EN [R];
// по заднему   фронту EN [F];
// по изменению фронта EN [C];
// по высокому  уровню EN [H];
// по низкому   уровню EN [L];
//	Вход: ago-рабочий регистр(255), EN-внешний вход;	
//  Выход: возвращаемое значение bool;
//=================================================================================================
  uint8_t generatorChangeFrontR(uint8_t &ago, uint8_t EN){if(ago!=EN) {ago=EN; if( EN){return 1;}} return 0;};//--По переднему фронту;
  uint8_t generatorChangeFrontC(uint8_t &ago, uint8_t EN){if(ago!=EN) {ago=EN;         return 1;}  return 0;};//--По обеим фронтам;
  uint8_t generatorChangeFrontF(uint8_t &ago, uint8_t EN){if(ago!=EN) {ago=EN; if(!EN){return 1;}} return 0;};//--По заднему фронту;	
  uint8_t generatorChangeFrontH(uint8_t &ago, uint8_t EN){(void)ago;  {        if( EN){return 1;}} return 0;};//--По высокому уровню;
  uint8_t generatorChangeFrontL(uint8_t &ago, uint8_t EN){(void)ago;  {        if(!EN){return 1;}} return 0;};//--По низкому уровню;
//=================================================================================================
//						   2.2.Генератор событий с делителем
//================================================================================================= 
 uint8_t  generatorDividerR(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){
	(void)cnt; (void)divider; (void)EN;
	if(ago!=EN){ago =EN;	if( EN){if(++cnt>divider){cnt=1; return 1;}}} return 0;};
//-------------------------------------------------------------------------------------------------
 uint8_t  generatorDividerF(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){
	(void)cnt; (void)divider;
	if(ago!=EN){ago =EN;	if(!EN){if(++cnt>divider){cnt=1; return 1;}}} return 0;};
//-------------------------------------------------------------------------------------------------
 uint8_t  generatorDividerC(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){
	(void)cnt; (void)divider;
	if(ago!=EN){ago =EN;	        if(++cnt>divider){cnt=1; return 1;}}  return 0;};	
//-------------------------------------------------------------------------------------------------
 uint8_t  generatorDividerH(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){
	(void)ago; (void)cnt; (void)divider;
	if( EN){if(++cnt>divider){cnt=1; return 1;}} return 0;};		
//-------------------------------------------------------------------------------------------------
 uint8_t  generatorDividerL(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){
	(void)ago; (void)cnt; (void)divider;
	if( EN){if(++cnt>divider){cnt=1; return 1;}} return 0;};
//--------------------- Для совместимости со старыми блоками (аналог generatorDividerR() ----------
 uint8_t  generatorDivider(uint8_t &ago, uint8_t &cnt, uint8_t divider, uint8_t EN){return generatorDividerR(ago, cnt, divider, EN);}
//=================================================================================================
//							2.3. Генератор событий с делителем событий(divder) по переднему фронту EN
//================================================================================================= 
  uint8_t 	dividerEventR (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){		
	(void)divider; (void)EN;
	id.event=0; if(generatorChangeFrontR(id.ago,EN)){if(++id.cnt>divider){
				id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
  };
//=================================================================================================
//							2.4. Генератор событий с делителем событий(divder) по изменению фронта EN 
//================================================================================================= 
  uint8_t 	dividerEventC (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){
	(void)divider; (void)EN;
	id.event=0;	if(generatorChangeFrontC(id.ago,EN)){if(++id.cnt>divider){
				id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
  };
//=================================================================================================
//							2.5. Генератор событий с делителем событий(divder) по заднему   фронту EN 
//================================================================================================= 
uint8_t dividerEventF (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){
	(void)divider; (void)EN;
	id.event=0; if(generatorChangeFrontF(id.ago,EN)){if(++id.cnt>divider) {
	id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
};
//=================================================================================================
//							2.6. Генератор событий с делителем событий(divder) по EN=1
//================================================================================================= 
uint8_t dividerEventH (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){
	(void)divider; (void)EN;
	id.event=0; if(EN){if(++id.cnt>divider) {
	id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
}; 
 //=================================================================================================
//							2.7. Генератор событий с делителем событий(divder) по EN=0
//================================================================================================= 
uint8_t 	dividerEventL (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){
	(void)divider; (void)EN;
	id.event=0; if(!EN){if(++id.cnt>divider) {
	id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
};  
//=================================================================================================
//							2.8. Генератор событий с делителем событий(divder) в каждом цикле
//================================================================================================= 
uint8_t dividerEventE (RT_HW_GENERATOR_FRONT_ID &id, uint8_t divider, uint8_t EN){
	(void)divider; (void)EN;
	id.event=0; if(EN){if(++id.cnt>divider) {
	id.cnt=1; id.event=1; if(id.meander){id.meander=0;} else {id.meander=1;}}}
	return id.event;	
};
//=================================================================================================
//							2.9. Генератор событий(меандров) периодический ms
//	Вход: id-рабочая структура, EN-внешний вход, period-период ms.
//	Выход: возвращаемое значение bool=id.event;		id.event-событие; id.meander-меандр.
//=================================================================================================
uint8_t  generatorEventMs(RT_HW_GENERATOR_EVENT_ID &id, uint32_t period, uint8_t EN=1){
	(void)period; (void)EN;
	id.event=0;
	if(id.dir==0) {id.time=millis(); id.event=id.meander=1; id.dir++;}
	if(id.dir==1) {if(EN) {id.dir++;} else {id.event=id.meander=0; id.time=millis();}}
	if(id.dir==2) {if(periodMs(id.time,period)) {id.event=1; if(EN) {id.meander=!id.meander;} else {id.meander=0;} id.dir=1;}}
	return id.event;	
};
//=================================================================================================
//							2.10. Генератор событий(меандров) периодический mсs
//	Вход: id-рабочая структура, EN-внешний вход, period-период mcs.
//	Выход: возвращаемое значение bool=id.event;	id.event-событие; id.meander-меандр.
//=================================================================================================
uint8_t generatorEventUs(RT_HW_GENERATOR_EVENT_ID &id, uint32_t period, uint8_t EN=1){	 
	(void)period; (void)EN;		
	id.event=0;
	if(id.dir==0) {id.time=micros(); id.event=id.meander=1; id.dir++;}
	if(id.dir==1) {if(EN) {id.dir++;} else {id.event=id.meander=0; id.time=micros();}}
	if(id.dir==2) {if(periodUs(id.time,period)) {id.event=1; if(EN) {id.meander=!id.meander;} else {id.meander=0;} id.dir=1;}}
	return id.event;	
};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										3. ГЕНЕРАТОРЫ БЕГУЩЕЙ ЕДИНИЦЫ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						  	3.1. ГЕНЕРАТОР БЕГУЩЕЙ ЕДИНИЦЫ (внешнее тактирование)
// 1.EN-тактирующий меандр(передний фронт);
// 2.Для непрерывной работы: 1->EN, 0->id.agoEN; 
//==================================================================================================
  void 	generatorRunOne(RT_HW_GENERATOR_RUN_ID &id, char mode, uint8_t bottom=1, uint8_t top=8, uint8_t EN=1){
		(void)mode; (void)bottom; (void)top; (void)EN;
		if(id.agoEN!=EN) {id.agoEN=EN; if(EN){
//-------------------------------------------------------------------------------------------------
		if(mode=='S'){if(id.cnt==top){id.cnt=bottom;} 
					  else           {id.cnt++; if(id.cnt>top)  {id.cnt=top;}}}	  
//-------------------------------------------------------------------------------------------------
		else         {if(id.cnt==top)   {id.direct=1;} 
					  if(id.cnt==bottom){id.direct=0;}
					  if(id.direct==0)	{id.cnt++; if(id.cnt>top)    {id.cnt=top;}}
					  else              {id.cnt--; if(id.cnt<bottom) {id.cnt=bottom;}}}	
//-------------------------------------------------------------------------------------------------
		if((id.cnt>0) && (id.cnt<=32)){id.dc=(uint32_t)1<<(id.cnt-1);} 
		else                          {id.dc=0;} 						//--32-разрядный дешифратор;    
    }}
//-------------------------------------------------------------------------------------------------
	};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										4. ГЕНЕРАТОРЫ ИМПУЛЬСОВ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							4.1. Генератор импульсов uint16_t ms
//	Вход: id-рабочая структура, EN-внешний вход, timeON-время включения в ms, timeOFF-время выключения в ms.
//	Выход: возвращаемое значение bool -генерируемый импульс.
//=================================================================================================  
  uint8_t generatorPulse16(RT_HW_GENERATOR_PULSE_ID &id, uint16_t timeON, uint16_t timeOFF, uint8_t EN=1){
		(void)timeON;	(void)timeOFF;	(void)EN;
		if(id.dir==0) {if(EN) {id.dir=3;} else {return 0;}}
		if(id.dir==1) {if(millis()==id.time) {id.time+=timeOFF; id.dir=2; return 0;} else {return 1;}}
		if(id.dir==2) {if(millis()>=id.time) {id.dir=3;}}
		if(id.dir==3) {if(EN) {id.time=millis()+timeON; 		id.dir=1; return 1;}
					   else 								   {id.dir=0; return 0;}}	   
	return 0;};
//=================================================================================================
//							4.2. Генератор импульсов uint32_t ms
//	Вход: id-рабочая структура, EN-внешний вход, timeON-время включения в ms, timeOFF-время выключения в ms.
//	Выход: возвращаемое значение bool -генерируемый импульс.
//================================================================================================= 
  uint8_t generatorPulse32(RT_HW_GENERATOR_PULSE_ID &id, uint32_t timeON, uint32_t timeOFF, uint8_t EN=1){	
		(void)timeON;	(void)timeOFF;	(void)EN;		
		if(id.dir==0) {id.dir=3;}
		if(id.dir==1) {if((millis()-id.time)>= timeON) 			{id.dir=2; 	 return 0;} else {return 1;}}
		if(id.dir==2) {if((millis()-id.time)>=(timeON+timeOFF)) {id.dir=3;}}
		if(id.dir==3) {if(!EN) 		 {return 0;} id.time=millis(); 
					   if(timeON ==0){return 0;} 
					   if(timeOFF==0){return 1;}
					   id.dir=1; 	  return 1;}	
	return 0;
  };
  uint8_t generatorPul32(RT_HW_GENERATOR_PULSE_ID &id, uint32_t timeON, uint32_t timeOFF, uint8_t EN=1){
	      return generatorPulse32(id, timeON, timeOFF, EN); 
  };
  
  
//=================================================================================================
//							4.3. Генератор импульсов uint32_t mcs
//	Вход: id-рабочая структура, timeON-время включения в ms, timeOFF-время выключения в ms.
//	Выход: возвращаемое значение bool -генерируемый импульс.
//================================================================================================= 
  uint8_t generatorPul32mcs(RT_HW_GENERATOR_PULSE_ID &id, uint32_t timeON, uint32_t timeOFF, uint8_t EN=1){		//==Генератор импульсов (v32);		
		(void)timeON;	(void)timeOFF;	(void)EN;
		if(id.dir==0) {id.dir=3;}
		if(id.dir==1) {if((micros()-id.time)>= timeON) 			{id.dir=2; 	 return 0;} else {return 1;}}
		if(id.dir==2) {if((micros()-id.time)>=(timeON+timeOFF)) {id.dir=3;}}
		if(id.dir==3) {if(!EN) 		 {return 0;} id.time=micros(); 
					   if(timeON ==0){return 0;} 
					   if(timeOFF==0){return 1;}
					   id.dir=1; 	  return 1;}	
	return 0;};
//=================================================================================================
//							4.4. Генератор PWW(период ms, скважность)
//	Вход: id-рабочая структура, timeON-время включения в ms, timeOFF-время выключения в ms.
//	Выход: возвращаемое значение bool -генерируемый импульс.
//================================================================================================= 			   
	uint8_t generatorPWM  (RT_HW_GENERATOR_PWM_ID   &id, uint32_t period, uint8_t wprc, uint8_t EN=1){ 			//==Генератор PWM;
		(void)period; (void)wprc;  (void)EN;
		if(id.dir==0) {id.dir=3;}
		if(id.dir==1) {if((millis()-id.time)>= id.timeON) 	 {id.dir=2; 	 return 0;} else {return 1;}}
		if(id.dir==2) {if((millis()-id.time)>=(id.timeFULL)) {id.dir=3;}}
		if(id.dir==3) {if(!EN) 		    {return 0;} id.time=millis(); 
					   if(period==0)    {return 0;}			   
					   if(wprc==0) 	    {return 0;} 
					   if(wprc>=100) 	{return 1;} 
					   id.timeFULL=period; 			  
					   id.timeON=(((uint32_t)period*wprc)/100); 
					   id.dir=1; return 1;}	
	return 0;};
//=================================================================================================
//							4.5. Генератор PWW (период mcs, скважность)
//	Вход: id-рабочая структура, timeON-время включения в mсs, timeOFF-время выключения в ms.
//	Выход: возвращаемое значение bool -генерируемый импульс.
//================================================================================================= 			   
	uint8_t generatorPWM_Us  (RT_HW_GENERATOR_PWM_ID   &id, uint32_t period, uint8_t wprc, uint8_t EN=1){ 			//==Генератор PWM;
		(void)period; (void)wprc;  (void)EN;
		if(id.dir==0) {id.dir=3;}
		if(id.dir==1) {if((micros()-id.time)>= id.timeON) 	 {id.dir=2; 	 return 0;} else {return 1;}}
		if(id.dir==2) {if((micros()-id.time)>=(id.timeFULL)) {id.dir=3;}}
		if(id.dir==3) {if(!EN) 		    {return 0;} id.time=micros(); 
					   if(period==0)    {return 0;}			   
					   if(wprc==0) 	    {return 0;} 
					   if(wprc>=100) 	{return 1;} 
					   id.timeFULL=period; 			  
					   id.timeON=(((uint32_t)period*wprc)/100); 
					   id.dir=1; return 1;}		
	return 0;};
//=================================================================================================
//							4.6. Генератор (пачки)пачек импульсов
//================================================================================================= 
	uint8_t generatorGroup(RT_HW_GENERATOR_GROUP_ID &id, uint32_t period, uint32_t periodFULL, uint8_t qnt, uint8_t EN){	
		(void)period; (void)periodFULL;  (void)qnt; (void)EN;
		if(id.dir==0) {id.ago=0; id.dir++;}
		if(id.dir==1) {id.meander=id.cnt=0;
					   if(periodFULL==0) {id.dir=2;} else   {id.time=millis(); id.dir=3;}}
		if(id.dir==2) {if(generatorChangeFrontR(id.ago,EN)) {id.time=millis(); id.dir=4;}} //--Обработка задержки
		if(id.dir==3) {if(getPastMs(id.time)>periodFULL)    {id.dir=4;}} //     по времени при EN=1;
		if(id.dir==4) {if(id.cnt>=qnt) {id.dir=1; id.meander=0;} else {id.dir++;}}
		if(id.dir==5) {id.time=millis(); id.meander=1; id.dir++;}
		if(id.dir==6) {if(getPastMs(id.time)>=period) {id.meander=0; id.time=millis(); id.dir++; }}
		if(id.dir==7) {if(getPastMs(id.time)>=period) {++id.cnt; id.dir=4;}}
	return id.meander;};

//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//									5. ГЕНЕРАТОРЫ НОМЕРОВ ЗАДАЧ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							5.1.Генератор задач автономный
//================================================================================================= 
	uint8_t generatorTask (uint8_t &cnt, uint8_t qnt, uint8_t EN=1){ 											
		(void)cnt; (void)qnt;  (void)EN;
		if(EN){if(++cnt > qnt) {cnt=1;} return cnt;} else {return 0;}};

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//									6. ГЕНЕРАТОРЫ СЛУЧАЙНЫХ ЧИСЕЛ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						   	6.1. Генератор случайных чисел
// depth - разрядность генерируемого числа (8,16,24,32)
//=================================================================================================
  uint32_t genRandom(uint8_t depth=32){
  uint32_t bf32=board.base;
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_CORE_CODE)
	bf32+=RT_HW_CORE_CODE;
#endif   
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_BOARD_CODE)
	bf32+=RT_HW_BOARD_CODE;
#endif     
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_BOARD_PIN_LED)
	bf32+=RT_HW_BOARD_PIN_LED;
#endif
//-------------------------------------------------------------------------------------------------
#if defined(RT_HW_BOARD_PIN_BUTTON)
	bf32+=RT_HW_BOARD_PIN_BUTTON;
#endif           
//-------------------------------------------------------------------------------------------------
  bf32+=micros();;					
  bf32+=unixID.timeSTART;	//--Здесь могут быть дополнительные компоненты (например температура процессора);
//-------------------------------------------------------------------------------------------------
  bf32=bf32*1103515245 + 12345678;  					//--Линейный метод;
  if((depth!=8) && (depth!=16) && (depth!=24) && (depth!=32)) {depth=8;}	//--Нормализация разрядности depth;
  if(depth ==8){bf32=((bf32>>24) % 0xFF);}    			//--Вернуть значение в диапазоне 0-255;
  if(depth==16){bf32=((bf32>>16) % 0xFFFF);}   			//--Вернуть значение в диапазоне 0-65535;
  if(depth==24){bf32=((bf32>> 8) % 0xFFFFFF);}			//--Вернуть значение в диапазоне 0-16777215;
  if(depth==32){;}     									//--Вернуть значение в диапазоне 0-4294967295;
//-------------------------------------------------------------------------------------------------
  board.base+=(uint8_t)((bf32>>24)&0xFF); 
  board.base+=(uint8_t)((bf32>>16)&0xFF);
  board.base+=(uint8_t)((bf32>>8)&0xFF);
  board.base+=(uint8_t)((bf32)&0xFF);
  if(board.base==0){board.base=87;}
//-------------------------------------------------------------------------------------------------
  return   bf32;										//--Выход; 
};
//=================================================================================================


//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//												УСТАРЕВШИЕ ФУНКЦИИ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							2.0.Генератор события по изменению фронта EN [F,C,B,H,D,N]
//										УСТАРЕВШАЯ ФУНКЦИЯ !!!
//	Вход: ago-рабочий регистр, EN-внешний вход, mode-режим работы.
//	Выход: возвращаемое значение bool;
//=================================================================================================
bool generatorChangeEN(uint8_t &ago, char mode, uint8_t EN=1){	
	(void)ago;	(void)mode; (void)EN;
if(mode=='F') {if(ago!=EN) {ago=EN; if( EN) {return 1;}}}	//--Выделение заднего фронта;	
if(mode=='C') {if(ago!=EN) {ago=EN;          return 1;}}	//--Выделение изменения сигнала;	
if(mode=='R') {if(ago!=EN) {ago=EN; if(!EN) {return 1;}}}	//--Выделение переднего фронта;	
if(mode=='H') {								 return EN;}    //--По высокому уровню;
if(mode=='L') {								 return (!EN);} //--По низкому уровню;                
return 0;
};
//=================================================================================================
//							1.2.Генератор событий ms
//	Вход: id-рабочая структура, EN-внешний вход, period-периодичность.
//	Выход: возвращаемое значение bool=id.event;
//		   id.event-событие; id.meander-меандр.
//=================================================================================================
bool generatorEvent(RT_HW_GENERATOR_EVENT_ID &id, uint32_t period, uint8_t EN=1){
	(void)period; (void)EN;
id.event=0;
if(id.dir==0) {id.time=millis(); id.event=id.meander=1; id.dir++;}
if(id.dir==1) {if(EN) {id.dir++;} else {id.event=id.meander=0; id.time=millis();}}
if(id.dir==2) {if(periodMs(id.time,period)) {id.event=1; if(EN) {id.meander=!id.meander;} else {id.meander=0;} id.dir=1;}} 
return id.event;
}; 
//=================================================================================================