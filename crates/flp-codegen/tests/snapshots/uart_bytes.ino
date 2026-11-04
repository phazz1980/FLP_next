// Сгенерировано CrabLogic из проекта «uart_bytes». Ручные правки будут перезаписаны.
// Порядок выполнения: rx → echo → half → level → cfg

#include <Arduino.h>

// Выходы блоков
int32_t rx_byte = 0;
bool rx_new = false;
int32_t rx_available = 0;
bool echo_sent = false;
float half_out = 0.0f;
bool level_sent = false;
int32_t cfg_baud = 0;
bool cfg_ok = false;

// level: uart_send
float level_last;
bool level_started = false;

// cfg: uart_config
bool cfg_prev = false;

// uart_config: общий код
// Arduino frame constant for a format like "8E1"; 0 if the text is not a format.
uint32_t flp_uart_format(const String& text) {
  String f = text;
  f.trim();
  f.toUpperCase();
  if (f == "5N1") return SERIAL_5N1;
  if (f == "5N2") return SERIAL_5N2;
  if (f == "5E1") return SERIAL_5E1;
  if (f == "5E2") return SERIAL_5E2;
  if (f == "5O1") return SERIAL_5O1;
  if (f == "5O2") return SERIAL_5O2;
  if (f == "6N1") return SERIAL_6N1;
  if (f == "6N2") return SERIAL_6N2;
  if (f == "6E1") return SERIAL_6E1;
  if (f == "6E2") return SERIAL_6E2;
  if (f == "6O1") return SERIAL_6O1;
  if (f == "6O2") return SERIAL_6O2;
  if (f == "7N1") return SERIAL_7N1;
  if (f == "7N2") return SERIAL_7N2;
  if (f == "7E1") return SERIAL_7E1;
  if (f == "7E2") return SERIAL_7E2;
  if (f == "7O1") return SERIAL_7O1;
  if (f == "7O2") return SERIAL_7O2;
  if (f == "8N1") return SERIAL_8N1;
  if (f == "8N2") return SERIAL_8N2;
  if (f == "8E1") return SERIAL_8E1;
  if (f == "8E2") return SERIAL_8E2;
  if (f == "8O1") return SERIAL_8O1;
  if (f == "8O2") return SERIAL_8O2;
  return 0;
}

void setup() {
  // UART2: 115200 бод, 8N1, RX GPIO16, TX GPIO17
  Serial2.setRxBufferSize(256);
  Serial2.begin(115200, SERIAL_8N1, 16, 17);

}

void loop() {
  // rx: uart_read
  rx_new = Serial2.available() > 0;
  if (rx_new) {
    rx_byte = Serial2.read();
  }
  rx_available = Serial2.available();

  // echo: uart_send
  {
    bool fire = rx_new;
    echo_sent = false;
    if (fire) {
      uint8_t data = (uint8_t)constrain(rx_byte, 0L, 255L);
      if ((size_t)Serial2.availableForWrite() >= sizeof(data)) {
        Serial2.write((const uint8_t*)&data, sizeof(data));
        echo_sent = true;
      }
    }
  }

  // half: scale
  half_out = 0.0f + ((float)rx_byte - 0.0f) * (100.0f - 0.0f) / (255.0f - 0.0f);
  half_out = constrain(half_out, 0.0f, 100.0f);

  // level: uart_send
  {
    // On change; the first value after start goes out too.
    bool changed = !level_started || fabsf(half_out - level_last) >= 0.5f;
    bool fire = true && changed;
    level_sent = false;
    if (fire) {
      float data = half_out;
      if ((size_t)Serial2.availableForWrite() >= sizeof(data)) {
        Serial2.write((const uint8_t*)&data, sizeof(data));
        level_sent = true;
      }
      if (level_sent) {
        level_last = half_out;
        level_started = true;
      }
    }
  }

  // cfg: uart_config
  // Speed and format from the inputs take effect on a rising edge of apply:
  // 0 keeps the speed, an empty string keeps the format.
  if (true && !cfg_prev) {
    String format = String("");
    format.trim();
    uint32_t config = format.length() > 0 ? flp_uart_format(format) : 0;
    cfg_ok = format.length() == 0 || config != 0;
    unsigned long speed = 57600 > 0 ? (unsigned long)57600 : Serial2.baudRate();
    if (config != 0) {
      // A new frame format needs the port restarted.
      Serial2.end();
      Serial2.setRxBufferSize(256);
      Serial2.begin(speed, config, 16, 17);
    } else if (57600 > 0) {
      Serial2.updateBaudRate(speed);
    }
  }
  cfg_prev = true;
  cfg_baud = (int32_t)Serial2.baudRate();
}
