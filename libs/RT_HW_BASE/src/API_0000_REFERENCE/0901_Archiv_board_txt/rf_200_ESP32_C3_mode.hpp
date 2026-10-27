##############################################################

esp32c3.name=ESP32C3 Dev Module
esp32c3.vid.0=0x303a
esp32c3.pid.0=0x1001

esp32c3.bootloader.tool=esptool_py
esp32c3.bootloader.tool.default=esptool_py

esp32c3.upload.tool=esptool_py
esp32c3.upload.tool.default=esptool_py
esp32c3.upload.tool.network=esp_ota

esp32c3.upload.maximum_size=1310720
esp32c3.upload.maximum_data_size=327680
esp32c3.upload.flags=
esp32c3.upload.extra_flags=
esp32c3.upload.use_1200bps_touch=false
esp32c3.upload.wait_for_upload_port=false

esp32c3.serial.disableDTR=false
esp32c3.serial.disableRTS=false

esp32c3.build.tarch=riscv32
esp32c3.build.target=esp
esp32c3.build.mcu=esp32c3
esp32c3.build.core=esp32
esp32c3.build.variant=esp32c3
esp32c3.build.board=ESP32C3_DEV
esp32c3.build.bootloader_addr=0x0

esp32c3.build.cdc_on_boot=0


##############################################################
ttgo-t-oi-plus.name=TTGO T-OI PLUS RISC-V ESP32-C3

ttgo-t-oi-plus.bootloader.tool=esptool_py
ttgo-t-oi-plus.bootloader.tool.default=esptool_py

ttgo-t-oi-plus.upload.tool=esptool_py
ttgo-t-oi-plus.upload.tool.default=esptool_py
ttgo-t-oi-plus.upload.tool.network=esp_ota

ttgo-t-oi-plus.upload.maximum_size=1310720
ttgo-t-oi-plus.upload.maximum_data_size=327680
ttgo-t-oi-plus.upload.flags=
ttgo-t-oi-plus.upload.extra_flags=

ttgo-t-oi-plus.serial.disableDTR=false
ttgo-t-oi-plus.serial.disableRTS=false

ttgo-t-oi-plus.build.tarch=riscv32
ttgo-t-oi-plus.build.target=esp
ttgo-t-oi-plus.build.mcu=esp32c3
ttgo-t-oi-plus.build.core=esp32
ttgo-t-oi-plus.build.variant=ttgo-t-oi-plus
ttgo-t-oi-plus.build.board=TTGO-T-OI-PLUS_DEV
ttgo-t-oi-plus.build.bootloader_addr=0x0

ttgo-t-oi-plus.build.cdc_on_boot=0
##############################################################

lolin_c3_mini.name=LOLIN C3 Mini
lolin_c3_mini.vid.0=0x303a
lolin_c3_mini.pid.0=0x1001

lolin_c3_mini.bootloader.tool=esptool_py
lolin_c3_mini.bootloader.tool.default=esptool_py

lolin_c3_mini.upload.tool=esptool_py
lolin_c3_mini.upload.tool.default=esptool_py
lolin_c3_mini.upload.tool.network=esp_ota

lolin_c3_mini.upload.maximum_size=1310720
lolin_c3_mini.upload.maximum_data_size=327680
lolin_c3_mini.upload.flags=
lolin_c3_mini.upload.extra_flags=
lolin_c3_mini.upload.use_1200bps_touch=false
lolin_c3_mini.upload.wait_for_upload_port=false

lolin_c3_mini.serial.disableDTR=true
lolin_c3_mini.serial.disableRTS=true

lolin_c3_mini.build.tarch=riscv32
lolin_c3_mini.build.target=esp
lolin_c3_mini.build.mcu=esp32c3
lolin_c3_mini.build.core=esp32
lolin_c3_mini.build.variant=lolin_c3_mini
lolin_c3_mini.build.board=LOLIN_C3_MINI
lolin_c3_mini.build.bootloader_addr=0x0

lolin_c3_mini.build.cdc_on_boot=1

##############################################################
# Adafruit QT Py ESP32-C3

adafruit_qtpy_esp32c3.name=Adafruit QT Py ESP32-C3
adafruit_qtpy_esp32c3.vid.0=0x303a
adafruit_qtpy_esp32c3.pid.0=0x1001

adafruit_qtpy_esp32c3.bootloader.tool=esptool_py
adafruit_qtpy_esp32c3.bootloader.tool.default=esptool_py

adafruit_qtpy_esp32c3.upload.tool=esptool_py
adafruit_qtpy_esp32c3.upload.tool.default=esptool_py
adafruit_qtpy_esp32c3.upload.tool.network=esp_ota

adafruit_qtpy_esp32c3.upload.maximum_size=1310720
adafruit_qtpy_esp32c3.upload.maximum_data_size=327680
adafruit_qtpy_esp32c3.upload.flags=
adafruit_qtpy_esp32c3.upload.extra_flags=
adafruit_qtpy_esp32c3.upload.use_1200bps_touch=false
adafruit_qtpy_esp32c3.upload.wait_for_upload_port=false

