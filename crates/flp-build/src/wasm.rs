//! Building the simulation module of a block (`block.wasm`) from its C++ source with zig.
//!
//! The source comes from `flp_codegen::sim_source`. A module built here is marked with
//! an empty `block.wasm.generated` file next to it; a `block.wasm` without the mark was
//! made by hand and is never overwritten.

use std::path::Path;

use crate::hidden_command;

/// File name of the module in the block folder.
pub const MODULE_FILE: &str = "block.wasm";
/// Mark next to the module: it was generated from the template, not written by hand.
pub const GENERATED_MARK: &str = "block.wasm.generated";

/// What [`build_module`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleBuild {
    /// `block.wasm` was built and marked as generated.
    Built,
    /// A hand-made `block.wasm` is there; it was left alone.
    KeptManual,
}

/// Version line of the compiler (`zig version`), to check the path in the settings.
pub fn zig_version(zig: &Path) -> Result<String, String> {
    let output = hidden_command(zig)
        .arg("version")
        .output()
        .map_err(|e| format!("не удалось запустить zig: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Compiles C++ `source` into the WASM module `out` with `zig c++`.
///
/// The module has no libc and no entry point; its imports (`flp.*`) stay open for the
/// simulator to provide.
pub fn compile_wasm(zig: &Path, source: &str, out: &Path) -> Result<(), String> {
    let src = out.with_extension("sim.cpp");
    std::fs::write(&src, source).map_err(|e| format!("{}: {e}", src.display()))?;
    let result = hidden_command(zig)
        .args([
            "c++",
            "--target=wasm32-freestanding",
            "-std=c++17",
            "-O2",
            "-fno-exceptions",
            "-fno-rtti",
            "-nostdlib",
            "-Wl,--no-entry",
            "-o",
        ])
        .arg(out)
        .arg(&src)
        .output();
    let _ = std::fs::remove_file(&src);
    let output = result.map_err(|e| format!("не удалось запустить zig: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let text = String::from_utf8_lossy(&output.stderr);
        let errors: Vec<&str> = text
            .lines()
            .filter(|l| l.contains("error"))
            .take(5)
            .collect();
        Err(if errors.is_empty() {
            text.trim().to_owned()
        } else {
            errors.join("\n")
        })
    }
}

/// Builds `block.wasm` in `block_dir` from `source` and marks it as generated.
///
/// The old module stays in place until the new one is built, so a failed build loses
/// nothing. A module without the mark is not touched.
pub fn build_module(zig: &Path, block_dir: &Path, source: &str) -> Result<ModuleBuild, String> {
    let module = block_dir.join(MODULE_FILE);
    let mark = block_dir.join(GENERATED_MARK);
    if module.exists() && !mark.exists() {
        return Ok(ModuleBuild::KeptManual);
    }
    let tmp = block_dir.join("block.new.wasm");
    if let Err(e) = compile_wasm(zig, source, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &module).map_err(|e| format!("{}: {e}", module.display()))?;
    std::fs::write(&mark, b"").map_err(|e| format!("{}: {e}", mark.display()))?;
    Ok(ModuleBuild::Built)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("flp-wasm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manual_module_is_never_overwritten() {
        let dir = temp_dir("manual");
        std::fs::write(dir.join(MODULE_FILE), b"mine").unwrap();
        let zig = Path::new("no-such-zig");
        assert_eq!(
            build_module(zig, &dir, "").unwrap(),
            ModuleBuild::KeptManual
        );
        assert_eq!(std::fs::read(dir.join(MODULE_FILE)).unwrap(), b"mine");
        assert!(!dir.join(GENERATED_MARK).exists());
    }

    #[test]
    fn failed_build_keeps_the_generated_module() {
        let dir = temp_dir("failed");
        std::fs::write(dir.join(MODULE_FILE), b"old").unwrap();
        std::fs::write(dir.join(GENERATED_MARK), b"").unwrap();
        let err = build_module(Path::new("no-such-zig"), &dir, "").unwrap_err();
        assert!(err.contains("zig"), "{err}");
        assert_eq!(std::fs::read(dir.join(MODULE_FILE)).unwrap(), b"old");
        assert!(!dir.join("block.new.wasm").exists());
    }

    /// Needs a real compiler: `FLP_ZIG=<path to zig> cargo test -p flp-build`.
    #[test]
    fn generated_module_is_built_and_marked() {
        let Some(zig) = std::env::var_os("FLP_ZIG") else {
            return;
        };
        let dir = temp_dir("built");
        let source = flp_codegen_test_source();
        let built = build_module(Path::new(&zig), &dir, source).unwrap();
        assert_eq!(built, ModuleBuild::Built);
        assert!(dir.join(GENERATED_MARK).exists());
        let bytes = std::fs::read(dir.join(MODULE_FILE)).unwrap();
        assert_eq!(&bytes[..4], b"\0asm");
    }

    fn flp_codegen_test_source() -> &'static str {
        r#"extern "C" {
__attribute__((import_module("flp"), import_name("input"))) double input(int);
__attribute__((import_module("flp"), import_name("output"))) void output(int, double);
__attribute__((export_name("step"))) void step(int dt) { output(0, input(0) + dt); }
}
"#
    }
}
