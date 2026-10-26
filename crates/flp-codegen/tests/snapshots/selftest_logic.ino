// Сгенерировано FLP_next из проекта «selftest_logic». Ручные правки будут перезаписаны.
// Порядок выполнения: abs → mn → mx → lim → rt → sqr → pw → md → sw → mx4 → dm → gen → tog → deb → tt_n → deb_n → tick → t_abs → t_min → t_max → t_lim → t_sqrt → t_sq → t_pow → t_mod → t_sw → t_mux → t_dms1 → t_dms2 → t_dms3 → t_tt → t_deb → gn → t_gen → line1 → tx1 → line2 → tx2 → line3 → tx3 → line4 → tx4

#include <Arduino.h>

// Выходы блоков
float abs_out = 0.0f;
float mn_out = 0.0f;
float mx_out = 0.0f;
float lim_out = 0.0f;
float rt_out = 0.0f;
float sqr_out = 0.0f;
float pw_out = 0.0f;
float md_out = 0.0f;
float sw_out = 0.0f;
float mx4_out = 0.0f;
float dm_o1 = 0.0f;
float dm_o2 = 0.0f;
float dm_o3 = 0.0f;
bool gen_out = false;
bool tog_out = false;
bool deb_out = false;
float tt_n_out = 0.0f;
float deb_n_out = 0.0f;
bool tick_out = false;
String t_abs_out = String();
String t_min_out = String();
String t_max_out = String();
String t_lim_out = String();
String t_sqrt_out = String();
String t_sq_out = String();
String t_pow_out = String();
String t_mod_out = String();
String t_sw_out = String();
String t_mux_out = String();
String t_dms1_out = String();
String t_dms2_out = String();
String t_dms3_out = String();
String t_tt_out = String();
String t_deb_out = String();
float gn_out = 0.0f;
String t_gen_out = String();
String line1_out = String();
bool tx1_sent = false;
String line2_out = String();
bool tx2_sent = false;
String line3_out = String();
bool tx3_sent = false;
String line4_out = String();
bool tx4_sent = false;

// gen: generator
unsigned long gen_start = 0;

// tog: t_trigger
bool tog_prev = false;

// deb: debounce
bool deb_last = false;
unsigned long deb_changed = 0;

// tick: generator
unsigned long tick_start = 0;

// tx1: uart_send
bool tx1_prev = false;

// tx2: uart_send
bool tx2_prev = false;

// tx3: uart_send
bool tx3_prev = false;

// tx4: uart_send
bool tx4_prev = false;

void setup() {
  // USB: 115200 бод
  Serial.begin(115200);

  // Память под строки заранее: меньше дробления кучи.
  t_abs_out.reserve(32);
  t_min_out.reserve(32);
  t_max_out.reserve(32);
  t_lim_out.reserve(32);
  t_sqrt_out.reserve(32);
  t_sq_out.reserve(32);
  t_pow_out.reserve(32);
  t_mod_out.reserve(32);
  t_sw_out.reserve(32);
  t_mux_out.reserve(32);
  t_dms1_out.reserve(32);
  t_dms2_out.reserve(32);
  t_dms3_out.reserve(32);
  t_tt_out.reserve(32);
  t_deb_out.reserve(32);
  t_gen_out.reserve(32);
  line1_out.reserve(32);
  line2_out.reserve(32);
  line3_out.reserve(32);
  line4_out.reserve(32);

}

