// Prototype master firmware: wasm3 modules from LittleFS, one FreeRTOS task per module,
// watchdog, journal and hot swap over Serial. See specs/wasm-on-mcu/plan.md.
//
// Serial protocol (115200, lines end with \n):
//   put <name> <len> <crc32 hex> <allow,list|->   then <len> raw bytes -> stores and (re)starts the module
//   in|param <name> <i> <i|f> <value>  set an input / parameter; the letter is the type (i = int/bool, f = float)
//   reset <name>                     restart the module paused, inputs and outputs zero
//   tick <name> <n> <dt_ms>          pause the module and run n steps now
//   force <name> <idx> <i|f> <val>   show <val> as output idx of a module built with watch (idx < 128)
//   unforce <name> <idx|all>         release forced outputs
//   out <name> <n>                   first n (<=128) outputs as 32-bit words in hex (8 digits each)
// Values are 4-byte words (ABI 3): the bits of an i32 (bool, int) or of an f32 (float).
//   swap <name>      reload the module from LittleFS      list      stats      log      del <name>
#include <LittleFS.h>
#include <esp_crc.h>
// Drivers are compiled in only when the project needs them (FLP_SENSORS, FLP_PCF defined by the editor).
#ifdef FLP_PCF
#include <Wire.h>
#endif
#ifdef FLP_SENSORS
#include <DHT.h>
#include <DallasTemperature.h>
#include <OneWire.h>
#endif
#include <wasm3.h>
#include <m3_env.h>

static const int kMaxSlots = 4;
static const int kMaxOutputs = 128;  // watched signals per module
// Modules and the watchdog share one core: core 1 where there are two, core 0 on single-core chips (C3).
static const int kCore = portNUM_PROCESSORS > 1 ? 1 : 0;
static const int kStackWords = 2048;
// The wasm3 compiler recurses deeply on a big module (300 blocks overflowed 6 KB), so the task needs room.
static const int kTaskStackBytes = 16384;
static const uint32_t kStepPeriodMs = 10;
static const uint32_t kStepLimitMs = 100;  // watchdog: a longer step means a hang
static const int kJournal = 50;

enum { P_GPIO_READ = 1, P_GPIO_WRITE = 2, P_ADC = 4, P_PWM = 8, P_MILLIS = 16, P_SENSOR = 32,
       P_PCF_READ = 64, P_PCF_WRITE = 128 };

#if defined(FLP_SENSORS) || defined(FLP_PCF)
#define FLP_HWTASK 1
#endif

#ifdef FLP_SENSORS
// ---- sensors: one task polls them (a read blocks for milliseconds), modules take the last values ----
enum { SENSOR_DS18B20 = 0, SENSOR_DHT22 = 1 };
struct Sensor {
  volatile bool used = false;
  int32_t key = 0;
  uint8_t kind = 0, pin = 0, index = 0;
  uint32_t period = 1000;
  volatile float temp = 0, hum = 0;
  volatile bool ok = false;
  // driver state, touched by sensorTask only
  bool ready = false;
  uint32_t last = 0;
  OneWire *wire = nullptr;
  DallasTemperature *bus = nullptr;
  DHT *dht = nullptr;
};
static const int kMaxSensors = 8;
static Sensor g_sensors[kMaxSensors];
#endif
#ifdef FLP_HWTASK
static SemaphoreHandle_t g_sensorLock;
#endif

#ifdef FLP_PCF

// ---- PCF8574 expanders on I2C: the same task talks to them, modules take the last byte / hand over a new one ----
struct Pcf {
  volatile bool used = false;
  int32_t key = 0;       // address | sda << 8 | scl << 16 (a pin is one byte, 0xFF = board default)
  bool writer = false;   // a chip is either read or written by the modules
  uint32_t period = 20;
  volatile uint8_t value = 0xFF;  // last byte read / byte to write
  volatile bool ok = false;
  // task state
  bool ready = false;
  int16_t sent = -1;     // last byte written, -1 = none yet
  uint32_t last = 0;
};
static const int kMaxPcf = 8;
static Pcf g_pcf[kMaxPcf];
static bool g_wireStarted = false;
#endif

struct Msg { char topic[24]; uint8_t data[64]; uint8_t len; };

struct Slot {
  bool used = false;
  char name[16] = "";
  uint8_t perms = 0;
  uint8_t *wasm = nullptr;  // wasm3 keeps pointing into these bytes
  size_t wasmLen = 0;
  IM3Environment env = nullptr;
  IM3Runtime rt = nullptr;
  IM3Function step = nullptr, alloc = nullptr, onMessage = nullptr;
  TaskHandle_t task = nullptr;
  volatile bool inStep = false, needRestart = false, paused = false;
  volatile uint32_t stepStart = 0, steps = 0, restarts = 0;
  char subs[4][24];
  int nSubs = 0;
  Msg queue[4];
  volatile int qHead = 0, qTail = 0;
  uint32_t outputs[kMaxOutputs], inputs[8], params[8];  // bits of i32 / f32
  uint32_t forceVal[kMaxOutputs];                         // value shown instead of the module's own
  bool forced[kMaxOutputs];
};

