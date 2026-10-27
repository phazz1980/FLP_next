#include "flprogNetworkOtaStm32Wiznet.h"

#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
#include "../flprogNetwork.h"
#include "flprogStm32OtaFlash.h"
#include <ctype.h>
#include <string.h>

namespace
{
constexpr uint32_t kStageStart = FlprogOtaFlash::stage;
constexpr uint32_t kStageData = kStageStart + 0x100UL;
constexpr uint32_t kStageEnd = FlprogOtaFlash::end;
constexpr uint32_t kAppCapacity = FlprogOtaFlash::capacity;
constexpr uint16_t kBlockSize = 1024;
constexpr uint8_t kRequestMagic[8] = {'F','L','P','O','T','A','2',0};
constexpr uint8_t kStageMagic[8] = {'F','L','P','O','T','A','1',0};
constexpr uint8_t kReplyReady[4] = {'R','D','Y','1'};
constexpr uint8_t kReplyDone[4] = {'O','K','A','Y'};
constexpr uint8_t kReplyFail[4] = {'E','R','R','!'};
const IPAddress kMdnsGroup(224, 0, 0, 251);
constexpr uint16_t kMdnsPort = 5353;
constexpr uint16_t kMdnsAnswerPort = 3233;
constexpr char kMdnsService[] = "_arduino._tcp.local";
constexpr char kMdnsInstance[] = "flprog-fle4d._arduino._tcp.local";
constexpr char kMdnsHost[] = "flprog-fle4d.local";

bool mdnsPut8(uint8_t *packet, size_t capacity, size_t &length, uint8_t value)
{
    if (length >= capacity) return false;
    packet[length++] = value;
    return true;
}

bool mdnsPut16(uint8_t *packet, size_t capacity, size_t &length, uint16_t value)
{
    return mdnsPut8(packet, capacity, length, (uint8_t)(value >> 8)) &&
           mdnsPut8(packet, capacity, length, (uint8_t)value);
}

bool mdnsPut32(uint8_t *packet, size_t capacity, size_t &length, uint32_t value)
{
    return mdnsPut16(packet, capacity, length, (uint16_t)(value >> 16)) &&
           mdnsPut16(packet, capacity, length, (uint16_t)value);
}

bool mdnsPutName(uint8_t *packet, size_t capacity, size_t &length, const char *name)
{
    const char *label = name;
    while (*label != 0)
    {
        const char *end = label;
        while (*end != 0 && *end != '.') ++end;
        const size_t labelLength = (size_t)(end - label);
        if (labelLength == 0 || labelLength > 63 ||
            !mdnsPut8(packet, capacity, length, (uint8_t)labelLength) ||
            length + labelLength > capacity)
            return false;
        memcpy(packet + length, label, labelLength);
        length += labelLength;
        label = (*end == '.') ? end + 1 : end;
    }
    return mdnsPut8(packet, capacity, length, 0);
}

bool mdnsPutRecord(uint8_t *packet, size_t capacity, size_t &length,
                   const char *name, uint16_t type, uint16_t recordClass,
                   const uint8_t *data, uint16_t dataLength)
{
    return mdnsPutName(packet, capacity, length, name) &&
           mdnsPut16(packet, capacity, length, type) &&
           mdnsPut16(packet, capacity, length, recordClass) &&
           mdnsPut32(packet, capacity, length, 120) &&
           mdnsPut16(packet, capacity, length, dataLength) &&
           (length + dataLength <= capacity) &&
           (memcpy(packet + length, data, dataLength), length += dataLength, true);
}

bool mdnsPutPtrRecord(uint8_t *packet, size_t capacity, size_t &length)
{
    uint8_t data[96];
    size_t dataLength = 0;
    if (!mdnsPutName(data, sizeof(data), dataLength, kMdnsInstance)) return false;
    return mdnsPutRecord(packet, capacity, length, kMdnsService, 12, 1,
                         data, (uint16_t)dataLength);
}

bool mdnsPutSrvRecord(uint8_t *packet, size_t capacity, size_t &length)
{
    uint8_t data[96];
    size_t dataLength = 0;
    if (!mdnsPut16(data, sizeof(data), dataLength, 0) ||
        !mdnsPut16(data, sizeof(data), dataLength, 0) ||
        !mdnsPut16(data, sizeof(data), dataLength, kMdnsAnswerPort) ||
        !mdnsPutName(data, sizeof(data), dataLength, kMdnsHost)) return false;
    return mdnsPutRecord(packet, capacity, length, kMdnsInstance, 33, 0x8001,
                         data, (uint16_t)dataLength);
}

bool mdnsPutTxtRecord(uint8_t *packet, size_t capacity, size_t &length)
{
    static const char *const fields[] = {
        "board=FLProg_FLE4D_30MT_OTA",
        "auth_upload=yes",
        "tcp_check=no",
        "ssh_upload=no"
    };
    uint8_t data[128];
    size_t dataLength = 0;
    for (size_t i = 0; i < sizeof(fields) / sizeof(fields[0]); ++i)
    {
        const size_t fieldLength = strlen(fields[i]);
        if (fieldLength > 255 || !mdnsPut8(data, sizeof(data), dataLength,
                                           (uint8_t)fieldLength) ||
            dataLength + fieldLength > sizeof(data)) return false;
        memcpy(data + dataLength, fields[i], fieldLength);
        dataLength += fieldLength;
    }
    return mdnsPutRecord(packet, capacity, length, kMdnsInstance, 16, 0x8001,
                         data, (uint16_t)dataLength);
}

bool mdnsPutAddressRecord(uint8_t *packet, size_t capacity, size_t &length,
                          IPAddress address)
{
    const uint8_t data[4] = {address[0], address[1], address[2], address[3]};
    return mdnsPutRecord(packet, capacity, length, kMdnsHost, 1, 0x8001,
                         data, sizeof(data));
}

bool mdnsQueryForArduino(const uint8_t *packet, size_t length)
{
    // The Arduino IDE asks DNS-SD for _arduino._tcp. Matching the label in
    // the encoded question is enough; the response below includes the full
    // PTR/SRV/TXT/A record set for this one advertised device.
    static const char needle[] = "_arduino";
    // Do not answer other devices' responses (multicast feedback loop).
    if (length < 12 || (packet[2] & 0x80U)) return false;
    for (size_t i = 12; i + sizeof(needle) - 1 <= length; ++i)
    {
        size_t j = 0;
        while (j < sizeof(needle) - 1 &&
               (uint8_t)tolower(packet[i + j]) == (uint8_t)needle[j]) ++j;
        if (j == sizeof(needle) - 1) return true;
    }
    return false;
}

uint32_t readLe32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

void writeLe32(uint8_t *p, uint32_t value)
{
    p[0]=(uint8_t)value; p[1]=(uint8_t)(value>>8);
    p[2]=(uint8_t)(value>>16); p[3]=(uint8_t)(value>>24);
}

uint32_t crc32Update(uint32_t crc, const uint8_t *data, size_t length)
{
    for (size_t i=0;i<length;++i)
    {
        crc ^= data[i];
        for (uint8_t bit=0;bit<8;++bit)
            crc = (crc >> 1) ^ ((crc & 1U) ? 0xEDB88320UL : 0U);
    }
    return crc;
}

bool constantTimeEqual(const uint8_t *a, const uint8_t *b, size_t n)
{
    uint8_t difference=0;
    for (size_t i=0;i<n;++i) difference |= (uint8_t)(a[i]^b[i]);
    return difference==0;
}

bool flashProgramWord(uint32_t address, uint32_t value)
{
    return FlprogOtaFlash::programWord(address, value);
}
}
#endif

