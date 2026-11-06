//! ESP32 cores per chip: which chips have their compiler and libraries on disk, and
//! downloading a chip's pack from the project release.
//!
//! Presence is decided by looking at the folders (the `esp32core` data folder next to
//! the program, then the system `Arduino15`), not by running arduino-cli.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::{app_dir, BuildError, CORE_DIR};

/// Release of the project that carries the packs (the packs are attached to it).
pub const CORE_PACKS_URL: &str = "https://github.com/phazz1980/FLP_next/releases/download/v1.2.0";

/// One downloadable pack: the files a chip needs on top of the base ESP32 core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorePack {
    /// Chip id as in FQBN: `esp32`, `esp32s3`, `esp32c3`.
    pub chip: &'static str,
    /// Name for the settings window.
    pub title: &'static str,
    /// Archive name on the release page.
    pub file: &'static str,
    /// Download size, MB (for the settings window).
    pub size_mb: u32,
    pub sha256: &'static str,
    /// Folders under `packages/esp32/tools` that must be present.
    tools: &'static [&'static str],
}

pub const PACKS: &[CorePack] = &[
    CorePack {
        chip: "esp32",
        title: "ESP32",
        file: "FLP_next-toolchain-esp32.7z",
        size_mb: 224,
        sha256: "280f180a492e8ade61f80959e5fba313d19451c1f4d3ce1a9e80ca9073989aec",
        tools: &["esp-x32", "esp32-libs"],
    },
    CorePack {
        chip: "esp32s3",
        title: "ESP32-S3",
        file: "FLP_next-core-esp32s3.7z",
        size_mb: 47,
        sha256: "e678a558c82d6b6820b8f9faeb61f2549ff79ed6b6f0fea752765a230229f527",
        tools: &["esp-x32", "esp32s3-libs"],
    },
    CorePack {
        chip: "esp32c3",
        title: "ESP32-C3",
        file: "FLP_next-core-esp32c3.7z",
        size_mb: 224,
        sha256: "cb7317250c3e3dc56eb7e826bfd7ebb4f8c976b38e0fc49c31b3ae46e21d9f2d",
        tools: &["esp-rv32", "esp32c3-libs"],
    },
];

/// The pack of board `fqbn`; `None` for boards outside the ESP32 core.
pub fn pack_for_fqbn(fqbn: &str) -> Option<&'static CorePack> {
    use flp_core::pins::Chip;
    let chip = match Chip::from_fqbn(fqbn)? {
        Chip::Esp32 => "esp32",
        Chip::Esp32S3 => "esp32s3",
        Chip::Esp32C3 | Chip::Esp32C3SuperMini => "esp32c3",
    };
    PACKS.iter().find(|p| p.chip == chip)
}

/// Data folders where a core may live, in search order.
pub fn core_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(app) = app_dir() {
        dirs.push(app.join(CORE_DIR));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(Path::new(&local).join("Arduino15"));
    }
    dirs
}

fn has_entries(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_some())
}

/// Whether `data` (an arduino-cli data folder) holds everything `pack` needs.
pub fn installed_in(data: &Path, pack: &CorePack) -> bool {
    let esp = data.join("packages").join("esp32");
    let has_hardware = std::fs::read_dir(esp.join("hardware").join("esp32"))
        .is_ok_and(|d| d.flatten().any(|v| v.path().join("boards.txt").is_file()));
    has_hardware
        && pack.tools.iter().all(|tool| {
            std::fs::read_dir(esp.join("tools").join(tool))
                .is_ok_and(|d| d.flatten().any(|v| has_entries(&v.path())))
        })
}

/// Whether the chip's core is on disk in any of the data folders.
pub fn is_installed(pack: &CorePack) -> bool {
    core_dirs().iter().any(|d| installed_in(d, pack))
}

/// Whether a board can be chosen: its chip's core is present. Boards of other cores
/// are always available (arduino-cli installs them on demand).
pub fn fqbn_available(fqbn: &str) -> bool {
    pack_for_fqbn(fqbn).is_none_or(is_installed)
}

/// The message for a build with a chip whose core is missing.
pub fn missing_message(pack: &CorePack) -> String {
    format!(
        "Ядро для {} не скачано: откройте Настройки → Ядра ESP32 и скачайте его",
        pack.title
    )
}

/// Downloads the pack and unpacks it into the program folder (`esp32core/...`).
/// `progress(done, total)` is called as bytes arrive (`total` 0 when unknown).
pub fn download(
    pack: &CorePack,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(&str),
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), BuildError> {
    let bad = BuildError::BadOutput;
    let app = app_dir().ok_or_else(|| bad("не найдена папка программы".into()))?;
    let work = app.join("esp32core-download");
    std::fs::create_dir_all(&work).map_err(|e| bad(format!("{}: {e}", work.display())))?;
    let part = work.join(format!("{}.part", pack.file));
    let url = format!("{CORE_PACKS_URL}/{}", pack.file);
    on_line(&format!("Скачиваю {} ({} МБ)...", pack.file, pack.size_mb));
    let result = fetch(&url, &part, pack, cancel, progress)
        .and_then(|()| unpack(&part, &app, cancel, on_line));
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_dir(&work);
    result?;
    if installed_in(&app.join(CORE_DIR), pack) {
        on_line(&format!("Ядро {} установлено", pack.title));
    } else {
        on_line(&format!(
            "Пакет распакован, но для {} нужен ещё и базовый пакет ESP32",
            pack.title
        ));
    }
    Ok(())
}

