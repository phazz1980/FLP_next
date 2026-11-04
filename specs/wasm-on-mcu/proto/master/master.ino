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
#ifdef FLP_MODBUS
// After wasm3: the Modbus library redefines names the sensor libraries use.
#include "flprogUart.h"
#include "flprogModbusRTU.h"
#endif
#ifdef FLP_NET
#include "flprogNetwork.h"
#ifdef FLP_WEB
#include "flprogWebServer.h"
#include <esp_mac.h>
#include <Preferences.h>
#ifdef FLP_WIFI
#include <WiFi.h>
#endif
#endif
#endif

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
       P_PCF_READ = 64, P_PCF_WRITE = 128, P_MODBUS = 256, P_UART = 512, P_TEXT = 1024, P_NET = 2048 };

#if defined(FLP_SENSORS) || defined(FLP_PCF) || defined(FLP_MODBUS) || defined(FLP_NET)
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
  uint16_t perms = 0;
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
#ifdef FLP_MODBUS
static void modbusPoll();
#endif
#ifdef FLP_NET
static void netPoll();
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
#ifdef FLP_MODBUS
    modbusPoll();
#endif
#ifdef FLP_NET
    netPoll();
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

#ifdef FLP_MODBUS
// ---- Modbus RTU: the master task runs the protocol, modules read and write the tables ----
// One call does it all: modbus(op, a, b, c, d); floats travel as their bits.
//   op 0 setting: a = code, b = value; code 0 starts a new setting list, code 63 applies it (once per role)
//   op 1 read:   a = slave address (master), b = table | kind << 4 | master << 8, c = register
//   op 2 write:  as read, d = value
//   op 3 status: a >= 0 slave of the master answers (1/0), a < 0 last error of our slave
// Setting codes: 0 role (1 master, 2 slave), 1 uart, 2 speed, 3 parity, 4 rx, 5 tx, 6 de, 7 count / address,
// 8..11 slave tables (holding, input, coils, discrete), 16 + 8 n + k master slave n: 0 address, 1 period,
// 2 timeout, 3..6 table sizes.
static FlprogUartExecutor g_mbUart;
static ModbusMasterRTU *g_mbMaster = nullptr;
static ModbusSlaveRTU *g_mbSlave = nullptr;
static int32_t g_mb[64];
static const uint8_t kMbTables[4] = {FLPROG_HOLDING_REGISTR, FLPROG_INPUT_REGISTR, FLPROG_COIL, FLPROG_DISCRETE_INPUT};

// A macro, not a template: the Arduino prototype generator cannot handle templates.
#define MB_SETUP(o)   do {     (o)->setSpeedUart(g_mb[2]);     (o)->setParityUart(g_mb[3]);     if (g_mb[4] >= 0) (o)->setPinRxUart(g_mb[4]);     if (g_mb[5] >= 0) (o)->setPinTxUart(g_mb[5]);     if (g_mb[6] >= 0) (o)->setPinPeDe(g_mb[6]);   } while (0)

static void modbusCommit() {
  if (g_mb[0] == 1 && !g_mbMaster) {
    int n = g_mb[7] < 1 ? 1 : g_mb[7] > 4 ? 4 : g_mb[7];
    auto *m = new ModbusMasterRTU(g_mb[1], n, &g_mbUart);
    MB_SETUP(m);
    for (int i = 0; i < n; i++) {
      int b = 16 + 8 * i;
      uint8_t addr = g_mb[b];
      m->setSlaveAddress(i, addr);
      m->setPollingPeriod(addr, (uint32_t)g_mb[b + 1]);
      m->setTimeOutTime(addr, (uint32_t)g_mb[b + 2]);
      for (int t = 0; t < 4; t++) {
        if (g_mb[b + 3 + t] > 0) m->configDataTable(addr, kMbTables[t], g_mb[b + 3 + t]);
      }
    }
    m->begin();
    g_mbMaster = m;
  } else if (g_mb[0] == 2 && !g_mbSlave) {
    auto *m = new ModbusSlaveRTU(g_mb[1], &g_mbUart);
    MB_SETUP(m);
    m->setSlaveAddress(g_mb[7]);
    for (int t = 0; t < 4; t++) {
      if (g_mb[8 + t] > 0) m->configDataTable(kMbTables[t], g_mb[8 + t]);
    }
    m->begin();
    g_mbSlave = m;
  }
}

static void modbusPoll() {
  xSemaphoreTake(g_sensorLock, portMAX_DELAY);
  if (g_mbMaster) g_mbMaster->pool();
  if (g_mbSlave) g_mbSlave->pool();
  xSemaphoreGive(g_sensorLock);
}

m3ApiRawFunction(h_modbus) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, op);
  m3ApiGetArg(int32_t, a);
  m3ApiGetArg(int32_t, b);
  m3ApiGetArg(int32_t, c);
  m3ApiGetArg(int32_t, d);
  int32_t r = 0;
  xSemaphoreTake(g_sensorLock, portMAX_DELAY);
  uint8_t table = kMbTables[b & 3];
  int kind = b >> 4 & 15;
  bool master = b >> 8 & 1;
  if (op == 0) {
    if (a == 0) memset(g_mb, 0, sizeof g_mb);
    if (a == 63) modbusCommit();
    else if (a >= 0 && a < 63) g_mb[a] = b;
  } else if (op == 1) {
    if (master && g_mbMaster) {
      if (kind == 1) { float f = g_mbMaster->readFloat(a, table, c); memcpy(&r, &f, 4); }
      else if (kind == 2) r = g_mbMaster->readBool(a, table, c);
      else r = (int16_t)g_mbMaster->readInteger(a, table, c);
    } else if (!master && g_mbSlave) {
      if (kind == 1) { float f = g_mbSlave->readFloat(table, c); memcpy(&r, &f, 4); }
      else if (kind == 2) r = g_mbSlave->readBool(table, c);
      else r = (int16_t)g_mbSlave->readInteger(table, c);
    }
  } else if (op == 2) {
    float f;
    memcpy(&f, &d, 4);
    if (master && g_mbMaster) {
      if (kind == 1) g_mbMaster->saveFloat(a, f, table, c);
      else if (kind == 2) g_mbMaster->saveBool(a, d != 0, table, c);
      else g_mbMaster->saveInteger(a, (int16_t)d, table, c);
    } else if (!master && g_mbSlave) {
      if (kind == 1) g_mbSlave->saveFloat(f, table, c);
      else if (kind == 2) g_mbSlave->saveBool(d != 0, table, c);
      else g_mbSlave->saveInteger((int16_t)d, table, c);
    }
  } else if (op == 3) {
    if (a >= 0) r = g_mbMaster && g_mbMaster->slaveStatus(a, false) ? 1 : 0;
    else r = g_mbSlave ? g_mbSlave->getLastError() : 0;
  }
  xSemaphoreGive(g_sensorLock);
  m3ApiReturn(r);
}
#endif

#ifdef FLP_TEXT
// ---- text operations over the module memory ----
// text(op, out, cap, a, alen, b, blen); a string is (ptr, len), `cur` is where an append continues.
//  1 append a to out at cur=b             2 append float (bits in a, alen decimals) at cur=b
//  3 append int a at cur=b                4 append byte a at cur=b        (all return the new length, cap is the room)
//  5 compare a with b: mode out (0 equal, 1 contains, 2 starts, 3 ends), ignore case cap -> 0/1
//  6 a as number -> float bits at out, returns 1 if it is one
//  7 a from b, blen bytes (-1: to the end) -> out (cap), returns the length
//  8 position of b in a from out, or -1     9 byte of a at b, or -1
// 10 a as "x.y.z.w" -> 4 bytes at out, returns 1 if valid     11 four bytes packed in a -> "x.y.z.w" in out
static bool lowerEq(char x, char y) { return tolower((unsigned char)x) == tolower((unsigned char)y); }

