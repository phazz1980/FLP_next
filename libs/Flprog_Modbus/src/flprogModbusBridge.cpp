#include "flprogModbusBridge.h"

ModbusBridge::ModbusBridge(uint8_t portNumber, FLProgAbstractTcpInterface *sourse, FlprogAbstractUartExecutor *executor)
{
  _interface = sourse;
  _uart = portNumber;
  _executor = executor;
  _status = FLPROG_MODBUS_READY;
  _server.setSourse(sourse);
  _tcpClient.setSourse(sourse);
}

void ModbusBridge::setMode(uint8_t mode)
{
  if (mode > 2)
  {
    return;
  }
  if (_mode == mode)
  {
    return;
  }
  _mode = mode;
  _isInit = false;
}

void ModbusBridge::setTCPPort(int16_t port)
{
  if (_port == port)
  {
    return;
  }
  _port = port;
  _isInit = false;
}

void ModbusBridge::setTCPRemoteIp(uint8_t newIpFirst, uint8_t newIpSecond, uint8_t newIpThird, uint8_t newIpFourth)
{
  setTCPRemoteIp(IPAddress(newIpFirst, newIpSecond, newIpThird, newIpFourth));
}

void ModbusBridge::setTCPRemoteIp(IPAddress newIp)
{
  _remoteServerAsHost = false;
  if (newIp == _ip)
  {
    return;
  }
  _ip = newIp;
  _isInit = false;
}

void ModbusBridge::setTCPRemoteHost(String host)
{
  if (host.length() == 0)
  {
    return;
  }
  _remoteServerAsHost = true;
  if (host.equals(String(_remoteServerHost)))
  {
    return;
  }
  host.toCharArray(_remoteServerHost, FLPROG_HOST_NAME_LENGTH);
  _isInit = false;
}

void ModbusBridge::byServer()
{
  if (_isServer)
  {
    return;
  }
  _isServer = true;
  _isInit = false;
}

void ModbusBridge::byClient()
{
  if (!_isServer)
  {
    return;
  }
  _isServer = false;
  _isInit = false;
}

void ModbusBridge::setBridgeMode(bool mode)
{
  if (mode)
  {
    byServer();
  }
  else
  {
    byClient();
  }
}

void ModbusBridge::setKaScadaCloudIp(IPAddress newIp)
{
  _cloudAdressAsHost = false;
  if (newIp == _kasCadaCloudIP)
  {
    return;
  }
  _kasCadaCloudIP = newIp;
  _isInit = false;
}

void ModbusBridge::setKaScadaCloudHost(String host)
{
  if (host.length() == 0)
  {
    return;
  }
  _cloudAdressAsHost = true;
  if (host.equals(String(_cloudHost)))
  {
    return;
  }
  host.toCharArray(_cloudHost, FLPROG_HOST_NAME_LENGTH);
  _isInit = false;
}

void ModbusBridge::setKaScadaCloudPort(int16_t port)
{
  if (port == _kasCadaCloudPort)
  {
    return;
  }
  _kasCadaCloudPort = port;
  _isInit = false;
}

void ModbusBridge::setKaScadaCloudDevceId(String id)
{
  _deniceId = id;
}

void ModbusBridge::setKaScadaCloudIp(uint8_t first_octet, uint8_t second_octet, uint8_t third_octet, uint8_t fourth_octet)
{
  setKaScadaCloudIp(IPAddress(first_octet, second_octet, third_octet, fourth_octet));
}

void ModbusBridge::onPeDePin()
{
  if (_pinPeDe < 0)
  {
    return;
  }
  digitalWrite(_pinPeDe, HIGH);
}

void ModbusBridge::offPeDePin()
{
  if (_pinPeDe < 0)
  {
    return;
  }
  digitalWrite(_pinPeDe, LOW);
}

