//! One sheet of the schema as a WASM module: which blocks it holds and which values cross
//! its border. A value crosses the border as a tag of the master (`tag_get` / `tag_set`):
//! a variable used on several sheets, or the output of a block linked to a block on
//! another sheet. Tag numbers follow one project-wide table, so modules agree without
//! exchanging anything.

use std::collections::BTreeSet;

use flp_core::ident::{c_ident, ident_from_name};
use flp_core::{ParamValue, SignalType};

use crate::program::{Program, Source};

/// Longest module name the master accepts (see `flp_build::mcu_module::MAX_NAME`).
const MAX_NAME: usize = 15;

/// What a tag of the table stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagKind {
    /// A variable of the project, by name.
    Var(String),
    /// An output of a block read on another sheet, as `block.port`.
    Signal(String),
}

/// One row of the project-wide tag table.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub number: u32,
    pub kind: TagKind,
    pub ty: SignalType,
    /// Sheet that holds the initial value and writes it at start.
    pub owner: u32,
}

/// A tag as one sheet uses it.
#[derive(Debug, Clone, PartialEq)]
pub struct TagUse {
    pub tag: Tag,
    /// The sheet reads the tag (`tag_get`).
    pub reads: bool,
    /// The sheet writes the tag (`tag_set`).
    pub writes: bool,
    /// The sheet is the owner: it writes this value once at start.
    pub init: Option<ParamValue>,
}

/// A variable used on this sheet only: it lives in the memory of the module.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalVar {
    pub name: String,
    pub ty: SignalType,
    pub init: ParamValue,
}

/// A sheet cut out of a [`Program`].
#[derive(Debug, Clone, PartialEq)]
pub struct SheetModule {
    pub sheet: u32,
    /// Name for the master: `a-z`, `0-9`, `_`, up to 15 characters.
    pub name: String,
    /// Indices into [`Program::blocks`], in execution order.
    pub blocks: Vec<usize>,
    /// Tags the sheet touches, by number.
    pub tags: Vec<TagUse>,
    pub locals: Vec<LocalVar>,
}

/// Why a sheet cannot become a module.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SheetError {
    #[error("на листе {0} нет блоков или он выключен")]
    Empty(u32),
    #[error(
        "лист {sheet}: переменная «{var}» хранится во флеш; в модулях сохранение пока не поддержано"
    )]
    Retain { sheet: u32, var: String },
}