static int textMatch(const char *a, int la, const char *b, int lb, int mode, bool nocase) {
  if (mode == 0 && la != lb) return 0;
  if (lb > la) return 0;
  int first = mode == 3 ? la - lb : 0;
  int last = mode == 1 ? la - lb : mode == 0 ? 0 : first;
  for (int at = first; at <= last; at++) {
    int i = 0;
    while (i < lb && (nocase ? lowerEq(a[at + i], b[i]) : a[at + i] == b[i])) i++;
    if (i == lb) return 1;
  }
  return 0;
}

// Appends n bytes of s to out at cur (the room is cap); returns the new length.
static int textPut(char *out, int cap, int cur, const char *s, int n) {
  if (cur < 0) cur = 0;
  if (cur > cap) cur = cap;
  if (n > cap - cur) n = cap - cur;
  if (n > 0) memmove(out + cur, s, n);
  return cur + (n > 0 ? n : 0);
}

m3ApiRawFunction(h_text) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, op);
  m3ApiGetArg(int32_t, out);
  m3ApiGetArg(int32_t, cap);
  m3ApiGetArg(int32_t, a);
  m3ApiGetArg(int32_t, alen);
  m3ApiGetArg(int32_t, b);
  m3ApiGetArg(int32_t, blen);
  char *m = (char *)_mem;
  bool readsA = op == 1 || (op >= 5 && op <= 10);
  bool readsB = op == 5 || op == 8;
  bool writesOut = op >= 1 && op <= 4;
  bool ok = op >= 1 && op <= 11 && (!readsA || inMem(runtime, a, alen)) && (!readsB || inMem(runtime, b, blen));
  if (op == 7 || op == 11) writesOut = true;
  if (writesOut) ok = ok && inMem(runtime, out, cap);
  if (op == 6 || op == 10) ok = ok && inMem(runtime, out, 4);
  if (!ok) m3ApiReturn(op == 8 || op == 9 ? -1 : 0);
  int32_t r = 0;
  char t[48];
  switch (op) {
    case 1: r = textPut(m + out, cap, b, m + a, alen); break;
    case 2: {
      float f;
      memcpy(&f, &a, 4);
      int n = snprintf(t, sizeof t, "%.*f", alen < 0 ? 0 : alen > 9 ? 9 : alen, (double)f);
      r = textPut(m + out, cap, b, t, n < 0 ? 0 : n > 47 ? 47 : n);
      break;
    }
    case 3: r = textPut(m + out, cap, b, t, snprintf(t, sizeof t, "%ld", (long)a)); break;
    case 4: { char c = (char)(a & 0xFF); r = textPut(m + out, cap, b, &c, 1); break; }
    case 5: r = textMatch(m + a, alen, m + b, blen, out, cap != 0); break;
    case 6: {
      // Spaces around are skipped, a decimal comma is accepted.
      int n = alen < 47 ? alen : 47;
      memcpy(t, m + a, n);
      t[n] = 0;
      char *s = t;
      while (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n') s++;
      char *e = s + strlen(s);
      while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\r' || e[-1] == '\n')) *--e = 0;
      for (char *p = s; *p; p++) if (*p == ',') *p = '.';
      char *end = nullptr;
      double v = *s ? strtod(s, &end) : 0;
      if (*s && *end == 0 && alen < 48 && isfinite(v)) {
        float f = (float)v;
        memcpy(m + out, &f, 4);
        r = 1;
      }
      break;
    }
    case 7: {
      int start = b < 0 ? 0 : b > alen ? alen : b;
      int end = blen < 0 || start + blen > alen ? alen : start + blen;
      r = textPut(m + out, cap, 0, m + a + start, end - start);
      break;
    }
    case 8: {
      int from = out < 0 ? 0 : out;
      r = -1;
      if (from <= alen) {
        if (blen == 0) r = from;
        else for (int at = from; at + blen <= alen; at++)
          if (!memcmp(m + a + at, m + b, blen)) { r = at; break; }
      }
      break;
    }
    case 9: r = (b >= 0 && b < alen) ? (uint8_t)m[a + b] : -1; break;
    case 10: {
      int v[4] = {0, 0, 0, 0}, part = 0;
      bool digits = false, good = true;
      for (int i = 0; i < alen && good; i++) {
        char ch = m[a + i];
        if (ch >= '0' && ch <= '9') { v[part] = v[part] * 10 + (ch - '0'); digits = true; if (v[part] > 255) good = false; }
        else if (ch == '.' && digits && part < 3) { part++; digits = false; }
        else good = false;
      }
      if (good && digits && part == 3) { for (int i = 0; i < 4; i++) m[out + i] = (char)v[i]; r = 1; }
      break;
    }
    case 11: {
      int n = snprintf(t, sizeof t, "%u.%u.%u.%u", (unsigned)(a & 255), (unsigned)((a >> 8) & 255), (unsigned)((a >> 16) & 255), (unsigned)((a >> 24) & 255));
      r = textPut(m + out, cap, 0, t, n);
      break;
    }
  }
  m3ApiReturn(r);
}
#endif

#ifdef FLP_UART
// ---- UART bytes of ports 1 and 2 (port 0 is the console of the master's own protocol) ----
// uart(op, port, a, b):
//   op 0 setting: a = code, b = value; code 0 starts the list, 63 starts the port (once)
//        codes: 1 speed, 2 data bits, 3 parity (0 N, 1 E, 2 O), 4 stop bits, 5 rx, 6 tx, 7 de, 8 receive buffer
//   op 1 next byte or -1      op 2 bytes waiting      op 3 write: a = value bits, b = size code (0: 1 byte, 1: 2, 2: 4);
//   returns 1 if it fitted into the transmit buffer      op 4 speed now      op 5 set speed a
//   op 6 write text: a = pointer, b = length; returns 1 if it fitted
static int32_t g_ua[3][16];
static bool g_uaOn[3];

static HardwareSerial *uartOf(int port) {
  if (port == 1) return &Serial1;
#if SOC_UART_NUM > 2
  if (port == 2) return &Serial2;
#endif
  return nullptr;
}

static uint32_t uartFrame(int data, int parity, int stop) {
  static_assert(SERIAL_8E2 == SERIAL_8N1 + 0x22 && SERIAL_5N1 + 12 == SERIAL_8N1, "frame constants changed");
  if (data < 5 || data > 8) data = 8;
  return SERIAL_5N1 + (data - 5) * 4 + (parity == 1 ? 2 : parity == 2 ? 3 : 0) + (stop == 2 ? 0x20 : 0);
}