void ModbusBridge::connect()
{
  if (_tcpClient.connected())
  {

    _status = FLPROG_MODBUS_READY;
    return;
  }
  uint8_t result;
  if (_mode == FLPROG_KASCADA_CLOUD_MODBUS)
  {
    if (_cloudAdressAsHost)
    {
      result = _tcpClient.connect(_cloudHost, _kasCadaCloudPort);
    }
    else
    {
      result = _tcpClient.connect(_kasCadaCloudIP, _kasCadaCloudPort);
    }
  }
  else
  {
    if (_remoteServerAsHost)
    {
      result = _tcpClient.connect(_remoteServerHost, _port);
    }
    else
    {
      result = _tcpClient.connect(_ip, _port);
    }
  }
  if (result == FLPROG_WAIT)
  {
    _status = FLPROG_MODBUS_WAITING_CONNECT_CLIENT;
    return;
  }
  _status = FLPROG_MODBUS_READY;
}

void ModbusBridge::pool()
{
  setFlags();
  if (!_enable)
  {
    return;
  }
  if (_interface == 0)
  {
    return;
  }
  if (_eventsCount < _skippingEvents)
  {
    _eventsCount++;
    return;
  }
  _eventsCount = 0;
  if (_isPause)
  {
    return;
  }
  if (!_interface->isReady())
  {
    return;
  }
  if (!_isInit)
  {
    begin();
  }
  if (_status == FLPROG_MODBUS_WAITING_CONNECT_CLIENT)
  {
    connect();
    return;
  }
  if ((!_isServer) || (_mode == FLPROG_KASCADA_CLOUD_MODBUS))
  {
    if (!_tcpClient.connected())
    {
      connect();
      return;
    }
  }
  if (_mode == FLPROG_KASCADA_CLOUD_MODBUS)
  {
    if (flprog::isTimer(_kaScadaCloudTimeStartTime, 5000))
    {

      _tcpClient.print(_deniceId);
      _kaScadaCloudTimeStartTime = millis();
    }
  }
  if (_status == FLPROG_MODBUS_WAITING_SENDING)
  {
    if ((flprog::isTimer(_startSendTime, _timeOfSend)))
    {
      _status = FLPROG_MODBUS_READY;
      offPeDePin();
      return;
    }
    else
    {
      return;
    }
  }
  if (_currentStep == FLPROG_BRIDGE_WAITING_FOR_REQUEST)
  {
    requestProcess();
    return;
  }
  responseProcess();
}

void ModbusBridge::requestProcess()
{
  if (_mode == FLPROG_KASCADA_CLOUD_MODBUS)
  {
    kascadaCloudRequestProcess();
    return;
  }
  if (_isServer)
  {
    serverRequestProcess();
    return;
  }
  clientrRequestProcess();
}

void ModbusBridge::responseProcess()
{
  if (_mode == FLPROG_KASCADA_CLOUD_MODBUS)
  {
    kascadaCloudResponseProcess();
    return;
  }
  if (_isServer)
  {
    serverResponseProcess();
    return;
  }
  clientResponseProcess();
}

void ModbusBridge::serverRequestProcess()
{
  if (!_server.connected())
  {
    return;
  }
  if (_mode == FLPROG_RTU_OVER_TCP_MODBUS)
  {
    serverRtuOverTcpModeRequestProcess();
    return;
  }
  serverTcpModeRequestProcess();
}

