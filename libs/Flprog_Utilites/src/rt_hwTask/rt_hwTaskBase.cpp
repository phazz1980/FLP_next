#include "rt_hwTaskBase.h"

FlprogRtHwTask::FlprogRtHwTask(uint8_t type)
{
  setType(type);
}

void FlprogRtHwTask::setType(uint8_t type)
{
  if (type == FLPROG_TASK_EXTERNAL_CHNGED_EVENT_SKIP)
  {
    _task = new FlprogRtHwTaskC();
  }
  if (type == FLPROG_TASK_EXTERNAL_CHNGED)
  {
    _task = new FlprogRtHwTaskQ();
  }
  if (type == FLPROG_TASK_COMAND_START)
  {
    _task = new FlprogRtHwTaskE();
  }
  if (type == FLPROG_TASK_ONE_TIME_START)
  {
    _task = new FlprogRtHwTaskF();
  }

  if (type == FLPROG_TASK_EVERY_CYCLE_START)
  {
    _task = new FlprogRtHwTaskR();
  }
  if (type == FLPROG_TASK_PERIODIK_EVENT_SKIP)
  {
    _task = new FlprogRtHwTaskP();
  }
  if (type == FLPROG_TASK_PERIODIK_SECONDS_EVENT_SKIP)
  {
    _task = new FlprogRtHwTaskPS();
  }
  if (type == FLPROG_TASK_EXTERNAL_CHNGED_AND_PERIODIK_EVENT_SKIP)
  {
    _task = new FlprogRtHwTaskT();
  }
  if (type == FLPROG_TASK_EXTERNAL_CHNGED_OR_PERIODIK_EVENT_SKIP)
  {
    _task = new FlprogRtHwTaskA();
  }
}

void FlprogRtHwTask::setQntPass(uint8_t qntPass)
{
  if (_task)
  {
    _task->setQntPass(qntPass);
  }
}

void FlprogRtHwTask::setPeriod(uint16_t period)
{
  if (_task)
  {
    _task->setPeriod(period);
  }
}

uint8_t FlprogRtHwTask::direct(uint8_t extEN)
{
  if (_task)
  {
    return _task->direct(extEN);
  }
  return 0;
}

uint8_t FlprogRtHwTask::run()
{
  if (_task)
  {
    return _task->run();
  }
  return 0;
}

bool FlprogRtHwTask::canWork()
{
  if (_task)
  {
    return _task->canWork();
  }
  return false;
}

void FlprogRtHwTask::reset()
{
  if (_task)
  {
    _task->reset();
  }
}

void FlprogRtHwTask::run(uint8_t value)
{
  if (_task)
  {
    _task->run(value);
  }
}