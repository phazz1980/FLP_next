//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											I. УСТАНОВКА СУФФИКСА 
///+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									1.1. Установка cуффикса
//=================================================================================================

 void  messSetSuff(RT_HW_MESS_ID &id, char suff){
		if(suff=='~'){id.suff='~'; 	id.lenSuff=0; id.lenCR=0;	return;} 
		if(suff=='E'){id.suff=';'; 	id.lenSuff=1; id.lenCR=1;	return;}
		if(suff=='L'){id.suff='~';	id.lenSuff=0; id.lenCR=1;	return;}	
					  id.suff=suff; id.lenSuff=1; id.lenCR=0;
  };   
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//											II. УСТАНОВКА ПЕРЕМЕННЫХ 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									2.1. Установка переменныx v8,v16,v32,float
// mode:I-intx, U-uintx; 
//      H-HEX c добавлением впереди 0x;				h-HEX без добавления впереди 0;
//      B-BIN двоичная впереди младший бит; 		b-BIN впереди старший бит;
//		C-char заглавные буквы;			 			c-char прописные буквы;
//		D-pin  значение uint8_t; 	T-pin  наименование(PA0,D1);	A-pin  наименование+значение uint8_t; 
//		d-addr значение uint8_t; 	t-addr значение HEX;       		a-addr значение HEX/значение uint8_t;
//		M-формат времени с дополнением впереди '0';	m-формат времени с дополнением впереди '0';
//		E-IP адрес из uint32_t;
//      0,1,2,3,4 - кол-во знаков после точки for float,uintx; 
//		U-String заглавные буквы;	L-String прописные буквы;		S-String строка без изменения;
//      
//=================================================================================================
  void 	messSetVar (RT_HW_MESS_ID &id,   uint8_t v, char mode='N'){		//--[mode: U, I, H/h, B/b, O, C/с, D/T/A, d/t/a, M/m, 0, 1, 2, 3];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 				 	{id.str=String((int8_t)v); 			 	 return;}
	    if (mode=='U')                 					{id.str=String(v); 			 			 return;}
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id, v,mode);				 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id, v,mode);				 return;} 				
		if((mode=='C') || (mode=='c')) 				  	{messSetCHAR(id, v,mode);				 return;}
		if((mode=='D') || (mode=='A') || (mode=='T')) 	{messSetPIN (id, v,mode);				 return;}
		if((mode=='d') || (mode=='a') || (mode=='t')) 	{messSetADDR(id, v,mode);				 return;}				
	    if((mode=='M') || (mode=='m'))                  {messSetTIM (id, v,mode);				 return;}
		if (mode=='O') 				  					{messSetBIT (id, v,mode);				 return;} 		
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,    int8_t v, char mode='N'){		//--[mode: U, 0, 1, 2, 3];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 					{id.str=String(v); 			 			 return;}	
	    if (mode=='U')                 					{id.str=String((uint8_t)v); 			 return;}				
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id,(uint8_t)v,mode);		 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id,(uint8_t)v,mode);		 return;} 
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,  uint16_t v, char mode='N'){		//--[mode: U, I, H/h, B/b, 0, 1, 2, 3];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 				 	{id.str=String( (int16_t)v);			 return;}
	    if (mode=='U')                 					{id.str=String(v);						 return;}
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id, v,mode);				 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id, v,mode);				 return;} 				
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,   int16_t v, char mode='N'){		//--[mode: U, 0, 1, 2, 3];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 				 	{id.str=String(  v); 					 return;}
	    if (mode=='U')                 					{id.str=String((uint16_t)v); 			 return;}	
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id, (uint16_t)v,mode);		 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id, (uint16_t)v,mode);		 return;} 			
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,  uint32_t v, char mode='N'){		//--[mode: I,U, H/h, B/b, i, 0, 1, 2, 3, 4];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 				 	{id.str=String((int32_t)v); 			 return;}
	    if (mode=='U')                 				 	{id.str=String(v); 			 			 return;}
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id, v,mode);				 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id, v,mode);				 return;}
		if (mode=='i')             						{messSetIP  (id, v,mode);				 return;}		
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}
		if (mode=='4')                                	{float b=v; b/=10000;id.str=String(b,4); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,   int32_t v, char mode='N'){		//--[mode: I,U, 0, 1, 2, 3,4];
	#if !defined(RT_HW_BASE_COMPACT)
	    if (mode=='I')                 				 	{id.str=String(v); 			 			 return;}
	    if (mode=='U')                 					{id.str=String((uint32_t)v); 	         return;}				
		if((mode=='H') || (mode=='h')) 				  	{messSetHEX (id,(uint32_t)v,mode);		 return;} 
		if((mode=='B') || (mode=='b'))              	{messSetBIN (id,(uint32_t)v,mode);		 return;}
		if (mode=='i')             						{messSetIP  (id,(uint32_t)v,mode);		 return;}	
		if (mode=='0')                                	{float b=v;          id.str=String(b,0); return;}
		if (mode=='1')                                	{float b=v; b/=10;   id.str=String(b,1); return;}
		if (mode=='2')                                	{float b=v; b/=100;  id.str=String(b,2); return;}
		if (mode=='3')                                	{float b=v; b/=1000; id.str=String(b,3); return;}
		if (mode=='4')                                	{float b=v; b/=10000;id.str=String(b,4); return;}		
	#endif	
		id.str=String(v);					
  };
  void 	messSetVar (RT_HW_MESS_ID &id,     float v, char mode='2'){
	    (void)mode;
        if(mode=='0') {id.str=String(v,0); return;}  
		if(mode=='1') {id.str=String(v,1); return;} 
		if(mode=='2') {id.str=String(v,3); return;}
	    if(mode=='3') {id.str=String(v,3); return;}  
		if(mode=='4') {id.str=String(v,4); return;} 
		id.str=String(v,2);
	};
  void 	messSetVar (RT_HW_MESS_ID &id,   String  v, char mode='N'){		//--mode: U,L,N;
	    id.str=v;        
		if(mode=='U') {id.str.toUpperCase();}			//--Форматирование - всё заглавными символами;
		if(mode=='L') {id.str.toLowerCase();}			//--Форматирование - всё строчными  символами; 
  };