void ModbusBridge::clientrRequestProcess()
{
  while (_executor->availableUart(_uart))
  {
    _rtuSendTime = millis();
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
    {
      _buffer[_bufferSize++] = _executor->readUart(_uart);
    }
    else
    {
      _executor->readUart(_uart);
    }
  }
  if (_bufferSize == 0)
  {
    return;
  }
  uint16_t pacadgeSize = flprogModus::slaveRTUPacadgeSize(_bufferSize, _buffer);
  if ((pacadgeSize == 0) || (_bufferSize < pacadgeSize))
  {
    if ((_bufferSize > 0) && flprog::isTimer(_rtuSendTime, 50))
    {
      _bufferSize = 0;
    }
    return;
  }
  if (!flprogModus::checkCRCOnBuffer(pacadgeSize, _buffer))
  {
    return;
  }
  if (!_tcpClient.connected())
  {
    _bufferSize = 0;
    return;
  }
  while (_tcpClient.available())
  {
    _tcpClient.read();
  }
  if (_mode == FLPROG_RTU_OVER_TCP_MODBUS)
  {
    _tcpClient.write(_buffer, pacadgeSize);
  }
  else
  {
    _bufferSize = pacadgeSize - 2;
    _transactionId++;
    _mbapBuffer[0] = highByte(_transactionId);
    _mbapBuffer[1] = lowByte(_transactionId);
    _mbapBuffer[2] = 0;
    _mbapBuffer[3] = 0;
    _mbapBuffer[4] = highByte(_bufferSize);
    _mbapBuffer[5] = lowByte(_bufferSize);

    // БЫЛО — исправил ander: write(MBAP), затем write(PDU).
    // СТАЛО — исправил ander: полный Modbus TCP ADU одним write().
    uint8_t tcpFrame[FLPROG_MODBUS_BUFER_SIZE + 6];
    for (uint8_t i = 0; i < 6; i++) tcpFrame[i] = _mbapBuffer[i];
    for (uint16_t i = 0; i < _bufferSize; i++) tcpFrame[i + 6] = _buffer[i];
    _tcpClient.write(tcpFrame, ((uint16_t)_bufferSize) + 6);
  }
  _rtuSendTime = millis();
  _bufferSize = 0;
  _tcpRxMbapSize = 0;
  _tcpRxExpectedSize = 0;
  _tcpRxBodySize = 0;
  _rtuOverTcpRxSize = 0;
  _currentStep = FLPROG_BRIDGE_WAITING_FOR_RESPONSE;
}

void ModbusBridge::kascadaCloudRequestProcess()
{
  if (!_tcpClient.connected()) return;
  if (((_tcpRxMbapSize > 0) || (_tcpRxBodySize > 0)) &&
      flprog::isTimer(_tcpRxStartTime, _timeoutTime))
  {
    _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
  }
  _bufferSize = 0;
  while (_tcpClient.available())
  {
    uint8_t value = _tcpClient.read();
    if (_tcpRxMbapSize < 6)
    {
      if ((_tcpRxMbapSize == 0) && (_tcpRxBodySize == 0)) _tcpRxStartTime = millis();
      _mbapBuffer[_tcpRxMbapSize++] = value;
      if (_tcpRxMbapSize == 6)
      {
        _tcpRxExpectedSize = (uint16_t)word(_mbapBuffer[4], _mbapBuffer[5]);
        if ((_tcpRxExpectedSize == 0) || (_tcpRxExpectedSize > FLPROG_MODBUS_BUFER_SIZE))
        {
          _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0; return;
        }
      }
      continue;
    }
    if (_tcpRxBodySize < _tcpRxExpectedSize) _buffer[_tcpRxBodySize++] = value;
    if ((_tcpRxMbapSize == 6) && (_tcpRxBodySize >= _tcpRxExpectedSize)) break;
  }
  if ((_tcpRxMbapSize < 6) || (_tcpRxExpectedSize == 0) || (_tcpRxBodySize < _tcpRxExpectedSize)) return;
  _bufferSize = (uint8_t)_tcpRxExpectedSize;
  _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
  while (_executor->availableUart(_uart)) _executor->readUart(_uart);
  sendRTUBuffer();
  _rtuSendTime = millis();
  _currentStep = FLPROG_BRIDGE_WAITING_FOR_RESPONSE;
}

