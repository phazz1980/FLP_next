/*
 * FLProg_Network — flprogSpiBackend.cpp
 * Назначение: Платформенно-зависимый SPI backend для Wiznet/W5500.
 *
 * Архитектурные заметки:
 * - Выбирает SPI/SPI1/HSPI и проверяет доступность пользовательских пинов.
 * - Переназначение SPI-пинов включается только при наличии API выбранного Arduino core.
 *   STM32duino использует setSCLK/setMISO/setMOSI, RP-core-ы — setSCK/setRX/setTX.
 * - beginTransaction/endTransaction обязаны держать CS и SPI transaction симметрично.
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
#include "flprogSpiBackend.h"
#include <type_traits>
#include <utility>

// RP cores expose more than one SPI API. The Earle Philhower core provides
// setSCK()/setRX()/setTX(), while Arduino Mbed RP2040 uses HardwareSPI and
// does not. Detect the optional pin-remap surface at compile time instead of
// assuming it from ARDUINO_ARCH_RP2040 alone.
// RP Arduino cores are not API-identical. Some provide setSCK/setRX/setTX,
// while FLProg: RP2040 W uses arduino::HardwareSPI without those methods.
// Detect capability at compile time instead of assuming a particular core.
template <typename T>
class FLProgSpiHasRpPinRemap
{
private:
    template <typename U>
    static auto test(int) -> decltype(
        std::declval<U &>().setSCK(0),
        std::declval<U &>().setRX(0),
        std::declval<U &>().setTX(0),
        std::true_type());

    template <typename>
    static std::false_type test(...);

public:
    static const bool value = decltype(test<T>(0))::value;
};

// STM32duino exposes pin remapping through setSCLK()/setMISO()/setMOSI().
// Keep this optional surface compile-time detected so other Arduino cores do
// not need to provide STM32-specific SPI methods.
template <typename T>
class FLProgSpiHasStm32PinRemap
{
private:
    template <typename U>
    static auto test(int) -> decltype(
        std::declval<U &>().setSCLK(0),
        std::declval<U &>().setMISO(0),
        std::declval<U &>().setMOSI(0),
        std::true_type());

    template <typename>
    static std::false_type test(...);

public:
    static const bool value = decltype(test<T>(0))::value;
};

template <typename T>
typename std::enable_if<FLProgSpiHasRpPinRemap<T>::value, bool>::type
flprogApplyRpSpiPins(T *spi, int sck, int miso, int mosi)
{
    if (sck >= 0)
    {
        spi->setSCK(sck);
    }
    if (miso >= 0)
    {
        spi->setRX(miso);
    }
    if (mosi >= 0)
    {
        spi->setTX(mosi);
    }
    return true;
}

template <typename T>
typename std::enable_if<!FLProgSpiHasRpPinRemap<T>::value, bool>::type
flprogApplyRpSpiPins(T *, int, int, int)
{
    return false;
}

template <typename T>
typename std::enable_if<FLProgSpiHasStm32PinRemap<T>::value, bool>::type
flprogApplyStm32SpiPins(T *spi, int sck, int miso, int mosi)
{
    if (sck >= 0)
    {
        spi->setSCLK(sck);
    }
    if (miso >= 0)
    {
        spi->setMISO(miso);
    }
    if (mosi >= 0)
    {
        spi->setMOSI(mosi);
    }
    return true;
}

template <typename T>
typename std::enable_if<!FLProgSpiHasStm32PinRemap<T>::value, bool>::type
flprogApplyStm32SpiPins(T *, int, int, int)
{
    return false;
}

#if defined(ESP32) && defined(HSPI)
static SPIClass flprogNetworkBackendSecondSpi(HSPI);
#endif

FLProgArduinoSpiBackend::FLProgArduinoSpiBackend()
{
}

void FLProgArduinoSpiBackend::configure(
    uint8_t requestedBus,
    int requestedCs,
    uint32_t speed)
{
    _bus = (requestedBus == 255) ? 0 : requestedBus;
    _cs = requestedCs;
    _speed = speed;
    _status = FLPROG_SPI_STATUS_OK;
}

// Resolve a logical bus number to the SPI object supplied by the selected
// Arduino core. Unsupported secondary buses return nullptr explicitly.
SPIClass *FLProgArduinoSpiBackend::resolveSpiObject()
{
    if (_spiObject != nullptr)
    {
        return _spiObject;
    }

    if (_bus == 0)
    {
        return &SPI;
    }

#if defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
    if (_bus == 1)
    {
        return &SPI1;
    }
#endif

#if defined(ESP32) && defined(HSPI)
    if (_bus == 1)
    {
        return &flprogNetworkBackendSecondSpi;
    }
#endif

    return nullptr;
}

SPIClass *FLProgArduinoSpiBackend::spiObject()
{
    return resolveSpiObject();
}

bool FLProgArduinoSpiBackend::busAvailable() const
{
    if (_spiObject != nullptr)
    {
        return true;
    }

    if (_bus == 0)
    {
        return true;
    }

#if defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
    if (_bus == 1)
    {
        return true;
    }
#endif

#if defined(ESP32) && defined(HSPI)
    if (_bus == 1)
    {
        return true;
    }
#endif

    return false;
}

// Custom pin remap is a capability, not a platform-name promise. ESP32 uses
// begin(sck,miso,mosi); STM32 and RP use their detected core-specific methods.
bool FLProgArduinoSpiBackend::pinsSupported() const
{
    if (!_customPins)
    {
        return true;
    }

#if defined(ESP32)
    return true;
#elif defined(ARDUINO_ARCH_STM32)
    return FLProgSpiHasStm32PinRemap<SPIClass>::value;
#elif defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
    return FLProgSpiHasRpPinRemap<SPIClass>::value;
#else
    return false;
#endif
}

void FLProgArduinoSpiBackend::setPins(int sck, int miso, int mosi)
{
    _sck = sck;
    _miso = miso;
    _mosi = mosi;
    _customPins = true;
}

void FLProgArduinoSpiBackend::clearPins()
{
    _sck = -1;
    _miso = -1;
    _mosi = -1;
    _customPins = false;
}

// Configure the physical SPI object but do not open a chip transaction yet.
// CS is left HIGH so compile/configuration stages cannot accidentally select W5500.
bool FLProgArduinoSpiBackend::begin()
{
    if (!busAvailable())
    {
        _status = FLPROG_SPI_STATUS_UNAVAILABLE;
        return false;
    }

    if (!pinsSupported())
    {
        _status = FLPROG_SPI_STATUS_PIN_ERROR;
        return false;
    }

    if (_cs < 0)
    {
        _status = FLPROG_SPI_STATUS_UNAVAILABLE;
        return false;
    }

    SPIClass *spi = resolveSpiObject();

    if (spi == nullptr)
    {
        _status = FLPROG_SPI_STATUS_UNAVAILABLE;
        return false;
    }

#if defined(ESP32)
    if (_customPins)
    {
        spi->begin(_sck, _miso, _mosi, -1);
    }
    else
    {
        spi->begin();
    }
#elif defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
    if (_customPins && !flprogApplyRpSpiPins(spi, _sck, _miso, _mosi))
    {
        _status = FLPROG_SPI_STATUS_PIN_ERROR;
        return false;
    }

    spi->begin();
#elif defined(ARDUINO_ARCH_STM32)
    if (_customPins && !flprogApplyStm32SpiPins(spi, _sck, _miso, _mosi))
    {
        _status = FLPROG_SPI_STATUS_PIN_ERROR;
        return false;
    }

    spi->begin();
#else
    spi->begin();
#endif

    pinMode(_cs, OUTPUT);
    digitalWrite(_cs, HIGH);

    _status = FLPROG_SPI_STATUS_OK;
    return true;
}

// Every successful beginTransaction() lowers CS exactly once and records an
// open transaction. endTransaction() is the only normal path that raises it.
bool FLProgArduinoSpiBackend::beginTransaction()
{
    SPIClass *spi = resolveSpiObject();

    if (spi == nullptr || _cs < 0)
    {
        _status = FLPROG_SPI_STATUS_UNAVAILABLE;
        return false;
    }

    spi->beginTransaction(
        SPISettings(
            _speed,
            MSBFIRST,
            SPI_MODE0
        )
    );

    digitalWrite(_cs, LOW);
    _transactionOpen = true;
    _status = FLPROG_SPI_STATUS_OK;

    return true;
}

void FLProgArduinoSpiBackend::endTransaction()
{
    if (!_transactionOpen)
    {
        return;
    }

    SPIClass *spi = resolveSpiObject();

    digitalWrite(_cs, HIGH);

    if (spi != nullptr)
    {
        spi->endTransaction();
    }

    _transactionOpen = false;
}

uint8_t FLProgArduinoSpiBackend::transfer(uint8_t value)
{
    SPIClass *spi = resolveSpiObject();

    if (spi == nullptr)
    {
        _status = FLPROG_SPI_STATUS_UNAVAILABLE;
        return 0xFF;
    }

    return spi->transfer(value);
}

#endif // FLPROG_NETWORK_COMPILE_WIRED