static Slot g_slots[kMaxSlots];
static SemaphoreHandle_t g_mgmt;
static uint32_t g_tags[64];  // bits of i32 / f32

static inline uint32_t bitsOf(float v) { uint32_t u; memcpy(&u, &v, 4); return u; }
static inline float floatOf(uint32_t u) { float v; memcpy(&v, &u, 4); return v; }
static inline int32_t intOf(uint32_t u) { return (int32_t)u; }
static portMUX_TYPE g_mux = portMUX_INITIALIZER_UNLOCKED;

struct Entry { uint32_t ms; char text[96]; };
static Entry g_journal[kJournal];
static int g_jCount = 0;

static void jlog(const char *name, const char *fmt, ...) {
  char buf[80];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(buf, sizeof buf, fmt, ap);
  va_end(ap);
  portENTER_CRITICAL(&g_mux);
  Entry &e = g_journal[g_jCount++ % kJournal];
  e.ms = millis();
  snprintf(e.text, sizeof e.text, "[%s] %s", name, buf);
  portEXIT_CRITICAL(&g_mux);
}

// ---- host functions (userdata = Slot*) ----
static Slot *slotOf(IM3ImportContext c) { return (Slot *)c->userdata; }
static bool inMem(IM3Runtime rt, int32_t ptr, int32_t len) {
  return ptr >= 0 && len >= 0 && (uint64_t)ptr + len <= rt->memory.mallocated->length;
}