fn fetch(
    url: &str,
    part: &Path,
    pack: &CorePack,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), BuildError> {
    let bad = BuildError::BadOutput;
    let response = ureq::get(url)
        .call()
        .map_err(|e| bad(format!("загрузка {}: {e}", pack.file)))?;
    let total: u64 = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut reader = response.into_reader();
    let mut out =
        std::fs::File::create(part).map_err(|e| bad(format!("{}: {e}", part.display())))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(bad("загрузка отменена".into()));
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| bad(format!("загрузка {}: {e}", pack.file)))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| bad(format!("{}: {e}", part.display())))?;
        done += n as u64;
        progress(done, total);
    }
    out.flush()
        .map_err(|e| bad(format!("{}: {e}", part.display())))?;
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !pack.sha256.is_empty() && got != pack.sha256 {
        return Err(bad(format!(
            "контрольная сумма {} не совпала, файл отброшен",
            pack.file
        )));
    }
    Ok(())
}

fn unpack(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(&str),
) -> Result<(), BuildError> {
    let bad = BuildError::BadOutput;
    on_line("Распаковываю (несколько минут)...");
    sevenz_rust::decompress_file_with_extract_fn(archive, dest, |entry, reader, path| {
        if cancel.load(Ordering::Relaxed) {
            return Err(sevenz_rust::Error::other("распаковка отменена"));
        }
        // Never write outside the program folder.
        let name = Path::new(entry.name());
        if name.is_absolute()
            || name
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(sevenz_rust::Error::other("недопустимый путь в архиве"));
        }
        sevenz_rust::default_entry_extract_fn(entry, reader, path)
    })
    .map_err(|e| bad(format!("распаковка {}: {e}", archive.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(data: &Path, tools: &[&str], hardware: bool) {
        let esp = data.join("packages").join("esp32");
        if hardware {
            let h = esp.join("hardware").join("esp32").join("3.3.12");
            std::fs::create_dir_all(&h).unwrap();
            std::fs::write(h.join("boards.txt"), "x").unwrap();
        }
        for t in tools {
            let d = esp.join("tools").join(t).join("1.0");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("f"), "x").unwrap();
        }
    }

    fn pack(chip: &str) -> &'static CorePack {
        PACKS.iter().find(|p| p.chip == chip).unwrap()
    }

    #[test]
    fn chip_is_found_by_its_folders() {
        let dir = std::env::temp_dir().join(format!("flp-cores-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!installed_in(&dir, pack("esp32")));
        make(&dir, &["esp-x32", "esp32-libs"], true);
        assert!(installed_in(&dir, pack("esp32")));
        assert!(!installed_in(&dir, pack("esp32s3")));
        assert!(!installed_in(&dir, pack("esp32c3")));
        make(&dir, &["esp32s3-libs"], false);
        assert!(installed_in(&dir, pack("esp32s3")));
        make(&dir, &["esp-rv32"], false);
        assert!(!installed_in(&dir, pack("esp32c3")));
        make(&dir, &["esp32c3-libs"], false);
        assert!(installed_in(&dir, pack("esp32c3")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_tool_folder_does_not_count() {
        let dir = std::env::temp_dir().join(format!("flp-cores-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        make(&dir, &["esp-x32"], true);
        std::fs::create_dir_all(dir.join("packages/esp32/tools/esp32-libs/3.3.12")).unwrap();
        assert!(!installed_in(&dir, pack("esp32")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pack_is_chosen_by_fqbn() {
        assert_eq!(pack_for_fqbn("esp32:esp32:esp32").unwrap().chip, "esp32");
        assert_eq!(
            pack_for_fqbn("esp32:esp32:esp32s3:CDCOnBoot=cdc")
                .unwrap()
                .chip,
            "esp32s3"
        );
        assert_eq!(
            pack_for_fqbn("esp32:esp32:esp32c3").unwrap().chip,
            "esp32c3"
        );
        assert!(pack_for_fqbn("arduino:avr:uno").is_none());
        assert!(fqbn_available("arduino:avr:uno"));
    }

    /// Manual check against a real pack: `FLP_TEST_PACK=<file.7z> cargo test -p flp-build -- --ignored`.
    #[test]
    #[ignore]
    fn real_pack_unpacks() {
        let file = std::env::var("FLP_TEST_PACK").expect("FLP_TEST_PACK");
        let dir = std::env::temp_dir().join("flp-real-pack");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unpack(Path::new(&file), &dir, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(dir.join("esp32core/packages/esp32/tools").is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
