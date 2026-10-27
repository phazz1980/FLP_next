//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//										СТРУКТУРЫ СЧЕТЧИКОВ
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//=================================================================================================
//						           0.1.Структуры счетчиков
//=================================================================================================
  struct RT_HW_COUNTER_ID  {uint8_t reset=1, first=1; uint8_t direct=0; int16_t begin=0, step=1,  bottom=0, top=100,cnt=0; int32_t var;	
	RT_HW_COUNTER_ID(int16_t _begin, int16_t _step, int16_t _bottom, int16_t _top){begin=_begin; step=_step; bottom=_bottom; top=_top; reset=0;};
 	RT_HW_COUNTER_ID(                int16_t _step, int16_t _bottom, int16_t _top){              step=_step; bottom=_bottom; top=_top; reset=0;}; 
 	RT_HW_COUNTER_ID(                               int16_t _bottom, int16_t _top){                          bottom=_bottom; top=_top; reset=0;}; 	
   	RT_HW_COUNTER_ID(                                                int16_t _top){                                          top=_top; reset=0;};
	RT_HW_COUNTER_ID(){;};
  }; 
//-------------------------------------------------------------------------------------------------
  struct RT_HW_COUNTER32_ID{uint8_t reset=1, first=1; uint8_t direct=0; int32_t begin=0, step=1, top=100, bottom=0, cnt=0; int32_t var;
  	RT_HW_COUNTER32_ID(int32_t _begin, int32_t _step, int32_t _bottom, int32_t _top){begin=_begin; step=_step; bottom=_bottom; top=_top; reset=0;};
 	RT_HW_COUNTER32_ID(                int32_t _step, int32_t _bottom, int32_t _top){              step=_step; bottom=_bottom; top=_top; reset=0;}; 
 	RT_HW_COUNTER32_ID(                               int32_t _bottom, int32_t _top){                          bottom=_bottom; top=_top; reset=0;}; 	
   	RT_HW_COUNTER32_ID(                                                int32_t _top){                                          top=_top; reset=0;};
	RT_HW_COUNTER32_ID(){;};  
  };
//================================================================================================= 