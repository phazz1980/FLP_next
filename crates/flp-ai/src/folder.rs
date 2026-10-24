//! [`Context`] over a project folder on disk: used by the MCP server and usable
//! without the editor. The project is re-read before each command so changes
//! saved by the editor are seen, and saved after each edit.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use flp_build::{ArduinoCli, BuildMode, CompileOptions};
use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::{Library, Project};

use crate::Context;

pub struct FolderContext {
    root: PathBuf,
    lib_dir: PathBuf,
    project: Project,
    lib: Library,
}

impl FolderContext {
    pub fn open(root: &Path, lib_dir: &Path) -> Result<Self, String> {
        let (lib, _) = Library::load(lib_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.to_owned(),
            lib_dir: lib_dir.to_owned(),
            project: Project::load(root).map_err(|e| e.to_string())?,
            lib,
        })
    }

    /// Re-reads the project from disk (call before each command).
    pub fn refresh(&mut self) -> Result<(), String> {
        self.project = Project::load(&self.root).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Context for FolderContext {
    fn project(&self) -> &Project {
        &self.project
    }

    fn library(&self) -> &Library {
        &self.lib
    }

    fn library_dir(&self) -> PathBuf {
        self.lib_dir.clone()
    }

    fn edit(
        &mut self,
        edit: &mut dyn FnMut(&mut Project) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut changed = self.project.clone();
        edit(&mut changed)?;
        changed.save().map_err(|e| e.to_string())?;
        self.project = changed;
        Ok(())
    }

    fn build(&mut self, upload: bool, port: Option<String>, debug: bool) -> Result<String, String> {
        let program = Program::build(&self.project, &self.lib).map_err(|e| e.to_string())?;
        let code = ArduinoCpp { debug }
            .generate(&program)
            .map_err(|e| e.to_string())?;
        flp_codegen::write_sketch(&self.root, &code).map_err(|e| e.to_string())?;
        let cli = ArduinoCli::locate().map_err(|e| e.to_string())?;
        let mut log = Vec::new();
        let mut collect = |line: &str| log.push(line.to_owned());
        let board = &self.project.config.board;
        cli.ensure_libraries(&program.libraries(), &mut collect)
            .map_err(|e| e.to_string())?;
        let opts = CompileOptions {
            mode: if debug {
                BuildMode::Debug
            } else {
                BuildMode::Release
            },
            ..Default::default()
        };
        let tail = |log: &[String]| log[log.len().saturating_sub(15)..].join("\n");
        if let Err(e) = cli.compile(&self.root, board, &opts, &mut collect) {
            return Err(format!("{e}\n{}", tail(&log)));
        }
        if !upload {
            return Ok(tail(&log));
        }
        let port = cli
            .choose_port(port.as_deref(), board)
            .map_err(|e| e.to_string())?;
        if let Err(e) = cli.upload(&self.root, board, &port, &mut collect) {
            return Err(format!("{e}\n{}", tail(&log)));
        }
        Ok(format!("Прошивка залита на {port}\n{}", tail(&log)))
    }

    fn live_values(&mut self) -> Result<Vec<(String, String, bool)>, String> {
        let port = self
            .project
            .config
            .board
            .port
            .clone()
            .ok_or("порт платы не задан в project.toml ([board] port = \"COM3\")")?;
        let program = Program::build(&self.project, &self.lib).map_err(|e| e.to_string())?;
        let table = flp_codegen::debug::debug_table(&program);
        let session =
            flp_debug::Session::start(table.clone(), flp_debug::transport::serial(&port), || {});
        // Wait for a full set of values or give up after a few seconds.
        let mut live = session.snapshot();
        for _ in 0..30 {
            thread::sleep(Duration::from_millis(100));
            live = session.snapshot();
            if live.schema_matches == Some(false) || live.values.iter().all(Option::is_some) {
                break;
            }
        }
        session.stop();
        if live.schema_matches != Some(true) {
            return Err(format!("нет живых значений: {}", live.status));
        }
        Ok(table
            .entries
            .iter()
            .zip(live.values.iter().zip(&live.forced))
            .map(|(e, (v, f))| {
                let name = match &e.source {
                    flp_debug::Source::Output(p) => p.to_string(),
                    flp_debug::Source::Var(v) => format!("var {v}"),
                };
                (
                    name,
                    v.map_or("—".to_owned(), |v| v.to_string()),
                    f.is_some(),
                )
            })
            .collect())
    }

    fn reload_library(&mut self) -> Result<(), String> {
        let (lib, errors) = Library::load(&self.lib_dir).map_err(|e| e.to_string())?;
        self.lib = lib;
        match errors.first() {
            Some(e) => Err(e.to_string()),
            None => Ok(()),
        }
    }
}