//=================================================================================================
//									2.2. Установка float
//=================================================================================================
  void 	messSetFLOAT(RT_HW_MESS_ID &id, float v, char mode='2'){
	    (void)mode;
        if((mode=='0') || (mode==0)){id.str=String(v,0); return;}  
		if((mode=='1') || (mode==1)){id.str=String(v,1); return;} 
		if((mode=='2') || (mode==2)){id.str=String(v,3); return;}
	    if((mode=='3') || (mode==3)){id.str=String(v,3); return;}  
		if((mode=='4') || (mode==4)){id.str=String(v,4); return;} 
		id.str=String(v,2);
	};
//=================================================================================================
//									2.3. Установка HEX, BIN, BIT, PIN,ADDR,TIM,CHAR,STR,IP,BAR
//=================================================================================================
  void 	messSetHEX (RT_HW_MESS_ID &id, uint8_t  v, char mode='H'){	//--mode: H/h H=добавление впереди 0x;
		id.str=String(v,HEX); 
		id.str.toUpperCase(); 										//--Строка с заглавными буквами;
		if(v<=0xF)   {id.str=String("0") +id.str;} 					//--Добавление 0 в старшую позицию;
		if(mode=='H'){id.str=String("0x")+id.str;};					//--Добавление префикса 0x;
  };
  void 	messSetHEX (RT_HW_MESS_ID &id, uint16_t v, char mode='H'){	//--mode: H/h H=добавление впереди 0x;
		id.str=String(v,HEX); 
		id.str.toUpperCase();
		for(uint8_t i=id.str.length(); i<4; i++){id.str=String("0")+id.str;}
		if(mode=='H'){id.str=String("0x")+id.str;};
  };	
  void 	messSetHEX (RT_HW_MESS_ID &id, uint32_t v, char mode='H'){	//--mode: H/h H=добавление впереди 0x;
		id.str=String(v,HEX); 
		id.str.toUpperCase();  
		for(uint8_t i=id.str.length(); i<8; i++){id.str=String("0")+id.str;}
		if(mode=='H'){id.str=String("0x")+id.str;};
  };	
  void 	messSetBIN (RT_HW_MESS_ID &id, uint8_t  v, char mode='B'){	//--mode: B/b B=впереди старший разряд; b=впереди младший разряд;
		id.str=String("");
		if(mode=='B'){v=reverseV8(v);}						//--Реверс переменной;
		for(uint8_t i=0; i<8; i++) {						//--Побитовый вывод;
			if(bitRead(v,7-i)){id.str+=String('+');} 
			else              {id.str+=String('-');}}  
  };
  void 	messSetBIN (RT_HW_MESS_ID &id, uint16_t v, char mode='B'){	//--mode: B/b B=впереди старший разряд; b=впереди младший разряд;
		id.str=String("");
		if(mode=='B'){v=reverseV16(v);}						//--Реверс переменной;
		for(uint8_t i=0; i<16; i++) {						//--Побитовый вывод;
			if(bitRead(v,15-i)){id.str+=String('+');} 
			else               {id.str+=String('-');}}  
  };
