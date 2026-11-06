//! A block with a native part (`native.cpp`, `specs/drivers`) is run by the master: the module
//! hands it the inputs and parameters as words and takes the outputs back.

use std::path::{Path, PathBuf};

use crab_codegen::Program;
use crab_core::{Library, Project};
use crab_sim::mcu::{Manifest, McuModule, Word};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Library with `bmp` (an input, a float output and a bool output, two parameters) and
/// `twice` (one float in and out), project with two `bmp` and one `twice`.
fn setup(root: &Path) -> (Library, Project) {
    let _ = std::fs::remove_dir_all(root);
    let lib_dir = root.join("library");
    write(
        &lib_dir.join("bmp/block.toml"),
        "[block]\ntype = \"bmp\"\nname = \"Датчик\"\ngroup = \"Тест\"\n\n[[input]]\nname = \"enable\"\ntype = \"bool\"\ndefault = true\n\n[[output]]\nname = \"temperature\"\ntype = \"float\"\n\n[[output]]\nname = \"ok\"\ntype = \"bool\"\n\n[[param]]\nname = \"address\"\ntype = \"int\"\ndefault = 118\n\n[[param]]\nname = \"scale\"\ntype = \"float\"\ndefault = 1.5\n",
    );
    write(
        &lib_dir.join("bmp/native.cpp"),
        "CRAB_STEP(State& s, In in, Out& out) {}\n",
    );
    write(
        &lib_dir.join("twice/block.toml"),
        "[block]\ntype = \"twice\"\nname = \"Удвоитель\"\ngroup = \"Тест\"\n\n[[input]]\nname = \"x\"\ntype = \"float\"\ndefault = 0.0\n\n[[output]]\nname = \"y\"\ntype = \"float\"\n",
    );
    write(
        &lib_dir.join("twice/native.cpp"),
        "CRAB_STEP(State& s, In in, Out& out) {}\n",
    );
    let (lib, errors) = Library::load(&lib_dir).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let dir = root.join("project");
    write(
        &dir.join("project.toml"),
        "[project]\nname = \"native_module\"\n\n[board]\nfqbn = \"esp32:esp32:esp32\"\n",
    );
    write(
        &dir.join("logic.toml"),
        "[[block]]\nid = \"first\"\ntype = \"bmp\"\naddress = 119\n\n[[block]]\nid = \"second\"\ntype = \"bmp\"\n\n[[block]]\nid = \"dbl\"\ntype = \"twice\"\n\n[[link]]\nfrom = \"first.temperature\"\nto = \"dbl.x\"\n",
    );
    (lib, Project::load(&dir).unwrap())
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("crab_{name}_{}", std::process::id()))
}

#[test]
fn the_module_calls_the_native_parts_of_its_blocks() {
    let root = temp_root("native_module");
    let (lib, project) = setup(&root);
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["native"]);
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built.watched.iter().position(|l| l == label).unwrap();
        module.outputs()[i]
    };

    // Kinds are sorted by name: bmp is 0 (two instances), twice is 1. Words: inputs, then parameters.
    let io = module.io_mut();
    io.native_words.insert(0, 3); // enable, address, scale
    io.native_words.insert(1, 1); // x
    io.native_out
        .insert((0, 0), vec![Word::float(21.5).0 as i32, 1]);
    io.native_out
        .insert((0, 1), vec![Word::float(-4.0).0 as i32, 0]);
    io.native_out
        .insert((1, 0), vec![Word::float(43.0).0 as i32]);
    module.step(50).unwrap();

    assert_eq!(out(&module, "first.temperature"), Word::float(21.5));
    assert_eq!(out(&module, "first.ok"), Word::int(1));
    assert_eq!(out(&module, "second.temperature"), Word::float(-4.0));
    assert_eq!(out(&module, "second.ok"), Word::int(0));
    assert_eq!(out(&module, "dbl.y"), Word::float(43.0));

    // Each block got its own instance number and its own parameters, inputs first.
    let calls = &module.io_mut().native_in;
    let bits = |x: f32| Word::float(x).0 as i32;
    let by_address: Vec<&Vec<i32>> = vec![&calls[&(0, 0)], &calls[&(0, 1)]];
    let mut addresses: Vec<i32> = by_address.iter().map(|w| w[1]).collect();
    addresses.sort_unstable();
    assert_eq!(addresses, [118, 119], "{calls:?}");
    for words in by_address {
        assert_eq!(words[0], 1, "the enable input defaults to true");
        assert_eq!(words[2], bits(1.5), "the float parameter keeps its bits");
    }
    // The second block of the chain got the first one's temperature (one step later or the same).
    assert!(calls.contains_key(&(1, 0)));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_part_the_master_cannot_run_leaves_the_outputs_alone() {
    let root = temp_root("native_module_off");
    let (lib, project) = setup(&root);
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    module.io_mut().native_words.insert(0, 3);
    // No answer is set: the host says 0 for every call.
    module.step(50).unwrap();
    let i = built
        .watched
        .iter()
        .position(|l| l == "first.temperature")
        .unwrap();
    assert_eq!(module.outputs()[i], Word::float(0.0));
    let _ = std::fs::remove_dir_all(&root);
}
