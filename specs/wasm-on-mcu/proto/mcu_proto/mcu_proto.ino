// Prototype: wasm3 on ESP32, blink module via crab.gpio_write / crab.millis.
// Prints RAM per module, step time, hot-swap time and heap fragmentation.
#include <wasm3.h>
#include <m3_env.h>
#include "blink_wasm.h"

static const int kStackWords = 2048;

m3ApiRawFunction(m3_gpio_write) {
  m3ApiGetArg(int32_t, pin);
  m3ApiGetArg(int32_t, value);
  digitalWrite(pin, value);
  m3ApiSuccess();
}

m3ApiRawFunction(m3_millis) {
  m3ApiReturnType(int32_t);
  m3ApiReturn((int32_t)millis());
}


static void report(const char *tag) {
  Serial.printf("%s: free=%u largest=%u min_free=%u\n", tag, ESP.getFreeHeap(),
                (unsigned)heap_caps_get_largest_free_block(MALLOC_CAP_8BIT),
                ESP.getMinFreeHeap());
}

struct Slot {
  IM3Environment env = nullptr;
  IM3Runtime rt = nullptr;
  IM3Function step = nullptr;
  bool load() {
    env = m3_NewEnvironment();
    if (!env) return false;
    rt = m3_NewRuntime(env, kStackWords, nullptr);
    if (!rt) return false;
    IM3Module mod;
    if (m3_ParseModule(env, &mod, blink_wasm, blink_wasm_len)) return false;
    if (m3_LoadModule(rt, mod)) return false;
    m3_LinkRawFunction(mod, "crab", "gpio_write", "v(ii)", &m3_gpio_write);
    m3_LinkRawFunction(mod, "crab", "millis", "i()", &m3_millis);
    return !m3_FindFunction(&step, rt, "step");
  }
  void unload() {
    if (rt) m3_FreeRuntime(rt);
    if (env) m3_FreeEnvironment(env);
    env = nullptr; rt = nullptr; step = nullptr;
  }
};

Slot g_mod;

void setup() {
  Serial.begin(115200);
  delay(1500);
  pinMode(2, OUTPUT);
  Serial.println("== wasm3 prototype ==");
  report("baseline");
  uint32_t before = ESP.getFreeHeap();
  if (!g_mod.load()) { Serial.println("load failed"); return; }
  Serial.printf("RAM per module: %u bytes\n", before - ESP.getFreeHeap());
  report("loaded");

  // step time (millis() check inside the module makes most calls cheap)
  const int n = 20000;
  uint32_t t0 = micros();
  for (int i = 0; i < n; i++) m3_CallV(g_mod.step, (int32_t)0);
  Serial.printf("step: %.2f us\n", (micros() - t0) / (float)n);

  // hot swap: single and 1000x, then fragmentation
  uint32_t s0 = micros();
  g_mod.unload();
  g_mod.load();
  Serial.printf("hot swap: %u us\n", micros() - s0);
  report("before 1000 swaps");
  for (int i = 0; i < 1000; i++) { g_mod.unload(); if (!g_mod.load()) { Serial.printf("swap %d failed\n", i); break; } }
  report("after 1000 swaps");
}

void loop() {
  if (g_mod.step) m3_CallV(g_mod.step, (int32_t)0);
  delay(1);
}
