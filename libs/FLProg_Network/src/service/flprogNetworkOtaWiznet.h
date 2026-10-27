/*
 * W5500 route-aware Arduino espota receiver for ESP32.
 * This is separate from ArduinoOTA: ArduinoOTA's NetworkUDP/NetworkClient
 * sockets use lwIP and do not route through the FLProg_Network W5500 backend.
 */
#pragma once

#include <Arduino.h>
#include "flprogNetworkService.h"
#include "../flprogNetworkProfile.h"

class FLProgNetworkInterface;
class FLProgNetworkUdp;
class FLProgNetworkTcpClient;

#ifndef FLPROG_NETWORK_COMPILE_WIZNET_OTA
// Compile only where ESP32 + a wired backend are available. Link-time section
// garbage collection removes the implementation when the service is unused.
#if defined(ESP32) && !FLPROG_NETWORK_PROFILE_WIFI_ONLY && FLPROG_NETWORK_COMPILE_WIRED
#define FLPROG_NETWORK_COMPILE_WIZNET_OTA 1
#else
#define FLPROG_NETWORK_COMPILE_WIZNET_OTA 0
#endif
#endif

class FLProgNetworkOtaWiznet : public FLProgNetworkService
{
public:
    FLProgNetworkOtaWiznet();
    ~FLProgNetworkOtaWiznet() override;

    bool begin(FLProgNetworkInterface &network, const char *password, uint16_t port = 3232);
    void end();
    void pool() override;

    bool supported() const { return FLPROG_NETWORK_COMPILE_WIZNET_OTA != 0; }
    bool ready() const;
    bool updating() const { return _updating; }
    uint8_t error() const { return _error; }
    uint32_t updateStartCount() const { return _updateStartCount; }
    uint32_t updateEndCount() const { return _updateEndCount; }
    uint32_t updateErrorCount() const { return _updateErrorCount; }
    uint32_t bytesReceived() const { return _bytesReceived; }

private:
    bool receiveInvitation();
    bool verifyAuth(const char *cnonce, const char *response);
    void fail(uint8_t code);
    void abortUpdate();
    void streamFirmware();

    FLProgNetworkUdp *_udp;
    FLProgNetworkTcpClient *_client;
    char _password[64];
    char _nonce[65];
    char _expectedMd5[33];
    IPAddress _host;
    uint16_t _hostPort;
    uint16_t _listenPort;
    uint32_t _imageSize;
    uint32_t _bytesReceived;
    uint32_t _updateStartCount;
    uint32_t _updateEndCount;
    uint32_t _updateErrorCount;
    uint32_t _restartAt;
    uint8_t _state;
    uint8_t _error;
    bool _ready;
    bool _updating;
    bool _connectRequested;
};
