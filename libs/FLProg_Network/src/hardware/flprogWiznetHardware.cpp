/*
 * FLProg_Network — flprogWiznetHardware.cpp
 * Назначение: Низкоуровневый драйвер Wiznet/W5500 для FLProg_Network.
 *
 * Архитектурные заметки:
 * - Отвечает за регистры чипа, socket-команды и аппаратные параметры Ethernet.
 * - Верхние TCP/UDP классы работают через A3/backend и не должны знать регистровую модель Wiznet.
 * - Все hardware-specific ошибки должны подниматься наверх без ложной смены Wi-Fi transport.
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
#include "flprogWiznetHardware.h"

#define FLPROG_WIZNET_CHIP_DEFER 254

// Четыре поддерживаемые микросхемы используют три разных формата SPI-кадра:
//   W5100/W5100S : 0x0F read / 0xF0 write + 16-bit address
//   W5200        : 16-bit address + R/W bit + 15-bit transfer length
//   W5500        : 16-bit address + block/RW/mode control byte
//
// Сведение этих форматов в один аппаратный класс создаёт основу для
// независимого от транспорта слоя сокетов, запланированного на v0.5.x.

FLProgWiznetHardware::FLProgWiznetHardware()
{
    syncSpiBackendConfiguration();
}

FLProgWiznetHardware::FLProgWiznetHardware(int pinCs, uint8_t bus)
{
    _pinCs = pinCs;
    _spiBus = bus;
    syncSpiBackendConfiguration();
}

void FLProgWiznetHardware::setPinCs(int pinCs)
{
    if (_pinCs == pinCs)
    {
        return;
    }

    _pinCs = pinCs;
    syncSpiBackendConfiguration();
    resetInit();
}

void FLProgWiznetHardware::setSpiBus(uint8_t bus)
{
    if (_spiBus == bus)
    {
        return;
    }

    _spiBus = bus;
    syncSpiBackendConfiguration();
    resetInit();
}

void FLProgWiznetHardware::setSpiSpeed(uint32_t speed)
{
    if (_spiSpeed == speed)
    {
        return;
    }

    _spiSpeed = speed;
    syncSpiBackendConfiguration();
    resetInit();
}

void FLProgWiznetHardware::setSpiBackend(FLProgSpiBackend *backend)
{
    FLProgSpiBackend *next =
        (backend == nullptr)
            ? static_cast<FLProgSpiBackend *>(&_arduinoSpiBackend)
            : backend;

    if (_spiBackend == next)
    {
        return;
    }

    _spiBackend = next;
    syncSpiBackendConfiguration();
    resetInit();
}

void FLProgWiznetHardware::useArduinoSpiBackend()
{
    setSpiBackend(&_arduinoSpiBackend);
}

uint8_t FLProgWiznetHardware::spiBackendKind() const
{
    return (_spiBackend != nullptr)
        ? _spiBackend->kind()
        : FLPROG_SPI_BACKEND_CUSTOM;
}

uint8_t FLProgWiznetHardware::effectiveSpiBus() const
{
    return (_spiBackend != nullptr)
        ? _spiBackend->bus()
        : ((_spiBus == 255) ? 0 : _spiBus);
}

int FLProgWiznetHardware::effectivePinCs() const
{
    return (_spiBackend != nullptr)
        ? _spiBackend->cs()
        : _pinCs;
}

void FLProgWiznetHardware::syncSpiBackendConfiguration()
{
    if (_spiBackend == nullptr)
    {
        _spiBackend = &_arduinoSpiBackend;
    }

    _spiBackend->configure(
        _spiBus,
        _pinCs,
        _spiSpeed
    );
}

void FLProgWiznetHardware::setSpiObject(SPIClass *object)
{
    if (_spiObject == object)
    {
        return;
    }

    _spiObject = object;
    _arduinoSpiBackend.setSpiObject(object);
    resetInit();
}

void FLProgWiznetHardware::clearSpiObject()
{
    setSpiObject(nullptr);
}

SPIClass *FLProgWiznetHardware::spiObject()
{
    return _arduinoSpiBackend.spiObject();
}

void FLProgWiznetHardware::setSpiPins(int sck, int miso, int mosi)
{
    _spiSckPin = sck;
    _spiMisoPin = miso;
    _spiMosiPin = mosi;
    _customSpiPins = true;
    _arduinoSpiBackend.setPins(sck, miso, mosi);
    resetInit();
}

void FLProgWiznetHardware::clearSpiPins()
{
    _spiSckPin = -1;
    _spiMisoPin = -1;
    _spiMosiPin = -1;
    _customSpiPins = false;
    _arduinoSpiBackend.clearPins();
    resetInit();
}

void FLProgWiznetHardware::setSpiLockCallbacks(
    FLProgSpiLockCallback lockCallback,
    FLProgSpiUnlockCallback unlockCallback,
    void *context)
{
    _lockCallback = lockCallback;
    _unlockCallback = unlockCallback;
    _lockContext = context;
}

uint8_t FLProgWiznetHardware::normalizedBus() const
{
    return effectiveSpiBus();
}

bool FLProgWiznetHardware::spiBusAvailable() const
{
    return
        _spiBackend != nullptr &&
        _spiBackend->busAvailable();
}

bool FLProgWiznetHardware::spiPinsSupported() const
{
    if (_spiBackend == nullptr)
    {
        return false;
    }

    // Custom pin settings belong to the Arduino backend. Other backends own
    // their device/pin configuration and report support themselves.
    if (_spiBackend->kind() != FLPROG_SPI_BACKEND_ARDUINO)
    {
        return _spiBackend->pinsSupported();
    }

    return _arduinoSpiBackend.pinsSupported();
}

bool FLProgWiznetHardware::beginBus()
{
    syncSpiBackendConfiguration();

    if (_spiBackend == nullptr || !_spiBackend->busAvailable())
    {
        _lastSpiStatus = FLPROG_SPI_STATUS_UNAVAILABLE;
        _spiOperationStatus = FLPROG_SPI_STATUS_UNAVAILABLE;
        return false;
    }

    if (!_spiBackend->pinsSupported())
    {
        _lastSpiStatus = FLPROG_SPI_STATUS_PIN_ERROR;
        _spiOperationStatus = FLPROG_SPI_STATUS_PIN_ERROR;
        return false;
    }

    if (!_spiBackend->begin())
    {
        _lastSpiStatus = _spiBackend->status();
        _spiOperationStatus = _lastSpiStatus;
        return false;
    }

    _lastSpiStatus = FLPROG_SPI_STATUS_OK;
    return true;
}

bool FLProgWiznetHardware::acquireSpi()
{
    const uint8_t bus = normalizedBus();

    // Повторный захват тем же объектом Wiznet не вызывает внешние hooks второй раз
    // и не проверяет собственный общий Busy-флаг.
    if (FLProgSpiArbiter::isOwnedBy(bus, this))
    {
        return FLProgSpiArbiter::tryLock(bus, this);
    }

    if (_sharedBusyFlag != nullptr && *_sharedBusyFlag)
    {
        FLProgSpiArbiter::noteConflict(bus);
        _lastSpiStatus = FLPROG_SPI_STATUS_BUSY;
        _spiOperationStatus = FLPROG_SPI_STATUS_BUSY;
        _spiDeferredCount++;
        return false;
    }

    if (!FLProgSpiArbiter::tryLock(bus, this))
    {
        _lastSpiStatus = FLPROG_SPI_STATUS_BUSY;
        _spiOperationStatus = FLPROG_SPI_STATUS_BUSY;
        _spiDeferredCount++;
        return false;
    }

    if (_lockCallback != nullptr)
    {
        if (!_lockCallback(bus, _lockContext))
        {
            FLProgSpiArbiter::unlock(bus, this);
            FLProgSpiArbiter::noteConflict(bus);
            _lastSpiStatus = FLPROG_SPI_STATUS_BUSY;
            _spiDeferredCount++;
            return false;
        }

        _externalLockHeld = true;
    }

    if (_sharedBusyFlag != nullptr)
    {
        *_sharedBusyFlag = true;
        _sharedBusyHeld = true;
    }

    _lastSpiStatus = FLPROG_SPI_STATUS_OK;
    return true;
}

void FLProgWiznetHardware::releaseSpi()
{
    const uint8_t bus = normalizedBus();
    const bool outermost =
        FLProgSpiArbiter::unlock(bus, this);

    if (!outermost)
    {
        return;
    }

    if (_sharedBusyHeld && _sharedBusyFlag != nullptr)
    {
        *_sharedBusyFlag = false;
        _sharedBusyHeld = false;
    }

    if (_externalLockHeld)
    {
        if (_unlockCallback != nullptr)
        {
            _unlockCallback(bus, _lockContext);
        }
        _externalLockHeld = false;
    }
}

bool FLProgWiznetHardware::spiBusy() const
{
    if (_sharedBusyFlag != nullptr && *_sharedBusyFlag)
    {
        if (!FLProgSpiArbiter::isOwnedBy(normalizedBus(), this))
        {
            return true;
        }
    }

    return FLProgSpiArbiter::isBusy(
        normalizedBus(),
        this
    );
}

uint32_t FLProgWiznetHardware::spiConflictCount() const
{
    return FLProgSpiArbiter::conflictCount(normalizedBus());
}

uint32_t FLProgWiznetHardware::spiTransactionCount() const
{
    return FLProgSpiArbiter::transactionCount(normalizedBus());
}

void FLProgWiznetHardware::resetSpiStatistics()
{
    _spiDeferredCount = 0;
    FLProgSpiArbiter::resetStatistics(normalizedBus());
}

bool FLProgWiznetHardware::beginTransaction()
{
    if (!acquireSpi())
    {
        return false;
    }

    if (_spiBackend == nullptr)
    {
        _lastSpiStatus = FLPROG_SPI_STATUS_UNAVAILABLE;
        _spiOperationStatus = FLPROG_SPI_STATUS_UNAVAILABLE;
        releaseSpi();
        return false;
    }

    if (!_spiBackend->beginTransaction())
    {
        _lastSpiStatus = _spiBackend->status();
        _spiOperationStatus = _lastSpiStatus;

        if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
        {
            FLProgSpiArbiter::noteConflict(normalizedBus());
            _spiDeferredCount++;
        }

        releaseSpi();
        return false;
    }

    FLProgSpiArbiter::noteTransaction(normalizedBus());
    _lastSpiStatus = FLPROG_SPI_STATUS_OK;
    return true;
}

void FLProgWiznetHardware::endTransaction()
{
    if (_spiBackend != nullptr)
    {
        _spiBackend->endTransaction();
    }

    releaseSpi();
}

uint8_t FLProgWiznetHardware::transfer(uint8_t value)
{
    if (_spiBackend == nullptr)
    {
        _lastSpiStatus = FLPROG_SPI_STATUS_UNAVAILABLE;
        return 0xFF;
    }

    return _spiBackend->transfer(value);
}

void FLProgWiznetHardware::resetInit()
{
    _chip = FLPROG_WIZNET_CHIP_NONE;
    _initState = 0;
    _initStarted = 0;
    _lastSpiStatus = FLPROG_SPI_STATUS_OK;
    _spiOperationStatus = FLPROG_SPI_STATUS_OK;
    _cachedLinkStatus = FLPROG_WIZNET_LINK_UNKNOWN;

    for (uint8_t i = 0; i < 8; i++)
    {
        _txBase[i] = 0;
        _rxBase[i] = 0;
        _txMask[i] = 0;
        _rxMask[i] = 0;
    }
}

uint8_t FLProgWiznetHardware::init()
{
    if (isInit())
    {
        return FLPROG_WIZNET_HW_READY;
    }

    syncSpiBackendConfiguration();

    if (
        effectivePinCs() < 0 ||
        !spiBusAvailable()
    )
    {
        return FLPROG_WIZNET_HW_ERROR;
    }

    if (_initState == 0)
    {
        if (!acquireSpi())
        {
            return FLPROG_WIZNET_HW_WAIT;
        }

        const bool busStarted = beginBus();
        releaseSpi();

        if (!busStarted)
        {
            return FLPROG_WIZNET_HW_ERROR;
        }

        _initStarted = millis();
        _initState = 1;
        return FLPROG_WIZNET_HW_WAIT;
    }

    // Даём внешнему модулю завершить включение без delay().
    if ((uint32_t)(millis() - _initStarted) < 2)
    {
        return FLPROG_WIZNET_HW_WAIT;
    }

    _chip = detectChip();

    if (_chip == FLPROG_WIZNET_CHIP_DEFER)
    {
        _chip = FLPROG_WIZNET_CHIP_NONE;
        return FLPROG_WIZNET_HW_WAIT;
    }

    if (_chip == FLPROG_WIZNET_CHIP_NONE)
    {
        _initState = 0;
        return FLPROG_WIZNET_HW_ERROR;
    }

    if (!configureDefaultSocketMemory())
    {
        if (_spiOperationStatus == FLPROG_SPI_STATUS_BUSY)
        {
            return FLPROG_WIZNET_HW_WAIT;
        }

        _chip = FLPROG_WIZNET_CHIP_NONE;
        _initState = 0;
        return FLPROG_WIZNET_HW_ERROR;
    }

    _initState = 2;
    return FLPROG_WIZNET_HW_READY;
}

const char *FLProgWiznetHardware::chipName() const
{
    switch (_chip)
    {
        case FLPROG_WIZNET_CHIP_W5100:  return "W5100";
        case FLPROG_WIZNET_CHIP_W5100S: return "W5100S";
        case FLPROG_WIZNET_CHIP_W5200:  return "W5200";
        case FLPROG_WIZNET_CHIP_W5500:  return "W5500";
        default:                         return "NONE";
    }
}

uint8_t FLProgWiznetHardware::maxSocketNum() const
{
    switch (_chip)
    {
        case FLPROG_WIZNET_CHIP_W5100:
        case FLPROG_WIZNET_CHIP_W5100S:
            return 4;

        case FLPROG_WIZNET_CHIP_W5200:
        case FLPROG_WIZNET_CHIP_W5500:
            return 8;

        default:
            return 0;
    }
}

uint16_t FLProgWiznetHardware::txMemoryTotal() const
{
    switch (_chip)
    {
        case FLPROG_WIZNET_CHIP_W5100:
        case FLPROG_WIZNET_CHIP_W5100S:
            return 8192;

        case FLPROG_WIZNET_CHIP_W5200:
        case FLPROG_WIZNET_CHIP_W5500:
            return 16384;

        default:
            return 0;
    }
}

uint16_t FLProgWiznetHardware::rxMemoryTotal() const
{
    return txMemoryTotal();
}

// -----------------------------------------------------------------------------
// Обнаружение микросхемы
// -----------------------------------------------------------------------------

uint8_t FLProgWiznetHardware::detectChip()
{
    uint8_t value = 0;

    // W5500 VERSIONR = 0x04 at common-register offset 0x0039.
    readW5500(0x0039, 0x00, &value, 1);
    if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
    {
        return FLPROG_WIZNET_CHIP_DEFER;
    }
    if (value == 0x04)
    {
        return FLPROG_WIZNET_CHIP_W5500;
    }

    // W5200 VERSIONR = 0x03 at absolute address 0x001F.
    value = 0;
    readW5200(0x001F, &value, 1);
    if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
    {
        return FLPROG_WIZNET_CHIP_DEFER;
    }
    if (value == 0x03)
    {
        return FLPROG_WIZNET_CHIP_W5200;
    }

    // W5100S VERR = 0x51 at absolute address 0x0080.
    value = 0;
    readW5100S(0x0080, &value, 1);
    if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
    {
        return FLPROG_WIZNET_CHIP_DEFER;
    }
    if (value == 0x51)
    {
        return FLPROG_WIZNET_CHIP_W5100S;
    }

    // У оригинального W5100 нет регистра версии. Проверяем документированный
    // регистр повторной передачи, сразу восстанавливаем его и принимаем W5100 только
    // если оба байта корректно читаются и записываются через SPI-кадр W5100.
    if (probeW5100())
    {
        return FLPROG_WIZNET_CHIP_W5100;
    }

    if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
    {
        return FLPROG_WIZNET_CHIP_DEFER;
    }

    return FLPROG_WIZNET_CHIP_NONE;
}

bool FLProgWiznetHardware::probeW5100()
{
    uint8_t oldValue[2] = {0, 0};
    uint8_t testValue[2] = {0x12, 0x34};
    uint8_t checkValue[2] = {0, 0};

    readW5100(0x0017, oldValue, 2);
    writeW5100(0x0017, testValue, 2);
    readW5100(0x0017, checkValue, 2);
    writeW5100(0x0017, oldValue, 2);

    return
        checkValue[0] == testValue[0] &&
        checkValue[1] == testValue[1];
}

// -----------------------------------------------------------------------------
// W5500 SPI
// -----------------------------------------------------------------------------

void FLProgWiznetHardware::readW5500(
    uint16_t address,
    uint8_t block,
    uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));
    transfer((uint8_t)(block << 3)); // read, variable-length mode

    for (uint16_t i = 0; i < length; i++)
    {
        buffer[i] = transfer(0x00);
    }

    endTransaction();
}

void FLProgWiznetHardware::writeW5500(
    uint16_t address,
    uint8_t block,
    const uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));
    transfer((uint8_t)((block << 3) | 0x04)); // write

    for (uint16_t i = 0; i < length; i++)
    {
        transfer(buffer[i]);
    }

    endTransaction();
}

// -----------------------------------------------------------------------------
// W5200 SPI
// -----------------------------------------------------------------------------

void FLProgWiznetHardware::readW5200(
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));
    transfer((uint8_t)((length >> 8) & 0x7F)); // OP=0 read
    transfer((uint8_t)(length & 0xFF));

    for (uint16_t i = 0; i < length; i++)
    {
        buffer[i] = transfer(0x00);
    }

    endTransaction();
}

void FLProgWiznetHardware::writeW5200(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));
    transfer((uint8_t)(0x80 | ((length >> 8) & 0x7F))); // OP=1 write
    transfer((uint8_t)(length & 0xFF));

    for (uint16_t i = 0; i < length; i++)
    {
        transfer(buffer[i]);
    }

    endTransaction();
}

// -----------------------------------------------------------------------------
// W5100S SPI
// -----------------------------------------------------------------------------

void FLProgWiznetHardware::readW5100S(
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer(0x0F);
    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));

    for (uint16_t i = 0; i < length; i++)
    {
        buffer[i] = transfer(0x00);
    }

    endTransaction();
}

void FLProgWiznetHardware::writeW5100S(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    if (!beginTransaction())
    {
        return;
    }

    transfer(0xF0);
    transfer((uint8_t)(address >> 8));
    transfer((uint8_t)(address & 0xFF));

    for (uint16_t i = 0; i < length; i++)
    {
        transfer(buffer[i]);
    }

    endTransaction();
}

// -----------------------------------------------------------------------------
// Original W5100 SPI
// -----------------------------------------------------------------------------

void FLProgWiznetHardware::readW5100(
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    // W5100 работает 32-битными блоками, поэтому каждый байт получает отдельный кадр.
    for (uint16_t i = 0; i < length; i++)
    {
        const uint16_t current = address + i;

        if (!beginTransaction())
        {
            return;
        }

        transfer(0x0F);
        transfer((uint8_t)(current >> 8));
        transfer((uint8_t)(current & 0xFF));
        buffer[i] = transfer(0x00);

        endTransaction();
    }
}

void FLProgWiznetHardware::writeW5100(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (buffer == nullptr || length == 0)
    {
        return;
    }

    for (uint16_t i = 0; i < length; i++)
    {
        const uint16_t current = address + i;

        if (!beginTransaction())
        {
            return;
        }

        transfer(0xF0);
        transfer((uint8_t)(current >> 8));
        transfer((uint8_t)(current & 0xFF));
        transfer(buffer[i]);

        endTransaction();
    }
}

// -----------------------------------------------------------------------------
// Единый доступ к общим регистрам и регистрам сокетов
// -----------------------------------------------------------------------------

void FLProgWiznetHardware::readLinear(
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    switch (_chip)
    {
        case FLPROG_WIZNET_CHIP_W5100:
            readW5100(address, buffer, length);
            break;

        case FLPROG_WIZNET_CHIP_W5100S:
            readW5100S(address, buffer, length);
            break;

        case FLPROG_WIZNET_CHIP_W5200:
            readW5200(address, buffer, length);
            break;

        default:
            break;
    }
}

void FLProgWiznetHardware::writeLinear(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    switch (_chip)
    {
        case FLPROG_WIZNET_CHIP_W5100:
            writeW5100(address, buffer, length);
            break;

        case FLPROG_WIZNET_CHIP_W5100S:
            writeW5100S(address, buffer, length);
            break;

        case FLPROG_WIZNET_CHIP_W5200:
            writeW5200(address, buffer, length);
            break;

        default:
            break;
    }
}

uint8_t FLProgWiznetHardware::readCommon(uint16_t address)
{
    uint8_t value = 0;
    readCommon(address, &value, 1);
    return value;
}

void FLProgWiznetHardware::readCommon(
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        readW5500(address, 0x00, buffer, length);
    }
    else
    {
        readLinear(address, buffer, length);
    }
}

void FLProgWiznetHardware::writeCommon(uint16_t address, uint8_t value)
{
    writeCommon(address, &value, 1);
}

bool FLProgWiznetHardware::takeIpConflictFlag()
{
    clearSpiOperationStatus();
    const uint8_t value = readCommon(REG_IR);
    if (spiOperationStatus() != FLPROG_SPI_STATUS_OK || (value & 0x80) == 0)
    {
        return false;
    }

    // Common IR.CONFLICT is write-one-to-clear on the supported W5x00 family.
    clearSpiOperationStatus();
    writeCommon(REG_IR, 0x80);
    return spiOperationStatus() == FLPROG_SPI_STATUS_OK;
}

void FLProgWiznetHardware::writeCommon(
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        writeW5500(address, 0x00, buffer, length);
    }
    else
    {
        writeLinear(address, buffer, length);
    }
}

uint16_t FLProgWiznetHardware::absoluteSocketAddress(
    uint8_t socket,
    uint16_t offset) const
{
    if (_chip == FLPROG_WIZNET_CHIP_W5200)
    {
        return (uint16_t)(0x4000 + ((uint16_t)socket * 0x0100) + offset);
    }

    // W5100 and W5100S share the traditional socket-register map.
    return (uint16_t)(0x0400 + ((uint16_t)socket * 0x0100) + offset);
}

uint8_t FLProgWiznetHardware::readSocket(
    uint8_t socket,
    uint16_t address)
{
    uint8_t value = 0;
    readSocket(socket, address, &value, 1);
    return value;
}

void FLProgWiznetHardware::readSocket(
    uint8_t socket,
    uint16_t address,
    uint8_t *buffer,
    uint16_t length)
{
    if (
        buffer == nullptr ||
        socket >= maxSocketNum() ||
        length == 0
    )
    {
        return;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        const uint8_t block = (uint8_t)(0x01 + (socket * 4));
        readW5500(address, block, buffer, length);
        return;
    }

    readLinear(
        absoluteSocketAddress(socket, address),
        buffer,
        length
    );
}

void FLProgWiznetHardware::writeSocket(
    uint8_t socket,
    uint16_t address,
    uint8_t value)
{
    writeSocket(socket, address, &value, 1);
}

void FLProgWiznetHardware::writeSocket(
    uint8_t socket,
    uint16_t address,
    const uint8_t *buffer,
    uint16_t length)
{
    if (
        buffer == nullptr ||
        socket >= maxSocketNum() ||
        length == 0
    )
    {
        return;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        const uint8_t block = (uint8_t)(0x01 + (socket * 4));
        writeW5500(address, block, buffer, length);
        return;
    }

    writeLinear(
        absoluteSocketAddress(socket, address),
        buffer,
        length
    );
}

// -----------------------------------------------------------------------------
// Состояние линка и сетевые настройки
// -----------------------------------------------------------------------------

uint8_t FLProgWiznetHardware::getLinkStatus()
{
    if (!isInit())
    {
        return FLPROG_WIZNET_LINK_UNKNOWN;
    }

    uint8_t value = 0;

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        value = readCommon(0x002E);

        if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
        {
            return _cachedLinkStatus;
        }

        _cachedLinkStatus = (value & 0x01)
            ? FLPROG_WIZNET_LINK_ON
            : FLPROG_WIZNET_LINK_OFF;

        return _cachedLinkStatus;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5200)
    {
        value = readCommon(0x0035);

        if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
        {
            return _cachedLinkStatus;
        }

        _cachedLinkStatus = (value & 0x20)
            ? FLPROG_WIZNET_LINK_ON
            : FLPROG_WIZNET_LINK_OFF;

        return _cachedLinkStatus;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5100S)
    {
        value = readCommon(0x003C);

        if (_lastSpiStatus == FLPROG_SPI_STATUS_BUSY)
        {
            return _cachedLinkStatus;
        }

        _cachedLinkStatus = (value & 0x01)
            ? FLPROG_WIZNET_LINK_ON
            : FLPROG_WIZNET_LINK_OFF;

        return _cachedLinkStatus;
    }

    // Original W5100 does not expose a portable PHY-link status register.
    _cachedLinkStatus = FLPROG_WIZNET_LINK_UNKNOWN;
    return _cachedLinkStatus;
}

void FLProgWiznetHardware::setMACAddress(const uint8_t *address)
{
    if (address != nullptr && isInit())
    {
        writeCommon(REG_SHAR, address, 6);
    }
}

void FLProgWiznetHardware::getMACAddress(uint8_t *address)
{
    if (address != nullptr && isInit())
    {
        readCommon(REG_SHAR, address, 6);
    }
}

void FLProgWiznetHardware::setIPAddress(IPAddress address)
{
    uint8_t data[4] = {address[0], address[1], address[2], address[3]};
    writeCommon(REG_SIPR, data, 4);
}

IPAddress FLProgWiznetHardware::getIPAddress()
{
    uint8_t data[4] = {0, 0, 0, 0};

    if (isInit())
    {
        readCommon(REG_SIPR, data, 4);
    }

    return IPAddress(data[0], data[1], data[2], data[3]);
}

void FLProgWiznetHardware::setGatewayIp(IPAddress address)
{
    uint8_t data[4] = {address[0], address[1], address[2], address[3]};
    writeCommon(REG_GAR, data, 4);
}

IPAddress FLProgWiznetHardware::getGatewayIp()
{
    uint8_t data[4] = {0, 0, 0, 0};

    if (isInit())
    {
        readCommon(REG_GAR, data, 4);
    }

    return IPAddress(data[0], data[1], data[2], data[3]);
}

void FLProgWiznetHardware::setSubnetMask(IPAddress address)
{
    uint8_t data[4] = {address[0], address[1], address[2], address[3]};
    writeCommon(REG_SUBR, data, 4);
}

IPAddress FLProgWiznetHardware::getSubnetMask()
{
    uint8_t data[4] = {0, 0, 0, 0};

    if (isInit())
    {
        readCommon(REG_SUBR, data, 4);
    }

    return IPAddress(data[0], data[1], data[2], data[3]);
}

void FLProgWiznetHardware::setNetSettings(
    const uint8_t *mac,
    IPAddress ip,
    IPAddress gateway,
    IPAddress subnet)
{
    if (!isInit())
    {
        return;
    }

    setMACAddress(mac);
    setGatewayIp(gateway);
    setSubnetMask(subnet);
    setIPAddress(ip);
}

void FLProgWiznetHardware::setRetransmissionTime(uint16_t value)
{
    if (!isInit())
    {
        return;
    }

    const uint16_t address =
        (_chip == FLPROG_WIZNET_CHIP_W5500)
            ? 0x0019
            : 0x0017;

    uint8_t data[2] = {
        (uint8_t)(value >> 8),
        (uint8_t)(value & 0xFF)
    };

    writeCommon(address, data, 2);
}

uint16_t FLProgWiznetHardware::getRetransmissionTime()
{
    if (!isInit())
    {
        return 0;
    }

    const uint16_t address =
        (_chip == FLPROG_WIZNET_CHIP_W5500)
            ? 0x0019
            : 0x0017;

    uint8_t data[2] = {0, 0};
    readCommon(address, data, 2);

    return ((uint16_t)data[0] << 8) | data[1];
}

void FLProgWiznetHardware::setRetransmissionCount(uint8_t value)
{
    if (!isInit())
    {
        return;
    }

    const uint16_t address =
        (_chip == FLPROG_WIZNET_CHIP_W5500)
            ? 0x001B
            : 0x0019;

    writeCommon(address, value);
}

uint8_t FLProgWiznetHardware::getRetransmissionCount()
{
    if (!isInit())
    {
        return 0;
    }

    const uint16_t address =
        (_chip == FLPROG_WIZNET_CHIP_W5500)
            ? 0x001B
            : 0x0019;

    return readCommon(address);
}

// -----------------------------------------------------------------------------
// Детерминированная таблица памяти: 2 КБ на сокет
// -----------------------------------------------------------------------------

bool FLProgWiznetHardware::configureDefaultSocketMemory()
{
    if (_chip == FLPROG_WIZNET_CHIP_NONE)
    {
        return false;
    }

    clearSpiOperationStatus();

    const uint8_t sockets = maxSocketNum();

    for (uint8_t i = 0; i < 8; i++)
    {
        _txBase[i] = 0;
        _rxBase[i] = 0;
        _txMask[i] = 0;
        _rxMask[i] = 0;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5100)
    {
        // W5100 RMSR/TMSR: 01b на сокет означает 2 КБ.
        writeCommon(0x001A, 0x55);
        writeCommon(0x001B, 0x55);

        for (uint8_t socket = 0; socket < 4; socket++)
        {
            _txBase[socket] = (uint16_t)(0x4000 + (socket * 0x0800));
            _rxBase[socket] = (uint16_t)(0x6000 + (socket * 0x0800));
            _txMask[socket] = 0x07FF;
            _rxMask[socket] = 0x07FF;
        }

        return _spiOperationStatus == FLPROG_SPI_STATUS_OK;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5100S)
    {
        // Сохраняем совместимость с упакованной конфигурацией TMSR/RMSR W5100.
        writeCommon(0x001A, 0x55);
        writeCommon(0x001B, 0x55);

        for (uint8_t socket = 0; socket < 4; socket++)
        {
            _txBase[socket] = (uint16_t)(0x8000 + (socket * 0x0800));
            _rxBase[socket] = (uint16_t)(0xC000 + (socket * 0x0800));
            _txMask[socket] = 0x07FF;
            _rxMask[socket] = 0x07FF;
        }

        return _spiOperationStatus == FLPROG_SPI_STATUS_OK;
    }

    if (
        _chip == FLPROG_WIZNET_CHIP_W5200 ||
        _chip == FLPROG_WIZNET_CHIP_W5500
    )
    {
        for (uint8_t socket = 0; socket < sockets; socket++)
        {
            // 2 КБ TX и 2 КБ RX на сокет.
            writeSocket(socket, SN_RXBUF_SIZE, 2);
            writeSocket(socket, SN_TXBUF_SIZE, 2);

            _txMask[socket] = 0x07FF;
            _rxMask[socket] = 0x07FF;

            if (_chip == FLPROG_WIZNET_CHIP_W5200)
            {
                _txBase[socket] = (uint16_t)(0x8000 + (socket * 0x0800));
                _rxBase[socket] = (uint16_t)(0xC000 + (socket * 0x0800));
            }
            // W5500 использует отдельные селекторы TX/RX-блоков вместо линейных
            // абсолютных адресов памяти, поэтому base остаётся нулевым.
        }

        return _spiOperationStatus == FLPROG_SPI_STATUS_OK;
    }

    return false;
}

uint16_t FLProgWiznetHardware::txBufferBase(uint8_t socket) const
{
    return (socket < maxSocketNum()) ? _txBase[socket] : 0;
}

uint16_t FLProgWiznetHardware::rxBufferBase(uint8_t socket) const
{
    return (socket < maxSocketNum()) ? _rxBase[socket] : 0;
}

uint16_t FLProgWiznetHardware::txBufferMask(uint8_t socket) const
{
    return (socket < maxSocketNum()) ? _txMask[socket] : 0;
}

uint16_t FLProgWiznetHardware::rxBufferMask(uint8_t socket) const
{
    return (socket < maxSocketNum()) ? _rxMask[socket] : 0;
}

void FLProgWiznetHardware::readBufferPart(
    bool tx,
    uint8_t socket,
    uint16_t offset,
    uint8_t *buffer,
    uint16_t length)
{
    if (
        buffer == nullptr ||
        socket >= maxSocketNum() ||
        length == 0
    )
    {
        return;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        const uint8_t block = (uint8_t)((tx ? 0x02 : 0x03) + (socket * 4));
        readW5500(offset, block, buffer, length);
        return;
    }

    const uint16_t base = tx ? _txBase[socket] : _rxBase[socket];
    readLinear((uint16_t)(base + offset), buffer, length);
}

void FLProgWiznetHardware::writeBufferPart(
    bool tx,
    uint8_t socket,
    uint16_t offset,
    const uint8_t *buffer,
    uint16_t length)
{
    if (
        buffer == nullptr ||
        socket >= maxSocketNum() ||
        length == 0
    )
    {
        return;
    }

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        const uint8_t block = (uint8_t)((tx ? 0x02 : 0x03) + (socket * 4));
        writeW5500(offset, block, buffer, length);
        return;
    }

    const uint16_t base = tx ? _txBase[socket] : _rxBase[socket];
    writeLinear((uint16_t)(base + offset), buffer, length);
}

static uint16_t flprogNetworkMinU16(uint16_t a, uint16_t b)
{
    return (a < b) ? a : b;
}

void FLProgWiznetHardware::readTxBuffer(
    uint8_t socket,
    uint16_t offset,
    uint8_t *buffer,
    uint16_t length)
{
    if (socket >= maxSocketNum() || buffer == nullptr || length == 0)
    {
        return;
    }

    const uint16_t mask = _txMask[socket];
    uint16_t pos = offset & mask;
    uint16_t first = flprogNetworkMinU16(length, (uint16_t)((mask + 1) - pos));

    readBufferPart(true, socket, pos, buffer, first);

    if (first < length)
    {
        readBufferPart(true, socket, 0, buffer + first, length - first);
    }
}

void FLProgWiznetHardware::writeTxBuffer(
    uint8_t socket,
    uint16_t offset,
    const uint8_t *buffer,
    uint16_t length)
{
    if (socket >= maxSocketNum() || buffer == nullptr || length == 0)
    {
        return;
    }

    const uint16_t mask = _txMask[socket];
    uint16_t pos = offset & mask;
    uint16_t first = flprogNetworkMinU16(length, (uint16_t)((mask + 1) - pos));

    writeBufferPart(true, socket, pos, buffer, first);

    if (first < length)
    {
        writeBufferPart(true, socket, 0, buffer + first, length - first);
    }
}

void FLProgWiznetHardware::readRxBuffer(
    uint8_t socket,
    uint16_t offset,
    uint8_t *buffer,
    uint16_t length)
{
    if (socket >= maxSocketNum() || buffer == nullptr || length == 0)
    {
        return;
    }

    const uint16_t mask = _rxMask[socket];
    uint16_t pos = offset & mask;
    uint16_t first = flprogNetworkMinU16(length, (uint16_t)((mask + 1) - pos));

    readBufferPart(false, socket, pos, buffer, first);

    if (first < length)
    {
        readBufferPart(false, socket, 0, buffer + first, length - first);
    }
}

void FLProgWiznetHardware::writeRxBuffer(
    uint8_t socket,
    uint16_t offset,
    const uint8_t *buffer,
    uint16_t length)
{
    if (socket >= maxSocketNum() || buffer == nullptr || length == 0)
    {
        return;
    }

    const uint16_t mask = _rxMask[socket];
    uint16_t pos = offset & mask;
    uint16_t first = flprogNetworkMinU16(length, (uint16_t)((mask + 1) - pos));

    writeBufferPart(false, socket, pos, buffer, first);

    if (first < length)
    {
        writeBufferPart(false, socket, 0, buffer + first, length - first);
    }
}

// -----------------------------------------------------------------------------
// W5500 BCASTB
// -----------------------------------------------------------------------------

uint8_t FLProgWiznetHardware::prepareSocketMode(
    uint8_t mode,
    bool broadcastBlocking) const
{
    if (_chip != FLPROG_WIZNET_CHIP_W5500)
    {
        return mode;
    }

    const uint8_t protocol = mode & 0x0F;

    if (protocol != 0x02 && protocol != 0x04)
    {
        return mode;
    }

    return broadcastBlocking
        ? (uint8_t)(mode | 0x40)
        : (uint8_t)(mode & 0xBF);
}

uint8_t FLProgWiznetHardware::prepareMacRawMode(
    bool macFilterEnabled,
    bool broadcastBlocking) const
{
    uint8_t mode = 0x04; // MACRAW

    if (_chip == FLPROG_WIZNET_CHIP_W5500)
    {
        if (macFilterEnabled)
        {
            mode |= 0x80; // MFEN
        }

        return prepareSocketMode(
            mode,
            broadcastBlocking
        );
    }

    if (
        _chip == FLPROG_WIZNET_CHIP_W5100 ||
        _chip == FLPROG_WIZNET_CHIP_W5100S ||
        _chip == FLPROG_WIZNET_CHIP_W5200
    )
    {
        if (macFilterEnabled)
        {
            mode |= 0x40; // MF
        }
    }

    return mode;
}

bool FLProgWiznetHardware::setSocketBroadcastBlocking(
    uint8_t socket,
    bool value)
{
    if (
        _chip != FLPROG_WIZNET_CHIP_W5500 ||
        socket >= maxSocketNum()
    )
    {
        return false;
    }

    const uint8_t oldMode = readSocket(socket, SN_MR);
    const uint8_t protocol = oldMode & 0x0F;

    if (protocol != 0x02 && protocol != 0x04)
    {
        return false;
    }

    const uint8_t newMode =
        prepareSocketMode(oldMode, value);

    if (newMode == oldMode)
    {
        return false;
    }

    writeSocket(socket, SN_MR, newMode);
    return true;
}

void FLProgWiznetHardware::applyBroadcastBlocking(bool value)
{
    if (_chip != FLPROG_WIZNET_CHIP_W5500)
    {
        return;
    }

    for (uint8_t socket = 0; socket < maxSocketNum(); socket++)
    {
        setSocketBroadcastBlocking(socket, value);
    }
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
