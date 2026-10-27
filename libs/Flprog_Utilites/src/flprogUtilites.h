#pragma once

#include <Arduino.h>
#include "RT_HW_BASE.h"
#include "IPAddress.h"
#include "rt_hwTask\rt_hwTaskBase.h"

#ifdef ARDUINO_ARCH_AVR
#define FLPROG_SOFTWARE_SERIAL
#define FLPROG_WRITE_BUFFER_SIZE 100
#if !(defined(__AVR_ATmega2560__) || defined(__AVR_ATmega2561__) || defined(__AVR_ATmega1280__) || defined(__AVR_ATmega1281__) || defined(__AVR_ATmega640__) || defined(__AVR_ATmega641__) || defined(ARDUINO_AVR_MEGA2560))
#define FLPROG_COMPACT_LIBRARY_MODE
#endif
#else
#define FLPROG_WRITE_BUFFER_SIZE 800
#endif

#include "pin/FlprogPin.h"

#ifdef RT_HW_CORE_STM32
#include "variant/stm32/flprogUtilitesSTM32.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

#ifdef RT_HW_CORE_RP2040
#include "variant/rp2040/flprogUtilitesRP2040.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

#ifdef RT_HW_CORE_ESP32
#include "variant/esp32/flprogUtilitesESP32.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

#ifdef RT_HW_CORE_ESP8266
#include "variant/esp8266/flprogUtilitesESP8266.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

#ifdef RT_HW_CORE_AVR
#include "variant/avr/flprogUtilitesAVR.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

#ifndef FLPROG_UTILITES_CORE_SELECT
#include "variant/anon/flprogUtilitesAnon.h"
#define FLPROG_UTILITES_CORE_SELECT
#endif

// Базовые константы
#define FLPROG_INADDR_NONE IPAddress(0, 0, 0, 0)

#ifndef FLPROG_ETHERNET_MAX_SOCK_NUM
#if defined(RAMEND) && defined(RAMSTART) && ((RAMEND - RAMSTART) <= 2048)
#define FLPROG_ETHERNET_MAX_SOCK_NUM 4
#else
#define FLPROG_ETHERNET_MAX_SOCK_NUM 8
#endif
#endif

#ifdef FLPROG_COMPACT_LIBRARY_MODE
#define FLPROG_MODBUS_BUFER_SIZE 64
#define FLPROG_HOST_NAME_LENGTH 64
#define FLPROG_LED_PIN 13
#else
#define FLPROG_MODBUS_BUFER_SIZE 250
#define FLPROG_HOST_NAME_LENGTH 250
#define FLPROG_LED_PIN RT_HW_Base.getPinLed();
#endif

// Константы для датчиков
#define FLPROG_SENSOR_NOT_ERROR 0
#define FLPROG_SENSOR_NOT_READY_ERROR 1
#define FLPROG_SENSOR_DEVICE_NOT_FOUND_ERROR 2
#define FLPROG_SENSOR_DEVICE_NOT_CORRECT_BUS_NUMBER_ERROR 65
#define FLPROG_SENSOR_DEVICE_NOT_CORRECT_DEVICE_ADDRESS_ERROR 61
#define FLPROG_SENSOR_DEVICE_NOT_DEFINED_ERROR 5
#define FLPROG_SENSOR_DEVICE_NOT_CORRECT_DATA_ERROR 6
#define FLPROG_SENSOR_CRC_ERROR 72
#define FLPROG_SENSOR_WAITING_READ_STEP 0
#define FLPROG_SENSOR_WAITING_DELAY 1

// Типы сетевых интерфейсов
#define FLPROG_ANON_INTERFACE 0
#define FLPROG_ETHERNET_INTERFACE 1
#define FLPROG_ON_BOARD_WIFI_INTERFACE 2
#define FLPROG_ETHERNET_ON_BOARD_WIFI_ANON 3
#define FLPROG_ETHERNET_ON_BOARD_WIFI_ESP32 4
#define FLPROG_ETHERNET_ON_BOARD_WIFI_ESP8266 5
#define FLPROG_ETHERNET_ON_BOARD_WIFI_RP2040 6

// Контсанты результатов выполнения операций
#define FLPROG_ERROR 0
#define FLPROG_SUCCESS 1
#define FLPROG_WAIT 2

