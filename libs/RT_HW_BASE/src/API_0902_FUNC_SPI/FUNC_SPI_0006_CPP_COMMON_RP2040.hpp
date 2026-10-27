	 //+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//											ОБЪЕКТЫ SPI STM32
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if !defined(RT_HW_CREATE_UART_CODE) && defined(RT_HW_CORE_STM32)
	#define  RT_HW_CREATE_UART_CODE  
//=================================================================================================
// 											0. Объекты UART
//=================================================================================================
	#if defined(RT_HW_PERMIT_SPI0)
       #define RT_HW_PRESENT_SPI0    
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI1)
	#if defined(RT_HW_CREATE_SPI1)
       #define RT_HW_PRESENT_SPI1  
		SPIClass 			 SPI1x;
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI2)
	#if defined(RT_HW_CREATE_SPI2)
       #define RT_HW_PRESENT_SPI2  
		SPIClass 			 SPI2x;
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI3)
	#if defined(RT_HW_CREATE_SPI3)
       #define RT_HW_PRESENT_SPI3  
		SPIClass 			 SPI3x;
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI4)
	#if defined(RT_HW_CREATE_SPI4)
       #define RT_HW_PRESENT_SPI4  
		SPIClass 			 SPI4x;
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI5)
	#if defined(RT_HW_CREATE_SPI5)
       #define RT_HW_PRESENT_SPI5  
		SPIClass 			 SPI5x;
    #endif
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PERMIT_SPI6)
	#if defined(RT_HW_CREATE_SPI6)
       #define RT_HW_PRESENT_SPI6  
		SPIClass 			 SPI6x;
    #endif
	#endif	  
