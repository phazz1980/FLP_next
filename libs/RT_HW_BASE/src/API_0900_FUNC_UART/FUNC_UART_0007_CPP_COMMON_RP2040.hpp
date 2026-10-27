#include "RT_HW_BASE.h"	 
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//											ОБЪЕКТЫ UART RP2040
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 //=================================================================================================
#if !defined(RT_HW_CREATE_UART_CODE) && defined(RT_HW_CORE_RP2040)
	#define  RT_HW_CREATE_UART_CODE 			
//=================================================================================================
// 											1. Флаги UART
//=================================================================================================
	RT_HW_STRUCT_UART_RXTX 	uart;
	RT_HW_STRUCT_UART_USB 	uartUSB;
//=================================================================================================
// 											2. Объекты UART
//=================================================================================================
	#if defined(RT_HW_PERMIT_UART0)
       #define RT_HW_PRESENT_UART0    
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_UART1)
	   #define RT_HW_PRESENT_UART1
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_UART2)
	   #define RT_HW_PRESENT_UART2  
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_UART3)
	#if defined(RT_HW_CREATE_UART3)
       #define RT_HW_PRESENT_UART3  
		SerialPIO  		   Serial3((uint8_t)RT_HW_UART3_TX, (uint8_t)RT_HW_UART3_RX,128);
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
    #if defined(RT_HW_PERMIT_UART4)
	#if defined(RT_HW_CREATE_UART4)
	   #define RT_HW_PRESENT_UART4 
		SerialPIO  		   Serial4((uint8_t)RT_HW_UART4_TX, (uint8_t)RT_HW_UART4_RX,128);
	#endif  
    #endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_UART5)
	#if defined(RT_HW_CREATE_UART5)	
       #define RT_HW_PRESENT_UART5 
		SerialPIO  		   Serial5((uint8_t)RT_HW_UART5_TX, (uint8_t)RT_HW_UART5_RX,128);
	#endif
    #endif
//-------------------------------------------------------------------------------------------------
    #if defined(RT_HW_PERMIT_UART6)
	#if defined(RT_HW_CREATE_UART6)
       #define RT_HW_PRESENT_UART6
		SerialPIO  		   Serial6((uint8_t)RT_HW_UART6_TX, (uint8_t)RT_HW_UART6_RX,128);
    #endif  	
    #endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_UART200)		//--(для развития);
	#if defined(RT_HW_CREATE_UART200)
    #endif 
    #endif
//-------------------------------------------------------------------------------------------------
//=================================================================================================	

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										1. ФУНКЦИИ НАСТРОЙКИ UART
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						        		1.1. Инициализация UARTn как MASTER
//=================================================================================================	
  uint8_t	RT_HW_BASE::  uartBegin(uint8_t num){
	(void)num;
//-------------------------------------------------------------------------------------------------
	uartSetCodeErr(0,num);											//--Сброс кода ошибки;
	if(!uartCheckNum(num))	{uartSetCodeErr(65,num);	return 1;}	//--Выход, если недопустимый номер UART;
	if( uartGetStatus(num)==1){						 	return 1;} 	//--Выход, если UARTn уже настроен как Master;
    uartEnd(num);													//--Отключение UART (если вдруг раньше была инициализирована функцией Serialx.begin();
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)	                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  
		if(num==0){
			if(uartGetStatus(num)==0){
				Serial.ignoreFlowControl(1);			//--Выполняет запись в порт, даже если данные могут быть потеряны;
				Serial.begin(uartGetSpeed(num));  
				uartSetStatus(1,num);
			}
		return 1;
		}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(num==1){
			if(uartGetStatus(num)==0){
				Serial1.setFIFOSize(128);				//--Установка размера буфера FIFO;
				Serial1.begin(uartGetSpeed(num));  
				uartSetStatus(1,num);
				}
		return 1;
		}
	#endif	