// Константы статусов
#define FLPROG_NOT_REDY_STATUS 0
#define FLPROG_READY_STATUS 1
#define FLPROG_WAIT_ETHERNET_HARDWARE_INIT_STATUS 2

#define FLPROG_WAIT_ETHERNET_DHCP_STATUS 3
#define FLPROG_WAIT_ETHERNET_DNS_STATUS 4
#define FLPROG_WAIT_ETHERNET_UDP_STATUS 5

#define FLPROG_WAIT_ETHERNET_CLIENT_CONNECT_STATUS 6
#define FLPROG_WAIT_ETHERNET_LINK_ON_STATUS 7
#define FLPROG_ETHERNET_HARDWARE_INIT_STATUS 8
#define FLPROG_WAIT_ETHERNET_CONNECT_STATUS 9
#define FLPROG_WAIT_WIFI_CLIENT_CONNECT_STATUS 10
#define FLPROG_WAIT_ETHERNET_START_CLIENT_CONNECT_STATUS 11
#define FLPROG_WAIT_WEB_SERVER_READ_REQEST 12
#define FLPROG_WAIT_WEB_SERVER_SEND_ANSVER 13
#define FLPROG_WAIT_SEND_UDP_PACAGE 14
#define FLPROG_WAIT_UDP_PACAGE_ANSVER 15

#define FLPROG_WAIT_I2C_INIT 16
#define FLPROG_WAIT_I2C_FIND_ADDRESS 17
#define FLPROG_WAIT_DISPLAY_INIT 18
#define FLPROG_WAIT_SEND_DISPLAY_BUFFER 19
#define FLPROG_WAIT_I2C_DEVICE_INIT 22
#define FLPROG_WAIT_I2C_REFIND_ADDRES_PAUSE 23

// Коды состояния клиента
#define FLPROG_WAIT_ETHERNET_CLIENT_ANSWER_STATUS 20

// Коды состояния wifi
#define FLPROG_WAIT_WIFI_CLIENT_CONNECT 21

// Коды статуса линии Ethernet
#define FLPROG_ETHERNET_LINK_UNKNOWN 16
#define FLPROG_ETHERNET_LINK_ON 17
#define FLPROG_ETHERNET_LINK_OFF 18

// Коды чипов
#define FLPROG_ETHERNET_NO_HARDWARE 0
#define FLPROG_ETHERNET_W5100 51
#define FLPROG_ETHERNET_W5200 52
#define FLPROG_ETHERNET_W5500 55
#define FLPROG_ETHERNET_WIFI 90

// Коды ошибок
#define FLPROG_NOT_ERROR 0

#define FLPROG_ETHERNET_INTERFACE_NOT_READY_ERROR 10
#define FLPROG_ETHERNET_HARDWARE_INIT_ERROR 11
#define FLPROG_ETHERNET_SOKET_INDEX_ERROR 12
#define FLPROG_ETHERNET_SOKET_NOT_INIT_ERROR 13
#define FLPROG_ETHERNET_SOKET_SEND_TIMEOUT_ERROR 14
#define FLPROG_ETHERNET_SOKET_UDP_NOT_CORRECT_DATA_ERROR 15
#define FLPROG_ETHERNET_LINK_OFF_ERROR 16

#define FLPROG_ETHERNET_DHCP_NOT_CORRECT_RESULT_ERROR 20
#define FLPROG_ETHERNET_DHCP_TIMEOUT_ERROR 21
#define FLPROG_ETHERNET_DHCP_DISCOVERY_ERROR_ID_ERROR 22
#define FLPROG_ETHERNET_DHCP_REREQUEST_ERROR_ID_ERROR 23
#define FLPROG_ETHERNET_DHCP_REREQUEST_NAK_ERROR 24
#define FLPROG_ETHERNET_DHCP_NOT_DEFINED_ERROR 25
#define FLPROG_ETHERNET_DHCP_NOT_READY_ERROR 26

#define FLPROG_ETHERNET_DNS_NOT_READY_ERROR 30
#define FLPROG_ETHERNET_DNS_INVALID_SERVER 31
#define FLPROG_ETHERNET_DNS_TRUNCATED 32
#define FLPROG_ETHERNET_DNS_INVALID_RESPONSE 33

