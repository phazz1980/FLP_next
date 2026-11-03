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
