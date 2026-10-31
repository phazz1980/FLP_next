//! Blocks that run from a WASM module (`block.wasm`) in the firmware.
//!
//! `flp-build` translates the module into C (`block.wasm.c`, made by `w2c2`). Here that
//! text goes into the sketch as the code of the block type, and every instance of the
//! block gets its own module instance plus arrays of inputs, parameters and outputs
//! that the module reads and writes through the host functions `flp.input`,
//! `flp.param` and `flp.output`.

use std::collections::BTreeMap;

use flp_core::ident::c_ident;
use flp_core::library::FIRMWARE_C_FILE;
use flp_core::{ParamValue, SignalType};
use minijinja::Value;

use super::{cpp_type, Sections};
use crate::{GenError, Instance, Program};

/// Runtime of `w2c2` (memory, loads and stores, traps).
const BASE: &str = include_str!("w2c2_base.h");

/// Most inputs, parameters or outputs a module block can have.
const MAX_IO: usize = 16;

/// Whether any block of the program runs from a module.
pub(super) fn uses_modules(program: &Program) -> bool {
    program
        .blocks
        .iter()
        .any(|i| i.lib.firmware_source() == flp_core::library::FirmwareSource::Module)
}

/// Code placed once before all modules: the `w2c2` runtime and the host functions.
pub(super) fn runtime() -> String {
    format!(
        "// WASM-блоки: среда выполнения w2c2 и функции хоста\n\
         // In a namespace: the Arduino prototype generator leaves it alone.\n\
         namespace flpwasm {{\n\
         {BASE}\n\
         struct FlpWasmIo {{\n  double in[{MAX_IO}];\n  double param[{MAX_IO}];\n  double out[{MAX_IO}];\n}};\n\
         // Data of the block being stepped: modules reach it through the host functions.\n\
         static FlpWasmIo *flpWasmIo;\n\
         extern \"C\" F64 i3_flp5_input(wasmModuleInstance*, U32 i) {{ return i < {MAX_IO} ? flpWasmIo->in[i] : 0; }}\n\
         extern \"C\" F64 i3_flp5_param(wasmModuleInstance*, U32 i) {{ return i < {MAX_IO} ? flpWasmIo->param[i] : 0; }}\n\
         extern \"C\" void i3_flp6_output(wasmModuleInstance*, U32 i, F64 v) {{ if (i < {MAX_IO}) flpWasmIo->out[i] = v; }}\n\
         extern \"C\" void i3_flp3_log(wasmModuleInstance*, U32, U32) {{}}\n\
         }}\n\
         using namespace flpwasm;"
    )
}

/// Identifier made of `m<N>_...` that ends with `suffix` (and has `Export<N>` before it
/// when `export` is set), as `w2c2` names the instance functions.
fn find_symbol<'a>(text: &'a str, suffix: &str, export: bool) -> Option<&'a str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .find(|t| {
            let Some(rest) = t.strip_prefix('m') else {
                return false;
            };
            let Some(body) = t.strip_suffix(suffix) else {
                return false;
            };
            rest.starts_with(|c: char| c.is_ascii_digit())
                && body.contains('_')
                && (!export || export_number_before(body))
        })
}

/// Whether `body` ends with `Export<digits>_`... — the part left before `_step`.
fn export_number_before(body: &str) -> bool {
    let digits = body.trim_end_matches(|c: char| c.is_ascii_digit());
    digits.len() < body.len() && digits.ends_with("Export")
}

