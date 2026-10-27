//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//							1.Контроллер ESP32C3 SuperMini
//	ESP32-C3 поддерживает прерывания на всех GPIO, кроме зарезервированных:
//	GPIO0–GPIO21 — поддерживают attachInterrupt()
//  ❗ Исключение: пины, используемые для внутренней загрузки / Flash, не всегда надёжны для внешних прерываний.
//	Рекомендуемые пины для прерываний: GPIO2, GPIO3, GPIO10, GPIO18, GPIO19, GPIO20, GPIO21
//
//	ESP32-C3 имеет 6 каналов PWM (LEDC). Любой GPIO может быть назначен как PWM-выход.
//	PWM поддерживается на всех пинах, кроме: GPIO8 и GPIO9 — используются для Flash SPI, не рекомендуется использовать
//	Для servo используется analogWrite() или ESP32Servo (библиотека)
//  Рекомендуемые пины для PWM:       	GPIO2, GPIO3, GPIO10, GPIO18, GPIO19, GPIO20, GPIO21
//
//  ESP32-C3 поддерживает до 3 UART, но только один с аппаратным буфером (UART0)
//  Любые пины можно использовать с помощью Serial1.begin(baud, SERIAL_8N1, rxPin, txPin);
//  UART0 обычно используется для загрузки / монитора. Не используйте его для общения с устройствами без перенастройки.
//  UART0: RX-20, TX-21
//
//	ESP32-C3 поддерживает SPI через SPI периферии, назначаемые на любые GPIO:
//	Аппаратный SPI:	MOSI: любые GPIO, MISO: любые GPIO, SCK: любые GPIO, CS: любые GPIO
//  Переназначиение на любые другие пины через SPI.begin(SCK, MISO, MOSI, SS);
//
//	ESP32-C3 не имеет фиксированных I2C-пинов. Можно выбрать Wire.begin(SDA_pin, SCL_pin);
//  Рекомендованные пины: SDA: GPIO8, SCL: GPIO9
//	GPIO8 и GPIO9 также используются для встроенной Flash, так что лучше:
//	SDA: GPIO10	SCL: GPIO3 (или 19, 21 и т.п.)
//
//	Для использования BLE (Bluetooth Low Energy) на ESP32-C3 Super Mini можно воспользоваться библиотекой ESP32 BLE Arduino. 
//  На ESP32-C3 BLE работает через стек NimBLE, он легче и эффективнее, чем классический Bluedroid.
//  Найди NimBLE-Arduino от h2zero и установи.
//  Пример простого BLE-сервера
//	Этот скетч запускает BLE-сервер, который можно обнаружить с телефона:
/*
#include <NimBLEDevice.h>

NimBLEServer* pServer = nullptr;

void setup() {
  Serial.begin(115200);
  Serial.println("Starting BLE...");

  NimBLEDevice::init("ESP32C3_BLE");
  pServer = NimBLEDevice::createServer();

  NimBLEService *pService = pServer->createService("180F"); // Battery Service UUID
  NimBLECharacteristic *pCharacteristic = pService->createCharacteristic(
      "2A19",                    // Battery Level UUID
      NIMBLE_PROPERTY::READ | NIMBLE_PROPERTY::NOTIFY
  );

  pCharacteristic->setValue((uint8_t)100); // Уровень заряда = 100%
  pService->start();

  NimBLEAdvertising* pAdvertising = NimBLEDevice::getAdvertising();
  pAdvertising->addServiceUUID("180F");
  pAdvertising->start();

  Serial.println("BLE server started, advertising...");
}

void loop() {
  // Здесь можно обновлять значение характеристик
}
*/
// Как проверить
// Установи на телефон приложение nRF Connect или LightBlue. Запусти скетч, BLE устройство появится как ESP32C3_BLE.
// Подключись и прочитай характеристику.
// Особенности BLE на ESP32-C3
//		Работает только BLE, без классического Bluetooth.
//		Поддерживает GATT-сервер и клиент, включая уведомления и индикации.
//		NimBLE эффективнее, чем Bluedroid (экономит ОЗУ).
//		BLE и Wi-Fi одновременно работают, но могут мешать друг другу при высоких нагрузках.
//	🔄 Что можно сделать:
//	BLE-сенсоры (датчики, передающие данные телефону).
//	BLE-клавиатура/джойстик (с HID-профилем).
//	Передача данных между двумя ESP32 по BLE.
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if	!defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_ESP32) && (defined(ARDUINO_ESP32C3_SUPER_MINI_EIS) || defined(ARDUINO_ESP32C3_SUPER_MINI_EIS_BLE))
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_CODE     (uint8_t)54 				//--Идентификатор выбора платы;
	#define  RT_HW_CORE_ESP32C3								//--Дополнительный идентификатор для ESP32C3;

