//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//										Получение данных Ethernet
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++ 
//=================================================================================================
//										1. Получение номера шины Ethernet(0-3)
//=================================================================================================
  uint8_t PLC_getBusETH0(){
	#if defined(RT_HW_PLC_BUS_ETH0)
		return  RT_HW_PLC_BUS_ETH0
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getBusETH1(){
	#if defined(RT_HW_PLC_BUS_ETH1)
		return  RT_HW_PLC_BUS_ETH1
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getBusETH2(){
	#if defined(RT_HW_PLC_BUS_ETH2)
		return  RT_HW_PLC_BUS_ETH2
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getBusETH3(){
	#if defined(RT_HW_PLC_BUS_ETH3)
		return  RT_HW_PLC_BUS_ETH3
	#else
		return 255;	 
	#endif	  
  };  
//=================================================================================================
//										2. Получение cs Ethernet(0-3)
//=================================================================================================
  uint8_t PLC_getCsETH0(){
	#if defined(RT_HW_PLC_CS_ETH0)
		return  RT_HW_PLC_CS_ETH0
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getCsETH1(){
	#if defined(RT_HW_PLC_CS_ETH1)
		return  RT_HW_PLC_CS_ETH1
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getCsETH2(){
	#if defined(RT_HW_PLC_CS_ETH2)
		return  RT_HW_PLC_CS_ETH2
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getCsETH3(){
	#if defined(RT_HW_PLC_cs_ETH3)
		return  RT_HW_PLC_cs_ETH3
	#else
		return 255;	 
	#endif	  
  };   
//=================================================================================================
//										3. Получение oe Ethernet(0-3)
//================================================================================================= 
  uint8_t PLC_getOeETH0(){
	#if defined(RT_HW_PLC_OE_ETH0)
		return  RT_HW_PLC_OE_ETH0
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getOeETH1(){
	#if defined(RT_HW_PLC_OE_ETH1)
		return  RT_HW_PLC_OE_ETH1
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getOeETH2(){
	#if defined(RT_HW_PLC_OE_ETH2)
		return  RT_HW_PLC_OE_ETH2
	#else
		return 255;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint8_t PLC_getOeETH3(){
	#if defined(RT_HW_PLC_OE_ETH3)
		return  RT_HW_PLC_OE_ETH3
	#else
		return 255;	 
	#endif	  
  };
//=================================================================================================
//										4. Получение speed Ethernet(0-3)
//================================================================================================= 
  uint32_t PLC_getSpeedETH0(){
	#if 	defined(RT_HW_PLC_SPEED_ETH0)
		return  RT_HW_PLC_SPEED_ETH0
	#else
		return  RT_HW_SPI_SPEED;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint32_t PLC_getSpeedETH1(){
	#if defined(RT_HW_PLC_SPEED_ETH1)
		return  RT_HW_PLC_SPEED_ETH1
	#else
		return  RT_HW_SPI_SPEED;
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint32_t PLC_getSpeedETH2(){
	#if defined(RT_HW_PLC_SPEED_ETH2)
		return  RT_HW_PLC_SPEED_ETH2
	#else
		return  RT_HW_SPI_SPEED;	 
	#endif	  
  };
//-------------------------------------------------------------------------------------------------
  uint32_t PLC_getSpeedETH3(){
	#if defined(RT_HW_PLC_SPEED_ETH3)
		return  RT_HW_PLC_SPEED_ETH3
	#else
		return  RT_HW_SPI_SPEED; 
	#endif	  
  };
//=================================================================================================
 