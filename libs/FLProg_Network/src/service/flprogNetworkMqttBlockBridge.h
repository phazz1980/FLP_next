/*
 * FLProg_Network — flprogNetworkMqttBlockBridge.h
 *
 * Russian: вспомогательный владелец MQTT-клиента для пользовательских блоков
 * FLProg. Он не создаёт второй transport и не вызывает network.pool(): оба
 * действия остаются ответственностью штатного сетевого блока. Bridge только
 * ожидает готовый Wi-Fi STA и затем один раз связывает MQTT service с ним.
 *
 * English: helper MQTT-client owner for FLProg user blocks. It neither creates
 * a second transport nor calls network.pool(); those remain owned by the
 * normal network block. The bridge waits for a ready Wi-Fi STA and binds the
 * MQTT service to it once.
 */
#pragma once

#include "flprogNetworkMqttClient.h"

class FLProgNetworkMqttBlockBridge
{
public:
    // Safe on every generated loop pass. Returns true once the MQTT service
    // is attached to a ready FLProg Wi-Fi interface.
    bool update(uint8_t route = 0);

    bool ready() const { return _bound; }
    FLProgNetworkMqttClient &client() { return _client; }
    const FLProgNetworkMqttClient &client() const { return _client; }

private:
    FLProgNetworkMqttClient _client;
    bool _bound = false;
};