//-------------------------------------------------------------------------------------------------
 #if defined(ARDUINO_ESP32C3_SUPER_MINI_EIS_BLE)
	#define  RT_HW_BOARD_NAME  	 "FLProg: ESP32C3 Super Mini BLE 5.0"	//--Наименование платы;
	#define  RT_HW_ESP32_PERMIT_BLE										//--Подключить BLE 5.0;
	#define  RT_HW_ESP32C3_PERMIT_BLE									//--Подключить BLE 5.0;	
 #else
	#define  RT_HW_BOARD_NAME  	 "FLProg: ESP32C3 Super Mini"			//--Наименование платы;	
 #endif
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_MEMORY   "FLASH=4/32Mb; SRAM=400Kb; ROM=384Kb; RTC SRAM=8Kb;"//--Параметры памяти;
	#define  RT_HW_BOARD_FCPU     (uint16_t)160				//--Частота процессора;	
	#define  RT_HW_BOARD_VCC   	  3300						//--Значение Vсс процессора;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_ESP32C3_SUPER_MINI 				//--Идентификатор типа платы;
//=================================================================================================
//								1.1. Ограничение ресурсов контроллера
//=================================================================================================
	#define RT_HW_SPECIFIC_USING_1  "Download:: D3=1"
//=================================================================================================
//								1.2. Системные пины
//=================================================================================================
	#define  RT_HW_BOARD_PIN_LED        	8		//--Led на плате;
 	#define  RT_HW_BOARD_PIN_BUTTON    		9		//--Кнопка на плате;
	#define  RT_HW_BOARD_PIN_ADC       		4		//--ADC;	
	#define  RT_HW_BOARD_PIN_CS0        	10		//--SPI cs.0;
	#define  RT_HW_BOARD_PIN_CS1        	0		//--SPI cs.0;
//	#define  RT_HW_BOARD_PIN_CS2        	2		//--SPI cs.1;	
//	#define  RT_HW_BOARD_PIN_CS3        	3		//--SPI cs.3;	
//==================================================================================================
//								1.3. Тестовые устройства
//==================================================================================================
	#define  RT_HW_BOARD_PIN_SERVO    		2		//--Сенсор DS1820;
	#define  RT_HW_BOARD_PIN_SR04      		3		//--Сенсор HC-SR04;	
	#define  RT_HW_BOARD_PIN_DS1820    		4		//--Сенсор DS1820;	
	#define  RT_HW_BOARD_PIN_NST1001      	10		//--Сенсор NST1001;	
//=================================================================================================
//								1.4. Тестовые UART
//=================================================================================================		
	#define  RT_HW_BOARD_UART_CONSOLE   	0		//--Консоль через UART;	
	#define  RT_HW_BOARD_UART_MODBUS1  		1		//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_HMI  	   		1		//--Номер UART для HMI 1;