//-------------------------------------------------------------------------------------------------	
	#if defined(RT_HW_PRESENT_UART2)
		if(num==2)    {if(LinkUART2.status==0)  {Serial2.begin(LinkUART2.speed); 	LinkUART2.status=1;} 	return 1;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if	defined(RT_HW_PRESENT_UART3)
		if(num==3)    {if(LinkUART3.status==0)  {Serial3.begin(LinkUART3.speed); 	LinkUART3.status=1;} 	return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if	defined(RT_HW_PRESENT_UART4)
		if(num==4)    {if(LinkUART4.status==0)  {Serial4.begin(LinkUART4.speed); 	LinkUART4.status=1;} 	return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if	defined(RT_HW_PRESENT_UART5)
		if(num==5)    {if(LinkUART5.status==0)  {Serial5.begin(LinkUART5.speed);  	LinkUART5.status=1;} 	return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if	defined(RT_HW_PRESENT_UART6)
		if(num==6)    {if(LinkUART6.status==0)  {Serial6.begin(LinkUART6.speed); 	LinkUART6.status=1;} 	return 1;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)	                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  
		if(num==200)  {if(LinkUART200.status==0){Serial200.begin(); 				LinkUART200.status=1;} 	return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
		return 0;
  };	//====END	uint8_t  RT_HW_BASE:: uartBegin(uint8_t num)
//=================================================================================================
// 										1.2. Отключение UARTn
//=================================================================================================
  void	  	RT_HW_BASE::	uartEnd(uint8_t num){
	(void)num;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)		
		if(num==0)    {if(LinkUART0.status==1)  {Serial.end();		LinkUART0.status=0;		return;}}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(num==1)    {if(LinkUART1.status==1)  {Serial1.end();		LinkUART1.status=0;		return;}}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(num==2)    {if(LinkUART2.status==1)  {Serial2.end();		LinkUART2.status=0;		return;}}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(num==3)    {if(LinkUART3.status==1)  {Serial3.end();		LinkUART3.status=0;		return;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(num==4)    {if(LinkUART4.status==1)  {Serial4.end();		LinkUART4.status=0;		return;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(num==5)    {if(LinkUART5.status==1)  {Serial5.end();		LinkUART5.status=0;		return;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)		
		if(num==6)    {if(LinkUART6.status==1)  {Serial6.end();		LinkUART6.status=0;		return;}}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(num==200)  {if(LinkUART200.status==1){					LinkUART200.status=0;	return;}}
	#endif	
//==================================================================================================
  };	//===END void RT_HW_BASE:: uartEnd(uint8_t num)			
//=================================================================================================
// 										1.3. Проверка подключения UART
//=================================================================================================
  uint8_t 	RT_HW_BASE:: 	uartSerial(uint8_t num){
	(void)num;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(num==0){
			if(uartUSB.dir==3) {return 1;}
			if(uartUSB.dir==0) {if(++uartUSB.cnt<50)			{return 0;} 
								uartUSB.cnt=0; uartUSB.timeBegin=millis()+100;
								if(!Serial)         			{return 0;}
								uartUSB.dir=2;
								}
			if(uartUSB.dir==1) {if(++uartUSB.cnt<50)			{return 0;} 
								uartUSB.cnt=0;					
								if(millis()<uartUSB.timeBegin)	{return 0;}
								if(!Serial){uartUSB.dir=0; 		 return 0;}					 				
								uartUSB.dir=2;					 		  }
			if(uartUSB.dir==2) {uartUSB.dir=3;					 return 1;}				
		}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)	
		if(num==1)  	{if(Serial1) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)	
		if(num==2)  	{if(Serial2) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)	
		if(num==3)  	{if(Serial3) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)	
		if(num==4)  	{if(Serial4) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)	
		if(num==5)  	{if(Serial5) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)	
		if(num==6)  	{if(Serial6) 	{bitSet(uart.ready,num); 	return 1;} else {return 0;}}
	#endif	
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)	
		if(num==200)	{return 1;}
	#endif		
//-------------------------------------------------------------------------------------------------
    return 0;
  };	//====END uint8_t	RT_HW_BASE:: 	uartSerial(uint8_t num)
//=================================================================================================
//										1.4. Установка пинов RX,TX в объект Linkn
//=================================================================================================
  void     	RT_HW_BASE:: 	uartSetPins(uint8_t rx, uint8_t tx, uint8_t num){
	(void)rx; (void)tx; (void)num;
//=================================================================================================
	#if defined(RT_HW_PRESENT_UART0)	  
		if(num==0){                 LinkUART0.rx=47; LinkUART0.tx=46; return;}  //----UART0->USB: pin chip 47,46
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(num==1){		//----UART1 (rx,tx): 1,0; 13,12; 17,16; 29,28;
			if((rx==1) && (tx==0)) {LinkUART1.rx=rx; LinkUART1.tx=tx; return;} 
			if((rx==13)&& (tx==12)){LinkUART1.rx=rx; LinkUART1.tx=tx; return;} 
			if((rx==17)&& (tx==16)){LinkUART1.rx=rx; LinkUART1.tx=tx; return;}
			if((rx==29)&& (tx==28)){LinkUART1.rx=rx; LinkUART1.tx=tx; return;}
			return;
		}
    #endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2) 
		if(num==2){			//----UART2 (rx,tx): 5,4; 9,8; 21,20; 25,24;
			if((rx==5) && (tx==4)) {LinkUART2.rx=rx; LinkUART2.tx=tx; return;} 
			if((rx==9) && (tx==8)) {LinkUART2.rx=rx; LinkUART2.tx=tx; return;} 
			if((rx==21)&& (tx==20)){LinkUART2.rx=rx; LinkUART2.tx=tx; return;}
			if((rx==25)&& (tx==24)){LinkUART2.rx=rx; LinkUART2.tx=tx; return;}
			return;
		}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(num==3)		 {LinkUART3.rx=rx; LinkUART3.tx=tx; return;}    //----UART3 (rx,tx): other for PIO; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(num==4)       {LinkUART4.rx=rx; LinkUART4.tx=tx; return;}     //----UART4 (rx,tx): other for PIO;
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(num==5)       {LinkUART5.rx=rx; LinkUART5.tx=tx; return;}     //----UART5 (rx,tx): other for PIO; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(num==6)       {LinkUART6.rx=rx; LinkUART6.tx=tx; return;}     //----UART6 (rx,tx): other for PIO; 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(num==200){;return;}   								 		//----UART200 (rx,tx) (для развития); 
	#endif	
//-------------------------------------------------------------------------------------------------
	uartSetCodeErr(64,num); return;
//-------------------------------------------------------------------------------------------------
};

 //+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										2. ФУНКЦИИ ОБМЕНА С UART
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
// 										2.1. Проверка входного буфера UARTn
//================================================================================================= 
  uint16_t	RT_HW_BASE:: uartAvailable(uint8_t n){
	(void)n;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)  	{return Serial.available();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)    {return Serial1.available();}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)    {return Serial2.available();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)    {return Serial3.available();}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)    {return Serial4.available();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)  	{return Serial5.available();}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)  	{return Serial6.available();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)  {return Serial200.available();}
	#endif		
//-------------------------------------------------------------------------------------------------
	return 	0;	
  };	
///=================================================================================================
// 										2.2. Проверка выходного буфера UARTn
//================================================================================================= 
  uint16_t	RT_HW_BASE:: uartAvailableForWrite(uint8_t n){												
	(void)n;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)    {return Serial.availableForWrite();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)    {return Serial1.availableForWrite();}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)  	{return Serial2.availableForWrite();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)  	{return Serial3.availableForWrite();}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)  	{return Serial4.availableForWrite();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)    {return Serial5.availableForWrite();}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)    {return Serial6.availableForWrite();}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)	{return Serial200.availableForWrite();}		
	#endif		
