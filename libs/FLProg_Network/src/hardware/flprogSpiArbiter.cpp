/*
 * FLProg_Network — flprogSpiArbiter.cpp
 * Назначение: Арбитр SPI-доступа между сетевыми операциями.
 *
 * Архитектурные заметки:
 * - Не допускает пересечения транзакций одного физического SPI ресурса.
 * - Владелец освобождает шину только после завершения своей транзакции.
 * - Арбитр не должен подменять platform SPI backend или хранить сетевое состояние.
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
#include "../flprogNetworkProfile.h"

#if FLPROG_NETWORK_COMPILE_WIRED
#include "flprogSpiArbiter.h"

const void *FLProgSpiArbiter::_owner[BUS_COUNT] = {nullptr, nullptr, nullptr, nullptr};
uint8_t FLProgSpiArbiter::_depth[BUS_COUNT] = {0, 0, 0, 0};
uint32_t FLProgSpiArbiter::_conflicts[BUS_COUNT] = {0, 0, 0, 0};
uint32_t FLProgSpiArbiter::_transactions[BUS_COUNT] = {0, 0, 0, 0};

bool FLProgSpiArbiter::tryLock(uint8_t bus, const void *owner)
{
    const uint8_t index = normalize(bus);

    if (owner == nullptr)
    {
        noteConflict(index);
        return false;
    }

    if (_owner[index] == nullptr)
    {
        _owner[index] = owner;
        _depth[index] = 1;
        return true;
    }

    if (_owner[index] == owner)
    {
        if (_depth[index] < 255)
        {
            _depth[index]++;
        }
        return true;
    }

    noteConflict(index);
    return false;
}

bool FLProgSpiArbiter::unlock(uint8_t bus, const void *owner)
{
    const uint8_t index = normalize(bus);

    if (_owner[index] != owner || _depth[index] == 0)
    {
        return false;
    }

    _depth[index]--;

    if (_depth[index] == 0)
    {
        _owner[index] = nullptr;
        return true;
    }

    return false;
}

bool FLProgSpiArbiter::isBusy(uint8_t bus, const void *owner)
{
    const uint8_t index = normalize(bus);

    if (_owner[index] == nullptr)
    {
        return false;
    }

    if (owner != nullptr && _owner[index] == owner)
    {
        return false;
    }

    return true;
}

bool FLProgSpiArbiter::isOwnedBy(uint8_t bus, const void *owner)
{
    const uint8_t index = normalize(bus);
    return owner != nullptr && _owner[index] == owner;
}

void FLProgSpiArbiter::noteConflict(uint8_t bus)
{
    _conflicts[normalize(bus)]++;
}

void FLProgSpiArbiter::noteTransaction(uint8_t bus)
{
    _transactions[normalize(bus)]++;
}

uint32_t FLProgSpiArbiter::conflictCount(uint8_t bus)
{
    return _conflicts[normalize(bus)];
}

uint32_t FLProgSpiArbiter::transactionCount(uint8_t bus)
{
    return _transactions[normalize(bus)];
}

uint8_t FLProgSpiArbiter::lockDepth(uint8_t bus)
{
    return _depth[normalize(bus)];
}

void FLProgSpiArbiter::resetStatistics(uint8_t bus)
{
    const uint8_t index = normalize(bus);
    _conflicts[index] = 0;
    _transactions[index] = 0;
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
