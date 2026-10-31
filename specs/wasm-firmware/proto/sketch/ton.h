#ifndef m5_blockHeader
#define m5_blockHeader

#ifdef __cplusplus
extern "C" {
#endif

#include "w2c2_base.h"

#if W2C2_RUNTIME_CHECKS
extern const wasmFuncType m5_blockFuncType0;
extern const wasmFuncType m5_blockFuncType1;
extern const wasmFuncType m5_blockFuncType2;
extern const wasmFuncType m5_blockFuncType3;
#endif

typedef struct m5_blockInstance {
  wasmModuleInstance common;
  wasmMemory* m0;
} m5_blockInstance;

  F64 i3_flp5_input(wasmModuleInstance*, U32);

  F64 i3_flp5_param(wasmModuleInstance*, U32);

  void i3_flp6_output(wasmModuleInstance*, U32, F64);

void f3(wasmModuleInstance*);

void f4(wasmModuleInstance*, U32);

wasmMemory* m5_blockExport6_memory(m5_blockInstance* i);

void m5_blockExport4_init(m5_blockInstance* i);

void m5_blockExport4_step(m5_blockInstance* i, U32 l0);

void m5_blockInstantiate(m5_blockInstance* instance, wasmImportResolver resolve);

void m5_blockFreeInstance(m5_blockInstance* instance);

#ifdef __cplusplus
}
#endif

#endif /* m5_blockHeader */

