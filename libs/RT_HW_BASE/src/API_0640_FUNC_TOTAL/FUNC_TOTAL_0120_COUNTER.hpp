
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//	
//						0. СТРУКТУРЫ генераторов, задержек времени, генераторов фронтов
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						           0.1.Структуры счетчиков
//=================================================================================================
//  struct RT_HW_COUNTER_ID  {uint8_t reset=1; uint8_t direct=0; int16_t begin=0, step=1, top=100, bottom=0, cnt=0; int32_t var;}; 
//  struct RT_HW_COUNTER32_ID{uint8_t reset=1; uint8_t direct=0; int32_t begin=0, step=1, top=100, bottom=0, cnt=0; int32_t var;}; 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										1.СЧЕТЧИКИ int16
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							 		1.1. УСТАНОВКА ПАРАМЕТРОВ
//=================================================================================================		
  void counterSetParam(RT_HW_COUNTER_ID &id,int16_t top, int16_t bottom, int16_t step, int16_t begin){
		id.begin=begin; 	id.step=step; 	id.top=top; 
		id.bottom=bottom; 	
		id.begin=begin;		
		id.direct=0;  		
		id.var=id.cnt=id.begin;
		id.reset=0;
  };
//=================================================================================================
//							 		1.2. УСТАНОВКА ПАРАМЕТРОВ
//=================================================================================================		
  void counterSetParam(RT_HW_COUNTER_ID &id,int16_t top, int16_t bottom, int16_t step){
	int16_t begin;
	if(id.step==0)		{begin=0;}
	if(id.step>0)		{begin=id.bottom;}
	if(id.step<0)		{begin=id.top;}	
	counterSetParam(id,top,bottom,step,begin);	
  };  
//=================================================================================================
//							 		2.1. УСТАНОВКА ПАРАМЕТРОВ
//=================================================================================================		
  void counterSetParam(RT_HW_COUNTER32_ID &id,int16_t top, int16_t bottom, int16_t step, int16_t begin){
		id.begin=begin; 	id.step=step; 	id.top=top; 
		id.bottom=bottom; 	
		id.begin=begin;		
		id.direct=0;  		
		id.var=id.cnt=id.begin;
		id.reset=0;
  };
//=================================================================================================
//							 		2.1. УСТАНОВКА ПАРАМЕТРОВ
//=================================================================================================		
  void counterSetParam(RT_HW_COUNTER32_ID &id,int16_t top, int16_t bottom, int16_t step){
	int16_t begin;
	if(id.step==0)		{begin=0;}
	if(id.step>0)		{begin=id.bottom;}
	if(id.step<0)		{begin=id.top;}	
	counterSetParam(id,top,bottom,step,begin);	
  };   



//=================================================================================================
//	(УСТАРЕВШАЯ !!!)				1.1.2. СБРОС СЧЕТЧИКА С ПАРАМЕТРАМИ ПО УМОЛЧАНИЮ (УСТАРЕВШАЯ !!!)
//=================================================================================================
  void counterReset(RT_HW_COUNTER_ID &id){
		id.step=1; id.top=100; id.bottom=0; id.begin=0;          
		id.cnt=id.begin; id.direct=0; id.var=0; id.reset=0;}
//=================================================================================================
//		(УСТАРЕВШАЯ !!!)			1.1.3. СБРОС СЧЕТЧИКА С ПАРАМЕТРАМИ 
//=================================================================================================
  void counterReset(RT_HW_COUNTER_ID &id,int16_t top, int16_t bottom, int16_t step, int16_t begin=0){
		id.begin=begin; id.step=step; id.top=top; id.bottom=bottom; id.begin=begin;
		id.cnt=id.begin; id.direct=0;  id.var=0; id.reset=0;}
	