FLProgNetworkOtaStm32Wiznet::FLProgNetworkOtaStm32Wiznet()
    : _client(nullptr), _wordBytes(0), _requestBytes(0), _bufferBytes(0),
      _expectedSize(0), _expectedCrc(0), _runningCrc(0xFFFFFFFFUL),
      _received(0), _stageAddress(0), _lastActivity(0), _restartPending(false), _restartScheduledAt(0), _startCount(0),
      _endCount(0), _errorCount(0), _blockExpected(0), _blockReceived(0), _macBytes(0),
      _state(Idle), _error(0), _ready(false), _updating(false), _mdnsStarted(false),
      _mdnsAnnouncementPending(false), _lastMdnsAnnounceAt(0),
      _mdnsPacketCount(0), _mdnsQueryCount(0), _mdnsResponseCount(0)
{
    memset(_model,0,sizeof(_model));
    memset(_version,0,sizeof(_version));
    _password[0]=0;
    memset(_request,0,sizeof(_request));
    memset(_expectedMac,0,sizeof(_expectedMac));
}

void FLProgNetworkOtaStm32Wiznet::setIdentity(const char *model, const char *version)
{
    memset(_model,0,sizeof(_model));
    memset(_version,0,sizeof(_version));
    if (model) strncpy(_model,model,sizeof(_model)-1);
    if (version) strncpy(_version,version,sizeof(_version)-1);
}

