//! The same input sequence runs in `flp-sim` (wasmi) and in the C code that goes into the
//! firmware (`block.wasm.c`, built on the PC with zig cc); the outputs must be identical.
//! Needs a compiler: `FLP_ZIG=<path to zig> cargo test -p flp-sim --test firmware_c`.
//! With `FLP_W2C2` set the module is translated again before the run.

use std::path::{Path, PathBuf};
use std::process::Command;

use flp_build::wasm_c::{translate_firmware, FIRMWARE_C_FILE};
use flp_sim::SimModule;

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

/// Inputs of the scenario: (input, step in ms, time parameter in ms).
fn scenario() -> Vec<(f64, u32, f64)> {
    let mut seed = 777_u32;
    let mut input = 0.0;
    (0..4000)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            if (seed >> 16).is_multiple_of(40) {
                input = 1.0 - input;
            }
            (input, 5 + (seed >> 20) % 20, 1000.0)
        })
        .collect()
}

#[test]
fn firmware_c_matches_the_simulation() {
    let Some(zig) = std::env::var_os("FLP_ZIG") else {
        eprintln!("FLP_ZIG is not set: skipped");
        return;
    };
    let work = std::env::temp_dir().join("flp_sim_firmware_c");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let block = work.join("timer_on_rs");
    std::fs::create_dir_all(&block).unwrap();
    for f in ["block.wasm", "block.toml"] {
        std::fs::copy(repo("library/timer_on_rs").join(f), block.join(f)).unwrap();
    }
    if let Some(w2c2) = std::env::var_os("FLP_W2C2") {
        translate_firmware(Path::new(&w2c2), &block, "timer_on_rs").unwrap();
    } else {
        for f in [FIRMWARE_C_FILE, "block.wasm.c.generated"] {
            std::fs::copy(repo("library/timer_on_rs").join(f), block.join(f)).unwrap();
        }
    }
    let code = std::fs::read_to_string(block.join(FIRMWARE_C_FILE)).unwrap();
    let find = |suffix: &str| -> String {
        code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .find(|t| t.starts_with('m') && t.ends_with(suffix) && t.contains('_'))
            .unwrap_or_else(|| panic!("no symbol *{suffix}"))
            .to_owned()
    };
    let (instance, instantiate) = (find("Instance"), find("Instantiate"));
    let (init, step) = (find("Export4_init"), find("Export4_step"));

    let base = std::fs::read_to_string(repo("crates/flp-codegen/src/arduino/w2c2_base.h")).unwrap();
    let main = format!(
        r#"#include <stdio.h>
{base}
static double g_in, g_param, g_out;
F64 i3_flp5_input(wasmModuleInstance* i, U32 n) {{ (void)i; (void)n; return g_in; }}
F64 i3_flp5_param(wasmModuleInstance* i, U32 n) {{ (void)i; (void)n; return g_param; }}
void i3_flp6_output(wasmModuleInstance* i, U32 n, F64 v) {{ (void)i; (void)n; g_out = v; }}
void i3_flp3_log(wasmModuleInstance* i, U32 a, U32 b) {{ (void)i; (void)a; (void)b; }}
{code}
int main(void) {{
    static {instance} m;
    unsigned dt;
    {instantiate}(&m, NULL);
    {init}(&m);
    while (scanf("%lf %u %lf", &g_in, &dt, &g_param) == 3) {{
        {step}(&m, dt);
        printf("%g\n", g_out);
    }}
    return 0;
}}
"#
    );
    std::fs::write(work.join("main.c"), main).unwrap();
    let exe = work.join("harness.exe");
    let out = Command::new(&zig)
        .args(["cc", "-O1", "-w", "-o"])
        .arg(&exe)
        .arg(work.join("main.c"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let scenario = scenario();
    let stdin: String = scenario
        .iter()
        .map(|(i, dt, p)| format!("{i} {dt} {p}\n"))
        .collect();
    let mut child = Command::new(&exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let c_out: Vec<f64> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| l.parse().unwrap())
        .collect();
    assert_eq!(c_out.len(), scenario.len());

    let wasm = std::fs::read(block.join("block.wasm")).unwrap();
    let mut module = SimModule::load(&wasm, 1).unwrap();
    let mut rose = false;
    for (n, (i, dt, p)) in scenario.iter().enumerate() {
        let sim = module.step(*dt, &[*i], &[*p]).unwrap();
        assert_eq!(sim[0], c_out[n], "step {n}: input {i}, dt {dt}");
        rose |= sim[0] != 0.0;
    }
    assert!(rose, "the scenario never switched the output on");
}