//-------------------------------------------------------------------------------------------------
	return 	63;
  };
//================================================================================================= 
//================================================================================================= 
// 										2.3. Чтение байта из UARTn
//================================================================================================= 
  uint8_t	RT_HW_BASE:: uartRead(uint8_t n){
	(void)n;
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)  	{return Serial.read();}
	#endif	
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)  	{return Serial1.read();}
	#endif		
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2) 
		if(n==2)  	{return Serial2.read();}
	#endif	
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)  	{return Serial3.read();}
	#endif
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)  	{return Serial4.read();}
	#endif	
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)  	{return Serial5.read();}
	#endif	
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)  	{return Serial6.read();}
	#endif		
//---------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)	{return Serial200.read();}
	#endif
//---------------------------------------------------------------------------------------------
	return 	0;	
  };
//=================================================================================================  
// 										2.4. Запись байта в UARTn
//	Примечание: для UART0.RP2040 реализована "заплатка!!!  (см. также радел 8)
//              Следует отслеживать новые версии CORE RP2040 и со временем отказаться от "заплатки". 
//================================================================================================= 
  uint8_t  RT_HW_BASE:: uartWrite(uint8_t v, uint8_t n){
	(void)v; (void)n;
//--------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)	    	
		if(n==0)           {Serial.write(char(v));	return 1;}	//--Исключение для UART0 RP2040;
	#endif	
//--------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)	       {Serial1.write(v); 		return 1;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)		   {Serial2.write(v); 		return 1;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)		   {Serial3.write(v); 		return 1;}
	#endif
//-------------------------------------------------------------------------------------------------  
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)		  {Serial4.write(v); 		return 1;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)		 {Serial5.write(v); 		return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)		   {Serial6.write(v); 		return 1;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200){return 1;} //SerialB.write(v); return 1;}
	#endif			
