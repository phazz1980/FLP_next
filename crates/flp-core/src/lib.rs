//! Core of FLP_next: project model, TOML loading/saving and the block library.
//!
//! The core knows nothing about GUI, AI or arduino-cli.

pub mod check;
pub mod edit;
pub mod error;
pub mod ident;
pub mod layout;
pub mod library;
pub mod local;
pub mod logic;
pub mod packed;
pub mod pins;
pub mod project;
pub mod uart;
pub mod value;
pub mod vars;

mod toml_io;

pub use check::{bindable_type, check, check_project, Diagnostic, Report, Severity};
pub use edit::{EditError, Fragment};
pub use error::{Error, Result};
pub use layout::{BlockPos, Layout, SheetInfo};
pub use library::{
    save_block, user_library_dir, BlockDef, Choice, Library, LibraryBlock, ParamDef, ParamKind,
    ParamShow, PinMode, PortDef, SignalType,
};
pub use local::LocalValues;
pub use logic::{Block, Link, Logic, PortRef, PortRefError};
pub use project::{BoardConfig, Diagnostics, Project, ProjectConfig, ProjectInfo};
pub use uart::{UartAccess, UartConfig, UartUse};
pub use value::ParamValue;
pub use vars::{Var, VarType};