#define FLPROG_ETHERNET_UDP_SOKET_START_ERROR 40
#define FLPROG_ETHERNET_UDP_TIMEOUT_ERROR 41

#define FLPROG_ETHERNET_CLIENT_SOKET_START_ERROR 50
#define FLPROG_ETHERNET_CLIENT_CONNECT_TIMEOUT_ERROR 51
#define FLPROG_ETHERNET_CLIENT_SOKET_CLOSED_ERROR 52
#define FLPROG_ETHERNET_CLIENT_ANSWER_TIMEOUT_ERROR 53

#define FLPROG_ETHERNET_SERVER_NOT_CALLBACK_ERROR 60
#define FLPROG_ETHERNET_SERVER_SOKET_ERROR 61

#define FLPROG_ETHERNET_NTP_NOT_SERVER_ERROR 70

// Типы UART-ов
#define FLPROG_USB_UART 0
#define FLPROG_UART_UART 1
#define FLPROG_USART_UART 2

// Константы скоростей UART
#define FLPROG_SPEED_300 0
#define FLPROG_SPEED_600 1
#define FLPROG_SPEED_1200 2
#define FLPROG_SPEED_2400 3
#define FLPROG_SPEED_4800 4
#define FLPROG_SPEED_9600 5
#define FLPROG_SPEED_14400 6
#define FLPROG_SPEED_19200 7
#define FLPROG_SPEED_28800 8
#define FLPROG_SPEED_38400 9
#define FLPROG_SPEED_57600 10
#define FLPROG_SPEED_115200 11

// Константы Stop Bits UART
#define FLPROG_PORT_STOP_BITS_1 1
#define FLPROG_PORT_STOP_BITS_2 2

// Константы Data Bits UART
#define FLPROG_PORT_DATA_BITS_5 5
#define FLPROG_PORT_DATA_BITS_6 6
#define FLPROG_PORT_DATA_BITS_7 7
#define FLPROG_PORT_DATA_BITS_8 8

// Константы четности UART
#define FLPROG_PORT_PARITY_NONE 0
#define FLPROG_PORT_PARITY_EVEN 1
#define FLPROG_PORT_PARITY_ODD 2

namespace flprog
{
  //-------------Базовое--------------------
  void specialSetup();
  void specialPool();
  //-------------Тестирование--------------------
  bool isNumberChar(char value);
  bool isHexNumberChar(char value);

  //-------------Конвертация------------------------
  bool stringToBool(String value);
  char stringToChar(String value);
  int32_t hexStringToLong(String value);
  uint32_t hexStringToUnsignedLong(String value);
  int32_t binStringToLong(String value);
  uint32_t binStringToUnsignedLong(String value);

  //-------------Таймера------------------------
  bool isTimer(uint32_t startTime, uint32_t period);
  bool isTimerMicros(uint32_t startTime, uint32_t period);
  uint32_t difference32(uint32_t start, uint32_t end);
  uint32_t timeBack(uint32_t value);

  //---------------Сеть---------------------------------------------
  bool applyMac(uint8_t m0, uint8_t m1, uint8_t m2, uint8_t m3, uint8_t m4, uint8_t m5, uint8_t *target);
  bool checkMacAddres(uint8_t *target);
  void parseMacAddressString(String value, uint8_t *array);
  String macAddresToString(uint8_t *array);
  bool checkMacAddresString(String value);
  bool compareMacAddresWithString(uint8_t *array, String value);
  bool compareMacAddreses(uint8_t *array1, uint8_t *array2);
  int hexStrToInt(String str);
  bool inet_aton_1(const char *aIPAddrString, IPAddress &aResult);
  void ipToArray(IPAddress ip, uint8_t *array);
  uint32_t ipToNumber(IPAddress value);
  String ipToString(const IPAddress &ipAddress);
  IPAddress numberToIp(uint32_t value);
  IPAddress stringToIp(String value);
  bool checkIPAdressString(String value);
  String flprogErrorCodeName(uint8_t code);
  String flprogStatusCodeName(uint8_t code);
  // void printConsole(String title = "");
};

