// Сгенерировано FLP_next из проекта «selftest_logic3». Ручные правки будут перезаписаны.
// Порядок выполнения: poly → poly_text → ip → ip_back → ip_bad → ok_show → bad_show → ok_text → bad_text → d_text → line → line2 → tick → tx → tx2

#include <Arduino.h>

// Выходы блоков
float poly_out = 0.0f;
String poly_text_out = String();
String ip_out = String();
int32_t ip_back_a = 0;
int32_t ip_back_b = 0;
int32_t ip_back_c = 0;
int32_t ip_back_d = 0;
bool ip_back_ok = false;
int32_t ip_bad_a = 0;
int32_t ip_bad_b = 0;
int32_t ip_bad_c = 0;
int32_t ip_bad_d = 0;
bool ip_bad_ok = false;
float ok_show_out = 0.0f;
float bad_show_out = 0.0f;
String ok_text_out = String();
String bad_text_out = String();
String d_text_out = String();
String line_out = String();
String line2_out = String();
bool tick_out = false;
bool tx_sent = false;
bool tx2_sent = false;

// tick: generator
unsigned long tick_start = 0;

// tx: uart_send
bool tx_prev = false;

// tx2: uart_send
bool tx2_prev = false;

void setup() {
  // USB: 115200 бод
  Serial.begin(115200);

  // Память под строки заранее: меньше дробления кучи.
  poly_text_out.reserve(32);
  ip_out.reserve(32);
  ok_text_out.reserve(32);
  bad_text_out.reserve(32);
  d_text_out.reserve(32);
  line_out.reserve(32);
  line2_out.reserve(32);

}

void loop() {
  // poly: scale_table
  {
    // points are parsed from the "x:y;x:y" string once
    static float xs[32], ys[32];
    static int n = 0;
    if (n == 0) {
      const char *p = "0:0;50:20;100:100";
      while (*p && n < 32) {
        char *end;
        xs[n] = strtof(p, &end);
        if (*end != ':') break;
        p = end + 1;
        ys[n] = strtof(p, &end);
        n++;
        p = (*end == ';') ? end + 1 : end;
      }
      if (n == 0) { xs[0] = 0; ys[0] = 0; n = 1; }
    }
    float v = 75.0f;
    if (v <= xs[0]) {
      poly_out = ys[0];
    } else if (v >= xs[n - 1]) {
      poly_out = ys[n - 1];
    } else {
      int i = 1;
      while (v > xs[i]) i++;
      poly_out = ys[i - 1] + (ys[i] - ys[i - 1]) * (v - xs[i - 1]) / (xs[i] - xs[i - 1]);
    }
  }

  // poly_text: num_to_text
  poly_text_out = "poly=";
  poly_text_out += String(poly_out, 1);
  poly_text_out += "";

  // ip: ip_to_text
  ip_out = String(192 & 0xFF) + "." + String(168 & 0xFF) + "." + String(0 & 0xFF) + "." + String(69 & 0xFF);

  // ip_back: text_to_ip
  {
    int v[4] = {0, 0, 0, 0};
    int part = 0;
    bool digits = false;
    bool good = true;
    for (unsigned int i = 0; i < String("10.20.30.1").length() && good; i++) {
      char ch = String("10.20.30.1")[i];
      if (ch >= '0' && ch <= '9') {
        v[part] = v[part] * 10 + (ch - '0');
        digits = true;
        if (v[part] > 255) good = false;
      } else if (ch == '.' && digits && part < 3) {
        part++;
        digits = false;
      } else {
        good = false;
      }
    }
    good = good && digits && part == 3;
    if (good) {
      ip_back_a = v[0];
      ip_back_b = v[1];
      ip_back_c = v[2];
      ip_back_d = v[3];
    }
    ip_back_ok = good;
  }

  // ip_bad: text_to_ip
  {
    int v[4] = {0, 0, 0, 0};
    int part = 0;
    bool digits = false;
    bool good = true;
    for (unsigned int i = 0; i < String("10.20.300.1").length() && good; i++) {
      char ch = String("10.20.300.1")[i];
      if (ch >= '0' && ch <= '9') {
        v[part] = v[part] * 10 + (ch - '0');
        digits = true;
        if (v[part] > 255) good = false;
      } else if (ch == '.' && digits && part < 3) {
        part++;
        digits = false;
      } else {
        good = false;
      }
    }
    good = good && digits && part == 3;
    if (good) {
      ip_bad_a = v[0];
      ip_bad_b = v[1];
      ip_bad_c = v[2];
      ip_bad_d = v[3];
    }
    ip_bad_ok = good;
  }

  // ok_show: switch
  ok_show_out = ip_back_ok ? 1.0f : 0.0f;

  // bad_show: switch
  bad_show_out = ip_bad_ok ? 1.0f : 0.0f;

  // ok_text: num_to_text
  ok_text_out = "ok=";
  ok_text_out += String(ok_show_out, 0);
  ok_text_out += "";

  // bad_text: num_to_text
  bad_text_out = "bad=";
  bad_text_out += String(bad_show_out, 0);
  bad_text_out += "";

  // d_text: num_to_text
  d_text_out = "d=";
  d_text_out += String((float)ip_back_d, 0);
  d_text_out += "";

  // line: text_concat
  line_out = poly_text_out;
  line_out += " ";
  line_out += ok_text_out;
  line_out += " ";
  line_out += bad_text_out;
  line_out += " ";
  line_out += d_text_out;
  line_out += " ";
  line_out += String("");

  // line2: text_concat
  line2_out = String("ip=");
  line2_out += " ";
  line2_out += ip_out;

  // tick: generator
  if (true) {
    unsigned long phase = (millis() - tick_start) % (1000UL);
    tick_out = phase < 100UL;
  } else {
    tick_start = millis();
    tick_out = false;
  }

  // tx: uart_send
  {
    bool fire = tick_out && !tx_prev;
    tx_prev = tick_out;
    tx_sent = false;
    if (fire) {
      const String& text = line_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx_sent = true;
      }
    }
  }

  // tx2: uart_send
  {
    bool fire = tick_out && !tx2_prev;
    tx2_prev = tick_out;
    tx2_sent = false;
    if (fire) {
      const String& text = line2_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx2_sent = true;
      }
    }
  }
}