//=================================================================================================
  void 	messSetBIN (RT_HW_MESS_ID &id, uint32_t v, char mode='B'){	//--mode: B/b B=впереди старший разряд; b=впереди младший разряд;
		id.str=String("");
		if(mode=='B'){v=reverseV32(v);}					//--Реверс переменной;
		for(uint8_t i=0; i<32; i++) {					//--Вывод qnt bit;
			if(bitRead(v,31-i)){id.str+=String('+');} 
			else               {id.str+=String('-');}
		}									
  };
//=================================================================================================
  void 	messSetBIT (RT_HW_MESS_ID &id, uint8_t  v, char mode='O'){	//--mode: не имеет значения;
		(void)mode; if(v){id.str=String('+');} else {id.str=String('-');}
  };
//=================================================================================================
  void  messSetPIN (RT_HW_MESS_ID &id, uint8_t  v, char mode='T'){ //==mode: T,A,D;
	char alig='R'; uint8_t len=2;
	(void)id; (void)v; (void)mode; (void)len; (void)alig;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_CORE_STM32)	
		if(mode=='T') 	{if(len<5) {len=5;} id.str=getNamePin(v);}
		if(mode=='A') 	{if(len<9) {len=9;} id.str=getNamePin(v); id.str+=String('['); 
								 if(v<254){ id.str+=String(v);}  
								 else     { id.str+=String('*');} id.str+=String(']');
						}
		if((mode!='T') && (mode!='A'))
						{
								 if(v<254){ id.str=String(v);} 
								 else     { id.str=String('*');}
					    }
//-------------------------------------------------------------------------------------------------
	#elif defined(RT_HW_CORE_ESP8266)
		if(mode=='T') 	{if(len<3) {len=3;} id.str=getNamePin(v);}
		if(mode=='A') 	{if(len<7) {len=7;} id.str=getNamePin(v); id.str+=String('['); 
								 if(v<254){ id.str+=String(v);}      
								 else     { id.str+=String('*');} id.str+=String(']');}
		if((mode!='T') && (mode!='A'))
						{
								 if(v<254){id.str=String(v);}     
								 else     {id.str=String('*');}                     
						}   
//-------------------------------------------------------------------------------------------------
	#else
		if(len<2){len=2;}
								 if(v<254){id.str=String(v);} 
								 else	  {id.str= String(' ') +String('*');}													
	#endif
