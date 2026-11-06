//! Native parts of blocks inside the master firmware (`specs/drivers`): the glue code
//! around each `native.cpp`.
//!
//! A module of a sheet calls the import `crab.native(kind, inst, in_ptr, out_ptr)`. The
//! words (4 bytes: `bool` and `int` as `i32`, `float` as `f32` bits) at `in_ptr` are the
//! inputs followed by the parameters of the block; the outputs are written as words to
//! `out_ptr`. `kind` is the index of the block type in [`MasterFeatures::natives`] (sorted
//! by type name), `inst` is the instance of that type in the project.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crab_core::library::{Library, LibraryBlock, ParamKind, SignalType};

/// A type that crosses into native code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeType {
    Bool,
    Int,
    Float,
}

impl NativeType {
    fn cpp(self) -> &'static str {
        match self {
            NativeType::Bool => "bool",
            NativeType::Int => "int32_t",
            NativeType::Float => "float",
        }
    }

    /// C++ that reads a word at `p` as this type.
    fn read(self, p: &str) -> String {
        match self {
            NativeType::Bool => format!("crabNatI({p}) != 0"),
            NativeType::Int => format!("crabNatI({p})"),
            NativeType::Float => format!("crabNatF({p})"),
        }
    }

    /// C++ statement that writes `value` as a word to `q`.
    fn write(self, value: &str, q: &str) -> String {
        match self {
            NativeType::Bool => format!("crabNatPutI({q}, {value} ? 1 : 0);"),
            NativeType::Int => format!("crabNatPutI({q}, {value});"),
            NativeType::Float => format!("crabNatPutF({q}, {value});"),
        }
    }
}

/// One block type with a native part, as many times as the project uses it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NativeBlock {
    pub kind: String,
    /// `native.cpp` as written.
    pub source: String,
    /// Arduino libraries to install.
    pub libs: Vec<String>,
    pub has_init: bool,
    pub has_state: bool,
    pub inputs: Vec<(String, NativeType)>,
    pub outputs: Vec<(String, NativeType)>,
    pub params: Vec<(String, NativeType)>,
    /// How many blocks of this type the project has: each one gets its own `State`.
    pub instances: u32,
}

fn signal(ty: SignalType) -> Option<NativeType> {
    match ty {
        SignalType::Bool => Some(NativeType::Bool),
        SignalType::Int => Some(NativeType::Int),
        SignalType::Float => Some(NativeType::Float),
        _ => None,
    }
}

fn param(kind: ParamKind) -> Option<NativeType> {
    match kind {
        ParamKind::Bool => Some(NativeType::Bool),
        ParamKind::Int | ParamKind::Pin | ParamKind::Duration => Some(NativeType::Int),
        ParamKind::Float => Some(NativeType::Float),
        ParamKind::Text => None,
    }
}

impl NativeBlock {
    /// The native part of a library block, or `None` for a block without one.
    pub fn from_library(block: &LibraryBlock, instances: u32) -> crab_core::Result<Option<Self>> {
        let Some(path) = block.native_path() else {
            return Ok(None);
        };
        let source = std::fs::read_to_string(&path).map_err(|source| crab_core::Error::Read {
            path: path.clone(),
            source,
        })?;
        Self::from_parts(&block.def, &source, instances)
            .map(Some)
            .map_err(|message| crab_core::Error::InvalidNative { path, message })
    }

    /// The native part from a definition and the text of `native.cpp` (a block being edited);
    /// the error is the Russian text of the problems found.
    pub fn from_parts(
        def: &crab_core::BlockDef,
        source: &str,
        instances: u32,
    ) -> Result<Self, String> {
        let part = crab_core::native::parse_native(source, def)
            .map_err(|errors| crab_core::native::join_errors(&errors))?;
        let ports = |list: &[crab_core::PortDef]| -> Vec<(String, NativeType)> {
            list.iter()
                .filter_map(|p| signal(p.ty).map(|t| (p.name.clone(), t)))
                .collect()
        };
        Ok(Self {
            kind: def.info.kind.clone(),
            source: source.to_owned(),
            libs: part.libs,
            has_init: part.has_init,
            has_state: part.has_state,
            inputs: ports(&def.inputs),
            outputs: ports(&def.outputs),
            params: def
                .params
                .iter()
                .filter_map(|p| param(p.kind).map(|t| (p.name.clone(), t)))
                .collect(),
            instances: instances.max(1),
        })
    }

    /// Words a module hands over: the inputs, then the parameters.
    pub fn in_words(&self) -> usize {
        self.inputs.len() + self.params.len()
    }
}

