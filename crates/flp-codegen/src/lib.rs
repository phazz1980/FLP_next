//! Firmware code generation.
//!
//! The common part turns a checked schema into a [`Program`]; a [`Backend`] renders
//! it into source code for one language. The first backend is Arduino C++
//! ([`ArduinoCpp`]); a Rust backend can be added next to it later.

pub mod arduino;
pub mod error;
pub mod program;

use std::fs;
use std::path::{Path, PathBuf};

pub use arduino::ArduinoCpp;
pub use error::GenError;
pub use program::{Input, Instance, Program, Signal, Source};

use flp_core::{Library, Project};

/// Folder inside a project with generated and build files.
pub const WORK_DIR: &str = ".flp";

/// Output for one target language.
pub trait Backend {
    /// Template file a block must have to be supported by this backend.
    fn template_file(&self) -> &'static str;

    /// Renders the whole program into source code.
    fn generate(&self, program: &Program) -> Result<String, GenError>;
}

/// Checks the project schema and generates source code with `backend`.
pub fn generate(
    project: &Project,
    lib: &Library,
    backend: &dyn Backend,
) -> Result<String, GenError> {
    let program = Program::build(project, lib)?;
    backend.generate(&program)
}

/// Writes the Arduino sketch to `<project>/.flp/sketch/sketch.ino`.
///
/// The file is rewritten only when its contents change, so the build cache
/// is not invalidated by a no-op generation. Returns the path and whether it changed.
pub fn write_sketch(project_root: &Path, code: &str) -> Result<(PathBuf, bool), GenError> {
    let dir = sketch_dir(project_root);
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
pub fn sketch_dir(project_root: &Path) -> PathBuf {
    project_root.join(WORK_DIR).join("sketch")
}
