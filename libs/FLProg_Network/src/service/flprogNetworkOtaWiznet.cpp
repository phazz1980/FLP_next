#include "flprogNetworkOtaWiznet.h"

#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
#include "../flprogNetwork.h"
#include "../protocol/flprogNetworkUdp.h"
#include "../protocol/flprogNetworkTcpClient.h"
#include <Update.h>
#include <mbedtls/md.h>
#include <stdio.h>
#include <string.h>
#endif

namespace
{
enum : uint8_t
{
    WIZNET_OTA_IDLE = 0,
    WIZNET_OTA_WAIT_AUTH = 1,
    WIZNET_OTA_WAIT_CLIENT = 2,
    WIZNET_OTA_RECEIVE = 3,
    WIZNET_OTA_RESTART = 4
};

#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
bool makeSha256(const uint8_t *data, size_t length, uint8_t output[32])
{
    const mbedtls_md_info_t *info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
    return info != nullptr && mbedtls_md(info, data, length, output) == 0;
}

void toHex(const uint8_t *data, size_t length, char *output)
{
    static const char digits[] = "0123456789abcdef";
    for (size_t i = 0; i < length; ++i)
    {
        output[i * 2] = digits[data[i] >> 4];
        output[i * 2 + 1] = digits[data[i] & 0x0f];
    }
    output[length * 2] = 0;
}

bool constantTimeEqual(const char *left, const char *right, size_t length)
{
    uint8_t difference = 0;
    for (size_t i = 0; i < length; ++i)
    {
        difference |= static_cast<uint8_t>(left[i] ^ right[i]);
    }
    return difference == 0;
}

bool derivePbkdf2Sha256(const char *password, const char *salt, uint8_t result[32])
{
    const mbedtls_md_info_t *info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
    if (info == nullptr)
    {
        return false;
    }
    mbedtls_md_context_t context;
    mbedtls_md_init(&context);
    bool ok = mbedtls_md_setup(&context, info, 1) == 0;
    const size_t passwordLength = strlen(password);
    const size_t saltLength = strlen(salt);
    if (ok)
    {
        ok = mbedtls_md_hmac_starts(&context,
            reinterpret_cast<const unsigned char *>(password), passwordLength) == 0;
    }

    uint8_t u[32] = {0};
    uint8_t block[4] = {0, 0, 0, 1};
    if (ok)
    {
        ok = mbedtls_md_hmac_update(&context,
            reinterpret_cast<const unsigned char *>(salt), saltLength) == 0 &&
            mbedtls_md_hmac_update(&context, block, sizeof(block)) == 0 &&
            mbedtls_md_hmac_finish(&context, u) == 0;
    }
    if (ok)
    {
        memcpy(result, u, sizeof(u));
        for (uint16_t iteration = 1; iteration < 10000; ++iteration)
        {
            if (mbedtls_md_hmac_reset(&context) != 0 ||
                mbedtls_md_hmac_update(&context, u, sizeof(u)) != 0 ||
                mbedtls_md_hmac_finish(&context, u) != 0)
            {
                ok = false;
                break;
            }
            for (size_t i = 0; i < sizeof(u); ++i)
            {
                result[i] ^= u[i];
            }
        }
    }
    mbedtls_md_free(&context);
    memset(u, 0, sizeof(u));
    return ok;
}
#endif
}

FLProgNetworkOtaWiznet::FLProgNetworkOtaWiznet()
    : _udp(nullptr), _client(nullptr), _hostPort(0), _listenPort(3232),
      _imageSize(0), _bytesReceived(0), _updateStartCount(0), _updateEndCount(0),
      _updateErrorCount(0), _restartAt(0), _state(WIZNET_OTA_IDLE), _error(0),
      _ready(false), _updating(false), _connectRequested(false)
{
    _password[0] = 0;
    _nonce[0] = 0;
    _expectedMd5[0] = 0;
}

FLProgNetworkOtaWiznet::~FLProgNetworkOtaWiznet()
{
    end();
}

bool FLProgNetworkOtaWiznet::ready() const
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    const FLProgNetworkInterface *network = serviceNetwork();
    return _ready && network != nullptr && network->wiznetIsReady();
#else
    return false;
#endif
}

bool FLProgNetworkOtaWiznet::begin(FLProgNetworkInterface &network, const char *password, uint16_t port)
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    if (password == nullptr || password[0] == 0 || strlen(password) >= sizeof(_password) || port == 0)
    {
        _error = 1;
        return false;
    }
    end();
    strcpy(_password, password);
    _listenPort = port;
    _udp = new FLProgNetworkUdp();
    _client = new FLProgNetworkTcpClient();
    if (_udp == nullptr || _client == nullptr ||
        !_udp->begin(network, FLPROG_NETWORK_ROUTE_WIZNET, _listenPort) ||
        !_client->begin(network, FLPROG_NETWORK_ROUTE_WIZNET) || !attach(network))
    {
        end();
        _error = 2;
        return false;
    }
    _ready = true;
    _error = 0;
    return true;