//------------------------Форматирование с учетом размера длины поля пина len----------------------	 
	id.lenVar=id.str.length();						//--Размер строки;
	id.lenBegin=id.lenEnd=id.lenSuff=0;				//--Очистка строки;
	if(id.lenVar>=len){ return;}					//--Ограничение размера;
	if(alig=='C'){id.lenBegin=(len-id.lenVar)/2;} 	//--Кол-во передних заполнителей - по центру;
	if(alig=='R'){id.lenBegin= len-id.lenVar;}		//--Кол-во передних заполнителей - форматирование=вправо; 
	id.lenEnd=len-id.lenBegin-id.lenVar;			//--Кол-во задних   заполнителей;
};
//=================================================================================================
  void  messSetADDR(RT_HW_MESS_ID &id, uint8_t  v, char mode='t'){	//--mode: t,a,d;
	if((mode=='a') || (mode=='t')){
		id.str=String("0x");
		if(v<0x0F) {id.str+=String('0');} 
		id.str+=String(v,HEX); 
		if(mode=='a') {id.str+=String('/'); id.str+=String(v);}  
    } 
	else {id.str=String(v);};	    
  };  
  void  messSetTIM (RT_HW_MESS_ID &id, uint8_t  v, char mode='M'){	//--mode: M/m'
		if(v>99){v=99;}
		id.str=String(v);		
		if(v<10) {if(mode=='M'){id.str=String('0')+ String(v);}  
				  if(mode=='m'){id.str=String(' ')+ String(v);}}
  }; 
  void 	messSetCHAR(RT_HW_MESS_ID &id, uint8_t  v, char mode='C'){	//--mode: C/c С- заглавная; с-прописная;
	    id.str=String((char)v);
		if(mode=='C'){id.str.toUpperCase(); return;}			//--Форматирование - заглавный символ;
		if(mode=='c'){id.str.toLowerCase(); return;}			//--Форматирование - строчный символ;
		
  };
  void 	messSetSTR (RT_HW_MESS_ID &id, String   v, char mode='N'){	//--mode: U,L,N;
	    id.str=v;        
		if(mode=='U') {id.str.toUpperCase();}			//--Форматирование - всё заглавными символами;
		if(mode=='L') {id.str.toLowerCase();}			//--Форматирование - всё строчными  символами; 
  };
  void 	messSetIP  (RT_HW_MESS_ID &id, uint32_t v, char mode='N'){	//	
	    (void)mode;
		id.str=String((v)&0xFF) + '.' + String((v>> 8)&0xFF) + '.' + String((v>>16)&0xFF) + '.' +String((v>>24)&0xFF);
 };	

//=================================================================================================
//									2.4. Установка \линейного индикатора;
//=================================================================================================
  void 	messSetBAR (RT_HW_MESS_ID &id, uint8_t  v, uint8_t len=15, char sign='-'){ 	
		id.str="";
		//-----Расчет длины линии------------------------------------------------------------------
		if(v>100){v=100;} 
		len=((uint16_t)v*len)/100;
		if((v>0) && (len==0)){len=1;}
		//-----Формирование строки----------------------------------------------------------------
		for(uint8_t i=0; i<len; i++){id.str+=String(sign);}  
  }; 
	
//=================================================================================================
//									2.5. Время из uint32_t формата UNIX
// mode: '0'-hh:mm:ss, '1'-hh:mm:ss week;
//=================================================================================================
  void  messSetTIME (RT_HW_MESS_ID &id, uint32_t v, uint16_t zone, char mode='0'){
		RT_HW_STRUCT_UNIX_ID ux;		
		ux.timeUNIX=v;
		ux.zone=zone;
		unixSetTime(ux); //-Фиксация времени;	
		id.str= messGetFormatTime(ux.hours)   + String(':') + 	//--Часы;
				messGetFormatTime(ux.minutes) + String(':') + 	//--Минуты;
				messGetFormatTime(ux.seconds); 					//--Секунды;				
		if(mode=='1'){
			unixSetDate(ux); //-Фиксация даты;	
			id.str+=String(' ')+String(unixID.weekDay);
		}
  };
//=================================================================================================
//									2.6. Дата из uint32_t формата UNIX
// mode: '0'-dd.mm.yy, '1'-yy.mm.dd, '2'-dd.mm.yyyy,  '3'-yyyy.mm.dd
//=================================================================================================  
  void  messSetDATE (RT_HW_MESS_ID &id, uint32_t v, uint16_t zone, char mode='0'){
		RT_HW_STRUCT_UNIX_ID ux;		
		ux.timeUNIX=v;
		ux.zone=zone;
		unixSetDate(ux);	//--Фиксация даты;
		char md='0';
	id.str="";
	if( mode=='2')					{mode='0'; md='1';}
	if( mode=='3')					{mode='1'; md='1';}	
	if((mode!='0') && (mode!='1')) 	{mode='0';}	
//-------------------------------------------------------------------------------------------------
	if(mode=='0'){	id.str= messGetFormatTime(ux.day) 		+ String('.') + //--День; 
							messGetFormatTime(ux.month)		+ String('.') + //--Месяц;  
							messGetFormatYear(ux.year,md);				  	//--Год;
					 return;
					}					
//-------------------------------------------------------------------------------------------------
	if(mode=='1')	{id.str=messGetFormatYear(ux.year,md) 	+ String('.') + //--Год; 
							messGetFormatTime(ux.month)		+ String('.') +	//--Месяц;  
							messGetFormatTime(ux.day);				  		//--День;	
					 return;
					}
//-------------------------------------------------------------------------------------------------					
  };
 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//									III. ФОРМАТИРОВАНИЕ  СООБЩЕНИЙ 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//									3.1. Нормализация сообщениий ускоренная (для переменных)