//==================================================================================================
//								1.5. Коммуникации
//==================================================================================================
// 	#define  RT_HW_PERMIT_USB						//--Разрешение работы с USB;
	#define  RT_HW_PERMIT_UART0						//--Разрешение работы с UART0;	
	#define  RT_HW_PERMIT_UART1						//--Разрешение работы с UART1;		
    #define  RT_HW_PERMIT_I2C0						//--Разрешение работы с i2c0; 
    #define  RT_HW_PERMIT_SPI0						//--Разрешение работы с SPI0; 
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART0_RX   				20
    #define  RT_HW_UART0_TX   				21	
    #define  RT_HW_UART1_RX   				 0
    #define  RT_HW_UART1_TX   				 1	 
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SCL  				 5	
    #define  RT_HW_I2C0_SDA  				 6
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK  				 8
    #define  RT_HW_SPI0_MISO 				 7
    #define  RT_HW_SPI0_MOSI 				 9
    #define  RT_HW_SPI0_SS    				 10	
//=================================================================================================
//										1.10. Доступные пины
//=================================================================================================  
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)13	
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)13	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)1	   
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)21
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)6				
    #define RT_HW_PERMIT_PIN_INT	(uint8_t)21 
    #define RT_HW_PERMIT_PIN_N5V	(uint8_t)21 
//--------------------------------Доступные пины контроллера---------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={ 0, 1, 2, 3, 4, 5, 	  		  		   RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={       2, 3,                  10,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={       2, 3,                  10,20,21, RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,20,21, RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
#endif

