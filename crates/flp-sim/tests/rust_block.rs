//! `library/timer_on_rs` is a TON written in Rust and shipped as `block.wasm`; it must
//! behave like the built-in `timer_on`. Needs no compiler: the module is in the repository.

use std::path::PathBuf;

use flp_core::Library;
use flp_sim::builtin::{behavior, Ctx};
use flp_sim::SimModule;

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn rust_timer_on_matches_the_built_in_one() {
    let (lib, _) = Library::load(repo("library")).unwrap();
    let def = lib.get("timer_on_rs").expect("block is in the library");
    let wasm = std::fs::read(repo("library/timer_on_rs/block.wasm")).unwrap();
    let mut module = SimModule::load(&wasm, def.def.outputs.len()).unwrap();
    let mut expected = behavior("timer_on").expect("built-in behaviour");

    let params = [1000.0];
    let active = [true];
    let mut inputs = [0.0];
    let mut outputs = [0.0];
    let mut seed = 12345_u32;
    let mut now = 0_u64;
    let mut rose = false;
    for step in 0..5000 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        if (seed >> 16).is_multiple_of(40) {
            inputs[0] = 1.0 - inputs[0];
        }
        now += 10;
        expected.step(
            &Ctx {
                now_ms: now,
                inputs: &inputs,
                active: &active,
                params: &params,
                texts: &[],
                param_texts: &[],
                bench: &flp_blocks::NO_BENCH,
            },
            &mut outputs,
        );
        let got = module.step(10, &inputs, &params).unwrap();
        assert_eq!(got, outputs, "step {step}, input {inputs:?}");
        rose |= got[0] != 0.0;
    }
    assert!(rose, "the sequence never switched the output on");
}
