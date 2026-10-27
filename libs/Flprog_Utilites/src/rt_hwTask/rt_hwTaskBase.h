#pragma once
#include <Arduino.h>
#include "RT_HW_BASE.h"

//  Режимы запуска без контроля периода:
#define FLPROG_TASK_EXTERNAL_CHNGED_EVENT_SKIP 1 //	 'C'-запуск по внешним условиям(на вход поступает run=5) с ограничением вызова пропуском событий (qntPass);
#define FLPROG_TASK_EXTERNAL_CHNGED 2            //   'Q'-запуск по внешним условиям(на вход поступает run=5). Для быстрых процессов и для тестирования;
#define FLPROG_TASK_COMAND_START 3               //   'E'-запуск в каждом цикле. Можно запускать периодически  попереднему фронту EN;
#define FLPROG_TASK_ONE_TIME_START 4             //   'F'-однократное выполнение. Для повтора можно установить id.first=1;
#define FLPROG_TASK_EVERY_CYCLE_START 5          //   'R'-запуск в каждом цикле (при EN=1)

//  Режимы запуска c контролем периода:
#define FLPROG_TASK_PERIODIK_EVENT_SKIP 6                     //   'P'-запуск периодический (period) с ограничением вызова пропуском событий (qntPass);
#define FLPROG_TASK_PERIODIK_SECONDS_EVENT_SKIP 7             //   'PS'-запуск периодический (period) с ограничением вызова пропуском событий (qntPass) в секундах;
#define FLPROG_TASK_EXTERNAL_CHNGED_AND_PERIODIK_EVENT_SKIP 7 //   'T'-запуск по внешним условиям(на вход поступает run=5) с ограничением вызова пропуском событий (qntPass)  и ограничением вызова периодом  вызова  (period);
#define FLPROG_TASK_EXTERNAL_CHNGED_OR_PERIODIK_EVENT_SKIP 8  //	 'A'-запуск по внешним условиям(на вход поступает run=5) с ограничением вызова пропуском событий (qntPass) 	 и/или периодическим запуском(period).

//=================================================================================================
//							  < Aбстактный класс для всех типов задач>
//=================================================================================================
class FlprogAbstractRtHwTask
{
public:
  virtual void setQntPass(uint8_t qntPass) { (void)qntPass; }; //--Кол-во пропусков перед проверкой времени;
  virtual void setPeriod(uint16_t period) { (void)period; };   //--Период вызова;
  virtual uint8_t direct(uint8_t extEN) = 0;                   //--Проверка условий для запуска задачи. Возвращает 1 если условия для запуска выполнены, иначе 0. extEN - разрешение проверки условий внешней функции. Для задач без контроля периода всегда 1, для задач с контролем периода 1 при проверке условий внешней функции и при проверке времени, иначе 0;
  virtual uint8_t run() = 0;                                   // --Указатель на переменную запуска задачи. Внешняя функция может устанавливать эту переменную в значение 5 для разрешения запуска задачи при проверке условий внешней функции. Например при изменении параметров. После проверки и выполнения условий->run=5; может устанавливаться внешней функцией в значение =5. Например при изменении параметров.
  virtual void run(uint8_t value) = 0;                         // --Установка значения переменной запуска задачи.
  bool canWork() { return run() != 0; };                       //  --run	=0 нет запуска;   =1 безусловный запуск; 	=2 разрешение проверки условий внешней функции. После проверки и выполнения условий->run=5; может устанавливаться внешней функцией в значение =5. Например при изменения параметров.
  virtual void reset() {};                                     //  --Очистка разрешающего флага запуска задачи.
protected:
};

//=================================================================================================
//							  < Универсальный класс для всех типов задач>
//=================================================================================================