#else
    (void)network;
    (void)password;
    (void)port;
    _error = 3;
    return false;
#endif
}

void FLProgNetworkOtaWiznet::end()
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    abortUpdate();
    if (_client != nullptr)
    {
        _client->stop();
        _client->end();
        delete _client;
        _client = nullptr;
    }
    if (_udp != nullptr)
    {
        _udp->stop();
        delete _udp;
        _udp = nullptr;
    }
#endif
    detach();
    _ready = false;
    _state = WIZNET_OTA_IDLE;
    memset(_password, 0, sizeof(_password));
}

void FLProgNetworkOtaWiznet::pool()
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    FLProgNetworkInterface *network = serviceNetwork();
    if (!_ready || network == nullptr || !network->wiznetIsReady())
    {
        if (_updating || Update.isRunning())
        {
            abortUpdate();
            _error = 4;
            _updateErrorCount++;
        }
        return;
    }

    if (_state == WIZNET_OTA_RESTART)
    {
        if (static_cast<int32_t>(millis() - _restartAt) >= 0)
        {
            ESP.restart();
        }
        return;
    }

    if (_state == WIZNET_OTA_IDLE)
    {
        receiveInvitation();
    }
    else if (_state == WIZNET_OTA_WAIT_AUTH)
    {
        if (_udp->parsePacket() > 0)
        {
            char message[160] = {0};
            const int count = _udp->read(reinterpret_cast<uint8_t *>(message), sizeof(message) - 1);
            if (count > 0)
            {
                message[count] = 0;
                unsigned command = 0;
                char cnonce[65] = {0};
                char response[65] = {0};
                if (sscanf(message, "%u %64s %64s", &command, cnonce, response) == 3 &&
                    command == 200 && verifyAuth(cnonce, response))
                {
                    if (!Update.begin(_imageSize, U_FLASH) || !Update.setMD5(_expectedMd5))
                    {
                        abortUpdate();
                        _error = 7;
                        _state = WIZNET_OTA_IDLE;
                        _udp->beginPacket(_udp->remoteIP(), _udp->remotePort());
                        _udp->write(reinterpret_cast<const uint8_t *>("Update begin failed"), 19);
                        _udp->endPacket();
                        return;
                    }
                    _udp->beginPacket(_udp->remoteIP(), _udp->remotePort());
                    _udp->write(reinterpret_cast<const uint8_t *>("OK"), 2);
                    _udp->endPacket();
                    _state = WIZNET_OTA_WAIT_CLIENT;
                    _connectRequested = false;
                    _error = 0;
                }
                else
                {
                    _udp->beginPacket(_udp->remoteIP(), _udp->remotePort());
                    _udp->write(reinterpret_cast<const uint8_t *>("Authentication Failed"), 21);
                    _udp->endPacket();
                    _error = 5;
                    _state = WIZNET_OTA_IDLE;
                }
            }
        }
    }
    else if (_state == WIZNET_OTA_WAIT_CLIENT)
    {
        if (!_connectRequested)
        {
            _connectRequested = true;
            const uint8_t result = _client->connect(_host, _hostPort);
            if (result == FLPROG_SOCKET_RESULT_ERROR)
            {
                abortUpdate();
                fail(6);
                return;
            }
        }
        if (_client->connected())
        {
            _state = WIZNET_OTA_RECEIVE;
            _updating = true;
            _updateStartCount++;
        }
    }
    else if (_state == WIZNET_OTA_RECEIVE)
    {
        streamFirmware();
    }
#endif
}

