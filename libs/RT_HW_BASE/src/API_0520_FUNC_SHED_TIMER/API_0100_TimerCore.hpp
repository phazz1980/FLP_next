//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//						             RT_HW_BASE.h
// ecoins@mail.ru
// flprog@mail.ru
// 02.03.2020г.
// 10.06.2024г.
// 14.02.2025г. 28.03.2025г.

//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//#ifndef   RT_HW_BASE_h
//#define   RT_HW_BASE_h
#ifndef RT_HW_TimerCore_hpp
#define RT_HW_TimerCore_hpp
	#include "RT_HW_BASE.h"
//=================================================================================================
	#define   RT_HW_TimerCore_VER_NUM    	(uint16_t)101		//--Номер версии;
	#define   RT_HW_TimerCore_VER_NAME 	"v.1.0.1 28.03.2025"	//--Имя вес;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_SUPPORTS_TASK_TIMER) && defined(RT_HW_PERMIT_TASK_TIMER)
	    #define RT_HW_OK_TASK_TIMER
	#endif
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//
//                             УНИВЕРСАЛЬНАЯ ОБЁРТКА ДЛЯ АППАРАТНЫХ ТАЙМЕРОВ
//
//   RT_HW_TimerCore — это кроссплатформенная обёртка, которая автоматически выбирает подходящий
//   аппаратный таймер на основе платформы. Поддерживаются ESP32, ESP8266, AVR, STM32, RP2040 и др.
//   Если прерывания не поддерживаются — используется таймер на базе millis().
//
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#include <TimerInterrupt_Generic.h>		// Подключаем нужную реализацию из TimerInterrupt_Generic

//==============================================================================================
//                     КЛАСС RT_HW_TimerCore 
//==============================================================================================
class RT_HW_TimerCore {
public:


    typedef void (*Callback)();

    Callback cb;
    uint32_t interval=0;
    uint32_t lastCall=0;
    bool attached=0;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_CORE_AVR)
		AVRTimer* timer = nullptr;
	#elif defined(RT_HW_CORE_SAM3X)
		SAM3XTimer* timer = nullptr;
	#elif defined(RT_HW_CORE_ESP8266)
		ESP8266Timer* timer = nullptr;
	#elif defined(RT_HW_CORE_ESP32)
		ESP32Timer* timer = nullptr;
	#elif defined(RT_HW_CORE_STM32)
		STM32TimerInterrupt* timer = nullptr;
	#elif defined(RT_HW_CORE_RP2040)
		RPI_PICO_Timer* timer = nullptr;
	#else
		void* timer = nullptr;
	#endif
//=================================================================================================
//                     1.Конструктор
//=================================================================================================
    RT_HW_TimerCore() : attached(false), lastCall(0), interval(1000), cb(nullptr) {};
//=================================================================================================
//                     2.begin() - Инициализация 
//=================================================================================================
  void begin() {
	#if defined(RT_HW_CORE_AVR)
        timer = new AVRTimer();
	#elif defined(RT_HW_CORE_SAM3X)
        timer = new SAM3XTimer();
	#elif defined(ESP8266)
        timer = new ESP8266Timer();
	#elif defined(ESP32)
        timer = new ESP32Timer(0);
	#elif defined(ARDUINO_ARCH_STM32)
        timer = new STM32Timer();
	#elif defined(ARDUINO_ARCH_RP2040)
        timer = new RPI_PICO_Timer(0);
	#elif defined(__AVR__)
        timer = new AVRTimer();
	#else
        timer = nullptr;
	#endif
  };
//=================================================================================================
//                     3.Подключение задачи
//=================================================================================================
  void attach(Callback _cb, uint32_t _intervalMs) {
    //cb = _cb;
    interval = _intervalMs;
    lastCall = millis();
    attached = true;
//-------------------------------------------------------------------------------------------------
	#if defined(RT_HW_OK_TASK_TIMER)
		if (timer) {timer->attachInterruptInterval(interval * 1000, _cb);} 		//--Микросекунды;
	#endif
  };
//=================================================================================================
//                     4.Отключение задачи
//=================================================================================================
  void detach() {
    if (!attached) return;
        attached = false;
	#if defined(RT_HW_OK_TASK_TIMER)
        if (timer) {timer->detachInterrupt();}
	#endif
 };
//=================================================================================================
//                     5.Отключение задачи
//=================================================================================================
    void pool() {
	#if !defined(RT_HW_OK_TASK_TIMER)
        if (!attached || cb == nullptr || timer != nullptr) return;
        if (millis() - lastCall >= interval) {
            lastCall = millis();
            cb();
        }
	#endif	
    };
//=================================================================================================
private:

 };
//=================================================================================================
#endif
//=================================================================================================
// Пример использования RT_HW_TimerCore
//=================================================================================================
/*
#include "RT_HW_TimerCore.hpp"

RT_HW_TimerCore myTimer;

void myBlink() {
    digitalWrite(LED_BUILTIN, !digitalRead(LED_BUILTIN));
}

void RT_HW_BaseBegin() {
    pinMode(LED_BUILTIN, OUTPUT);
    myTimer.begin();
    myTimer.attach(myBlink, 1000); // 1 секунда
}

void RT_HW_BasePool() {
    myTimer.pool(); // Только если без прерываний
}
*/