class FlprogRtHwTask
{
public:
  FlprogRtHwTask() {};
  FlprogRtHwTask(uint8_t type);
  void setQntPass(uint8_t qntPass);     //--Кол-во пропусков перед проверкой времени;
  void setPeriod(uint16_t period);      //--Период вызова;
  uint8_t direct(uint8_t extEN = 1);    //--Проверка условий для запуска задачи. Возвращает 1 если условия для запуска выполнены, иначе 0. extEN - разрешение проверки условий внешней функции. Для задач без контроля периода всегда 1, для задач с контролем периода 1 при проверке условий внешней функции и при проверке времени, иначе 0;
  uint8_t run();                        // --Значение переменной запуска задачи. Внешняя функция может устанавливать эту переменную в значение 5 для разрешения запуска задачи при проверке условий внешней функции. Например при изменении параметров. После проверки и выполнения условий->run=5; может устанавливаться внешней функцией в значение =5. Например при изменении параметров.
  void run(uint8_t value);              // --Установка значения переменной запуска задачи. Внешняя функция может устанавливать эту переменную в значение 5 для разрешения запуска задачи при проверке условий внешней функции. Например при изменении параметров. После проверки и выполнения условий->
  void reset();                         //  --Очистка разрешающего флага запуска задачи.
  void setType(uint8_t type);           //  --Установка типа задачи. При изменении типа задачи создается новый объект задачи соответствующего типа. Ресурсы старого объекта задачи освобождаются.
  bool isInit() { return _task != 0; }; //  --Проверка инициализации задачи.
  bool canWork();                       //  --Проверка разрешающего флага запуска задачи. Возвращает true если run != 0, иначе false.

private:
  FlprogAbstractRtHwTask *_task = 0;
};

//=================================================================================================
//							  Конкретные классы для каждого типа задач
//=================================================================================================
//
//
//=================================================================================================
//							  	1.1.C:: Запуск по внешним изменениям параметров  с ограничением вызова пропуском событий (qntPass)
//=================================================================================================

class FlprogRtHwTaskC : public FlprogAbstractRtHwTask
{
public:
  void setQntPass(uint8_t qntPass) { _task.qntPass = qntPass; };
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_C _task;
};

//=================================================================================================
//							  	1.2.Q:: Запуск по внешним условиям (для быстрых процессов и для тестирования)
//=================================================================================================

class FlprogRtHwTaskQ : public FlprogAbstractRtHwTask
{
public:
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_Q _task;
};

//=================================================================================================
//								1.3.E:: Запуск по фронту EN
//=================================================================================================
class FlprogRtHwTaskE : public FlprogAbstractRtHwTask
{
public:
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_E _task;
};

//=================================================================================================
//								1.4.F:: Запуск одноразовый (при EN=1)
//=================================================================================================
class FlprogRtHwTaskF : public FlprogAbstractRtHwTask
{
public:
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_F _task;
};

//=================================================================================================
//								1.5.R:: Запуск в каждом цикле при EN=1
//=================================================================================================
class FlprogRtHwTaskR : public FlprogAbstractRtHwTask
{
public:
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_R _task;
};

//=================================================================================================
//							2.1.P:: Запуск периодический с ограничением вызова пропуском событий (qntPass)
//=================================================================================================
class FlprogRtHwTaskP : public FlprogAbstractRtHwTask
{
public:
  void setQntPass(uint8_t qntPass) { _task.qntPass = qntPass; };
  void setPeriod(uint16_t period) { _task.period = period; };
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_P _task;
};

//=================================================================================================
//							2.1.PS:: Запуск периодический  в секундах с ограничением вызова пропуском событий (qntPass)
//=================================================================================================
class FlprogRtHwTaskPS : public FlprogAbstractRtHwTask
{
public:
  void setQntPass(uint8_t qntPass) { _task.qntPass = qntPass; };
  void setPeriod(uint16_t period) { _task.period = period; };
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_PS _task;
};

//=================================================================================================
//							    2.2.T:: Запуск по внешним условиям с ограничением вызова пропуском событий (qntPass) и периодом  вызова  (period);
//=================================================================================================
class FlprogRtHwTaskT : public FlprogAbstractRtHwTask
{
public:
  void setQntPass(uint8_t qntPass) { _task.qntPass = qntPass; };
  void setPeriod(uint16_t period) { _task.period = period; };
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_T _task;
};

//=================================================================================================
//							    2.3.A:: Запуск по внешним условиям с ограничением вызова пропуском событий (qntPass) или по периоду  вызова  (period);
//=================================================================================================
class FlprogRtHwTaskA : public FlprogAbstractRtHwTask
{
public:
  void setQntPass(uint8_t qntPass) { _task.qntPass = qntPass; };
  void setPeriod(uint16_t period) { _task.period = period; };
  uint8_t direct(uint8_t extEN) { return _task.direct(extEN); };
  uint8_t run() { return _task.run; };
  void run(uint8_t value) { _task.run = value; };
  void reset() { _task.run = 5; };

protected:
  RT_HW_DIRECT_TASK_A _task;
};