// Typed access to the slots: the module asks for `_i` or `_f`, the bits stay as they are.
#define FLP_SLOT_GET(fn, field, T, conv)   m3ApiRawFunction(fn) {     m3ApiReturnType(T);     m3ApiGetArg(int32_t, i);     m3ApiReturn((i >= 0 && i < 8) ? conv(slotOf(_ctx)->field[i]) : (T)0);   }
FLP_SLOT_GET(h_input_i, inputs, int32_t, intOf)
FLP_SLOT_GET(h_input_f, inputs, float, floatOf)
FLP_SLOT_GET(h_param_i, params, int32_t, intOf)
FLP_SLOT_GET(h_param_f, params, float, floatOf)
// The module reports the output and gets back the value to use: the forced one while forced.
m3ApiRawFunction(h_output_i) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, i);
  m3ApiGetArg(int32_t, v);
  if (i >= 0 && i < kMaxOutputs) {
    Slot *s = slotOf(_ctx);
    if (s->forced[i]) v = intOf(s->forceVal[i]);
    s->outputs[i] = (uint32_t)v;
  }
  m3ApiReturn(v);
}
m3ApiRawFunction(h_output_f) {
  m3ApiReturnType(float);
  m3ApiGetArg(int32_t, i);
  m3ApiGetArg(float, v);
  if (i >= 0 && i < kMaxOutputs) {
    Slot *s = slotOf(_ctx);
    if (s->forced[i]) v = floatOf(s->forceVal[i]);
    s->outputs[i] = bitsOf(v);
  }
  m3ApiReturn(v);
}
m3ApiRawFunction(h_log) {
  m3ApiGetArg(int32_t, p);
  m3ApiGetArg(int32_t, l);
  if (inMem(runtime, p, l)) {
    char t[64];
    int n = l < 63 ? l : 63;
    memcpy(t, (uint8_t *)_mem + p, n);
    t[n] = 0;
    jlog(slotOf(_ctx)->name, "%s", t);
  }
  m3ApiSuccess();
}
m3ApiRawFunction(h_tag_get_i) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, id);
  m3ApiReturn((id >= 0 && id < 64) ? (int32_t)g_tags[id] : 0);
}
m3ApiRawFunction(h_tag_get_f) {
  m3ApiReturnType(float);
  m3ApiGetArg(int32_t, id);
  m3ApiReturn((id >= 0 && id < 64) ? floatOf(g_tags[id]) : 0.0f);
}
m3ApiRawFunction(h_tag_set_i) {
  m3ApiGetArg(int32_t, id);
  m3ApiGetArg(int32_t, v);
  if (id >= 0 && id < 64) g_tags[id] = (uint32_t)v;
  m3ApiSuccess();
}
m3ApiRawFunction(h_tag_set_f) {
  m3ApiGetArg(int32_t, id);
  m3ApiGetArg(float, v);
  if (id >= 0 && id < 64) g_tags[id] = bitsOf(v);
  m3ApiSuccess();
}
m3ApiRawFunction(h_subscribe) {
  m3ApiGetArg(int32_t, p);
  m3ApiGetArg(int32_t, l);
  Slot *s = slotOf(_ctx);
  if (inMem(runtime, p, l) && l > 0 && l < 24 && s->nSubs < 4) {
    memcpy(s->subs[s->nSubs], (uint8_t *)_mem + p, l);
    s->subs[s->nSubs][l] = 0;
    s->nSubs++;
  }
  m3ApiSuccess();
}
m3ApiRawFunction(h_publish) {
  m3ApiGetArg(int32_t, tp);
  m3ApiGetArg(int32_t, tl);
  m3ApiGetArg(int32_t, p);
  m3ApiGetArg(int32_t, l);
  if (inMem(runtime, tp, tl) && inMem(runtime, p, l) && tl > 0 && tl < 24 && l <= 64) {
    char topic[24];
    memcpy(topic, (uint8_t *)_mem + tp, tl);
    topic[tl] = 0;
    for (auto &d : g_slots) {
      if (!d.used || !d.task) continue;
      for (int i = 0; i < d.nSubs; i++) {
        if (strcmp(d.subs[i], topic)) continue;
        portENTER_CRITICAL(&g_mux);
        int next = (d.qTail + 1) % 4;
        if (next != d.qHead) {
          Msg &m = d.queue[d.qTail];
          strcpy(m.topic, topic);
          memcpy(m.data, (uint8_t *)_mem + p, l);
          m.len = l;
          d.qTail = next;
        }
        portEXIT_CRITICAL(&g_mux);
      }
    }
  }
  m3ApiSuccess();
}
// Pin modes are set on first use (modules cannot call pinMode): one bit per GPIO.
static uint64_t g_pinInit = 0;
static void pinOnce(int32_t pin, uint8_t mode) {
  if (pin < 0 || pin > 63 || (g_pinInit >> pin & 1)) return;
  g_pinInit |= 1ULL << pin;
  pinMode(pin, mode);
}
m3ApiRawFunction(h_gpio_read) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, pin);
  // Bit 8 of the pin number asks for the internal pull-up.
  bool pullup = pin & 0x100;
  pin &= 0xFF;
  pinOnce(pin, pullup ? INPUT_PULLUP : INPUT);
  m3ApiReturn(digitalRead(pin));
}
m3ApiRawFunction(h_gpio_write) {
  m3ApiGetArg(int32_t, pin);
  m3ApiGetArg(int32_t, v);
  pinOnce(pin, OUTPUT);
  digitalWrite(pin, v);
  m3ApiSuccess();
}
m3ApiRawFunction(h_adc_read) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, pin);
  m3ApiReturn(analogRead(pin));
}
m3ApiRawFunction(h_pwm_write) {
  m3ApiGetArg(int32_t, pin);
  m3ApiGetArg(int32_t, v);  // 8 bits, 0..255
  analogWrite(pin, v);
  m3ApiSuccess();
}
m3ApiRawFunction(h_millis) {
  m3ApiReturnType(int32_t);
  m3ApiReturn((int32_t)millis());
}

#ifdef FLP_SENSORS
// ---- sensor access ----
// key = kind | index << 4 | pin << 8
static Sensor *sensorFor(int32_t key, uint32_t period) {
  Sensor *found = nullptr;
  xSemaphoreTake(g_sensorLock, portMAX_DELAY);
  for (auto &s : g_sensors) {
    if (s.used && s.key == key) { found = &s; break; }
  }
  if (!found) {
    for (auto &s : g_sensors) {
      if (s.used) continue;
      s.key = key;
      s.kind = key & 15;
      s.index = key >> 4 & 15;
      s.pin = key >> 8 & 0xFF;
      s.period = period < 100 ? 100 : period;
      s.used = true;  // last: the task sees a complete entry
      found = &s;
      break;
    }
  }
  xSemaphoreGive(g_sensorLock);
  return found;
}

static void sensorPoll(Sensor &s, uint32_t now) {
  if (s.kind == SENSOR_DS18B20) {
    if (!s.ready) {
      s.wire = new OneWire(s.pin);
      s.bus = new DallasTemperature(s.wire);
      s.bus->begin();
      s.bus->setWaitForConversion(false);  // do not block for 750 ms: request now, read next period
      s.bus->requestTemperatures();
      s.last = now;
      s.ready = true;
    } else if (now - s.last >= s.period) {
      float t = s.bus->getTempCByIndex(s.index);
      s.ok = t != DEVICE_DISCONNECTED_C;
      if (s.ok) s.temp = t;
      s.bus->requestTemperatures();
      s.last = now;
    }
  } else {
    if (!s.ready) {
      s.dht = new DHT(s.pin, DHT22);
      s.dht->begin();
      s.last = now - s.period;  // the first read is due at once
      s.ready = true;
    }
    if (now - s.last >= s.period) {
      s.last = now;
      float t = s.dht->readTemperature(), h = s.dht->readHumidity();
      s.ok = !isnan(t) && !isnan(h);
      if (s.ok) { s.temp = t; s.hum = h; }
    }
  }
}

