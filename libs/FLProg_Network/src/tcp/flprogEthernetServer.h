/*
 * FLProg_Network — flprogEthernetServer.h
 * Назначение: Legacy FLProgEthernetServer adapter поверх FLProgNetworkTcpServer/A3.
 *
 * Архитектурные заметки:
 * - Нужен старым FLProg-проектам и встроенной Flprog_Modbus TCP библиотеке.
 * - setSourse/setPort/begin/connected/read/write/stop сохраняют старый внешний контракт.
 * - Функциональный Modbus reconnect ×3 подтверждён на dev.14-fix3.
 *
 * Комментарии описывают контракт и причины решений; рабочую логику файла не дублируют.
 * Расширенный контракт сопровождения:
 * - Legacy UDP/DNS API остается фасадом над native route-aware реализацией и не дублирует socket state.
 * - Повторные вызовы должны сохранять неблокирующую семантику SUCCESS/WAIT/ERROR, ожидаемую FLProg.
 * - Кэш, TTL и packet boundaries переводятся в compatibility слое, а не в hardware backend.
 * Extended maintenance contract:
 * - The legacy UDP/DNS API remains a facade over the native route-aware implementation and does not duplicate socket state.
 * - Repeated calls keep the non-blocking SUCCESS/WAIT/ERROR semantics expected by FLProg.
 * - Cache, TTL and packet boundaries are translated in the compatibility layer, not in the hardware backend.
 */
#pragma once

#include <Arduino.h>
#include <IPAddress.h>
#include "flprogUtilites.h"
#include "../abstract/flprogAbstractTcpInterface.h"
#include "../protocol/flprogNetworkTcpServer.h"

// -----------------------------------------------------------------------------
// FLProg_Ethernet compatibility adapter for the legacy FLProgEthernetServer.
//
// The public source-level contract is preserved, but all transport/socket work
// is delegated to FLProgNetworkTcpServer and the RC6-A A3 architecture.
// No legacy raw socket table is restored.
// -----------------------------------------------------------------------------
class FLProgEthernetServer : public Print
{
public:
    FLProgEthernetServer() = default;
    FLProgEthernetServer(FLProgAbstractTcpInterface *sourse, uint16_t port = 80)
    {
        _sourse = sourse;
        _port = port;
    }

    // Legacy spelling is intentionally preserved.
    void setSourse(FLProgAbstractTcpInterface *sourse);
    FLProgAbstractTcpInterface *getSourse() { return _sourse; }
    const FLProgAbstractTcpInterface *getSourse() const { return _sourse; }

    uint8_t pool();
    uint8_t begin();
    uint8_t setPort(uint16_t port);
    uint8_t connected();
    void stopConnection();
    void setCallback(void (*func)(void)) { _callbackFunction = func; }
    void stop();

    int available();
    int read();
    int read(uint8_t *buffer, size_t length);
    int peek();

    size_t write(uint8_t value) override;
    size_t write(const uint8_t *buffer, size_t length) override;
    void flush() override;

    void readToNull(uint16_t count);

    uint16_t localPort();
    IPAddress remoteIP();
    uint16_t remotePort();

    uint8_t getStatus() const { return _status; }
    uint8_t getError() const { return _errorCode; }
    uint8_t getErrorCode() const { return _errorCode; }

    bool getIsChangeStatus() const { return _isChangeStatus; }
    bool getIsChangeStatusWithReset();
    void setIsChangeStatus(bool value) { _isChangeStatus = value; }

    bool getIsChangeError() const { return _isChangeError; }
    bool getIsChangeErrorWithReset();
    void setIsChangeError(bool value) { _isChangeError = value; }

    uint32_t statusForExt() const { return _statusForExt; }
    bool statusForExtGetBit(uint8_t bit) const { return bitRead(_statusForExt, bit); }
    void statusForExtResetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 0); }
    bool statusForExtGetBitWithReset(uint8_t bit);
    void statusForExtSetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 1); }

    void setFlags();

    // Native implementation is exposed only for diagnostics/new code.
    FLProgNetworkTcpServer &nativeServer() { return _server; }
    const FLProgNetworkTcpServer &nativeServer() const { return _server; }
    FLProgNetworkTcpServerClient *nativeClient() { return currentClient(); }

private:
    FLProgNetworkTcpServerClient *currentClient();
    void noteNativeError();

    FLProgAbstractTcpInterface *_sourse = nullptr;
    FLProgNetworkTcpServer _server;
    uint16_t _port = 80;
    void (*_callbackFunction)(void) = nullptr;
    bool _isInit = false;

    uint8_t _status = FLPROG_NOT_REDY_STATUS;
    uint8_t _errorCode = FLPROG_NOT_ERROR;
    uint8_t _oldStatus = FLPROG_NOT_REDY_STATUS;
    bool _isChangeStatus = false;
    uint8_t _oldError = FLPROG_NOT_ERROR;
    bool _isChangeError = false;
    uint32_t _statusForExt = 1;
};
