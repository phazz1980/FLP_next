// Baseline: same TON written directly in C++ (the shape of esp32.cpp.tmpl).
static double g_in, g_param = 1000, g_out;
static double elapsed = 0;
static void step(int dt) {
  if (g_in != 0.0) { elapsed += dt; g_out = elapsed >= g_param ? 1.0 : 0.0; }
  else { elapsed = 0; g_out = 0; }
}
void setup() { Serial.begin(115200); }
void loop() {
  g_in = 1;
  uint32_t t0 = micros();
  for (int k = 0; k < 1000; k++) step(1);
  uint32_t dt = micros() - t0;
  Serial.printf("out=%d, 1000 steps: %lu us\n", (int)g_out, (unsigned long)dt);
  g_in = 0; step(1);
  delay(1000);
}
