//! Real builds with arduino-cli. Slow and need the ESP32 core installed, so ignored
//! by default: `cargo test -p flp-build -- --ignored`.

use std::path::PathBuf;

use flp_build::{ArduinoCli, CompileOptions};
use flp_codegen::{generate, write_sketch, ArduinoCpp};
use flp_core::{Library, Project};

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
        flp_build::MasterFeatures::of_blocks(project.logic.blocks.iter().map(|b| b.kind.as_str()));
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
        flp_build::MasterFeatures::of_blocks(project.logic.blocks.iter().map(|b| b.kind.as_str()));
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
