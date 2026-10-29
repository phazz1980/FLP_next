#pragma once
#include <flprogNetwork.h>
#include <flprogNTP.h>
#ifndef FLPROG_NTP_TIMEZONE_API
#error "Update FLProg_Network: NTP timezone API 1 is required"
#endif
// No network I/O here: the project's existing network.pool() services NTP.
class FLProgNtpUserBlock {
    FLProgNetworkInterface *network_=nullptr;
    String server_;
    bool enabled_=false,prevSync_=false,cached_=false;
    uint8_t localError_=0,mode_=255;
    int offset_=9999;
    uint32_t cachedUtc_=0,seen_=0;
public:
    FLProgNetworkNtp ntp;
    FLProgNtpLocalDateTime local={};
    bool synced=false;
    void control(FLProgNetworkInterface *network,bool en,bool sync,const String &server,long period,long timeout,long offset,long mode){
        bool edge=sync&&!prevSync_;prevSync_=sync;synced=false;enabled_=en;localError_=0;
        if(!en){if(network_){ntp.end();network_=nullptr;}cached_=false;return;}
        if(period==0)period=3600;
        if(timeout==0)timeout=15000;
        if(!network){localError_=100;ntp.setEnabled(false);return;}
        if(!server.length() || server.length()>FLPROG_DNS_MAX_HOSTNAME_LENGTH || period<1 || period>4294967 || timeout<100 || timeout>60000){localError_=101;ntp.setEnabled(false);return;}
        if(offset< -720 || offset>840 || mode<0 || mode>2){localError_=102;ntp.setEnabled(false);return;}
        if(offset_!=offset || mode_!=mode){
            if(!ntp.setTimeZone((int)offset,(uint8_t)mode)){localError_=102;ntp.setEnabled(false);return;}
            offset_=(int)offset;mode_=(uint8_t)mode;cached_=false;
        }
        if(network_!=network){
            if(network_)ntp.end();
            if(!ntp.begin(*network)){localError_=ntp.error();network_=nullptr;return;}
            network_=network;seen_=ntp.successCount();cached_=false;
        }
        if(server_!=server){server_=server;ntp.setServer(server.c_str());}
        if(ntp.requestPeriod()!=(uint32_t)period)ntp.setRequestPeriod((uint32_t)period);
        ntp.setTimeout((uint32_t)timeout);ntp.setEnabled(true);
        if(edge)ntp.syncNow();
        uint32_t count=ntp.successCount();synced=count!=seen_;seen_=count;
        if(ntp.hasTime() && (!cached_ || cachedUtc_!=ntp.unixTime())){local=ntp.localDateTime();cachedUtc_=ntp.unixTime();cached_=true;}
    }
    bool valid() const {return enabled_ && !localError_ && network_ && ntp.hasTime();}
    uint8_t error() const {return !enabled_?0:(localError_?localError_:ntp.error());}
};
