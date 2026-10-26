// Сгенерировано FLP_next из проекта «selftest_logic2». Ручные правки будут перезаписаны.
// Порядок выполнения: cmp → sr → sn → cs → tn → band → bor → bxor → bnot → bread → bwrite → bts → tby → dec → enc → f2i → i2f → w2b → b2w → d2b → b2d → fnd → chr → addc → pid1 → and3 → xor3 → mux16 → dms16 → chg → schg → bchg → rt → rnd → tick → n_cmp → n_sr → n_bread → n_b0 → n_b1 → n_b2 → n_dec → n_any → n_and3 → n_xor3 → n_chg → n_schg → n_bchg → t_cmp → t_sr → t_sin → t_cos → t_tan → t_band → t_bor → t_bxor → t_bnot → t_bread → t_bwrite → t_b0 → t_b1 → t_b2 → t_tobyte → t_dec → t_enc → t_any → t_f2i → t_i2f → t_lo → t_hi → t_word → t_d0 → t_d1 → t_d2 → t_d3 → t_dword → t_find → t_code → t_and3 → t_xor3 → t_pid → t_mux16 → t_dms16 → t_chg → t_schg → t_bchg → t_rnd → line1 → tx1 → line2 → tx2 → line3 → tx3 → line4 → tx4 → line5 → tx5 → line6 → tx6 → line7 → tx7 → line8 → tx8 → line9 → tx9 → line10 → tx10

#include <Arduino.h>

// Выходы блоков
bool cmp_out = false;
bool sr_out = false;
float sn_out = 0.0f;
float cs_out = 0.0f;
float tn_out = 0.0f;
int32_t band_out = 0;
int32_t bor_out = 0;
int32_t bxor_out = 0;
int32_t bnot_out = 0;
bool bread_out = false;
int32_t bwrite_out = 0;
bool bts_b0 = false;
bool bts_b1 = false;
bool bts_b2 = false;
bool bts_b3 = false;
bool bts_b4 = false;
bool bts_b5 = false;
bool bts_b6 = false;
bool bts_b7 = false;
int32_t tby_out = 0;
bool dec_o0 = false;
bool dec_o1 = false;
bool dec_o2 = false;
bool dec_o3 = false;
int32_t enc_out = 0;
bool enc_any = false;
int32_t f2i_out = 0;
float i2f_out = 0.0f;
int32_t w2b_lo = 0;
int32_t w2b_hi = 0;
int32_t b2w_out = 0;
int32_t d2b_b0 = 0;
int32_t d2b_b1 = 0;
int32_t d2b_b2 = 0;
int32_t d2b_b3 = 0;
int32_t b2d_out = 0;
int32_t fnd_pos = 0;
bool fnd_found = false;
String chr_char = String();
int32_t chr_code = 0;
String addc_out = String();
float pid1_out = 0.0f;
bool and3_out = false;
bool xor3_out = false;
float mux16_out = 0.0f;
float dms16_o1 = 0.0f;
float dms16_o2 = 0.0f;
float dms16_o3 = 0.0f;
float dms16_o4 = 0.0f;
float dms16_o5 = 0.0f;
float dms16_o6 = 0.0f;
float dms16_o7 = 0.0f;
float dms16_o8 = 0.0f;
float dms16_o9 = 0.0f;
float dms16_o10 = 0.0f;
float dms16_o11 = 0.0f;
float dms16_o12 = 0.0f;
float dms16_o13 = 0.0f;
float dms16_o14 = 0.0f;
float dms16_o15 = 0.0f;
float dms16_o16 = 0.0f;
bool chg_out = false;
bool schg_out = false;
bool bchg_out = false;
bool rt_out = false;
float rnd_out = 0.0f;
bool tick_out = false;
float n_cmp_out = 0.0f;
float n_sr_out = 0.0f;
float n_bread_out = 0.0f;
float n_b0_out = 0.0f;
float n_b1_out = 0.0f;
float n_b2_out = 0.0f;
float n_dec_out = 0.0f;
float n_any_out = 0.0f;
float n_and3_out = 0.0f;
float n_xor3_out = 0.0f;
float n_chg_out = 0.0f;
float n_schg_out = 0.0f;
float n_bchg_out = 0.0f;
String t_cmp_out = String();
String t_sr_out = String();
String t_sin_out = String();
String t_cos_out = String();
String t_tan_out = String();
String t_band_out = String();
String t_bor_out = String();
String t_bxor_out = String();
String t_bnot_out = String();
String t_bread_out = String();
String t_bwrite_out = String();
String t_b0_out = String();
String t_b1_out = String();
String t_b2_out = String();
String t_tobyte_out = String();
String t_dec_out = String();
String t_enc_out = String();
String t_any_out = String();
String t_f2i_out = String();
String t_i2f_out = String();
String t_lo_out = String();
String t_hi_out = String();
String t_word_out = String();
String t_d0_out = String();
String t_d1_out = String();
String t_d2_out = String();
String t_d3_out = String();
String t_dword_out = String();
String t_find_out = String();
String t_code_out = String();
String t_and3_out = String();
String t_xor3_out = String();
String t_pid_out = String();
String t_mux16_out = String();
String t_dms16_out = String();
String t_chg_out = String();
String t_schg_out = String();
String t_bchg_out = String();
String t_rnd_out = String();
String line1_out = String();
bool tx1_sent = false;
String line2_out = String();
bool tx2_sent = false;
String line3_out = String();
bool tx3_sent = false;
String line4_out = String();
bool tx4_sent = false;
String line5_out = String();
bool tx5_sent = false;
String line6_out = String();
bool tx6_sent = false;
String line7_out = String();
bool tx7_sent = false;
String line8_out = String();
bool tx8_sent = false;
String line9_out = String();
bool tx9_sent = false;
String line10_out = String();
bool tx10_sent = false;

