//! Self-update: download the release archive, unpack it, and let a helper (a copy of this
//! exe started with `--apply-update`) replace the installed files once the editor exits.
//!
//! A running exe cannot be overwritten on Windows, hence the helper. The replaced files go
//! to `.update_backup` next to the exe and come back if anything fails halfway. The user's
//! own blocks (`user_library/`) are not in the archive and stay untouched.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Command line flag of the helper process.
pub const APPLY_FLAG: &str = "--apply-update";
const BACKUP_DIR: &str = ".update_backup";
/// The helper waits this long for the editor to close (the user may be answering "save?").
const WAIT_EXIT: Duration = Duration::from_secs(600);

fn work_dir() -> PathBuf {
    std::env::temp_dir().join("flp_update")
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "flp-editor.exe"
    } else {
        "flp-editor"
    }
}

/// Folder with the installed editor, or `None` when the editor runs from a build folder
/// (no `library/` next to the exe): there is nothing to update in place.
pub fn install_dir() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    dir.join("library").is_dir().then_some(dir)
}

/// Downloads `url` and unpacks it into a fresh folder. `done` counts the bytes received,
/// `total` gets the archive size when the server tells it.
pub fn download_and_stage(
    url: &str,
    done: &AtomicU64,
    total: &AtomicU64,
) -> Result<PathBuf, String> {
    if !url.to_lowercase().ends_with(".zip") {
        return Err("в релизе нет архива .zip — скачайте его вручную со страницы релиза".into());
    }
    let dir = work_dir();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("временная папка: {e}"))?;
    let zip_path = dir.join("FLP_next.zip");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .build();
    let response = agent
        .get(url)
        .set(
            "User-Agent",
            concat!("FLP_next/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("скачивание: {e}"))?;
    if let Some(n) = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
    {
        total.store(n, Ordering::Relaxed);
    }
    let mut reader = response.into_reader();
    let mut file = File::create(&zip_path).map_err(|e| format!("запись архива: {e}"))?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("скачивание прервано: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("запись архива: {e}"))?;
        done.fetch_add(n as u64, Ordering::Relaxed);
    }
    drop(file);
    let staged = dir.join("new");
    unpack(&zip_path, &staged)?;
    let _ = fs::remove_file(&zip_path);
    if !staged.join(exe_name()).is_file() {
        return Err(format!("в архиве нет {}", exe_name()));
    }
    Ok(staged)
}

/// Safe relative path of an archive entry (Windows tools may write backslashes), or `None`.
fn entry_path(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in name.split('/').filter(|p| !p.is_empty()) {
        if part == ".." || part.contains(':') {
            return None;
        }
        out.push(part);
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn unpack(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("архив повреждён: {e}"))?;
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("архив повреждён: {e}"))?;
        names.push(entry.name().to_owned());
    }
    let paths: Vec<Option<PathBuf>> = names.iter().map(|n| entry_path(n)).collect();
    // The archive keeps everything inside one folder (`FLP_next/...`): strip it.
    let top = |p: &PathBuf| p.components().next().map(|c| c.as_os_str().to_owned());
    let first = paths.iter().flatten().next().and_then(top);
    let strip = first.filter(|f| {
        paths
            .iter()
            .flatten()
            .all(|p| top(p).as_deref() == Some(f.as_os_str()))
    });
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for (i, path) in paths.into_iter().enumerate() {
        let Some(mut path) = path else {
            return Err(format!("недопустимый путь в архиве: {}", names[i]));
        };
        if let Some(prefix) = &strip {
            path = path.strip_prefix(prefix).unwrap_or(&path).to_path_buf();
        }
        if path.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(&path);
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = File::create(&target).map_err(|e| format!("{}: {e}", target.display()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(())
}

/// Starts the helper that installs `staged` after this process exits.
pub fn launch_helper(staged: &Path, project: Option<&Path>) -> Result<(), String> {
    let install = install_dir().ok_or("редактор запущен не из установленной папки")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = work_dir().join(format!("flp-updater-{}.exe", std::process::id()));
    fs::copy(&exe, &helper).map_err(|e| format!("помощник обновления: {e}"))?;
    Command::new(&helper)
        .arg(APPLY_FLAG)
        .arg(staged)
        .arg(&install)
        .arg(&exe)
        .arg(std::process::id().to_string())
        .args(project)
        .spawn()
        .map_err(|e| format!("помощник обновления: {e}"))?;
    Ok(())
}

/// Entry point of the helper process:
/// `<staged> <install dir> <editor exe> <editor pid> [project to reopen]`.
pub fn run_helper(args: &[OsString]) {
    let [staged, install, exe, pid, rest @ ..] = args else {
        return;
    };
    let (staged, install, exe) = (Path::new(staged), Path::new(install), Path::new(exe));
    let pid = pid.to_string_lossy().parse::<u32>().unwrap_or(0);
    let result = wait_for_exit(pid).and_then(|()| apply(staged, install));
    let text = match &result {
        Ok(()) => "ok".to_owned(),
        Err(e) => format!("ошибка: {e}"),
    };
    let _ = fs::write(work_dir().join("update.log"), text);
    // Start the editor either way: the new version, or the old one after a rollback.
    let _ = Command::new(exe).args(rest).spawn();
}

/// Waits until the editor process that started the update is gone. Other editor windows may
/// keep running: their exe is only renamed by `apply`, which Windows allows.
fn wait_for_exit(pid: u32) -> Result<(), String> {
    let start = Instant::now();
    while is_running(pid) {
        if start.elapsed() > WAIT_EXIT {
            return Err("редактор не закрылся".into());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    // Let the system release the exe after the process is gone.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

fn is_running(pid: u32) -> bool {
    let mut cmd = Command::new("tasklist");
    cmd.args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
        .unwrap_or(false)
}

fn apply(staged: &Path, install: &Path) -> Result<(), String> {
    let backup = install.join(BACKUP_DIR);
    let _ = fs::remove_dir_all(&backup);
    fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
    let mut touched: Vec<OsString> = Vec::new();
    if let Err(e) = put_all(staged, install, &backup, &mut touched) {
        // Put the old files back: whatever was written of the new ones goes away first.
        for name in &touched {
            let dst = install.join(name);
            let _ = fs::remove_dir_all(&dst).or_else(|_| fs::remove_file(&dst));
            if backup.join(name).exists() {
                let _ = fs::rename(backup.join(name), &dst);
            }
        }
        return Err(e);
    }
    let _ = fs::remove_dir_all(staged);
    Ok(())
}

fn put_all(
    staged: &Path,
    install: &Path,
    backup: &Path,
    touched: &mut Vec<OsString>,
) -> Result<(), String> {
    for entry in fs::read_dir(staged).map_err(|e| e.to_string())? {
        let name: OsString = entry.map_err(|e| e.to_string())?.file_name();
        let dst = install.join(&name);
        touched.push(name.clone());
        if dst.exists() {
            fs::rename(&dst, backup.join(&name)).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
        copy_all(&staged.join(&name), &dst)?;
    }
    Ok(())
}

fn copy_all(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_all(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        fs::copy(src, dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    Ok(())
}

/// Result of the last helper run, shown once after the restart.
pub fn take_last_result() -> Option<String> {
    let path = work_dir().join("update.log");
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn own_process_is_running() {
        assert!(is_running(std::process::id()));
        assert!(!is_running(4_000_000_000));
    }

    #[test]
    fn entry_paths() {
        assert_eq!(
            entry_path("FLP_next\\library\\a.toml"),
            Some(PathBuf::from("FLP_next/library/a.toml"))
        );
        assert_eq!(entry_path("../x"), None);
        assert_eq!(entry_path("C:/x"), None);
        assert_eq!(entry_path("/"), None);
    }

    #[test]
    fn apply_replaces_and_keeps_user_files() {
        let root = std::env::temp_dir().join(format!("flp_upd_test_{}", std::process::id()));
        let (staged, install) = (root.join("new"), root.join("inst"));
        fs::create_dir_all(staged.join("library")).unwrap();
        fs::create_dir_all(install.join("library")).unwrap();
        fs::create_dir_all(install.join("user_library")).unwrap();
        fs::write(staged.join("flp-editor.exe"), "new").unwrap();
        fs::write(staged.join("library/a"), "new").unwrap();
        fs::write(install.join("flp-editor.exe"), "old").unwrap();
        fs::write(install.join("library/old_only"), "x").unwrap();
        fs::write(install.join("user_library/mine"), "mine").unwrap();
        apply(&staged, &install).unwrap();
        assert_eq!(
            fs::read_to_string(install.join("flp-editor.exe")).unwrap(),
            "new"
        );
        assert!(install.join("library/a").is_file());
        assert!(!install.join("library/old_only").exists());
        assert!(install.join("user_library/mine").is_file());
        assert_eq!(
            fs::read_to_string(install.join(BACKUP_DIR).join("flp-editor.exe")).unwrap(),
            "old"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
