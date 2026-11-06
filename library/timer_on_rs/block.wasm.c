#ifndef m11_timerX5FonX5FrsHeader
#define m11_timerX5FonX5FrsHeader

#ifdef __cplusplus
extern "C" {
#endif


#if W2C2_RUNTIME_CHECKS
extern const wasmFuncType m11_timerX5FonX5FrsFuncType0;
extern const wasmFuncType m11_timerX5FonX5FrsFuncType1;
extern const wasmFuncType m11_timerX5FonX5FrsFuncType2;
extern const wasmFuncType m11_timerX5FonX5FrsFuncType3;
#endif

typedef struct m11_timerX5FonX5FrsInstance {
wasmModuleInstance common;
wasmMemory* m0;
} m11_timerX5FonX5FrsInstance;

F64 i4_crab5_input(wasmModuleInstance*,U32);

F64 i4_crab5_param(wasmModuleInstance*,U32);

void i4_crab6_output(wasmModuleInstance*,U32,F64);

void timer_on_rs_f3(wasmModuleInstance*);

void timer_on_rs_f4(wasmModuleInstance*,U32);

wasmMemory*m11_timerX5FonX5FrsExport6_memory(m11_timerX5FonX5FrsInstance* i);

void m11_timerX5FonX5FrsExport4_init(m11_timerX5FonX5FrsInstance*i);

void m11_timerX5FonX5FrsExport4_step(m11_timerX5FonX5FrsInstance*i,U32 l0);

void m11_timerX5FonX5FrsInstantiate(m11_timerX5FonX5FrsInstance* instance, wasmImportResolver resolve);

void m11_timerX5FonX5FrsFreeInstance(m11_timerX5FonX5FrsInstance* instance);

#ifdef __cplusplus
}
#endif

#endif /* m11_timerX5FonX5FrsHeader */



void timer_on_rs_f3(wasmModuleInstance* instance) {
m11_timerX5FonX5FrsInstance* i = (m11_timerX5FonX5FrsInstance*)instance;
U32 si0,si1;
U64 sj1;
si0=0U;
sj1=W2C2_LL(0U);
i64_store(i->m0,(WasmMemoryAddress)si0+1024U,sj1);
si0=0U;
si1=0U;
i32_store8(i->m0,(WasmMemoryAddress)si0+1032U,si1);
L0:;
}

void timer_on_rs_f4(wasmModuleInstance* instance,U32 l0) {
m11_timerX5FonX5FrsInstance* i = (m11_timerX5FonX5FrsInstance*)instance;
F64 l1=0;
U32 si0,si1,si2,si3;
F64 sd0,sd1,sd2,sd3;
sd0=f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
l1=sd0;
si0=0U;
sd0=i4_crab5_input(instance,si0);
sd1=f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
si0=sd0 != sd1;
if(si0){
goto L2;
}
si0=0U;
si1=0U;
i32_store8(i->m0,(WasmMemoryAddress)si0+1032U,si1);
goto L1;
L2:;
si0=0U;
si0=i32_load8_u(i->m0,(WasmMemoryAddress)si0+1032U);
if(si0){
goto L4;
}
si0=0U;
si1=1U;
i32_store8(i->m0,(WasmMemoryAddress)si0+1032U,si1);
sd0=f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
l1=sd0;
goto L3;
L4:;
si0=0U;
sd0=f64_load(i->m0,(WasmMemoryAddress)si0+1024U);
si1=l0;
sd1=(F64)(I32)(si1);
sd0+=sd1;
l1=sd0;
L3:;
si0=0U;
sd1=l1;
f64_store(i->m0,(WasmMemoryAddress)si0+1024U,sd1);
sd0=f64_reinterpret_i64(W2C2_LL(0x3FF0000000000000U));
sd1=f64_reinterpret_i64(W2C2_LL(0x0000000000000000U));
sd2=l1;
si3=0U;
sd3=i4_crab5_param(instance,si3);
si2=sd2 >= sd3;
sd0=si2?sd0:sd1;
l1=sd0;
L1:;
si0=0U;
sd1=l1;
i4_crab6_output(instance,si0,sd1);
L0:;
}

#if W2C2_RUNTIME_CHECKS
const wasmFuncType m11_timerX5FonX5FrsFuncType0={1,"i",1,"d"};
const wasmFuncType m11_timerX5FonX5FrsFuncType1={2,"id",0,""};
const wasmFuncType m11_timerX5FonX5FrsFuncType2={0,"",0,""};
const wasmFuncType m11_timerX5FonX5FrsFuncType3={1,"i",0,""};
#endif

wasmFuncExport m11_timerX5FonX5FrsFuncExports[3] = {
{(wasmFunc)timer_on_rs_f3,{"init",4}},
{(wasmFunc)timer_on_rs_f4,{"step",4}},
{NULL,{NULL,0}}
};

static void m11_timerX5FonX5FrsInitMemories(m11_timerX5FonX5FrsInstance* i, m11_timerX5FonX5FrsInstance* parent) {
i->m0 = wasmMemoryAllocate(1, 1, false);
}

static void m11_timerX5FonX5FrsInitImports(m11_timerX5FonX5FrsInstance* i, wasmImportResolver resolve) {
if (resolve == NULL) { return; }
}

wasmMemory*m11_timerX5FonX5FrsExport6_memory(m11_timerX5FonX5FrsInstance* i){
return i->m0;
}

void m11_timerX5FonX5FrsExport4_init(m11_timerX5FonX5FrsInstance*i){
timer_on_rs_f3(&i->common);
}

void m11_timerX5FonX5FrsExport4_step(m11_timerX5FonX5FrsInstance*i,U32 l0){
timer_on_rs_f4(&i->common,l0);
}

static void m11_timerX5FonX5FrsFreeChild(wasmModuleInstance* child) {
m11_timerX5FonX5FrsInstance* i = (m11_timerX5FonX5FrsInstance*)child;
wasmMemoryFree(i->m0);
i->m0=NULL;
free(i);
}

static wasmModuleInstance* m11_timerX5FonX5FrsNewChild(wasmModuleInstance* instance) {
m11_timerX5FonX5FrsInstance* self = (m11_timerX5FonX5FrsInstance*)instance;
m11_timerX5FonX5FrsInstance* child = (m11_timerX5FonX5FrsInstance*)calloc(1, sizeof(m11_timerX5FonX5FrsInstance));
child->common.funcExports = self->common.funcExports;
child->common.resolveImports = self->common.resolveImports;
child->common.newChild = self->common.newChild;
child->common.freeChild = self->common.freeChild;
m11_timerX5FonX5FrsInitImports(child, self->common.resolveImports);
m11_timerX5FonX5FrsInitMemories(child, self);
return &child->common;
}

void m11_timerX5FonX5FrsInstantiate(m11_timerX5FonX5FrsInstance* i, wasmImportResolver resolveImports) {
i->common.funcExports = m11_timerX5FonX5FrsFuncExports;
i->common.resolveImports = resolveImports;
i->common.newChild = m11_timerX5FonX5FrsNewChild;
i->common.freeChild = m11_timerX5FonX5FrsFreeChild;
m11_timerX5FonX5FrsInitImports(i, resolveImports);
m11_timerX5FonX5FrsInitMemories(i, NULL);
}

void m11_timerX5FonX5FrsFreeInstance(m11_timerX5FonX5FrsInstance* i) {
wasmMemoryFree(i->m0);
i->m0=NULL;
}