#endif
#ifdef FLP_PCF
static void pcfPoll(Pcf &c, uint32_t now);
#endif

#ifdef FLP_HWTASK
static void hwTask(void *) {
  for (;;) {
    uint32_t now = millis();
#ifdef FLP_SENSORS
    for (auto &s : g_sensors) {
      if (s.used) sensorPoll(s, now);
    }
#endif
#ifdef FLP_PCF
    for (auto &c : g_pcf) {
      if (c.used) pcfPoll(c, now);
    }
#endif
    vTaskDelay(pdMS_TO_TICKS(20));
  }
}
#endif

#ifdef FLP_SENSORS
m3ApiRawFunction(h_sensor_read) {
  m3ApiReturnType(float);
  m3ApiGetArg(int32_t, key);
  m3ApiGetArg(int32_t, period);
  m3ApiGetArg(int32_t, field);  // 0 temperature, 1 humidity, 2 last read ok (1 or 0)
  Sensor *s = sensorFor(key, period < 0 ? 0 : period);
  float v = 0;
  if (s) v = field == 0 ? s->temp : field == 1 ? s->hum : (s->ok ? 1.0f : 0.0f);
  m3ApiReturn(v);
}
#endif

#ifdef FLP_PCF
// ---- PCF8574 access ----
static Pcf *pcfFor(int32_t key, bool writer, uint32_t period, uint8_t first) {
  Pcf *found = nullptr;
  xSemaphoreTake(g_sensorLock, portMAX_DELAY);
  for (auto &c : g_pcf) {
    if (c.used && c.key == key) { found = &c; break; }
  }
  if (!found) {
    for (auto &c : g_pcf) {
      if (c.used) continue;
      c.key = key;
      c.writer = writer;
      c.value = first;  // a writer's first byte goes out as the module wants it, not as 0xFF
      c.period = period < 5 ? 5 : period;
      c.used = true;  // last: the task sees a complete entry
      found = &c;
      break;
    }
  }
  xSemaphoreGive(g_sensorLock);
  return found;
}

static void pcfPoll(Pcf &c, uint32_t now) {
  uint8_t addr = c.key & 0x7F;
  if (!g_wireStarted) {
    int sda = c.key >> 8 & 0xFF, scl = c.key >> 16 & 0xFF;
    if (sda == 0xFF || scl == 0xFF) Wire.begin(); else Wire.begin(sda, scl);
    g_wireStarted = true;
  }
  if (!c.ready) {
    // A pin reads as an input while the chip holds it high.
    Wire.beginTransmission(addr);
    Wire.write(c.writer ? c.value : 0xFF);
    c.ok = Wire.endTransmission() == 0;
    if (c.writer && c.ok) c.sent = c.value;
    c.last = now;
    c.ready = true;
    return;
  }
  if (c.writer) {
    // Write on change; while the chip does not answer, retry twice a second.
    uint8_t want = c.value;
    if (c.ok ? want != c.sent : now - c.last >= 500) {
      Wire.beginTransmission(addr);
      Wire.write(want);
      c.ok = Wire.endTransmission() == 0;
      c.last = now;
      if (c.ok) c.sent = want;
    }
  } else if (now - c.last >= c.period) {
    c.last = now;
    if (Wire.requestFrom(addr, (uint8_t)1) == 1) { c.value = Wire.read(); c.ok = true; }
    else c.ok = false;
  }
}

m3ApiRawFunction(h_pcf_read) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, key);
  m3ApiGetArg(int32_t, period);
  m3ApiGetArg(int32_t, field);  // 0 last byte read, 1 chip answered (1 or 0)
  Pcf *c = pcfFor(key, false, period < 0 ? 0 : period, 0xFF);
  int32_t v = 0;
  if (c) v = field == 0 ? c->value : (c->ok ? 1 : 0);
  m3ApiReturn(v);
}

m3ApiRawFunction(h_pcf_write) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, key);
  m3ApiGetArg(int32_t, value);
  Pcf *c = pcfFor(key, true, 0, value & 0xFF);
  if (c) c->value = value & 0xFF;
  m3ApiReturn(c && c->ok ? 1 : 0);  // whether the chip answered the last write
}
#endif

