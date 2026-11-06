//! Translating the module of a block (`block.wasm`) into C for the firmware with `w2c2`.
//!
//! The result, `block.wasm.c`, is one file (generated header and body) that the code
//! generator puts into the sketch. It is marked with `block.wasm.c.generated` holding
//! the hash of the module it came from; a `block.wasm.c` without the mark was written
//! by hand and is never overwritten.

use std::path::Path;

use wasmparser::{CompositeInnerType, ExternalKind, Parser, Payload, TypeRef, ValType, Validator};

use crate::hidden_command;
use crate::wasm::MODULE_FILE;

/// C translation of the module, merged into one file (header and body).
pub use crab_core::library::FIRMWARE_C_FILE;
/// Mark next to the translation: the hash of the module it was made from.
pub const FIRMWARE_C_MARK: &str = "block.wasm.c.generated";
/// Linear memory a module may have in firmware: one WASM page. A block instance
/// keeps its whole memory in RAM.
pub const MAX_FIRMWARE_MEMORY: u64 = 65536;

/// Functions the host gives to a module (module `crab`) and their signatures.
const HOST_FUNCTIONS: [(&str, &[ValType], &[ValType]); 4] = [
    ("input", &[ValType::I32], &[ValType::F64]),
    ("param", &[ValType::I32], &[ValType::F64]),
    ("output", &[ValType::I32, ValType::F64], &[]),
    ("log", &[ValType::I32, ValType::I32], &[]),
];

/// What [`inspect_module`] found in a module that fits the firmware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    /// Host functions the module imports (names from module `crab`).
    pub imports: Vec<String>,
    /// Whether the module exports `init` (optional).
    pub has_init: bool,
    /// Linear memory of the module in bytes.
    pub memory_bytes: u64,
}

type FuncType = (Vec<ValType>, Vec<ValType>);

/// Checks that a module can run in firmware: valid, imports only the `crab` host
/// functions with the right signatures, exports `step(i32)`, memory bounded by
/// [`MAX_FIRMWARE_MEMORY`] (and a `max` given).
pub fn inspect_module(wasm: &[u8]) -> Result<ModuleInfo, String> {
    Validator::new()
        .validate_all(wasm)
        .map_err(|e| format!("модуль WASM неверен: {e}"))?;
    let mut types: Vec<FuncType> = Vec::new();
    // Type index of every function, imports first, like the function index space.
    let mut funcs: Vec<u32> = Vec::new();
    let mut info = ModuleInfo {
        imports: Vec::new(),
        has_init: false,
        memory_bytes: 0,
    };
    let mut step: Option<u32> = None;
    let mut init: Option<u32> = None;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(|e| e.to_string())? {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for ty in group.map_err(|e| e.to_string())?.types() {
                        if let CompositeInnerType::Func(f) = &ty.composite_type.inner {
                            types.push((f.params().to_vec(), f.results().to_vec()));
                        }
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(|e| e.to_string())?;
                    let TypeRef::Func(ty) = import.ty else {
                        return Err(format!(
                            "модуль импортирует «{}.{}» не функцией: в прошивке допустимы \
                             только функции модуля crab",
                            import.module, import.name
                        ));
                    };
                    let host = HOST_FUNCTIONS
                        .iter()
                        .find(|(name, _, _)| import.module == "crab" && import.name == *name);
                    let Some((name, params, results)) = host else {
                        return Err(format!(
                            "модуль импортирует «{}.{}»: в прошивке доступны только \
                             crab.input, crab.param, crab.output и crab.log",
                            import.module, import.name
                        ));
                    };
                    let (got_params, got_results) = &types[ty as usize];
                    if got_params != params || got_results != results {
                        return Err(format!(
                            "импорт crab.{name} имеет неверную сигнатуру: ожидается \
                             {params:?} -> {results:?}"
                        ));
                    }
                    funcs.push(ty);
                    info.imports.push((*name).to_owned());
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    funcs.push(ty.map_err(|e| e.to_string())?);
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|e| e.to_string())?;
                    if memory.memory64 || memory.shared {
                        return Err(
                            "память модуля 64-битная или общая: в прошивке не поддерживается"
                                .into(),
                        );
                    }
                    let Some(max) = memory.maximum else {
                        return Err("у памяти модуля нет верхней границы: задайте её при \
                                    сборке (для Rust: -C link-arg=--max-memory=65536)"
                            .into());
                    };
                    let bytes = max * 65536;
                    if bytes > MAX_FIRMWARE_MEMORY {
                        return Err(format!(
                            "память модуля до {bytes} байт, в прошивке допустимо не больше \
                             {MAX_FIRMWARE_MEMORY} (для Rust: -C link-arg=-zstack-size=1024 \
                             -C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536)"
                        ));
                    }
                    info.memory_bytes = bytes;
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|e| e.to_string())?;
                    if export.kind == ExternalKind::Func {
                        match export.name {
                            "step" => step = Some(export.index),
                            "init" => init = Some(export.index),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let signature = |index: u32| funcs.get(index as usize).map(|t| &types[*t as usize]);
    match step.and_then(signature) {
        None => return Err("модуль не экспортирует функцию step(dt_ms: i32)".into()),
        Some((params, results)) if params.as_slice() != [ValType::I32] || !results.is_empty() => {
            return Err("функция step должна быть вида step(dt_ms: i32)".into())
        }
        Some(_) => {}
    }
    if let Some((params, results)) = init.and_then(signature) {
        if !params.is_empty() || !results.is_empty() {
            return Err("функция init должна быть вида init()".into());
        }
        info.has_init = true;
    }
    Ok(info)
}

/// What [`translate_firmware`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirmwareTranslation {
    /// `block.wasm.c` was written.
    Translated(ModuleInfo),
    /// `block.wasm.c` already matches the current `block.wasm`.
    UpToDate,
    /// A hand-made `block.wasm.c` is there; it was left alone.
    KeptManual,
}

/// The `w2c2` to use: the configured path if it is a file, otherwise `CRAB_W2C2`, otherwise
/// `w2c2` from `PATH`. `None` when none of them exists.
pub fn find_w2c2(configured: Option<&Path>) -> Option<std::path::PathBuf> {
    let file = if cfg!(windows) { "w2c2.exe" } else { "w2c2" };
    let from_path = || {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|d| d.join(file))
            .find(|p| p.is_file())
    };
    configured
        .filter(|p| p.is_file())
        .map(Path::to_owned)
        .or_else(|| std::env::var_os("CRAB_W2C2").map(std::path::PathBuf::from))
        .filter(|p| p.is_file())
        .or_else(from_path)
}

