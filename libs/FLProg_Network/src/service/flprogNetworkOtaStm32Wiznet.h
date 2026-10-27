/*
 * Experimental STM32F4 + W5500 staged OTA service.
 *
 * This is deliberately separate from FLProgNetworkOtaWiznet: the latter uses
 * ESP32 Update/ArduinoOTA internals and remains unchanged. This receiver writes
 * an authenticated image to STM32F407 sectors 8..11; a matching bootloader
 * installs it into the application region after reset.
 */
#pragma once

#include <Arduino.h>
#include "flprogNetworkService.h"
#include "../flprogNetworkProfile.h"
#include "../protocol/flprogNetworkTcpServer.h"
#include "../protocol/flprogNetworkUdp.h"
#include "flprogNetworkOtaSha256.h"

#ifndef FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
#if ((defined(FLPROG_FLE4D_30MT_OTA_PROFILE) && FLPROG_FLE4D_30MT_OTA_PROFILE) || defined(FLPROG_STM32_OTA_LAYOUT)) && \
    defined(ARDUINO_ARCH_STM32) && (defined(STM32F407xx) || (defined(STM32F103xE) && defined(FLPROG_STM32_OTA_LAYOUT))) && FLPROG_NETWORK_COMPILE_WIRED
#define FLPROG_NETWORK_COMPILE_STM32_W5500_OTA 1
#else
#define FLPROG_NETWORK_COMPILE_STM32_W5500_OTA 0
#endif
#endif

class FLProgNetworkInterface;

class FLProgNetworkOtaStm32Wiznet : public FLProgNetworkService
{
public:
    FLProgNetworkOtaStm32Wiznet();
    ~FLProgNetworkOtaStm32Wiznet() override;

    bool begin(FLProgNetworkInterface &network, const char *password,
               uint16_t port = 3233);
    void setIdentity(const char *model, const char *version);
    const char *deviceModel() const override { return supported() ? _model : nullptr; }
    const char *firmwareVersion() const override { return supported() ? _version : nullptr; }
    void end();
    void pool() override;

    bool supported() const { return FLPROG_NETWORK_COMPILE_STM32_W5500_OTA != 0; }
    bool ready() const;
    bool updating() const { return _updating; }
    uint8_t error() const { return _error; }
    uint32_t bytesReceived() const { return _received; }
    uint32_t updateStartCount() const { return _startCount; }
    uint32_t updateEndCount() const { return _endCount; }
    uint32_t updateErrorCount() const { return _errorCount; }
    bool mdnsStarted() const { return _mdnsStarted; }
    uint32_t mdnsPacketCount() const { return _mdnsPacketCount; }
    uint32_t mdnsQueryCount() const { return _mdnsQueryCount; }
    uint32_t mdnsResponseCount() const { return _mdnsResponseCount; }

private:
    enum State : uint8_t { Idle, Header, Erasing, Receiving, Commit, Failed, Discovery };
    void resetTransfer(bool closeClient);
    void fail(uint8_t errorCode);
    void serviceClient();
    void serviceMdns();
    bool eraseStage();
    bool writeStage(const uint8_t *data, size_t length);
    bool commitStage();
    bool sendAck(uint32_t received);

    FLProgNetworkTcpServer _server;
    FLProgNetworkTcpServerClient *_client;
    FLProgNetworkUdp _mdns;
    char _model[48];
    char _version[32];
    char _password[65];
    uint8_t _request[48];
    uint8_t _buffer[1024];
    uint8_t _wordBuffer[4];
    uint8_t _wordBytes;
    uint16_t _requestBytes;
    uint16_t _bufferBytes;
    uint32_t _expectedSize;
    uint32_t _expectedCrc;
    uint32_t _runningCrc;
    uint32_t _received;
    uint32_t _stageAddress;
    uint32_t _lastActivity;
    bool _restartPending;
    uint32_t _restartScheduledAt;
    uint32_t _startCount;
    uint32_t _endCount;
    uint32_t _errorCount;
    uint8_t _expectedMac[32];
    uint16_t _blockExpected;
    uint16_t _blockReceived;
    uint8_t _macBytes;
    uint8_t _state;
    uint8_t _error;
    bool _ready;
    bool _updating;
    bool _mdnsStarted;
    bool _mdnsAnnouncementPending;
    uint32_t _lastMdnsAnnounceAt;
    uint32_t _mdnsPacketCount;
    uint32_t _mdnsQueryCount;
    uint32_t _mdnsResponseCount;
    FLProgOtaHmacSha256 _hmac;
};

