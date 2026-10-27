/* FLProg user-block bridge implementation; see flprogNetworkMqttBlockBridge.h. */
#include "flprogNetworkMqttBlockBridge.h"
#include "../flprogNetwork.h"

bool FLProgNetworkMqttBlockBridge::update(uint8_t route)
{
    if (_bound)
    {
        return true;
    }

    // Generated projects do not expose stable C++ names for their Wi-Fi
    // instances. The compatibility registry resolves the first ready STA;
    // no connection attempt is made here before it is available.
    FLProgNetworkInterface *network = FLProgNetworkInterface::legacyDefaultWifiInterface();
    if (network == nullptr)
    {
        return false;
    }

    _bound = _client.begin(*network, route);
    return _bound;
}
