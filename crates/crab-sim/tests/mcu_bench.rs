//! Step time of a generated sheet module on the PC (wasmi). Run by hand:
//! `CRAB_BENCH_DIR=<dir with sheet.wasm + module.toml> cargo test -p crab-sim --release --test mcu_bench -- --ignored --nocapture`

use crab_sim::mcu::{Manifest, McuModule};
use std::time::Instant;

#[test]
#[ignore]
fn step_time_of_a_big_module() {
    let Ok(dir) = std::env::var("CRAB_BENCH_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let wasm_path = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "wasm"))
        .expect("no .wasm in CRAB_BENCH_DIR");
    let wasm = std::fs::read(&wasm_path).unwrap();
    let manifest =
        Manifest::parse(&std::fs::read_to_string(dir.join("module.toml")).unwrap()).unwrap();
    let mut module = McuModule::load(&wasm, &manifest, 0).unwrap();
    let n = 20_000u32;
    for _ in 0..100 {
        module.step(10).unwrap();
    }
    let t = Instant::now();
    for _ in 0..n {
        module.step(10).unwrap();
    }
    let per = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("{} bytes: {per:.1} us per step", wasm.len());
}
