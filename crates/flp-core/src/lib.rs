//! Core of FLP_next: project model, TOML loading/saving and the block library.
//!
//! The core knows nothing about GUI, AI or arduino-cli.

pub mod check;
pub mod edit;
pub mod error;
pub mod ident;
pub mod layout;
pub mod library;
pub mod logic;
pub mod project;
pub mod value;
pub mod vars;

mod toml_io;

pub use check::{check, Diagnostic, Report, Severity};
pub use edit::{EditError, Fragment};
pub use error::{Error, Result};
pub use layout::{BlockPos, Layout, SheetInfo};
pub use library::{BlockDef, Library, LibraryBlock, ParamDef, ParamKind, PortDef, SignalType};
pub use logic::{Block, Link, Logic, PortRef, PortRefError};
pub use project::{BoardConfig, Project, ProjectConfig, ProjectInfo};
pub use value::ParamValue;
pub use vars::{Var, VarType};