//=================================================================================================
//						Абстактный класс для всех классов в проекте с контролем состояния, статуса и ошибок
//=================================================================================================
class AbstractFLProgClass
{
public:
  uint8_t getStatus() { return _status; };       // Получение текущего статуса
  uint8_t getError() { return _errorCode; };     // Получение текущей ошибки
  uint8_t getErrorCode() { return _errorCode; }; // Получение кода ошибки

  // Флаги изменения параметров
  bool getIsChangeStatus() { return _isChangeStatus; };            // Получение флага изменения статуса
  bool getIsChangeStatusWithReset();                               // Получение флага изменения статуса с последующим сбросом флага
  void setIsChangeStatus(bool value) { _isChangeStatus = value; }; //
  void resetIsChangeStatus() { _isChangeStatus = false; };         // Сброс флага изменения статуса

  bool getIsChangeError() { return _isChangeError; };            // Получение флага изменения ошибки
  bool getIsChangeErrorWithReset();                              // Получение флага изменения ошибки с последующим сбросом флага
  void setIsChangeError(bool value) { _isChangeError = value; }; // Установка флага изменения ошибки
  void resetIsChangeError() { _isChangeError = false; };         // Сброс флага изменения ошибки

  uint32_t statusForExt() { return _statusForExt; };                            // Получение статуса для внешних функций. Статус для внешних функций - это 32-битное число, в котором каждый бит может быть использован для хранения флага изменения определенного параметра. Например, бит 0 может быть использован для хранения флага изменения статуса, бит 1 - для хранения флага изменения ошибки, бит 2 - для хранения флага изменения определенного параметра и т.д. Внешние функции могут использовать эти биты для определения того, какие параметры были изменены и какие действия необходимо предпринять в ответ на эти изменения.
  bool statusForExtGetBit(uint8_t bit) { return bitRead(_statusForExt, bit); }; // Получение бита статуса для внешних функций. bit - это номер бита, который нужно получить. Например, если bit = 0, то будет получен бит 0 статуса для внешних функций, если bit = 1, то будет получен бит 1 статуса для внешних функций и т.д.
  void statusForExtResetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 0); };  // Сброс бита статуса для внешних функций. bit - это номер бита, который нужно сбросить. Например, если bit = 0, то будет сброшен бит 0 статуса для внешних функций, если bit = 1, то будет сброшен бит 1 статуса для внешних функций и т.д.
  bool statusForExtGetBitWithReset(uint8_t bit);                                // Получение бита статуса для внешних функций с последующим сбросом бита. bit - это номер бита, который нужно получить и сбросить. Например, если bit = 0, то будет получен и сброшен бит 0 статуса для внешних функций, если bit = 1, то будет получен и сброшен бит 1 статуса для внешних функций и т.д.
  void statusForExtSetBit(uint8_t bit) { bitWrite(_statusForExt, bit, 1); };    // Установка бита статуса для внешних функций. bit - это номер бита, который нужно установить. Например, если bit = 0, то будет установлен бит 0 статуса для внешних функций, если bit = 1, то будет установлен бит 1 статуса для внешних функций и т.д.

  virtual void setFlags(); // Виртуальная функция для установки флагов изменения статуса и ошибки. Должна быть переопределена в классе, который наследуется от AbstractFLProgClass. В функции setFlags() должны быть реализованы все действия по установке флагов изменения статуса и ошибки, которые должны выполняться при каждом изменении статуса и ошибки.

  void setStatus(uint8_t value) { _status = value; };   // Установка статуса. При установке статуса должны быть установлены флаги изменения статуса и статуса для внешних функций.
  void setError(uint8_t value) { _errorCode = value; }; // Установка ошибки. При установке ошибки должны быть установлены флаги изменения ошибки и статуса для внешних функций.

  protected:
  uint8_t _status = FLPROG_NOT_REDY_STATUS;
  uint8_t _errorCode = FLPROG_NOT_ERROR;

  // Флаги изменения параметров
  uint8_t _oldStatus = FLPROG_NOT_REDY_STATUS;
  bool _isChangeStatus = false;
  uint8_t _oldError = FLPROG_NOT_ERROR;
  bool _isChangeError = false;
  uint32_t _statusForExt = 1;
};