// Выход: строка+суффикс(str), количество заполнителей(id.lenBegin,end), перевод строки(lenCR)
// Суффикс(suff): '~'=No, L=перевод строки, E=';'+перевод строки;	
//=================================================================================================
  void  messFormat  (RT_HW_MESS_ID &id, uint8_t size=0, char suff='~', char alig='~'){
	(void)id; (void)size; (void)alig;
	id.lenBegin=id.lenVar=id.lenEnd=0;							//--Очистка параметров;    
	messSetSuff(id, suff);										//--Установка суфикса; 
	if(id.lenSuff){id.str+=String(id.suff); id.lenSuff=0;}		//--Добавление суффикса к строке;
	id.lenVar=id.str.length();									//--Определение длины строки(вместе с суффиксом);
//-------------------------------------------------------------------------------------------------
	if(((alig=='L') || (alig=='C') || (alig=='R')) && (size>0)){//--Проверка условий форматирования сообщения;					
		if(id.lenVar>size){id.lenVar=size; id.str.remove(size);}//--Усечение ссобщения сзади до заданной длины size; 
		if(alig=='C'){id.lenBegin=(size-id.lenVar+1)/2; 		//--C:: кол-во передних заполнителей;
		              id.lenEnd  = size-id.lenBegin-id.lenVar;} //--C:: кол-во   задних заполнителей;
		if(alig=='R'){id.lenBegin= size-id.lenVar;}				//--R:: кол-во передних заполнителей; 
		if(alig=='L'){id.lenEnd  = size-id.lenVar;}				//--L:: кол-во   задних заполнителей;
    }
//--------------------------------------------------------------------------------------------------mode    
  }; 
//=================================================================================================
//									3.1. Нормализация сообщениий ускоренная (для переменных)
// Выход: строка+суффикс(str), количество заполнителей(id.lenBegin,end), перевод строки(lenCR)
// Суффикс(suff): '~'=No, L=перевод строки, E=';'+перевод строки;	
//=================================================================================================
  void  messFormatFull(RT_HW_MESS_ID &id, uint8_t size=0, char suff='~', char alig='~'){
	(void)id; (void)size; (void)alig;
	id.lenBegin=id.lenVar=id.lenEnd=0;							//--Очистка параметров;    
	messSetSuff(id, suff);										//--Установка суфикса; 
	if(id.lenSuff){id.str+=String(id.suff); id.lenSuff=0;}		//--Добавление суффикса к строке;
	id.lenVar=id.str.length();									//--Определение длины строки(вместе с суффиксом);
//-------------------------------------------------------------------------------------------------
	if(((alig=='L') || (alig=='C') || (alig=='R')) && (size>0)){//--Проверка условий форматирования сообщения;					
		if(id.lenVar>size){id.lenVar=size; id.str.remove(size);}//--Усечение ссобщения сзади до заданной длины size; 
		if(alig=='C'){id.lenBegin=(size-id.lenVar+1)/2; 		//--C:: кол-во передних заполнителей;
		              id.lenEnd  = size-id.lenBegin-id.lenVar;} //--C:: кол-во   задних заполнителей;
		if(alig=='R'){id.lenBegin= size-id.lenVar;}				//--R:: кол-во передних заполнителей; 
		if(alig=='L'){id.lenEnd  = size-id.lenVar;}				//--L:: кол-во   задних заполнителей;
    }
	for(uint8_t i=0; i<id.lenBegin; i++){id.str=String(' ') + id.str;}
	for(uint8_t i=0; i<id.lenEnd;   i++){id.str=id.str + String(' ');}
//--------------------------------------------------------------------------------------------------   
  };  
  
