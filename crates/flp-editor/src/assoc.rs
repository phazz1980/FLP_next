//! Opening `.flpn` project files by double click: registers the running editor
//! as their handler for the current user (`HKCU\Software\Classes`, no admin rights).

use flp_core::project::PROJECT_EXT;

const PROG_ID: &str = "FLP_next.Project";

/// Registers this executable as the handler of project files.
#[cfg(windows)]
pub fn register() -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.display();
    let classes = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Classes", winreg::enums::KEY_ALL_ACCESS)
        .map_err(|e| e.to_string())?;
    let set = |path: &str, value: &str| -> std::io::Result<()> {
        let (key, _) = classes.create_subkey(path)?;
        key.set_value("", &value)
    };
    set(&format!(".{PROJECT_EXT}"), PROG_ID)
        .and_then(|()| set(PROG_ID, "Проект FLP_next"))
        .and_then(|()| set(&format!(r"{PROG_ID}\DefaultIcon"), &format!("\"{exe}\",0")))
        .and_then(|()| {
            set(
                &format!(r"{PROG_ID}\shell\open\command"),
                &format!("\"{exe}\" \"%1\""),
            )
        })
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn register() -> Result<(), String> {
    Err("связь с файлами настраивается только в Windows".into())
}