/// The native parts the given block types (one item per block of the project) need, sorted by
/// type name: the position in the result is the `kind` the module passes to `crab.native`.
pub fn collect_natives<'a>(
    library: &Library,
    kinds: impl IntoIterator<Item = &'a str>,
) -> crab_core::Result<Vec<NativeBlock>> {
    let mut count: BTreeMap<&str, u32> = BTreeMap::new();
    for kind in kinds {
        *count.entry(kind).or_default() += 1;
    }
    let mut out = Vec::new();
    for (kind, n) in count {
        if let Some(block) = library.get(kind) {
            out.extend(NativeBlock::from_library(block, n)?);
        }
    }
    Ok(out)
}

/// `kind` for `crab.native`: the index of the block type among the natives.
pub fn native_kind(natives: &[NativeBlock], kind: &str) -> Option<usize> {
    natives.iter().position(|n| n.kind == kind)
}

/// C++ for the master sketch: the native parts, each in its own namespace, and the two functions
/// the `native` import calls (`crab_native::words`, `crab_native::call`). All of it is inside
/// `namespace crab_native`: the Arduino preprocessor makes prototypes for global functions and
/// puts them before the sketch's types. Empty without natives.
pub fn glue(natives: &[NativeBlock]) -> String {
    if natives.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "// ---- native parts of blocks (generated from native.cpp, see specs/drivers) ----\n",
    );
    // Headers go to the top: a library header must not end up inside a namespace.
    let mut seen = BTreeSet::new();
    for n in natives {
        for line in n.source.lines() {
            let line = line.trim();
            if line.starts_with("#include") && seen.insert(line.to_owned()) {
                let _ = writeln!(out, "{line}");
            }
        }
    }
    out.push_str(
        "#define CRAB_INIT(...) static void crab_init(__VA_ARGS__)
#define CRAB_STEP(...) static void crab_step(__VA_ARGS__)
namespace crab_native {
static inline int32_t crabNatI(const uint8_t *p) { int32_t v; memcpy(&v, p, 4); return v; }
static inline float crabNatF(const uint8_t *p) { float v; memcpy(&v, p, 4); return v; }
static inline void crabNatPutI(uint8_t *p, int32_t v) { memcpy(p, &v, 4); }
static inline void crabNatPutF(uint8_t *p, float v) { memcpy(p, &v, 4); }
",
    );
    for (i, n) in natives.iter().enumerate() {
        block(&mut out, i, n);
    }
    // The two entry points of the `native` import.
    out.push_str(
        "static bool words(int32_t kind, int32_t *nin, int32_t *nout) {\n  switch (kind) {\n",
    );
    for (i, n) in natives.iter().enumerate() {
        let _ = writeln!(
            out,
            "    case {i}: *nin = {}; *nout = {}; return true;",
            n.in_words(),
            n.outputs.len()
        );
    }
    out.push_str("  }\n  return false;\n}\n");
    out.push_str("static int32_t call(int32_t kind, int32_t inst, const uint8_t *in, uint8_t *out) {\n  switch (kind) {\n");
    for i in 0..natives.len() {
        let _ = writeln!(
            out,
            "    case {i}: return crab_nat_{i}::call(inst, in, out);"
        );
    }
    out.push_str("  }\n  return 0;\n}\n}  // namespace crab_native\n");
    out
}

