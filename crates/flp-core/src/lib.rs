//! Core of CrabLogic: project model, TOML loading/saving and the block library.
//!
//! The core knows nothing about GUI, AI or arduino-cli.

pub mod blockfile;
pub mod boardfile;
pub mod boards;
pub mod check;
pub mod devices;
pub mod edit;
pub mod error;
pub mod expander;
pub mod ident;
pub mod layout;
pub mod library;
pub mod local;
pub mod logic;
pub mod pack;
pub mod packed;
pub mod pins;
pub mod project;
pub mod tags;
pub mod uart;
pub mod value;
pub mod vars;

mod toml_io;

pub use boards::{BoardDef, Boards, UserBoard};
pub use check::{bindable_type, check, check_project, DiagKind, Diagnostic, Report, Severity};
pub use edit::{EditError, Fragment};
pub use error::{Error, Result};
pub use expander::{Expander, ExpanderChip};
pub use layout::{BlockPos, Layout, SheetInfo, WireRoute};
pub use library::{
    save_block, user_library_dir, BlockDef, Choice, Library, LibraryBlock, ParamDef, ParamKind,
    ParamShow, PinMode, PortDef, SignalType,
};
pub use local::LocalValues;
pub use logic::{Block, Link, Logic, PortRef, PortRefError};
pub use pins::PinFunction;
pub use project::{BoardConfig, Diagnostics, Project, ProjectConfig, ProjectInfo};
pub use uart::{UartAccess, UartConfig, UartUse};
pub use value::ParamValue;
pub use vars::{Var, VarType};
