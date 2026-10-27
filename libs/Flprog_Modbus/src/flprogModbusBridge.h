#pragma once
#include "flprogModbusUtilites.h"
#include "flprogEthernet.h"
#include "flprogUartBase.h"

#define FLPROG_BRIDGE_WAITING_FOR_REQUEST 0
#define FLPROG_BRIDGE_WAITING_FOR_RESPONSE 1

class ModbusBridge : public AbstractFLProgClass
{
public:
  ModbusBridge(uint8_t portNumber, FLProgAbstractTcpInterface *sourse, FlprogAbstractUartExecutor *executor);
  void pool();

  void setUart(uint8_t portNumber) { _uart = portNumber; };
  void setSpeedUart(uint32_t speed) { _executor->setSpeedUart(speed, _uart); };
  void setDataBitUart(uint8_t value) { _executor->setDataBitUart(value, _uart); };
  void setStopBitUart(uint8_t value) { _executor->setStopBitUart(value, _uart); };
  void setParityUart(uint8_t value) { _executor->setParityUart(value, _uart); };
  void setPinRxUart(uint8_t pin) { _executor->setPinRxUart(pin, _uart); };
  void setPinTxUart(uint8_t pin) { _executor->setPinTxUart(pin, _uart); };

  void setTCPPort(int16_t port);
  int16_t tcpPort() { return _port; };

  void setTimeoutTime(uint32_t value) { _timeoutTime = value; };
  uint32_t getTimeoutTime() { return _timeoutTime; };

  void setTCPRemoteIp(IPAddress newIp);
  void setTCPRemoteIp(uint8_t newIpFirst, uint8_t newIpSecond, uint8_t newIpThird, uint8_t newIpFourtiph);
  IPAddress tcpRemoteIp() { return _ip; };

  void setTCPRemoteIpNumber(uint32_t newIp) { setTCPRemoteIp(flprog::numberToIp(newIp)); };
  void setTCPRemoteHost(String host);

  void setPinPeDe(uint8_t pin) { _pinPeDe = pin; };

  void setKaScadaCloudIp(IPAddress newIp);
  void setKaScadaCloudIpNumber(uint32_t newIp) { setKaScadaCloudIp(flprog::numberToIp(newIp)); };
  void setKaScadaCloudIp(uint8_t first_octet, uint8_t second_octet, uint8_t third_octet, uint8_t fourth_octet);
  IPAddress kaScadaCloudIp() { return _kasCadaCloudIP; };

  void setKaScadaCloudPort(int16_t port);
  int16_t kaScadaCloudPort() { return _kasCadaCloudPort; };

  void setKaScadaCloudDevceId(String id);
  String kaScadaCloudDevceId() { return _deniceId; };

  void setKaScadaCloudHost(String host);

  void byServer();
  void byClient();
  void setBridgeMode(bool mode);
  bool bridgeMode() { return _isServer; };

  void setMode(uint8_t mode);
  void byTcp() { setMode(FLPROG_TCP_MODBUS); };
  void byRtuOverTcp() { setMode(FLPROG_RTU_OVER_TCP_MODBUS); };
  void byKasCadaCloud() { setMode(FLPROG_KASCADA_CLOUD_MODBUS); };
  uint8_t mode() { return _mode; };

  uint16_t getSkippingEvents() { return _skippingEvents; };
  void setSkippingEvents(uint16_t value) { _skippingEvents = value; };
  void isPause(bool value) { _isPause = value; };
  bool isPause() { return _isPause; };
  void setEnable(bool enable) { _enable = enable; };
  bool getEnable() { return _enable; };

  uint32_t getPauseTime() { return _pauseTime; };
  void setPauseTime(uint32_t time) { _pauseTime = time; };

protected:
  void onPeDePin();
  void offPeDePin();
  void begin();
  void sendRTUBuffer();
  void connect();
  void requestProcess();
  void responseProcess();
  void serverRequestProcess();
  void serverTcpModeRequestProcess();
  void serverRtuOverTcpModeRequestProcess();
  void serverResponseProcess();
  void clientrRequestProcess();
  void clientResponseProcess();
  void clientTCPModeResponseProcess();
  void clientRtuOverTCPModeResponseProcess();
  void kascadaCloudRequestProcess();
  void kascadaCloudResponseProcess();

  FlprogAbstractUartExecutor *_executor = 0;

  uint16_t _skippingEvents = 0;
  uint16_t _eventsCount = 0;

  uint8_t _uart = 0;
  int16_t _port = 502;
  int16_t _kasCadaCloudPort = 25000;
  IPAddress _kasCadaCloudIP = IPAddress(94, 250, 249, 225);
  char _cloudHost[FLPROG_HOST_NAME_LENGTH] = "";
  bool _cloudAdressAsHost = false;

  bool _isServer = false;
  int16_t _pinPeDe = -1;
  uint8_t _bufferSize = 0;
  uint8_t _buffer[FLPROG_MODBUS_BUFER_SIZE];
  uint8_t _lastRec = 0;
  uint32_t _startT35;
  uint32_t _startSendTime;
  int16_t _timeOfSend;
  IPAddress _ip = FLPROG_INADDR_NONE;
  char _remoteServerHost[FLPROG_HOST_NAME_LENGTH] = "";
  bool _remoteServerAsHost = false;

  bool _isPause = false;

  bool _isInit = false;

  FLProgAbstractTcpInterface *_interface;
  FLProgEthernetServer _server;
  FLProgEthernetClient _tcpClient;
  uint8_t _mode = FLPROG_TCP_MODBUS;
  uint8_t _mbapBuffer[6];

  // TCP stream state — исправил ander.
  uint8_t _tcpRxMbapSize = 0;
  uint16_t _tcpRxExpectedSize = 0;
  uint16_t _tcpRxBodySize = 0;
  uint16_t _rtuOverTcpRxSize = 0;
  uint32_t _tcpRxStartTime = 0;

  uint8_t _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
  uint16_t _transactionId = 1;
  uint32_t _rtuSendTime;
  uint32_t _timeoutTime = 500;

  String _deniceId;
  uint32_t _kaScadaCloudTimeStartTime;
  bool _enable = true;
  uint32_t _startPauseTime;
  uint32_t _pauseTime = 10;
  bool _isPauseStatus = false;
};