FLProgNetworkOtaStm32Wiznet::~FLProgNetworkOtaStm32Wiznet()
{
    end();
}

bool FLProgNetworkOtaStm32Wiznet::ready() const
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    const FLProgNetworkInterface *network=serviceNetwork();
    return _ready && network!=nullptr && network->wiznetIsReady() && _server.listening();
#else
    return false;
#endif
}

bool FLProgNetworkOtaStm32Wiznet::begin(
    FLProgNetworkInterface &network, const char *password, uint16_t port)
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (password==nullptr || strlen(password)>=sizeof(_password) || port==0)
    {
        _error=1;
        return false;
    }
    end();
    if (!FlprogOtaFlash::hardwareMatches() || SCB->VTOR != FlprogOtaFlash::app)
    {
        _error=14;
        return false;
    }
    strcpy(_password,password);
    if (!attach(network) || !_server.beginChild(network,port,FLPROG_NETWORK_ROUTE_WIZNET,1))
    {
        end();
        _error=2;
        return false;
    }
    // mDNS is best-effort: OTA remains available at the known IP even if a
    // particular network filters multicast. Sharing the network manager keeps
    // this second socket on the same W5500 SPI owner as the OTA TCP listener.
    _mdnsStarted = _mdns.beginMulticast(
        network, FLPROG_NETWORK_ROUTE_WIZNET, kMdnsGroup, kMdnsPort);
    _mdnsAnnouncementPending = _mdnsStarted;
    _lastMdnsAnnounceAt = 0;
    _ready=true;
    _error=0;
    return true;
#else
    (void)network; (void)password; (void)port;
    _error=3;
    return false;
#endif
}

void FLProgNetworkOtaStm32Wiznet::end()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (_client!=nullptr) _client->stop();
    _server.end();
    _mdns.stop();
#endif
    detach();
    _client=nullptr;
    _ready=false;
    _restartPending=false;
    _updating=false;
    _mdnsStarted=false;
    _mdnsAnnouncementPending=false;
    _lastMdnsAnnounceAt=0;
    _state=Idle;
    memset(_password,0,sizeof(_password));
}

void FLProgNetworkOtaStm32Wiznet::pool()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    // Finish the success reply before resetting; reject further transfers meanwhile.
    if (_restartPending)
    {
        if ((uint32_t)(millis() - _restartScheduledAt) >= 1000UL)
            NVIC_SystemReset();
        return;
    }
    if (!_ready) return;
    _server.pool();
    serviceMdns();
    if (_client==nullptr)
    {
        _client=_server.takeNewClient();
        if (_client!=nullptr)
        {
            _state=Header;
            _requestBytes=0;
            _lastActivity=millis();
        }
    }
    if (_client==nullptr) return;
    if (!_client->active() || !_client->connected())
    {
        resetTransfer(false);
        return;
    }
    if ((uint32_t)(millis()-_lastActivity)>15000UL)
    {
        fail(4);
        return;
    }
    serviceClient();
#endif
}

