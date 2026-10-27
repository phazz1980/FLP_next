/*
 * FLProg_Network — flprogEthernetClient.h
 * Назначение: Legacy FLProgEthernetClient adapter поверх FLProgNetworkTcpClient/A3.
 *
 * Архитектурные заметки:
 * - Сохраняет старые имена методов и числовые FLPROG_ERROR/SUCCESS/WAIT.
 * - Native result codes нельзя возвращать напрямую: compatibility слой обязан их переводить.
 * - Adapter не восстанавливает старую raw-socket таблицу.
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
#include <string.h>
#include "flprogUtilites.h"
#include "../abstract/flprogAbstractTcpInterface.h"
#include "../protocol/flprogNetworkTcpClient.h"

// -----------------------------------------------------------------------------
// FLProg_Ethernet compatibility adapter.
//
// Public goal: preserve the legacy FLProgEthernetClient source-level surface
// used by existing FLProg projects while delegating all socket/transport work
// to FLProgNetworkTcpClient and the RC6-A A3 architecture.
//
// This adapter DOES NOT restore the old raw-socket interface or socket table.
// -----------------------------------------------------------------------------
class FLProgEthernetClient : public FLProgNetworkTcpClient, public Print
{
public:
    FLProgEthernetClient() = default;
    explicit FLProgEthernetClient(FLProgAbstractTcpInterface *sourse)
    {
        init(sourse);
    }

    // Legacy spelling is intentionally preserved.
    void setSourse(FLProgAbstractTcpInterface *sourse) { init(sourse); }
    void init(FLProgAbstractTcpInterface *sourse);

    // Legacy connect contract uses Flprog_Utilites numeric values:
    // ERROR=0, SUCCESS=1, WAIT=2. Native A3 uses a different numbering,
    // therefore the adapter translates explicitly.
    int connect(IPAddress ip, uint16_t port);
    int connect(const char *host, uint16_t port);

    uint8_t connected();

    // Legacy status surface. Gate 0.2 preserves the callable API; exact mapping
    // of old Wiznet SnSR values is intentionally deferred to the semantic gate.
    uint8_t status() const { return state(); }
    uint8_t getStatus() const { return _compatStatus; }

    uint8_t getError() const;
    uint8_t getErrorCode() const { return getError(); }

    uint8_t getSocketNumber() const;

    void setConnectionTimeout(uint16_t timeout)
    {
        _legacyConnectionTimeout = timeout;
        setTimeout(timeout);
    }
    uint16_t getConnectionTimeout() const { return _legacyConnectionTimeout; }

    void setDnsCacheStorageTime(uint32_t time);
    uint32_t getDnsCacheStorageTime() const { return _legacyDnsCacheStorageTime; }

    FLProgAbstractTcpInterface *getSourse() { return _sourse; }
    const FLProgAbstractTcpInterface *getSourse() const { return _sourse; }

    void stop();

    size_t write(uint8_t value) override
    {
        return FLProgNetworkTcpClient::write(value);
    }

    size_t write(const uint8_t *buffer, size_t length) override
    {
        return FLProgNetworkTcpClient::write(buffer, length);
    }

    void flush() override { FLProgNetworkTcpClient::flush(); }

    int available() { return FLProgNetworkTcpClient::available(); }
    int read() { return FLProgNetworkTcpClient::read(); }
    int read(uint8_t *buffer, size_t length)
    {
        return FLProgNetworkTcpClient::read(buffer, length);
    }
    int peek() { return FLProgNetworkTcpClient::peek(); }

    void readToNull(uint16_t count)
    {
        while (count > 0 && available() > 0)
        {
            if (read() < 0) break;
            --count;
        }
    }

    uint16_t localPort() const { return FLProgNetworkTcpClient::localPort(); }
    IPAddress remoteIP() const { return FLProgNetworkTcpClient::remoteIP(); }
    uint16_t remotePort() const { return FLProgNetworkTcpClient::remotePort(); }

    // Legacy change flags used by generated blocks and diagnostics.
    bool getIsChangeStatus() const { return _isChangeStatus; }
    bool getIsChangeStatusWithReset()
    {
        const bool value = _isChangeStatus;
        _isChangeStatus = false;
        return value;
    }
    void setIsChangeStatus(bool value) { _isChangeStatus = value; }

    bool getIsChangeError() const { return _isChangeError; }
    bool getIsChangeErrorWithReset()
    {
        const bool value = _isChangeError;
        _isChangeError = false;
        return value;
    }
    void setIsChangeError(bool value) { _isChangeError = value; }

    uint32_t statusForExt() const { return _statusForExt; }
    bool statusForExtGetBit(uint8_t bit) const { return bitRead(_statusForExt, bit); }
    void statusForExtResetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 0); }
    bool statusForExtGetBitWithReset(uint8_t bit)
    {
        const bool value = bitRead(_statusForExt, bit);
        bitWrite(_statusForExt, bit, 0);
        return value;
    }
    void statusForExtSetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 1); }

    void setFlags();

    // Native implementation remains reachable for new code and diagnostics.
    FLProgNetworkTcpClient &nativeClient() { return *this; }
    const FLProgNetworkTcpClient &nativeClient() const { return *this; }

private:
    void applyDnsCachePolicy();
    int translateCurrentResult(bool dnsContext);
    uint8_t translateLegacyError() const;
    bool sameIpTarget(IPAddress ip, uint16_t port) const;
    bool sameHostTarget(const char *host, uint16_t port) const;
    void rememberIpTarget(IPAddress ip, uint16_t port);
    void rememberHostTarget(const char *host, uint16_t port);

    FLProgAbstractTcpInterface *_sourse = nullptr;
    bool _isInit = false;
    uint16_t _legacyConnectionTimeout = 20000;
    uint32_t _legacyDnsCacheStorageTime = 60000UL;
    uint8_t _compatStatus = FLPROG_READY_STATUS;
    uint8_t _compatError = FLPROG_NOT_ERROR;

    bool _legacyTargetValid = false;
    bool _legacyTargetHost = false;
    IPAddress _legacyTargetIp = IPAddress(0, 0, 0, 0);
    uint16_t _legacyTargetPort = 0;
    char _legacyTargetHostName[FLPROG_TCP_HOSTNAME_MAX_LENGTH + 1] = {0};

    uint8_t _oldCompatStatus = FLPROG_READY_STATUS;
    uint8_t _oldCompatError = FLPROG_NOT_ERROR;
    bool _isChangeStatus = false;
    bool _isChangeError = false;
    uint32_t _statusForExt = 1;
};
