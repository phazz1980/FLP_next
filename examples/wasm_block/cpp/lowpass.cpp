// Simulation module of the `lowpass` block in C++: exponential smoothing.
// No standard library, no WASI: only the imports of the "flp" module.
#define FLP(name) __attribute__((import_module("flp"), import_name(#name)))
#define EXPORT(name) __attribute__((export_name(#name)))

extern "C" {
FLP(input) double input(int index);
FLP(param) double param(int index);
FLP(output) void output(int index, double value);

static double y = 0.0;

EXPORT(init) void init() { y = 0.0; }

EXPORT(step) void step(int dt_ms) {
  double tau = param(0);
  double dt = dt_ms;
  y += dt / (tau + dt) * (input(0) - y);
  output(0, y);
}
}
