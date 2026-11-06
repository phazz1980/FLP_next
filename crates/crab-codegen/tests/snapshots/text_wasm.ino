// Сгенерировано CrabLogic из проекта «text_wasm». Ручные правки будут перезаписаны.
// Порядок выполнения: rx → num → is_on → temp_text → cmd_text → reply → len → cfg

#include <Arduino.h>

// Выходы блоков
String rx_line = String();
bool rx_ready = false;
int32_t rx_available = 0;
float num_out = 0.0f;
bool num_ok = false;
bool is_on_out = false;
String temp_text_out = String();
String cmd_text_out = String();
bool reply_sent = false;
int32_t len_out = 0;
int32_t cfg_baud = 0;
bool cfg_ok = false;

// rx: uart_read
String rx_buf;
unsigned long rx_last = 0;

// cfg: uart_config
bool cfg_prev = false;

// text_to_num: общий код
// Number from text: spaces around are skipped, a decimal comma is accepted.
bool crab_text_to_float(const String& text, float& out) {
  String t = text;
  t.trim();
  t.replace(',', '.');
  if (t.length() == 0) {
    return false;
  }
  char* end = nullptr;
  double v = strtod(t.c_str(), &end);
  if (end == t.c_str() || *end != '\0') {
    return false;
  }
  out = (float)v;
  return true;
}

// text_compare: общий код
static inline char crab_lower(char c) {
  return (c >= 'A' && c <= 'Z') ? (char)(c + 32) : c;
}
bool crab_text_match_nocase(const String& a, const String& b, int mode) {
  int la = a.length(), lb = b.length();
  if (lb > la || (mode == 0 && lb != la)) {
    return false;
  }
  int first = mode == 3 ? la - lb : 0;
  int last = (mode == 1 ? la : first + lb) - lb;
  for (int at = first; at <= last; at++) {
    int i = 0;
    while (i < lb && crab_lower(a[at + i]) == crab_lower(b[i])) {
      i++;
    }
    if (i == lb) {
      return true;
    }
  }
  return false;
}

// uart_config: общий код
// Arduino frame constant for a format like "8E1"; 0 if the text is not a format.
uint32_t crab_uart_format(const String& text) {
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

  // Память под строки заранее: меньше дробления кучи.
  rx_line.reserve(32);
  temp_text_out.reserve(14);
  cmd_text_out.reserve(32);

  // rx: uart_read
  rx_buf.reserve(32);
}

void loop() {
  // rx: uart_read
  rx_ready = false;
  // One line per cycle: the next one waits in the UART buffer.
  while (Serial2.available() > 0) {
    char c = (char)Serial2.read();
    rx_last = millis();
    if (c == '\r') {
      continue;
    }
    bool end = c == '\n';
    if (!end) {
      rx_buf += c;
      end = rx_buf.length() >= 32;
    }
    if (end) {
      rx_line = rx_buf;
      rx_buf = "";
      rx_ready = true;
      break;
    }
  }
  rx_available = Serial2.available();

  // num: text_to_num
  {
    float v;
    num_ok = crab_text_to_float(rx_line, v);
    if (num_ok) {
      num_out = v;
    }
  }

  // is_on: text_compare
  is_on_out = crab_text_match_nocase(rx_line, "on", 0);

  // temp_text: num_to_text
  temp_text_out = "T=";
  temp_text_out += String(num_out, 1);
  temp_text_out += "C";

  // cmd_text: text_concat
  cmd_text_out = String("команда ");
  cmd_text_out += "";
  cmd_text_out += rx_line;

  // reply: uart_send
  {
    bool fire = rx_ready;
    reply_sent = false;
    if (fire) {
      const String& text = temp_text_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial2.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial2.print(text);
        Serial2.print(ending);
        reply_sent = true;
      }
    }
  }

  // len: text_length
  len_out = (int32_t)rx_line.length();

  // cfg: uart_config
  // Speed and format from the inputs take effect on a rising edge of apply:
  // 0 keeps the speed, an empty string keeps the format.
  if (false && !cfg_prev) {
    String format = String("");
    format.trim();
    uint32_t config = format.length() > 0 ? crab_uart_format(format) : 0;
    cfg_ok = format.length() == 0 || config != 0;
    unsigned long speed = 115200 > 0 ? (unsigned long)115200 : Serial2.baudRate();
    if (config != 0) {
      // A new frame format needs the port restarted.
      Serial2.end();
      Serial2.setRxBufferSize(256);
      Serial2.begin(speed, config, 16, 17);
    } else if (115200 > 0) {
      Serial2.updateBaudRate(speed);
    }
  }
  cfg_prev = false;
  cfg_baud = (int32_t)Serial2.baudRate();
}