static void linkAll(Slot &s, IM3Module m) {
  auto L = [&](const char *n, const char *sig, M3RawCall f) { m3_LinkRawFunctionEx(m, "flp", n, sig, f, &s); };
  L("input_i", "i(i)", h_input_i);
  L("input_f", "f(i)", h_input_f);
  L("param_i", "i(i)", h_param_i);
  L("param_f", "f(i)", h_param_f);
  L("output_i", "i(ii)", h_output_i);
  L("output_f", "f(if)", h_output_f);
  L("log", "v(ii)", h_log);
  L("tag_get_i", "i(i)", h_tag_get_i);
  L("tag_get_f", "f(i)", h_tag_get_f);
  L("tag_set_i", "v(ii)", h_tag_set_i);
  L("tag_set_f", "v(if)", h_tag_set_f);
  L("publish", "v(iiii)", h_publish);
  L("subscribe", "v(ii)", h_subscribe);
  if (s.perms & P_GPIO_READ) L("gpio_read", "i(i)", h_gpio_read);
  if (s.perms & P_GPIO_WRITE) L("gpio_write", "v(ii)", h_gpio_write);
  if (s.perms & P_ADC) L("adc_read", "i(i)", h_adc_read);
  if (s.perms & P_PWM) L("pwm_write", "v(ii)", h_pwm_write);
  if (s.perms & P_MILLIS) L("millis", "i()", h_millis);
#ifdef FLP_SENSORS
  if (s.perms & P_SENSOR) L("sensor_read", "f(iii)", h_sensor_read);
#endif
#ifdef FLP_PCF
  if (s.perms & P_PCF_READ) L("pcf_read", "i(iii)", h_pcf_read);
  if (s.perms & P_PCF_WRITE) L("pcf_write", "i(ii)", h_pcf_write);
#endif
}

// ---- module lifecycle (call with g_mgmt held) ----
static uint8_t parsePerms(const String &a) {
  uint8_t p = 0;
  if (a.indexOf("gpio_read") >= 0) p |= P_GPIO_READ;
  if (a.indexOf("gpio_write") >= 0) p |= P_GPIO_WRITE;
  if (a.indexOf("adc_read") >= 0) p |= P_ADC;
  if (a.indexOf("pwm_write") >= 0) p |= P_PWM;
  if (a.indexOf("millis") >= 0) p |= P_MILLIS;
  if (a.indexOf("sensor_read") >= 0) p |= P_SENSOR;
  if (a.indexOf("pcf_read") >= 0) p |= P_PCF_READ;
  if (a.indexOf("pcf_write") >= 0) p |= P_PCF_WRITE;
  return p;
}

static void slotTask(void *arg);

static void stopSlot(Slot &s) {
  if (s.task) {
    vTaskDelete(s.task);
    s.task = nullptr;
  }
  if (s.rt) m3_FreeRuntime(s.rt);
  if (s.env) m3_FreeEnvironment(s.env);
  s.rt = nullptr;
  s.env = nullptr;
  s.step = s.alloc = s.onMessage = nullptr;
  free(s.wasm);
  s.wasm = nullptr;
  s.inStep = false;
  s.needRestart = false;
  s.nSubs = 0;
  s.qHead = s.qTail = 0;
}

static bool startSlot(Slot &s, bool paused = false) {
  String base = String("/modules/") + s.name;
  File f = LittleFS.open(base + ".wasm", "r");
  if (!f) {
    jlog(s.name, "нет файла модуля");
    return false;
  }
  s.wasmLen = f.size();
  s.wasm = (uint8_t *)malloc(s.wasmLen);
  if (!s.wasm || f.read(s.wasm, s.wasmLen) != s.wasmLen) {
    f.close();
    jlog(s.name, "нет памяти под модуль");
    stopSlot(s);
    return false;
  }
  f.close();
  File a = LittleFS.open(base + ".allow", "r");
  s.perms = a ? parsePerms(a.readString()) : 0;
  if (a) a.close();
  s.env = m3_NewEnvironment();
  s.rt = s.env ? m3_NewRuntime(s.env, kStackWords, nullptr) : nullptr;
  IM3Module mod;
  M3Result r = s.rt ? m3_ParseModule(s.env, &mod, s.wasm, s.wasmLen) : "нет памяти";
  if (!r) r = m3_LoadModule(s.rt, mod);
  if (!r) {
    linkAll(s, mod);
    r = m3_FindFunction(&s.step, s.rt, "step");
  }
  if (r) {
    jlog(s.name, "не загружен: %s", r);
    stopSlot(s);
    return false;
  }
  IM3Function av;
  int32_t ver = -1;
  if (m3_FindFunction(&av, s.rt, "abi_version") || m3_CallV(av) || m3_GetResultsV(av, &ver) || ver != 3) {
    jlog(s.name, "нужен abi_version() = 3 (получено %d)", (int)ver);
    stopSlot(s);
    return false;
  }
  IM3Function init;
  if (!m3_FindFunction(&init, s.rt, "init")) m3_CallV(init);
  m3_FindFunction(&s.alloc, s.rt, "alloc");
  m3_FindFunction(&s.onMessage, s.rt, "on_message");
  s.steps = 0;
  s.paused = paused;
  memset((void *)s.inputs, 0, sizeof s.inputs);
  memset((void *)s.params, 0, sizeof s.params);
  memset((void *)s.outputs, 0, sizeof s.outputs);
  memset((void *)s.forced, 0, sizeof s.forced);
  xTaskCreatePinnedToCore(slotTask, s.name, kTaskStackBytes, &s, 1, &s.task, kCore);
  return true;
}