adafruit_qtpy_esp32c3.serial.disableDTR=false
adafruit_qtpy_esp32c3.serial.disableRTS=false

adafruit_qtpy_esp32c3.build.tarch=riscv32
adafruit_qtpy_esp32c3.build.bootloader_addr=0x0
adafruit_qtpy_esp32c3.build.target=esp
adafruit_qtpy_esp32c3.build.mcu=esp32c3
adafruit_qtpy_esp32c3.build.core=esp32
adafruit_qtpy_esp32c3.build.variant=adafruit_qtpy_esp32c3
adafruit_qtpy_esp32c3.build.board=ADAFRUIT_QTPY_ESP32C3

adafruit_qtpy_esp32c3.build.cdc_on_boot=1
adafruit_qtpy_esp32c3.build.f_cpu=160000000L
adafruit_qtpy_esp32c3.build.flash_size=4MB
adafruit_qtpy_esp32c3.build.flash_freq=80m
adafruit_qtpy_esp32c3.build.flash_mode=dio
adafruit_qtpy_esp32c3.build.boot=qio
adafruit_qtpy_esp32c3.build.partitions=default
adafruit_qtpy_esp32c3.build.defines=

adafruit_qtpy_esp32c3.menu.CDCOnBoot.cdc=Enabled
adafruit_qtpy_esp32c3.menu.CDCOnBoot.cdc.build.cdc_on_boot=1
adafruit_qtpy_esp32c3.menu.CDCOnBoot.default=Disabled
adafruit_qtpy_esp32c3.menu.CDCOnBoot.default.build.cdc_on_boot=0


##############################################################
dfrobot_beetle_esp32c3.name=DFRobot Beetle ESP32-C3
#dfrobot_beetle_esp32c3.vid.0=0x3343
#dfrobot_beetle_esp32c3.pid.0=0x8364
dfrobot_beetle_esp32c3.vid.0=0x303a
dfrobot_beetle_esp32c3.pid.0=0x1001

dfrobot_beetle_esp32c3.bootloader.tool=esptool_py
dfrobot_beetle_esp32c3.bootloader.tool.default=esptool_py

dfrobot_beetle_esp32c3.upload.tool=esptool_py
dfrobot_beetle_esp32c3.upload.tool.default=esptool_py
dfrobot_beetle_esp32c3.upload.tool.network=esp_ota

dfrobot_beetle_esp32c3.upload.maximum_size=1310720
dfrobot_beetle_esp32c3.upload.maximum_data_size=327680
dfrobot_beetle_esp32c3.upload.flags=
dfrobot_beetle_esp32c3.upload.extra_flags=
dfrobot_beetle_esp32c3.upload.use_1200bps_touch=false
dfrobot_beetle_esp32c3.upload.wait_for_upload_port=false

dfrobot_beetle_esp32c3.serial.disableDTR=false
dfrobot_beetle_esp32c3.serial.disableRTS=false

dfrobot_beetle_esp32c3.build.tarch=riscv32
dfrobot_beetle_esp32c3.build.target=esp
dfrobot_beetle_esp32c3.build.mcu=esp32c3
dfrobot_beetle_esp32c3.build.core=esp32
dfrobot_beetle_esp32c3.build.variant=dfrobot_beetle_esp32c3
dfrobot_beetle_esp32c3.build.board=DFROBOT_BEETLE_ESP32_C3
dfrobot_beetle_esp32c3.build.board=ESP32C3_DEV
dfrobot_beetle_esp32c3.build.bootloader_addr=0x0

dfrobot_beetle_esp32c3.build.cdc_on_boot=0
======================================================
esp32c3.build.tarch=riscv32
esp32c3.build.target=esp
esp32c3.build.mcu=esp32c3
esp32c3.build.core=esp32
esp32c3.build.variant=esp32c3
esp32c3.build.board=ESP32C3_DEV
esp32c3.build.bootloader_addr=0x0


//===================================================-
//          DFRobot Beetle ESP32-C3 
//====================================================
#ifndef Pins_Arduino_h
#define Pins_Arduino_h

#include <stdint.h>

#define USB_VID            0x3343
#define USB_PID            0x8364
#define USB_MANUFACTURER   "DFRobot"
#define USB_PRODUCT        "Beetle ESP32-C3"
#define USB_SERIAL         "" // Empty string for MAC adddress


#define EXTERNAL_NUM_INTERRUPTS 22
#define NUM_DIGITAL_PINS        22
#define NUM_ANALOG_INPUTS       6

#define analogInputToDigitalPin(p)  (((p)<NUM_ANALOG_INPUTS)?(analogChannelToDigitalPin(p)):-1)
#define digitalPinToInterrupt(p)    (((p)<NUM_DIGITAL_PINS)?(p):-1)
#define digitalPinHasPWM(p)         (p < EXTERNAL_NUM_INTERRUPTS)

static const uint8_t LED_BUILTIN = 10;

static const uint8_t TX = 21;
static const uint8_t RX = 20;

static const uint8_t SDA = 8;
static const uint8_t SCL = 9;