m3ApiRawFunction(h_uart) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, op);
  m3ApiGetArg(int32_t, port);
  m3ApiGetArg(int32_t, a);
  m3ApiGetArg(int32_t, b);
  int32_t r = 0;
  HardwareSerial *s = uartOf(port);
  if (s) {
    int32_t *c = g_ua[port];
    if (op == 0) {
      if (a == 0) memset(c, 0, sizeof g_ua[0]);
      else if (a == 63) {
        if (!g_uaOn[port]) {
          s->setRxBufferSize(c[8] > 0 ? c[8] : 256);
          s->begin(c[1], uartFrame(c[2], c[3], c[4]), c[5], c[6]);
          if (c[7] >= 0) {
            s->setPins(-1, -1, -1, c[7]);
            s->setMode(UART_MODE_RS485_HALF_DUPLEX);
          }
          g_uaOn[port] = true;
        }
      } else if (a > 0 && a < 16) c[a] = b;
    } else if (!g_uaOn[port]) {
      r = op == 1 ? -1 : 0;
    } else if (op == 1) r = s->read();
    else if (op == 2) r = s->available();
    else if (op == 3) {
      size_t n = 1u << b;
      if ((size_t)s->availableForWrite() >= n) { s->write((const uint8_t *)&a, n); r = 1; }
    } else if (op == 4) r = s->baudRate();
    else if (op == 5 && a > 0) s->updateBaudRate(a);
    else if (op == 6 && inMem(runtime, a, b)) {
      // Text: b bytes at a, all or nothing.
      if (s->availableForWrite() >= (b < 64 ? b : 64)) { s->write((const uint8_t *)_mem + a, b); r = 1; }
    }
  } else if (op == 1) r = -1;
  m3ApiReturn(r);
}
#endif

#ifdef FLP_NET
// ---- Wi-Fi, Ethernet and NTP (FLProg_Network): modules ask, the hardware task does the work ----
// net(op, a, b, c, d); modules only write requests and read the last snapshot, the task owns the interfaces.
// FLP_WIFI and FLP_ETH say which interfaces the master carries.
//   op 0 number: a = code (1 dhcp, 2 NTP period ms, 3 NTP interface 0 Wi-Fi / 1 Ethernet, 63 Wi-Fi settings are complete,
//        64 start NTP), b = value
//   op 1 text setting: a = code (1 ssid, 2 password, 3 ip, 4 gateway, 5 subnet, 6 ap ssid, 7 ap password, 8 NTP server),
//        b = pointer, c = length
//   op 2 radio: a = 0 off, 1 client, 2 access point, 3 both
//   op 3 Wi-Fi: a = field (0 client ready, 1 access point ready, 2 status, 3 error, 4 address as a number)
//   op 4 NTP: a = field (0 has time, 1 age in s, 2 hour, 3 minute, 4 second, 5 day, 6 month, 7 year, 8 weekday 1..7,
//        9 Unix UTC), b = offset from UTC in seconds
//   op 5 time as text "HH:MM:SS": a = pointer, b = capacity, c = offset; returns the length
//   op 6 Ethernet number: a = code (1 dhcp, 2 sck, 3 miso, 4 mosi, 5 cs, 6 last MAC byte, 63 settings are complete), b = value
//   op 7 Ethernet text: a = code (1 ip, 2 gateway, 3 subnet), b = pointer, c = length
//   op 8 Ethernet: a = field (0 ready, 1 address as a number)
//   MQTT (FLP_MQTT, ops 9..16; the client works on the interface chosen by code 3):
//   op 9 number: a = code (1 port, 2 keep-alive s, 3 network 0 Wi-Fi / 1 Ethernet, 4 max subscriptions, 63 settings are complete), b = value
//   op 10 text setting: a = code (1 broker, 2 client id, 3 user, 4 password), b = pointer, c = length
//   op 11 read: a = field (0 connected, 1 error)
//   op 12 / 13 publish without / with retain: a, b topic (pointer, length), c, d payload (pointer, length); 1 queued, 0 no link or queue full
//   op 14 subscribe: a, b filter (pointer, length); returns the slot (the same for equal filters of any module), -1 when full
//   op 15 slot text: a = slot, b = pointer, c = capacity; returns the length
//   op 16 slot counter: a = slot; grows with every message that matched the filter
//   Web page (FLP_WEB, ops 17..22; the page and its server are the master's, the module gives layout and values):
//   op 17 setup: a = 1 layout JSON (b, c pointer, length), 2 widget table (b pointer, c count; 16 bytes each: kind 0 number / 1 text shown /
//         2 text input / 3 secret text input, rw, dec, reserved, lo, hi, initial as f32), 3 interfaces (b: 1 Wi-Fi, 2 Ethernet, 3 both),
//         4 password (b, c), 5 network form defaults (b, c: ten lines ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay; d widget), 63 setup complete; interfaces mask also has bit 4 = the «Система» page
//   (a chart is kind 4: reserved byte = points, initial = period in seconds; the master samples its value itself)
//   op 19 show number: a = widget, b = f32 bits;  op 20 show text: a = widget, b, c pointer and length
//   op 23 network form: a = 0..9 field as text (b pointer, c capacity; returns length), 10 saved flag (cleared), 11 DHCP, 12 mode
//   op 21 number written by the page: a = widget; returns f32 bits;  op 22 text written by the page: a = widget, b = pointer, c = capacity; returns the length
// SPI pins, cs and MAC are taken when the Ethernet interface is created (first start); changing them needs a reset.
#ifdef FLP_WIFI
static FLProgOnBoardWifiInterface g_wifi;
#endif
#ifdef FLP_ETH
static FLProgWiznetInterface *g_eth = nullptr;
#endif
static FLProgNetworkNtp g_ntp;
struct NetSettings {
  char text[8][65];
  int32_t dhcp = 1, ntpPeriod = 3600000, ntpIface = 0;
  volatile bool complete = false, ntpAsked = false;
  volatile int32_t want = 0;
  // Ethernet settings: ethNum is indexed by the code of op 6
  char ethText[3][65];
  int32_t ethNum[7] = {0, 1, 18, 19, 23, 5, 40};
  volatile bool ethComplete = false;
  // task state
  uint32_t applied = 0, ethApplied = 0;
  bool appliedOnce = false, ntpOn = false;
  uint8_t state = 255;
  uint32_t tick = 0, retry = 0;
  // snapshot for modules
  volatile int32_t clientReady = 0, apReady = 0, status = 0, error = 0, hasTime = 0, ethReady = 0;
  volatile uint32_t ip = 0, ethIp = 0, unixTime = 0, syncAge = 0;
};
static NetSettings g_net;

#ifdef FLP_WEB
// Settings of the web page network form (web_net): they replace the ones the Wi-Fi and Ethernet blocks gave.
// Fields: ssid, password, dhcp, ip, gateway, subnet, ap ssid, ap password, mode, ap delay.
static String g_wnf[10];
static bool g_wnOn = false;
static int g_wnItem = -1;
static volatile bool g_wnChanged = false;
static uint32_t g_wnSig = 0;
static Preferences g_wnPrefs;
static const char *const kWnKeys[10] = {"wn_s", "wn_p", "wn_d", "wn_i", "wn_g", "wn_m", "wn_a", "wn_b", "wn_o", "wn_w"};

// Changes with the fields that need the radio or the address to be set again (not mode and delay).
static void wnSign() {
  uint32_t h = 2166136261UL;
  for (int k = 0; k < 8; k++) {
    for (size_t n = 0; n < g_wnf[k].length(); n++) h = (h ^ (uint8_t)g_wnf[k][n]) * 16777619UL;
    h = (h ^ 0xFF) * 16777619UL;
  }
  g_wnSig = h;
}
#endif

static uint32_t netHashOf(char (*t)[65], int count, int32_t dhcp) {
  uint32_t h = 2166136261u;
  for (int i = 0; i < count; i++) {
    for (const char *c = t[i]; *c; c++) h = (h ^ (uint8_t)*c) * 16777619u;
    h = (h ^ 0xFF) * 16777619u;
  }
  return (h ^ (uint32_t)dhcp) * 16777619u;
}