static void deliver(Slot &s) {
  for (;;) {
    Msg m;
    portENTER_CRITICAL(&g_mux);
    bool have = s.qHead != s.qTail;
    if (have) {
      m = s.queue[s.qHead];
      s.qHead = (s.qHead + 1) % 4;
    }
    portEXIT_CRITICAL(&g_mux);
    if (!have) return;
    if (!s.alloc || !s.onMessage) continue;
    int32_t tp = 0, dp = 0;
    int tl = strlen(m.topic);
    if (m3_CallV(s.alloc, (int32_t)tl) || m3_GetResultsV(s.alloc, &tp)) continue;
    if (m3_CallV(s.alloc, (int32_t)m.len) || m3_GetResultsV(s.alloc, &dp)) continue;
    if (!inMem(s.rt, tp, tl) || !inMem(s.rt, dp, m.len)) continue;
    uint8_t *mem = m3_GetMemory(s.rt, nullptr, 0);
    memcpy(mem + tp, m.topic, tl);
    memcpy(mem + dp, m.data, m.len);
    s.inStep = true;
    s.stepStart = millis();
    m3_CallV(s.onMessage, tp, (int32_t)tl, dp, (int32_t)m.len);
    s.inStep = false;
  }
}

static void slotTask(void *arg) {
  Slot &s = *(Slot *)arg;
  for (;;) {
    while (s.paused) vTaskDelay(pdMS_TO_TICKS(5));
    s.stepStart = millis();
    s.inStep = true;
    M3Result r = m3_CallV(s.step, (int32_t)kStepPeriodMs);
    s.inStep = false;
    if (r) {
      jlog(s.name, "ошибка шага: %s", r);
      s.needRestart = true;
      vTaskDelay(portMAX_DELAY);
    }
    s.steps++;
    deliver(s);
    vTaskDelay(pdMS_TO_TICKS(kStepPeriodMs));
  }
}

static void watchdogTask(void *) {
  for (;;) {
    vTaskDelay(pdMS_TO_TICKS(25));
    xSemaphoreTake(g_mgmt, portMAX_DELAY);
    for (auto &s : g_slots) {
      if (!s.used || !s.task) continue;
      bool hung = s.inStep && millis() - s.stepStart > kStepLimitMs;
      if (hung || s.needRestart) {
        if (hung) jlog(s.name, "завис (шаг дольше %u мс), перезапуск", (unsigned)kStepLimitMs);
        stopSlot(s);
        s.restarts++;
        startSlot(s);
      }
    }
    xSemaphoreGive(g_mgmt);
  }
}

// ---- serial protocol ----
static Slot *findSlot(const char *name, bool create) {
  for (auto &s : g_slots)
    if (s.used && !strcmp(s.name, name)) return &s;
  if (!create) return nullptr;
  for (auto &s : g_slots)
    if (!s.used) {
      s.used = true;
      strlcpy(s.name, name, sizeof s.name);
      s.restarts = 0;
      return &s;
    }
  return nullptr;
}

static bool validName(const String &n) {
  if (n.length() == 0 || n.length() > 15) return false;
  for (char c : n)
    if (!isalnum(c) && c != '_') return false;
  return true;
}

