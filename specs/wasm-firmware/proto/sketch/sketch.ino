// Prototype: TON compiled from Rust -> WASM -> C (w2c2), driven from a sketch.
extern "C" {
#include "ton.h"
}

static double g_in[1], g_param[1], g_out[1];

extern "C" F64 i4_crab5_input(wasmModuleInstance*, U32 i) { return g_in[i]; }
extern "C" F64 i4_crab5_param(wasmModuleInstance*, U32 i) { return g_param[i]; }
extern "C" void i4_crab6_output(wasmModuleInstance*, U32 i, F64 v) { g_out[i] = v; }

static m5_blockInstance ton;

void setup() {
  Serial.begin(115200);
  g_param[0] = 1000;
  m5_blockInstantiate(&ton, NULL);
  m5_blockExport4_init(&ton);
}

void loop() {
  g_in[0] = 1;
  uint32_t t0 = micros();
  for (int k = 0; k < 1000; k++) m5_blockExport4_step(&ton, 1);
  uint32_t dt = micros() - t0;
  Serial.printf("out=%d, 1000 steps: %lu us\n", (int)g_out[0], (unsigned long)dt);
  g_in[0] = 0;
  m5_blockExport4_step(&ton, 1);
  delay(1000);
}
