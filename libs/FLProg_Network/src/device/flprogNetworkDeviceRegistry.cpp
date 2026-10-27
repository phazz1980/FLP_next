/*
 * FLProg_Network — flprogNetworkDeviceRegistry.cpp
 * Назначение: Реестр NetworkDevice: привязка NET NUM к живому FLProgNetworkInterface.
 *
 * Архитектурные заметки:
 * - Один NET NUM должен однозначно указывать на один NetworkDevice.
 * - Повторная регистрация того же объекта и конфликт ID различаются диагностически.
 * - Реестр не владеет объектами и не должен создавать/удалять transport самостоятельно.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Реализация вызывается из cooperative lifecycle и не должна добавлять блокирующие ожидания или скрытые delay().
 * - Смена состояния обязана оставаться согласованной с transport identity, generation и диагностическими счетчиками.
 * - Platform-specific ветки остаются локальными: верхние слои не получают прямой доступ к hardware API.
 * Extended maintenance contract:
 * - The implementation runs in the cooperative lifecycle and must not introduce blocking waits or hidden delay().
 * - State changes must remain consistent with transport identity, generation and diagnostic counters.
 * - Keep platform-specific branches local; upper layers must not access hardware APIs directly.
 */
#include "flprogNetworkDeviceRegistry.h"

// Single global registry used by generated FLProg code. The registry stores
// references only; NetworkDevice lifetime remains owned by the sketch/block.
FLProgNetworkDeviceRegistry flprogNetworkDevices;

FLProgNetworkDeviceRegistry::FLProgNetworkDeviceRegistry()
{
    clear();
}

void FLProgNetworkDeviceRegistry::setLast(
    FLProgNetworkDeviceRegistryResult result,
    FLProgNetworkDeviceId id) const
{
    _lastResult = result;
    _lastId = id;
}

// Clear only bindings. This function never stops transports or destroys
// NetworkDevice instances.
void FLProgNetworkDeviceRegistry::clear()
{
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_DEVICES; ++i)
    {
        _entries[i].id = FLPROG_NETWORK_DEVICE_INVALID;
        _entries[i].device = nullptr;
        _entries[i].used = false;
    }
    _count = 0;
    setLast(FLPROG_NETWORK_DEVICE_OK, FLPROG_NETWORK_DEVICE_INVALID);
}

int8_t FLProgNetworkDeviceRegistry::findById(FLProgNetworkDeviceId id) const
{
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_DEVICES; ++i)
    {
        if (_entries[i].used && _entries[i].id == id)
        {
            return (int8_t)i;
        }
    }
    return -1;
}

int8_t FLProgNetworkDeviceRegistry::findByDevice(
    const FLProgNetworkInterface *device) const
{
    if (device == nullptr)
    {
        return -1;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_DEVICES; ++i)
    {
        if (_entries[i].used && _entries[i].device == device)
        {
            return (int8_t)i;
        }
    }
    return -1;
}

int8_t FLProgNetworkDeviceRegistry::findFree() const
{
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_DEVICES; ++i)
    {
        if (!_entries[i].used)
        {
            return (int8_t)i;
        }
    }
    return -1;
}

// Registration enforces both uniqueness directions:
//   NET NUM -> exactly one device, and one device -> exactly one NET NUM.
FLProgNetworkDeviceRegistryResult FLProgNetworkDeviceRegistry::registerDevice(
    FLProgNetworkDeviceId id,
    FLProgNetworkInterface *device)
{
    if (id == FLPROG_NETWORK_DEVICE_INVALID)
    {
        setLast(FLPROG_NETWORK_DEVICE_INVALID_ID, id);
        return FLPROG_NETWORK_DEVICE_INVALID_ID;
    }

    if (device == nullptr)
    {
        setLast(FLPROG_NETWORK_DEVICE_NULL, id);
        return FLPROG_NETWORK_DEVICE_NULL;
    }

    const int8_t sameId = findById(id);
    if (sameId >= 0)
    {
        if (_entries[(uint8_t)sameId].device == device)
        {
            setLast(FLPROG_NETWORK_DEVICE_ALREADY_REGISTERED, id);
            return FLPROG_NETWORK_DEVICE_ALREADY_REGISTERED;
        }

        setLast(FLPROG_NETWORK_DEVICE_DUPLICATE_ID, id);
        return FLPROG_NETWORK_DEVICE_DUPLICATE_ID;
    }

    const int8_t sameDevice = findByDevice(device);
    if (sameDevice >= 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_ALREADY_BOUND, id);
        return FLPROG_NETWORK_DEVICE_ALREADY_BOUND;
    }

    const int8_t freeIndex = findFree();
    if (freeIndex < 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_REGISTRY_FULL, id);
        return FLPROG_NETWORK_DEVICE_REGISTRY_FULL;
    }

    FLProgNetworkDeviceEntry &entry = _entries[(uint8_t)freeIndex];
    entry.id = id;
    entry.device = device;
    entry.used = true;
    ++_count;

    setLast(FLPROG_NETWORK_DEVICE_OK, id);
    return FLPROG_NETWORK_DEVICE_OK;
}

