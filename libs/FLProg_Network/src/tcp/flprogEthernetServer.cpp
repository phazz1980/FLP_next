/*
 * FLProg_Network — flprogEthernetServer.cpp
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
#include "flprogEthernetServer.h"

// Legacy server keeps a source pointer; ownership stays with the FLProg NetworkDevice.
void FLProgEthernetServer::setSourse(FLProgAbstractTcpInterface *sourse)
{
    if (_sourse == sourse)
    {
        return;
    }
    stop();
    _sourse = sourse;
}

uint8_t FLProgEthernetServer::begin()
{
    if (_sourse == nullptr)
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_HARDWARE_INIT_ERROR;
        _isInit = false;
        setFlags();
        return FLPROG_ERROR;
    }

    if (!_sourse->isReady())
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        _isInit = false;
        setFlags();
        return FLPROG_ERROR;
    }

    _server.end();

    // Legacy FLProgEthernetServer exposes one current client through the server
    // object itself, so the compatibility adapter intentionally uses maxClients=1.
    if (!_server.begin(*_sourse, _port, FLPROG_NETWORK_ROUTE_DEFAULT, 1))
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_SERVER_SOKET_ERROR;
        _isInit = false;
        setFlags();
        return FLPROG_ERROR;
    }

    _status = FLPROG_READY_STATUS;
    _errorCode = FLPROG_NOT_ERROR;
    _isInit = true;
    setFlags();
    return FLPROG_SUCCESS;
}

uint8_t FLProgEthernetServer::setPort(uint16_t port)
{
    if (_port == port)
    {
        return FLPROG_SUCCESS;
    }

    _port = port;
    if (_isInit)
    {
        stop();
    }
    return FLPROG_SUCCESS;
}

FLProgNetworkTcpServerClient *FLProgEthernetServer::currentClient()
{
    // Gate 0.3 compatibility server owns exactly one accepted-client slot.
    FLProgNetworkTcpServerClient *client = _server.client(0);
    if (client == nullptr || !client->connected())
    {
        return nullptr;
    }
    return client;
}

void FLProgEthernetServer::noteNativeError()
{
    if (_server.serverError() == FLPROG_TCP_SERVER_ERROR_NONE)
    {
        return;
    }
    _errorCode = FLPROG_ETHERNET_SERVER_SOKET_ERROR;
}

// Report the currently accepted legacy client without exposing native backend details.
uint8_t FLProgEthernetServer::connected()
{
    if (_sourse == nullptr)
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_HARDWARE_INIT_ERROR;
        setFlags();
        return 0;
    }

    if (!_sourse->isReady())
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR;
        setFlags();
        return 0;
    }

    if (!_isInit)
    {
        begin();
        return 0;
    }

    noteNativeError();
    FLProgNetworkTcpServerClient *client = currentClient();
    if (client == nullptr)
    {
        setFlags();
        return 0;
    }

    _status = FLPROG_READY_STATUS;
    _errorCode = FLPROG_NOT_ERROR;
    setFlags();
    return 1;
}

uint8_t FLProgEthernetServer::pool()
{
    // Native server progress is intentionally NOT called here. It is a managed
    // FLProg_Network service and is serviced exactly once from network.pool().
    setFlags();

    if (_sourse == nullptr)
    {
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_HARDWARE_INIT_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    if (_callbackFunction == nullptr)
    {
        // Preserve the legacy pool() contract. Direct connected()/available()
        // use (e.g. Flprog_Modbus) does not require a callback and never calls
        // server.pool().
        _status = FLPROG_NOT_REDY_STATUS;
        _errorCode = FLPROG_ETHERNET_SERVER_NOT_CALLBACK_ERROR;
        setFlags();
        return FLPROG_ERROR;
    }

    if (!connected() || available() <= 0)
    {
        return FLPROG_SUCCESS;
    }

    _callbackFunction();
    return FLPROG_SUCCESS;
}

// Close only the accepted client connection; keep server/network lifecycle separate.
void FLProgEthernetServer::stopConnection()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    if (client != nullptr)
    {
        client->stop();
    }
}

void FLProgEthernetServer::stop()
{
    _server.end();
    _isInit = false;
    _status = FLPROG_NOT_REDY_STATUS;
    _errorCode = FLPROG_NOT_ERROR;
    setFlags();
}

int FLProgEthernetServer::available()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? 0 : client->available();
}

int FLProgEthernetServer::read()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? -1 : client->read();
}

int FLProgEthernetServer::read(uint8_t *buffer, size_t length)
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? 0 : client->read(buffer, length);
}

int FLProgEthernetServer::peek()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? -1 : client->peek();
}

size_t FLProgEthernetServer::write(uint8_t value)
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? 0 : client->write(value);
}

size_t FLProgEthernetServer::write(const uint8_t *buffer, size_t length)
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? 0 : client->write(buffer, length);
}

void FLProgEthernetServer::flush()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    if (client != nullptr)
    {
        client->flush();
    }
}

void FLProgEthernetServer::readToNull(uint16_t count)
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

uint16_t FLProgEthernetServer::localPort()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? _port : client->localPort();
}

IPAddress FLProgEthernetServer::remoteIP()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? IPAddress(0, 0, 0, 0) : client->remoteIP();
}

uint16_t FLProgEthernetServer::remotePort()
{
    FLProgNetworkTcpServerClient *client = currentClient();
    return client == nullptr ? 0 : client->remotePort();
}

void FLProgEthernetServer::setFlags()
{
    if (_oldStatus != _status)
    {
        _oldStatus = _status;
        _isChangeStatus = true;
    }
    if (_oldError != _errorCode)
    {
        _oldError = _errorCode;
        _isChangeError = true;
    }
}

bool FLProgEthernetServer::getIsChangeStatusWithReset()
{
    const bool value = _isChangeStatus;
    _isChangeStatus = false;
    return value;
}

bool FLProgEthernetServer::getIsChangeErrorWithReset()
{
    const bool value = _isChangeError;
    _isChangeError = false;
    return value;
}

bool FLProgEthernetServer::statusForExtGetBitWithReset(uint8_t bit)
{
    const bool value = bitRead(_statusForExt, bit);
    bitWrite(_statusForExt, bit, 0);
    return value;
}
