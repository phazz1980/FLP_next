/*
 * FLProg_Network — flprogNetworkService.cpp
 * Назначение: Базовый lifecycle сетевых сервисов FLProg_Network.
 *
 * Архитектурные заметки:
 * - Сервис использует готовый NetworkDevice и не создаёт transport сам.
 * - pool() должен быть неблокирующим и согласованным с состоянием сети.
 * - Общие retry/error правила размещаются здесь только если одинаковы для всех сервисов.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Transport описывает availability, identity и generation, а не прикладной TCP/HTTP policy.
 * - Реальная смена link/readiness меняет generation один раз и уведомляет socket layer без каскадных ложных reset.
 * - Stub остается inert: включение заголовка не регистрирует backend и не меняет поведение существующей сети.
 * Extended maintenance contract:
 * - Transport describes availability, identity and generation, not application TCP/HTTP policy.
 * - A real link/readiness change increments generation once and notifies the socket layer without cascaded false resets.
 * - A stub stays inert: including its header neither registers a backend nor changes existing network behavior.
 */
#include "flprogNetworkService.h"
#include "../flprogNetwork.h"

FLProgNetworkService::~FLProgNetworkService()
{
    detach();
}

bool FLProgNetworkService::attach(FLProgNetworkInterface &network)
{
    if (_serviceNetwork == &network)
    {
        return true;
    }

    detach();

    if (!network.registerManagedService(this))
    {
        return false;
    }

    _serviceNetwork = &network;
    return true;
}

void FLProgNetworkService::detach()
{
    if (_serviceNetwork == nullptr)
    {
        return;
    }

    FLProgNetworkInterface *network = _serviceNetwork;
    _serviceNetwork = nullptr;
    network->unregisterManagedService(this);
}