// expectedOwner protects against an old block unregistering a NET NUM that
// has already been rebound to another NetworkDevice.
FLProgNetworkDeviceRegistryResult FLProgNetworkDeviceRegistry::unregisterDevice(
    FLProgNetworkDeviceId id,
    FLProgNetworkInterface *expectedOwner)
{
    if (id == FLPROG_NETWORK_DEVICE_INVALID)
    {
        setLast(FLPROG_NETWORK_DEVICE_INVALID_ID, id);
        return FLPROG_NETWORK_DEVICE_INVALID_ID;
    }

    const int8_t index = findById(id);
    if (index < 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_NOT_FOUND, id);
        return FLPROG_NETWORK_DEVICE_NOT_FOUND;
    }

    FLProgNetworkDeviceEntry &entry = _entries[(uint8_t)index];
    if (expectedOwner != nullptr && entry.device != expectedOwner)
    {
        setLast(FLPROG_NETWORK_DEVICE_OWNER_MISMATCH, id);
        return FLPROG_NETWORK_DEVICE_OWNER_MISMATCH;
    }

    entry.id = FLPROG_NETWORK_DEVICE_INVALID;
    entry.device = nullptr;
    entry.used = false;
    if (_count > 0)
    {
        --_count;
    }

    setLast(FLPROG_NETWORK_DEVICE_OK, id);
    return FLPROG_NETWORK_DEVICE_OK;
}

// Lookup updates lastResult/lastId for diagnostics but does not mutate the
// registered entry itself.
FLProgNetworkInterface *FLProgNetworkDeviceRegistry::getDevice(
    FLProgNetworkDeviceId id)
{
    if (id == FLPROG_NETWORK_DEVICE_INVALID)
    {
        setLast(FLPROG_NETWORK_DEVICE_INVALID_ID, id);
        return nullptr;
    }

    const int8_t index = findById(id);
    if (index < 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_NOT_FOUND, id);
        return nullptr;
    }

    setLast(FLPROG_NETWORK_DEVICE_OK, id);
    return _entries[(uint8_t)index].device;
}

const FLProgNetworkInterface *FLProgNetworkDeviceRegistry::getDevice(
    FLProgNetworkDeviceId id) const
{
    if (id == FLPROG_NETWORK_DEVICE_INVALID)
    {
        setLast(FLPROG_NETWORK_DEVICE_INVALID_ID, id);
        return nullptr;
    }

    const int8_t index = findById(id);
    if (index < 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_NOT_FOUND, id);
        return nullptr;
    }

    setLast(FLPROG_NETWORK_DEVICE_OK, id);
    return _entries[(uint8_t)index].device;
}

bool FLProgNetworkDeviceRegistry::exists(FLProgNetworkDeviceId id) const
{
    return id != FLPROG_NETWORK_DEVICE_INVALID && findById(id) >= 0;
}

FLProgNetworkDeviceId FLProgNetworkDeviceRegistry::idFor(
    const FLProgNetworkInterface *device) const
{
    const int8_t index = findByDevice(device);
    if (index < 0)
    {
        setLast(FLPROG_NETWORK_DEVICE_NOT_FOUND, FLPROG_NETWORK_DEVICE_INVALID);
        return FLPROG_NETWORK_DEVICE_INVALID;
    }

    const FLProgNetworkDeviceId id = _entries[(uint8_t)index].id;
    setLast(FLPROG_NETWORK_DEVICE_OK, id);
    return id;
}

const FLProgNetworkDeviceEntry *FLProgNetworkDeviceRegistry::entryAt(
    uint8_t index) const
{
    if (index >= FLPROG_NETWORK_MAX_DEVICES)
    {
        return nullptr;
    }
    return &_entries[index];
}