/*
Вот пример реализации BLE-кнопки на ESP32-C3 — устройство, которое эмулирует Bluetooth Low Energy HID-клавиатуру и при нажатии кнопки отправляет на смартфон или ПК "нажатие клавиши" (например, Enter).

🔹 Что нужно:
Кнопка, подключённая к пину (например, GPIO2)

Arduino IDE + библиотека NimBLE-Arduino

Телефон/ПК с поддержкой BLE и HID (например, Android с BT 4.0+)

📦 Установка библиотек
Установи:

NimBLE-Arduino (в Library Manager ищи NimBLE-Arduino от h2zero)

Если HID нет в NimBLE, можно использовать готовый HID-профиль (см. ниже)

📜 Код BLE HID-кнопки
cpp
Копировать
Редактировать
#include <NimBLEDevice.h>
#include <NimBLEHIDDevice.h>

#define BUTTON_PIN 2

NimBLEHIDDevice* hid;
NimBLECharacteristic* inputReport;
bool connected = false;

class MyCallbacks : public NimBLEServerCallbacks {
  void onConnect(NimBLEServer* pServer) {
    connected = true;
    Serial.println("BLE device connected");
  }
  void onDisconnect(NimBLEServer* pServer) {
    connected = false;
    NimBLEDevice::startAdvertising();
    Serial.println("BLE device disconnected");
  }
};

void setup() {
  Serial.begin(115200);
  pinMode(BUTTON_PIN, INPUT_PULLUP);

  NimBLEDevice::init("ESP32C3_Button");
  NimBLEServer* pServer = NimBLEDevice::createServer();
  pServer->setCallbacks(new MyCallbacks());

  hid = new NimBLEHIDDevice(pServer);
  inputReport = hid->inputReport(1); // Report ID = 1

  hid->manufacturer()->setValue("OpenAI");

  // HID Report Map for simple keyboard (Enter key)
  const uint8_t reportMap[] = {
    0x05, 0x01,     // Usage Page (Generic Desktop)
    0x09, 0x06,     // Usage (Keyboard)
    0xA1, 0x01,     // Collection (Application)
    0x05, 0x07,     //   Usage Page (Key Codes)
    0x19, 0xe0,     //   Usage Minimum (224)
    0x29, 0xe7,     //   Usage Maximum (231)
    0x15, 0x00,     //   Logical Minimum (0)
    0x25, 0x01,     //   Logical Maximum (1)
    0x75, 0x01,     //   Report Size (1)
    0x95, 0x08,     //   Report Count (8)
    0x81, 0x02,     //   Input (Data, Variable, Absolute)
    0x95, 0x01,     //   Report Count (1)
    0x75, 0x08,     //   Report Size (8)
    0x81, 0x01,     //   Input (Constant)
    0x95, 0x05,     //   Report Count (5)
    0x75, 0x01,     //   Report Size (1)
    0x05, 0x08,     //   Usage Page (LEDs)
    0x19, 0x01,     //   Usage Minimum (1)
    0x29, 0x05,     //   Usage Maximum (5)
    0x91, 0x02,     //   Output (Data, Variable, Absolute)
    0x95, 0x01,     //   Report Count (1)
    0x75, 0x03,     //   Report Size (3)
    0x91, 0x01,     //   Output (Constant)
    0x95, 0x06,     //   Report Count (6)
    0x75, 0x08,     //   Report Size (8)
    0x15, 0x00,     //   Logical Minimum (0)
    0x25, 0x65,     //   Logical Maximum (101)
    0x05, 0x07,     //   Usage Page (Key Codes)
    0x19, 0x00,     //   Usage Minimum (0)
    0x29, 0x65,     //   Usage Maximum (101)
    0x81, 0x00,     //   Input (Data, Array)
    0xC0            // End Collection
  };

  hid->reportMap((uint8_t*)reportMap, sizeof(reportMap));
  hid->startServices();

  NimBLEAdvertising* pAdvertising = NimBLEDevice::getAdvertising();
  pAdvertising->setAppearance(HID_KEYBOARD);
  pAdvertising->addServiceUUID(hid->hidService()->getUUID());
  pAdvertising->start();

  Serial.println("BLE HID Button ready.");
}

void loop() {
  static bool lastPressed = false;
  bool pressed = !digitalRead(BUTTON_PIN); // Нажат — LOW

  if (connected && pressed && !lastPressed) {
    Serial.println("Button Pressed → Send Enter");

    uint8_t enterKey[8] = {0, 0, 0x28, 0, 0, 0, 0, 0};  // HID code for Enter
    inputReport->setValue(enterKey, sizeof(enterKey));
    inputReport->notify();

    delay(10); // debounce

    uint8_t releaseKey[8] = {0};  // Отпустить все
    inputReport->setValue(releaseKey, sizeof(releaseKey));
    inputReport->notify();
  }

  lastPressed = pressed;
}
🧪 Проверка
На ПК/телефоне включи Bluetooth.

Включи поиск HID-устройств (например, в разделе клавиатур).

Подключись к ESP32C3_Button.

Нажимай кнопку — должна приходить клавиша Enter.

*/




//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
//							2.Контроллер ESP32C3 STAMP-C3U EIS
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
#if	!defined(RT_HW_BOARD_CODE) && defined(RT_HW_CORE_ESP32) && defined(ARDUINO_ESP32C3_STAMP_C3U_EIS)
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_CODE     (uint8_t)54 				//--Идентификатор выбора платы;
	#define  RT_HW_CORE_ESP32C3								//--Дополнительный идентификатор для ESP32C3;

//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_NAME  	 "ESP32C3 STAMP-C3U EIS"		//--Наименование платы;	
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_MEMORY   "FLASH=4/32Mb; SRAM=400Kb; ROM=384Kb; RTC SRAM=8Kb;"//--Параметры памяти;
	#define  RT_HW_BOARD_FCPU     (uint16_t)160				//--Частота процессора;	
	#define  RT_HW_BOARD_VCC   	  3300						//--Значение Vсс процессора;
//-------------------------------------------------------------------------------------------------
	#define  RT_HW_BOARD_ESP32C3_STAMP_C3_EIS 				//--Идентификатор типа платы;
//=================================================================================================
//								1.1. Ограничение ресурсов контроллера
//=================================================================================================
	#define RT_HW_SPECIFIC_USING_1  "Download:: D3=1"
