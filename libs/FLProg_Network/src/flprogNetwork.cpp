/*
 * FLProg_Network — flprogNetwork.cpp
 * Назначение: Реализация жизненного цикла FLProgNetworkInterface.
 *
 * Архитектурные заметки:
 * - pool() обслуживает выбранный transport и публикует единое состояние для верхних слоёв.
 * - Смена transport-состояния должна согласованно обновлять generation и уведомлять socket/A3 слой.
 * - Wi-Fi, Wiznet и RMII не должны создавать ложные reconnect/stale события друг для друга.
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
#include "flprogNetwork.h"
#include <new>

// EXP-2A: real wired accessor definitions exist only when the wired backend
// is compiled. Wi-Fi-only live code contains no wired references.
#if FLPROG_NETWORK_COMPILE_WIRED
FLProgWiznetHardware &FLProgNetworkInterface::wiznetRef()
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetHardware stub;
    return stub;
#else
    return _wiznet;
#endif
}

const FLProgWiznetHardware &FLProgNetworkInterface::wiznetRef() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetHardware stub;
    return stub;
#else
    return _wiznet;
#endif
}

FLProgRmiiEthernet &FLProgNetworkInterface::rmiiRef()
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgRmiiEthernet stub;
    return stub;
#else
    return _rmii;
#endif
}

const FLProgRmiiEthernet &FLProgNetworkInterface::rmiiRef() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgRmiiEthernet stub;
    return stub;
#else
    return _rmii;
#endif
}

FLProgWiznetDhcp &FLProgNetworkInterface::wiznetDhcpRef()
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetDhcp stub;
    return stub;
#else
    return _wiznetDhcp;
#endif
}

const FLProgWiznetDhcp &FLProgNetworkInterface::wiznetDhcpRef() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetDhcp stub;
    return stub;
#else
    return _wiznetDhcp;
#endif
}

FLProgWiznetTransport &FLProgNetworkInterface::wiznetTransportRef()
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetTransport stub;
    return stub;
#else
    return _wiznetTransport;
#endif
}

const FLProgWiznetTransport &FLProgNetworkInterface::wiznetTransportRef() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgWiznetTransport stub;
    return stub;
#else
    return _wiznetTransport;
#endif
}

FLProgRmiiTransport &FLProgNetworkInterface::rmiiTransportRef()
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgRmiiTransport stub;
    return stub;
#else
    return _rmiiTransport;
#endif
}

const FLProgRmiiTransport &FLProgNetworkInterface::rmiiTransportRef() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    static FLProgRmiiTransport stub;
    return stub;
#else
    return _rmiiTransport;
#endif
}

#endif // FLPROG_NETWORK_COMPILE_WIRED

// Платформенные сетевые заголовки не выносятся в публичный .h.
// Это предотвращает конфликт ESP8266WiFi и Arduino Ethernet по MAX_SOCK_NUM
// в скетчах, генерируемых FLProg.

#if FLPROG_NETWORK_COMPILE_WIFI && defined(ESP8266)
    #include <ESP8266WiFi.h>
    extern "C" {
        #include <user_interface.h>
    }
    #define FLPROG_NETWORK_HAS_WIFI 1
#elif FLPROG_NETWORK_COMPILE_WIFI && defined(ESP32)
    #include <WiFi.h>
    #include <esp_wifi.h>
    #define FLPROG_NETWORK_HAS_WIFI 1
#elif defined(ARDUINO_ARCH_RP2040) || defined(ARDUINO_ARCH_RP2350)
    // v0.9.2 compatibility closeout: do not silently use the Arduino-Pico
    // high-level WiFi API. Per-interface route/socket ownership for Pico W is
    // not implemented yet, so the built-in Wi-Fi backend stays explicitly
    // unsupported until a raw-lwIP/owned adapter is added.
    #define FLPROG_NETWORK_HAS_WIFI 0
#else
    #define FLPROG_NETWORK_HAS_WIFI 0
#endif

#include <SPI.h>

FLProgNetworkInterface *FLProgNetworkInterface::_legacyInterfaceList[FLPROG_NETWORK_MAX_DEVICES] = {nullptr};
uint8_t FLProgNetworkInterface::_legacyInterfaceCount = 0;

// -----------------------------------------------------------------------------
// Создание объекта и сброс рабочего состояния
// -----------------------------------------------------------------------------

// Создаёт интерфейс с настройками по умолчанию. Обращение к аппаратуре начинается
// только после того, как pool() обработает разрешение работы или запрос подключения.
FLProgNetworkInterface::FLProgNetworkInterface()
{
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    wiznetRef().setPinCs(_pinCs);
    wiznetRef().setSpiBus(_spiBus);
#endif
    initTransportLayer();
    for (uint8_t route = 0; route <= FLPROG_NETWORK_ROUTE_WIFI_AP; route++)
    {
        clearRouteTransportIdentity(route);
    }
    resetRuntime();
    registerLegacyInterface();
}

FLProgNetworkInterface::FLProgNetworkInterface(int pinCs, uint8_t bus)
{
    _pinCs = pinCs;
    _spiBus = bus;
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    wiznetRef().setPinCs(_pinCs);
    wiznetRef().setSpiBus(_spiBus);
#endif
    initTransportLayer();
    for (uint8_t route = 0; route <= FLPROG_NETWORK_ROUTE_WIFI_AP; route++)
    {
        clearRouteTransportIdentity(route);
    }
    resetRuntime();
    registerLegacyInterface();
}

FLProgNetworkInterface::~FLProgNetworkInterface()
{
    unregisterLegacyInterface();
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    wiznetDhcpRef().end(false);
#endif

    if (_dnsResolver != nullptr)
    {
        _dnsResolver->end();
        delete _dnsResolver;
        _dnsResolver = nullptr;
    }
}

void FLProgNetworkInterface::registerLegacyInterface()
{
    for (uint8_t i = 0; i < _legacyInterfaceCount; ++i)
    {
        if (_legacyInterfaceList[i] == this) return;
    }
    if (_legacyInterfaceCount < FLPROG_NETWORK_MAX_DEVICES)
    {
        _legacyInterfaceList[_legacyInterfaceCount++] = this;
    }
}

void FLProgNetworkInterface::unregisterLegacyInterface()
{
    for (uint8_t i = 0; i < _legacyInterfaceCount; ++i)
    {
        if (_legacyInterfaceList[i] != this) continue;
        for (uint8_t j = i + 1; j < _legacyInterfaceCount; ++j)
        {
            _legacyInterfaceList[j - 1] = _legacyInterfaceList[j];
        }
        _legacyInterfaceList[--_legacyInterfaceCount] = nullptr;
        return;
    }
}

FLProgNetworkInterface *FLProgNetworkInterface::legacyDefaultWifiInterface()
{
    for (uint8_t i = 0; i < _legacyInterfaceCount; ++i)
    {
        FLProgNetworkInterface *candidate = _legacyInterfaceList[i];
        if (candidate != nullptr && candidate->wifiIsReady())
        {
            return candidate;
        }
    }
    return nullptr;
}

// Сбрасывает только текущее рабочее состояние, не изменяя пользовательские
// настройки: SSID, пароль, адресацию, вывод CS и временные параметры.
void FLProgNetworkInterface::resetRuntime()
{
    _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;
    _status = _workMode ? FLPROG_NETWORK_IDLE_STATUS : FLPROG_NETWORK_DISABLED_STATUS;
    _error = FLPROG_NETWORK_NOT_ERROR;
    _wifiStartRequest = false;
    _apStartRequest = false;
    _ethernetStartRequest = false;
    _rmiiStartRequest = false;
    _wifiStarted = false;
    _apStarted = false;
    _ethernetStarted = false;
    _rmiiStarted = false;
    _ethernetConfigured = false;
    _wifiReady = false;
    _apReady = false;
    _ethernetReady = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);
    _rmiiReady = false;
    _manualStop = false;
    _connectionStartTime = millis();
    _lastWifiAttempt = 0;
    _lastApAttempt = 0;
    _lastEthernetAttempt = 0;
    _lastRmiiAttempt = 0;
    _lastEthernetCheck = 0;
    _lastMaintainTime = 0;
    _apStationCount = 0;
    _prevApReady = false;
    _prevWifiReadyForEvents = false;
    _wifiScanState = FLPROG_WIFI_SCAN_IDLE;
    _wifiScanCount = 0;
    _wifiEvents = FLPROG_WIFI_EVENT_NONE;
}


// Возвращает true, если выбранный общий режим разрешает использование Wi-Fi.
bool FLProgNetworkInterface::usesWifi() const
{
    if (!_wifiAllowed)
    {
        return false;
    }
    return
        (_networkMode == FLPROG_NETWORK_MODE_WIFI) ||
        (_networkMode == FLPROG_NETWORK_MODE_AUTO) ||
        (_networkMode == FLPROG_NETWORK_MODE_BOTH);
}

// Возвращает true, если выбранный общий режим разрешает использование Ethernet.
bool FLProgNetworkInterface::usesWiznet() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    return false;
#else
    if (!_wiznetAllowed)
    {
        return false;
    }

    return
        (_networkMode == FLPROG_NETWORK_MODE_ETHERNET) ||
        (_networkMode == FLPROG_NETWORK_MODE_AUTO) ||
        (_networkMode == FLPROG_NETWORK_MODE_BOTH);
#endif
}

bool FLProgNetworkInterface::usesRmii() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    return false;
#else
    if (!_rmiiAllowed)
    {
        return false;
    }

    return
        (_networkMode == FLPROG_NETWORK_MODE_ETHERNET) ||
        (_networkMode == FLPROG_NETWORK_MODE_AUTO) ||
        (_networkMode == FLPROG_NETWORK_MODE_BOTH);
#endif
}

bool FLProgNetworkInterface::usesEthernet() const
{
    return usesWiznet() || usesRmii();
}


// -----------------------------------------------------------------------------
// Универсальный транспортный слой
// -----------------------------------------------------------------------------

bool FLProgNetworkInterface::transportWifiReady(void *context)
{
    FLProgNetworkInterface *network =
        static_cast<FLProgNetworkInterface *>(context);

    // RC6-A-dev.3-fix3: during an intentional live STA restart the native
    // stack can still report WL_CONNECTED for a short time after WiFi.begin().
    // Do not admit a replacement socket lease in that transient window.
    return network->_wifiReady && !network->_wifiSessionRestartGate;
}

IPAddress FLProgNetworkInterface::transportWifiIp(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->wifiIP();
}

uint8_t FLProgNetworkInterface::transportWifiLink(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->wifiIsReady()
        ? FLPROG_TRANSPORT_LINK_ON
        : FLPROG_TRANSPORT_LINK_OFF;
}

bool FLProgNetworkInterface::transportWifiApReady(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->accessPointIsReady();
}

IPAddress FLProgNetworkInterface::transportWifiApIp(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->accessPointIP();
}

uint8_t FLProgNetworkInterface::transportWifiApLink(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->accessPointIsReady()
        ? FLPROG_TRANSPORT_LINK_ON
        : FLPROG_TRANSPORT_LINK_OFF;
}

bool FLProgNetworkInterface::transportWiznetReady(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->wiznetIsReady();
}

IPAddress FLProgNetworkInterface::transportWiznetIp(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->wiznetIP();
}

uint8_t FLProgNetworkInterface::transportWiznetLink(void *context)
{
    const uint8_t value =
        static_cast<FLProgNetworkInterface *>(context)->linkStatus();

    if (value == FLPROG_WIZNET_LINK_ON)
    {
        return FLPROG_TRANSPORT_LINK_ON;
    }

    if (value == FLPROG_WIZNET_LINK_OFF)
    {
        return FLPROG_TRANSPORT_LINK_OFF;
    }

    return FLPROG_TRANSPORT_LINK_UNKNOWN;
}

uint8_t FLProgNetworkInterface::transportWiznetSocketLimit(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->maxSocketNum();
}

bool FLProgNetworkInterface::transportRmiiReady(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->rmiiIsReady();
}

IPAddress FLProgNetworkInterface::transportRmiiIp(void *context)
{
    return static_cast<FLProgNetworkInterface *>(context)->rmiiIP();
}

uint8_t FLProgNetworkInterface::transportRmiiLink(void *context)
{
    const uint8_t value =
        static_cast<FLProgNetworkInterface *>(context)->rmiiLinkStatus();

    if (value == 1)
    {
        return FLPROG_TRANSPORT_LINK_ON;
    }

    if (value == 2)
    {
        return FLPROG_TRANSPORT_LINK_OFF;
    }

    return FLPROG_TRANSPORT_LINK_UNKNOWN;
}

void FLProgNetworkInterface::initTransportLayer()
{
    // RC6-A.3: instance ids are local to a transport kind.  The built-in
    // single transports are therefore instance #0; route/ACTIVE ids remain a
    // separate concept and are not reused as transport instance ids.
#if FLPROG_NETWORK_COMPILE_WIFI
    _wifiTransport.begin(
        0,
        this,
        &FLProgNetworkInterface::transportWifiReady,
        &FLProgNetworkInterface::transportWifiIp,
        &FLProgNetworkInterface::transportWifiLink
    );

    _wifiApTransport.begin(
        0,
        this,
        &FLProgNetworkInterface::transportWifiApReady,
        &FLProgNetworkInterface::transportWifiApIp,
        &FLProgNetworkInterface::transportWifiApLink
    );
#endif

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    wiznetTransportRef().begin(
        0,
        this,
        &FLProgNetworkInterface::transportWiznetReady,
        &FLProgNetworkInterface::transportWiznetIp,
        &FLProgNetworkInterface::transportWiznetLink,
        &FLProgNetworkInterface::transportWiznetSocketLimit
    );

    rmiiTransportRef().begin(
        0,
        this,
        &FLProgNetworkInterface::transportRmiiReady,
        &FLProgNetworkInterface::transportRmiiIp,
        &FLProgNetworkInterface::transportRmiiLink
    );
#endif

#if FLPROG_NETWORK_COMPILE_WIFI
    _socketManager.registerTransport(&_wifiTransport);
    _socketManager.registerTransport(&_wifiApTransport);
#endif
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    _socketManager.registerTransport(&wiznetTransportRef());
    _socketManager.registerTransport(&rmiiTransportRef());
#endif
}

void FLProgNetworkInterface::updateTransportLayer()
{
#if FLPROG_NETWORK_COMPILE_WIFI
    _wifiTransport.pool();
    _wifiApTransport.pool();
#endif
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    wiznetTransportRef().pool();
    rmiiTransportRef().pool();
#endif
}

bool FLProgNetworkInterface::registerManagedSocket(
    FLProgNetworkSocket *socket)
{
    if (socket == nullptr)
    {
        return false;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SOCKETS; i++)
    {
        if (_managedSockets[i] == socket)
        {
            return true;
        }
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SOCKETS; i++)
    {
        if (_managedSockets[i] == nullptr)
        {
            _managedSockets[i] = socket;
            return true;
        }
    }

    return false;
}

void FLProgNetworkInterface::unregisterManagedSocket(
    FLProgNetworkSocket *socket)
{
    if (socket == nullptr)
    {
        return;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SOCKETS; i++)
    {
        if (_managedSockets[i] == socket)
        {
            _managedSockets[i] = nullptr;
        }
    }
}

uint8_t FLProgNetworkInterface::managedSocketCount() const
{
    uint8_t count = 0;

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SOCKETS; i++)
    {
        if (_managedSockets[i] != nullptr)
        {
            count++;
        }
    }

    return count;
}

void FLProgNetworkInterface::updateSocketLayer()
{
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SOCKETS; i++)
    {
        FLProgNetworkSocket *socket = _managedSockets[i];

        if (socket != nullptr)
        {
            socket->pool();
        }
    }
}

bool FLProgNetworkInterface::registerManagedService(
    FLProgNetworkService *service)
{
    if (service == nullptr)
    {
        return false;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SERVICES; i++)
    {
        if (_managedServices[i] == service)
        {
            return true;
        }
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SERVICES; i++)
    {
        if (_managedServices[i] == nullptr)
        {
            _managedServices[i] = service;
            return true;
        }
    }

    return false;
}

void FLProgNetworkInterface::unregisterManagedService(
    FLProgNetworkService *service)
{
    if (service == nullptr)
    {
        return;
    }

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SERVICES; i++)
    {
        if (_managedServices[i] == service)
        {
            _managedServices[i] = nullptr;
        }
    }
}

uint8_t FLProgNetworkInterface::managedServiceCount() const
{
    uint8_t count = 0;

    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SERVICES; i++)
    {
        if (_managedServices[i] != nullptr)
        {
            count++;
        }
    }

    return count;
}

FLProgNetworkCapacitySnapshot FLProgNetworkInterface::capacitySnapshot() const
{
    FLProgNetworkCapacitySnapshot result;

    result.logicalLeaseLimit = FLPROG_SOCKET_MAX_LEASES;
    result.logicalLeasesUsed = _socketManager.activeLeaseCount();
    result.logicalLeasesFree = _socketManager.freeLeaseCount();

    result.managedSocketLimit = FLPROG_NETWORK_MAX_MANAGED_SOCKETS;
    result.managedSocketsUsed = managedSocketCount();
    result.managedSocketsFree = managedSocketFreeCount();

    result.managedServiceLimit = FLPROG_NETWORK_MAX_MANAGED_SERVICES;
    result.managedServicesUsed = managedServiceCount();
    result.managedServicesFree = managedServiceFreeCount();

    result.wiznetSocketLimit = wiznetTransportRef().nativeSocketLimit();
    result.wiznetSocketsReserved = wiznetTransportRef().nativeSocketReservedCount();
    result.wiznetSocketsFree = wiznetTransportRef().nativeSocketAvailableCount();

    result.dnsResolverPresent = _dnsResolver != nullptr;
    result.dnsResolverBusy = _dnsResolver != nullptr && _dnsResolver->busy();
    result.dhcpState = wiznetDhcpRef().state();
    result.strictAcdActive = wiznetDhcpRef().strictAcdActive();
    result.strictAcdSocketReserved = wiznetDhcpRef().strictAcdSocketReserved();

    return result;
}

void FLProgNetworkInterface::updateServiceLayer()
{
    for (uint8_t i = 0; i < FLPROG_NETWORK_MAX_MANAGED_SERVICES; i++)
    {
        FLProgNetworkService *service = _managedServices[i];

        if (service != nullptr)
        {
            service->pool();
        }
    }
}

FLProgNetworkTransport *FLProgNetworkInterface::transportById(
    uint8_t transportId)
{
    return _socketManager.transportById(transportId);
}

const FLProgNetworkTransport *FLProgNetworkInterface::transportById(
    uint8_t transportId) const
{
    return _socketManager.transportById(transportId);
}

FLProgNetworkTransport *FLProgNetworkInterface::transportByIdentity(
    FLProgTransportKind kind,
    FLProgTransportInstanceId instanceId)
{
    return _socketManager.transportByIdentity(kind, instanceId);
}

const FLProgNetworkTransport *FLProgNetworkInterface::transportByIdentity(
    FLProgTransportKind kind,
    FLProgTransportInstanceId instanceId) const
{
    return _socketManager.transportByIdentity(kind, instanceId);
}

bool FLProgNetworkInterface::setRouteTransportIdentity(
    uint8_t route,
    const FLProgTransportIdentity &identity)
{
    if (route > FLPROG_NETWORK_ROUTE_WIFI_AP || !identity.valid())
    {
        return false;
    }

    _routeTransportIdentity[route] = identity;
    return true;
}

void FLProgNetworkInterface::clearRouteTransportIdentity(uint8_t route)
{
    if (route > FLPROG_NETWORK_ROUTE_WIFI_AP)
    {
        return;
    }

    _routeTransportIdentity[route].kind = FLPROG_TRANSPORT_NONE;
    _routeTransportIdentity[route].instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;
}

FLProgTransportIdentity FLProgNetworkInterface::configuredRouteTransportIdentity(
    uint8_t route) const
{
    FLProgTransportIdentity result;
    result.kind = FLPROG_TRANSPORT_NONE;
    result.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    if (route > FLPROG_NETWORK_ROUTE_WIFI_AP)
    {
        return result;
    }

    return _routeTransportIdentity[route];
}

FLProgTransportIdentity FLProgNetworkInterface::transportIdentityForRoute(
    uint8_t route,
    bool requireReady) const
{
    FLProgTransportIdentity result;
    result.kind = FLPROG_TRANSPORT_NONE;
    result.instanceId = FLPROG_TRANSPORT_INSTANCE_INVALID;

    // A3-6: an explicit route binding has priority over legacy built-in
    // selection. Readiness is checked against the exact transport instance.
    const FLProgTransportIdentity configured =
        configuredRouteTransportIdentity(route);
    if (configured.valid())
    {
        const FLProgNetworkTransport *transport =
            transportByIdentity(configured);

        if (transport == nullptr)
        {
            return result;
        }

        if (requireReady && !transport->isReady())
        {
            return result;
        }

        return configured;
    }

    if (route == FLPROG_NETWORK_ROUTE_WIFI)
    {
        if (!_wifiAllowed || (requireReady && !_wifiReady))
        {
            return result;
        }

        result.kind = FLPROG_TRANSPORT_WIFI;
        result.instanceId = 0;
        return result;
    }

    if (route == FLPROG_NETWORK_ROUTE_WIFI_AP)
    {
        if (!_apAllowed || (requireReady && !_apReady))
        {
            return result;
        }

        result.kind = FLPROG_TRANSPORT_WIFI_AP;
        result.instanceId = 0;
        return result;
    }

    if (route == FLPROG_NETWORK_ROUTE_WIZNET)
    {
        if (!_wiznetAllowed || (requireReady && !_ethernetReady))
        {
            return result;
        }

        result.kind = FLPROG_TRANSPORT_WIZNET;
        result.instanceId = 0;
        return result;
    }

    if (route == FLPROG_NETWORK_ROUTE_RMII)
    {
        if (!_rmiiAllowed || (requireReady && !_rmiiReady))
        {
            return result;
        }

        result.kind = FLPROG_TRANSPORT_RMII;
        result.instanceId = 0;
        return result;
    }

    if (requireReady)
    {
        const uint8_t interfaceId = resolveRoute(route);

        if (interfaceId == FLPROG_NETWORK_ACTIVE_WIFI)
        {
            result.kind = FLPROG_TRANSPORT_WIFI;
            result.instanceId = 0;
            return result;
        }

        if (interfaceId == FLPROG_NETWORK_ACTIVE_WIFI_AP)
        {
            result.kind = FLPROG_TRANSPORT_WIFI_AP;
            result.instanceId = 0;
            return result;
        }

        if (interfaceId == FLPROG_NETWORK_ACTIVE_WIZNET)
        {
            result.kind = FLPROG_TRANSPORT_WIZNET;
            result.instanceId = 0;
            return result;
        }

        if (interfaceId == FLPROG_NETWORK_ACTIVE_RMII)
        {
            result.kind = FLPROG_TRANSPORT_RMII;
            result.instanceId = 0;
            return result;
        }

        return result;
    }

    if (route == FLPROG_NETWORK_ROUTE_ETHERNET)
    {
        if (_wiredPriority == FLPROG_NETWORK_WIRED_PRIORITY_RMII)
        {
            if (_rmiiAllowed)
            {
                result.kind = FLPROG_TRANSPORT_RMII;
                result.instanceId = 0;
                return result;
            }
            if (_wiznetAllowed)
            {
                result.kind = FLPROG_TRANSPORT_WIZNET;
                result.instanceId = 0;
                return result;
            }
        }
        else
        {
            if (_wiznetAllowed)
            {
                result.kind = FLPROG_TRANSPORT_WIZNET;
                result.instanceId = 0;
                return result;
            }
            if (_rmiiAllowed)
            {
                result.kind = FLPROG_TRANSPORT_RMII;
                result.instanceId = 0;
                return result;
            }
        }

        return result;
    }

    const FLProgTransportIdentity wired =
        transportIdentityForRoute(
            FLPROG_NETWORK_ROUTE_ETHERNET,
            false
        );

    if (wired.valid())
    {
        return wired;
    }

    if (_wifiAllowed)
    {
        result.kind = FLPROG_TRANSPORT_WIFI;
        result.instanceId = 0;
    }

    return result;
}

uint8_t FLProgNetworkInterface::transportIdForRoute(
    uint8_t route,
    bool requireReady) const
{
    const FLProgTransportIdentity identity =
        transportIdentityForRoute(route, requireReady);

    return identity.valid()
        ? identity.kind
        : FLPROG_TRANSPORT_NONE;
}

FLProgNetworkTransport *FLProgNetworkInterface::transportForRoute(
    uint8_t route,
    bool requireReady)
{
    const FLProgTransportIdentity identity =
        transportIdentityForRoute(route, requireReady);

    return identity.valid()
        ? transportByIdentity(identity)
        : nullptr;
}

const FLProgNetworkTransport *FLProgNetworkInterface::transportForRoute(
    uint8_t route,
    bool requireReady) const
{
    const FLProgTransportIdentity identity =
        transportIdentityForRoute(route, requireReady);

    return identity.valid()
        ? transportByIdentity(identity)
        : nullptr;
}

FLProgNetworkDns *FLProgNetworkInterface::dnsResolver()
{
    if (_dnsResolver != nullptr)
    {
        return _dnsResolver;
    }

    _dnsResolver = new (std::nothrow) FLProgNetworkDns();

    if (_dnsResolver == nullptr)
    {
        return nullptr;
    }

    if (!_dnsResolver->begin(*this, FLPROG_NETWORK_ROUTE_DEFAULT))
    {
        delete _dnsResolver;
        _dnsResolver = nullptr;
        return nullptr;
    }

    return _dnsResolver;
}

IPAddress FLProgNetworkInterface::dnsServerForTransport(
    uint8_t transportId) const
{
    const IPAddress zero(0, 0, 0, 0);

    if (transportId == FLPROG_TRANSPORT_WIFI)
    {
#if FLPROG_NETWORK_HAS_WIFI
        if (_wifiReady)
        {
            const IPAddress activeDns = WiFi.dnsIP();
            if (activeDns != zero)
            {
                return activeDns;
            }
        }
#endif
        if (_dnsIp != zero)
        {
            return _dnsIp;
        }

        if (_gatewayIp != zero)
        {
            return _gatewayIp;
        }

        return zero;
    }

#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    // Wi-Fi-only profile: no W5500/RMII DNS fallback objects exist.
    return zero;
#else
    if (transportId == FLPROG_TRANSPORT_WIZNET)
    {
        if (_dhcp && wiznetDhcpRef().bound())
        {
            const IPAddress activeDns = wiznetDhcpRef().dnsIP();
            if (activeDns != zero)
            {
                return activeDns;
            }

            const IPAddress dhcpGateway = wiznetDhcpRef().gatewayIP();
            if (dhcpGateway != zero)
            {
                return dhcpGateway;
            }
        }

        if (_dnsIp != zero)
        {
            return _dnsIp;
        }

        if (_gatewayIp != zero)
        {
            return _gatewayIp;
        }

        const IPAddress gatewayIp =
            const_cast<FLProgWiznetHardware &>(wiznetRef()).getGatewayIp();

        return gatewayIp;
    }

    if (transportId == FLPROG_TRANSPORT_RMII)
    {
        const IPAddress activeDns = rmiiRef().currentDns();
        if (activeDns != zero)
        {
            return activeDns;
        }

        if (rmiiRef().configuredDns() != zero)
        {
            return rmiiRef().configuredDns();
        }

        if (rmiiRef().currentGateway() != zero)
        {
            return rmiiRef().currentGateway();
        }

        return rmiiRef().configuredGateway();
    }

    return zero;
#endif
}

IPAddress FLProgNetworkInterface::dnsServerForRoute(
    uint8_t route,
    bool requireReady) const
{
    return dnsServerForTransport(
        transportIdForRoute(route, requireReady)
    );
}

uint8_t FLProgNetworkInterface::reserveSocket(
    const void *owner,
    uint8_t route,
    bool requireReady)
{
    const uint8_t transportId =
        transportIdForRoute(route, requireReady);

    if (transportId == FLPROG_TRANSPORT_NONE)
    {
        return FLPROG_SOCKET_INVALID_HANDLE;
    }

    return _socketManager.acquire(
        owner,
        transportId,
        requireReady
    );
}

// -----------------------------------------------------------------------------
// Настройка и управление Wi-Fi
// -----------------------------------------------------------------------------

// Обновляет SSID. Изменение настройки во время работы преобразуется в запрос
// переподключения; непосредственное обращение к аппаратуре остаётся внутри pool().
void FLProgNetworkInterface::setClientSsid(String ssid)
{
    if (_clientSsid == ssid)
    {
        return;
    }
    _clientSsid = ssid;
    if (_workMode && usesWifi())
    {
        _wifiStartRequest = true;
    }
}

void FLProgNetworkInterface::setClientPassword(String password)
{
    if (_clientPassword == password)
    {
        return;
    }
    _clientPassword = password;
    if (_workMode && usesWifi())
    {
        _wifiStartRequest = true;
    }
}

void FLProgNetworkInterface::markAccessPointConfigurationChanged()
{
    _apConfigBlocked = false;
    _apConfigGeneration++;
    if (_apConfigGeneration == 0) _apConfigGeneration = 1;
    if (_workMode && _apAllowed && !_manualStop) _apStartRequest = true;
}

void FLProgNetworkInterface::setAccessPointSsid(String ssid)
{
    if (_apSsid == ssid) return;
    _apSsid = ssid;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointPassword(String password)
{
    if (_apPassword == password) return;
    _apPassword = password;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::apMac(uint8_t m0, uint8_t m1, uint8_t m2, uint8_t m3, uint8_t m4, uint8_t m5)
{
    const uint8_t next[6] = {m0, m1, m2, m3, m4, m5};
    bool changed = false;
    for (uint8_t i = 0; i < 6; ++i)
    {
        if (_apMac[i] != next[i]) changed = true;
        _apMac[i] = next[i];
    }
    if (changed) markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::apDns(IPAddress ip)
{
    if (_apDns == ip) return;
    _apDns = ip;
    // Legacy FLProg_Ethernet stored this value but did not feed it into
    // the ESP SoftAP DHCP server. Keep that behavior for compatibility.
}

void FLProgNetworkInterface::setAccessPointChannel(uint8_t channel)
{
    if (channel < 1) channel = 1;
    if (channel > 13) channel = 13;
    if (_apChannel == channel) return;
    _apChannel = channel;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointHidden(bool value)
{
    if (_apHidden == value) return;
    _apHidden = value;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointMaxClients(uint8_t value)
{
    if (value == 0) value = 1;
#if defined(ESP8266)
    if (value > 8) value = 8;
#elif defined(ESP32)
    // Arduino-ESP32 2.x and 3.x document the legacy softAP API as 1..4.
    if (value > 4) value = 4;
#else
    // Preserve a portable desired value on unsupported platforms; no AP
    // backend will be started there.
    if (value > 8) value = 8;
#endif
    if (_apMaxClients == value) return;
    _apMaxClients = value;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointIP(IPAddress value)
{
    if (_apIp == value) return;
    _apIp = value;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointGateway(IPAddress value)
{
    if (_apGateway == value) return;
    _apGateway = value;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::setAccessPointSubnet(IPAddress value)
{
    if (_apSubnet == value) return;
    _apSubnet = value;
    markAccessPointConfigurationChanged();
}

void FLProgNetworkInterface::accessPointOn()
{
    // AP_RUNTIME_ON_FIX1:
    // Legacy/generated FLProg code may call apMode(true) on every scan while
    // the AP enable input remains HIGH.  Starting the AP is edge-driven: once
    // it is already allowed, repeated TRUE levels must not queue another AP
    // restart.  Configuration setters still request a restart independently
    // through markAccessPointConfigurationChanged().
    if (_apAllowed) return;

    _apAllowed = true;
    if (_workMode && !_manualStop) _apStartRequest = true;
}

void FLProgNetworkInterface::accessPointOff()
{
    _apAllowed = false;
    _apConfigBlocked = false;
    _apStartRequest = false;
#if FLPROG_NETWORK_HAS_WIFI
    WiFi.softAPdisconnect(false);
#endif
    _apStarted = false;
    _apReady = false;
    _apStationCount = 0;
    applyWifiRadioMode();
}

void FLProgNetworkInterface::accessPointMode(bool value)
{
    if (value) accessPointOn();
    else accessPointOff();
}

IPAddress FLProgNetworkInterface::accessPointIP() const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_apReady) return WiFi.softAPIP();
#endif
    return IPAddress(0, 0, 0, 0);
}

bool FLProgNetworkInterface::startWifiScan(bool showHidden)
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState == FLPROG_WIFI_SCAN_RUNNING ||
        _wifiScanState == FLPROG_WIFI_SCAN_QUEUED)
    {
        return false;
    }

    _wifiScanShowHidden = showHidden;
    _wifiScanCount = 0;
    _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_NONE;
    _wifiScanCancelRequest = false;
    // Preserve a pending clear request. clear + start before the next pool()
    // deterministically frees old results first and then starts the new scan.
    _wifiScanStartRequest = true;
    _wifiScanState = FLPROG_WIFI_SCAN_QUEUED;
    return true;
#else
    (void)showHidden;
    _wifiScanState = FLPROG_WIFI_SCAN_ERROR;
    _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_START;
    return false;
#endif
}

bool FLProgNetworkInterface::cancelWifiScan()
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState != FLPROG_WIFI_SCAN_RUNNING &&
        _wifiScanState != FLPROG_WIFI_SCAN_QUEUED)
    {
        return false;
    }
    _wifiScanCancelRequest = true;
    return true;
#else
    return false;
#endif
}

void FLProgNetworkInterface::clearWifiScanResults()
{
#if FLPROG_NETWORK_HAS_WIFI
    _wifiScanClearRequest = true;
    if (_wifiScanState == FLPROG_WIFI_SCAN_RUNNING ||
        _wifiScanState == FLPROG_WIFI_SCAN_QUEUED)
    {
        _wifiScanCancelRequest = true;
    }
#else
    _wifiScanCount = 0;
    _wifiScanState = FLPROG_WIFI_SCAN_IDLE;
    _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_NONE;
#endif
}

String FLProgNetworkInterface::wifiScanSsid(uint8_t index) const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState == FLPROG_WIFI_SCAN_DONE && index < (uint8_t)_wifiScanCount)
        return WiFi.SSID(index);
#else
    (void)index;
#endif
    return String();
}

int32_t FLProgNetworkInterface::wifiScanRssi(uint8_t index) const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState == FLPROG_WIFI_SCAN_DONE && index < (uint8_t)_wifiScanCount)
        return WiFi.RSSI(index);
#else
    (void)index;
#endif
    return 0;
}

int32_t FLProgNetworkInterface::wifiScanChannel(uint8_t index) const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState == FLPROG_WIFI_SCAN_DONE && index < (uint8_t)_wifiScanCount)
        return WiFi.channel(index);
#else
    (void)index;
#endif
    return 0;
}

uint8_t FLProgNetworkInterface::wifiScanEncryption(uint8_t index) const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiScanState == FLPROG_WIFI_SCAN_DONE && index < (uint8_t)_wifiScanCount)
        return (uint8_t)WiFi.encryptionType(index);
#else
    (void)index;
#endif
    return 0;
}

bool FLProgNetworkInterface::wifiScanHidden(uint8_t index) const
{
    return wifiScanSsid(index).length() == 0;
}

void FLProgNetworkInterface::clientOn()
{
    _wifiAllowed = true;

    // RC6-A-dev.3-fix1: enabling STA is a new user-requested connection
    // window. Do not inherit an expired global connect timer from the time
    // while the interface was intentionally disabled.
    if (_workMode && usesWifi())
    {
        _error = FLPROG_NETWORK_NOT_ERROR;
        _connectionStartTime = millis();
        _wifiStartRequest = true;
        if (!_manualStop)
        {
            _status = FLPROG_NETWORK_CONNECTING_STATUS;
        }
    }
}

void FLProgNetworkInterface::clientOff()
{
    _wifiAllowed = false;
#if FLPROG_NETWORK_HAS_WIFI
    WiFi.disconnect();
#endif
    _wifiStarted = false;
    _wifiReady = false;
    _wifiStartRequest = false;

    if (_activeInterface == FLPROG_NETWORK_ACTIVE_WIFI)
    {
        _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;
    }

    // If STA was the last enabled network transport, this is an intentional
    // idle state, not a failed connection attempt. Cancel the old timeout
    // window and clear its interface-level error. This is the EN=0 contract
    // used by the FLProg A2 WiFi block.
    if (!usesWifi() && !usesWiznet() && !usesRmii() && !_apAllowed)
    {
        _connectionStartTime = millis();
        _error = FLPROG_NETWORK_NOT_ERROR;
        if (_workMode)
        {
            _status = FLPROG_NETWORK_IDLE_STATUS;
        }
    }
}

void FLProgNetworkInterface::clientMode(bool value)
{
    if (value)
    {
        clientOn();
    }
    else
    {
        clientOff();
    }
}

// -----------------------------------------------------------------------------
// Политика приёма пакетов сокетами W5500
// -----------------------------------------------------------------------------

// Сохраняет требуемую политику BCASTB. Запись в регистры откладывается до
// обслуживания Ethernet-интерфейса методом pool().
void FLProgNetworkInterface::setBroadcastBlocking(bool value)
{
    if (_broadcastBlocking == value)
    {
        return;
    }

    _broadcastBlocking = value;
    _broadcastPolicyDirty = true;
}

// Изменяет только Sn_MR.BCASTB (бит 6). Биты протокола и остальные параметры,
// включая MFEN, остаются без изменений.
uint8_t FLProgNetworkInterface::prepareW5500SocketMode(uint8_t mode) const
{
    const uint8_t protocol = mode & 0x0F;

    if (
        protocol != 0x02 &&   // UDP
        protocol != 0x04      // MACRAW
    )
    {
        return mode;
    }

    if (_broadcastBlocking)
    {
        return mode | 0x40;   // Sn_MR.BCASTB
    }

    return mode & 0xBF;
}

// Изменяет BCASTB одного сокета W5500 только в режиме UDP или MACRAW.
// W5100/W5200 намеренно исключены, поскольку этот параметр относится к W5500.
bool FLProgNetworkInterface::setSocketBroadcastBlocking(
    uint8_t socket,
    bool value)
{
    if (!_ethernetStarted || !wiznetRef().isInit())
    {
        return false;
    }

    return wiznetRef().setSocketBroadcastBlocking(socket, value);
}

uint8_t FLProgNetworkInterface::applyBroadcastBlocking()
{
    if (!_ethernetStarted || !wiznetRef().isInit())
    {
        return FLPROG_NETWORK_WAIT;
    }

    wiznetRef().applyBroadcastBlocking(_broadcastBlocking);
    _broadcastPolicyDirty = false;

    return FLPROG_NETWORK_SUCCESS;
}

void FLProgNetworkInterface::updateWiznetSocketPolicies()
{
    if (!_ethernetStarted)
    {
        return;
    }

    // Если блокировка включена, сокеты проверяются при каждом pool(), чтобы новый
    // UDP/MACRAW-сокет получил политику уже на следующем цикле FLProg.
// При выключении блокировки достаточно одного прохода, чтобы снять BCASTB
// с уже открытых сокетов.
    if (
        _broadcastBlocking ||
        _broadcastPolicyDirty
    )
    {
        applyBroadcastBlocking();
    }
}

// -----------------------------------------------------------------------------
// Настройка Ethernet и IP
// -----------------------------------------------------------------------------

// После изменения вывода CS предыдущее состояние Ethernet нельзя считать
// достоверным, поэтому планируется перезапуск интерфейса.
void FLProgNetworkInterface::setPinCs(int pinCs)
{
    if (_pinCs == pinCs)
    {
        return;
    }
    _pinCs = pinCs;
    wiznetRef().setPinCs(pinCs);
    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);
    if (_workMode && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::setSpiBus(uint8_t bus)
{
    if (_spiBus == bus)
    {
        return;
    }

    _spiBus = bus;
    wiznetRef().setSpiBus(bus);

    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);

    if (_workMode && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::setSpiObject(SPIClass *object)
{
    wiznetRef().setSpiObject(object);
    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);

    if (_workMode && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::clearSpiObject()
{
    setSpiObject(nullptr);
}

void FLProgNetworkInterface::setWiznetSpiBackend(
    FLProgSpiBackend *backend)
{
    wiznetRef().setSpiBackend(backend);
    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;

    if (_workMode && usesEthernet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::useArduinoWiznetSpiBackend()
{
    setWiznetSpiBackend(nullptr);
}

void FLProgNetworkInterface::setSpiPins(int sck, int miso, int mosi)
{
    wiznetRef().setSpiPins(sck, miso, mosi);
    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);

    if (_workMode && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::clearSpiPins()
{
    wiznetRef().clearSpiPins();
    _ethernetStarted = false;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);

    if (_workMode && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::dhcpMode(bool value)
{
    if (_dhcp == value)
    {
        return;
    }

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    // Wired DHCP owns its own lease state. When leaving the previous DHCP
    // mode, stop that state machine before changing the common addressing
    // flag. In a Wi-Fi-only profile there is no Wiznet DHCP object at all.
    if (_dhcp)
    {
        wiznetDhcpRef().end(false);
    }
#endif

    _dhcp = value;

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (_workMode && usesEthernet())
    {
        _ethernetStartRequest = true;
    }
#endif

    // Native ESP Wi-Fi DHCP/static selection is applied in startWifi().
    // Therefore Wi-Fi-only builds only need to request a Wi-Fi restart.
    if (_workMode && usesWifi())
    {
        _wifiStartRequest = true;
    }
}

void FLProgNetworkInterface::localIP(IPAddress ip)
{
    _configuredIp = ip;
    if (!_dhcp && _workMode)
    {
        if (usesWifi())
        {
            _wifiStartRequest = true;
        }
        if (usesEthernet())
        {
            _ethernetStartRequest = true;
        }
    }
}

IPAddress FLProgNetworkInterface::wifiIP() const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiReady)
    {
        return WiFi.localIP();
    }
#endif
    return IPAddress(0, 0, 0, 0);
}

IPAddress FLProgNetworkInterface::wiznetIP() const
{
    return _ethernetReady
        ? _wiznetIpCache
        : IPAddress(0, 0, 0, 0);
}

IPAddress FLProgNetworkInterface::rmiiIP() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    return IPAddress(0, 0, 0, 0);
#else
    return _rmiiReady
        ? rmiiRef().currentIP()
        : IPAddress(0, 0, 0, 0);
#endif
}

IPAddress FLProgNetworkInterface::ethernetIP() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    return IPAddress(0, 0, 0, 0);
#else
    const uint8_t route = resolveRoute(FLPROG_NETWORK_ROUTE_ETHERNET);

    if (route == FLPROG_NETWORK_ACTIVE_WIZNET)
    {
        return wiznetIP();
    }

    if (route == FLPROG_NETWORK_ACTIVE_RMII)
    {
        return rmiiIP();
    }

    return IPAddress(0, 0, 0, 0);
#endif
}

bool FLProgNetworkInterface::interfaceIsReady(uint8_t interfaceId) const
{
    if (interfaceId == FLPROG_NETWORK_ACTIVE_WIFI)
    {
        return _wifiReady;
    }

    if (interfaceId == FLPROG_NETWORK_ACTIVE_WIFI_AP)
    {
        return _apReady;
    }

    if (interfaceId == FLPROG_NETWORK_ACTIVE_WIZNET)
    {
        return _ethernetReady;
    }

    if (interfaceId == FLPROG_NETWORK_ACTIVE_RMII)
    {
        return _rmiiReady;
    }

    if (interfaceId == FLPROG_NETWORK_ACTIVE_BOTH)
    {
        return _wifiReady && (_ethernetReady || _rmiiReady);
    }

    if (interfaceId == FLPROG_NETWORK_ACTIVE_MULTI)
    {
        const uint8_t count =
            (_wifiReady ? 1 : 0) +
            (_ethernetReady ? 1 : 0) +
            (_rmiiReady ? 1 : 0);
        return count > 1;
    }

    return false;
}

uint8_t FLProgNetworkInterface::resolveRoute(uint8_t route) const
{
    if (route == FLPROG_NETWORK_ROUTE_WIFI)
    {
        return _wifiReady
            ? FLPROG_NETWORK_ACTIVE_WIFI
            : FLPROG_NETWORK_ACTIVE_NONE;
    }

    if (route == FLPROG_NETWORK_ROUTE_WIFI_AP)
    {
        return _apReady
            ? FLPROG_NETWORK_ACTIVE_WIFI_AP
            : FLPROG_NETWORK_ACTIVE_NONE;
    }

    if (route == FLPROG_NETWORK_ROUTE_WIZNET)
    {
        return _ethernetReady
            ? FLPROG_NETWORK_ACTIVE_WIZNET
            : FLPROG_NETWORK_ACTIVE_NONE;
    }

    if (route == FLPROG_NETWORK_ROUTE_RMII)
    {
        return _rmiiReady
            ? FLPROG_NETWORK_ACTIVE_RMII
            : FLPROG_NETWORK_ACTIVE_NONE;
    }

    if (route == FLPROG_NETWORK_ROUTE_ETHERNET)
    {
        if (_wiredPriority == FLPROG_NETWORK_WIRED_PRIORITY_RMII)
        {
            if (_rmiiReady) return FLPROG_NETWORK_ACTIVE_RMII;
            if (_ethernetReady) return FLPROG_NETWORK_ACTIVE_WIZNET;
        }
        else
        {
            if (_ethernetReady) return FLPROG_NETWORK_ACTIVE_WIZNET;
            if (_rmiiReady) return FLPROG_NETWORK_ACTIVE_RMII;
        }

        return FLPROG_NETWORK_ACTIVE_NONE;
    }

    // AUTO / DEFAULT prefer a wired route, then Wi-Fi.
    const uint8_t wired = resolveRoute(FLPROG_NETWORK_ROUTE_ETHERNET);
    if (wired != FLPROG_NETWORK_ACTIVE_NONE)
    {
        return wired;
    }

    return _wifiReady
        ? FLPROG_NETWORK_ACTIVE_WIFI
        : FLPROG_NETWORK_ACTIVE_NONE;
}

IPAddress FLProgNetworkInterface::localIP() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (_wifiReady)
    {
        return wifiIP();
    }
    return _configuredIp;
#else
    const uint8_t route = resolveRoute(FLPROG_NETWORK_ROUTE_DEFAULT);

    if (route == FLPROG_NETWORK_ACTIVE_WIFI)
    {
        return wifiIP();
    }

    if (route == FLPROG_NETWORK_ACTIVE_WIZNET)
    {
        return wiznetIP();
    }

    if (route == FLPROG_NETWORK_ACTIVE_RMII)
    {
        return rmiiIP();
    }

    return _configuredIp;
#endif
}


const char *FLProgNetworkInterface::deviceModel() const
{
    for (uint8_t i=0;i<FLPROG_NETWORK_MAX_MANAGED_SERVICES;++i) {
        const FLProgNetworkService *service=_managedServices[i];
        if (service) { const char *value=service->deviceModel(); if(value) return value; }
    }
    return "";
}
const char *FLProgNetworkInterface::firmwareVersion() const
{
    for (uint8_t i=0;i<FLPROG_NETWORK_MAX_MANAGED_SERVICES;++i) {
        const FLProgNetworkService *service=_managedServices[i];
        if (service && service->deviceModel()!=nullptr) {
            const char *value=service->firmwareVersion(); return value ? value : "";
        }
    }
    return "";
}

IPAddress FLProgNetworkInterface::activeGateway() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiReady) return WiFi.gatewayIP();
#endif
#else
    const uint8_t route = resolveRoute(FLPROG_NETWORK_ROUTE_DEFAULT);
#if FLPROG_NETWORK_HAS_WIFI
    if (route == FLPROG_NETWORK_ACTIVE_WIFI) return WiFi.gatewayIP();
#endif
    if (route == FLPROG_NETWORK_ACTIVE_WIZNET) {
        if (_dhcp && wiznetDhcpRef().bound()) return wiznetDhcpRef().gatewayIP();
        return const_cast<FLProgWiznetHardware &>(wiznetRef()).getGatewayIp();
    }
    if (route == FLPROG_NETWORK_ACTIVE_RMII) return rmiiRef().currentGateway();
#endif
    return IPAddress(0, 0, 0, 0);
}

IPAddress FLProgNetworkInterface::activeSubnet() const
{
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiReady) return WiFi.subnetMask();
#endif
#else
    const uint8_t route = resolveRoute(FLPROG_NETWORK_ROUTE_DEFAULT);
#if FLPROG_NETWORK_HAS_WIFI
    if (route == FLPROG_NETWORK_ACTIVE_WIFI) return WiFi.subnetMask();
#endif
    if (route == FLPROG_NETWORK_ACTIVE_WIZNET) {
        if (_dhcp && wiznetDhcpRef().bound()) return wiznetDhcpRef().subnetMask();
        return const_cast<FLProgWiznetHardware &>(wiznetRef()).getSubnetMask();
    }
    if (route == FLPROG_NETWORK_ACTIVE_RMII) return rmiiRef().currentSubnet();
#endif
    return IPAddress(0, 0, 0, 0);
}

void FLProgNetworkInterface::dns(IPAddress ip)
{
    _dnsIp = ip;
}

void FLProgNetworkInterface::subnet(IPAddress ip)
{
    _subnetIp = ip;
}

void FLProgNetworkInterface::gateway(IPAddress ip)
{
    _gatewayIp = ip;
}

void FLProgNetworkInterface::mac(uint8_t m0, uint8_t m1, uint8_t m2, uint8_t m3, uint8_t m4, uint8_t m5)
{
    _macAddress[0] = m0;
    _macAddress[1] = m1;
    _macAddress[2] = m2;
    _macAddress[3] = m3;
    _macAddress[4] = m4;
    _macAddress[5] = m5;
}

void FLProgNetworkInterface::mac(uint8_t *address)
{
    if (address == nullptr)
    {
        return;
    }
    for (uint8_t i = 0; i < 6; i++)
    {
        _macAddress[i] = address[i];
    }
}

// Проверяет, задан ли MAC-адрес пользователем. Шесть нулевых байтов означают:
// «сформировать локально администрируемый адрес автоматически».
bool FLProgNetworkInterface::macIsEmpty() const
{
    for (uint8_t i = 0; i < 6; i++)
    {
        if (_macAddress[i] != 0)
        {
            return false;
        }
    }
    return true;
}

// Формирует локально администрируемый unicast MAC-адрес. На платформах с Wi-Fi
// аппаратный MAC Wi-Fi используется как источник уникальности и изменяется так,
// чтобы Ethernet-интерфейс не получил тот же адрес.
void FLProgNetworkInterface::makeDefaultEthernetMac()
{
    if (!macIsEmpty())
    {
        return;
    }

#if FLPROG_NETWORK_HAS_WIFI
    WiFi.macAddress(_macAddress);
#else
    uint32_t seed = micros();
    _macAddress[0] = 0x02;
    _macAddress[1] = 0x46;
    _macAddress[2] = 0x4C;
    _macAddress[3] = (seed >> 16) & 0xFF;
    _macAddress[4] = (seed >> 8) & 0xFF;
    _macAddress[5] = seed & 0xFF;
#endif

    // Принудительно формируем локально администрируемый unicast-адрес и делаем
    // его отличным от встроенного MAC-адреса Wi-Fi.
    _macAddress[0] = (_macAddress[0] | 0x02) & 0xFE;
    _macAddress[5] ^= 0x01;
}

// -----------------------------------------------------------------------------
// Общий автомат выбора сетевого транспорта
// -----------------------------------------------------------------------------

// Переключает Wi-Fi, Ethernet и AUTO. Изменение режима планирует запуск
// нужного транспорта без блокировки цикла FLProg.
void FLProgNetworkInterface::wiznetMode(bool value)
{
    if (_wiznetAllowed == value)
    {
        return;
    }

    _wiznetAllowed = value;

    if (!value)
    {
        _ethernetReady = false;
        _ethernetStartRequest = false;
    }
    else if (_workMode && !_manualStop && usesWiznet())
    {
        _ethernetStartRequest = true;
    }
}

void FLProgNetworkInterface::rmiiMode(bool value)
{
    if (_rmiiAllowed == value)
    {
        return;
    }

    _rmiiAllowed = value;

    if (!value)
    {
        rmiiRef().end();
        _rmiiStarted = false;
        _rmiiReady = false;
        _rmiiStartRequest = false;
    }
    else if (_workMode && !_manualStop && usesRmii())
    {
        _rmiiStartRequest = true;
    }
}

void FLProgNetworkInterface::setWiredPriority(uint8_t value)
{
    if (value > FLPROG_NETWORK_WIRED_PRIORITY_RMII)
    {
        value = FLPROG_NETWORK_WIRED_PRIORITY_WIZNET;
    }

    _wiredPriority = value;
}

void FLProgNetworkInterface::networkMode(uint8_t value)
{
    if (value > FLPROG_NETWORK_MODE_BOTH)
    {
        value = FLPROG_NETWORK_MODE_WIFI;
    }
    if (_networkMode == value)
    {
        return;
    }
    _networkMode = value;
    _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;
    _error = FLPROG_NETWORK_NOT_ERROR;
    if (_workMode && !_manualStop)
    {
        requestSelectedInterfaces();
    }
}

void FLProgNetworkInterface::workMode(bool value)
{
    if (_workMode == value)
    {
        return;
    }
    _workMode = value;
    if (!_workMode)
    {
        disconnect();
        _manualStop = false;
        _status = FLPROG_NETWORK_DISABLED_STATUS;
        return;
    }
    _manualStop = false;
    _status = FLPROG_NETWORK_IDLE_STATUS;
    if (_autoConnect)
    {
        connect();
    }
}

// Преобразует уровень входа CON FLProg в событие положительного фронта.
void FLProgNetworkInterface::connectInput(bool value)
{
    if (value && !_prevConnectInput)
    {
        connect();
    }
    _prevConnectInput = value;
}

// Преобразует уровень входа DIS FLProg в событие положительного фронта.
void FLProgNetworkInterface::disconnectInput(bool value)
{
    if (value && !_prevDisconnectInput)
    {
        disconnect();
    }
    _prevDisconnectInput = value;
}

void FLProgNetworkInterface::requestSelectedInterfaces()
{
    if (usesWifi())
    {
        _wifiStartRequest = true;
    }
    if (_apAllowed)
    {
        _apStartRequest = true;
    }
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (usesWiznet())
    {
        _ethernetStartRequest = true;
    }
    if (usesRmii())
    {
        _rmiiStartRequest = true;
    }
#endif
}

// Снимает ручную остановку и запрашивает все транспорты, разрешённые networkMode().
void FLProgNetworkInterface::connect()
{
    if (!_workMode)
    {
        return;
    }
    _manualStop = false;
    _error = FLPROG_NETWORK_NOT_ERROR;
    _status = FLPROG_NETWORK_CONNECTING_STATUS;
    _connectionStartTime = millis();
    requestSelectedInterfaces();
}

// Останавливает автоматические подключения. У Ethernet-драйвера нет переносимого
// метода end(), поэтому остановка проводного интерфейса представляется логически.
void FLProgNetworkInterface::disconnect()
{
#if FLPROG_NETWORK_HAS_WIFI
    WiFi.disconnect();
#endif
#if FLPROG_NETWORK_HAS_WIFI
    WiFi.softAPdisconnect(false);
#endif
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    rmiiRef().end();
    wiznetDhcpRef().end(_dhcp);
#endif
    _wifiStarted = false;
    _apStarted = false;
    _ethernetStarted = false;
    _rmiiStarted = false;
    _ethernetConfigured = false;
    _wifiReady = false;
    _apReady = false;
    _ethernetReady = false;
    _rmiiReady = false;
    _wifiSessionRestartGate = false;
    _wifiSessionRestartSawDown = false;
    _wifiSessionRestartStarted = 0;
    _wifiStartRequest = false;
    _apStartRequest = false;
    _ethernetStartRequest = false;
    _rmiiStartRequest = false;
    _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;
    _manualStop = true;
    _status = _workMode ? FLPROG_NETWORK_IDLE_STATUS : FLPROG_NETWORK_DISABLED_STATUS;
    _error = FLPROG_NETWORK_NOT_ERROR;
}

// Запускает одну попытку Wi-Fi подключения. Цикла ожидания здесь нет;
// готовность проверяется updateWifi() в последующих вызовах pool().
void FLProgNetworkInterface::startWifi()
{
    _wifiStartRequest = false;
    _lastWifiAttempt = millis();

#if FLPROG_NETWORK_HAS_WIFI
    // RC6-A-dev.3: invalidate only an already live STA session. Initial start
    // does not advance generation. invalidateSessions() is generation-only and
    // never changes READY/IP/link snapshots.
    const bool restartingLiveTransport =
        _wifiReady || _wifiTransport.isReady();

    if (_clientSsid.length() == 0)
    {
        _wifiStarted = false;
        _wifiReady = false;
        _error = FLPROG_NETWORK_WIFI_SSID_ERROR;
        return;
    }

#if FLPROG_NETWORK_HAS_WIFI
    applyWifiRadioMode();
#endif

    if (!_dhcp && _configuredIp != IPAddress(0, 0, 0, 0))
    {
        IPAddress gatewayIp = _gatewayIp;
        IPAddress dnsIp = _dnsIp;
        if (gatewayIp == IPAddress(0, 0, 0, 0))
        {
            gatewayIp = _configuredIp;
            gatewayIp[3] = 1;
        }
        if (dnsIp == IPAddress(0, 0, 0, 0))
        {
            dnsIp = gatewayIp;
        }
        WiFi.config(_configuredIp, gatewayIp, _subnetIp, dnsIp);
    }

#if FLPROG_NETWORK_HAS_WIFI
    // FLProg_Network owns the reconnect policy and timing.
    // Native ESP auto-reconnect must stay disabled, otherwise
    // AUTO_RECONNECT=0 and RECONNECT_MS cannot be deterministic.
    WiFi.setAutoReconnect(false);
#endif
    if (restartingLiveTransport)
    {
        // RC6-A-dev.3-fix3:
        // 1) invalidate the old socket generation immediately;
        // 2) block fresh socket leases during the short interval in which the
        //    ESP Wi-Fi stack may still report WL_CONNECTED;
        // 3) force a real STA down/up phase so the gate has a deterministic
        //    release point and a replacement TCP connection is opened once.
        _wifiTransport.invalidateSessions();
        _wifiSessionRestartGate = true;
        _wifiSessionRestartSawDown = false;
        _wifiSessionRestartStarted = millis();
        WiFi.disconnect();
    }
    WiFi.begin(_clientSsid.c_str(), _clientPassword.c_str());
    _wifiStarted = true;
#else
    _wifiStarted = false;
    _wifiReady = false;
#endif
}

void FLProgNetworkInterface::applyWifiRadioMode()
{
#if FLPROG_NETWORK_HAS_WIFI
    const bool wantSta = usesWifi() || _wifiStarted;
    const bool wantAp = _apAllowed || _apStarted || _apStartRequest;

    uint8_t desiredMode = 0U;
    if (wantSta && wantAp) desiredMode = 3U;
    else if (wantAp) desiredMode = 2U;
    else if (wantSta) desiredMode = 1U;

    if (desiredMode == 0U)
    {
        _wifiRadioMode = 0xFFU;
        return;
    }
    if (_wifiRadioMode == desiredMode) return;

    bool applied = false;
    if (desiredMode == 3U) applied = WiFi.mode(WIFI_AP_STA);
    else if (desiredMode == 2U) applied = WiFi.mode(WIFI_AP);
    else applied = WiFi.mode(WIFI_STA);

    if (applied) _wifiRadioMode = desiredMode;
#endif
}

void FLProgNetworkInterface::startAccessPoint()
{
    _apStartRequest = false;
    _lastApAttempt = millis();
#if FLPROG_NETWORK_HAS_WIFI
    const bool restarting = _apStarted || _apReady;
    _apLastError = FLPROG_WIFI_AP_ERROR_NONE;

    if (!_apAllowed)
    {
        return;
    }

    if (_apSsid.length() == 0 || _apSsid.length() > 32)
    {
        _apLastError = FLPROG_WIFI_AP_ERROR_INVALID_SSID;
        _apConfigBlocked = true;
        _apFailureCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_FAILED;
        return;
    }

    if (_apPassword.length() != 0 &&
        (_apPassword.length() < 8 || _apPassword.length() > 63))
    {
        _apLastError = FLPROG_WIFI_AP_ERROR_INVALID_PASSWORD;
        _apConfigBlocked = true;
        _apFailureCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_FAILED;
        return;
    }

    applyWifiRadioMode();

    // Validate/apply the IP tuple before disconnecting a live AP. This keeps
    // the current AP alive when softAPConfig() rejects the new configuration.
    if (!WiFi.softAPConfig(_apIp, _apGateway, _apSubnet))
    {
        _apLastError = FLPROG_WIFI_AP_ERROR_CONFIG_FAILED;
        _apConfigBlocked = true;
        _apFailureCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_FAILED;
        return;
    }

    if (restarting)
    {
        // Configuration has already been validated. Existing AP-bound sockets
        // must not survive the intentional AP restart with an old generation.
        _wifiApTransport.invalidateSessions();
        WiFi.softAPdisconnect(false);
        _apStarted = false;
        _apReady = false;
    }

    bool customApMac = false;
    for (uint8_t i = 0; i < 6; ++i)
    {
        if (_apMac[i] != 0)
        {
            customApMac = true;
            break;
        }
    }
    if (customApMac)
    {
#if defined(ESP8266)
        wifi_set_macaddr(SOFTAP_IF, _apMac);
#elif defined(ESP32)
        esp_wifi_set_mac(WIFI_IF_AP, _apMac);
#endif
    }

    const char *password = _apPassword.length() == 0 ? nullptr : _apPassword.c_str();
    _apStarted = WiFi.softAP(
        _apSsid.c_str(),
        password,
        _apChannel,
        _apHidden,
        _apMaxClients);
    _apReady = _apStarted && WiFi.softAPIP() != IPAddress(0, 0, 0, 0);
    _apStationCount = _apReady ? WiFi.softAPgetStationNum() : 0;

    if (!_apStarted || !_apReady)
    {
        _apStarted = false;
        _apReady = false;
        _apLastError = FLPROG_WIFI_AP_ERROR_START_FAILED;
        _apFailureCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_FAILED;
        return;
    }

    _apConfigBlocked = false;
    _apAppliedSsid = _apSsid;
    _apAppliedChannel = _apChannel;
    _apAppliedHidden = _apHidden;
    _apAppliedMaxClients = _apMaxClients;
    _apAppliedIp = _apIp;
    _apAppliedGateway = _apGateway;
    _apAppliedSubnet = _apSubnet;
    _apAppliedConfigGeneration = _apConfigGeneration;
    _apAppliedConfigValid = true;
    _apStartCount++;
    if (restarting)
    {
        _apRestartCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_RESTARTED;
    }
#else
    _apStarted = false;
    _apReady = false;
#endif
}

void FLProgNetworkInterface::updateWifiExtended()
{
#if FLPROG_NETWORK_HAS_WIFI
    _apReady = _apStarted && WiFi.softAPIP() != IPAddress(0, 0, 0, 0);

    if (_wifiReady != _prevWifiReadyForEvents)
    {
        _wifiEvents |= _wifiReady ? FLPROG_WIFI_EVENT_STA_UP : FLPROG_WIFI_EVENT_STA_DOWN;
        _prevWifiReadyForEvents = _wifiReady;
    }

    if (_apReady != _prevApReady)
    {
        _wifiEvents |= _apReady ? FLPROG_WIFI_EVENT_AP_STARTED : FLPROG_WIFI_EVENT_AP_STOPPED;
        _prevApReady = _apReady;
    }

    const uint8_t stations = _apReady ? WiFi.softAPgetStationNum() : 0;
    if (stations > _apStationCount)
    {
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_CLIENT_JOINED;
        _apClientJoinCount += (uint32_t)(stations - _apStationCount);
    }
    else if (stations < _apStationCount)
    {
        _wifiEvents |= FLPROG_WIFI_EVENT_AP_CLIENT_LEFT;
        _apClientLeaveCount += (uint32_t)(_apStationCount - stations);
    }
    _apStationCount = stations;

    if (_wifiScanCancelRequest)
    {
        if (_wifiScanState == FLPROG_WIFI_SCAN_RUNNING) WiFi.scanDelete();
        _wifiScanCancelRequest = false;
        _wifiScanStartRequest = false;
        _wifiScanCount = 0;
        _wifiScanState = FLPROG_WIFI_SCAN_CANCELED;
        _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_NONE;
        _wifiScanCancelCount++;
        _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_CANCELED;
    }

    if (_wifiScanClearRequest)
    {
        WiFi.scanDelete();
        _wifiScanClearRequest = false;
        _wifiScanCancelRequest = false;
        _wifiScanCount = 0;
        _wifiScanState = _wifiScanStartRequest ?
            FLPROG_WIFI_SCAN_QUEUED : FLPROG_WIFI_SCAN_IDLE;
        _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_NONE;
    }

    if (_wifiScanState == FLPROG_WIFI_SCAN_QUEUED && _wifiScanStartRequest)
    {
        WiFi.scanDelete();
        _wifiScanStartRequest = false;
        _wifiScanStartCount++;
        _wifiScanStartedAt = millis();
        const int16_t result = WiFi.scanNetworks(true, _wifiScanShowHidden);

        if (result >= 0)
        {
            _wifiScanCount = result;
            _wifiScanState = FLPROG_WIFI_SCAN_DONE;
            _wifiScanCompleteCount++;
            _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_DONE;
        }
        else if (result == -1)
        {
            _wifiScanState = FLPROG_WIFI_SCAN_RUNNING;
        }
        else
        {
            _wifiScanCount = 0;
            _wifiScanState = FLPROG_WIFI_SCAN_ERROR;
            _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_START;
            _wifiScanFailureCount++;
            _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_FAILED;
        }
    }

    if (_wifiScanState == FLPROG_WIFI_SCAN_RUNNING)
    {
        const int16_t result = WiFi.scanComplete();
        if (result >= 0)
        {
            _wifiScanCount = result;
            _wifiScanState = FLPROG_WIFI_SCAN_DONE;
            _wifiScanCompleteCount++;
            _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_DONE;
        }
        else if (result < -1)
        {
            _wifiScanCount = 0;
            _wifiScanState = FLPROG_WIFI_SCAN_ERROR;
            _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_RUNTIME;
            _wifiScanFailureCount++;
            _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_FAILED;
        }
        else if ((uint32_t)(millis() - _wifiScanStartedAt) >= FLPROG_WIFI_SCAN_TIMEOUT_MS)
        {
            WiFi.scanDelete();
            _wifiScanCount = 0;
            _wifiScanState = FLPROG_WIFI_SCAN_ERROR;
            _wifiScanLastError = FLPROG_WIFI_SCAN_ERROR_TIMEOUT;
            _wifiScanFailureCount++;
            _wifiScanTimeoutCount++;
            _wifiEvents |= FLPROG_WIFI_EVENT_SCAN_FAILED;
        }
    }
#else
    _apReady = false;
#endif
}

// Start or reconfigure the Wiznet interface. DHCP is serviced incrementally
// by FLProgWiznetDhcp and never calls Arduino Ethernet.begin().
// Запускает или перенастраивает Wiznet. DHCP обслуживается пошагово через
// FLProgWiznetDhcp и больше не вызывает Arduino Ethernet.begin().
void FLProgNetworkInterface::startEthernet()
{
    _ethernetStartRequest = false;
    _lastEthernetAttempt = millis();

    const bool restartingLiveTransport =
        _ethernetReady || wiznetTransportRef().isReady();

    makeDefaultEthernetMac();

    wiznetRef().setPinCs(_pinCs);
    wiznetRef().setSpiBus(_spiBus);

    if (
        !wiznetRef().spiBusAvailable() ||
        wiznetRef().effectivePinCs() < 0
    )
    {
        _ethernetStarted = false;
        _ethernetReady = false;
        _error = FLPROG_NETWORK_ETHERNET_HARDWARE_ERROR;
        return;
    }

    if (restartingLiveTransport)
    {
        wiznetTransportRef().invalidateSessions();
    }

    _ethernetStarted = true;
    _ethernetReady = false;
    _ethernetConfigured = false;
    _wiznetIpCache = IPAddress(0, 0, 0, 0);

    if (_dhcp)
    {
        wiznetDhcpRef().setAcquireTimeout(_dhcpTimeout);
        wiznetDhcpRef().setResponseTimeout(_responseDhcpTimeout);
        wiznetDhcpRef().begin(*this);
        _error = FLPROG_NETWORK_NOT_ERROR;
        return;
    }

    wiznetDhcpRef().end(false);
    _error = FLPROG_NETWORK_NOT_ERROR;
}

void FLProgNetworkInterface::startRmii()
{
    _rmiiStartRequest = false;
    _lastRmiiAttempt = millis();

    const bool restartingLiveTransport =
        _rmiiReady || rmiiTransportRef().isReady();
    if (restartingLiveTransport)
    {
        rmiiTransportRef().invalidateSessions();
    }

    const uint8_t result = rmiiRef().begin();
    _rmiiStarted = rmiiRef().started() || result == FLPROG_RMII_WAIT;
    _rmiiReady = rmiiRef().isReady();

    if (result == FLPROG_RMII_UNSUPPORTED)
    {
        _error = FLPROG_NETWORK_RMII_UNSUPPORTED_ERROR;
    }
    else if (result == FLPROG_RMII_ERROR)
    {
        _error =
            (rmiiRef().error() == FLPROG_RMII_ERROR_CONFIG)
                ? FLPROG_NETWORK_RMII_CONFIG_ERROR
                : FLPROG_NETWORK_RMII_BEGIN_ERROR;
    }
}

void FLProgNetworkInterface::updateRmii()
{
    if (!_rmiiAllowed)
    {
        _rmiiReady = false;
        return;
    }

    if (!_rmiiStarted && !_rmiiStartRequest)
    {
        _rmiiReady = false;
        return;
    }

    const uint8_t result = rmiiRef().pool();
    _rmiiStarted = rmiiRef().started() || result == FLPROG_RMII_WAIT;
    _rmiiReady = rmiiRef().isReady();

    if (result == FLPROG_RMII_UNSUPPORTED)
    {
        _error = FLPROG_NETWORK_RMII_UNSUPPORTED_ERROR;
    }
    else if (result == FLPROG_RMII_ERROR)
    {
        _error =
            (rmiiRef().error() == FLPROG_RMII_ERROR_CONFIG)
                ? FLPROG_NETWORK_RMII_CONFIG_ERROR
                : FLPROG_NETWORK_RMII_BEGIN_ERROR;
    }
}

bool FLProgNetworkInterface::configureStaticEthernet()
{
    if (_ethernetConfigured)
    {
        return true;
    }

    const uint8_t hwState = wiznetRef().init();

    if (hwState == FLPROG_WIZNET_HW_WAIT)
    {
        return false;
    }

    if (hwState != FLPROG_WIZNET_HW_READY)
    {
        _error = wiznetRef().spiBusAvailable()
            ? FLPROG_NETWORK_ETHERNET_HARDWARE_ERROR
            : FLPROG_NETWORK_ETHERNET_SPI_BUS_ERROR;

        return false;
    }

    IPAddress ip = _configuredIp;
    IPAddress gatewayIp = _gatewayIp;

    if (gatewayIp == IPAddress(0, 0, 0, 0))
    {
        gatewayIp = ip;
        gatewayIp[3] = 1;
    }

    wiznetRef().clearSpiOperationStatus();

    wiznetRef().setNetSettings(
        _macAddress,
        ip,
        gatewayIp,
        _subnetIp
    );

    wiznetRef().setRetransmissionTime(_retransmissionTime);
    wiznetRef().setRetransmissionCount(_retransmissionCount);

    if (wiznetRef().spiOperationStatus() == FLPROG_SPI_STATUS_BUSY)
    {
        return false;
    }

    if (wiznetRef().spiOperationStatus() != FLPROG_SPI_STATUS_OK)
    {
        _error = FLPROG_NETWORK_ETHERNET_SPI_BUS_ERROR;
        return false;
    }

    _wiznetIpCache = ip;
    _ethernetConfigured = true;
    return true;
}

void FLProgNetworkInterface::updateWifi()
{
#if FLPROG_NETWORK_HAS_WIFI
    const bool nativeReady =
        _wifiStarted && (WiFi.status() == WL_CONNECTED);

    _wifiReady = nativeReady;

    if (_wifiSessionRestartGate)
    {
        if (!nativeReady)
        {
            _wifiSessionRestartSawDown = true;
        }
        else if (_wifiSessionRestartSawDown)
        {
            // Native STA completed the required down -> up transition. New
            // socket leases may now bind to the recovered transport.
            _wifiSessionRestartGate = false;
            _wifiSessionRestartSawDown = false;
            _wifiSessionRestartStarted = 0;
        }
        else if (
            _wifiSessionRestartStarted != 0 &&
            (uint32_t)(millis() - _wifiSessionRestartStarted) >= 2000UL)
        {
            // Defensive escape hatch for cores that do not expose the brief
            // disconnected status after an explicit WiFi.disconnect()/begin().
            // The old generation was already invalidated, so releasing the
            // session gate here still cannot resurrect the old lease.
            _wifiSessionRestartGate = false;
            _wifiSessionRestartStarted = 0;
        }
    }
#else
    _wifiReady = false;
    _wifiSessionRestartGate = false;
    _wifiSessionRestartSawDown = false;
    _wifiSessionRestartStarted = 0;
#endif
}

// Обновляет состояние W5500, физического линка и IP, а в режиме DHCP
// периодически обслуживает продление аренды адреса.
void FLProgNetworkInterface::updateEthernet()
{
    if (!_ethernetStarted)
    {
        _ethernetReady = false;
        return;
    }

    const uint32_t now = millis();

    // Собственный DHCP выполняет шаг на каждом цикле FLProg. В отличие от старого
    // Ethernet.begin()/maintain() он не блокирует loop и работает через любой
    // FLProgSpiBackend и поддерживаемую SPI-шину.
    if (_dhcp)
    {
        wiznetDhcpRef().pool();

        if (wiznetDhcpRef().failed())
        {
            _ethernetReady = false;
            _wiznetIpCache = IPAddress(0, 0, 0, 0);
            _error = FLPROG_NETWORK_ETHERNET_DHCP_ERROR;
            return;
        }

        if (wiznetDhcpRef().bound())
        {
            _wiznetIpCache = wiznetDhcpRef().leasedIP();
            _ethernetConfigured = true;
        }
        else
        {
            _wiznetIpCache = IPAddress(0, 0, 0, 0);
            _ethernetConfigured = false;
            _ethernetReady = false;
        }
    }
    else if (!configureStaticEthernet())
    {
        if (wiznetRef().spiOperationStatus() != FLPROG_SPI_STATUS_BUSY)
        {
            _ethernetReady = false;
        }
        return;
    }

    if ((uint32_t)(now - _lastEthernetCheck) < _checkStatusPeriod)
    {
        if (_dhcp && wiznetDhcpRef().bound())
        {
            _ethernetReady = true;
        }
        return;
    }

    _lastEthernetCheck = now;

    const uint8_t link = wiznetRef().getLinkStatus();

    if (wiznetRef().lastSpiStatus() == FLPROG_SPI_STATUS_BUSY)
    {
        return;
    }

    if (link == FLPROG_WIZNET_LINK_OFF)
    {
        _ethernetReady = false;
        _error = FLPROG_NETWORK_ETHERNET_LINK_ERROR;
        return;
    }

    if (
        link != FLPROG_WIZNET_LINK_ON &&
        !(
            link == FLPROG_WIZNET_LINK_UNKNOWN &&
            wiznetRef().chipCode() == FLPROG_WIZNET_CHIP_W5100
        )
    )
    {
        _ethernetReady = false;
        _error = FLPROG_NETWORK_ETHERNET_HARDWARE_ERROR;
        return;
    }

    _ethernetReady =
        _wiznetIpCache != IPAddress(0, 0, 0, 0);

    if (_ethernetReady)
    {
        _error = FLPROG_NETWORK_NOT_ERROR;
    }
}

void FLProgNetworkInterface::updateActiveInterface()
{
    _activeInterface = FLPROG_NETWORK_ACTIVE_NONE;

    if (_networkMode == FLPROG_NETWORK_MODE_WIFI)
    {
        if (_wifiReady)
        {
            _activeInterface = FLPROG_NETWORK_ACTIVE_WIFI;
        }
        return;
    }

    if (_networkMode == FLPROG_NETWORK_MODE_ETHERNET)
    {
        _activeInterface = resolveRoute(FLPROG_NETWORK_ROUTE_ETHERNET);
        return;
    }

    if (_networkMode == FLPROG_NETWORK_MODE_BOTH)
    {
        const uint8_t count =
            (_wifiReady ? 1 : 0) +
            (_ethernetReady ? 1 : 0) +
            (_rmiiReady ? 1 : 0);

        if (count > 1)
        {
            // Keep the legacy BOTH value for Wi-Fi + exactly one wired link.
            if (
                count == 2 &&
                _wifiReady &&
                (_ethernetReady != _rmiiReady)
            )
            {
                _activeInterface = FLPROG_NETWORK_ACTIVE_BOTH;
            }
            else
            {
                _activeInterface = FLPROG_NETWORK_ACTIVE_MULTI;
            }
        }
        else if (_wifiReady)
        {
            _activeInterface = FLPROG_NETWORK_ACTIVE_WIFI;
        }
        else if (_ethernetReady)
        {
            _activeInterface = FLPROG_NETWORK_ACTIVE_WIZNET;
        }
        else if (_rmiiReady)
        {
            _activeInterface = FLPROG_NETWORK_ACTIVE_RMII;
        }

        return;
    }

    // AUTO reports the selected default route, while all enabled interfaces
    // continue to be serviced in parallel.
    _activeInterface = resolveRoute(FLPROG_NETWORK_ROUTE_AUTO);
}

void FLProgNetworkInterface::updateState()
{
    if (!_workMode)
    {
        _status = FLPROG_NETWORK_DISABLED_STATUS;
        return;
    }

    if (_manualStop)
    {
        _status = FLPROG_NETWORK_IDLE_STATUS;
        return;
    }

    if (isReady())
    {
        _status = FLPROG_NETWORK_READY_STATUS;
        _error = FLPROG_NETWORK_NOT_ERROR;
        return;
    }

    const bool connectionPending =
        _wifiStarted ||
        _apStarted ||
        _ethernetStarted ||
        _rmiiStarted ||
        _wifiStartRequest ||
        _apStartRequest ||
        _ethernetStartRequest ||
        _rmiiStartRequest;

    // RC6-A-dev.3-fix1: connect timeout only exists while there is a real
    // connection attempt in flight. An intentionally disabled transport
    // (for example A2 EN=0 -> clientMode(false)) must remain IDLE forever
    // and must not turn into STATE=5 / WIFI_TIMEOUT after 15 seconds.
    if (!connectionPending)
    {
        _status = FLPROG_NETWORK_IDLE_STATUS;
        return;
    }

    uint32_t now = millis();
    if ((uint32_t)(now - _connectionStartTime) >= _connectTimeout)
    {
        _status = FLPROG_NETWORK_ERROR_STATUS;
        if (_error == FLPROG_NETWORK_NOT_ERROR)
        {
            _error = FLPROG_NETWORK_WIFI_TIMEOUT_ERROR;
        }
        return;
    }

    _status = FLPROG_NETWORK_CONNECTING_STATUS;
}


// -----------------------------------------------------------------------------
// Слой привязки FLProg
// -----------------------------------------------------------------------------

// Читает текущие входы FLProg до сетевой обработки. Настройки применяются до EN,
// поэтому включение EN сразу использует самые свежие значения параметров.
void FLProgNetworkInterface::syncBlockInputs()
{
    if (!_blockIoBound)
    {
        return;
    }

    // Сначала применяем настройки. Если EN включается в этом же цикле FLProg,
// workMode(true) запускает подключение уже с текущими параметрами.
    if (_netReader != nullptr)
    {
        networkMode(_netReader(_netPtr));
    }

    if (_ssidReader != nullptr)
    {
        setClientSsid(_ssidReader(_ssidPtr));
    }

    if (_passwordReader != nullptr)
    {
        setClientPassword(_passwordReader(_passwordPtr));
    }

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (_ethCsReader != nullptr)
    {
        setPinCs(_ethCsReader(_ethCsPtr));
    }

    if (_spiBusReader != nullptr)
    {
        setSpiBus(_spiBusReader(_spiBusPtr));
    }

    if (_broadcastBlockingReader != nullptr)
    {
        setBroadcastBlocking(
            _broadcastBlockingReader(
                _broadcastBlockingPtr
            )
        );
    }
#endif

    if (_enReader != nullptr)
    {
        workMode(_enReader(_enPtr));
    }

    if (_conReader != nullptr)
    {
        connectInput(_conReader(_conPtr));
    }

    if (_disReader != nullptr)
    {
        disconnectInput(_disReader(_disPtr));
    }
}

// Записывает согласованный снимок состояния библиотеки во все привязанные выходы.
void FLProgNetworkInterface::syncBlockOutputs()
{
    if (!_blockIoBound)
    {
        return;
    }

    if (_connectedWriter != nullptr)
    {
        _connectedWriter(_connectedPtr, isReady());
    }

    if (_connectingWriter != nullptr)
    {
        _connectingWriter(_connectingPtr, isConnecting());
    }

    if (_errorWriter != nullptr)
    {
        _errorWriter(_errorOutPtr, hasError());
    }

    if (_wifiOkWriter != nullptr)
    {
        _wifiOkWriter(_wifiOkPtr, clientIsReady());
    }

    if (_ethernetOkWriter != nullptr)
    {
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _ethernetOkWriter(_ethernetOkPtr, false);
#else
        _ethernetOkWriter(_ethernetOkPtr, ethernetIsReady());
#endif
    }

    if (_rssiWriter != nullptr)
    {
        _rssiWriter(_rssiPtr, rssi());
    }

    if (_ipWriter != nullptr)
    {
        _ipWriter(_ipPtr, ipString());
    }

    if (_wifiIpWriter != nullptr)
    {
        _wifiIpWriter(_wifiIpPtr, wifiIPString());
    }

    if (_ethernetIpWriter != nullptr)
    {
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _ethernetIpWriter(_ethernetIpPtr, String("0.0.0.0"));
#else
        _ethernetIpWriter(_ethernetIpPtr, ethernetIPString());
#endif
    }

    if (_chipOutWriter != nullptr)
    {
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _chipOutWriter(_chipOutPtr, 0);
#else
        _chipOutWriter(_chipOutPtr, chipCode());
#endif
    }

    if (_socketsOutWriter != nullptr)
    {
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _socketsOutWriter(_socketsOutPtr, 0);
#else
        _socketsOutWriter(_socketsOutPtr, maxSocketNum());
#endif
    }

    if (_linkOutWriter != nullptr)
    {
#if FLPROG_NETWORK_PROFILE_WIFI_ONLY
        _linkOutWriter(_linkOutPtr, 0);
#else
        _linkOutWriter(_linkOutPtr, linkStatus());
#endif
    }

    if (_activeWriter != nullptr)
    {
        _activeWriter(_activePtr, activeInterface());
    }

    if (_stateWriter != nullptr)
    {
        _stateWriter(_statePtr, getStatus());
    }
}

// -----------------------------------------------------------------------------
// Главный циклический метод обслуживания
// -----------------------------------------------------------------------------

// Это единственный метод, который требуется вызывать в LoopSection FLProg.
uint8_t FLProgNetworkInterface::pool()
{
    syncBlockInputs();

    uint8_t result = FLPROG_NETWORK_SUCCESS;

    if (!_workMode)
    {
        _status = FLPROG_NETWORK_DISABLED_STATUS;
        updateSocketLayer();
        updateServiceLayer();
        syncBlockOutputs();
        return result;
    }

    uint32_t now = millis();

#if FLPROG_NETWORK_COMPILE_WIFI
    if (_wifiStartRequest && usesWifi() && !_manualStop)
    {
        startWifi();
    }

    if (_apStartRequest && _apAllowed && !_manualStop)
    {
        startAccessPoint();
    }
#endif

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    if (_ethernetStartRequest && usesWiznet() && !_manualStop)
    {
        startEthernet();
    }

    if (_rmiiStartRequest && usesRmii() && !_manualStop)
    {
        startRmii();
    }
#endif

#if FLPROG_NETWORK_COMPILE_WIFI
    updateWifi();
    updateWifiExtended();
#endif
#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
    updateEthernet();
    updateRmii();
    updateWiznetSocketPolicies();
#endif
    updateActiveInterface();
    updateTransportLayer();
    updateSocketLayer();
    updateServiceLayer();

    if (!_manualStop && _autoReconnect)
    {
#if FLPROG_NETWORK_COMPILE_WIFI
        if (usesWifi() && !_wifiReady && !_wifiStartRequest &&
            (uint32_t)(now - _lastWifiAttempt) >= _reconnectionPeriod)
        {
            _wifiStartRequest = true;
        }

        if (_apAllowed && !_apReady && !_apStartRequest && !_apConfigBlocked &&
            (uint32_t)(now - _lastApAttempt) >= _reconnectionPeriod)
        {
            _apStartRequest = true;
        }
#endif

#if !FLPROG_NETWORK_PROFILE_WIFI_ONLY
        const bool nativeDhcpOwnsWiznetLifecycle =
            _dhcp &&
            wiznetDhcpRef().active() &&
            !wiznetDhcpRef().failed();

        if (usesWiznet() && !_ethernetReady && !_ethernetStartRequest &&
            !nativeDhcpOwnsWiznetLifecycle &&
            (uint32_t)(now - _lastEthernetAttempt) >= _reconnectionPeriod)
        {
            _ethernetStartRequest = true;
        }

        if (usesRmii() && !_rmiiReady && !_rmiiStartRequest &&
            (uint32_t)(now - _lastRmiiAttempt) >= _reconnectionPeriod)
        {
            _rmiiStartRequest = true;
        }
#endif
    }

    updateState();

    if (_status == FLPROG_NETWORK_ERROR_STATUS)
    {
        result = FLPROG_NETWORK_ERROR;
    }
    else if (
        _status != FLPROG_NETWORK_READY_STATUS &&
        _status != FLPROG_NETWORK_IDLE_STATUS &&
        _status != FLPROG_NETWORK_DISABLED_STATUS)
    {
        result = FLPROG_NETWORK_WAIT;
    }

    syncBlockOutputs();
    return result;
}

int32_t FLProgNetworkInterface::rssi() const
{
#if FLPROG_NETWORK_HAS_WIFI
    if (_wifiReady)
    {
        return WiFi.RSSI();
    }
#endif
    return 0;
}

uint8_t FLProgNetworkInterface::hardwareStatus() const
{
    switch (wiznetRef().chipCode())
    {
        case FLPROG_WIZNET_CHIP_W5100:
            return 1;

        case FLPROG_WIZNET_CHIP_W5200:
            return 2;

        case FLPROG_WIZNET_CHIP_W5500:
            return 3;

        case FLPROG_WIZNET_CHIP_W5100S:
            return 4;

        default:
            break;
    }


    return 0;
}


uint8_t FLProgNetworkInterface::linkStatus() const
{
    if (wiznetRef().isInit())
    {
        return
            const_cast<FLProgWiznetHardware &>(wiznetRef())
                .getLinkStatus();
    }


    return FLPROG_WIZNET_LINK_UNKNOWN;
}
