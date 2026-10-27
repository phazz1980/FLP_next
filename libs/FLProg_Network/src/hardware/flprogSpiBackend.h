/*
 * FLProg_Network — flprogSpiBackend.h
 * Назначение: Платформенно-зависимый SPI backend для Wiznet/W5500.
 *
 * Архитектурные заметки:
 * - Выбирает SPI/SPI1/HSPI и проверяет доступность пользовательских пинов.
 * - RP2040 core-ы различаются: setSCK/setRX/setTX разрешены только если API реально существует.
 * - beginTransaction/endTransaction обязаны держать CS и SPI transaction симметрично.
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
#include <SPI.h>

// -----------------------------------------------------------------------------
// FLProg_Network v0.4.5 - абстракция SPI backend.
//
// Политика владения и аппаратное выполнение намеренно разделены:
//   FLProgSpiArbiter -> владение / неблокирующее откладывание / статистика
//   FLProgSpiBackend -> физические операции begin/CS/transfer/end
//
// Стандартный Arduino backend сохраняет FLProg_Network самостоятельной. Адаптер
// RT_HW_Base шаблонный и не создаёт зависимости, пока пользователь сам не
// создаст его с RT_HW_Base и RT_HW_STRUCT_SPI_DEV.
// -----------------------------------------------------------------------------

#ifndef FLPROG_SPI_STATUS_OK
#define FLPROG_SPI_STATUS_OK          0
#define FLPROG_SPI_STATUS_BUSY        1
#define FLPROG_SPI_STATUS_UNAVAILABLE 2
#define FLPROG_SPI_STATUS_PIN_ERROR   3
#endif

#define FLPROG_SPI_BACKEND_ARDUINO 0
#define FLPROG_SPI_BACKEND_RT_HW   1
#define FLPROG_SPI_BACKEND_CUSTOM  2

typedef bool (*FLProgSpiBusyProbeCallback)(uint8_t bus, void *context);

class FLProgSpiBackend
{
public:
    virtual ~FLProgSpiBackend() {}

    virtual void configure(
        uint8_t requestedBus,
        int requestedCs,
        uint32_t speed) = 0;

    virtual bool begin() = 0;

    // beginTransaction() отвечает и за настройку SPI-транзакции, и за установку CS.
    // endTransaction() отвечает за снятие CS и завершение транзакции.
    virtual bool beginTransaction() = 0;
    virtual void endTransaction() = 0;
    virtual uint8_t transfer(uint8_t value) = 0;

    virtual uint8_t bus() const = 0;
    virtual int cs() const = 0;
    virtual bool busAvailable() const = 0;
    virtual bool pinsSupported() const = 0;
    virtual uint8_t kind() const = 0;
    virtual uint8_t status() const = 0;
};

class FLProgArduinoSpiBackend : public FLProgSpiBackend
{
public:
    FLProgArduinoSpiBackend();

    virtual void configure(
        uint8_t requestedBus,
        int requestedCs,
        uint32_t speed);

    virtual bool begin();
    virtual bool beginTransaction();
    virtual void endTransaction();
    virtual uint8_t transfer(uint8_t value);

    virtual uint8_t bus() const { return _bus; }
    virtual int cs() const { return _cs; }
    virtual bool busAvailable() const;
    virtual bool pinsSupported() const;
    virtual uint8_t kind() const { return FLPROG_SPI_BACKEND_ARDUINO; }
    virtual uint8_t status() const { return _status; }

    void setSpiObject(SPIClass *object) { _spiObject = object; }
    void clearSpiObject() { _spiObject = nullptr; }
    SPIClass *spiObject();
    bool hasExplicitSpiObject() const { return _spiObject != nullptr; }

    void setPins(int sck, int miso, int mosi);
    void clearPins();
    bool customPins() const { return _customPins; }

private:
    SPIClass *resolveSpiObject();

    uint8_t _bus = 0;
    int _cs = -1;
    uint32_t _speed = 14000000UL;
    uint8_t _status = FLPROG_SPI_STATUS_OK;

    SPIClass *_spiObject = nullptr;
    int _sck = -1;
    int _miso = -1;
    int _mosi = -1;
    bool _customPins = false;
    bool _transactionOpen = false;
};

// -----------------------------------------------------------------------------
// Адаптер RT_HW_Base.
//
// Класс намеренно НЕ подключает заголовки RT_HW_Base. TRtHwBase и
// TDevice — шаблонные типы, поэтому адаптер создаётся только в
// проектах, где RT_HW_Base уже доступна.
//
// Требуемый API RT_HW_Base (подтверждён в Flprog_Ethernet):
//   getBusETH0(), getCsETH0()
//   spiInitDevice(device)
//   spiSetBusy(bus), spiClearBusy(bus)
//   spiBeginTransaction(device), spiEndTransaction(device)
//   spiBeginCS(device), spiEndCS(device)
//   spiTransfer(value, bus)
//
// В этих вызовах Flprog_Ethernet не показывает читаемой проверки Busy.
// Поэтому предусмотрен необязательный callback проверки Busy. Если в RT_HW_Base появится
// spiIsBusy()/spiTryLock(), его можно привязать здесь, и FLProg_Network отложит
// транзакцию, если шина занята внешним устройством.
// -----------------------------------------------------------------------------

template <typename TRtHwBase, typename TDevice>
class FLProgRtHwSpiBackend : public FLProgSpiBackend
{
public:
    explicit FLProgRtHwSpiBackend(TRtHwBase &base)
        : _base(&base)
    {
    }

    virtual void configure(
        uint8_t requestedBus,
        int requestedCs,
        uint32_t speed)
    {
        if (_base == nullptr)
        {
            _status = FLPROG_SPI_STATUS_UNAVAILABLE;
            return;
        }

        _device.bus =
            (requestedBus == 255)
                ? _base->getBusETH0()
                : requestedBus;

        _device.cs =
            (requestedCs < 0)
                ? _base->getCsETH0()
                : requestedCs;

        _device.speed = speed;
        _status = FLPROG_SPI_STATUS_OK;
    }

    virtual bool begin()
    {
        if (_base == nullptr || _device.cs < 0)
        {
            _status = FLPROG_SPI_STATUS_UNAVAILABLE;
            return false;
        }

        _base->spiInitDevice(_device);
        _status = FLPROG_SPI_STATUS_OK;
        return true;
    }

    virtual bool beginTransaction()
    {
        if (_base == nullptr)
        {
            _status = FLPROG_SPI_STATUS_UNAVAILABLE;
            return false;
        }

        if (
            _busyProbe != nullptr &&
            _busyProbe(_device.bus, _busyProbeContext)
        )
        {
            _status = FLPROG_SPI_STATUS_BUSY;
            return false;
        }

        _base->spiSetBusy(_device.bus);
        _busyHeld = true;

        _base->spiBeginTransaction(_device);
        _base->spiBeginCS(_device);
        _transactionOpen = true;

        _status = FLPROG_SPI_STATUS_OK;
        return true;
    }

    virtual void endTransaction()
    {
        if (_base == nullptr)
        {
            return;
        }

        if (_transactionOpen)
        {
            _base->spiEndCS(_device);
            _base->spiEndTransaction(_device);
            _transactionOpen = false;
        }

        if (_busyHeld)
        {
            _base->spiClearBusy(_device.bus);
            _busyHeld = false;
        }
    }

    virtual uint8_t transfer(uint8_t value)
    {
        if (_base == nullptr)
        {
            _status = FLPROG_SPI_STATUS_UNAVAILABLE;
            return 0xFF;
        }

        return _base->spiTransfer(value, _device.bus);
    }

    virtual uint8_t bus() const { return _device.bus; }
    virtual int cs() const { return _device.cs; }
    virtual bool busAvailable() const { return _base != nullptr; }
    virtual bool pinsSupported() const { return true; }
    virtual uint8_t kind() const { return FLPROG_SPI_BACKEND_RT_HW; }
    virtual uint8_t status() const { return _status; }

    TDevice &device() { return _device; }
    const TDevice &device() const { return _device; }

    void setBusyProbe(
        FLProgSpiBusyProbeCallback callback,
        void *context = nullptr)
    {
        _busyProbe = callback;
        _busyProbeContext = context;
    }

private:
    TRtHwBase *_base = nullptr;
    TDevice _device{};

    FLProgSpiBusyProbeCallback _busyProbe = nullptr;
    void *_busyProbeContext = nullptr;

    uint8_t _status = FLPROG_SPI_STATUS_OK;
    bool _busyHeld = false;
    bool _transactionOpen = false;
};

// Удобный макрос для Arduino-проектов C++11.
#define FLPROG_DECLARE_RT_HW_SPI_BACKEND(name, baseObject, deviceType) \
    FLProgRtHwSpiBackend<decltype(baseObject), deviceType> name(baseObject)
