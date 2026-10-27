/*
 * FLProg_Network — flprogNetworkOpticalStub.h
 *
 * Public, inert configuration scaffold for a future optical transport.
 * It deliberately contains no PHY driver, no pin assumptions, no speed or
 * framing decision and is not registered in the transport manager. Therefore
 * merely including the current library cannot affect Wi-Fi, W5500 or RMII.
 * Расширенный контракт сопровождения:
 * - Transport описывает availability, identity и generation, а не прикладной TCP/HTTP policy.
 * - Реальная смена link/readiness меняет generation один раз и уведомляет socket layer без каскадных ложных reset.
 * - Stub остается inert: включение заголовка не регистрирует backend и не меняет поведение существующей сети.
 * Extended maintenance contract:
 * - Transport describes availability, identity and generation, not application TCP/HTTP policy.
 * - A real link/readiness change increments generation once and notifies the socket layer without cascaded false resets.
 * - A stub stays inert: including its header neither registers a backend nor changes existing network behavior.
 */
#pragma once

#include <Arduino.h>
#include "flprogNetworkTransport.h"

#ifndef FLPROG_NETWORK_ENABLE_OPTICAL_STUB
#define FLPROG_NETWORK_ENABLE_OPTICAL_STUB 0
#endif

#ifndef FLPROG_OPTICAL_DEFAULT_MTU
#define FLPROG_OPTICAL_DEFAULT_MTU 1500U
#endif

enum FLProgOpticalTransportMode : uint8_t
{
    FLPROG_OPTICAL_MODE_DISABLED = 0,
    FLPROG_OPTICAL_MODE_ETHERNET_PHY = 1,
    FLPROG_OPTICAL_MODE_PACKET_LINK = 2
};

// Configuration is retained only for the future backend integration gate.
// Calling these methods performs no I/O and never creates a network route.
class FLProgNetworkOpticalStub
{
public:
    void setEnabled(bool value) { _enabled = value; }
    bool enabled() const { return _enabled; }

    void setMode(FLProgOpticalTransportMode value) { _mode = value; }
    FLProgOpticalTransportMode mode() const { return _mode; }

    void setMtu(uint16_t value) { _mtu = value; }
    uint16_t mtu() const { return _mtu; }

    bool available() const { return false; }
    bool ready() const { return false; }
    bool supported() const { return false; }
    FLProgTransportIdentity identity() const
    {
        FLProgTransportIdentity result;
        result.kind = FLPROG_TRANSPORT_KIND_OPTICAL;
        result.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
        return result;
    }

private:
    bool _enabled = false;
    FLProgOpticalTransportMode _mode = FLPROG_OPTICAL_MODE_DISABLED;
    uint16_t _mtu = FLPROG_OPTICAL_DEFAULT_MTU;
};
