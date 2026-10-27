/*
 * FLProg_Network — flprogEthernet.h
 * Назначение: Legacy umbrella-header совместимости с Flprog_Ethernet.
 *
 * Архитектурные заметки:
 * - Старый проект может по-прежнему подключать только <flprogEthernet.h>.
 * - Внутри экспортируются новые FLProg_Network типы и тонкие legacy-адаптеры.
 * - Старая raw-socket архитектура здесь не восстанавливается.
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

// FLProg_Ethernet source-compatibility umbrella for FLProg_Network.
// Existing generated projects may keep: #include <flprogEthernet.h>
// All networking is implemented by FLProg_Network/A3; the legacy raw socket
// implementation is not restored.

#include <Arduino.h>
#include "flprogUtilites.h"
#include "flprogNetwork.h"
#include "abstract/flprogAbstractTcpInterface.h"
#include "tcp/flprogEthernetClient.h"
#include "tcp/flprogEthernetServer.h"
#include "udp/flprogUdp.h"
#include "udp/flprogDns.h"