void FLProgNetworkOtaStm32Wiznet::serviceMdns()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (!_mdnsStarted) return;
    const int packetSize = _mdns.parsePacket();
    uint8_t query[512];
    size_t received = 0;
    if (packetSize <= 0)
    {
        const uint32_t now = millis();
        const bool periodic = !_mdnsAnnouncementPending &&
                              (uint32_t)(now - _lastMdnsAnnounceAt) >= 60000UL;
        if (!_mdnsAnnouncementPending && !periodic) return;
        // Avoid hammering the shared W5500 SPI bus while its UDP socket is
        // still opening or another network owner is using the bus.
        if ((uint32_t)(now - _lastMdnsAnnounceAt) < 1000UL) return;
        _lastMdnsAnnounceAt = now;
    }
    else
    {
        ++_mdnsPacketCount;
        received = (size_t)_mdns.read(
            query, (size_t)packetSize < sizeof(query) ? (size_t)packetSize : sizeof(query));
        if (received < 12 || !mdnsQueryForArduino(query, received))
        {
            while (_mdns.available() > 0) _mdns.read();
            return;
        }
        ++_mdnsQueryCount;
    }

    FLProgNetworkInterface *network = serviceNetwork();
    if (network == nullptr || !network->wiznetIsReady()) return;

    uint8_t response[512] = {};
    // mDNS uses transaction ID zero, even if a sender supplies another value.
    // More importantly, answer browse queries on the multicast group: the
    // W5500 socket is opened in MULTI mode and cannot send a unicast reply.
    response[0] = 0;
    response[1] = 0;
    response[2] = 0x84; // authoritative response / unsolicited announcement
    response[3] = 0x00;
    response[6] = 0;
    response[7] = 4;    // PTR, SRV, TXT, A
    size_t responseLength = 12;
    const bool encoded = mdnsPutPtrRecord(response, sizeof(response), responseLength) &&
                         mdnsPutSrvRecord(response, sizeof(response), responseLength) &&
                         mdnsPutTxtRecord(response, sizeof(response), responseLength) &&
                         mdnsPutAddressRecord(response, sizeof(response), responseLength,
                                              network->wiznetIP());
    if (!encoded) return;

    // DNS-SD browse packets can arrive from an ephemeral source port (QU
    // questions). This multicast listener's UDP backend intentionally locks
    // outbound packets to its configured group, so trying to answer directly
    // to remoteIP:remotePort fails silently. A multicast answer is valid for
    // the shared mDNS browse query and is consumed by Arduino IDE/JmDNS.
    if (_mdns.beginPacket(kMdnsGroup, kMdnsPort) == 0) return;
    if (_mdns.write(response, responseLength) != responseLength)
    {
        _mdns.endPacket();
        return;
    }
    if (_mdns.endPacket() != 0)
    {
        ++_mdnsResponseCount;
        _lastMdnsAnnounceAt = millis();
        _mdnsAnnouncementPending = false;
    }
#endif
}

void FLProgNetworkOtaStm32Wiznet::resetTransfer(bool closeClient)
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (closeClient && _client!=nullptr) _client->stop();
#else
    (void)closeClient;
#endif
    _client=nullptr;
    _state=Idle;
    _updating=false;
    _requestBytes=0;
    _bufferBytes=0;
    _wordBytes=0;
    _blockExpected=0;
    _blockReceived=0;
    _macBytes=0;
    _received=0;
}

void FLProgNetworkOtaStm32Wiznet::fail(uint8_t errorCode)
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (_client!=nullptr)
    {
        _client->write(kReplyFail,sizeof(kReplyFail));
        _client->stop();
    }
    HAL_FLASH_Lock();
#endif
    _error=errorCode;
    ++_errorCount;
    resetTransfer(false);
}

