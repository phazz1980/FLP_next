//! Modules written by `flp-codegen` give the same numbers as the simulation of the schema.

use std::path::PathBuf;

use flp_codegen::Program;
use flp_core::{Library, Project};
use flp_sim::mcu::{Manifest, McuModule};
use flp_sim::schema::Simulation;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn library() -> Library {
    let (lib, errors) = Library::load(repo_path("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    lib
}

/// Runs the sheet as a module next to the simulation and compares every output after every step.
fn compare(project_dir: &str, outputs: usize, steps: u32, dt: u32) {
    let lib = library();
    let project = Project::load(repo_path(project_dir)).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    assert!(built.watched.len() == outputs, "{:?}", built.watched);
    for step in 0..steps {
        sim.step(dt).unwrap();
        module.step(dt).unwrap();
        for (i, label) in built.watched.iter().enumerate() {
            let (block, port) = label.split_once('.').unwrap();
            let expected = sim.output(block, port).unwrap();
            let got = module.outputs()[i];
            assert_eq!(got, expected, "step {step}: {label}");
        }
    }
}

#[test]
fn stateless_blocks_match_the_simulation() {
    compare("crates/flp-sim/tests/fixtures/wasm_logic", 20, 40, 100);
}

#[test]
fn timers_and_memory_blocks_match_the_simulation() {
    compare("crates/flp-sim/tests/fixtures/wasm_timers", 10, 120, 50);
}

#[test]
fn pins_match_the_simulation() {
    let lib = library();
    let project = Project::load(repo_path("crates/flp-sim/tests/fixtures/wasm_hw")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(
        manifest.allow,
        ["gpio_read", "gpio_write", "adc_read", "pwm_write"]
    );
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    for step in 0..120u32 {
        // The button is pressed (pulled low) for a while; the level sweeps over the ADC range.
        let pressed = (30..70).contains(&step);
        let counts = (step * 53) % 4400;
        sim.bench_mut()
            .set_level(4, if pressed { 0.0 } else { 1.0 });
        sim.bench_mut().set_level(34, f64::from(counts));
        module.io_mut().gpio_in.insert(4, i32::from(!pressed));
        module.io_mut().adc_in.insert(34, counts as i32);
        sim.step(50).unwrap();
        module.step(50).unwrap();
        for (i, label) in built.watched.iter().enumerate() {
            let (block, port) = label.split_once('.').unwrap();
            let expected = sim.output(block, port).unwrap();
            assert_eq!(module.outputs()[i], expected, "step {step}: {label}");
        }
        let io = module.io();
        for pin in [2_i64, 5] {
            let want = sim.bench().driven(pin).unwrap_or(0.0) as i32;
            assert_eq!(
                io.gpio_out.get(&(pin as i32)).copied().unwrap_or(0),
                want,
                "step {step}: pin {pin}"
            );
        }
        let duty = sim.bench().driven(18).unwrap_or(0.0);
        let want = (duty * 2.55).round() as i32;
        assert_eq!(
            io.pwm_out.get(&18).copied().unwrap_or(0),
            want,
            "step {step}: pwm"
        );
    }
}