//=================================================================================================
//									3.2. Нормализация параметров для строковых переменных с учетом UTF-8
//                                (количество заполнителей, суффикс, перевод строки)
// Суффикс(suff): '~'=No, G=пробел, L=перевод строки, E=';'+перевод строки, S=';', V=',' или иной символ;	
//=================================================================================================
  void messFormatSTR(RT_HW_MESS_ID &id, uint8_t size=0, char suff='~', char alig='~', char mode='~'){
	String outStr;
	uint8_t bf, i;  
	id.lenBegin=id.lenEnd=id.lenVar=0;							//--Сброс длины полей;
    messSetSuff(id, suff);										//--Установка суфикса;
    if(mode=='U') {id.str.toUpperCase();}						//--Форматирование - всё заглавными символами;
    if(mode=='L') {id.str.toLowerCase();}						//--Форматирование - всё строчными  символами; 
	if(id.lenSuff){id.str+=String(id.suff); id.lenSuff=0;}		//--Добавление суффикса к строке;	
//---------------------------------------------------------------------------------------------------
	const char * data=id.str.c_str();							//--Установка указателя на начало строки;
	if(size>80){size=80;}										//--Ограничение размера сообщения;
	for(i=0; i<=160; i++){										//--Последовательный перебор  символов в строке;
		bf=data[i];  											//--Сохранение в буфере текущего символа;								
		if((bf=='\0') ||(bf==RT_HW_MESS_BREAK)){break;}			//--Выход если символ конец строки(\0) или символ RT_HS_MESS_BREAK(~);								
		if( bf <0xC0) {++id.lenVar;} 							//--Увеличение количества символов;
		if(size>0)    {outStr+=(char)bf;}						//--Добавление в буфер очередного символа;	
		//               outStr+=(char)bf;						//--Добавление в буфер очередного символа;	
		if(id.lenVar>=size) 				   {break;}			//--Выход по размеру сообщения;
	}															//--Конец цикла      перебора символов в строке;
	id.lenByte=i;												//--Сохранение размера сообщения в байтах;
//-----------------------------------------------------------------------------------------------------
	if(size>0){id.str=outStr;}									//--Сохранение нового значения текста;
	//           id.str=outStr; 								//--Сохранение нового значения текста;
	id.lenByte=i;												//--Сохранение размера сообщения в байтах;	
//-------------------------------------------------------------------------------------------------
	if(((alig=='L') || (alig=='C') || (alig=='R')) && (size>0)){//--Проверка условий форматирования сообщения;					
		if(id.lenVar>size){id.lenVar=size; id.str.remove(size);}//--Усечение ссобщения сзади до заданной длины size; 
		if(alig=='C'){id.lenBegin=(size-id.lenVar+1)/2; 		//--C:: кол-во передних заполнителей;
		              id.lenEnd  = size-id.lenBegin-id.lenVar;} //--C:: кол-во   задних заполнителей;
		if(alig=='R'){id.lenBegin= size-id.lenVar;}				//--R:: кол-во передних заполнителей; 
		if(alig=='L'){id.lenEnd  = size-id.lenVar;}				//--L:: кол-во   задних заполнителей;
    }  
//-------------------------------------------------------------------------------------------------  
};
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										IV. Утилиты 
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//								4.1. Форматирование переменной в формате времени [00] 
//											(вспомогательная функция)
//=================================================================================================
  String messGetFormatTime(uint8_t vr){if(vr<10) {return String('0')+ String(vr);}  else {return String(vr);}};
//=================================================================================================
//								4.2. Форматирование переменной в формате года [00]   
//											(вспомогательная функция)
//  mode: '1' - впереди добавляется "20";
//=================================================================================================
  String messGetFormatYear(uint8_t vr, char mode='0'){
	if(mode=='1'){if(vr<10) {return String("200")+ String(vr);}  else {return String("20")+String(vr);}}
				  if(vr<10) {return String('0')  + String(vr);}  else {return String(vr);}
  };
//=================================================================================================
//								4.3. Формирование строки с IP Ethernet
//=================================================================================================
  String messEthernetIPtoMess(uint8_t e3, uint8_t e2, uint8_t e1, uint8_t e0){
	return (String(e0) + '.' + String(e1) + '.' + String(e2) + '.' + String(e3));
  };
//=================================================================================================