//=================================================================================================
//									1.3.СЧЕТЧИК ПИЛА
//=================================================================================================
  int16_t counterS(RT_HW_COUNTER_ID &id, uint8_t EN=1){
//-------------------------------------------------------------------------------------------------
	if(EN){
		if(!id.first){id.first=0; return id.cnt;}
		if( id.step>=0){if(id.cnt==id.top)   {id.var=id.bottom;} 
						else                 {id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}}
		else           {if(id.cnt==id.bottom){id.cnt=id.top;} 
						else                 {id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
		id.cnt=(int16_t)id.var;
	}
//------------------------------------------------------------------------------------------------- 
	return id.cnt;
  };				
//=================================================================================================
//									1.4.СЧЕТЧИК ТРЕУГОЛЬНИК
//=================================================================================================
  int16_t counterT(RT_HW_COUNTER_ID &id, uint8_t EN=1){
	if(EN){
	if(!id.first){id.first=0; return id.cnt;}
//-------------------------------------------------------------------------------------------------
	if(id.step>=0) {if(id.cnt==id.top)   {id.direct=1;} 
					if(id.cnt==id.bottom){id.direct=0;}
//-------------------------------------------------------------------------------------------------
			   if(id.direct==0){id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}
               else            {id.var-=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
//-------------------------------------------------------------------------------------------------
else          {if(id.cnt==id.top)   {id.direct=0;} 
               if(id.cnt==id.bottom){id.direct=1;}
//-------------------------------------------------------------------------------------------------
			   if(id.direct==0){id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
			   if(id.direct==1){id.var-=id.step; if(id.var>id.top)    {id.var=id.top;}}}
id.cnt=(int16_t)id.var;} 
return id.cnt;};
//=================================================================================================
//									1.5.СЧЕТЧИК ЛИМИТ
//=================================================================================================
int16_t counterL(RT_HW_COUNTER_ID &id, uint8_t EN=1) {
	if(EN){
		if(!id.first){id.first=0; return id.cnt;}	
if(id.step>=0) {if(id.cnt<id.top)   {id.var=id.cnt; id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}}
else           {if(id.cnt>id.bottom){id.var=id.cnt; id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
id.cnt=(int16_t)id.var;} 
return id.cnt;};
//=================================================================================================
//									1.6. СЧЕТЧИК РЕВЕРСИВНЫЙ
//=================================================================================================
int16_t counterR(RT_HW_COUNTER_ID &id, uint8_t ENP=1, uint8_t ENM=0){
if(ENP){
		if(!id.first){id.first=0; return id.cnt;}
if(id.step>=0) {id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}
else           {id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
id.cnt=(int16_t)id.var; return id.cnt;};
if(ENM){
if(id.step>=0) {id.var-=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
else           {id.var-=id.step; if(id.var>id.top)    {id.var=id.top; }}}
id.cnt=(int16_t)id.var; 
	return id.cnt;
  };
//=================================================================================================



//=================================================================================================
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										2.СЧЕТЧИКИ int32
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							 		2.1.СБРОС СЧЕТЧИКА С ПАРАМЕТРАМИ ПО УМОЛЧАНИЮ
//=================================================================================================
void counterReset(RT_HW_COUNTER32_ID &id){
id.step=1; id.top=100; id.bottom=0; id.begin=0;          
id.cnt=id.begin; id.direct=0; id.var=0; id.reset=0;}
//=================================================================================================
//							 		2.1.СБРОС СЧЕТЧИКА С ПАРАМЕТРАМИ
//=================================================================================================
void counterReset(RT_HW_COUNTER32_ID &id,int16_t top, int16_t bottom, int16_t step, int16_t begin=0){
id.begin=begin; id.step=step; id.top=top; id.bottom=bottom; id.begin=begin;
id.cnt=id.begin; id.direct=0;  id.var=0; id.reset=0;}
//=================================================================================================
//									2.2. СЧЕТЧИК ПИЛА
//=================================================================================================
int32_t counterS(RT_HW_COUNTER32_ID &id, uint8_t EN=1){
if(EN){
	if(!id.first){id.first=0; return id.cnt;}
if( id.step>=0){if(id.cnt==id.top)   {id.var=id.bottom;} 
                else                 {id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}}
else           {if(id.cnt==id.bottom){id.cnt=id.top;} 
                else                 {id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
id.cnt=(int16_t)id.var;} 
return id.cnt;};				
//=================================================================================================
//									2.3. СЧЕТЧИК ТРЕУГОЛЬНИК
//=================================================================================================
int32_t counterT(RT_HW_COUNTER32_ID &id, uint8_t EN=1){
if(EN){
		if(!id.first){id.first=0; return id.cnt;}
//-------------------------------------------------------------------------------------------------
if(id.step>=0){if(id.cnt==id.top)   {id.direct=1;} 
               if(id.cnt==id.bottom){id.direct=0;}
//-------------------------------------------------------------------------------------------------
			   if(id.direct==0){id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}
               else            {id.var-=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
//-------------------------------------------------------------------------------------------------
else          {if(id.cnt==id.top)   {id.direct=0;} 
               if(id.cnt==id.bottom){id.direct=1;}
//-------------------------------------------------------------------------------------------------
			   if(id.direct==0){id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
			   if(id.direct==1){id.var-=id.step; if(id.var>id.top)    {id.var=id.top;}}}
id.cnt=(int16_t)id.var;} 
return id.cnt;};
//=================================================================================================
//									2.4. СЧЕТЧИК ЛИМИТ
//=================================================================================================
  int32_t counterL(RT_HW_COUNTER32_ID &id, uint8_t EN=1) {
	if(EN){
		if(!id.first){id.first=0; return id.cnt;}
		
if(id.step>=0) {if(id.cnt<id.top)   {id.var=id.cnt; id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}}
else           {if(id.cnt>id.bottom){id.var=id.cnt; id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}}
id.cnt=(int16_t)id.var;} 
	return id.cnt;
	};
//=================================================================================================
//									2.5. СЧЕТЧИК РЕВЕРСИВНЫЙ
//=================================================================================================
  int32_t counterR(RT_HW_COUNTER32_ID &id, uint8_t ENP=1, uint8_t ENM=0){
	if(ENP){
		if(!id.first){id.first=0; return id.cnt;}
if(id.step>=0) {id.var+=id.step; if(id.var>id.top)    {id.var=id.top;}}
else           {id.var+=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
id.cnt=(int16_t)id.var; return id.cnt;};
if(ENM){
if(id.step>=0) {id.var-=id.step; if(id.var<id.bottom) {id.var=id.bottom;}}
else           {id.var-=id.step; if(id.var>id.top)    {id.var=id.top; }}}
id.cnt=(int16_t)id.var; 
	return id.cnt;
  };
//=================================================================================================
