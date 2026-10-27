/*
 * FLProg_Network — flprogNetworkDeviceRegistry.h
 * Назначение: Реестр NetworkDevice: привязка NET NUM к живому FLProgNetworkInterface.
 *
 * Архитектурные заметки:
 * - Один NET NUM должен однозначно указывать на один NetworkDevice.
 * - Повторная регистрация того же объекта и конфликт ID различаются диагностически.
 * - Реестр не владеет объектами и не должен создавать/удалять transport самостоятельно.
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

#include <stdint.h>

// RC6-A.1 public logical network-device identity for FLProg blocks.
// This layer intentionally does not alter route, transport, socket, or service
// behavior. It only provides stable NET NUM -> FLProgNetworkInterface mapping.

#ifndef FLPROG_NETWORK_MAX_DEVICES
#define FLPROG_NETWORK_MAX_DEVICES 8
#endif

class FLProgNetworkInterface;

typedef uint8_t FLProgNetworkDeviceId;

#ifndef FLPROG_NETWORK_DEVICE_INVALID
#define FLPROG_NETWORK_DEVICE_INVALID ((FLProgNetworkDeviceId)0xFFU)
#endif

enum FLProgNetworkDeviceRegistryResult : uint8_t
{
    FLPROG_NETWORK_DEVICE_OK = 0,
    FLPROG_NETWORK_DEVICE_ALREADY_REGISTERED = 1,

    FLPROG_NETWORK_DEVICE_INVALID_ID = 10,
    FLPROG_NETWORK_DEVICE_NULL = 11,
    FLPROG_NETWORK_DEVICE_DUPLICATE_ID = 12,
    FLPROG_NETWORK_DEVICE_ALREADY_BOUND = 13,
    FLPROG_NETWORK_DEVICE_REGISTRY_FULL = 14,
    FLPROG_NETWORK_DEVICE_NOT_FOUND = 15,
    FLPROG_NETWORK_DEVICE_OWNER_MISMATCH = 16
};

struct FLProgNetworkDeviceEntry
{
    FLProgNetworkDeviceId id = FLPROG_NETWORK_DEVICE_INVALID;
    FLProgNetworkInterface *device = nullptr;
    bool used = false;
};

class FLProgNetworkDeviceRegistry
{
public:
    FLProgNetworkDeviceRegistry();

    // Register a public logical NET NUM.
    // Rules:
    //  - 0xFF is reserved and rejected.
    //  - repeating the same id + same object is idempotent and returns
    //    ALREADY_REGISTERED;
    //  - the same id for another object returns DUPLICATE_ID;
    //  - the same object under another id returns ALREADY_BOUND;
    //  - no dynamic memory is used.
    FLProgNetworkDeviceRegistryResult registerDevice(
        FLProgNetworkDeviceId id,
        FLProgNetworkInterface *device);

    FLProgNetworkDeviceRegistryResult registerDevice(
        FLProgNetworkDeviceId id,
        FLProgNetworkInterface &device)
    {
        return registerDevice(id, &device);
    }

    FLProgNetworkDeviceRegistryResult unregisterDevice(
        FLProgNetworkDeviceId id,
        FLProgNetworkInterface *expectedOwner = nullptr);

    // Resolve NET NUM to the network object. Missing ids return nullptr and set
    // lastResult() to DEVICE_NOT_FOUND.
    FLProgNetworkInterface *getDevice(FLProgNetworkDeviceId id);
    const FLProgNetworkInterface *getDevice(FLProgNetworkDeviceId id) const;

    bool exists(FLProgNetworkDeviceId id) const;

    FLProgNetworkDeviceId idFor(const FLProgNetworkInterface *device) const;

    uint8_t count() const { return _count; }
    uint8_t capacity() const { return FLPROG_NETWORK_MAX_DEVICES; }
    uint8_t freeCount() const
    {
        return _count >= FLPROG_NETWORK_MAX_DEVICES
            ? 0
            : (uint8_t)(FLPROG_NETWORK_MAX_DEVICES - _count);
    }

    FLProgNetworkDeviceRegistryResult lastResult() const { return _lastResult; }
    FLProgNetworkDeviceId lastId() const { return _lastId; }

    const FLProgNetworkDeviceEntry *entryAt(uint8_t index) const;

    // Intended for deterministic tests/startup reconfiguration. It does not
    // delete network objects because the registry never owns them.
    void clear();

private:
    int8_t findById(FLProgNetworkDeviceId id) const;
    int8_t findByDevice(const FLProgNetworkInterface *device) const;
    int8_t findFree() const;
    void setLast(
        FLProgNetworkDeviceRegistryResult result,
        FLProgNetworkDeviceId id) const;

    FLProgNetworkDeviceEntry _entries[FLPROG_NETWORK_MAX_DEVICES];
    uint8_t _count = 0;
    mutable FLProgNetworkDeviceRegistryResult _lastResult = FLPROG_NETWORK_DEVICE_OK;
    mutable FLProgNetworkDeviceId _lastId = FLPROG_NETWORK_DEVICE_INVALID;
};

// One registry for the generated FLProg application. User/network objects are
// still created explicitly; this object only maps stable logical numbers.
extern FLProgNetworkDeviceRegistry flprogNetworkDevices;