void ModbusBridge::serverTcpModeRequestProcess()
{
  if (((_tcpRxMbapSize > 0) || (_tcpRxBodySize > 0)) &&
      flprog::isTimer(_tcpRxStartTime, _timeoutTime))
  {
    _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
  }
  _bufferSize = 0;
  while (_server.available())
  {
    uint8_t value = _server.read();
    if (_tcpRxMbapSize < 6)
    {
      if ((_tcpRxMbapSize == 0) && (_tcpRxBodySize == 0)) _tcpRxStartTime = millis();
      _mbapBuffer[_tcpRxMbapSize++] = value;
      if (_tcpRxMbapSize == 6)
      {
        _tcpRxExpectedSize = (uint16_t)word(_mbapBuffer[4], _mbapBuffer[5]);
        if ((_tcpRxExpectedSize == 0) || (_tcpRxExpectedSize > FLPROG_MODBUS_BUFER_SIZE))
        {
          _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0; return;
        }
      }
      continue;
    }
    if (_tcpRxBodySize < _tcpRxExpectedSize) _buffer[_tcpRxBodySize++] = value;
    if ((_tcpRxMbapSize == 6) && (_tcpRxBodySize >= _tcpRxExpectedSize)) break;
  }
  if ((_tcpRxMbapSize < 6) || (_tcpRxExpectedSize == 0) || (_tcpRxBodySize < _tcpRxExpectedSize)) return;
  _bufferSize = (uint8_t)_tcpRxExpectedSize;
  _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
  if ((_mbapBuffer[2] != 0) || (_mbapBuffer[3] != 0))
  {
    _bufferSize = 0; return;
  }
  while (_executor->availableUart(_uart)) _executor->readUart(_uart);
  sendRTUBuffer();
  _rtuSendTime = millis();
  _currentStep = FLPROG_BRIDGE_WAITING_FOR_RESPONSE;
}

void ModbusBridge::serverRtuOverTcpModeRequestProcess()
{
  if ((_rtuOverTcpRxSize > 0) && flprog::isTimer(_tcpRxStartTime, _timeoutTime)) _rtuOverTcpRxSize = 0;
  while (_server.available())
  {
    uint8_t value = _server.read();
    if (_rtuOverTcpRxSize == 0) _tcpRxStartTime = millis();
    if (_rtuOverTcpRxSize < FLPROG_MODBUS_BUFER_SIZE) _buffer[_rtuOverTcpRxSize++] = value;
    uint16_t expectedNow = flprogModus::slaveRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
    if ((expectedNow > 0) && (_rtuOverTcpRxSize >= expectedNow)) break;
  }
  uint16_t pacadgeSize = flprogModus::slaveRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
  if ((pacadgeSize == 0) || (_rtuOverTcpRxSize < pacadgeSize)) return;
  if (!flprogModus::checkCRCOnBuffer(pacadgeSize, _buffer))
  {
    _rtuOverTcpRxSize = 0; return;
  }
  while (_executor->availableUart(_uart)) _executor->readUart(_uart);
  _bufferSize = pacadgeSize - 2;
  _rtuOverTcpRxSize = 0;
  sendRTUBuffer();
  _rtuSendTime = millis();
  _currentStep = FLPROG_BRIDGE_WAITING_FOR_RESPONSE;
}

void ModbusBridge::serverResponseProcess()
{
  uint16_t pacadgeSize;
  if (!_isPauseStatus)
  {
    if (flprog::isTimer(_rtuSendTime, _timeoutTime))
    {
      _bufferSize = 0;
      _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
      return;
    }
    if (!(_executor->availableUart(_uart)))
    {
      return;
    }
    while (_executor->availableUart(_uart))
    {
      if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE)
      {
        _buffer[_bufferSize] = _executor->readUart(_uart);
        _bufferSize++;
      }
      else
      {
        _executor->readUart(_uart);
      }
    }
    pacadgeSize = flprogModus::masterRTUPacadgeSize(_bufferSize, _buffer);
    if ((pacadgeSize == 0) || (_bufferSize < pacadgeSize))
    {
      return;
    }
    if (!_server.connected())
    {
      _bufferSize = 0;
      _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
      return;
    }
    _isPauseStatus = true;
    _startPauseTime = millis();
    return;
  }
  else
  {
    if ((flprog::isTimer(_startPauseTime, _pauseTime)))
    {
      _isPauseStatus = false;
    }
    else
    {
      return;
    }
    while (_server.available())
    {
      _server.read();
    }
    if (_mode == FLPROG_RTU_OVER_TCP_MODBUS)
    {
      _server.write(_buffer, _bufferSize);
    }
    else
    {
      pacadgeSize = flprogModus::masterRTUPacadgeSize(_bufferSize, _buffer);
      _bufferSize = pacadgeSize - 2;
      _mbapBuffer[4] = highByte(_bufferSize);
      _mbapBuffer[5] = lowByte(_bufferSize);
      // БЫЛО — исправил ander: два write(MBAP + PDU).
      // СТАЛО — исправил ander: полный ADU одним write().
      uint8_t tcpFrame[FLPROG_MODBUS_BUFER_SIZE + 6];
      for (uint8_t i = 0; i < 6; i++) tcpFrame[i] = _mbapBuffer[i];
      for (uint16_t i = 0; i < _bufferSize; i++) tcpFrame[i + 6] = _buffer[i];
      _server.write(tcpFrame, ((uint16_t)_bufferSize) + 6);
    }
    _bufferSize = 0;
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
  }
}