// DHCP or the fixed address, as the C++ blocks do it.
static void netAddress(FLProgNetworkInterface &net, int32_t dhcp, const char *ip, const char *gateway, const char *subnet) {
  auto addr = [](const char *t) { IPAddress a; a.fromString(t); return a; };
  if (dhcp) net.setDhcp();
  else {
    net.localIP(addr(ip));
    net.gateway(addr(gateway));
    net.dns(addr(gateway));
    net.subnet(addr(subnet));
    net.resetDhcp();
  }
}

#ifdef FLP_WIFI
static void wifiApply() {
  const char *ssid = g_net.text[0], *pass = g_net.text[1], *ip = g_net.text[2], *gw = g_net.text[3], *sn = g_net.text[4];
  const char *apSsid = g_net.text[5], *apPass = g_net.text[6];
  int32_t dhcp = g_net.dhcp;
#ifdef FLP_WEB
  if (g_wnOn) {
    ssid = g_wnf[0].c_str(); pass = g_wnf[1].c_str(); dhcp = g_wnf[2] != "0";
    ip = g_wnf[3].c_str(); gw = g_wnf[4].c_str(); sn = g_wnf[5].c_str();
    apSsid = g_wnf[6].c_str(); apPass = g_wnf[7].c_str();
  }
#endif
  g_wifi.setClientSsid(ssid);
  g_wifi.setClientPassword(pass);
  netAddress(g_wifi, dhcp, ip, gw, sn);
  g_wifi.setApSsid(apSsid);
  g_wifi.setApPassword(apPass);
}

#ifdef FLP_WEB
// Radio use chosen on the page: 0 client, 1 access point, 2 both, 3 client and the access point only without a connection.
static uint8_t wnWant() {
  int m = g_wnf[8].toInt();
  if (m == 3) {
    static uint32_t since = 0, joined = 0;
    static bool apUp = false;
    if (g_wifi.clientIsReady()) {
      // The access point stays up for 10 s after the client got connected.
      if (!joined) joined = millis();
      if (millis() - joined >= 10000UL) apUp = false;
      since = millis();
    } else {
      joined = 0;
      if (!apUp && millis() - since >= (uint32_t)g_wnf[9].toInt() * 1000UL) apUp = true;
    }
    m = apUp ? 2 : 0;
  }
  return m == 0 ? 1 : m == 1 ? 2 : 3;
}
#endif

static void wifiPoll() {
  NetSettings &n = g_net;
  if (!n.complete) return;
  uint32_t h = netHashOf(n.text, 8, n.dhcp);
#ifdef FLP_WEB
  if (g_wnOn) h ^= g_wnSig;
#endif
  if (h != n.applied) {
    // New settings (first start, or a new module with other ones): re-apply and restart the radio.
    wifiApply();
    if (n.appliedOnce) { g_wifi.clientOff(); g_wifi.apOff(); n.state = 255; }
    n.applied = h;
    n.appliedOnce = true;
  }
  uint8_t want = (uint8_t)(n.want & 3);
#ifdef FLP_WEB
  if (g_wnOn) want = wnWant();
#endif
  if (want != n.state) {
    n.state = want;
    if (want & 1) g_wifi.clientOn(); else g_wifi.clientOff();
    if (want & 2) g_wifi.apOn(); else g_wifi.apOff();
  }
  g_wifi.pool();
  n.clientReady = g_wifi.clientIsReady();
  n.apReady = g_wifi.apIsReady();
  n.status = g_wifi.getStatus();
  n.error = g_wifi.getError();
  n.ip = n.clientReady ? (uint32_t)g_wifi.localIP() : n.apReady ? (uint32_t)g_wifi.accessPointIP() : 0;
}
#endif

#ifdef FLP_ETH
static void ethPoll() {
  NetSettings &n = g_net;
  if (!n.ethComplete) return;
  if (!g_eth) {
    g_eth = new FLProgWiznetInterface(n.ethNum[5]);
    g_eth->mac(0x78, 0xAC, 0xC0, 0x2C, 0x3E, (uint8_t)n.ethNum[6]);
    if (n.ethNum[2] != 18 || n.ethNum[3] != 19 || n.ethNum[4] != 23) g_eth->setSpiPins(n.ethNum[2], n.ethNum[3], n.ethNum[4]);
  }
  uint32_t h = netHashOf(n.ethText, 3, n.ethNum[1]);
  int32_t dhcp = n.ethNum[1];
  const char *ip = n.ethText[0], *gw = n.ethText[1], *sn = n.ethText[2];
#ifdef FLP_WEB
  if (g_wnOn) {
    h ^= g_wnSig;
    dhcp = g_wnf[2] != "0"; ip = g_wnf[3].c_str(); gw = g_wnf[4].c_str(); sn = g_wnf[5].c_str();
  }
#endif
  if (h != n.ethApplied) {
    netAddress(*g_eth, dhcp, ip, gw, sn);
    n.ethApplied = h;
  }
  g_eth->pool();
  n.ethReady = g_eth->ethernetIsReady();
  n.ethIp = n.ethReady ? (uint32_t)g_eth->localIP() : 0;
}
#endif

#ifdef FLP_MQTT
// ---- MQTT client: one per board, on the interface chosen by the module (ops 9..16 of net) ----
// Modules only queue publications, register filters and read the last message of a filter; the hardware task owns the client.
struct MqttSlot {
  char filter[129];
  char text[129];
  volatile int32_t len = 0;
  volatile uint32_t seq = 0;
  bool subscribed = false;
};
struct MqttPublish {
  char topic[129];
  char text[129];
  uint8_t len;
  bool retain;
};
static const int kMqttQueue = 4;
static FLProgNetworkMqttClient g_mqtt;
static portMUX_TYPE g_mqttMux = portMUX_INITIALIZER_UNLOCKED;
static char g_mqttText[4][65];  // broker, client id, user, password
static int32_t g_mqttNum[5] = {0, 1883, 60, 0, 4};  // [1] port, [2] keepalive s, [3] network, [4] max subscriptions
static volatile bool g_mqttComplete = false;
static bool g_mqttBegun = false;
static uint32_t g_mqttApplied = 0;
static MqttSlot *g_mqttSlots = nullptr;
static int g_mqttSlotCount = 0;
static MqttPublish g_mqttQueue[kMqttQueue];
static volatile int g_mqttHead = 0, g_mqttTail = 0;

// Topic against a filter with + (one level) and # (the rest).
static bool mqttMatch(const char *f, const char *t) {
  for (;;) {
    const char *fe = strchr(f, '/');
    const char *te = strchr(t, '/');
    size_t fn = fe ? (size_t)(fe - f) : strlen(f);
    size_t tn = te ? (size_t)(te - t) : strlen(t);
    if (fn == 1 && *f == '#') return true;
    if (!(fn == 1 && *f == '+') && (fn != tn || strncmp(f, t, fn) != 0)) return false;
    if (!fe || !te) return !fe && !te;
    f = fe + 1;
    t = te + 1;
  }
}

static void mqttReceive(const char *topic, const uint8_t *payload, size_t length, bool retained) {
  (void)retained;
  if (length > 128) length = 128;  // longer messages are cut, like every module string
  for (int i = 0; i < g_mqttSlotCount; i++) {
    MqttSlot &s = g_mqttSlots[i];
    if (!s.filter[0] || !mqttMatch(s.filter, topic)) continue;
    portENTER_CRITICAL(&g_mqttMux);
    memcpy(s.text, payload, length);
    s.text[length] = 0;
    s.len = (int32_t)length;
    s.seq = s.seq + 1;
    portEXIT_CRITICAL(&g_mqttMux);
  }
}

