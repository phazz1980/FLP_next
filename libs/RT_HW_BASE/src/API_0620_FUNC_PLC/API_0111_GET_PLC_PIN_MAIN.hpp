
//=================================================================================================
// RT_HW_PGM_END соответствует 254	
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//  uint8_t getPinPGM(const uint8_t *arr, uint8_t idx)
//-------------------------------------------------------------------------------------------------
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//							1.ДОСТУП К МАССИВУ PLC_PIN_DOT (FLProg:Yx) 
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DOT)
		uint8_t PLC_checkPinDOT(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DOT_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDOT  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DOT_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDOT (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DOT_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" Y0")+String(idx));}
												 else	   {return (String(" Y") +String(idx));}};
		uint8_t PLC_getFullPinDOT()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DOT_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDOT()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DOT_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDOT(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDOT  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDOT (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDOT()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDOT()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif
//=================================================================================================
//							2.ДОСТУП К МАССИВУ PLC_PIN_DIN (FLProg:Xx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DIN)			
		uint8_t PLC_checkPinDIN(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DIN_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDIN  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DIN_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDIN (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DIN_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" X0")+String(idx));}
												 else	   {return (String(" X") +String(idx));}};
		uint8_t PLC_getFullPinDIN()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DIN_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDIN()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DIN_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDIN(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDIN  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDIN (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDIN()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDIN()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif
//=================================================================================================
//							3.ДОСТУП К МАССИВУ PLC_PIN_ADC (FLProg:ADx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_ADC)
		uint8_t PLC_checkPinADC(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_ADC_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinADC  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_ADC_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameADC (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_ADC_ID,idx)==255){return " **";};//--Возвращает имя выхода;
																		return (String(" AD") +String(idx));};
		uint8_t PLC_getFullPinADC()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_ADC_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinADC()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_ADC_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinADC(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinADC  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameADC (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinADC()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinADC()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
//=================================================================================================
//							4.ДОСТУП К МАССИВУ PLC_PIN_DAC (FLProg:Dx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DAC)
		uint8_t PLC_checkPinDAC(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DAC_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDAC  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DAC_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDAC (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DAC_ID,idx)==255){return " **";};//--Возвращает имя выхода;
																		return (String(" DA") +String(idx));};
		uint8_t PLC_getFullPinDAC()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DAC_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDAC()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DAC_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDAC(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDAC  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDAC (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDAC()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDAC()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
	
//=================================================================================================
//							5.ДОСТУП К МАССИВУ PLC_PIN_PWM (FLProg:Px)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_PWM)
		uint8_t PLC_checkPinPWM(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_PWM_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinPWM  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_PWM_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNamePWM (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_PWM_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" PW")+String(idx));}
												 else	   {return (String("PW") +String(idx));}};
		uint8_t PLC_getFullPinPWM()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_PWM_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinPWM()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_PWM_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinPWM(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinPWM  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNamePWM (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinPWM()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinPWM()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
//=================================================================================================		

//=================================================================================================
//							6.ДОСТУП К МАССИВУ PLC_PIN_DOX  (FLProg:SYx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DOX)
		uint8_t PLC_checkPinDOX(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DOX_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDOX  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DOX_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDOX (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DOX_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" Y0")+String(idx));}
												 else	   {return (String(" Y") +String(idx));}};
		uint8_t PLC_getFullPinDOX()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DOX_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDOX()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DOX_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDOX(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDOX  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDOX (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDOX()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDOX()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif
//=================================================================================================
//							7.ДОСТУП К МАССИВУ PLC_PIN_DIX (FLProg:SXx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DIX)
		uint8_t PLC_checkPinDIX(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DIX_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDIX  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DIX_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDIX (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DIX_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" X0")+String(idx));}
												 else	   {return (String(" X") +String(idx));}};
		uint8_t PLC_getFullPinDIX()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DIX_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDIX()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DIX_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDIX(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDIX  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDIX (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDIX()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDIX()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif
//=================================================================================================
//							8.ДОСТУП К МАССИВУ PLC_PIN_ADX (FLProg:SADx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_ADX)
		uint8_t PLC_checkPinADX(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_ADX_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinADX  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_ADX_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameADX (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_ADX_ID,idx)==255){return " **";};//--Возвращает имя выхода;
																		return (String(" AD") +String(idx));};
		uint8_t PLC_getFullPinADX()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_ADX_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinADX()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_ADX_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinADX(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinADX  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameADX (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinADX()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinADX()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
//=================================================================================================
//							9.ДОСТУП К МАССИВУ PLC_PIN_DAX (FLProg:SDAx)
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_DAX)
		uint8_t PLC_checkPinDAX(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_DAX_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinDAX  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_DAX_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNameDAX (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_DAX_ID,idx)==255){return " **";};//--Возвращает имя выхода;
																		return (String(" DA") +String(idx));};
		uint8_t PLC_getFullPinDAX()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_DAX_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDAX()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_DAX_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinDAX(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinDAX  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNameDAX (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinDAX()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinDAX()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
//=================================================================================================
//							10.ДОСТУП К МАССИВУ PLC_PIN_PWX (FLProg:SPWx - пока не реализован) 
//=================================================================================================
	#if defined(RT_HW_PERMIT_PLC_PWX)
		uint8_t PLC_checkPinPWX(uint8_t pin) 	{return  checkPinPGM  (RT_HW_PGM_PLC_PWX_ID, pin);};//--Проверка наличия пина;
		uint8_t PLC_getPinPWX  (uint8_t idx)	{return  getPinPGM    (RT_HW_PGM_PLC_PWX_ID, idx);};//--Возвращает номер пина;
		String  PLC_getNamePWX (uint8_t idx)	{if(getPinPGM(RT_HW_PGM_PLC_PWX_ID,idx)==255){return " **";};//--Возвращает имя выхода;
												 if(idx<10){return (String(" PW")+String(idx));}
												 else	   {return (String("PW") +String(idx));}};
		uint8_t PLC_getFullPinPWX()		 		{return  getFullPinPGM(RT_HW_PGM_PLC_PWX_ID);};		//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinPWX()		 		{return  getSizePinPGM(RT_HW_PGM_PLC_PWX_ID);};		//--Возвращает фактическое кол-во пинов; 
	#else
		uint8_t PLC_checkPinPWX(uint8_t pin) 	{(void)pin; return     0;};	//--Проверка наличия пина;
		uint8_t PLC_getPinPWX  (uint8_t idx)	{(void)idx; return   255;};	//--Возвращает номер пина;
		String  PLC_getNamePWX (uint8_t idx)	{(void)idx; return " **";};	//--Возвращает имя выхода;	
		uint8_t PLC_getFullPinPWX()		 		{			return     0;};	//--Возвращает полный размер массива;
		uint8_t PLC_getSizePinPWX()		 		{			return     0;};	//--Возвращает фактическое кол-во пинов; 
	#endif	
//=================================================================================================	