void ModbusBridge::clientResponseProcess()
{
  if (flprog::isTimer(_rtuSendTime, _timeoutTime))
  {
    _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0; _rtuOverTcpRxSize = 0;
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
    return;
  }
  if (!_tcpClient.connected())
  {
    _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0; _rtuOverTcpRxSize = 0;
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
    return;
  }
  if (!_isPauseStatus)
  {
    if (!_tcpClient.available())
    {
      return;
    }
  }
  if (_mode == FLPROG_RTU_OVER_TCP_MODBUS)
  {
    clientRtuOverTCPModeResponseProcess();
    return;
  }
  clientTCPModeResponseProcess();
}

void ModbusBridge::clientRtuOverTCPModeResponseProcess()
{
  if (!_isPauseStatus)
  {
    while (_tcpClient.available())
    {
      uint8_t value = _tcpClient.read();
      if (_rtuOverTcpRxSize < FLPROG_MODBUS_BUFER_SIZE) _buffer[_rtuOverTcpRxSize++] = value;
      uint16_t expectedNow = flprogModus::masterRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
      if ((expectedNow > 0) && (_rtuOverTcpRxSize >= expectedNow)) break;
    }
    uint16_t pacadgeSize = flprogModus::masterRTUPacadgeSize(_rtuOverTcpRxSize, _buffer);
    if ((pacadgeSize == 0) || (_rtuOverTcpRxSize < pacadgeSize)) return;
    if (!flprogModus::checkCRCOnBuffer(pacadgeSize, _buffer))
    {
      _rtuOverTcpRxSize = 0; _bufferSize = 0;
      _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST; return;
    }
    _bufferSize = pacadgeSize - 2;
    _rtuOverTcpRxSize = 0;
    _isPauseStatus = true;
    _startPauseTime = millis();
    return;
  }
  if (flprog::isTimer(_startPauseTime, _pauseTime))
  {
    _isPauseStatus = false;
    sendRTUBuffer();
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
  }
}

void ModbusBridge::clientTCPModeResponseProcess()
{
  if (!_isPauseStatus)
  {
    _bufferSize = 0;
    while (_tcpClient.available())
    {
      uint8_t value = _tcpClient.read();
      if (_tcpRxMbapSize < 6)
      {
        _mbapBuffer[_tcpRxMbapSize++] = value;
        if (_tcpRxMbapSize == 6)
        {
          _tcpRxExpectedSize = (uint16_t)word(_mbapBuffer[4], _mbapBuffer[5]);
          if ((_tcpRxExpectedSize == 0) || (_tcpRxExpectedSize > FLPROG_MODBUS_BUFER_SIZE))
          {
            _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
            _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST; return;
          }
        }
        continue;
      }
      if (_tcpRxBodySize < _tcpRxExpectedSize) _buffer[_tcpRxBodySize++] = value;
      if ((_tcpRxMbapSize == 6) && (_tcpRxBodySize >= _tcpRxExpectedSize)) break;
    }
    if ((_tcpRxMbapSize < 6) || (_tcpRxExpectedSize == 0) || (_tcpRxBodySize < _tcpRxExpectedSize)) return;
    _bufferSize = (uint8_t)_tcpRxExpectedSize;
    _tcpRxMbapSize = 0; _tcpRxExpectedSize = 0; _tcpRxBodySize = 0;
    if ((_mbapBuffer[2] != 0) || (_mbapBuffer[3] != 0) ||
        (word(_mbapBuffer[0], _mbapBuffer[1]) != _transactionId))
    {
      _bufferSize = 0; _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST; return;
    }
    _isPauseStatus = true;
    _startPauseTime = millis();
    return;
  }
  if (flprog::isTimer(_startPauseTime, _pauseTime))
  {
    _isPauseStatus = false;
    sendRTUBuffer();
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
  }
}