/// One block: `namespace crab_nat_<i> { In, Out, Params, State, the user code, call }`.
fn block(out: &mut String, i: usize, n: &NativeBlock) {
    let fields = |list: &[(String, NativeType)]| -> String {
        list.iter()
            .map(|(name, ty)| format!(" {} {name};", ty.cpp()))
            .collect()
    };
    let _ = writeln!(out, "// block `{}`, {} instance(s)", n.kind, n.instances);
    let _ = writeln!(out, "namespace crab_nat_{i} {{");
    let _ = writeln!(out, "struct In {{{} }};", fields(&n.inputs));
    let _ = writeln!(out, "struct Out {{{} }};", fields(&n.outputs));
    let _ = writeln!(out, "struct Params {{{} }};", fields(&n.params));
    if !n.has_state {
        out.push_str("struct State {};\n");
    }
    // The headers are already at the top; the line stays so that line numbers hold.
    for line in n.source.lines() {
        if line.trim_start().starts_with("#include") {
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    let np = n.params.len();
    let _ = writeln!(out, "static State states[{}];", n.instances);
    let _ = writeln!(out, "static bool ready[{}];", n.instances);
    let _ = writeln!(
        out,
        "static uint8_t lastParams[{}][{}];",
        n.instances,
        (np * 4).max(1)
    );
    out.push_str("static int32_t call(int32_t inst, const uint8_t *w, uint8_t *o) {\n");
    let _ = writeln!(out, "  if (inst < 0 || inst >= {}) return 0;", n.instances);
    out.push_str("  In in{}; Params p{};\n  const uint8_t *r = w;\n");
    for (name, ty) in &n.inputs {
        let _ = writeln!(out, "  in.{name} = {}; r += 4;", ty.read("r"));
    }
    out.push_str("  const uint8_t *pw = r;\n");
    for (name, ty) in &n.params {
        let _ = writeln!(out, "  p.{name} = {}; r += 4;", ty.read("r"));
    }
    // The parameters arrive with every step: init runs at the first one and when they change.
    let _ = writeln!(
        out,
        "  bool changed = !ready[inst]{};",
        if np > 0 {
            format!(" || memcmp(lastParams[inst], pw, {}) != 0", np * 4)
        } else {
            String::new()
        }
    );
    out.push_str("  if (changed) {\n    ready[inst] = true;\n");
    if np > 0 {
        let _ = writeln!(out, "    memcpy(lastParams[inst], pw, {});", np * 4);
    }
    if n.has_init {
        out.push_str("    crab_init(states[inst], p);\n");
    }
    out.push_str("  }\n  Out out{};\n  crab_step(states[inst], in, out);\n  uint8_t *q = o;\n");
    for (name, ty) in &n.outputs {
        let _ = writeln!(out, "  {} q += 4;", ty.write(&format!("out.{name}"), "q"));
    }
    out.push_str("  return 1;\n}\n}\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp(instances: u32) -> NativeBlock {
        NativeBlock {
            kind: "bmp".into(),
            source: "// crab:native libs=\"Adafruit BMP280\"\n#include <Adafruit_BMP280.h>\nstruct State { Adafruit_BMP280 dev; };\nCRAB_INIT(State& s, Params p) { s.dev.begin(p.address); }\nCRAB_STEP(State& s, In in, Out& out) { out.temperature = s.dev.readTemperature(); }\n".into(),
            libs: vec!["Adafruit BMP280".into()],
            has_init: true,
            has_state: true,
            inputs: vec![("enable".into(), NativeType::Bool)],
            outputs: vec![
                ("temperature".into(), NativeType::Float),
                ("ok".into(), NativeType::Bool),
            ],
            params: vec![("address".into(), NativeType::Int)],
            instances,
        }
    }

    #[test]
    fn no_natives_no_glue() {
        assert_eq!(glue(&[]), "");
    }

    #[test]
    fn glue_has_an_instance_per_block_and_hoisted_includes() {
        let text = glue(&[bmp(2)]);
        // The header is before the namespace, not inside it.
        let include = text.find("#include <Adafruit_BMP280.h>").unwrap();
        let ns = text.find("namespace crab_nat_0").unwrap();
        assert!(include < ns);
        assert_eq!(text.matches("#include <Adafruit_BMP280.h>").count(), 1);
        // Two instances: two states, the check of the instance number.
        assert!(text.contains("static State states[2];"), "{text}");
        assert!(text.contains("if (inst < 0 || inst >= 2) return 0;"));
        // Inputs, then parameters; init on the first call and when they change.
        assert!(text.contains("in.enable = crabNatI(r) != 0; r += 4;"));
        assert!(text.contains("p.address = crabNatI(r); r += 4;"));
        assert!(text.contains("memcmp(lastParams[inst], pw, 4)"));
        assert!(text.contains("crab_init(states[inst], p);"));
        // Outputs as words.
        assert!(text.contains("crabNatPutF(q, out.temperature); q += 4;"));
        assert!(text.contains("crabNatPutI(q, out.ok ? 1 : 0); q += 4;"));
        // Two inputs-words (1 input + 1 parameter) and two outputs.
        assert!(text.contains("case 0: *nin = 2; *nout = 2; return true;"));
        assert!(text.contains("case 0: return crab_nat_0::call(inst, in, out);"));
    }

    #[test]
    fn a_block_without_state_or_init_gets_defaults() {
        let mut n = bmp(1);
        n.has_state = false;
        n.has_init = false;
        n.params.clear();
        n.source = "CRAB_STEP(State& s, In in, Out& out) {}\n".into();
        let text = glue(&[n]);
        assert!(text.contains("struct State {};"));
        assert!(!text.contains("crab_init(states"));
        assert!(text.contains("bool changed = !ready[inst];"));
    }

    #[test]
    fn kinds_are_indexes_in_order() {
        let mut b = bmp(1);
        b.kind = "zzz".into();
        let list = vec![bmp(1), b];
        assert_eq!(native_kind(&list, "zzz"), Some(1));
        assert_eq!(native_kind(&list, "nope"), None);
        assert!(glue(&list).contains("namespace crab_nat_1"));
    }
}