impl Program<'_> {
    /// Number of the sheet a block stands on; blocks without a position are on sheet 1.
    fn sheet_of(&self, block_id: &str) -> u32 {
        self.project
            .layout
            .blocks
            .get(block_id)
            .map_or(1, |p| p.sheet)
    }

    /// Numbers of the sheets that have blocks, ascending.
    pub fn sheets(&self) -> Vec<u32> {
        let set: BTreeSet<u32> = self
            .blocks
            .iter()
            .map(|b| self.sheet_of(&b.block.id))
            .collect();
        set.into_iter().collect()
    }

    /// The project-wide tag table: variables used on several sheets (in declaration
    /// order), then outputs linked across sheets (in program order).
    pub fn tag_table(&self) -> Vec<Tag> {
        let logic = &self.project.logic;
        let mut table = Vec::new();
        for var in &logic.vars {
            let uses = self.var_uses(&var.name);
            let sheets: BTreeSet<u32> = uses.iter().map(|u| u.0).collect();
            if sheets.len() < 2 {
                continue;
            }
            let owner = uses
                .iter()
                .filter(|u| u.1)
                .map(|u| u.0)
                .min()
                .or_else(|| sheets.first().copied())
                .expect("a var used on two sheets");
            table.push(Tag {
                number: table.len() as u32,
                kind: TagKind::Var(var.name.clone()),
                ty: var.ty.signal(),
                owner,
            });
        }
        let producer = self.producers();
        for (index, signal) in self.signals.iter().enumerate() {
            let from = producer[index];
            let from_sheet = self.sheet_of(&self.blocks[from].block.id);
            let crosses = self.blocks.iter().any(|b| {
                self.sheet_of(&b.block.id) != from_sheet
                    && b.inputs
                        .values()
                        .any(|i| matches!(i.source, Source::Signal { index: s, .. } if s == index))
            });
            if crosses {
                table.push(Tag {
                    number: table.len() as u32,
                    kind: TagKind::Signal(signal_label(self, index)),
                    ty: signal.ty,
                    owner: from_sheet,
                });
            }
        }
        table
    }

    /// Cuts sheet `sheet` out of the program.
    pub fn sheet_module(&self, sheet: u32) -> Result<SheetModule, SheetError> {
        let blocks: Vec<usize> = (0..self.blocks.len())
            .filter(|&i| self.sheet_of(&self.blocks[i].block.id) == sheet)
            .collect();
        if blocks.is_empty() {
            return Err(SheetError::Empty(sheet));
        }
        let logic = &self.project.logic;
        let table = self.tag_table();
        let producer = self.producers();
        let mut tags: Vec<TagUse> = Vec::new();
        let mut touch = |tag: &Tag, read: bool, write: bool| {
            let init = (write_owner(tag, sheet)).then(|| tag_init(self, tag));
            match tags.iter_mut().find(|t| t.tag.number == tag.number) {
                Some(t) => {
                    t.reads |= read;
                    t.writes |= write;
                }
                None => tags.push(TagUse {
                    tag: tag.clone(),
                    reads: read,
                    writes: write,
                    init,
                }),
            }
        };
        let mut locals: Vec<LocalVar> = Vec::new();
        let mut local = |name: &str| {
            if locals.iter().any(|l| l.name == name) {
                return Ok(());
            }
            let Some(var) = logic.var(name) else {
                return Ok(());
            };
            if var.retain {
                return Err(SheetError::Retain {
                    sheet,
                    var: name.to_owned(),
                });
            }
            locals.push(LocalVar {
                name: name.to_owned(),
                ty: var.ty.signal(),
                init: var.init.clone().unwrap_or_else(|| zero(var.ty.signal())),
            });
            Ok(())
        };
        for &bi in &blocks {
            let inst = &self.blocks[bi];
            for (name, read, write) in var_accesses(inst) {
                match table
                    .iter()
                    .find(|t| t.kind == TagKind::Var(name.to_owned()))
                {
                    Some(tag) => {
                        if logic.var(&name).is_some_and(|v| v.retain) {
                            return Err(SheetError::Retain {
                                sheet,
                                var: name.to_owned(),
                            });
                        }
                        touch(tag, read, write);
                    }
                    None => local(&name)?,
                }
            }
            for input in inst.inputs.values() {
                if let Source::Signal { index, .. } = input.source {
                    let from = producer[index];
                    if self.sheet_of(&self.blocks[from].block.id) != sheet {
                        let label = signal_label(self, index);
                        let tag = table
                            .iter()
                            .find(|t| t.kind == TagKind::Signal(label.clone()))
                            .expect("cross-sheet link has a tag");
                        touch(tag, true, false);
                    }
                }
            }
            for &index in inst.outputs.values() {
                let label = signal_label(self, index);
                if let Some(tag) = table
                    .iter()
                    .find(|t| t.kind == TagKind::Signal(label.clone()))
                {
                    touch(tag, false, true);
                }
            }
        }
        tags.sort_by_key(|t| t.tag.number);
        let name = self
            .project
            .layout
            .sheet(sheet)
            .map(|s| module_name(&s.name))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("sheet{sheet}"));
        Ok(SheetModule {
            sheet,
            name,
            blocks,
            tags,
            locals,
        })
    }

    /// For every signal, the index of the block that produces it.
    fn producers(&self) -> Vec<usize> {
        let mut out = vec![0; self.signals.len()];
        for (bi, b) in self.blocks.iter().enumerate() {
            for &s in b.outputs.values() {
                out[s] = bi;
            }
        }
        out
    }

    /// Every block that touches variable `name`, as `(sheet, writes)`.
    fn var_uses(&self, name: &str) -> Vec<(u32, bool)> {
        self.blocks
            .iter()
            .flat_map(|b| {
                var_accesses(b)
                    .into_iter()
                    .filter(|a| a.0 == name)
                    .map(|a| (self.sheet_of(&b.block.id), a.2))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

/// Variables a block reads or writes: `(name, reads, writes)`.
fn var_accesses(inst: &crate::program::Instance) -> Vec<(String, bool, bool)> {
    let mut out = Vec::new();
    if let Some(name) = inst.block.var_name() {
        let write = inst.block.kind == flp_core::vars::VAR_SET;
        out.push((name.to_owned(), !write, write));
    }
    for input in inst.inputs.values() {
        if let Source::Var(name) = &input.source {
            out.push((name.clone(), true, false));
        }
    }
    out.extend(
        inst.param_vars
            .values()
            .map(|v| ((*v).to_owned(), true, false)),
    );
    out.extend(
        inst.output_vars
            .values()
            .map(|v| ((*v).to_owned(), false, true)),
    );
    out
}

fn write_owner(tag: &Tag, sheet: u32) -> bool {
    tag.owner == sheet
}

/// Initial value an owner writes at start.
fn tag_init(program: &Program, tag: &Tag) -> ParamValue {
    match &tag.kind {
        TagKind::Var(name) => program
            .project
            .logic
            .var(name)
            .and_then(|v| v.init.clone())
            .unwrap_or_else(|| zero(tag.ty)),
        TagKind::Signal(_) => zero(tag.ty),
    }
}

fn zero(ty: SignalType) -> ParamValue {
    match ty {
        SignalType::Bool => ParamValue::Bool(false),
        SignalType::Int => ParamValue::Int(0),
        SignalType::String => ParamValue::Text(String::new()),
        _ => ParamValue::Float(0.0),
    }
}

/// `block.port` of a signal.
pub(crate) fn signal_label(program: &Program, index: usize) -> String {
    for b in &program.blocks {
        for (port, &s) in &b.outputs {
            if s == index {
                return format!("{}.{}", b.block.id, port);
            }
        }
    }
    unreachable!("every signal has a producer")
}

/// Module name from a sheet name: lower-case ASCII, `_` between words, at most 15 characters.
fn module_name(sheet_name: &str) -> String {
    if sheet_name.trim().is_empty() {
        return String::new();
    }
    let mut out = c_ident(&ident_from_name(sheet_name)).to_lowercase();
    out.retain(|c| c.is_ascii_alphanumeric() || c == '_');
    out.truncate(MAX_NAME);
    out.trim_end_matches('_').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_names_fit_the_master() {
        assert_eq!(
            module_name("Насос 1"),
            module_name("Насос 1").to_lowercase()
        );
        assert!(module_name("Очень длинное название листа").len() <= MAX_NAME);
        assert_eq!(module_name("Blink LED"), "blink_led");
        assert_eq!(module_name("  "), "");
    }
}
