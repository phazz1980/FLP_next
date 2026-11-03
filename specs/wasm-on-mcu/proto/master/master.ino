// Prototype master firmware: wasm3 modules from LittleFS, one FreeRTOS task per module,
// watchdog, journal and hot swap over Serial. See specs/wasm-on-mcu/plan.md.
//
// Serial protocol (115200, lines end with \n):
//   put <name> <len> <crc32 hex> <allow,list|->   then <len> raw bytes -> stores and (re)starts the module
//   in|param <name> <i> <value>      set an input / parameter (module tests)
//   reset <name>                     restart the module paused, inputs and outputs zero
//   tick <name> <n> <dt_ms>          pause the module and run n steps now
//   out <name> <n>                   first n outputs as f64 bits in hex
//   swap <name>      reload the module from LittleFS      list      stats      log      del <name>
#include <LittleFS.h>
#include <esp_crc.h>
#include <wasm3.h>
#include <m3_env.h>

static const int kMaxSlots = 4;
static const int kStackWords = 2048;
static const uint32_t kStepPeriodMs = 10;
static const uint32_t kStepLimitMs = 100;  // watchdog: a longer step means a hang
static const int kJournal = 50;

enum { P_GPIO_READ = 1, P_GPIO_WRITE = 2, P_ADC = 4, P_PWM = 8, P_MILLIS = 16 };

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
  double outputs[8], inputs[8], params[8];
};

static Slot g_slots[kMaxSlots];
static SemaphoreHandle_t g_mgmt;
static double g_tags[64];
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

m3ApiRawFunction(h_input) {
  m3ApiReturnType(double);
  m3ApiGetArg(int32_t, i);
  m3ApiReturn((i >= 0 && i < 8) ? slotOf(_ctx)->inputs[i] : 0.0);
}
m3ApiRawFunction(h_param) {
  m3ApiReturnType(double);
  m3ApiGetArg(int32_t, i);
  m3ApiReturn((i >= 0 && i < 8) ? slotOf(_ctx)->params[i] : 0.0);
}
m3ApiRawFunction(h_output) {
  m3ApiGetArg(int32_t, i);
  m3ApiGetArg(double, v);
  if (i >= 0 && i < 8) slotOf(_ctx)->outputs[i] = v;
  m3ApiSuccess();
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
m3ApiRawFunction(h_tag_get) {
  m3ApiReturnType(double);
  m3ApiGetArg(int32_t, id);
  m3ApiReturn((id >= 0 && id < 64) ? g_tags[id] : 0.0);
}
m3ApiRawFunction(h_tag_set) {
  m3ApiGetArg(int32_t, id);
  m3ApiGetArg(double, v);
  if (id >= 0 && id < 64) g_tags[id] = v;
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
m3ApiRawFunction(h_gpio_read) {
  m3ApiReturnType(int32_t);
  m3ApiGetArg(int32_t, pin);
  m3ApiReturn(digitalRead(pin));
}
m3ApiRawFunction(h_gpio_write) {
  m3ApiGetArg(int32_t, pin);
  m3ApiGetArg(int32_t, v);
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
  m3ApiGetArg(int32_t, v);
  analogWrite(pin, v);
  m3ApiSuccess();
}
m3ApiRawFunction(h_millis) {
  m3ApiReturnType(int32_t);
  m3ApiReturn((int32_t)millis());
}

static void linkAll(Slot &s, IM3Module m) {
  auto L = [&](const char *n, const char *sig, M3RawCall f) { m3_LinkRawFunctionEx(m, "flp", n, sig, f, &s); };
  L("input", "F(i)", h_input);
  L("param", "F(i)", h_param);
  L("output", "v(iF)", h_output);
  L("log", "v(ii)", h_log);
  L("tag_get", "F(i)", h_tag_get);
  L("tag_set", "v(iF)", h_tag_set);
  L("publish", "v(iiii)", h_publish);
  L("subscribe", "v(ii)", h_subscribe);
  if (s.perms & P_GPIO_READ) L("gpio_read", "i(i)", h_gpio_read);
  if (s.perms & P_GPIO_WRITE) L("gpio_write", "v(ii)", h_gpio_write);
  if (s.perms & P_ADC) L("adc_read", "i(i)", h_adc_read);
  if (s.perms & P_PWM) L("pwm_write", "v(ii)", h_pwm_write);
  if (s.perms & P_MILLIS) L("millis", "i()", h_millis);
}

// ---- module lifecycle (call with g_mgmt held) ----
static uint8_t parsePerms(const String &a) {
  uint8_t p = 0;
  if (a.indexOf("gpio_read") >= 0) p |= P_GPIO_READ;
  if (a.indexOf("gpio_write") >= 0) p |= P_GPIO_WRITE;
  if (a.indexOf("adc_read") >= 0) p |= P_ADC;
  if (a.indexOf("pwm_write") >= 0) p |= P_PWM;
  if (a.indexOf("millis") >= 0) p |= P_MILLIS;
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
  if (m3_FindFunction(&av, s.rt, "abi_version") || m3_CallV(av) || m3_GetResultsV(av, &ver) || ver != 2) {
    jlog(s.name, "нужен abi_version() = 2 (получено %d)", (int)ver);
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
  xTaskCreatePinnedToCore(slotTask, s.name, 6144, &s, 1, &s.task, 1);
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
  char cmd[8], name[32];
  int idx = 0;
  double val = 0;
  unsigned n = 0, dt = 0;
  Slot *s = nullptr;
  if (sscanf(line.c_str(), "%7s %31s", cmd, name) == 2) s = findSlot(name, false);
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
    if (sscanf(line.c_str(), "%*s %*s %d %lf", &idx, &val) != 2 || idx < 0 || idx >= 8) {
      Serial.println("err bad index");
      return;
    }
    (cmd[0] == 'i' ? s->inputs : s->params)[idx] = val;
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
    if (sscanf(line.c_str(), "%*s %*s %u", &n) != 1 || n > 8) {
      Serial.println("err bad count");
      return;
    }
    Serial.print("out");
    for (unsigned i = 0; i < n; i++) {
      uint64_t bits;
      memcpy(&bits, &s->outputs[i], 8);
      Serial.printf(" %08x%08x", (unsigned)(bits >> 32), (unsigned)bits);
    }
    Serial.println();
    Serial.println("ok");
  }
}

static void cmdLine(String line) {
  line.trim();
  if (line.startsWith("in ") || line.startsWith("param ") || line.startsWith("tick ") || line.startsWith("out ") || line.startsWith("reset ")) {
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
        Serial.printf("%s %s steps=%u restarts=%u out0=%.1f\n", s.name, s.task ? "run" : "stop", (unsigned)s.steps, (unsigned)s.restarts, s.outputs[0]);
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

void setup() {
  Serial.begin(115200);
  Serial.setTimeout(200);
  g_mgmt = xSemaphoreCreateMutex();
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
  xTaskCreatePinnedToCore(watchdogTask, "wdt", 4096, nullptr, 3, nullptr, 1);  // same core + higher priority than modules: deleting a hung task never crosses cores
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