//-------------------------------------------------------------------------------------------------
	return 1;
  };
//================================================================================================= 
// 										2.5.1. Вывод flush 
//================================================================================================= 
  void   RT_HW_BASE:: uartFlush(uint8_t n){uartFlush(0, n);};
//================================================================================================= 
// 										2.5.2. Вывод flush 
//=================================================================================================
  void    RT_HW_BASE:: uartFlush(uint32_t timeout, uint8_t n){
	(void)timeout; (void)n;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)		   {Serial.flush();		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)		   {Serial1.flush();	return;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)		   {Serial2.flush();	return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)		   {Serial3.flush();	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)		   {Serial4.flush();	return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)		   {Serial5.flush();	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)		   {Serial6.flush();	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)							   {return;}
	#endif	
//-------------------------------------------------------------------------------------------------
  };
//================================================================================================= 
// 										2.6. Вывод print(String) 
//  Примечание: это временная "заплатка" для совместимости с CORE RP2040 
//              Следует отслеживать новые версии CORE RP2040 и со временем отказаться от "заплатки". 
//================================================================================================== 
  void	RT_HW_BASE:: uartPrint( String str,uint8_t n){
	(void)str; (void)n;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)           {Serial.print(str); 		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)  		   {Serial1.print(str); 	return;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)           {Serial2.print(str); 	return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)  		   {Serial3.print(str); 	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)      	   {Serial4.print(str); 	return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)  		   {Serial5.print(str); 	return;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)           {Serial6.print(str); 	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)		   {return;}
	#endif
//-------------------------------------------------------------------------------------------------	
  };
//================================================================================================== 
// 										2.7. Вывод print(uint8_t) 
//  Примечание: это временная "заплатка" для совместимости с STM32F1 
//==================================================================================================
  void     RT_HW_BASE:: uartPrint(uint8_t v, uint8_t n){
	(void)v; (void)n;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART0)
		if(n==0)		   {Serial.print(v); 		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART1)
		if(n==1)           {Serial1.print(v); 		return;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART2)
		if(n==2)		   {Serial2.print(v); 		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART3)
		if(n==3)           {Serial3.print(v); 		return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART4)
		if(n==4)		   {Serial4.print(v); 		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART5)
		if(n==5)           {Serial5.print(v); 		return;}
	#endif		
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART6)
		if(n==6)		   {Serial6.print(v); 		return;}
	#endif	
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_UART200)
		if(n==200)		   {return;}
	#endif		
  };
//=================================================================================================
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#endif		//====END #if !defined(RT_HW_CREATE_UART_CODE) && defined(RT_HW_CORE_RP2040)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
  