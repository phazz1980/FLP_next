#pragma once
#include <flprogNetwork.h>
#include <string.h>

// RU: Один клиент на проект, сеть обслуживается существующим flpNetwork.pool().
// EN: One client per project; the existing network block owns network.pool().
class FLProgMqttUserBlocks;
inline FLProgMqttUserBlocks &flpMqttBlocks();

class FLProgMqttUserBlocks {
public:
    FLProgNetworkMqttClient mqtt;
    String topic, message;
    uint32_t sequence = 0;
    bool retained = false;

    void control(FLProgNetworkInterface &network, bool en, bool con, bool dis,
                 const String &broker, long port, const String &id,
                 const String &user, const String &password) {
        bool start = en && (!prevEn || (con && !prevCon));
        bool stop = (!en && prevEn) || (dis && !prevDis);
        prevEn = en; prevCon = con; prevDis = dis;
        if (stop) { mqtt.disconnect(); return; }
        if (!start) return;
        // Configuration changes require a fresh service session.
        if (bound) { mqtt.end(); bound = false; }
        mqtt.setBroker(broker.c_str());
        mqtt.setPort(port > 0 && port <= 65535 ? (uint16_t)port : 1883);
        if (!mqtt.setClientId(id.c_str())) return;
        if (user.length()) {
            if (!mqtt.setCredentials(user.c_str(), password.c_str())) return;
        } else mqtt.clearCredentials();
        mqtt.setMessageCallback(receive);
        mqtt.setAutoReconnect(true);
        bound = mqtt.begin(network);
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
    bool bound=false, prevEn=false, prevCon=false, prevDis=false;
    static void receive(const char *t, const uint8_t *p, size_t n, bool r) {
        FLProgMqttUserBlocks &b=flpMqttBlocks();
        b.topic=t; b.message="";
        for (size_t i=0;i<n;++i) b.message+=(char)p[i];
        b.retained=r; ++b.sequence;
    }
};
inline FLProgMqttUserBlocks &flpMqttBlocks() {
    static FLProgMqttUserBlocks value;
    return value;
}
