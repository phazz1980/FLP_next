//! Real builds with arduino-cli. Slow and need the ESP32 core installed, so ignored
//! by default: `cargo test -p crab-build -- --ignored`.

use std::path::PathBuf;

use crab_build::{ArduinoCli, CompileOptions};
use crab_codegen::{generate, write_sketch, ArduinoCpp};
use crab_core::{Library, Project};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
#[ignore = "needs arduino-cli with esp32:esp32"]
fn examples_compile() {
    let cli = ArduinoCli::locate().unwrap();
    let (lib, _) = Library::load(repo_path("library")).unwrap();
    for name in ["pump", "button"] {
        let project = Project::load(repo_path("examples").join(name)).unwrap();
        let code = generate(&project, &lib, &ArduinoCpp::RELEASE).unwrap();
        write_sketch(&project.work_dir(), &code).unwrap();
        let mut log = Vec::new();
        let result = cli.compile(
            &project.work_dir(),
            &project.config.board,
            &CompileOptions {
                low_priority: true,
                ..Default::default()
            },
            &mut |line| log.push(line.to_owned()),
        );
        assert!(result.is_ok(), "{name}: {result:?}\n{}", log.join("\n"));
    }
}

#[test]
#[ignore = "needs arduino-cli with esp32:esp32"]
fn master_with_web_compiles() {
    let cli = ArduinoCli::locate().unwrap();
    let project = Project::load(repo_path("examples/web_wasm")).unwrap();
    let features =
        crab_build::MasterFeatures::of_blocks(project.logic.blocks.iter().map(|b| b.kind.as_str()));
    let mut log = Vec::new();
    let result = cli.compile_master(
        &project.work_dir(),
        &project.config.board,
        &features,
        true,
        &mut |line| log.push(line.to_owned()),
    );
    let errors: Vec<&String> = log.iter().filter(|l| l.contains("error")).collect();
    assert!(result.is_ok(), "{:?}\n{errors:?}", result.as_ref().err());
}

#[test]
#[ignore = "needs arduino-cli with esp32:esp32"]
fn master_with_modbus_tcp_compiles() {
    let cli = ArduinoCli::locate().unwrap();
    let project = Project::load(repo_path("examples/modbus_tcp_master")).unwrap();
    let features =
        crab_build::MasterFeatures::of_blocks(project.logic.blocks.iter().map(|b| b.kind.as_str()));
    let mut log = Vec::new();
    let result = cli.compile_master(
        &project.work_dir(),
        &project.config.board,
        &features,
        true,
        &mut |line| log.push(line.to_owned()),
    );
    let errors: Vec<&String> = log
        .iter()
        .filter(|l| l.contains("error") || l.contains("undefined") || l.contains("overflow"))
        .collect();
    assert!(
        result.is_ok(),
        "{:?}
{errors:?}",
        result.as_ref().err()
    );
}

/// A block folder with `block.toml` and `native.cpp` (`specs/drivers`).
fn write_native_block(lib: &std::path::Path, kind: &str, toml: &str, cpp: &str) {
    let dir = lib.join(kind);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("block.toml"), toml).unwrap();
    std::fs::write(dir.join("native.cpp"), cpp).unwrap();
}

#[test]
#[ignore = "needs arduino-cli with esp32:esp32"]
fn master_with_native_blocks_compiles() {
    use crab_build::native::collect_natives;
    let root = std::env::temp_dir().join("crab_native_master_test");
    let _ = std::fs::remove_dir_all(&root);
    let lib_dir = root.join("library");
    write_native_block(
        &lib_dir,
        "nat_counter",
        "[block]\ntype = \"nat_counter\"\nname = \"Счётчик\"\ngroup = \"Тест\"\n\n[[input]]\nname = \"enable\"\ntype = \"bool\"\n\n[[output]]\nname = \"count\"\ntype = \"int\"\n\n[[param]]\nname = \"start\"\ntype = \"int\"\ndefault = 0\n",
        "// crab:native\nstruct State { int32_t n; };\nCRAB_INIT(State& s, Params p) { s.n = p.start; }\nCRAB_STEP(State& s, In in, Out& out) { if (in.enable) s.n++; out.count = s.n; }\n",
    );
    write_native_block(
        &lib_dir,
        "nat_twice",
        "[block]\ntype = \"nat_twice\"\nname = \"Удвоитель\"\ngroup = \"Тест\"\n\n[[input]]\nname = \"x\"\ntype = \"float\"\n\n[[output]]\nname = \"y\"\ntype = \"float\"\n",
        "// crab:native\nCRAB_STEP(State& s, In in, Out& out) { out.y = in.x * 2.0f; }\n",
    );
    let (lib, errors) = Library::load(&lib_dir).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let natives = collect_natives(&lib, ["nat_counter", "nat_counter", "nat_twice"]).unwrap();
    assert_eq!(natives.len(), 2);
    assert_eq!(natives[0].instances, 2);

    let cli = ArduinoCli::locate().unwrap();
    let project = Project::load(repo_path("examples/pump")).unwrap();
    let features = crab_build::MasterFeatures {
        natives,
        ..Default::default()
    };
    let mut log = Vec::new();
    let result = cli.compile_master(&root, &project.config.board, &features, true, &mut |line| {
        log.push(line.to_owned())
    });
    let errors: Vec<&String> = log.iter().filter(|l| l.contains("error")).collect();
    assert!(result.is_ok(), "{:?}\n{errors:?}", result.as_ref().err());
}
