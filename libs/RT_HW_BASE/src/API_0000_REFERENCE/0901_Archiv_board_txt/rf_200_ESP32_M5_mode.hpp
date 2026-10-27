##############################################################
stamp-pico.name=STAMP-PICO

stamp-pico.upload.tool=esptool_py
stamp-pico.upload.maximum_size=1310720
stamp-pico.upload.maximum_data_size=327680
stamp-pico.upload.flags=
stamp-pico.upload.extra_flags=

stamp-pico.serial.disableDTR=true
stamp-pico.serial.disableRTS=true

stamp-pico.build.tarch=xtensa
stamp-pico.build.bootloader_addr=0x1000
stamp-pico.build.target=esp32
stamp-pico.build.mcu=esp32
stamp-pico.build.core=esp32
stamp-pico.build.variant=m5stack_stamp_pico
stamp-pico.build.board=STAMP_PICO
//--------------------------------
#define EXTERNAL_NUM_INTERRUPTS 16
#define NUM_DIGITAL_PINS        40
#define NUM_ANALOG_INPUTS       16

#define analogInputToDigitalPin(p)  (((p)<20)?(esp32_adc2gpio[(p)]):-1)
#define digitalPinToInterrupt(p)    (((p)<40)?(p):-1)
#define digitalPinHasPWM(p)         (p < 34)


static const uint8_t TX = 1;
static const uint8_t RX = 3;

static const uint8_t SDA = 21;
static const uint8_t SCL = 22;

static const uint8_t G26 = 26;
static const uint8_t G36 = 36;
static const uint8_t G18 = 18;
static const uint8_t G19 = 19;
static const uint8_t G21 = 21;
static const uint8_t G22 = 22;
static const uint8_t G25 = 25;
static const uint8_t G1 = 1;
static const uint8_t G3 = 3;
static const uint8_t G0 = 0;

static const uint8_t G32 = 32;
static const uint8_t G33 = 33;

static const uint8_t SS    = 19;
static const uint8_t MOSI  = 26;
static const uint8_t MISO  = 36;
static const uint8_t SCK   = 18;

#endif /* Pins_Arduino_h */



##############################################################

stamp-c3.name=STAMP-C3
stamp-c3.vid.0=0x303a
stamp-c3.pid.0=0x1001

stamp-c3.upload.tool=esptool_py
stamp-c3.upload.maximum_size=1310720
stamp-c3.upload.maximum_data_size=327680
stamp-c3.upload.flags=
stamp-c3.upload.extra_flags=
stamp-c3.upload.use_1200bps_touch=false
stamp-c3.upload.wait_for_upload_port=false

stamp-c3.serial.disableDTR=false
stamp-c3.serial.disableRTS=false

stamp-c3.build.tarch=riscv32
stamp-c3.build.target=esp
stamp-c3.build.mcu=esp32c3
stamp-c3.build.core=esp32
stamp-c3.build.variant=m5stack_stamp_c3
stamp-c3.build.board=STAMP_C3
stamp-c3.build.bootloader_addr=0x0

stamp-c3.build.cdc_on_boot=0
//-------------------------------------------------------------------
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
##############################################################
