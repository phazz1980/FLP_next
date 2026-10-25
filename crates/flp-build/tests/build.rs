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
