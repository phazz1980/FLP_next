//! Firmware code generation.
//!
//! The common part turns a checked schema into a [`Program`]; a [`Backend`] renders
//! it into source code for one language. The first backend is Arduino C++
//! ([`ArduinoCpp`]); a Rust backend can be added next to it later.

pub mod arduino;
pub mod debug;
pub mod error;
pub mod expander;
pub mod lang;
pub mod preview;
pub mod program;
pub mod retain;
pub mod sheet;
pub mod stats;
pub mod tasks;
pub mod wasm;

use std::fs;
use std::path::{Path, PathBuf};

pub use arduino::simsrc::{builtin_cpp_preview, sim_source, sim_source_builtin, SimSourceError};
pub use arduino::ArduinoCpp;
pub use error::GenError;
pub use lang::Lang;
pub use preview::{preview_block, preview_id};
pub use program::{Input, Instance, Program, Signal, Source};
pub use sheet::{LocalVar, SheetError, SheetModule, Tag, TagKind, TagUse};
pub use wasm::{WasmError, WasmModule};

use flp_core::{Library, Project};

/// Output for one target language.
pub trait Backend {
    /// Template file a block must have to be supported by this backend.
    fn template_file(&self) -> &'static str;

    /// Renders the whole program into source code.
    fn generate(&self, program: &Program) -> Result<String, GenError>;
}

/// The project as the generators take it: disabled links dropped, and an unused output
/// (it waits in the project tree with nothing to write) left out. An unused input is
/// harmless and stays.
pub fn prepare(project: &Project, lib: &Library) -> Project {
    let mut project = project.without_disabled();
    for id in project.unused_tags(lib) {
        let output = project
            .logic
            .block(&id)
            .and_then(|b| lib.resolve(&project.logic, b))
            .is_some_and(|l| l.def.outputs.is_empty());
        if output {
            project.remove_block(&id).expect("listed by unused_tags");
        }
    }
    project
}

/// Checks the project schema and generates source code with `backend`.
pub fn generate(
    project: &Project,
    lib: &Library,
    backend: &dyn Backend,
) -> Result<String, GenError> {
    let project = prepare(project, lib);
    let program = Program::build(&project, lib)?;
    backend.generate(&program)
}

/// Adds the project to the sketch: its archive (encrypted when the project has a
/// password) as `FLP_PROJECT[]` in flash. The firmware only stores it; the editor reads
/// it back from the board and opens it.
///
/// Pass the project as saved: values kept outside the file (Wi-Fi passwords) must not
/// get into the firmware image of the project.
pub fn embed_project(code: &str, project: &Project) -> Result<String, GenError> {
    let archive = project.archive_bytes()?;
    let password = &project.config.project.password;
    let mut out = if project.config.project.embed {
        let blob = flp_core::protect::seal(&archive, password)?;
        embed_blob(code, &blob)
    } else {
        code.to_owned()
    };
    // The identity of the project for the debug module's `HELLO`.
    if out.contains("flp_dbg_locked") {
        out.push_str(&format!(
            "
// Project identity (reported in the debug hello).
const uint8_t FLP_PROJECT_HASH[8] = {{ {} }};
",
            project
                .content_hash()?
                .iter()
                .map(|b| format!("0x{b:02X}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    // The password gate of the debug module (see `debug::runtime`), only where there is one.
    if let Some(auth) = flp_core::protect::auth_material(&archive, password) {
        if out.contains("flp_dbg_locked") {
            embed_auth(&mut out, &auth);
        }
    }
    Ok(out)
}

/// The access key and salt of the debug gate as `FLP_AUTH_*` constants in flash.
fn embed_auth(out: &mut String, auth: &flp_core::protect::AuthMaterial) {
    let list = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|b| format!("0x{b:02X}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    const SETUP: &str = "
void setup() {
";
    if let Some(at) = out.find(SETUP) {
        out.insert_str(
            at + SETUP.len(),
            "  asm volatile(\"\" : : \"r\"(FLP_AUTH_KEY), \"r\"(FLP_AUTH_SALT), \"r\"(&FLP_AUTH_ON));
",
        );
        out.insert_str(
            at,
            "
extern const uint8_t FLP_AUTH_ON;
extern const uint8_t FLP_AUTH_KEY[32];
extern const uint8_t FLP_AUTH_SALT[16];
",
        );
    }
    out.push_str(
        "
// Debug password gate: salt and access key (not the password itself).
",
    );
    out.push_str(
        "const uint8_t FLP_AUTH_ON = 1;
",
    );
    out.push_str(&format!(
        "const uint8_t FLP_AUTH_SALT[16] = {{ {} }};
",
        list(&auth.salt)
    ));
    out.push_str(&format!(
        "const uint8_t FLP_AUTH_KEY[32] = {{ {} }};
",
        list(&auth.key)
    ));
}

fn embed_blob(code: &str, blob: &[u8]) -> String {
    let mut out = String::with_capacity(code.len() + blob.len() * 6 + 512);
    out.push_str(code);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n// Project archive (read back by the editor).\n");
    out.push_str("const uint8_t FLP_PROJECT[] PROGMEM = {");
    for (i, byte) in blob.iter().enumerate() {
        if i % 16 == 0 {
            out.push_str("\n ");
        }
        out.push_str(&format!(" 0x{byte:02X},"));
    }
    out.push_str("\n};\n");
    out.push_str(&format!(
        "const uint32_t FLP_PROJECT_SIZE = {};\n",
        blob.len()
    ));
    // Takes the address of the array in `setup()` so the linker keeps it: nothing else
    // reads it until the board is asked for the project.
    const SETUP: &str = "\nvoid setup() {\n";
    if let Some(at) = out.find(SETUP) {
        out.insert_str(
            at + SETUP.len(),
            "  asm volatile(\"\" : : \"r\"(FLP_PROJECT));\n",
        );
        out.insert_str(
            at,
            "\nextern const uint8_t FLP_PROJECT[] PROGMEM;\nextern const uint32_t FLP_PROJECT_SIZE;\n",
        );
    }
    out
}

/// Writes the Arduino sketch to `<work_dir>/sketch/sketch.ino`
/// (`work_dir` is [`Project::work_dir`]).
///
/// The file is rewritten only when its contents change, so the build cache
/// is not invalidated by a no-op generation. Returns the path and whether it changed.
pub fn write_sketch(work_dir: &Path, code: &str) -> Result<(PathBuf, bool), GenError> {
    let dir = sketch_dir(work_dir);
    let path = dir.join("sketch.ino");
    if fs::read_to_string(&path).is_ok_and(|old| old == code) {
        return Ok((path, false));
    }
    let io_err = |source| GenError::Io {
        path: path.clone(),
        source,
    };
    fs::create_dir_all(&dir).map_err(io_err)?;
    fs::write(&path, code).map_err(io_err)?;
    Ok((path, true))
}

/// Folder of the generated sketch; arduino-cli requires it to match the `.ino` name.
pub fn sketch_dir(work_dir: &Path) -> PathBuf {
    work_dir.join("sketch")
}
