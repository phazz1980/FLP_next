#include "w2c2_base.h"

#include "ton.h"

void f3(wasmModuleInstance* instance) {
  m5_blockInstance* i = (m5_blockInstance*)instance;
  U32 si0;
  U64 sj1;
  si0 = 0U;
  sj1 = W2C2_LL(0U);
  i64_store(i->m0, (WasmMemoryAddress)si0 + 1024U, sj1);
  L0:;
}

void f4(wasmModuleInstance* instance, U32 l0) {
  m5_blockInstance* i = (m5_blockInstance*)instance;
  F64 l1 = 0;
  U32 si0, si1, si2, si3;
  U64 sj1;
  F64 sd0, sd1, sd2, sd3;
  sd0 = f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
  l1 = sd0;
  {
    {
      si0 = 0U;
      sd0 = i4_crab5_input(instance, si0);
      sd1 = f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
      si0 = sd0 != sd1;
      if (si0) {
        goto L2;
      }
      si0 = 0U;
      sj1 = W2C2_LL(0U);
      i64_store(i->m0, (WasmMemoryAddress)si0 + 1024U, sj1);
      goto L1;
    }
    L2:;
    si0 = 0U;
    si1 = 0U;
    sd1 = f64_load(i->m0, (WasmMemoryAddress)si1 + 1024U);
    si2 = l0;
    sd2 = (F64)(I32)(si2);
    sd1 += sd2;
    l1 = sd1;
    f64_store(i->m0, (WasmMemoryAddress)si0 + 1024U, sd1);
    sd0 = f64_reinterpret_i64(W2C2_LL(0x3FF0000000000000U));
    sd1 = f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
    sd2 = l1;
    si3 = 0U;
    sd3 = i4_crab5_param(instance, si3);
    si2 = sd2 >= sd3;
    sd0 = si2 ? sd0 : sd1;
    l1 = sd0;
  }
  L1:;
  si0 = 0U;
  sd1 = l1;
  i4_crab6_output(instance, si0, sd1);
  L0:;
}

#if W2C2_RUNTIME_CHECKS
const wasmFuncType m5_blockFuncType0 = {1, "i", 1, "d"};
const wasmFuncType m5_blockFuncType1 = {2, "id", 0, ""};
const wasmFuncType m5_blockFuncType2 = {0, "", 0, ""};
const wasmFuncType m5_blockFuncType3 = {1, "i", 0, ""};
#endif

wasmFuncExport m5_blockFuncExports[3] = {
{(wasmFunc)f3,{"init",4}},
{(wasmFunc)f4,{"step",4}},
{NULL,{NULL,0}}
};

static void m5_blockInitMemories(m5_blockInstance* i, m5_blockInstance* parent) {
  i->m0 = wasmMemoryAllocate(1, 1, false);
}

static void m5_blockInitImports(m5_blockInstance* i, wasmImportResolver resolve) {
  if (resolve == NULL) { return; }
}

wasmMemory* m5_blockExport6_memory(m5_blockInstance* i) {
  return i->m0;
}

void m5_blockExport4_init(m5_blockInstance* i) {
  f3(&i->common);
}

void m5_blockExport4_step(m5_blockInstance* i, U32 l0) {
  f4(&i->common, l0);
}

static void m5_blockFreeChild(wasmModuleInstance* child) {
  m5_blockInstance* i = (m5_blockInstance*)child;
  wasmMemoryFree(i->m0);
  i->m0 = NULL;
  free(i);
}

static wasmModuleInstance* m5_blockNewChild(wasmModuleInstance* instance) {
  m5_blockInstance* self = (m5_blockInstance*)instance;
  m5_blockInstance* child = (m5_blockInstance*)calloc(1, sizeof(m5_blockInstance));
  child->common.funcExports = self->common.funcExports;
  child->common.resolveImports = self->common.resolveImports;
  child->common.newChild = self->common.newChild;
  child->common.freeChild = self->common.freeChild;
  m5_blockInitImports(child, self->common.resolveImports);
  m5_blockInitMemories(child, self);
  return &child->common;
}

void m5_blockInstantiate(m5_blockInstance* i, wasmImportResolver resolveImports) {
  i->common.funcExports = m5_blockFuncExports;
  i->common.resolveImports = resolveImports;
  i->common.newChild = m5_blockNewChild;
  i->common.freeChild = m5_blockFreeChild;
  m5_blockInitImports(i, resolveImports);
  m5_blockInitMemories(i, NULL);
}

void m5_blockFreeInstance(m5_blockInstance* i) {
  wasmMemoryFree(i->m0);
  i->m0 = NULL;
}

