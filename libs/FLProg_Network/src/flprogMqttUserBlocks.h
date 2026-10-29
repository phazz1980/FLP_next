#pragma once
#include <flprogNetwork.h>
#include <string.h>
#include <device/flprogNetworkDeviceRegistry.h>

// RU: Один клиент на проект; сеть и network.pool() задаёт проект FLProg.
// EN: One client per project; the existing network block owns network.pool().
class FLProgMqttUserBlocks;
inline FLProgMqttUserBlocks &flpMqttBlocks();

class FLProgMqttUserBlocks {
public:
    FLProgNetworkMqttClient mqtt;
    String topic, message;
    uint32_t sequence = 0;
    bool retained = false;

    // v0.1 source compatibility.
    void control(FLProgNetworkInterface &network, bool en, bool con, bool dis,
                 const String &broker, long port, const String &id,
                 const String &user, const String &password) {
        control(&network, en, con, dis, broker, port, id, user, password, 0);
    }
    uint8_t error() const { return configError ? configError : mqtt.error(); }
    // 19: selected interface not found; 20: invalid SUB_LIMIT/port/POLL_MS.
    void control(FLProgNetworkInterface *network, bool en, bool con, bool dis,
                 const String &broker, long port, const String &id,
                 const String &user, const String &password, long limit, long pollMs = 0) {
        const bool start = en && (!prevEn || (con && !prevCon));
        const bool stop = (!en && prevEn) || (dis && !prevDis);
        prevEn = en; prevCon = con; prevDis = dis;
        if (stop) { mqtt.disconnect(); return; }
        if (!start) return;
        // A rejected configuration must not continue an older broker session.
        if (bound) { mqtt.end(); bound = false; }
        configError = 0;
        if (!network) { configError = 19; return; }
        if (limit < 0 || port < 1 || port > 65535 || pollMs < 0 || pollMs > 1000) { configError = 20; return; }
        if (!mqtt.setSubscriptionLimit((size_t)limit)) return;
        mqtt.setPollInterval((uint16_t)(pollMs == 0 ? FLPROG_MQTT_POLL_INTERVAL_MS : pollMs));
        mqtt.setBroker(broker.c_str());
        mqtt.setPort((uint16_t)port);
        if (!mqtt.setClientId(id.c_str())) return;
        if (user.length()) {
            if (!mqtt.setCredentials(user.c_str(), password.c_str())) return;
        } else mqtt.clearCredentials();
        mqtt.setMessageCallback(receive);
        mqtt.setAutoReconnect(true);
        bound = mqtt.begin(*network);
        if (bound) mqtt.connect();
    }

    // MQTT level matching, including empty levels and the $ topic boundary.
    static bool matches(const char *f, const char *t) {
        if (*t == '$' && (*f == '+' || *f == '#')) return false;
        for (;;) {
            const char *fe = strchr(f, '/');
            const char *te = strchr(t, '/');
            size_t fn = fe ? (size_t)(fe-f) : strlen(f);
            size_t tn = te ? (size_t)(te-t) : strlen(t);
            if (fn == 1 && *f == '#' && !fe) return true;
            if (!(fn == 1 && *f == '+') && (fn != tn || strncmp(f,t,fn))) return false;
            if (!fe && !te) return true;
            if (!te) return fe && strcmp(fe+1,"#") == 0;
            if (!fe) return false;
            f=fe+1; t=te+1;
        }
    }
private:
    uint8_t configError=0;
    bool bound=false, prevEn=false, prevCon=false, prevDis=false;
    static void receive(const char *t, const uint8_t *p, size_t n, bool r) {
        FLProgMqttUserBlocks &b=flpMqttBlocks();
        b.topic=t; b.message="";
        b.message.concat(reinterpret_cast<const char *>(p), (unsigned int)n);
        b.retained=r; ++b.sequence;
    }
};
inline FLProgMqttUserBlocks &flpMqttBlocks() {
    static FLProgMqttUserBlocks value;
    return value;
}