void loop() {
  // abs: abs
  abs_out = fabs(-3.5f);

  // mn: min
  mn_out = 2.0f < 5.0f ? 2.0f : 5.0f;

  // mx: max
  mx_out = 2.0f > 5.0f ? 2.0f : 5.0f;

  // lim: limit
  lim_out = 150.0f < 0.0f ? 0.0f : (150.0f > 100.0f ? 100.0f : 150.0f);

  // rt: sqrt
  rt_out = 16.0f > 0.0f ? sqrtf(16.0f) : 0.0f;

  // sqr: sq
  sqr_out = -3.0f * -3.0f;

  // pw: pow
  pw_out = powf(2.0f, 10.0f);

  // md: mod
  md_out = 5.0f != 0.0f ? fmodf(17.0f, 5.0f) : 0.0f;

  // sw: switch
  sw_out = true ? 2.0f : 1.0f;

  // mx4: mux
  {
    int idx = (int)2.0f;
    if (idx <= 0) {
      mx4_out = 10.0f;
    } else
    if (idx <= 1) {
      mx4_out = 20.0f;
    } else
    mx4_out = 30.0f;
  }

  // dm: dms
  {
    int idx = (int)1.0f;
    dm_o1 = idx == 0 ? 7.0f : 0.0f;
    dm_o2 = idx == 1 ? 7.0f : 0.0f;
    dm_o3 = idx == 2 ? 7.0f : 0.0f;
  }

  // gen: generator
  if (true) {
    unsigned long phase = (millis() - gen_start) % (1400UL);
    gen_out = phase < 700UL;
  } else {
    gen_start = millis();
    gen_out = false;
  }

  // tog: t_trigger
  if (false) {
    tog_out = false;
  } else if (gen_out && !tog_prev) {
    tog_out = !tog_out;
  }
  tog_prev = gen_out;

  // deb: debounce
  if (gen_out != deb_last) {
    deb_last = gen_out;
    deb_changed = millis();
  }
  if (millis() - deb_changed >= 100UL) {
    deb_out = deb_last;
  }

  // tt_n: switch
  tt_n_out = tog_out ? 1.0f : 0.0f;

  // deb_n: switch
  deb_n_out = deb_out ? 1.0f : 0.0f;

  // tick: generator
  if (true) {
    unsigned long phase = (millis() - tick_start) % (1000UL);
    tick_out = phase < 100UL;
  } else {
    tick_start = millis();
    tick_out = false;
  }

  // t_abs: num_to_text
  t_abs_out = "abs=";
  t_abs_out += String(abs_out, 1);
  t_abs_out += "";

  // t_min: num_to_text
  t_min_out = "min=";
  t_min_out += String(mn_out, 1);
  t_min_out += "";

  // t_max: num_to_text
  t_max_out = "max=";
  t_max_out += String(mx_out, 1);
  t_max_out += "";

  // t_lim: num_to_text
  t_lim_out = "lim=";
  t_lim_out += String(lim_out, 1);
  t_lim_out += "";

  // t_sqrt: num_to_text
  t_sqrt_out = "sqrt=";
  t_sqrt_out += String(rt_out, 1);
  t_sqrt_out += "";

  // t_sq: num_to_text
  t_sq_out = "sq=";
  t_sq_out += String(sqr_out, 1);
  t_sq_out += "";

  // t_pow: num_to_text
  t_pow_out = "pow=";
  t_pow_out += String(pw_out, 1);
  t_pow_out += "";

  // t_mod: num_to_text
  t_mod_out = "mod=";
  t_mod_out += String(md_out, 1);
  t_mod_out += "";

  // t_sw: num_to_text
  t_sw_out = "sw=";
  t_sw_out += String(sw_out, 1);
  t_sw_out += "";

  // t_mux: num_to_text
  t_mux_out = "mux=";
  t_mux_out += String(mx4_out, 1);
  t_mux_out += "";

  // t_dms1: num_to_text
  t_dms1_out = "dms1=";
  t_dms1_out += String(dm_o1, 1);
  t_dms1_out += "";

  // t_dms2: num_to_text
  t_dms2_out = "dms2=";
  t_dms2_out += String(dm_o2, 1);
  t_dms2_out += "";

  // t_dms3: num_to_text
  t_dms3_out = "dms3=";
  t_dms3_out += String(dm_o3, 1);
  t_dms3_out += "";

  // t_tt: num_to_text
  t_tt_out = "tt=";
  t_tt_out += String(tt_n_out, 1);
  t_tt_out += "";

  // t_deb: num_to_text
  t_deb_out = "deb=";
  t_deb_out += String(deb_n_out, 1);
  t_deb_out += "";

  // gn: switch
  gn_out = gen_out ? 1.0f : 0.0f;

  // t_gen: num_to_text
  t_gen_out = "gen=";
  t_gen_out += String(gn_out, 1);
  t_gen_out += "";

  // line1: text_concat
  line1_out = t_abs_out;
  line1_out += " ";
  line1_out += t_min_out;
  line1_out += " ";
  line1_out += t_max_out;
  line1_out += " ";
  line1_out += t_lim_out;

  // tx1: uart_send
  {
    bool fire = tick_out && !tx1_prev;
    tx1_prev = tick_out;
    tx1_sent = false;
    if (fire) {
      const String& text = line1_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx1_sent = true;
      }
    }
  }

  // line2: text_concat
  line2_out = t_sqrt_out;
  line2_out += " ";
  line2_out += t_sq_out;
  line2_out += " ";
  line2_out += t_pow_out;
  line2_out += " ";
  line2_out += t_mod_out;

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

  // line3: text_concat
  line3_out = t_sw_out;
  line3_out += " ";
  line3_out += t_mux_out;
  line3_out += " ";
  line3_out += t_dms1_out;
  line3_out += " ";
  line3_out += t_dms2_out;

  // tx3: uart_send
  {
    bool fire = tick_out && !tx3_prev;
    tx3_prev = tick_out;
    tx3_sent = false;
    if (fire) {
      const String& text = line3_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx3_sent = true;
      }
    }
  }

  // line4: text_concat
  line4_out = t_dms3_out;
  line4_out += " ";
  line4_out += t_tt_out;
  line4_out += " ";
  line4_out += t_deb_out;
  line4_out += " ";
  line4_out += t_gen_out;

  // tx4: uart_send
  {
    bool fire = tick_out && !tx4_prev;
    tx4_prev = tick_out;
    tx4_sent = false;
    if (fire) {
      const String& text = line4_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx4_sent = true;
      }
    }
  }
}