void FLProgNetworkOtaStm32Wiznet::serviceClient()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (_state==Discovery)
    {
        if (_bufferBytes<108)
        {
            const size_t sent=_client->write(_buffer+_bufferBytes,108-_bufferBytes);
            _bufferBytes+=(uint16_t)sent;
            if (sent) _lastActivity=millis();
            if ((uint32_t)(millis()-_lastActivity)>2000UL) resetTransfer(true);
        }
        else if ((uint32_t)(millis()-_lastActivity)>=500UL) resetTransfer(true);
        return;
    }
    if (_state==Header)
    {
        while (_client->available()>0 && _requestBytes<sizeof(_request))
        {
            const int value=_client->read();
            if (value<0) break;
            _request[_requestBytes++]=(uint8_t)value;
            _lastActivity=millis();
            // Read-only discovery: never authenticate, erase or start a transfer.
            if (_requestBytes==8 && memcmp(_request,"FLPINFO?",8)==0)
            {
                uint8_t info[108]={'F','L','P','O','T','A','2',0};
                writeLe32(info+8,FLPROG_STM32_OTA_LAYOUT);
                writeLe32(info+12,FlprogOtaFlash::flashKiB*1024UL);
                writeLe32(info+16,FlprogOtaFlash::app);
                writeLe32(info+20,FlprogOtaFlash::capacity);
                memcpy(info+24,"ID01",4);
                memcpy(info+28,_model,sizeof(_model));
                memcpy(info+76,_version,sizeof(_version));
                memcpy(_buffer,info,sizeof(info));
                _bufferBytes=0;
                _state=Discovery;
                return;
            }
        }
        if (_requestBytes<sizeof(_request)) return;
        if (memcmp(_request,kRequestMagic,sizeof(kRequestMagic))!=0)
        {
            fail(5); return;
        }
        _expectedSize=readLe32(_request+8);
        _expectedCrc=readLe32(_request+12);
        if (_expectedSize<8 || _expectedSize>kAppCapacity ||
            kStageData+_expectedSize>kStageEnd)
        {
            fail(6); return;
        }
        FLProgOtaHmacSha256 headerMac;
        headerMac.begin((const uint8_t *)_password,strlen(_password));
        headerMac.update(_request,16);
        uint8_t expectedHeaderMac[32];
        headerMac.finish(expectedHeaderMac);
        if (!constantTimeEqual(expectedHeaderMac,_request+16,32))
        {
            fail(7); return;
        }
        memcpy(_expectedMac,_request+16,32);
        _hmac.begin((const uint8_t *)_password,strlen(_password));
        _hmac.update(_request,16);
        _runningCrc=0xFFFFFFFFUL;
        _received=0;
        _stageAddress=kStageData;
        _wordBytes=0;
        _blockExpected=0;
        _blockReceived=0;
        _updating=true;
        ++_startCount;
        _state=Erasing;
        if (!eraseStage()) { fail(8); return; }
        _state=Receiving;
        _lastActivity=millis();
        _client->write(kReplyReady,sizeof(kReplyReady));
        return;
    }

    if (_state==Receiving)
    {
        if (_blockExpected==0)
        {
            if (_client->available()<2) return;
            const int lo=_client->read(); const int hi=_client->read();
            if (lo<0 || hi<0) return;
            _blockExpected=(uint16_t)((uint16_t)lo|((uint16_t)hi<<8));
            const uint32_t remaining=_expectedSize-_received;
            const uint16_t required=(uint16_t)(remaining>kBlockSize?kBlockSize:remaining);
            if (_blockExpected==0 || _blockExpected!=required) { fail(9); return; }
            _blockReceived=0;
            _lastActivity=millis();
        }
        const int available=_client->available();
        if (available<=0) return;
        uint16_t count=(uint16_t)available;
        if (count>_blockExpected-_blockReceived) count=(uint16_t)(_blockExpected-_blockReceived);
        const int got=_client->read(_buffer+_blockReceived,count);
        if (got<=0) return;
        _blockReceived+=(uint16_t)got;
        _lastActivity=millis();
        if (_blockReceived<_blockExpected) return;
        if (!writeStage(_buffer,_blockExpected)) { fail(10); return; }
        _hmac.update(_buffer,_blockExpected);
        _runningCrc=crc32Update(_runningCrc,_buffer,_blockExpected);
        _received+=_blockExpected;
        _blockExpected=0;
        if (!sendAck(_received)) { fail(11); return; }
        if (_received==_expectedSize) _state=Commit;
        return;
    }

    if (_state==Commit)
    {
        while (_client->available()>0 && _macBytes<32)
        {
            const int value=_client->read();
            if (value<0) break;
            _request[_macBytes++]=(uint8_t)value;
            _lastActivity=millis();
        }
        if (_macBytes<32) return;
        uint8_t calculatedMac[32];
        _hmac.finish(calculatedMac);
        if ((~_runningCrc)!=_expectedCrc || !constantTimeEqual(calculatedMac,_request,32))
        {
            fail(12); return;
        }
        if (!commitStage()) { fail(13); return; }
        _client->write(kReplyDone,sizeof(kReplyDone));
        ++_endCount;
        _error=0;
        resetTransfer(true);
        // Only a verified, committed staging image may trigger a reset.
        _restartScheduledAt=millis();
        _restartPending=true;
    }