static void mqttApply() {
  g_mqtt.setBroker(g_mqttText[0]);
  g_mqtt.setPort((uint16_t)g_mqttNum[1]);
  if (g_mqttText[2][0]) g_mqtt.setCredentials(g_mqttText[2], g_mqttText[3]);
  else g_mqtt.clearCredentials();
  if (g_mqttText[1][0]) g_mqtt.setClientId(g_mqttText[1]);
  else {
    char cid[24];
    snprintf(cid, sizeof(cid), "flp-%08lx", (unsigned long)ESP.getEfuseMac());
    g_mqtt.setClientId(cid);
  }
  g_mqtt.setKeepAlive((uint16_t)g_mqttNum[2]);
}

static void mqttPoll() {
  if (!g_mqttComplete) return;
  if (!g_mqttBegun) {
    FLProgNetworkInterface *iface = nullptr;
#ifdef FLP_WIFI
    if (g_mqttNum[3] == 0 && g_net.complete) iface = &g_wifi;
#endif
#ifdef FLP_ETH
    if (g_mqttNum[3] == 1 && g_eth) iface = g_eth;
#endif
    if (!iface) return;
    g_mqtt.setSubscriptionLimit((size_t)g_mqttNum[4]);
    g_mqttBegun = g_mqtt.begin(*iface);
    if (!g_mqttBegun) return;
    mqttApply();
    g_mqtt.setMessageCallback(mqttReceive);
    g_mqtt.setAutoReconnect(true);
    g_mqttApplied = netHashOf(g_mqttText, 4, g_mqttNum[1] ^ (g_mqttNum[2] << 16));
    g_mqtt.connect();
  } else {
    uint32_t h = netHashOf(g_mqttText, 4, g_mqttNum[1] ^ (g_mqttNum[2] << 16));
    if (h != g_mqttApplied) {
      // Settings written while running: reconnect with the new ones.
      g_mqttApplied = h;
      g_mqtt.disconnect();
      mqttApply();
      g_mqtt.connect();
    }
  }
  if (g_mqtt.connected()) {
    for (int i = 0; i < g_mqttSlotCount; i++) {
      MqttSlot &s = g_mqttSlots[i];
      if (s.filter[0] && !s.subscribed) s.subscribed = g_mqtt.subscribe(s.filter);
    }
    while (g_mqttTail != g_mqttHead) {
      MqttPublish &p = g_mqttQueue[g_mqttTail];
      if (!g_mqtt.publish(p.topic, (const uint8_t *)p.text, p.len, p.retain)) break;
      g_mqttTail = (g_mqttTail + 1) % kMqttQueue;
    }
  }
}

// Slot of a filter (the same one for every module); -1 when there is no room.
static int32_t mqttSubscribe(const char *filter) {
  if (!g_mqttComplete) return -1;
  if (!g_mqttSlots) {
    int n = g_mqttNum[4] < 1 ? 1 : g_mqttNum[4] > 16 ? 16 : g_mqttNum[4];
    MqttSlot *slots = new MqttSlot[n]();
    g_mqttSlots = slots;
    g_mqttSlotCount = n;
  }
  int free = -1;
  for (int i = 0; i < g_mqttSlotCount; i++) {
    if (!strcmp(g_mqttSlots[i].filter, filter)) return i;
    if (free < 0 && !g_mqttSlots[i].filter[0]) free = i;
  }
  if (free < 0) return -1;
  portENTER_CRITICAL(&g_mqttMux);
  strcpy(g_mqttSlots[free].filter, filter);
  portEXIT_CRITICAL(&g_mqttMux);
  return free;
}
#endif

#ifdef FLP_WEB
// ---- Web page: one per board, served by the master from the registry the module fills (ops 17..22 of net) ----
// The page (flp_html, put in front of the sketch by the build) is the same as in a C++ sketch; the module gives the
// layout JSON, a table of the widgets and then the values of every widget each step.
struct WebItem {
  uint8_t kind;  // 0 number, 1 text shown, 2 text input, 3 secret text input, 4 chart
  bool rw;       // the page may change the value
  uint8_t dec;   // digits after the point
  float lo, hi;
  volatile float value;
  char *text;    // 129 bytes for the text kinds
  float *hist;   // chart: ring buffer of hn points
  uint16_t hn, hpos, hfill;
  uint32_t hlast, hperiod;  // ms
};
static const int kWebMax = 32;
static WebItem g_webItems[kWebMax];
static int g_webCount = 0;
static char *g_webLayout = nullptr;
static int g_webLayoutCap = 0;
static char g_webPass[65];
static int32_t g_webMask = 1;  // 1 Wi-Fi, 2 Ethernet
static volatile bool g_webComplete = false;
static bool g_webBegun = false;
static bool g_webSystem = false;
static FLProgNetworkHttpServer g_web;
static portMUX_TYPE g_webMux = portMUX_INITIALIZER_UNLOCKED;

