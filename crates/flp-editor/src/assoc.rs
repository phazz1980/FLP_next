//! Opening `.crab` project files by double click: registers the running editor
//! as their handler for the current user (`HKCU\Software\Classes`, no admin rights).

use flp_core::project::PROJECT_EXT;

const PROG_ID: &str = "FLP_next.Project";
const BLOCK_PROG_ID: &str = "FLP_next.BlockPack";
const BLOCK_EXT: &str = flp_core::pack::EXTENSION;
const BOARD_PROG_ID: &str = "FLP_next.Board";
const BOARD_EXT: &str = flp_core::boardfile::EXTENSION;

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
    let class = |ext: &str, prog_id: &str, title: &str| -> std::io::Result<()> {
        set(&format!(".{ext}"), prog_id)?;
        set(prog_id, title)?;
        set(&format!(r"{prog_id}\DefaultIcon"), &format!("\"{exe}\",0"))?;
        set(
            &format!(r"{prog_id}\shell\open\command"),
            &format!("\"{exe}\" \"%1\""),
        )
    };
    class(PROJECT_EXT, PROG_ID, crate::i18n::tr("Проект CrabLogic"))
        .and_then(|()| {
            class(
                BLOCK_EXT,
                BLOCK_PROG_ID,
                crate::i18n::tr("Набор блоков CrabLogic"),
            )
        })
        .and_then(|()| class(BOARD_EXT, BOARD_PROG_ID, crate::i18n::tr("Плата CrabLogic")))
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn register() -> Result<(), String> {
    Err(crate::i18n::tr("связь с файлами настраивается только в Windows").into())
}
