//! Snapshot tests: every example in `examples/` is generated and compared with
//! `tests/snapshots/<example>.ino`. A missing snapshot is created; to accept
//! changes run `UPDATE_SNAPSHOTS=1 cargo test` and review the diff in git.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use flp_codegen::{generate, ArduinoCpp, GenError};
use flp_core::{Library, Project};

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

fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(repo_path("examples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("project.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn assert_snapshot(path: &Path, actual: &str) {
    let expected = fs::read_to_string(path)
        .ok()
        .map(|s| s.replace("\r\n", "\n"));
    if expected.as_deref() == Some(actual) {
        return;
    }
    if expected.is_none() || std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::write(path, actual).unwrap();
        eprintln!("снимок записан: {}", path.display());
        return;
    }
    panic!(
        "сгенерированный код отличается от снимка {}\n--- получено ---\n{actual}",
        path.display()
    );
}

#[test]
fn examples_match_snapshots() {
    let lib = library();
    for dir in examples() {
        let project = Project::load(&dir).unwrap();
        let code = generate(&project, &lib, &ArduinoCpp)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let name = dir.file_name().unwrap().to_str().unwrap();
        let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(format!("{name}.ino"));
        assert_snapshot(&snapshot, &code);
    }
}

/// Every library block must be covered by an example, and thus by a snapshot.
#[test]
fn every_block_has_an_example() {
    let lib = library();
    let used: BTreeSet<String> = examples()
        .iter()
        .flat_map(|dir| Project::load(dir).unwrap().logic.blocks)
        .map(|b| b.kind)
        .collect();
    let missing: Vec<&str> = lib
        .iter()
        .map(|b| b.def.info.kind.as_str())
        .filter(|kind| !used.contains(*kind))
        .collect();
    assert!(
        missing.is_empty(),
        "блоки без примера в examples/: {missing:?}"
    );
}

#[test]
fn schema_errors_stop_generation() {
    let mut project = Project::load(repo_path("examples/pump")).unwrap();
    project.logic.links.pop();
    match generate(&project, &library(), &ArduinoCpp) {
        Err(GenError::Schema(diagnostics)) => {
            assert!(diagnostics.iter().any(|d| d.message.contains("pump")));
        }
        other => panic!("ожидалась ошибка схемы, получено {other:?}"),
    }
}

#[test]
fn defaults_are_applied() {
    // `less.b` and `digital_out.inverted` are omitted and take their defaults.
    let mut project = Project::load(repo_path("examples/pump")).unwrap();
    for b in &mut project.logic.blocks {
        b.params.remove("b");
    }
    let code = generate(&project, &library(), &ArduinoCpp).unwrap();
    assert!(code.contains("low_out = pressure_out < 0.0f;"), "{code}");
    assert!(
        code.contains("digitalWrite(26, (delay_out) ? HIGH : LOW);"),
        "{code}"
    );
}