static const uint8_t SS    = 7;
static const uint8_t MOSI  = 6;
static const uint8_t MISO  = 5;
static const uint8_t SCK   = 4;

static const uint8_t A0 = 0;
static const uint8_t A1 = 1;
static const uint8_t A2 = 2;
static const uint8_t A3 = 3;
static const uint8_t A4 = 4;
static const uint8_t A5 = 5;

#define GDI_DISPLAY_FPC_INTERFACE
#ifdef  GDI_DISPLAY_FPC_INTERFACE

#define GDI_BLK   LED_BUILTIN
#define GDI_SPI_SCLK  SCK
#define GDI_SPI_MOSI  MOSI
#define GDI_SPI_MISO  MISO
#define GDI_DC    A1
#define GDI_RES   A2
#define GDI_CS    SS
#define GDI_SDCS  A0
#define GDI_TCS   A3
#define GDI_SCL   SCL
#define GDI_SDA   SDA

#endif

#endif /* Pins_Arduino_h */


//===========================
//           M5Stamp
//---------------------------
#ifndef Pins_Arduino_h
#define Pins_Arduino_h

#include <stdint.h>

#define EXTERNAL_NUM_INTERRUPTS 22
#define NUM_DIGITAL_PINS        22
#define NUM_ANALOG_INPUTS       6

#define analogInputToDigitalPin(p)  (((p)<NUM_ANALOG_INPUTS)?(esp32_adc2gpio[(p)]):-1)
#define digitalPinToInterrupt(p)    (((p)<NUM_DIGITAL_PINS)?(p):-1)
#define digitalPinHasPWM(p)         (p < EXTERNAL_NUM_INTERRUPTS)

static const uint8_t TX = 21;
static const uint8_t RX = 20;

static const uint8_t SDA = 8;
static const uint8_t SCL = 9;

static const uint8_t SS    = 7;
static const uint8_t MOSI  = 6;
static const uint8_t MISO  = 5;
static const uint8_t SCK   = 4;

static const uint8_t A0 = 0;
static const uint8_t A1 = 1;
static const uint8_t A2 = 2;
static const uint8_t A3 = 3;
static const uint8_t A4 = 4;
static const uint8_t A5 = 5;

#endif /* Pins_Arduino_h */

//========================================
//  ESP32C3 Dev
//---------------------------------------
#ifndef Pins_Arduino_h
#define Pins_Arduino_h

#include <stdint.h>
#include "soc/soc_caps.h"

#define EXTERNAL_NUM_INTERRUPTS 22
#define NUM_DIGITAL_PINS        22
#define NUM_ANALOG_INPUTS       6

static const uint8_t LED_BUILTIN = SOC_GPIO_PIN_COUNT+8;
#define BUILTIN_LED  LED_BUILTIN // backward compatibility
#define LED_BUILTIN LED_BUILTIN
#define BOARD_HAS_NEOPIXEL
#define LED_BRIGHTNESS 64

#define analogInputToDigitalPin(p)  (((p)<NUM_ANALOG_INPUTS)?(analogChannelToDigitalPin(p)):-1)
#define digitalPinToInterrupt(p)    (((p)<NUM_DIGITAL_PINS)?(p):-1)
#define digitalPinHasPWM(p)         (p < EXTERNAL_NUM_INTERRUPTS)

static const uint8_t TX = 21;
static const uint8_t RX = 20;

static const uint8_t SDA = 8;
static const uint8_t SCL = 9;

static const uint8_t SS    = 7;
static const uint8_t MOSI  = 6;
static const uint8_t MISO  = 5;
static const uint8_t SCK   = 4;

static const uint8_t A0 = 0;
static const uint8_t A1 = 1;
static const uint8_t A2 = 2;
static const uint8_t A3 = 3;
static const uint8_t A4 = 4;
static const uint8_t A5 = 5;

#endif /* Pins_Arduino_h */


#ifdef SOC_ADC_SUPPORTED
#include "soc/adc_periph.h"

int8_t digitalPinToAnalogChannel(uint8_t pin) 
{
    uint8_t channel = 0;
    if (pin < SOC_GPIO_PIN_COUNT) {
        for (uint8_t i = 0; i < SOC_ADC_PERIPH_NUM; i++) {
            for (uint8_t j = 0; j < SOC_ADC_MAX_CHANNEL_NUM; j++) {
                if (adc_channel_io_map[i][j] == pin) {
                    return channel;
                }
                channel++;
            }
        }
    }
    return -1;
}

int8_t analogChannelToDigitalPin(uint8_t channel) 
{
    if (channel >= (SOC_ADC_PERIPH_NUM * SOC_ADC_MAX_CHANNEL_NUM)) {
        return -1;
    }
    uint8_t adc_unit = (channel / SOC_ADC_MAX_CHANNEL_NUM);
    uint8_t adc_chan = (channel % SOC_ADC_MAX_CHANNEL_NUM);
    return adc_channel_io_map[adc_unit][adc_chan];
}
#else
// No Analog channels availible
int8_t analogChannelToDigitalPin(uint8_t channel) 
{
    return -1;
}
#endif