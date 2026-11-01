use std::path::PathBuf;

use flp_core::library::{FirmwareSource, LibraryBlock, ESP32_CPP_TEMPLATE, WASM_FILE};
use flp_core::Library;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn template_wins_over_module() {
    let dir = std::env::temp_dir().join(format!("flp-fwsrc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let block_dir = dir.join("timer_on_rs");
    std::fs::create_dir_all(&block_dir).unwrap();
    std::fs::copy(
        repo_path("library/timer_on_rs/block.toml"),
        block_dir.join("block.toml"),
    )
    .unwrap();

    let block = LibraryBlock::load(&block_dir).unwrap();
    assert_eq!(block.firmware_source(), FirmwareSource::None);

    std::fs::write(block_dir.join(WASM_FILE), b"\0asm\x01\0\0\0").unwrap();
    assert_eq!(block.firmware_source(), FirmwareSource::Module);

    std::fs::write(block_dir.join(ESP32_CPP_TEMPLATE), "// @loop\n").unwrap();
    assert_eq!(block.firmware_source(), FirmwareSource::Template);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn library_samples_report_their_source() {
    let (lib, _) = Library::load(repo_path("library")).unwrap();
    assert_eq!(
        lib.get("timer_on_rs").unwrap().firmware_source(),
        FirmwareSource::Module
    );
    assert_eq!(
        lib.get("digital_out").unwrap().firmware_source(),
        FirmwareSource::Template
    );
}
