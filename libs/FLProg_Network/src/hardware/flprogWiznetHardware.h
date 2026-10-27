/*
 * FLProg_Network — flprogWiznetHardware.h
 * Назначение: Низкоуровневый драйвер Wiznet/W5500 для FLProg_Network.
 *
 * Архитектурные заметки:
 * - Отвечает за регистры чипа, socket-команды и аппаратные параметры Ethernet.
 * - Верхние TCP/UDP классы работают через A3/backend и не должны знать регистровую модель Wiznet.
 * - Все hardware-specific ошибки должны подниматься наверх без ложной смены Wi-Fi transport.
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
#include <IPAddress.h>
#include <SPI.h>
#include "flprogSpiArbiter.h"
#include "flprogSpiBackend.h"

// -----------------------------------------------------------------------------
// FLProg_Network v0.4.5 - единый аппаратный слой Wiznet.
//
// Поддерживаемые микросхемы:
//   W5100   - 4 sockets, 8 KB TX + 8 KB RX
//   W5100S  - 4 sockets, 8 KB TX + 8 KB RX
//   W5200   - 8 sockets, 16 KB TX + 16 KB RX
//   W5500   - 8 sockets, 16 KB TX + 16 KB RX
//
// Библиотека самостоятельно формирует SPI-кадры для каждого семейства.
// FLProg_Ethernet и Arduino Ethernet не подключаются; native DHCP/DNS/UDP
// работают напрямую через этот аппаратный слой.
// -----------------------------------------------------------------------------

#define FLPROG_WIZNET_CHIP_NONE   0
#define FLPROG_WIZNET_CHIP_W5100  51
#define FLPROG_WIZNET_CHIP_W5200  52
#define FLPROG_WIZNET_CHIP_W5100S 53
#define FLPROG_WIZNET_CHIP_W5500  55

#define FLPROG_WIZNET_LINK_UNKNOWN 0
#define FLPROG_WIZNET_LINK_ON      1
#define FLPROG_WIZNET_LINK_OFF     2

#define FLPROG_WIZNET_HW_WAIT  0
#define FLPROG_WIZNET_HW_READY 1
#define FLPROG_WIZNET_HW_ERROR 2

class FLProgWiznetHardware
{
public:
    FLProgWiznetHardware();
    FLProgWiznetHardware(int pinCs, uint8_t bus = 255);

    // Выбор аппаратуры ------------------------------------------------------
    void setPinCs(int pinCs);
    int pinCs() const { return _pinCs; }

    // 255/0 = основная аппаратная SPI.
    // 1     = вторая аппаратная SPI, если она доступна на платформе.
    void setSpiBus(uint8_t bus);
    uint8_t spiBus() const { return _spiBus; }
    bool spiBusAvailable() const;

    // 14 МГц выбраны консервативно и совместимы с самым старым W5100.
    // При длинных или шумных линиях пользователь может снизить частоту.
    void setSpiSpeed(uint32_t speed);
    uint32_t spiSpeed() const { return _spiSpeed; }

    // SPI backend selection. nullptr restores the standalone Arduino backend.
    void setSpiBackend(FLProgSpiBackend *backend);
    void useArduinoSpiBackend();
    FLProgSpiBackend *spiBackend() { return _spiBackend; }
    const FLProgSpiBackend *spiBackend() const { return _spiBackend; }
    uint8_t spiBackendKind() const;
    uint8_t effectiveSpiBus() const;
    int effectivePinCs() const;

    // Необязательный явный объект SPI. Если он задан, он заменяет объект шины,
    // выбранный setSpiBus(), а номер шины продолжает использоваться
    // как ключ арбитража.
    void setSpiObject(SPIClass *object);
    void clearSpiObject();
    SPIClass *spiObject();
    bool hasExplicitSpiObject() const { return _spiObject != nullptr; }

    // Необязательные пользовательские выводы аппаратного SPI. Они задаются до begin().
    // ESP32 и Arduino-Pico поддерживают смену выводов; неподдерживаемые core
    // возвращают FLPROG_SPI_STATUS_PIN_ERROR, а не игнорируют настройку.
    void setSpiPins(int sck, int miso, int mosi);
    void clearSpiPins();
    bool customSpiPins() const { return _customSpiPins; }
    bool spiPinsSupported() const;

    // Кооперативный арбитраж. Занятая шина никогда не блокирует цикл FLProg.
    bool acquireSpi();
    void releaseSpi();
    bool spiBusy() const;

    // Общий Busy-флаг полезен, если другая библиотека не умеет вызывать наш арбитр.
    // FLProg_Network устанавливает флаг на время владения шиной и откладывает
    // транзакцию, если флаг уже установлен другим устройством.
    void setSharedSpiBusyFlag(volatile bool *flag) { _sharedBusyFlag = flag; }
    volatile bool *sharedSpiBusyFlag() const { return _sharedBusyFlag; }

    // Необязательные callbacks общего lock-механизма проекта (RT_HW_Base, RTOS mutex и т.п.).
    void setSpiLockCallbacks(
        FLProgSpiLockCallback lockCallback,
        FLProgSpiUnlockCallback unlockCallback,
        void *context = nullptr);

    uint8_t lastSpiStatus() const { return _lastSpiStatus; }
    uint8_t spiOperationStatus() const { return _spiOperationStatus; }
    void clearSpiOperationStatus() { _spiOperationStatus = FLPROG_SPI_STATUS_OK; }
    uint32_t spiConflictCount() const;
    uint32_t spiTransactionCount() const;
    uint32_t spiDeferredCount() const { return _spiDeferredCount; }
    void resetSpiStatistics();

    // Обнаружение / возможности -----------------------------------------------
    uint8_t init();
    void resetInit();

    bool isInit() const { return _initState == 2 && _chip != FLPROG_WIZNET_CHIP_NONE; }
    uint8_t getChip() const { return _chip; }
    uint8_t chipCode() const { return _chip; }
    const char *chipName() const;

    uint8_t maxSocketNum() const;
    uint16_t txMemoryTotal() const;
    uint16_t rxMemoryTotal() const;

    uint8_t getLinkStatus();
    bool linkIsUp() { return getLinkStatus() == FLPROG_WIZNET_LINK_ON; }

    // Доступ к общим регистрам --------------------------------------------------
    uint8_t readCommon(uint16_t address);
    void readCommon(uint16_t address, uint8_t *buffer, uint16_t length);
    void writeCommon(uint16_t address, uint8_t value);
    void writeCommon(uint16_t address, const uint8_t *buffer, uint16_t length);

    // Observe and clear common IR.CONFLICT cooperatively.
    bool takeIpConflictFlag();

    // Доступ к регистрам сокета. "address" — смещение внутри сокета N.
    uint8_t readSocket(uint8_t socket, uint16_t address);
    void readSocket(uint8_t socket, uint16_t address, uint8_t *buffer, uint16_t length);
    void writeSocket(uint8_t socket, uint16_t address, uint8_t value);
    void writeSocket(uint8_t socket, uint16_t address, const uint8_t *buffer, uint16_t length);

    // Общая настройка сети -------------------------------------------
    void setMACAddress(const uint8_t *address);
    void getMACAddress(uint8_t *address);

    void setIPAddress(IPAddress address);
    IPAddress getIPAddress();

    void setGatewayIp(IPAddress address);
    IPAddress getGatewayIp();

    void setSubnetMask(IPAddress address);
    IPAddress getSubnetMask();

    void setNetSettings(
        const uint8_t *mac,
        IPAddress ip,
        IPAddress gateway,
        IPAddress subnet);

    // RTR на всех поддерживаемых микросхемах задаётся в единицах 100 мкс.
    void setRetransmissionTime(uint16_t value);
    uint16_t getRetransmissionTime();

    void setRetransmissionCount(uint8_t value);
    uint8_t getRetransmissionCount();

    // Профиль памяти сокетов ---------------------------------------------------
    //
    // v0.4.2 стандартизует начальную конфигурацию: 2 КБ TX + 2 КБ RX на сокет.
    // Такая схема подходит всем поддерживаемым микросхемам и даёт будущему слою сокетов
    // детерминированную таблицу памяти.
    bool configureDefaultSocketMemory();
    uint16_t txBufferBase(uint8_t socket) const;
    uint16_t rxBufferBase(uint8_t socket) const;
    uint16_t txBufferMask(uint8_t socket) const;
    uint16_t rxBufferMask(uint8_t socket) const;

    void readTxBuffer(uint8_t socket, uint16_t offset, uint8_t *buffer, uint16_t length);
    void writeTxBuffer(uint8_t socket, uint16_t offset, const uint8_t *buffer, uint16_t length);
    void readRxBuffer(uint8_t socket, uint16_t offset, uint8_t *buffer, uint16_t length);
    void writeRxBuffer(uint8_t socket, uint16_t offset, const uint8_t *buffer, uint16_t length);

    // BCASTB только для W5500 ---------------------------------------------------
    //
    // W5100/W5100S/W5200 используют бит 6 с другой семантикой MAC-фильтра.
    // Поэтому Broadcast Blocking намеренно изменяется ТОЛЬКО на W5500.
    uint8_t prepareSocketMode(uint8_t mode, bool broadcastBlocking) const;

    // MACRAW filter bits are chip-specific:
    // W5500 uses MFEN bit 7, while W5100/W5100S/W5200 use MF bit 6.
    // BCASTB remains a separate W5500-only policy.
    uint8_t prepareMacRawMode(
        bool macFilterEnabled,
        bool broadcastBlocking) const;

    bool setSocketBroadcastBlocking(uint8_t socket, bool value);
    void applyBroadcastBlocking(bool value);

private:
    // Common addresses shared by W5100-family/W5200/W5500 where applicable.
    static const uint16_t REG_GAR  = 0x0001;
    static const uint16_t REG_SUBR = 0x0005;
    static const uint16_t REG_SHAR = 0x0009;
    static const uint16_t REG_SIPR = 0x000F;
    static const uint16_t REG_IR   = 0x0015;

    static const uint16_t SN_MR         = 0x0000;
    static const uint16_t SN_RXBUF_SIZE = 0x001E;
    static const uint16_t SN_TXBUF_SIZE = 0x001F;

    uint8_t normalizedBus() const;
    void syncSpiBackendConfiguration();
    bool beginBus();
    bool beginTransaction();
    void endTransaction();
    uint8_t transfer(uint8_t value);

    // Обнаружение микросхемы.
    uint8_t detectChip();
    bool probeW5100();

    // W5500: ADDR_H, ADDR_L, CONTROL, DATA...
    void readW5500(uint16_t address, uint8_t block, uint8_t *buffer, uint16_t length);
    void writeW5500(uint16_t address, uint8_t block, const uint8_t *buffer, uint16_t length);

    // W5200: ADDR_H, ADDR_L, OP+LEN_H, LEN_L, DATA...
    void readW5200(uint16_t address, uint8_t *buffer, uint16_t length);
    void writeW5200(uint16_t address, const uint8_t *buffer, uint16_t length);

    // W5100S uses OP(0x0F/0xF0), address and sequential N-byte data.
    void readW5100S(uint16_t address, uint8_t *buffer, uint16_t length);
    void writeW5100S(uint16_t address, const uint8_t *buffer, uint16_t length);

    // Original W5100 transfers one data byte per complete 32-bit SPI frame.
    void readW5100(uint16_t address, uint8_t *buffer, uint16_t length);
    void writeW5100(uint16_t address, const uint8_t *buffer, uint16_t length);

    uint16_t absoluteSocketAddress(uint8_t socket, uint16_t offset) const;

    // Доступ к буферу без обработки перехода через границу.
    void readLinear(uint16_t address, uint8_t *buffer, uint16_t length);
    void writeLinear(uint16_t address, const uint8_t *buffer, uint16_t length);
    void readBufferPart(bool tx, uint8_t socket, uint16_t offset, uint8_t *buffer, uint16_t length);
    void writeBufferPart(bool tx, uint8_t socket, uint16_t offset, const uint8_t *buffer, uint16_t length);

    int _pinCs = -1;
    uint8_t _spiBus = 255;
    uint32_t _spiSpeed = 14000000UL;

    FLProgArduinoSpiBackend _arduinoSpiBackend;
    FLProgSpiBackend *_spiBackend = &_arduinoSpiBackend;

    SPIClass *_spiObject = nullptr;
    int _spiSckPin = -1;
    int _spiMisoPin = -1;
    int _spiMosiPin = -1;
    bool _customSpiPins = false;

    volatile bool *_sharedBusyFlag = nullptr;
    FLProgSpiLockCallback _lockCallback = nullptr;
    FLProgSpiUnlockCallback _unlockCallback = nullptr;
    void *_lockContext = nullptr;
    bool _externalLockHeld = false;
    bool _sharedBusyHeld = false;

    uint8_t _lastSpiStatus = FLPROG_SPI_STATUS_OK;
    uint8_t _spiOperationStatus = FLPROG_SPI_STATUS_OK;
    uint32_t _spiDeferredCount = 0;
    uint8_t _cachedLinkStatus = FLPROG_WIZNET_LINK_UNKNOWN;

    uint8_t _chip = FLPROG_WIZNET_CHIP_NONE;
    uint8_t _initState = 0;
    uint32_t _initStarted = 0;

    uint16_t _txBase[8] = {0,0,0,0,0,0,0,0};
    uint16_t _rxBase[8] = {0,0,0,0,0,0,0,0};
    uint16_t _txMask[8] = {0,0,0,0,0,0,0,0};
    uint16_t _rxMask[8] = {0,0,0,0,0,0,0,0};
};