//=================================================================================================
//								1.2. Системные пины
//=================================================================================================
//	#define  RT_HW_BOARD_PIN_LED        	9		//--Led на плате;
 	#define  RT_HW_BOARD_PIN_BUTTON    		9		//--Кнопка на плате;
	#define  RT_HW_BOARD_PIN_ADC       		0		//--ADC;	
	#define  RT_HW_BOARD_PIN_PWM       		3		//--ADC;		
	#define  RT_HW_BOARD_PIN_WS2812       	2		//--WS2812;	
	#define  RT_HW_BOARD_PIN_CS0        	7		//--SPI cs.0;
	#define  RT_HW_BOARD_PIN_CS1        	10		//--SPI cs.1;	
//==================================================================================================
//								1.3. Тестовые устройства
//==================================================================================================
	#define  RT_HW_BOARD_PIN_DS1820    		9		//--Сенсор DS1820;33, 14,13,
	#define  RT_HW_BOARD_PIN_SR04      		10		//--Сенсор HC-SR04;	
//=================================================================================================
//								1.4. Тестовые UART
//=================================================================================================		
	#define  RT_HW_BOARD_UART_CONSOLE   	0		//--Консоль через UART;	
	#define  RT_HW_BOARD_UART_MODBUS1  		1		//--Номер UART для MODBUS 1;
	#define  RT_HW_BOARD_UART_HMI  	   		0		//--Номер UART для HMI 1;
//==================================================================================================
//								1.5. Коммуникации
//==================================================================================================
// 	#define  RT_HW_PERMIT_USB						//--Разрешение работы с USB;
	#define  RT_HW_PERMIT_UART0						//--Разрешение работы с UART0;
    #define  RT_HW_PERMIT_UART1						//--Разрешение работы с UART1;			
    #define  RT_HW_PERMIT_I2C0						//--Разрешение работы с i2c0; 
    #define  RT_HW_PERMIT_SPI0						//--Разрешение работы с SPI0; 
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_UART0_RX   				20
    #define  RT_HW_UART0_TX   				21	
    #define  RT_HW_UART1_RX   				18
    #define  RT_HW_UART1_TX   				19	 
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_I2C0_SCL  				 8	
    #define  RT_HW_I2C0_SDA  				 9
//-------------------------------------------------------------------------------------------------
    #define  RT_HW_SPI0_SCK  				 4
    #define  RT_HW_SPI0_MISO 				 5
    #define  RT_HW_SPI0_MOSI 				 6
    #define  RT_HW_SPI0_SS    				 7	
//=================================================================================================
//										1.10. Доступные пины
//=================================================================================================  
    #define RT_HW_PERMIT_PIN_ALL 	(uint8_t)21	
    #define RT_HW_PERMIT_PIN_DOT 	(uint8_t)21	
    #define RT_HW_PERMIT_PIN_PWM	(uint8_t)21	   
    #define RT_HW_PERMIT_PIN_DIN	(uint8_t)21
    #define RT_HW_PERMIT_PIN_ADC	(uint8_t)6				
    #define RT_HW_PERMIT_PIN_INT	(uint8_t)21 
    #define RT_HW_PERMIT_PIN_N5V	(uint8_t)21 
//--------------------------------Доступные пины контроллера---------------------------------------
	const uint8_t PROGMEM RT_HW_PGM_PIN_ALL_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_DIN_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_ADC_ID[] ={ 0, 1, 2, 3, 4, 5, 	  		  			          		   RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_DOT_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_PWM_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};
	const uint8_t PROGMEM RT_HW_PGM_PIN_INT_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};	
	const uint8_t PROGMEM RT_HW_PGM_PIN_N5V_ID[] ={ 0, 1, 2, 3, 4, 5, 6, 7, 8,9,10,12,14,15,16,17,18,19,20,21, RT_HW_PGM_END};
//-------------------------------------------------------------------------------------------------
#endif
//=================================================================================================