static void webEsc(String &o, const char *s) {
  for (; *s; s++) {
    if (*s == '\n') { o += "\\n"; continue; }
    if (*s == '"' || *s == '\\') o += '\\';
    if ((uint8_t)*s >= 32) o += *s;
  }
}
// The page itself is public; everything it reads or writes needs the password (argument k).
static bool webAllowed(FLProgNetworkHttpServer *server) {
  if (!g_webPass[0] || server->argumentValueAtKey("k") == g_webPass) return true;
  server->send(403, "text/plain", "auth");
  return false;
}
static void webPage(FLProgNetworkHttpServer *server) {
  if (!server->sendStreamProgmem(200, "text/html; charset=utf-8", flp_html, strlen(flp_html))) {
    server->write((const uint8_t *)flp_html, strlen(flp_html));
  }
}
static void webLayout(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  server->send(200, "application/json", g_webLayout ? g_webLayout : "{\"i\":[]}");
}
static void webState(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  String o = "[";
  char buf[129];
  for (int i = 0; i < g_webCount; i++) {
    WebItem &it = g_webItems[i];
    if (i) o += ',';
    if (it.kind == 0) {
      o += String((float)it.value, (unsigned int)it.dec);
    } else {
      buf[0] = 0;
      // A secret field is never sent back to the page.
      if (it.text && it.kind != 3) {
        portENTER_CRITICAL(&g_webMux);
        strcpy(buf, it.text);
        portEXIT_CRITICAL(&g_webMux);
      }
      o += '"';
      webEsc(o, buf);
      o += '"';
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
static void webSet(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  if (server->hasArgumentKey("i") && server->hasArgumentKey("v")) {
    int i = server->argumentValueAtKey("i").toInt();
    if (i >= 0 && i < g_webCount && g_webItems[i].rw) {
      WebItem &it = g_webItems[i];
      String v = server->argumentValueAtKey("v");
      if (it.kind >= 2 && it.text) {
        v.replace("\n", " ");
        v.replace("\r", " ");
        portENTER_CRITICAL(&g_webMux);
        strncpy(it.text, v.c_str(), 128);
        it.text[128] = 0;
        portEXIT_CRITICAL(&g_webMux);
      } else if (it.kind == 0) {
        float f = v.toFloat();
        if (it.hi > it.lo) f = constrain(f, it.lo, it.hi);
        it.value = f;
      }
    }
  }
  server->send(200, "text/plain", "ok");
}
static void webHist(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  String o = "[";
  int i = server->argumentValueAtKey("i").toInt();
  if (i >= 0 && i < g_webCount && g_webItems[i].hist) {
    WebItem &it = g_webItems[i];
    uint16_t start = (it.hpos + it.hn - it.hfill) % it.hn;
    for (uint16_t k = 0; k < it.hfill; k++) {
      if (k) o += ',';
      o += String(it.hist[(start + k) % it.hn], 2);
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
// Network settings form: the master validates, saves to flash and applies what the page sends.
static void wnShow() {
  if (g_wnItem < 0 || g_wnItem >= g_webCount || !g_webItems[g_wnItem].text) return;
  // Passwords are never sent back.
  String t = g_wnf[0] + "\n\n" + g_wnf[2] + "\n" + g_wnf[3] + "\n" + g_wnf[4] + "\n" + g_wnf[5] + "\n" + g_wnf[6] + "\n\n" + g_wnf[8] + "\n" + g_wnf[9];
  portENTER_CRITICAL(&g_webMux);
  strncpy(g_webItems[g_wnItem].text, t.c_str(), 128);
  g_webItems[g_wnItem].text[128] = 0;
  portEXIT_CRITICAL(&g_webMux);
}
// The module hands the defaults as ten lines; when they change (another module), the saved page values are dropped.
static void wnLoad(const String &t, int item) {
  String init[10], cur;
  int k = 0;
  for (size_t i = 0; i < t.length() && k < 10; i++) {
    if (t[i] == '\n') { init[k++] = cur; cur = ""; } else cur += t[i];
  }
  if (k < 10) init[k] = cur;
  g_wnPrefs.begin("flp", false);
  uint32_t sig = 2166136261UL;
  for (int j = 0; j < 10; j++) {
    for (size_t n = 0; n < init[j].length(); n++) sig = (sig ^ (uint8_t)init[j][n]) * 16777619UL;
    sig = (sig ^ 0xFF) * 16777619UL;
  }
  if (g_wnPrefs.getUInt("wn_h", 0) != sig) {
    for (int j = 0; j < 10; j++) g_wnPrefs.remove(kWnKeys[j]);
    g_wnPrefs.putUInt("wn_h", sig);
  }
  for (int j = 0; j < 10; j++) g_wnf[j] = g_wnPrefs.getString(kWnKeys[j], init[j]);
  g_wnItem = item;
  g_wnOn = true;
  wnSign();
  wnShow();
}
static bool wnPwOk(const String &p) {
  if (p.length() == 0) return true;
  for (size_t k = 0; k < p.length(); k++) if (p[k] < 32 || p[k] > 126) return false;
  if (p.length() >= 8 && p.length() <= 63) return true;
  if (p.length() != 64) return false;
  for (size_t k = 0; k < 64; k++) if (!isxdigit(p[k])) return false;
  return true;
}
static bool wnSsidOk(const String &s) {
  if (s.length() > 32) return false;
  for (size_t k = 0; k < s.length(); k++) if ((uint8_t)s[k] < 32) return false;
  return true;
}
static void webNet(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  static const char *const a[10] = {"s", "p", "d", "i", "g", "m", "a", "b", "o", "w"};
  String nf[10];
  for (int k = 0; k < 10; k++) {
    nf[k] = server->argumentValueAtKey(a[k]);
    nf[k].replace("\n", " ");
    nf[k].replace("\r", " ");
  }
  // An empty password keeps the saved one.
  if (!nf[1].length()) nf[1] = g_wnf[1];
  if (!nf[7].length()) nf[7] = g_wnf[7];
  if (!g_wnOn || !wnPwOk(nf[1]) || !wnPwOk(nf[7]) || !wnSsidOk(nf[0]) || !wnSsidOk(nf[6])) {
    server->send(400, "text/plain", "bad");
    return;
  }
  for (int k = 0; k < 10; k++) {
    if (nf[k] != g_wnf[k]) {
      g_wnf[k] = nf[k];
      // Flash wears out: only changed fields are written.
      g_wnPrefs.putString(kWnKeys[k], nf[k]);
      g_wnChanged = true;
    }
  }
  wnSign();
  wnShow();
  server->send(200, "text/plain", "ok");
}
// Wi-Fi scan: the first call starts it, "?" means still running, then one "ssid<TAB>rssi" per line.
static void webScan(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  String o;
#ifdef FLP_WIFI
  int n = WiFi.scanComplete();
  if (n == WIFI_SCAN_FAILED) {
    if (WiFi.getMode() == WIFI_MODE_NULL) WiFi.mode(WIFI_STA);
    else if (WiFi.getMode() == WIFI_MODE_AP) WiFi.mode(WIFI_MODE_APSTA);
    WiFi.scanNetworks(true);
    o = "?";
  } else if (n == WIFI_SCAN_RUNNING) {
    o = "?";
  } else {
    for (int k = 0; k < n; k++) {
      String s = WiFi.SSID(k);
      if (!s.length() || o.startsWith(s + "\t") || o.indexOf("\n" + s + "\t") >= 0) continue;
      String line = s + "\t" + String(WiFi.RSSI(k)) + "\n";
      // The server answers with at most 2 KB: the strongest networks, each name once.
      if (o.length() + line.length() > 1800) break;
      o += line;
    }
    WiFi.scanDelete();
  }
#endif
  server->send(200, "text/plain", o);
}
// Technical data for the «Система» page; there is no firmware upload over the air in the master.
static void webInfo(FLProgNetworkHttpServer *server) {
  if (!webAllowed(server)) return;
  char mac[18];
  uint8_t m[6];
  esp_read_mac(m, ESP_MAC_WIFI_STA);
  snprintf(mac, sizeof(mac), "%02X:%02X:%02X:%02X:%02X:%02X", m[0], m[1], m[2], m[3], m[4], m[5]);
  uint32_t ip = 0;
  int rssi = 0;
#ifdef FLP_WIFI
  if (g_net.complete) { ip = g_net.ip; rssi = g_wifi.rssi(); }
#endif
#ifdef FLP_ETH
  if (!ip && g_eth) ip = (uint32_t)g_eth->localIP();
#endif
  String o = "{\"ip\":\"";
  o += IPAddress(ip).toString();
  o += "\",\"mac\":\""; o += mac;
  o += "\",\"rssi\":"; o += String(rssi);
  o += ",\"up\":"; o += String(millis() / 1000);
  o += ",\"chip\":\""; o += ESP.getChipModel();
  o += "\",\"cpu\":"; o += String(ESP.getCpuFreqMHz());
  o += ",\"heap\":"; o += String(ESP.getFreeHeap());
  o += ",\"heapAll\":"; o += String(ESP.getHeapSize());
  o += ",\"flash\":"; o += String(ESP.getFlashChipSize());
  o += ",\"sketch\":"; o += String(ESP.getSketchSize());
  o += ",\"free\":"; o += String(ESP.getFreeSketchSpace());
  o += ",\"build\":\"" __DATE__ " " __TIME__ "\"";
  o += ",\"u\":false}";
  server->send(200, "application/json", o);
}
// Charts store a point of the shown value once per period.
static void webSample() {
  uint32_t now = millis();
  for (int i = 0; i < g_webCount; i++) {
    WebItem &it = g_webItems[i];
    if (!it.hist || (it.hfill && now - it.hlast < it.hperiod)) continue;
    it.hlast = now;
    float v = it.value;
    if (isnan(v) || isinf(v)) v = 0;
    it.hist[it.hpos] = v;
    it.hpos = (it.hpos + 1) % it.hn;
    if (it.hfill < it.hn) it.hfill++;
  }
}

static void webPoll() {
  webSample();
  if (!g_webComplete || g_webBegun) return;
  FLProgNetworkInterface *iface = nullptr;
#ifdef FLP_WIFI
  if ((g_webMask & 1) && g_net.complete) iface = &g_wifi;
#endif
#ifdef FLP_ETH
  if (!iface && (g_webMask & 2) && g_eth) iface = g_eth;
#endif
  if (!iface) return;
  g_web.setBrowserHeaderCompatibility(true);
  g_webBegun = g_web.begin(*iface, 80);
  if (!g_webBegun) return;
  g_web.addHandler("/", webPage);
  g_web.addHandler("/ui/layout", webLayout);
  g_web.addHandler("/ui/state", webState);
  g_web.addHandler("/ui/set", webSet);
  g_web.addHandler("/ui/hist", webHist);
  if (g_webSystem) g_web.addHandler("/ui/info", webInfo);
  if (g_wnOn) {
    g_web.addHandler("/ui/net", webNet);
    g_web.addHandler("/ui/scan", webScan);
  }
#if defined(FLP_WIFI) && defined(FLP_ETH)
  if (iface == &g_wifi && (g_webMask & 2) && g_eth) g_web.addSharedListener(*g_eth);
#endif
}
#endif

static void netPoll() {
  NetSettings &n = g_net;
#ifdef FLP_WIFI
  wifiPoll();
#endif
#ifdef FLP_ETH
  ethPoll();
#endif
  if (n.ntpAsked && !n.ntpOn) {
    FLProgNetworkInterface *iface = nullptr;
#ifdef FLP_WIFI
    if (n.ntpIface == 0 && n.complete) iface = &g_wifi;
#endif
#ifdef FLP_ETH
    if (n.ntpIface == 1 && g_eth) iface = g_eth;
#endif
    if (iface) {
      n.ntpOn = g_ntp.begin(*iface);
      if (n.ntpOn) {
        g_ntp.setServer(n.text[7]);
        g_ntp.setRequestPeriodMs((uint32_t)n.ntpPeriod);
      }
    }
  }
  uint32_t now = millis();
  if (n.ntpOn && (uint32_t)(now - n.tick) >= 1000) {
    n.tick = now;
    // After a failed request the service would wait for the whole period: ask again in 30 s.
    if (g_ntp.failed() && (uint32_t)(now - n.retry) >= 30000) { n.retry = now; g_ntp.syncNow(); }
    n.hasTime = g_ntp.hasTime();
    if (n.hasTime) {
      n.unixTime = g_ntp.unixTime();
      n.syncAge = (uint32_t)(now - g_ntp.lastSyncMillis()) / 1000;
    }
  }
#ifdef FLP_MQTT
  mqttPoll();
#endif
#ifdef FLP_WEB
  webPoll();
#endif
}

// The NTP time shifted by `offset` seconds, broken down.
static bool netTime(int32_t offset, struct tm *lt) {
  if (!g_net.hasTime) return false;
  time_t t = (time_t)((int64_t)g_net.unixTime + offset);
  gmtime_r(&t, lt);
  return true;
}

m3ApiRawFunction(h_net) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, op);
  m3ApiGetArg(int32_t, a);
  m3ApiGetArg(int32_t, b);
  m3ApiGetArg(int32_t, c);
  m3ApiGetArg(int32_t, d);
  NetSettings &n = g_net;
  int32_t r = 0;
  if (op == 0) {
    if (a == 1) n.dhcp = b;
    else if (a == 2 && b > 0) n.ntpPeriod = b;
    else if (a == 3) n.ntpIface = b;
    else if (a == 63) n.complete = true;
    else if (a == 64) n.ntpAsked = true;
  } else if (op == 1) {
    if (a >= 1 && a <= 8 && c >= 0 && inMem(runtime, b, c)) {
      char *t = n.text[a - 1];
      int32_t k = c < 64 ? c : 64;
      memcpy(t, (const uint8_t *)_mem + b, k);
      t[k] = 0;
    }
  } else if (op == 2) {
    n.want = a;
  } else if (op == 3) {
    r = a == 0 ? n.clientReady : a == 1 ? n.apReady : a == 2 ? n.status : a == 3 ? n.error : a == 4 ? (int32_t)n.ip : 0;
  } else if (op == 4) {
    struct tm lt;
    if (a == 0) r = n.hasTime;
    else if (a == 1) r = n.hasTime ? (int32_t)n.syncAge : -1;
    else if (a == 9) r = n.hasTime ? (int32_t)n.unixTime : 0;
    else if (netTime(b, &lt)) {
      r = a == 2 ? lt.tm_hour : a == 3 ? lt.tm_min : a == 4 ? lt.tm_sec : a == 5 ? lt.tm_mday
        : a == 6 ? lt.tm_mon + 1 : a == 7 ? lt.tm_year + 1900 : a == 8 ? (lt.tm_wday == 0 ? 7 : lt.tm_wday) : 0;
    }
  } else if (op == 5) {
    struct tm lt;
    if (b >= 9 && inMem(runtime, a, b) && netTime(c, &lt)) {
      snprintf((char *)_mem + a, 9, "%02d:%02d:%02d", lt.tm_hour, lt.tm_min, lt.tm_sec);
      r = 8;
    }
  } else if (op == 6) {
    if (a >= 1 && a <= 6) n.ethNum[a] = b;
    else if (a == 63) n.ethComplete = true;
  } else if (op == 7) {
    if (a >= 1 && a <= 3 && c >= 0 && inMem(runtime, b, c)) {
      char *t = n.ethText[a - 1];
      int32_t k = c < 64 ? c : 64;
      memcpy(t, (const uint8_t *)_mem + b, k);
      t[k] = 0;
    }
  } else if (op == 8) {
    r = a == 0 ? n.ethReady : a == 1 ? (int32_t)n.ethIp : 0;
#ifdef FLP_MQTT
  } else if (op == 9) {
    if (a >= 1 && a <= 4) g_mqttNum[a] = b;
    else if (a == 63) g_mqttComplete = true;
  } else if (op == 10) {
    if (a >= 1 && a <= 4 && c >= 0 && inMem(runtime, b, c)) {
      char *t = g_mqttText[a - 1];
      int32_t k = c < 64 ? c : 64;
      memcpy(t, (const uint8_t *)_mem + b, k);
      t[k] = 0;
    }
  } else if (op == 11) {
    r = a == 0 ? (g_mqttBegun && g_mqtt.connected()) : a == 1 ? g_mqtt.error() : 0;
  } else if (op == 12 || op == 13) {
    // Queued for the hardware task: topic (a, b) and payload (c, d), at most 128 bytes each.
    if (g_mqttBegun && g_mqtt.connected() && b > 0 && b <= 128 && d >= 0 && d <= 128 && inMem(runtime, a, b) && inMem(runtime, c, d)) {
      int next = (g_mqttHead + 1) % kMqttQueue;
      if (next != g_mqttTail) {
        MqttPublish &p = g_mqttQueue[g_mqttHead];
        memcpy(p.topic, (const uint8_t *)_mem + a, b);
        p.topic[b] = 0;
        memcpy(p.text, (const uint8_t *)_mem + c, d);
        p.text[d] = 0;
        p.len = (uint8_t)d;
        p.retain = op == 13;
        g_mqttHead = next;
        r = 1;
      }
    }
  } else if (op == 14) {
    char filter[129];
    r = -1;
    if (b > 0 && b <= 128 && inMem(runtime, a, b)) {
      memcpy(filter, (const uint8_t *)_mem + a, b);
      filter[b] = 0;
      r = mqttSubscribe(filter);
    }
  } else if (op == 15) {
    if (a >= 0 && a < g_mqttSlotCount && b >= 0 && c > 0 && inMem(runtime, b, c)) {
      MqttSlot &s = g_mqttSlots[a];
      portENTER_CRITICAL(&g_mqttMux);
      int32_t k = s.len < c ? s.len : c;
      memcpy((uint8_t *)_mem + b, s.text, k);
      portEXIT_CRITICAL(&g_mqttMux);
      r = k;
    }
  } else if (op == 16) {
    r = (a >= 0 && a < g_mqttSlotCount) ? (int32_t)g_mqttSlots[a].seq : 0;
#endif
#ifdef FLP_WEB
  } else if (op == 17) {
    if (a == 1 && c > 0 && c < 8192 && inMem(runtime, b, c)) {
      // The layout JSON; the buffer is reused when the module is loaded again.
      if (!g_webLayout || c + 1 > g_webLayoutCap) {
        free(g_webLayout);
        g_webLayout = (char *)malloc(c + 1);
        g_webLayoutCap = g_webLayout ? c + 1 : 0;
      }
      if (g_webLayout) {
        memcpy(g_webLayout, (const uint8_t *)_mem + b, c);
        g_webLayout[c] = 0;
      }
    } else if (a == 2 && c > 0 && c <= kWebMax && inMem(runtime, b, c * 16)) {
      // Table of the widgets, 16 bytes each: kind, rw, dec, reserved, lo, hi, initial (f32).
      const uint8_t *t = (const uint8_t *)_mem + b;
      for (int i = 0; i < c; i++, t += 16) {
        WebItem &it = g_webItems[i];
        it.kind = t[0];
        it.rw = t[1] != 0;
        it.dec = t[2];
        memcpy(&it.lo, t + 4, 4);
        memcpy(&it.hi, t + 8, 4);
        float init;
        memcpy(&init, t + 12, 4);
        it.value = init;
        if (it.kind == 4) {
          // A chart: reserved byte = points, initial = period in seconds; the history survives a module reload of the same size.
          uint16_t n = t[3] < 10 ? 10 : t[3] > 120 ? 120 : t[3];
          if (!it.hist || it.hn != n) {
            free(it.hist);
            it.hist = (float *)calloc(n, sizeof(float));
            it.hn = it.hist ? n : 0;
            it.hpos = it.hfill = 0;
          }
          it.hperiod = (uint32_t)(init < 1 ? 1 : init) * 1000UL;
          it.value = 0;
        }
        if ((it.kind == 1 || it.kind == 2 || it.kind == 3) && !it.text) it.text = (char *)calloc(129, 1);
        if (it.text) it.text[0] = 0;
      }
      g_webCount = c;
    } else if (a == 3) {
      g_webMask = b & 3;
      g_webSystem = (b & 4) != 0;
    } else if (a == 4 && c >= 0 && inMem(runtime, b, c)) {
      int32_t k = c < 64 ? c : 64;
      memcpy(g_webPass, (const uint8_t *)_mem + b, k);
      g_webPass[k] = 0;
    } else if (a == 5 && c > 0 && c < 700 && inMem(runtime, b, c)) {
      // Network form defaults, ten lines; d = widget of the form.
      String t;
      t.reserve(c);
      for (int32_t i = 0; i < c; i++) t += (char)((const uint8_t *)_mem)[b + i];
      wnLoad(t, d);
    } else if (a == 63) {
      g_webComplete = true;
    }
  } else if (op == 19) {
    // Show a number: a = widget, b = f32 bits.
    if (a >= 0 && a < g_webCount) {
      float f;
      memcpy(&f, &b, 4);
      g_webItems[a].value = f;
    }
  } else if (op == 20) {
    // Show a text: a = widget, b, c = pointer and length.
    if (a >= 0 && a < g_webCount && g_webItems[a].text && c >= 0 && inMem(runtime, b, c)) {
      int32_t k = c < 128 ? c : 128;
      portENTER_CRITICAL(&g_webMux);
      memcpy(g_webItems[a].text, (const uint8_t *)_mem + b, k);
      g_webItems[a].text[k] = 0;
      portEXIT_CRITICAL(&g_webMux);
    }
  } else if (op == 21) {
    // The number the page wrote, as f32 bits.
    if (a >= 0 && a < g_webCount) {
      float f = g_webItems[a].value;
      memcpy(&r, &f, 4);
    }
  } else if (op == 22) {
    // The text the page wrote: a = widget, b = pointer, c = capacity; returns the length.
    if (a >= 0 && a < g_webCount && g_webItems[a].text && b >= 0 && c > 0 && inMem(runtime, b, c)) {
      portENTER_CRITICAL(&g_webMux);
      int32_t k = strlen(g_webItems[a].text);
      if (k > c - 1) k = c - 1;
      memcpy((uint8_t *)_mem + b, g_webItems[a].text, k);
      ((uint8_t *)_mem)[b + k] = 0;
      portEXIT_CRITICAL(&g_webMux);
      r = k;
    }
  } else if (op == 23) {
    // Network form: a = 0..9 reads a field as text (b pointer, c capacity; returns the length), 10 the "saved" flag (cleared by the read), 11 DHCP, 12 mode.
    if (a == 10) {
      r = g_wnChanged;
      g_wnChanged = false;
    } else if (a == 11) {
      r = g_wnf[2] != "0";
    } else if (a == 12) {
      r = g_wnf[8].toInt();
    } else if (a >= 0 && a < 10 && b >= 0 && c > 0 && inMem(runtime, b, c)) {
      int32_t k = g_wnf[a].length();
      if (k > c - 1) k = c - 1;
      memcpy((uint8_t *)_mem + b, g_wnf[a].c_str(), k);
      ((uint8_t *)_mem)[b + k] = 0;
      r = k;
    }
#endif
  }
  m3ApiReturn(r);
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
#ifdef FLP_MODBUS
  if (s.perms & P_MODBUS) L("modbus", "i(iiiii)", h_modbus);
#endif
#ifdef FLP_UART
  if (s.perms & P_UART) L("uart", "i(iiii)", h_uart);
#endif
#ifdef FLP_TEXT
  if (s.perms & P_TEXT) L("text", "i(iiiiiii)", h_text);
#endif
#ifdef FLP_NET
  if (s.perms & P_NET) L("net", "i(iiiii)", h_net);
#endif
}

// ---- module lifecycle (call with g_mgmt held) ----
static uint16_t parsePerms(const String &a) {
  uint16_t p = 0;
  if (a.indexOf("gpio_read") >= 0) p |= P_GPIO_READ;
  if (a.indexOf("gpio_write") >= 0) p |= P_GPIO_WRITE;
  if (a.indexOf("adc_read") >= 0) p |= P_ADC;
  if (a.indexOf("pwm_write") >= 0) p |= P_PWM;
  if (a.indexOf("millis") >= 0) p |= P_MILLIS;
  if (a.indexOf("sensor_read") >= 0) p |= P_SENSOR;
  if (a.indexOf("pcf_read") >= 0) p |= P_PCF_READ;
  if (a.indexOf("pcf_write") >= 0) p |= P_PCF_WRITE;
  if (a.indexOf("modbus") >= 0) p |= P_MODBUS;
  if (a.indexOf("uart") >= 0) p |= P_UART;
  if (a.indexOf("text") >= 0) p |= P_TEXT;
  if (a.indexOf("net") >= 0) p |= P_NET;
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
  delay(2);
}
