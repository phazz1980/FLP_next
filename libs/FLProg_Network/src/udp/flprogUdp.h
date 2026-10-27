/*
 * FLProg_Network — flprogUdp.h
 * Назначение: Legacy FLProgUdpClient adapter поверх FLProgNetworkUdp/A3.
 *
 * Архитектурные заметки:
 * - Сохраняет старые begin/beginPacket/endPacket/read/write и change-flags.
 * - DNS WAIT/BUSY/SUCCESS/ERROR переводятся в legacy control-status/result codes.
 * - Это Gate 0.3: сначала compile/API, затем отдельный semantic UDP echo test.
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
#include "../protocol/flprogNetworkUdp.h"
#include "../protocol/flprogNetworkDns.h"

// -----------------------------------------------------------------------------
// FLProg_Ethernet compatibility adapter for legacy FLProgUdpClient.
//
// Public goal: preserve the old source-level UDP surface used by generated
// FLProg projects while delegating all transport/socket work to
// FLProgNetworkUdp and the RC6-A A3 architecture.
//
// The legacy raw-socket table is NOT restored.
// -----------------------------------------------------------------------------

#ifndef FLPROG_UDP_TX_PACKET_MAX_SIZE
#define FLPROG_UDP_TX_PACKET_MAX_SIZE 24
#endif

#ifndef FLPROG_UDP_TIMED_OUT
#define FLPROG_UDP_TIMED_OUT 4
#endif

class FLProgUdpClient : public FLProgNetworkUdp, public Print
{
public:
    FLProgUdpClient() = default;
    explicit FLProgUdpClient(FLProgAbstractTcpInterface *sourse)
    {
        setSourse(sourse);
    }

    // Legacy spelling is intentionally preserved.
    void setSourse(FLProgAbstractTcpInterface *sourse);
    FLProgAbstractTcpInterface *getSourse() { return _sourse; }
    const FLProgAbstractTcpInterface *getSourse() const { return _sourse; }

    uint8_t begin(uint16_t port);
    uint8_t beginMulticast(IPAddress ip, uint16_t port);

    int beginIpPacket(IPAddress ip, uint16_t port);
    int beginPacket(IPAddress ip, uint16_t port)
    {
        return beginIpPacket(ip, port);
    }
    int beginPacket(const char *host, uint16_t port);

    size_t write(uint8_t value) override
    {
        return FLProgNetworkUdp::write(value);
    }

    size_t write(const uint8_t *buffer, size_t size) override
    {
        return FLProgNetworkUdp::write(buffer, size);
    }

    using Print::write;

    int endPacket();
    int parsePacket();

    int available() { return FLProgNetworkUdp::available(); }
    int read() { return FLProgNetworkUdp::read(); }
    int read(uint8_t *buffer, size_t len)
    {
        return FLProgNetworkUdp::read(buffer, len);
    }
    int peek() { return FLProgNetworkUdp::peek(); }

    // Old FLProg UDP flush() was effectively a no-op.
    void flush() override {}

    void stop();

    IPAddress remoteIP() { return FLProgNetworkUdp::remoteIP(); }
    uint16_t remotePort() { return FLProgNetworkUdp::remotePort(); }
    uint16_t localPort() { return _port; }

    void setDnsCacheStorageTime(uint32_t time);
    uint32_t getDnsCacheStorageTime() const
    {
        return _legacyDnsCacheStorageTime;
    }

    uint8_t getStatus() const { return _compatStatus; }
    uint8_t getError() const { return _compatError; }
    uint8_t getErrorCode() const { return _compatError; }

    void readToNull(uint16_t count)
    {
        while (count > 0 && available() > 0)
        {
            if (read() < 0)
            {
                break;
            }
            --count;
        }
    }

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
    bool statusForExtGetBit(uint8_t bit) const
    {
        return bitRead(_statusForExt, bit);
    }
    void statusForExtResetBit(uint8_t bit)
    {
        bitWrite(_statusForExt, bit, 0);
    }
    bool statusForExtGetBitWithReset(uint8_t bit)
    {
        const bool value = bitRead(_statusForExt, bit);
        bitWrite(_statusForExt, bit, 0);
        return value;
    }
    void statusForExtSetBit(uint8_t bit)
    {
        bitWrite(_statusForExt, bit, 1);
    }

    void setFlags();

    FLProgNetworkUdp &nativeUdp() { return *this; }
    const FLProgNetworkUdp &nativeUdp() const { return *this; }

private:
    void applyDnsCachePolicy();

    FLProgAbstractTcpInterface *_sourse = nullptr;
    bool _nativeBegun = false;
    uint16_t _port = 0;

    uint32_t _legacyDnsCacheStorageTime = 60000UL;

    uint8_t _compatStatus = FLPROG_NOT_REDY_STATUS;
    uint8_t _compatError = FLPROG_NOT_ERROR;

    uint8_t _oldCompatStatus = FLPROG_NOT_REDY_STATUS;
    uint8_t _oldCompatError = FLPROG_NOT_ERROR;
    bool _isChangeStatus = false;
    bool _isChangeError = false;
    uint32_t _statusForExt = 1;
};
