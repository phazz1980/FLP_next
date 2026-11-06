//! A block with a native part (`native.cpp`, `specs/drivers`) cannot run on the PC; the values
//! of its `// crab:sim` line stand in for it. They are the first outputs, and the user may set
//! them by hand like those of any hardware block.

use crab_core::{Library, Logic};
use crab_sim::{Computed, Simulation};

fn library(root: &std::path::Path, cpp: &str) -> Library {
    let dir = root.join("nat_demo");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("block.toml"),
        "[block]\ntype = \"nat_demo\"\nname = \"Датчик\"\ngroup = \"Тест\"\n\n[[output]]\nname = \"t\"\ntype = \"float\"\n\n[[output]]\nname = \"ok\"\ntype = \"bool\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("native.cpp"), cpp).unwrap();
    Library::load(root).unwrap().0
}

fn logic() -> Logic {
    toml::from_str("[[block]]\nid = \"sensor\"\ntype = \"nat_demo\"\n").unwrap()
}

#[test]
fn sim_values_are_the_outputs_and_stay_editable() {
    let root = std::env::temp_dir().join("crab_native_sim_test");
    let _ = std::fs::remove_dir_all(&root);
    let lib = library(
        &root,
        "// crab:sim t=21.5 ok=true\nCRAB_STEP(State& s, In in, Out& out) {}\n",
    );
    let mut sim = Simulation::new(&logic(), &lib).unwrap();
    assert_eq!(sim.computed("sensor"), Some(Computed::Manual));
    assert_eq!(sim.output("sensor", "t"), Some(21.5));
    assert_eq!(sim.output("sensor", "ok"), Some(1.0));
    sim.step(10).unwrap();
    assert_eq!(sim.output("sensor", "t"), Some(21.5));
    // By hand, and back to the sim values on reset.
    assert!(sim.set_output("sensor", "t", 30.0));
    sim.step(10).unwrap();
    assert_eq!(sim.output("sensor", "t"), Some(30.0));
    sim.reset().unwrap();
    assert_eq!(sim.output("sensor", "t"), Some(21.5));
    assert!(sim.notes.is_empty(), "{:?}", sim.notes);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn outputs_without_a_sim_value_are_zero() {
    let root = std::env::temp_dir().join("crab_native_sim_test_zero");
    let _ = std::fs::remove_dir_all(&root);
    let lib = library(
        &root,
        "// crab:sim t=5\nCRAB_STEP(State& s, In in, Out& out) {}\n",
    );
    let sim = Simulation::new(&logic(), &lib).unwrap();
    assert_eq!(sim.output("sensor", "t"), Some(5.0));
    assert_eq!(sim.output("sensor", "ok"), Some(0.0));
    let _ = std::fs::remove_dir_all(&root);
}