// pid1: pid
float pid1_i = 0.0f;
float pid1_prev_err = 0.0f;
unsigned long pid1_t = 0;
bool pid1_run = false;

// chg: number_change
float chg_prev = 0.0f;
bool chg_init = false;

// schg: string_change
String schg_prev;
bool schg_init = false;

// bchg: bit_change
bool bchg_prev = false;
bool bchg_init = false;

// rt: generator
unsigned long rt_start = 0;

// rnd: random
bool rnd_prev = false;

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

// tx5: uart_send
bool tx5_prev = false;

// tx6: uart_send
bool tx6_prev = false;

// tx7: uart_send
bool tx7_prev = false;

// tx8: uart_send
bool tx8_prev = false;

// tx9: uart_send
bool tx9_prev = false;

// tx10: uart_send
bool tx10_prev = false;

void setup() {
  // USB: 115200 бод
  Serial.begin(115200);

  // Память под строки заранее: меньше дробления кучи.
  chr_char.reserve(32);
  addc_out.reserve(32);
  t_cmp_out.reserve(32);
  t_sr_out.reserve(32);
  t_sin_out.reserve(32);
  t_cos_out.reserve(32);
  t_tan_out.reserve(32);
  t_band_out.reserve(32);
  t_bor_out.reserve(32);
  t_bxor_out.reserve(32);
  t_bnot_out.reserve(32);
  t_bread_out.reserve(32);
  t_bwrite_out.reserve(32);
  t_b0_out.reserve(32);
  t_b1_out.reserve(32);
  t_b2_out.reserve(32);
  t_tobyte_out.reserve(32);
  t_dec_out.reserve(32);
  t_enc_out.reserve(32);
  t_any_out.reserve(32);
  t_f2i_out.reserve(32);
  t_i2f_out.reserve(32);
  t_lo_out.reserve(32);
  t_hi_out.reserve(32);
  t_word_out.reserve(32);
  t_d0_out.reserve(32);
  t_d1_out.reserve(32);
  t_d2_out.reserve(32);
  t_d3_out.reserve(32);
  t_dword_out.reserve(32);
  t_find_out.reserve(32);
  t_code_out.reserve(32);
  t_and3_out.reserve(32);
  t_xor3_out.reserve(32);
  t_pid_out.reserve(32);
  t_mux16_out.reserve(32);
  t_dms16_out.reserve(32);
  t_chg_out.reserve(32);
  t_schg_out.reserve(32);
  t_bchg_out.reserve(32);
  t_rnd_out.reserve(32);
  line1_out.reserve(32);
  line2_out.reserve(32);
  line3_out.reserve(32);
  line4_out.reserve(32);
  line5_out.reserve(32);
  line6_out.reserve(32);
  line7_out.reserve(32);
  line8_out.reserve(32);
  line9_out.reserve(32);
  line10_out.reserve(32);

  // random: общий код
  randomSeed(esp_random());
}

