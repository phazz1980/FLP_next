//! Modules written by `flp-codegen` give the same numbers as the simulation of the schema.

use std::path::PathBuf;

use flp_codegen::Program;
use flp_core::{Library, Project, SignalType};
use flp_sim::mcu::{Manifest, McuModule, Word};
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

/// The carrier value `expected` of the signal `label` as the 32-bit word the module must hold.
fn word(program: &Program, label: &str, expected: f64) -> Word {
    let name = label.replace('.', "_");
    let ty = program
        .signals
        .iter()
        .find(|s| s.name == name)
        .map_or(SignalType::Float, |s| s.ty);
    if ty == SignalType::Float {
        Word::float(expected as f32)
    } else {
        Word::int(expected as i32)
    }
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
            let expected = word(&program, label, sim.output(block, port).unwrap());
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
            let expected = word(&program, label, sim.output(block, port).unwrap());
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
        // Rounded half away from zero, in 32 bits like the module.
        let duty = sim.bench().driven(18).unwrap_or(0.0) as f32;
        let want = (duty * 2.55f32).round() as i32;
        assert_eq!(
            io.pwm_out.get(&18).copied().unwrap_or(0),
            want,
            "step {step}: pwm"
        );
    }
}

#[test]
fn blink_example_drives_the_led_like_the_simulation() {
    let lib = library();
    let project = Project::load(repo_path("examples/blink")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    assert!(built.manifest.contains("allow = [\"gpio_write\"]"));
    let manifest = Manifest::parse(&built.manifest).unwrap();
    let mut module = McuModule::load(&built.wasm, &manifest, 0).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    let mut levels = std::collections::BTreeSet::new();
    for step in 0..60 {
        sim.step(50).unwrap();
        module.step(50).unwrap();
        let want = sim.bench().driven(2).unwrap_or(0.0) as i32;
        let got = module.io().gpio_out.get(&2).copied().unwrap_or(0);
        assert_eq!(got, want, "step {step}");
        levels.insert(got);
    }
    assert_eq!(levels.len(), 2, "the led must blink");
}

#[test]
fn numeric_edges_match_the_simulation() {
    // `n` starts 3 below i32::MAX and wraps; division by zero, remainders and mixed operands.
    compare("crates/flp-sim/tests/fixtures/wasm_edges", 13, 12, 100);
}
