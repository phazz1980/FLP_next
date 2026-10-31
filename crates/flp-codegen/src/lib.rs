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
pub mod stats;
pub mod tasks;

use std::fs;
use std::path::{Path, PathBuf};

pub use arduino::ArduinoCpp;
pub use error::GenError;
pub use lang::Lang;
pub use preview::{preview_block, preview_id};
pub use program::{Input, Instance, Program, Signal, Source};

use flp_core::{Library, Project};

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
    let mut project = project.without_disabled();
    // An unused output waits in the project tree with nothing to write: it is left
    // out of the sketch. An unused input is harmless and stays.
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
    let program = Program::build(&project, lib)?;
    backend.generate(&program)
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