void loop() {
  // cmp: compare
  cmp_out = 5.0f > 3.0f;

  // sr: sr_trigger
  if (true) {
    sr_out = true;
  } else if (true) {
    sr_out = false;
  }

  // sn: sin
  sn_out = sinf(30.0f * 0.017453292f);

  // cs: cos
  cs_out = cosf(60.0f * 0.017453292f);

  // tn: tan
  tn_out = tanf(45.0f * 0.017453292f);

  // band: bit_and
  band_out = 12 & 10;

  // bor: bit_or
  bor_out = 12 | 10;

  // bxor: bit_xor
  bxor_out = 12 ^ 10;

  // bnot: bit_not
  bnot_out = ~0;

  // bread: bit_read
  bread_out = ((5 >> 2) & 1) != 0;

  // bwrite: bit_write
  bwrite_out = true ? (0 | (1 << 3)) : (0 & ~(1 << 3));

  // bts: byte_to_bits
  bts_b0 = ((5 >> 0) & 1) != 0;
  bts_b1 = ((5 >> 1) & 1) != 0;
  bts_b2 = ((5 >> 2) & 1) != 0;
  bts_b3 = ((5 >> 3) & 1) != 0;
  bts_b4 = ((5 >> 4) & 1) != 0;
  bts_b5 = ((5 >> 5) & 1) != 0;
  bts_b6 = ((5 >> 6) & 1) != 0;
  bts_b7 = ((5 >> 7) & 1) != 0;

  // tby: bits_to_byte
  tby_out = (true ? 1 : 0) | (false ? 2 : 0) | (true ? 4 : 0) | (false ? 8 : 0) | (false ? 16 : 0) | (false ? 32 : 0) | (false ? 64 : 0) | (true ? 128 : 0);

  // dec: decoder
  dec_o0 = 2 == 0;dec_o1 = 2 == 1;dec_o2 = 2 == 2;dec_o3 = 2 == 3;

  // enc: encoder
  enc_out = 0;
  enc_any = false;
  if (true) { enc_out = 3; enc_any = true; }if (false) { enc_out = 2; enc_any = true; }if (true) { enc_out = 1; enc_any = true; }if (false) { enc_out = 0; enc_any = true; }

  // f2i: float_to_int
  f2i_out = (int32_t)roundf(2.5f);

  // i2f: int_to_float
  i2f_out = (float)7;

  // w2b: word_to_bytes
  w2b_lo = 4660 & 0xFF;
  w2b_hi = (4660 >> 8) & 0xFF;

  // b2w: bytes_to_word
  b2w_out = ((18 & 0xFF) << 8) | (52 & 0xFF);

  // d2b: dword_to_bytes
  d2b_b0 = ((uint32_t)16909060 >> 0) & 0xFF;
  d2b_b1 = ((uint32_t)16909060 >> 8) & 0xFF;
  d2b_b2 = ((uint32_t)16909060 >> 16) & 0xFF;
  d2b_b3 = ((uint32_t)16909060 >> 24) & 0xFF;

  // b2d: bytes_to_dword
  b2d_out = (int32_t)(((uint32_t)(1 & 0xFF) << 24) | ((uint32_t)(2 & 0xFF) << 16) | ((uint32_t)(3 & 0xFF) << 8) | (uint32_t)(4 & 0xFF));

  // fnd: text_find
  fnd_pos = String("hello world").indexOf(String("wor"), 0);
  fnd_found = fnd_pos >= 0;

  // chr: text_char
  if (1 >= 0 && 1 < (int)String("abc").length()) {
    chr_code = (uint8_t)String("abc")[1];
    chr_char = String("abc").substring(1, 1 + 1);
  } else {
    chr_code = 0;
    chr_char = "";
  }

  // addc: text_add_char
  addc_out = String("ab");
  addc_out += (char)(99 & 0xFF);

  // pid1: pid
  if (true) {
    unsigned long now = millis();
    float err = 10.0f - 0.0f;
    if (pid1_run) {
      float dt = (now - pid1_t) / 1000.0f;
      if (dt > 0.0f) {
        pid1_i += err * dt * 0.0f;
        pid1_i = constrain(pid1_i, 0.0f, 100.0f);
        float d = (err - pid1_prev_err) / dt;
        pid1_out = constrain(2.0f * err + pid1_i + 0.0f * d, 0.0f, 100.0f);
        pid1_prev_err = err;
        pid1_t = now;
      }
    } else {
      pid1_run = true;
      pid1_t = now;
      pid1_prev_err = err;
      pid1_out = constrain(2.0f * err, 0.0f, 100.0f);
    }
  } else {
    pid1_run = false;
    pid1_i = 0.0f;
    pid1_out = 0.0f;
  }

  // and3: and
  and3_out = true && true && true;

  // xor3: xor
  xor3_out = true != true != true;

  // mux16: mux
  {
    int idx = (int)15.0f;
    if (idx <= 0) {
      mux16_out = 0.0f;
    } else
    if (idx <= 1) {
      mux16_out = 0.0f;
    } else
    if (idx <= 2) {
      mux16_out = 0.0f;
    } else
    if (idx <= 3) {
      mux16_out = 0.0f;
    } else
    if (idx <= 4) {
      mux16_out = 0.0f;
    } else
    if (idx <= 5) {
      mux16_out = 0.0f;
    } else
    if (idx <= 6) {
      mux16_out = 0.0f;
    } else
    if (idx <= 7) {
      mux16_out = 0.0f;
    } else
    if (idx <= 8) {
      mux16_out = 0.0f;
    } else
    if (idx <= 9) {
      mux16_out = 0.0f;
    } else
    if (idx <= 10) {
      mux16_out = 0.0f;
    } else
    if (idx <= 11) {
      mux16_out = 0.0f;
    } else
    if (idx <= 12) {
      mux16_out = 0.0f;
    } else
    if (idx <= 13) {
      mux16_out = 0.0f;
    } else
    if (idx <= 14) {
      mux16_out = 0.0f;
    } else
    mux16_out = 99.0f;
  }

  // dms16: dms
  {
    int idx = (int)15.0f;
    dms16_o1 = idx == 0 ? 5.0f : 0.0f;
    dms16_o2 = idx == 1 ? 5.0f : 0.0f;
    dms16_o3 = idx == 2 ? 5.0f : 0.0f;
    dms16_o4 = idx == 3 ? 5.0f : 0.0f;
    dms16_o5 = idx == 4 ? 5.0f : 0.0f;
    dms16_o6 = idx == 5 ? 5.0f : 0.0f;
    dms16_o7 = idx == 6 ? 5.0f : 0.0f;
    dms16_o8 = idx == 7 ? 5.0f : 0.0f;
    dms16_o9 = idx == 8 ? 5.0f : 0.0f;
    dms16_o10 = idx == 9 ? 5.0f : 0.0f;
    dms16_o11 = idx == 10 ? 5.0f : 0.0f;
    dms16_o12 = idx == 11 ? 5.0f : 0.0f;
    dms16_o13 = idx == 12 ? 5.0f : 0.0f;
    dms16_o14 = idx == 13 ? 5.0f : 0.0f;
    dms16_o15 = idx == 14 ? 5.0f : 0.0f;
    dms16_o16 = idx == 15 ? 5.0f : 0.0f;
  }

  // chg: number_change
  if (!chg_init) {
    chg_init = true;
    chg_prev = 1.0f;
    chg_out = false;
  } else {
    float diff = 1.0f - chg_prev;
    chg_out = (diff < 0 ? -diff : diff) > 0.0f;
    if (chg_out) chg_prev = 1.0f;
  }

  // schg: string_change
  schg_out = schg_init && String("x") != schg_prev;
  schg_prev = String("x");
  schg_init = true;

  // bchg: bit_change
  {
    bool now = ((2 >> 1) & 1) != 0;
    bchg_out = bchg_init && now != bchg_prev;
    bchg_prev = now;
    bchg_init = true;
  }

  // rt: generator
  if (true) {
    unsigned long phase = (millis() - rt_start) % (600UL);
    rt_out = phase < 300UL;
  } else {
    rt_start = millis();
    rt_out = false;
  }

  // rnd: random
  if (rt_out && !rnd_prev) {
    rnd_out = 10.0f + (20.0f - 10.0f) * (random(0, 10001) / 10000.0f);
  }
  rnd_prev = rt_out;

  // tick: generator
  if (true) {
    unsigned long phase = (millis() - tick_start) % (1000UL);
    tick_out = phase < 100UL;
  } else {
    tick_start = millis();
    tick_out = false;
  }

  // n_cmp: switch
  n_cmp_out = cmp_out ? 1.0f : 0.0f;

  // n_sr: switch
  n_sr_out = sr_out ? 1.0f : 0.0f;

  // n_bread: switch
  n_bread_out = bread_out ? 1.0f : 0.0f;

  // n_b0: switch
  n_b0_out = bts_b0 ? 1.0f : 0.0f;

  // n_b1: switch
  n_b1_out = bts_b1 ? 1.0f : 0.0f;

  // n_b2: switch
  n_b2_out = bts_b2 ? 1.0f : 0.0f;

  // n_dec: switch
  n_dec_out = dec_o2 ? 1.0f : 0.0f;

  // n_any: switch
  n_any_out = enc_any ? 1.0f : 0.0f;

  // n_and3: switch
  n_and3_out = and3_out ? 1.0f : 0.0f;

  // n_xor3: switch
  n_xor3_out = xor3_out ? 1.0f : 0.0f;

  // n_chg: switch
  n_chg_out = chg_out ? 1.0f : 0.0f;

  // n_schg: switch
  n_schg_out = schg_out ? 1.0f : 0.0f;

  // n_bchg: switch
  n_bchg_out = bchg_out ? 1.0f : 0.0f;

  // t_cmp: num_to_text
  t_cmp_out = "cmp=";
  t_cmp_out += String(n_cmp_out, 1);
  t_cmp_out += "";

  // t_sr: num_to_text
  t_sr_out = "sr=";
  t_sr_out += String(n_sr_out, 1);
  t_sr_out += "";

  // t_sin: num_to_text
  t_sin_out = "sin=";
  t_sin_out += String(sn_out, 1);
  t_sin_out += "";

  // t_cos: num_to_text
  t_cos_out = "cos=";
  t_cos_out += String(cs_out, 1);
  t_cos_out += "";

  // t_tan: num_to_text
  t_tan_out = "tan=";
  t_tan_out += String(tn_out, 1);
  t_tan_out += "";

  // t_band: num_to_text
  t_band_out = "band=";
  t_band_out += String((float)band_out, 1);
  t_band_out += "";

  // t_bor: num_to_text
  t_bor_out = "bor=";
  t_bor_out += String((float)bor_out, 1);
  t_bor_out += "";

  // t_bxor: num_to_text
  t_bxor_out = "bxor=";
  t_bxor_out += String((float)bxor_out, 1);
  t_bxor_out += "";

  // t_bnot: num_to_text
  t_bnot_out = "bnot=";
  t_bnot_out += String((float)bnot_out, 1);
  t_bnot_out += "";

  // t_bread: num_to_text
  t_bread_out = "bread=";
  t_bread_out += String(n_bread_out, 1);
  t_bread_out += "";

  // t_bwrite: num_to_text
  t_bwrite_out = "bwrite=";
  t_bwrite_out += String((float)bwrite_out, 1);
  t_bwrite_out += "";

  // t_b0: num_to_text
  t_b0_out = "b0=";
  t_b0_out += String(n_b0_out, 1);
  t_b0_out += "";

  // t_b1: num_to_text
  t_b1_out = "b1=";
  t_b1_out += String(n_b1_out, 1);
  t_b1_out += "";

  // t_b2: num_to_text
  t_b2_out = "b2=";
  t_b2_out += String(n_b2_out, 1);
  t_b2_out += "";

  // t_tobyte: num_to_text
  t_tobyte_out = "tobyte=";
  t_tobyte_out += String((float)tby_out, 1);
  t_tobyte_out += "";

  // t_dec: num_to_text
  t_dec_out = "dec=";
  t_dec_out += String(n_dec_out, 1);
  t_dec_out += "";

  // t_enc: num_to_text
  t_enc_out = "enc=";
  t_enc_out += String((float)enc_out, 1);
  t_enc_out += "";

  // t_any: num_to_text
  t_any_out = "any=";
  t_any_out += String(n_any_out, 1);
  t_any_out += "";

  // t_f2i: num_to_text
  t_f2i_out = "f2i=";
  t_f2i_out += String((float)f2i_out, 1);
  t_f2i_out += "";

  // t_i2f: num_to_text
  t_i2f_out = "i2f=";
  t_i2f_out += String(i2f_out, 1);
  t_i2f_out += "";

  // t_lo: num_to_text
  t_lo_out = "lo=";
  t_lo_out += String((float)w2b_lo, 1);
  t_lo_out += "";

  // t_hi: num_to_text
  t_hi_out = "hi=";
  t_hi_out += String((float)w2b_hi, 1);
  t_hi_out += "";

  // t_word: num_to_text
  t_word_out = "word=";
  t_word_out += String((float)b2w_out, 1);
  t_word_out += "";

  // t_d0: num_to_text
  t_d0_out = "d0=";
  t_d0_out += String((float)d2b_b0, 1);
  t_d0_out += "";

  // t_d1: num_to_text
  t_d1_out = "d1=";
  t_d1_out += String((float)d2b_b1, 1);
  t_d1_out += "";

  // t_d2: num_to_text
  t_d2_out = "d2=";
  t_d2_out += String((float)d2b_b2, 1);
  t_d2_out += "";

  // t_d3: num_to_text
  t_d3_out = "d3=";
  t_d3_out += String((float)d2b_b3, 1);
  t_d3_out += "";

  // t_dword: num_to_text
  t_dword_out = "dword=";
  t_dword_out += String((float)b2d_out, 1);
  t_dword_out += "";

  // t_find: num_to_text
  t_find_out = "find=";
  t_find_out += String((float)fnd_pos, 1);
  t_find_out += "";

  // t_code: num_to_text
  t_code_out = "code=";
  t_code_out += String((float)chr_code, 1);
  t_code_out += "";

  // t_and3: num_to_text
  t_and3_out = "and3=";
  t_and3_out += String(n_and3_out, 1);
  t_and3_out += "";

  // t_xor3: num_to_text
  t_xor3_out = "xor3=";
  t_xor3_out += String(n_xor3_out, 1);
  t_xor3_out += "";

  // t_pid: num_to_text
  t_pid_out = "pid=";
  t_pid_out += String(pid1_out, 1);
  t_pid_out += "";

  // t_mux16: num_to_text
  t_mux16_out = "mux16=";
  t_mux16_out += String(mux16_out, 1);
  t_mux16_out += "";

  // t_dms16: num_to_text
  t_dms16_out = "dms16=";
  t_dms16_out += String(dms16_o16, 1);
  t_dms16_out += "";

  // t_chg: num_to_text
  t_chg_out = "chg=";
  t_chg_out += String(n_chg_out, 1);
  t_chg_out += "";

  // t_schg: num_to_text
  t_schg_out = "schg=";
  t_schg_out += String(n_schg_out, 1);
  t_schg_out += "";

  // t_bchg: num_to_text
  t_bchg_out = "bchg=";
  t_bchg_out += String(n_bchg_out, 1);
  t_bchg_out += "";

  // t_rnd: num_to_text
  t_rnd_out = "rnd=";
  t_rnd_out += String(rnd_out, 1);
  t_rnd_out += "";

  // line1: text_concat
  line1_out = t_cmp_out;
  line1_out += " ";
  line1_out += t_sr_out;
  line1_out += " ";
  line1_out += t_sin_out;
  line1_out += " ";
  line1_out += t_cos_out;

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
  line2_out = t_tan_out;
  line2_out += " ";
  line2_out += t_band_out;
  line2_out += " ";
  line2_out += t_bor_out;
  line2_out += " ";
  line2_out += t_bxor_out;

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
  line3_out = t_bnot_out;
  line3_out += " ";
  line3_out += t_bread_out;
  line3_out += " ";
  line3_out += t_bwrite_out;
  line3_out += " ";
  line3_out += t_b0_out;

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
  line4_out = t_b1_out;
  line4_out += " ";
  line4_out += t_b2_out;
  line4_out += " ";
  line4_out += t_tobyte_out;
  line4_out += " ";
  line4_out += t_dec_out;

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

  // line5: text_concat
  line5_out = t_enc_out;
  line5_out += " ";
  line5_out += t_any_out;
  line5_out += " ";
  line5_out += t_f2i_out;
  line5_out += " ";
  line5_out += t_i2f_out;

  // tx5: uart_send
  {
    bool fire = tick_out && !tx5_prev;
    tx5_prev = tick_out;
    tx5_sent = false;
    if (fire) {
      const String& text = line5_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx5_sent = true;
      }
    }
  }

  // line6: text_concat
  line6_out = t_lo_out;
  line6_out += " ";
  line6_out += t_hi_out;
  line6_out += " ";
  line6_out += t_word_out;
  line6_out += " ";
  line6_out += t_d0_out;

  // tx6: uart_send
  {
    bool fire = tick_out && !tx6_prev;
    tx6_prev = tick_out;
    tx6_sent = false;
    if (fire) {
      const String& text = line6_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx6_sent = true;
      }
    }
  }

  // line7: text_concat
  line7_out = t_d1_out;
  line7_out += " ";
  line7_out += t_d2_out;
  line7_out += " ";
  line7_out += t_d3_out;
  line7_out += " ";
  line7_out += t_dword_out;

  // tx7: uart_send
  {
    bool fire = tick_out && !tx7_prev;
    tx7_prev = tick_out;
    tx7_sent = false;
    if (fire) {
      const String& text = line7_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx7_sent = true;
      }
    }
  }

  // line8: text_concat
  line8_out = t_find_out;
  line8_out += " ";
  line8_out += t_code_out;
  line8_out += " ";
  line8_out += t_and3_out;
  line8_out += " ";
  line8_out += t_xor3_out;

  // tx8: uart_send
  {
    bool fire = tick_out && !tx8_prev;
    tx8_prev = tick_out;
    tx8_sent = false;
    if (fire) {
      const String& text = line8_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx8_sent = true;
      }
    }
  }

  // line9: text_concat
  line9_out = t_pid_out;
  line9_out += " ";
  line9_out += t_mux16_out;
  line9_out += " ";
  line9_out += t_dms16_out;
  line9_out += " ";
  line9_out += t_chg_out;

  // tx9: uart_send
  {
    bool fire = tick_out && !tx9_prev;
    tx9_prev = tick_out;
    tx9_sent = false;
    if (fire) {
      const String& text = line9_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx9_sent = true;
      }
    }
  }

  // line10: text_concat
  line10_out = t_schg_out;
  line10_out += " ";
  line10_out += t_bchg_out;
  line10_out += " ";
  line10_out += t_rnd_out;
  line10_out += " ";
  line10_out += String("");

  // tx10: uart_send
  {
    bool fire = tick_out && !tx10_prev;
    tx10_prev = tick_out;
    tx10_sent = false;
    if (fire) {
      const String& text = line10_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx10_sent = true;
      }
    }
  }
}
