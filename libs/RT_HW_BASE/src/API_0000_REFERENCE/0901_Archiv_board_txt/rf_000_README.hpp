//=================================================================================================
//              Идентификаторы Архитектур Arduino IDE
//=================================================================================================
//#if defined(ARDUINO_ARCH_ESP32)
//#if defined(ARDUINO_ARCH_RP2040)

//=================================================================================================
//              Строки для настройки Arduino IDE
//=================================================================================================
http://arduino.esp8266.com/stable/package_esp8266com_index.json 
https://raw.githubusercontent.com/espressif/arduino-esp32/gh-pages/package_esp32_dev_index.json
https://github.com/stm32duino/BoardManagerFiles/raw/main/package_stmicroelectronics_index.json
https://github.com/earlephilhower/arduino-pico/releases/download/global/package_rp2040_index.json

//--------------------------------------------------------------------------------------------------
http://arduino.esp8266.com/stable/package_esp8266com_index.json 
https://files.seeedstudio.com/arduino/package_seeeduino_boards_index.json 
https://github.com/stm32duino/BoardManagerFiles/raw/main/package_stmicroelectronics_index.json
https://raw.githubusercontent.com/espressif/arduino-esp32/gh-pages/package_esp32_dev_index.json
https://github.com/earlephilhower/arduino-pico/releases/download/global/package_rp2040_index.json
https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/arduino/package_m5stack_index.json
//================================================================================================
Для использования ядра STM32 от Clark Roger (удобно для STM32F103) необходимо установить среду: 
https://github.com/rogerclarkmelbourne/Arduino_STM32
//=================================================================================================
Для использования дополнительных контроллеров и/или с контроллерами с дополнительными свойствами
рекомендуется заменить:
1.Файл board.txt в ядре CORE ESP32 на файл из библиотеки в папке 0120_С++_board.txt\0104_ESP32_borad.txt\board.txt 
//==================================================================================================
Подерживаемые контроллеры:
AVR: Arduino UNO,Nano,Pro,MEGA2560;
SAM: Arduino DUE;
ESP8266: ESP8266 GENERIC,LOLIN(WEMOS)D1 mini Lite,LOLIN(WEMOS)D1 mini Pro,NODEMCU 0.9,NODEMCU 1.0;
ESP32S:  ESP32 DEV Module, ESP32S DEV BlueTooth EIS, TTGO T-Display;
ESP32S2: ESP32S2 DEV(в работе);
ESP32S3: (в работе);
ESP32C3: ESP32C3 DEV,ESP32C3 STAMP-C3 EIS, ESP32C3 STAMP-C3U EIS,ESP32C3_TTGO_T_OI_PLUS_EIS,
         ESP32C3 LuatOS USB EIS, ESP32C3 LuatOS Classic EIS;
ESP32-M5:  M5Stack-Core-ESP32, M5Stack-ATOM;
ESP32_Pico: STAMP-PICO;
STM32F1(Clark): Generic STM32F103C series,Generic STM32F103R series,Generic STM32F103V series;
STM32F1(STM):   BluePill F103C8, BluePill F103CB, BlackPill F103C8,BlackPill F103CB,Generic F103CBTx,
                Generic F103RETx,Generic F103VETx;
STM32F4(Clark): Generic_STM32F407V-->DevEBox STM32F407VEt6 Mini, 
                Seeed Arch Max 1.1-->STM32F4xx_F4VE,
                BlackPill F401CCU6, BlackPill F411CE;
STM32F4(STM):   Black F407VE-->DevEBox STM32F407VEt6 Mini, 
                Black F407VG-->STM32F4xx_F4VE,
				BlackPill F401CC, BlackPill F411CE;
STM32H7: (в работе);    
SEEEDUINO: Seeeduno XIAO M0, Seeeduno Zero;
RASPBERRY PI PICO: Raspberry Pi Pico;
---------------------------------------------------------------------------------------------------

https://www.wemos.cc/en/latest/tutorials/s2/get_started_with_arduino_s2.html
Upload Code
---------------------------
Перевести S2 в режим Device Firmware Upgrade (DFU)->
 1.Удерживать кнопку Button 0; 
 2.Нажать Reset(однократно);
 3.После звукового сигнала о переподключении USB, отпустить кнопку Button 0. 
 
 В настройках IDE USB CDC On Boot: Enable
 Upload Mode: Internal USB
 
 Для платы "S2 mini" Arduino IDE удобно выбирать плату "LOLIN S2 Mini".
 
 После загрузки может быть сообщение:
"WARNING: ESP32-S2FNR2 chip was placed into download mode using GPIO0.
esptool.py can not exit the download mode over USB. To run the app, reset the chip manually.
To suppress this note, set --after option to 'no_reset'.
Произошла ошибка при загрузке скетч

Для продолжения выйти из монитора порта, нажать сброс,
после звукового сигнала о переподключении USB можно перейти в монитор порта.

//-------Документация на RP2040--------------------------------------------------------------------
https://arduino-pico.readthedocs.io/en/latest/



