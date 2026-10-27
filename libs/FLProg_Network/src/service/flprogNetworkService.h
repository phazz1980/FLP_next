/*
 * FLProg_Network — flprogNetworkService.h
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
#pragma once

class FLProgNetworkInterface;

// -----------------------------------------------------------------------------
// Лёгкая базовая прослойка для протоколов/сервисов, которые должны
// обслуживаться только из FLProgNetworkInterface::pool(). Сервис сам не
// владеет transport; конкретная реализация может использовать managed sockets.
// -----------------------------------------------------------------------------
class FLProgNetworkService
{
public:
    FLProgNetworkService() {}
    virtual ~FLProgNetworkService();

    bool attach(FLProgNetworkInterface &network);
    void detach();

    bool isAttached() const { return _serviceNetwork != nullptr; }

    virtual void pool() = 0;
    // Optional identity provider. Strings remain owned by the attached service.
    // nullptr means this service does not provide device identity.
    virtual const char *deviceModel() const { return nullptr; }
    virtual const char *firmwareVersion() const { return nullptr; }

protected:
    FLProgNetworkInterface *serviceNetwork() { return _serviceNetwork; }
    const FLProgNetworkInterface *serviceNetwork() const { return _serviceNetwork; }

private:
    FLProgNetworkInterface *_serviceNetwork = nullptr;
};