bool FLProgNetworkOtaWiznet::receiveInvitation()
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    if (_udp->parsePacket() <= 0)
    {
        return false;
    }
    char message[128] = {0};
    const int count = _udp->read(reinterpret_cast<uint8_t *>(message), sizeof(message) - 1);
    if (count <= 0)
    {
        return false;
    }
    message[count] = 0;
    unsigned command = 0;
    unsigned hostPort = 0;
    unsigned long imageSize = 0;
    char md5[33] = {0};
    if (sscanf(message, "%u %u %lu %32s", &command, &hostPort, &imageSize, md5) != 4 ||
        command != 0 || hostPort == 0 || imageSize == 0 || strlen(md5) != 32)
    {
        return false;
    }
    _host = _udp->remoteIP();
    _hostPort = static_cast<uint16_t>(hostPort);
    _imageSize = static_cast<uint32_t>(imageSize);
    _bytesReceived = 0;
    _connectRequested = false;
    strcpy(_expectedMd5, md5);

    uint8_t nonceBytes[32];
    for (size_t i = 0; i < sizeof(nonceBytes); i += 4)
    {
        const uint32_t randomValue = esp_random();
        memcpy(nonceBytes + i, &randomValue, sizeof(randomValue));
    }
    toHex(nonceBytes, sizeof(nonceBytes), _nonce);
    _udp->beginPacket(_udp->remoteIP(), _udp->remotePort());
    _udp->write(reinterpret_cast<const uint8_t *>("AUTH "), 5);
    _udp->write(reinterpret_cast<const uint8_t *>(_nonce), 64);
    _udp->endPacket();
    _state = WIZNET_OTA_WAIT_AUTH;
    return true;
#else
    return false;
#endif
}

bool FLProgNetworkOtaWiznet::verifyAuth(const char *cnonce, const char *response)
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    if (strlen(cnonce) != 64 || strlen(response) != 64)
    {
        return false;
    }
    uint8_t passwordHashBytes[32];
    char passwordHash[65];
    if (!makeSha256(reinterpret_cast<const uint8_t *>(_password), strlen(_password), passwordHashBytes))
    {
        return false;
    }
    toHex(passwordHashBytes, sizeof(passwordHashBytes), passwordHash);

    char salt[130];
    snprintf(salt, sizeof(salt), "%s:%s", _nonce, cnonce);
    uint8_t derivedKeyBytes[32];
    if (!derivePbkdf2Sha256(passwordHash, salt, derivedKeyBytes))
    {
        return false;
    }
    char derivedKey[65];
    toHex(derivedKeyBytes, sizeof(derivedKeyBytes), derivedKey);
    char challenge[194];
    snprintf(challenge, sizeof(challenge), "%s:%s:%s", derivedKey, _nonce, cnonce);
    uint8_t expectedBytes[32];
    char expected[65];
    if (!makeSha256(reinterpret_cast<const uint8_t *>(challenge), strlen(challenge), expectedBytes))
    {
        return false;
    }
    toHex(expectedBytes, sizeof(expectedBytes), expected);
    return constantTimeEqual(expected, response, 64);
#else
    (void)cnonce;
    (void)response;
    return false;
#endif
}

void FLProgNetworkOtaWiznet::streamFirmware()
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    uint8_t buffer[1024];
    const int available = _client->available();
    if (available <= 0)
    {
        if (!_client->connected())
        {
            abortUpdate();
            fail(6);
        }
        return;
    }
    size_t wanted = static_cast<size_t>(available);
    if (wanted > sizeof(buffer)) wanted = sizeof(buffer);
    const uint32_t remaining = _imageSize - _bytesReceived;
    if (wanted > remaining) wanted = remaining;
    const int received = _client->read(buffer, wanted);
    if (received <= 0)
    {
        return;
    }
    const size_t written = Update.write(buffer, static_cast<size_t>(received));
    if (written != static_cast<size_t>(received))
    {
        abortUpdate();
        fail(7);
        return;
    }
    _bytesReceived += static_cast<uint32_t>(written);
    char acknowledgement[12];
    const int ackLength = snprintf(acknowledgement, sizeof(acknowledgement), "%u", static_cast<unsigned>(written));
    _client->write(reinterpret_cast<const uint8_t *>(acknowledgement), static_cast<size_t>(ackLength));

    if (_bytesReceived >= _imageSize)
    {
        if (Update.end(true))
        {
            _client->write(reinterpret_cast<const uint8_t *>("OK"), 2);
            _client->stop();
            _updating = false;
            _updateEndCount++;
            _state = WIZNET_OTA_RESTART;
            _restartAt = millis() + 250;
        }
        else
        {
            abortUpdate();
            fail(8);
        }
    }
#endif
}

void FLProgNetworkOtaWiznet::abortUpdate()
{
#if FLPROG_NETWORK_COMPILE_WIZNET_OTA
    if (_updating || Update.isRunning())
    {
        Update.abort();
    }
    if (_client != nullptr)
    {
        _client->stop();
    }
#endif
    _updating = false;
    _connectRequested = false;
    _state = WIZNET_OTA_IDLE;
}

void FLProgNetworkOtaWiznet::fail(uint8_t code)
{
    _error = code;
    _updating = false;
    _state = WIZNET_OTA_IDLE;
    _updateErrorCount++;
}