void ModbusBridge::kascadaCloudResponseProcess()
{
  if (flprog::isTimer(_rtuSendTime, _timeoutTime))
  {
    _bufferSize = 0;
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
    return;
  }
  if (!(_executor->availableUart(_uart))) return;
  while (_executor->availableUart(_uart))
  {
    if (_bufferSize < FLPROG_MODBUS_BUFER_SIZE) _buffer[_bufferSize++] = _executor->readUart(_uart);
    else _executor->readUart(_uart);
  }
  uint16_t pacadgeSize = flprogModus::masterRTUPacadgeSize(_bufferSize, _buffer);
  if ((pacadgeSize == 0) || (_bufferSize < pacadgeSize)) return;
  if (!_tcpClient.connected())
  {
    _bufferSize = 0;
    _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
    return;
  }
  while (_tcpClient.available()) _tcpClient.read();
  _bufferSize = pacadgeSize - 2;
  _mbapBuffer[4] = highByte(_bufferSize);
  _mbapBuffer[5] = lowByte(_bufferSize);

  // БЫЛО — исправил ander: write(MBAP), write(PDU).
  // СТАЛО — исправил ander: один TCP ADU одним write().
  uint8_t tcpFrame[FLPROG_MODBUS_BUFER_SIZE + 6];
  for (uint8_t i = 0; i < 6; i++) tcpFrame[i] = _mbapBuffer[i];
  for (uint16_t i = 0; i < _bufferSize; i++) tcpFrame[i + 6] = _buffer[i];
  _tcpClient.write(tcpFrame, ((uint16_t)_bufferSize) + 6);
  _bufferSize = 0;
  _currentStep = FLPROG_BRIDGE_WAITING_FOR_REQUEST;
}

void ModbusBridge::begin()
{
  _tcpRxMbapSize = 0;
  _tcpRxExpectedSize = 0;
  _tcpRxBodySize = 0;
  _rtuOverTcpRxSize = 0;
  _tcpRxStartTime = millis();
  _isInit = true;
  _status = FLPROG_MODBUS_READY;
  _executor->beginUart(_uart);
  _tcpClient.stop();
  _server.stop();
  if (_pinPeDe >= 0)
  {
    pinMode(_pinPeDe, OUTPUT);
    digitalWrite(_pinPeDe, LOW);
  }
  if (_isServer && (_mode != FLPROG_KASCADA_CLOUD_MODBUS))
  {
    _server.setPort(_port);
  }
  _kaScadaCloudTimeStartTime = flprog::timeBack(5000);
}

void ModbusBridge::sendRTUBuffer()
{
  onPeDePin();
  uint16_t crc = flprogModus::modbusCalcCRC(_bufferSize, _buffer);
  _buffer[_bufferSize] = lowByte(crc);
  _bufferSize++;
  _buffer[_bufferSize] = highByte(crc);
  _bufferSize++;
  _executor->writeUart(_buffer, _bufferSize, _uart);
  if (!(_pinPeDe < 0))
  {
    uint8_t dataBits = 8;
    uint8_t stopBits = 1;
    uint8_t portParity = 0;
    uint16_t portSpeed = _executor->getSpeedUart(_uart);
    _timeOfSend = flprogModus::timeForSendBytes(dataBits, stopBits, portParity, portSpeed, _bufferSize);
    _startSendTime = millis();
    _status = FLPROG_MODBUS_WAITING_SENDING;
  }
  _bufferSize = 0;
}