//=================================================================================================

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//										1.БАЗОВЫЕ ФУНКЦИИ SPI
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//										1.1. Инициализации шины
//=================================================================================================
  uint8_t  	RT_HW_BASE:: 	spiBegin(uint8_t bus){
	(void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
	 if(bus==0){
			LinkSPI0.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI0.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI.setSCLK(LinkSPI0.sck); 				//--Установка sck;  
			SPI.setMISO(LinkSPI0.miso);				//--Установка miso;  
			SPI.setMOSI(LinkSPI0.mosi);				//--Установка mosi;
			SPI.begin(); 							//--Инициализация SPI;
						LinkSPI0.status=1;  		//--Установка статуса как Master;
						LinkSPI0.busy=0; 			//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
	 if(bus==1){
			LinkSPI1.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI1.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI1x.setSCLK(LinkSPI1.sck); 			//--Установка sck;  
			SPI1x.setMISO(LinkSPI1.miso);			//--Установка miso;  
			SPI1x.setMOSI(LinkSPI1.mosi);			//--Установка mosi;
			SPI1x.begin(); 							//--Инициализация SPI;
			              LinkSPI1.status=1;  		//--Установка статуса как Master;  
						  LinkSPI1.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
	 if(bus==2){
			LinkSPI2.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI2.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI2x.setSCLK(LinkSPI2.sck); 			//--Установка sck;  
			SPI2x.setMISO(LinkSPI2.miso);			//--Установка miso;  
			SPI2x.setMOSI(LinkSPI2.mosi);			//--Установка mosi;
			SPI2x.begin(); 							//--Инициализация SPI;
			              LinkSPI2.status=1;  		//--Установка статуса как Master;  
						  LinkSPI2.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
	 if(bus==3){
			LinkSPI3.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI3.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI3x.setSCLK(LinkSPI3.sck); 			//--Установка sck;  
			SPI3x.setMISO(LinkSPI3.miso);			//--Установка miso;  
			SPI3x.setMOSI(LinkSPI3.mosi);			//--Установка mosi;
			SPI3x.begin(); 							//--Инициализация SPI;
			              LinkSPI3.status=1;  		//--Установка статуса как Master;  
						  LinkSPI3.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
	 if(bus==4){
			LinkSPI4.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI4.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI4x.setSCLK(LinkSPI4.sck); 			//--Установка sck;  
			SPI4x.setMISO(LinkSPI4.miso);			//--Установка miso;  
			SPI4x.setMOSI(LinkSPI4.mosi);			//--Установка mosi;
			SPI4x.begin(); 							//--Инициализация SPI;
			              LinkSPI4.status=1;  		//--Установка статуса как Master;  
						  LinkSPI4.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
	 if(bus==5){
			LinkSPI5.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI5.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI5x.setSCLK(LinkSPI5.sck); 			//--Установка sck;  
			SPI5x.setMISO(LinkSPI5.miso);			//--Установка miso;  
			SPI5x.setMOSI(LinkSPI5.mosi);			//--Установка mosi;
			SPI5x.begin(); 							//--Инициализация SPI;
			              LinkSPI5.status=1;  		//--Установка статуса как Master;  
						  LinkSPI5.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
	 if(bus==6){
			LinkSPI6.codeErr=0;						//--Сброс кода ошибок;
	    if( LinkSPI6.status==1)	 {return 1;} 		//--Выход, если шина уже Master;
		if(!spiSetBusy(bus))	 {return 0;}		//--Захват шины или выход еcли шина занята;
		    spiEnd(bus);							//--Отключение шины SPI (если была инициал. SPIx.begin());
			SPI6x.setSCLK(LinkSPI6.sck); 			//--Установка sck;  
			SPI6x.setMISO(LinkSPI6.miso);			//--Установка miso;  
			SPI6x.setMOSI(LinkSPI6.mosi);			//--Установка mosi;
			SPI6x.begin(); 							//--Инициализация SPI;
			              LinkSPI6.status=1;  		//--Установка статуса как Master;  
						  LinkSPI6.busy=0;  		//--Освобождение шины;
		return 1;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	return 0;
};

//=================================================================================================
//										1.2. Отключение шины
//=================================================================================================
  void		RT_HW_BASE::	spiEnd (uint8_t bus){
	(void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
		if(bus==0) {SPI.end(); 		return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
		if(bus==1) {SPI1x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
		if(bus==2) {SPI2x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3) {SPI3x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
		if(bus==4) {SPI4x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
		if(bus==5) {SPI5x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
		if(bus==5) {SPI6x.end(); 	return;}			
	#endif
//-------------------------------------------------------------------------------------------------
  };
//=================================================================================================
//										1.3. Начало транзакции
//=================================================================================================
  void 		RT_HW_BASE:: 	spiBeginTransaction(uint32_t speed, uint8_t bitOrder, uint8_t dataMode, uint8_t bus){
	(void)speed; (void)bitOrder; (void)dataMode; (void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
		if(bus==0) {if(bitOrder){SPI.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
		if(bus==1) {if(bitOrder){SPI1x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI1x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
		if(bus==2) {if(bitOrder){SPI2x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI2x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3) {if(bitOrder){SPI3x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI3x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
		if(bus==4) {if(bitOrder){SPI4x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI4x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
		if(bus==5) {if(bitOrder){SPI5x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI5x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
		if(bus==6) {if(bitOrder){SPI6x.beginTransaction (SPISettings(speed,MSBFIRST,dataMode));}	
		else                    {SPI6x.beginTransaction (SPISettings(speed,LSBFIRST,dataMode));}
		return;
		}						 
	#endif
//-------------------------------------------------------------------------------------------------
  };

//=================================================================================================
//										1.4. Завершение транзакции
//=================================================================================================
  void		RT_HW_BASE::	spiEndTransaction  (uint8_t bus){
	(void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
		if(bus==0)   		 {SPI.endTransaction(); 	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
		if(bus==1) 			 {SPI1x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
		if(bus==2) 			 {SPI2x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3) 			 {SPI3x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
		if(bus==4) 			 {SPI4x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
		if(bus==5) 			 {SPI5x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
		if(bus==6) 			 {SPI6x.endTransaction(); return;}
	#endif
//-------------------------------------------------------------------------------------------------
  };	
//=================================================================================================
//										1.5. Отправка байта tx, возвращает принятый байт
//=================================================================================================
  uint8_t  	RT_HW_BASE:: 	spiTransfer   (uint8_t  tx, uint8_t bus){
	(void)tx; (void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
		if(bus==0)    {return SPI.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
		if(bus==1)    {return SPI1x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
		if(bus==2)    {return SPI2x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3)    {return SPI3x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3)    {return SPI3x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
		if(bus==4)    {return SPI4x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
		if(bus==5)    {return SPI5x.transfer(tx);}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
		if(bus==6)    {return SPI6x.transfer(tx);}
	 #endif
//-------------------------------------------------------------------------------------------------
	return 0;
  };
//=================================================================================================
//										1.6. Отправка байта tx, прием байта в rx
//=================================================================================================
  void  	RT_HW_BASE:: 	spiTransferVar (uint8_t  tx, uint8_t &rx, uint8_t bus){
	(void)tx; (void)rx; (void)bus;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
		if(bus==0)    {rx=SPI.transfer(tx);		return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
		if(bus==1)    {rx=SPI1x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
		if(bus==2)    {rx=SPI2x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3)    {rx=SPI3x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
		if(bus==3)    {rx=SPI3x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
		if(bus==4)    {rx=SPI4x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
		if(bus==5)    {rx=SPI5x.transfer(tx);	return;}
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
		if(bus==6)    {rx=SPI6x.transfer(tx);	return;}
	 #endif
//-------------------------------------------------------------------------------------------------
  };  
//=================================================================================================
//										1.7. Отправка qnt байт из arr[], 
//												прием qnt байт в  arr[]
//=================================================================================================
  void  	RT_HW_BASE:: 	spiTransferArr (uint8_t qnt, uint8_t *arr, uint8_t bus){
	for(uint8_t i=0; i<qnt; i++){
		arr[i]=spiTransfer(arr[i],bus);
	}
  }; 
//=================================================================================================
//										1.8. Отправка qnt байт из tx[], 
//												прием qnt байт в  rx[]
//=================================================================================================
  void  	RT_HW_BASE:: 	spiTransferArr (uint8_t qnt, uint8_t *tx, uint8_t *rx, uint8_t bus){
	(void)qnt; (void)tx; (void)rx; (void)bus; 
	for(uint8_t i=0; i<qnt; i++){
		rx[i]=spiTransfer(tx[i],bus);
	}
  };   
//=================================================================================================
//										1.9.Установка пинов sck,miso,mosi
// Для STM32   Ответственность за правильный выбор комбинации пинов лежит на пользователе;
//=================================================================================================
  void  	RT_HW_BASE:: 	spiSetPins(uint8_t sck, uint8_t miso, uint8_t mosi, uint8_t bus){ 
	(void)sck; (void)miso; (void)mosi; (void)bus; uint8_t bf; (void)bf;
//=================================================================================================
//++++SPI (SCK,MISO,MOSI) -  2, 4, 5;  6, 8, 7; 18,16;
//++++SPI1(SCK,MISO,MOSI) - 14, 8,15; 10,12,11;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI0)	
	 if(bus==0){
						LinkSPI0.sck=sck;			//--Сохранение пина sck;
						LinkSPI0.miso=miso;			//--Сохранение пина miso;
						LinkSPI0.mosi=mosi;			//--Сохранение пина mosi;
			SPI.setSCLK(LinkSPI0.sck); 				//--Установка пина sck;  
			SPI.setMISO(LinkSPI0.miso);				//--Установка пина miso;  
			SPI.setMOSI(LinkSPI0.mosi);				//--Установка пина mosi;
		    return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI1)	
	 if(bus==1){
						LinkSPI1.sck=sck;			//--Сохранение пина sck;
						LinkSPI1.miso=miso;			//--Сохранение пина miso;
						LinkSPI1.mosi=mosi;			//--Сохранение пина mosi;
		  SPI1x.setSCLK(LinkSPI1.sck); 				//--Установка пина sck;  
		  SPI1x.setMISO(LinkSPI1.miso);				//--Установка пина miso;  
		  SPI1x.setMOSI(LinkSPI1.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI2)	
	 if(bus==2){
						LinkSPI2.sck=sck;			//--Сохранение пина sck;
						LinkSPI2.miso=miso;			//--Сохранение пина miso;
						LinkSPI2.mosi=mosi;			//--Сохранение пина mosi;
		  SPI2x.setSCLK(LinkSPI2.sck); 				//--Установка пина sck;  
		  SPI2x.setMISO(LinkSPI2.miso);				//--Установка пина miso;  
		  SPI2x.setMOSI(LinkSPI2.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI3)	
	 if(bus==3){
						LinkSPI3.sck=sck;			//--Сохранение пина sck;
						LinkSPI3.miso=miso;			//--Сохранение пина miso;
						LinkSPI3.mosi=mosi;			//--Сохранение пина mosi;
		  SPI3x.setSCLK(LinkSPI3.sck); 				//--Установка пина sck;  
		  SPI3x.setMISO(LinkSPI3.miso);				//--Установка пина miso;  
		  SPI3x.setMOSI(LinkSPI3.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI4)	
	 if(bus==4){
						LinkSPI4.sck=sck;			//--Сохранение пина sck;
						LinkSPI4.miso=miso;			//--Сохранение пина miso;
						LinkSPI4.mosi=mosi;			//--Сохранение пина mosi;
		  SPI4x.setSCLK(LinkSPI4.sck); 				//--Установка пина sck;  
		  SPI4x.setMISO(LinkSPI4.miso);				//--Установка пина miso;  
		  SPI4x.setMOSI(LinkSPI4.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI5)	
	 if(bus==5){
						LinkSPI5.sck=sck;			//--Сохранение пина sck;
						LinkSPI5.miso=miso;			//--Сохранение пина miso;
						LinkSPI5.mosi=mosi;			//--Сохранение пина mosi;
		  SPI5x.setSCLK(LinkSPI5.sck); 				//--Установка пина sck;  
		  SPI5x.setMISO(LinkSPI5.miso);				//--Установка пина miso;  
		  SPI5x.setMOSI(LinkSPI5.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_PRESENT_SPI6)	
	 if(bus==6){
						LinkSPI6.sck=sck;			//--Сохранение пина sck;
						LinkSPI6.miso=miso;			//--Сохранение пина miso;
						LinkSPI6.mosi=mosi;			//--Сохранение пина mosi;
		  SPI6x.setSCLK(LinkSPI6.sck); 				//--Установка пина sck;  
		  SPI6x.setMISO(LinkSPI6.miso);				//--Установка пина miso;  
		  SPI6x.setMOSI(LinkSPI6.mosi);				//--Установка пина mosi;
		  return;
	}	
	#endif
//-------------------------------------------------------------------------------------------------
  };  
//=================================================================================================  
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#endif		//====END #if !defined(RT_HW_CREATE_SPI_CODE) && defined(RT_HW_CORE_STM32)
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