/// Sections of a module block: the C code once per type, a module instance per block.
pub(super) fn sections(
    program: &Program,
    inst: &Instance,
    input: &BTreeMap<&str, String>,
    param: &BTreeMap<&str, Value>,
) -> Result<Sections, GenError> {
    let block = inst.block.id.clone();
    let fail = |message: String| GenError::WasmBlock {
        block: block.clone(),
        message,
    };
    let def = &inst.lib.def;
    for (what, n) in [
        ("входов", def.inputs.len()),
        ("параметров", def.params.len()),
        ("выходов", def.outputs.len()),
    ] {
        if n > MAX_IO {
            return Err(fail(format!("больше {MAX_IO} {what}")));
        }
    }
    let path = inst.lib.dir.join(FIRMWARE_C_FILE);
    let code = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => GenError::MissingFirmwareC {
            kind: inst.block.kind.clone(),
        },
        _ => GenError::Io { path, source: e },
    })?;
    let kind = &inst.block.kind;
    let bad = |what: &str| {
        fail(format!(
            "в {FIRMWARE_C_FILE} типа «{kind}» не найден {what}"
        ))
    };
    let instance = find_symbol(&code, "Instance", false).ok_or_else(|| bad("тип экземпляра"))?;
    let step = find_symbol(&code, "_step", true).ok_or_else(|| bad("экспорт step"))?;
    let init = find_symbol(&code, "_init", true);
    let instantiate = find_symbol(&code, "Instantiate", false).ok_or_else(|| bad("Instantiate"))?;

    let id = c_ident(&inst.block.id);
    let mut looped = String::from("{\n  uint32_t now = millis();\n");
    for (i, port) in def.inputs.iter().enumerate() {
        if let Some(expr) = input.get(port.name.as_str()) {
            looped.push_str(&format!("  {id}_io.in[{i}] = (double)({expr});\n"));
        }
    }
    for (i, p) in def.params.iter().enumerate() {
        let Some(value) = param.get(p.name.as_str()) else {
            continue;
        };
        if matches!(inst.params.get(p.name.as_str()), Some(ParamValue::Text(_)))
            && p.kind != flp_core::ParamKind::Duration
        {
            return Err(fail(format!(
                "параметр «{}» текстовый: модуль получает только числа",
                p.name
            )));
        }
        looped.push_str(&format!("  {id}_io.param[{i}] = (double)({value});\n"));
    }
    looped.push_str(&format!(
        "  flpWasmIo = &{id}_io;\n  {step}(&{id}_w, now - {id}_t);\n  {id}_t = now;\n"
    ));
    for (i, port) in def.outputs.iter().enumerate() {
        let Some(&signal) = inst.outputs.get(port.name.as_str()) else {
            continue;
        };
        let signal = &program.signals[signal];
        let read = format!("{id}_io.out[{i}]");
        let value = match signal.ty {
            SignalType::Bool => format!("{read} != 0.0"),
            SignalType::Int | SignalType::Float => format!("({}){read}", cpp_type(signal.ty)),
            _ => {
                return Err(fail(format!(
                    "выход «{}» имеет тип {}: модуль выдаёт числа и логику",
                    port.name,
                    signal.ty.name()
                )))
            }
        };
        looped.push_str(&format!("  {} = {value};\n", signal.name));
    }
    looped.push('}');

    let mut setup = format!("flpWasmIo = &{id}_io;\n{instantiate}(&{id}_w, NULL);\n");
    if let Some(init) = init {
        setup.push_str(&format!("{init}(&{id}_w);\n"));
    }
    setup.push_str(&format!("{id}_t = millis();"));

    Ok(Sections {
        global: format!(
            "static {instance} {id}_w;\nstatic FlpWasmIo {id}_io;\nstatic uint32_t {id}_t;"
        ),
        setup,
        looped,
        once_global: format!(
            "namespace flpwasm {{
{}
}}",
            code.trim()
        ),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_symbols_of_a_module() {
        let c = "typedef struct m11_aX5FbInstance {\n} m11_aX5FbInstance;\n\
                 void m11_aX5FbExport4_step(m11_aX5FbInstance*i,U32 l0);\n\
                 void m11_aX5FbExport4_init(m11_aX5FbInstance*i);\n\
                 void m11_aX5FbInstantiate(m11_aX5FbInstance* i, wasmImportResolver r);\n";
        assert_eq!(find_symbol(c, "Instance", false), Some("m11_aX5FbInstance"));
        assert_eq!(find_symbol(c, "_step", true), Some("m11_aX5FbExport4_step"));
        assert_eq!(find_symbol(c, "_init", true), Some("m11_aX5FbExport4_init"));
        assert_eq!(
            find_symbol(c, "Instantiate", false),
            Some("m11_aX5FbInstantiate")
        );
        assert_eq!(find_symbol("int x;", "_step", true), None);
    }
}
