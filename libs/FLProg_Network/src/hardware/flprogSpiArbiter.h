/*
 * FLProg_Network — flprogSpiArbiter.h
 * Назначение: Арбитр SPI-доступа между сетевыми операциями.
 *
 * Архитектурные заметки:
 * - Не допускает пересечения транзакций одного физического SPI ресурса.
 * - Владелец освобождает шину только после завершения своей транзакции.
 * - Арбитр не должен подменять platform SPI backend или хранить сетевое состояние.
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

#include <Arduino.h>

// -----------------------------------------------------------------------------
// FLProg_Network v0.4.5 - кооперативный арбитраж SPI.
//
// Арбитр защищает SPI-шины, используемые объектами FLProg_Network. Он рассчитан
// на циклический код FLProg/Arduino: если шина занята, сетевой код откладывает
// транзакцию вместо ожидания в цикле.
//
// Внешние библиотеки могут участвовать через общий Busy-флаг или callbacks.
// Callback-lock может использовать RT_HW_Base, mutex или другой общий
// менеджер шины проекта.
// -----------------------------------------------------------------------------

#define FLPROG_SPI_STATUS_OK          0
#define FLPROG_SPI_STATUS_BUSY        1
#define FLPROG_SPI_STATUS_UNAVAILABLE 2
#define FLPROG_SPI_STATUS_PIN_ERROR   3

typedef bool (*FLProgSpiLockCallback)(uint8_t bus, void *context);
typedef void (*FLProgSpiUnlockCallback)(uint8_t bus, void *context);

class FLProgSpiArbiter
{
public:
    static bool tryLock(uint8_t bus, const void *owner);

    // Возвращает true, если этим вызовом освобождён внешний уровень блокировки.
    static bool unlock(uint8_t bus, const void *owner);

    static bool isBusy(uint8_t bus, const void *owner = nullptr);
    static bool isOwnedBy(uint8_t bus, const void *owner);

    static void noteConflict(uint8_t bus);
    static void noteTransaction(uint8_t bus);

    static uint32_t conflictCount(uint8_t bus);
    static uint32_t transactionCount(uint8_t bus);
    static uint8_t lockDepth(uint8_t bus);

    static void resetStatistics(uint8_t bus);

private:
    static const uint8_t BUS_COUNT = 4;

    static uint8_t normalize(uint8_t bus)
    {
        return (bus < BUS_COUNT) ? bus : 0;
    }

    static const void *_owner[BUS_COUNT];
    static uint8_t _depth[BUS_COUNT];
    static uint32_t _conflicts[BUS_COUNT];
    static uint32_t _transactions[BUS_COUNT];
};