/// Translates the module of every library block that has no template, so that the
/// generator finds `block.wasm.c`. Returns a message per block that could not be translated.
pub fn translate_library(w2c2: &Path, lib: &crab_core::Library) -> Vec<String> {
    let mut problems = Vec::new();
    for block in lib.iter() {
        if block.firmware_source() != crab_core::library::FirmwareSource::Module {
            continue;
        }
        let name = &block.def.info.kind;
        if let Err(e) = translate_firmware(w2c2, &block.dir, name) {
            problems.push(format!("{name}: {e}"));
        }
    }
    problems
}

/// Checks the path to `w2c2` from the settings: the program must start and name itself.
pub fn w2c2_check(w2c2: &Path) -> Result<(), String> {
    let output = hidden_command(w2c2)
        .output()
        .map_err(|e| format!("не удалось запустить w2c2: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    if text.contains("w2c2") {
        Ok(())
    } else {
        Err("указанный файл не похож на w2c2".into())
    }
}

fn module_hash(wasm: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(wasm)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Translates `block.wasm` of `block_dir` into C with `w2c2` (`block.wasm.c`: the
/// generated header and body without the runtime include, which the sketch supplies).
///
/// `name` is the block type; it becomes the prefix of the symbols. The module is checked
/// by [`inspect_module`] first. Does nothing when the existing translation was made
/// from the same module.
pub fn translate_firmware(
    w2c2: &Path,
    block_dir: &Path,
    name: &str,
) -> Result<FirmwareTranslation, String> {
    let module = block_dir.join(MODULE_FILE);
    let c_file = block_dir.join(FIRMWARE_C_FILE);
    let mark = block_dir.join(FIRMWARE_C_MARK);
    if c_file.exists() && !mark.exists() {
        return Ok(FirmwareTranslation::KeptManual);
    }
    let wasm = std::fs::read(&module).map_err(|e| format!("{}: {e}", module.display()))?;
    let info = inspect_module(&wasm)?;
    let hash = module_hash(&wasm);
    if c_file.exists() && std::fs::read_to_string(&mark).is_ok_and(|m| m.trim() == hash) {
        return Ok(FirmwareTranslation::UpToDate);
    }
    let work = std::env::temp_dir().join(format!("crab-w2c2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let result = run_w2c2(w2c2, &wasm, &work, name);
    let _ = std::fs::remove_dir_all(&work);
    // The old translation stays in place until a new one is made.
    std::fs::write(&c_file, result?).map_err(|e| format!("{}: {e}", c_file.display()))?;
    std::fs::write(&mark, hash).map_err(|e| format!("{}: {e}", mark.display()))?;
    Ok(FirmwareTranslation::Translated(info))
}

fn run_w2c2(w2c2: &Path, wasm: &[u8], work: &Path, name: &str) -> Result<String, String> {
    // Relative names keep the symbol prefix short and the same on every machine.
    let input = format!("{name}.wasm");
    std::fs::write(work.join(&input), wasm).map_err(|e| format!("{input}: {e}"))?;
    let output = hidden_command(w2c2)
        .current_dir(work)
        .arg(&input)
        .arg(format!("{name}.c"))
        .output()
        .map_err(|e| format!("не удалось запустить w2c2: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "w2c2: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let read = |ext: &str| {
        let path = work.join(format!("{name}.{ext}"));
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    // Both files include the runtime and the body includes the header; the sketch
    // supplies the runtime once for all blocks.
    let strip = |text: String| -> String {
        text.lines()
            .filter(|l| !l.trim_start().starts_with("#include"))
            .collect::<Vec<_>>()
            .join(
                "
",
            )
    };
    let header = strip(read("h")?);
    let text = format!(
        "{header}
{}
",
        strip(read("c")?)
    );
    Ok(mangle_functions(&text, &header, name))
}

/// `w2c2` names the module's own functions `f3`, `f4`, ... — the same in every module.
/// Prefixes them with the block type so that modules of several blocks can share a sketch.
fn mangle_functions(text: &str, header: &str, name: &str) -> String {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let numbered = |token: &str| {
        token.len() > 1 && token.starts_with('f') && token[1..].bytes().all(|b| b.is_ascii_digit())
    };
    // Functions declared in the header as `<type> f<N>(`.
    let declared: Vec<&str> = header
        .lines()
        .filter_map(|l| l.split('(').next())
        .filter_map(|l| l.split_whitespace().last())
        .filter(|t| numbered(t))
        .collect();
    let prefix: String = name
        .chars()
        .map(|c| if is_ident(c) { c } else { '_' })
        .collect();
    let mut out = String::with_capacity(text.len() + 64);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if declared.contains(&token.as_str()) {
            out.push_str(&prefix);
            out.push('_');
        }
        out.push_str(token);
        token.clear();
    };
    for c in text.chars() {
        if is_ident(c) {
            token.push(c);
        } else {
            flush(&mut token, &mut out);
            out.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(id: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![id, body.len() as u8];
        out.extend_from_slice(body);
        out
    }

    /// A module with the given pieces; all sizes stay below 128, so LEB is one byte.
    fn module(sections: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"\0asm\x01\0\0\0".to_vec();
        for s in sections {
            out.extend_from_slice(s);
        }
        out
    }

    /// Type 0 is `(i32) -> ()`, the signature of `step`.
    fn types() -> Vec<u8> {
        section(1, &[1, 0x60, 1, 0x7f, 0])
    }

    /// Function 0 of type 0 exported as `name`, empty body.
    fn function(name: &str) -> Vec<Vec<u8>> {
        let mut export = vec![1, name.len() as u8];
        export.extend_from_slice(name.as_bytes());
        export.extend_from_slice(&[0, 0]);
        vec![
            section(3, &[1, 0]),
            section(7, &export),
            section(10, &[1, 2, 0, 0x0b]),
        ]
    }

    fn minimal(extra: &[Vec<u8>]) -> Vec<u8> {
        // Sections must come in id order: put the extras where their ids belong.
        let mut sections = vec![types()];
        let mut function = function("step").into_iter();
        sections.extend(extra.iter().filter(|s| s[0] < 3).cloned());
        sections.extend(function.next());
        sections.extend(extra.iter().filter(|s| s[0] > 3 && s[0] < 7).cloned());
        sections.extend(function);
        module(&sections)
    }

    #[test]
    fn the_rust_sample_fits_the_firmware() {
        let wasm = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library/timer_on_rs/block.wasm"),
        )
        .unwrap();
        let info = inspect_module(&wasm).unwrap();
        assert_eq!(info.imports, ["input", "param", "output"]);
        assert!(info.has_init);
        assert_eq!(info.memory_bytes, 65536);
    }

    #[test]
    fn minimal_module_is_accepted() {
        let info = inspect_module(&minimal(&[])).unwrap();
        assert!(info.imports.is_empty() && !info.has_init);
        assert_eq!(info.memory_bytes, 0);
    }

    #[test]
    fn foreign_import_is_rejected_with_its_name() {
        // import env.write of type 0
        let import = section(
            2,
            &[
                1, 3, b'e', b'n', b'v', 5, b'w', b'r', b'i', b't', b'e', 0, 0,
            ],
        );
        let err = inspect_module(&minimal(&[import])).unwrap_err();
        assert!(err.contains("env.write"), "{err}");
    }

    #[test]
    fn host_import_with_a_wrong_signature_is_rejected() {
        // crab.input of type 0, which is (i32) -> () instead of (i32) -> f64
        let import = section(
            2,
            &[
                1, 4, b'c', b'r', b'a', b'b', 5, b'i', b'n', b'p', b'u', b't', 0, 0,
            ],
        );
        let err = inspect_module(&minimal(&[import])).unwrap_err();
        assert!(
            err.contains("crab.input") && err.contains("сигнатур"),
            "{err}"
        );
    }

    #[test]
    fn missing_step_is_reported() {
        let mut sections = vec![types()];
        sections.extend(function("other"));
        let err = inspect_module(&module(&sections)).unwrap_err();
        assert!(err.contains("step"), "{err}");
    }

    #[test]
    fn memory_without_a_maximum_or_too_big_is_rejected() {
        let unbounded = section(5, &[1, 0, 1]);
        let err = inspect_module(&minimal(&[unbounded])).unwrap_err();
        assert!(err.contains("верхней границы"), "{err}");
        let big = section(5, &[1, 1, 1, 2]);
        let err = inspect_module(&minimal(&[big])).unwrap_err();
        assert!(err.contains("131072"), "{err}");
        let fits = section(5, &[1, 1, 1, 1]);
        assert_eq!(
            inspect_module(&minimal(&[fits])).unwrap().memory_bytes,
            65536
        );
    }

    #[test]
    fn module_functions_get_the_block_prefix() {
        let header = "void f3(wasmModuleInstance*);
F64 i4_crab5_input(wasmModuleInstance*, U32);
";
        let body = "void f3(wasmModuleInstance* i) { f64_load(); }
void x() { f3(&i->common); }";
        let got = mangle_functions(body, header, "my-block");
        assert_eq!(
            got,
            "void my_block_f3(wasmModuleInstance* i) { f64_load(); }
void x() { my_block_f3(&i->common); }"
        );
    }

    #[test]
    fn garbage_is_not_a_module() {
        assert!(inspect_module(b"not wasm").is_err());
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("crab-wasmc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manual_translation_is_never_overwritten() {
        let dir = temp_dir("manual");
        std::fs::write(dir.join(FIRMWARE_C_FILE), "mine").unwrap();
        let got = translate_firmware(Path::new("no-such-w2c2"), &dir, "x").unwrap();
        assert_eq!(got, FirmwareTranslation::KeptManual);
        assert_eq!(
            std::fs::read_to_string(dir.join(FIRMWARE_C_FILE)).unwrap(),
            "mine"
        );
    }

    #[test]
    fn matching_translation_is_not_redone() {
        let dir = temp_dir("fresh");
        let wasm = minimal(&[]);
        std::fs::write(dir.join(MODULE_FILE), &wasm).unwrap();
        std::fs::write(dir.join(FIRMWARE_C_FILE), "c").unwrap();
        std::fs::write(dir.join(FIRMWARE_C_MARK), module_hash(&wasm)).unwrap();
        let got = translate_firmware(Path::new("no-such-w2c2"), &dir, "x").unwrap();
        assert_eq!(got, FirmwareTranslation::UpToDate);
    }

    #[test]
    fn changed_module_needs_the_translator() {
        let dir = temp_dir("stale");
        std::fs::write(dir.join(MODULE_FILE), minimal(&[])).unwrap();
        std::fs::write(dir.join(FIRMWARE_C_FILE), "old").unwrap();
        std::fs::write(dir.join(FIRMWARE_C_MARK), "other hash").unwrap();
        let err = translate_firmware(Path::new("no-such-w2c2"), &dir, "x").unwrap_err();
        assert!(err.contains("w2c2"), "{err}");
        assert_eq!(
            std::fs::read_to_string(dir.join(FIRMWARE_C_FILE)).unwrap(),
            "old"
        );
    }

    /// Needs the translator: `CRAB_W2C2=<path to w2c2> cargo test -p crab-build`.
    #[test]
    fn rust_sample_is_translated_into_one_file() {
        let Some(w2c2) = std::env::var_os("CRAB_W2C2") else {
            return;
        };
        let dir = temp_dir("real");
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library/timer_on_rs");
        std::fs::copy(sample.join(MODULE_FILE), dir.join(MODULE_FILE)).unwrap();
        let got = translate_firmware(Path::new(&w2c2), &dir, "timer_on_rs").unwrap();
        assert!(matches!(got, FirmwareTranslation::Translated(_)), "{got:?}");
        let text = std::fs::read_to_string(dir.join(FIRMWARE_C_FILE)).unwrap();
        assert!(
            text.contains("_step") && !text.contains("#include"),
            "{text}"
        );
        let again = translate_firmware(Path::new(&w2c2), &dir, "timer_on_rs").unwrap();
        assert_eq!(again, FirmwareTranslation::UpToDate);
    }
}