#endif
}

bool FLProgNetworkOtaStm32Wiznet::eraseStage()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (HAL_FLASH_Unlock()!=HAL_OK) return false;
    const bool ok=FlprogOtaFlash::erase(true);
    HAL_FLASH_Lock();
    return ok;
#else
    return false;
#endif
}

bool FLProgNetworkOtaStm32Wiznet::writeStage(const uint8_t *data,size_t length)
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (HAL_FLASH_Unlock()!=HAL_OK) return false;
    bool ok=true;
    for (size_t i=0;i<length && ok;++i)
    {
        _wordBuffer[_wordBytes++]=data[i];
        if (_wordBytes==4)
        {
            uint32_t word; memcpy(&word,_wordBuffer,sizeof(word));
            ok=flashProgramWord(_stageAddress,word);
            _stageAddress+=4;
            _wordBytes=0;
        }
    }
    if (_received+length==_expectedSize && _wordBytes!=0 && ok)
    {
        while (_wordBytes<4) _wordBuffer[_wordBytes++]=0xFF;
        uint32_t word; memcpy(&word,_wordBuffer,sizeof(word));
        ok=flashProgramWord(_stageAddress,word);
        _stageAddress+=4;
        _wordBytes=0;
    }
    HAL_FLASH_Lock();
    return ok && _stageAddress<=kStageEnd;
#else
    (void)data; (void)length; return false;
#endif
}

bool FLProgNetworkOtaStm32Wiznet::commitStage()
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    if (!FlprogOtaFlash::vectorValid(kStageData, _expectedSize)) return false;
    // Check the programmed staging bytes, not only the network input buffer.
    // Never publish a valid marker for data that the bootloader would reject.
    if ((~crc32Update(0xFFFFFFFFUL,
            reinterpret_cast<const uint8_t *>(kStageData),_expectedSize))!=_expectedCrc)
        return false;
    if (HAL_FLASH_Unlock()!=HAL_OK) return false;
    bool ok=true;
    ok=flashProgramWord(kStageStart+8,_expectedSize) &&
       flashProgramWord(kStageStart+12,_expectedCrc) &&
       flashProgramWord(kStageStart+16,FLPROG_STM32_OTA_LAYOUT);
    // Write the validity marker last: the bootloader ignores an interrupted header.
    if (ok)
    {
        uint32_t first,second;
        memcpy(&first,kStageMagic,4); memcpy(&second,kStageMagic+4,4);
        ok=flashProgramWord(kStageStart+4,second) && flashProgramWord(kStageStart,first);
    }
    HAL_FLASH_Lock();
    return ok;
#else
    return false;
#endif
}

bool FLProgNetworkOtaStm32Wiznet::sendAck(uint32_t received)
{
#if FLPROG_NETWORK_COMPILE_STM32_W5500_OTA
    uint8_t ack[4]; writeLe32(ack,received);
    return _client!=nullptr && _client->write(ack,sizeof(ack))==sizeof(ack);
#else
    (void)received; return false;
#endif
}

