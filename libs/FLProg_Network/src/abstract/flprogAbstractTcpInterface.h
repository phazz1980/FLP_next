/*
 * FLProg_Network — flprogAbstractTcpInterface.h
 * Назначение: Legacy-совместимый абстрактный сетевой контракт.
 *
 * Архитектурные заметки:
 * - Сохраняет имена и сигнатуры, которые генерировал старый FLProg.
 * - Реальная работа делегируется FLProgNetworkInterface.
 * - Добавлять сюда нужно только внешний контракт; старые внутренние socket-механизмы не возвращать.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Это публичная или межмодульная декларация: комментарии описывают владельца состояния и допустимый порядок вызовов.
 * - Конфигурация применяется до начала работы; runtime-методы обслуживаются из общего network.pool().
 * - При изменении API учитывать legacy generated-код и frozen API level 10006.
 * Extended maintenance contract:
 * - This declaration defines a public or cross-module boundary; document state ownership and call order here.
 * - Apply configuration before start; service runtime through the shared network.pool() lifecycle.
 * - Any API change must account for legacy generated code and frozen API level 10006.
 */
#pragma once

// FLProg_Ethernet source-compatibility bridge.
//
// The legacy FLProgAbstractTcpInterface was an abstract raw-socket backend.
// FLProg_Network keeps the public interface/configuration contract but routes
// transport/socket work through its A3 architecture instead of restoring the
// legacy virtual socket table.

#include "flprogNetwork.h"

using FLProgAbstractTcpInterface = FLProgNetworkInterface;