//=================================================================================================
//						Абстактный класс для всех устройств реботающих  по таскам
//=================================================================================================
class AbstractTaskDevice : public AbstractFLProgClass
{
public:
  void setTaskQntPass(uint8_t qntPass) { _taskQntPass = qntPass; }; // --Кол-во пропусков перед проверкой времени;
  void setTaskPeriod(uint16_t period) { _taskPeriod = period; };    // --Период вызова;
  void setTaskMode(uint8_t mode) { _taskMode = mode; };             // --Тип задачи. При изменении типа задачи создается новый объект задачи соответствующего типа. Ресурсы старого объекта задачи освобождаются.
  virtual void pool();                                               // --Пул устройства. Внутри функции pool происходит проверка условий для запуска задачи и при выполнении условий вызывается функция devicePool(), которая должна быть реализована в классе устройства. В функции pool() также происходит инициализация задачи при первом вызове функции pool().
  void setTaskEn(bool value) { _taskEn = value; };                  // --Разрешение проверки условий для запуска задачи. Для задач без контроля периода всегда 1, для задач с контролем периода 1 при проверке условий внешней функции и при проверке времени, иначе 0;
  bool taskCanWork() { return _task.canWork(); };                   // -- Разрешения от таска

protected:
  virtual void init() = 0;     // --Инициализация устройства. Должна быть реализована в классе устройства. В функции init() должны быть реализованы все действия по инициализации устройства, которые должны выполняться при каждом вызове функции pool() при статусе FLPROG_WAIT_I2C_DEVICE_INIT.
  virtual void workPool() = 0; // --Рабочий пул устройства. Должна быть реализована в классе устройства. В функции workPool() должны быть реализованы все действия по работе с устройством, которые должны выполняться при каждом вызове функции pool() при статусе FLPROG_READY_STATUS.

  FlprogRtHwTask _task;
  uint8_t _taskMode = FLPROG_TASK_PERIODIK_EVENT_SKIP;
  uint8_t _taskQntPass = 0;
  uint16_t _taskPeriod = 100;
  bool _taskEn = true;
};

class AbstractSPIorSPNDevice : public AbstractTaskDevice
{
public:
  void setSckPin(uint8_t pin) { _sckPin = pin; };
  void setMisoPin(uint8_t pin) { _misoPin = pin; };
  void setMosiPin(uint8_t pin) { _mosiPin = pin; };
  void setCsPin(uint8_t pin) { _csPin = pin; };

protected:
  uint8_t _sckPin = 255;  // --Номер пина SCK для программного SPI. 255-не используется
  uint8_t _misoPin = 255; // --Номер пина MISO для программного SPI. 255-не используется
  uint8_t _mosiPin = 255; // --Номер пина MOSI для программного SPI. 255-не используется
  uint8_t _csPin = 255;   // --Номер пина CS . 255-не используется
  uint8_t _bus = 255;     // -- Номер шины SPI. 0 -254 - аппаратный SPI, 255 - программный SPN
  uint32_t _speed;        // -- Скорость SPI. Для аппаратного SPI - это скорость шины, для программного SPN - это проценты от максимальной
};

//=================================================================================================
//						Абстактный класс для всех устройств реботающих  по таскам на шине I2C (не на базе библиотек RT_HW_BASE)
//=================================================================================================
class AbstractFLProgI2CDevice : public AbstractTaskDevice
{

protected:
  virtual void devicePool();                              // --Пул устройства. Внутри функции pool происходит проверка условий для запуска задачи и при выполнении условий вызывается функция workPool(), которая должна быть реализована в классе устройства. В функции pool() также происходит инициализация задачи при первом вызове функции pool().                              // --Инициализация устройства. Должна быть реализована в классе устройства. В функции init() должны быть реализованы все действия по инициализации устройства, которые должны выполняться при каждом вызове функции pool() при статусе FLPROG_WAIT_I2C_DEVICE_INIT.                         // --Рабочий пул устройства. Должна быть реализована в классе устройства. В функции workPool() должны быть реализованы все действия по работе с устройством, которые должны выполняться при каждом вызове функции pool() при статусе FLPROG_READY_STATUS.
  virtual uint8_t readRegister(uint8_t reg);              // --Чтение регистра устройства.
  virtual void writeRegister(uint8_t reg, uint8_t value); // --Запись регистра устройства.
  uint32_t _pauseStartTime;

  RT_HW_STRUCT_I2C_DEV _device;
};