static void cmdPut(const String &line) {
  char name[32], allow[96], crcs[16];
  unsigned len;
  if (sscanf(line.c_str(), "put %31s %u %15s %95s", name, &len, crcs, allow) != 4 || !validName(name) || len == 0 || len > 65536) {
    Serial.println("err bad put");
    return;
  }
  uint8_t *buf = (uint8_t *)malloc(len);
  if (!buf) {
    Serial.println("err no memory");
    return;
  }
  Serial.println("ready");
  size_t got = 0;
  uint32_t t = millis();
  while (got < len && millis() - t < 5000) {
    int n = Serial.readBytes(buf + got, len - got);
    got += n;
    if (n) t = millis();
  }
  uint32_t want = strtoul(crcs, nullptr, 16);
  if (got != len || esp_crc32_le(0, buf, len) != want) {
    free(buf);
    Serial.println("err crc/len");
    return;
  }
  String base = String("/modules/") + name;
  File f = LittleFS.open(base + ".wasm", "w");
  f.write(buf, len);
  f.close();
  free(buf);
  File a = LittleFS.open(base + ".allow", "w");
  a.print(strcmp(allow, "-") ? allow : "");
  a.close();
  xSemaphoreTake(g_mgmt, portMAX_DELAY);
  Slot *s = findSlot(name, true);
  bool ok = false;
  if (s) {
    uint32_t t0 = micros();
    stopSlot(*s);
    ok = startSlot(*s);
    Serial.printf("swap_us=%u\n", (unsigned)(micros() - t0));
  }
  xSemaphoreGive(g_mgmt);
  Serial.println(ok ? "ok" : "err not started (see log)");
}

// Module tests: set inputs, run exact steps, read outputs bit for bit.
static void cmdTest(const String &line) {
  char cmd[10], name[32];
  int idx = 0;
  char ty = 'i';
  char vs[32];
  unsigned n = 0, dt = 0;
  Slot *s = nullptr;
  if (sscanf(line.c_str(), "%9s %31s", cmd, name) == 2) s = findSlot(name, false);
  if (!s || !s->task) {
    Serial.println("err no such module");
    return;
  }
  if (!strcmp(cmd, "reset")) {
    xSemaphoreTake(g_mgmt, portMAX_DELAY);
    stopSlot(*s);
    bool ok = startSlot(*s, true);
    xSemaphoreGive(g_mgmt);
    Serial.println(ok ? "ok" : "err not started (see log)");
  } else if (!strcmp(cmd, "in") || !strcmp(cmd, "param")) {
    if (sscanf(line.c_str(), "%*s %*s %d %c %31s", &idx, &ty, vs) != 3 || idx < 0 || idx >= 8 || (ty != 'i' && ty != 'f')) {
      Serial.println("err bad index or type");
      return;
    }
    // `i`: a decimal int (clamped to the i32 range); `f`: a float parsed in 32 bits.
    long long iv = strtoll(vs, nullptr, 10);
    iv = iv < INT32_MIN ? INT32_MIN : (iv > INT32_MAX ? INT32_MAX : iv);
    uint32_t w = ty == 'f' ? bitsOf(strtof(vs, nullptr)) : (uint32_t)(int32_t)iv;
    (cmd[0] == 'i' ? s->inputs : s->params)[idx] = w;
    Serial.println("ok");
  } else if (!strcmp(cmd, "force")) {
    if (sscanf(line.c_str(), "%*s %*s %d %c %31s", &idx, &ty, vs) != 3 || idx < 0 || idx >= kMaxOutputs || (ty != 'i' && ty != 'f')) {
      Serial.println("err bad index or type");
      return;
    }
    long long iv = strtoll(vs, nullptr, 10);
    iv = iv < INT32_MIN ? INT32_MIN : (iv > INT32_MAX ? INT32_MAX : iv);
    s->forceVal[idx] = ty == 'f' ? bitsOf(strtof(vs, nullptr)) : (uint32_t)(int32_t)iv;
    s->forced[idx] = true;
    Serial.println("ok");
  } else if (!strcmp(cmd, "unforce")) {
    if (sscanf(line.c_str(), "%*s %*s %31s", vs) != 1) {
      Serial.println("err bad index");
      return;
    }
    if (!strcmp(vs, "all")) {
      memset((void *)s->forced, 0, sizeof s->forced);
    } else {
      idx = atoi(vs);
      if (idx < 0 || idx >= kMaxOutputs) {
        Serial.println("err bad index");
        return;
      }
      s->forced[idx] = false;
    }
    Serial.println("ok");
  } else if (!strcmp(cmd, "tick")) {
    if (sscanf(line.c_str(), "%*s %*s %u %u", &n, &dt) != 2 || n > 100000) {
      Serial.println("err bad tick");
      return;
    }
    xSemaphoreTake(g_mgmt, portMAX_DELAY);
    s->paused = true;
    for (int i = 0; i < 100 && s->inStep; i++) delay(2);
    bool ok = !s->inStep;
    for (unsigned i = 0; ok && i < n; i++) {
      s->stepStart = millis();
      s->inStep = true;
      ok = !m3_CallV(s->step, (int32_t)dt);
      s->inStep = false;
      s->steps++;
      deliver(*s);
    }
    xSemaphoreGive(g_mgmt);
    Serial.println(ok ? "ok" : "err step failed");
  } else {  // out
    if (sscanf(line.c_str(), "%*s %*s %u", &n) != 1 || n > kMaxOutputs) {
      Serial.println("err bad count");
      return;
    }
    Serial.print("out");
    for (unsigned i = 0; i < n; i++) {
      Serial.printf(" %08x", (unsigned)s->outputs[i]);
    }
    Serial.println();
    Serial.println("ok");
  }
}

