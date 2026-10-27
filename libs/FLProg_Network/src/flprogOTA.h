/*
 * Legacy public include facade for FLProg-generated OTA projects.
 *
 * The generated FLProg block constructs FLProgOTA without a network argument,
 * configures it in setup(), and calls pool() from the scheduler. Keep that
 * source contract, but delegate all work to FLProgNetworkOta: it owns the
 * ArduinoOTA singleton, network-ready check, service attachment and metrics.
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

#include "flprogNetwork.h"

class FLProgOTA : public FLProgNetworkOta
{
public:
    void pool() override
    {
        if (!_legacyBound)
        {
            // Respect a generated block disabled before its first pool().
            // A later setWorkStatus(true) permits normal lazy binding.
            if (!getWorkStatus()) return;

            FLProgNetworkInterface *network =
                FLProgNetworkInterface::legacyDefaultWifiInterface();
            if (network == nullptr) return;

            _legacyBound = begin(*network);
            if (!_legacyBound) return;
        }

        FLProgNetworkOta::pool();
    }

private:
    bool _legacyBound = false;
};