static void cmdLine(String line) {
  line.trim();
  if (line.startsWith("in ") || line.startsWith("param ") || line.startsWith("tick ") || line.startsWith("out ") || line.startsWith("reset ") || line.startsWith("force ") || line.startsWith("unforce ")) {
    cmdTest(line);
  } else if (line.startsWith("put ")) {
    cmdPut(line);
  } else if (line == "log") {
    int n = g_jCount < kJournal ? g_jCount : kJournal;
    for (int i = g_jCount - n; i < g_jCount; i++)
      Serial.printf("%u %s\n", (unsigned)g_journal[i % kJournal].ms, g_journal[i % kJournal].text);
    Serial.println("end");
  } else if (line == "list" || line == "stats") {
    for (auto &s : g_slots)
      if (s.used)
        Serial.printf("%s %s steps=%u restarts=%u out0=%08x crc=%08x\n", s.name, s.task ? "run" : "stop", (unsigned)s.steps, (unsigned)s.restarts, (unsigned)s.outputs[0], (unsigned)(s.wasm ? esp_crc32_le(0, s.wasm, s.wasmLen) : 0));
    Serial.printf("free=%u largest=%u\n", (unsigned)ESP.getFreeHeap(), (unsigned)heap_caps_get_largest_free_block(MALLOC_CAP_8BIT));
    Serial.println("end");
  } else if (line.startsWith("swap ") || line.startsWith("del ")) {
    String n = line.substring(line.indexOf(' ') + 1);
    xSemaphoreTake(g_mgmt, portMAX_DELAY);
    Slot *s = findSlot(n.c_str(), false);
    bool ok = false;
    if (s) {
      stopSlot(*s);
      if (line.startsWith("del ")) {
        LittleFS.remove("/modules/" + n + ".wasm");
        LittleFS.remove("/modules/" + n + ".allow");
        s->used = false;
        ok = true;
      } else {
        ok = startSlot(*s);
      }
    }
    xSemaphoreGive(g_mgmt);
    Serial.println(ok ? "ok" : "err");
  } else if (line.length()) {
    Serial.println("err unknown command");
  }
}

#ifdef FLP_WEB  // memory measurement: Wi-Fi access point + HTTP server (build with -DFLP_WEB)
#include <WiFi.h>
#include <WebServer.h>
static WebServer g_web(80);
static void webStart() {
  WiFi.mode(WIFI_AP);
  WiFi.softAP("flp-test", "12345678");
  g_web.on("/", []() { g_web.send(200, "text/html", "<!doctype html><title>FLP</title><h1>FLP master</h1>"); });
  g_web.on("/heap", []() { g_web.send(200, "text/plain", String(ESP.getFreeHeap()) + " " + ESP.getMaxAllocHeap()); });
  g_web.begin();
}
#endif

SET_LOOP_TASK_STACK_SIZE(16 * 1024);  // `tick` and `put` run wasm3 on the loop task

void setup() {
  Serial.setRxBufferSize(4096);  // native USB (C3) has only 256 bytes: a module sent at once would be cut
  Serial.begin(115200);
  Serial.setTimeout(200);
  g_mgmt = xSemaphoreCreateMutex();
#ifdef FLP_HWTASK
  g_sensorLock = xSemaphoreCreateMutex();
  xTaskCreate(hwTask, "hardware", 4096, nullptr, 1, nullptr);
#endif
  if (!LittleFS.begin(true)) Serial.println("LittleFS failed");
  LittleFS.mkdir("/modules");
  File dir = LittleFS.open("/modules");
  for (File f = dir.openNextFile(); f; f = dir.openNextFile()) {
    String n = f.name();
    if (n.endsWith(".wasm")) {
      n.remove(n.length() - 5);
      Slot *s = findSlot(n.c_str(), true);
      if (s) startSlot(*s);
    }
  }
  xTaskCreatePinnedToCore(watchdogTask, "wdt", 4096, nullptr, 3, nullptr, kCore);  // same core + higher priority than modules: deleting a hung task never crosses cores
#ifdef FLP_WEB
  webStart();
#endif
  Serial.println("master ready");
}

void loop() {
  static String line;
  while (Serial.available()) {
    char c = Serial.read();
    if (c == '\n') {
      cmdLine(line);
      line = "";
    } else if (c != '\r') {
      line += c;
    }
  }
#ifdef FLP_WEB
  g_web.handleClient();
#endif
  delay(2);
}
