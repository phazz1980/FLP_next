//! Language-independent program: blocks in execution order with resolved
//! inputs, outputs and parameter values.

use std::collections::{BTreeMap, HashMap};

use flp_core::ident::c_ident;
use flp_core::{check, Block, Library, LibraryBlock, ParamValue, PortRef, Project, SignalType};

use crate::GenError;

#[derive(Debug)]
pub struct Program<'a> {
    pub project: &'a Project,
    /// Blocks in execution order.
    pub blocks: Vec<Instance<'a>>,
    /// One variable per block output, in execution order of the producing blocks.
    pub signals: Vec<Signal>,
}

/// Variable holding the value of one block output between scans.
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    /// Variable name, `<block>_<port>`.
    pub name: String,
    pub ty: SignalType,
}

#[derive(Debug)]
pub struct Instance<'a> {
    pub block: &'a Block,
    pub lib: &'a LibraryBlock,
    /// Every enabled input port → its source (a link or the port default).
    pub inputs: BTreeMap<&'a str, Input>,
    /// Enabled output port → index in [`Program::signals`].
    pub outputs: BTreeMap<&'a str, usize>,
    /// Every parameter that has a value, with defaults filled in.
    pub params: BTreeMap<&'a str, ParamValue>,
}

/// Header of the flash storage used by variables with `retain`.
const RETAIN_HEADER: &str = "Preferences.h";

fn unique<'a>(items: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for item in items {
        if !out.contains(&item.as_str()) {
            out.push(item);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub source: Source,
    /// Type of the input port (may differ from the signal type by `int` → `float`).
    pub ty: SignalType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Index in [`Program::signals`].
    Signal(usize),
    /// Default value of an unconnected input.
    Default(ParamValue),
}

impl<'a> Program<'a> {
    /// Headers needed by the blocks, without duplicates, in order of first use.
    pub fn includes(&self) -> Vec<&'a str> {
        let mut out = unique(self.blocks.iter().flat_map(|b| &b.lib.def.info.includes));
        if self.project.logic.vars.iter().any(|v| v.retain) && !out.contains(&RETAIN_HEADER) {
            out.push(RETAIN_HEADER);
        }
        out
    }

    /// Arduino libraries needed by the blocks, without duplicates.
    pub fn libraries(&self) -> Vec<&'a str> {
        unique(self.blocks.iter().flat_map(|b| &b.lib.def.info.libraries))
    }

    /// Checks the schema and resolves it. Fails with [`GenError::Schema`] on check errors.
    pub fn build(project: &'a Project, lib: &'a Library) -> Result<Self, GenError> {
        let logic = &project.logic;
        let report = check(logic, lib, None);
        if report.has_errors() {
            return Err(GenError::Schema(report.diagnostics));
        }
        let lib_block = |b: &Block| {
            lib.resolve(logic, b)
                .expect("type checked by flp_core::check")
        };

        let mut signals = Vec::new();
        let mut by_port: HashMap<(&str, &str), usize> = HashMap::new();
        let mut names: HashMap<String, PortRef> = HashMap::new();
        for &bi in &report.order {
            let block = &logic.blocks[bi];
            for out in lib_block(block).def.active_outputs(&block.params) {
                // Ids and port names may be Cyrillic; the C++ name is transliterated.
                let name = format!("{}_{}", c_ident(&block.id), c_ident(&out.name));
                let port = PortRef::new(&block.id, &out.name);
                if let Some(a) = names.insert(name.clone(), port.clone()) {
                    return Err(GenError::NameClash { name, a, b: port });
                }
                by_port.insert((&block.id, &out.name), signals.len());
                signals.push(Signal { name, ty: out.ty });
            }
        }

        let mut blocks = Vec::with_capacity(report.order.len());
        for &bi in &report.order {
            let block = &logic.blocks[bi];
            let lib = lib_block(block);
            let def = &lib.def;
            let mut inputs = BTreeMap::new();
            for port in def.active_inputs(&block.params) {
                let link = logic
                    .links
                    .iter()
                    .find(|l| l.to.block == block.id && l.to.port == port.name);
                let source = match (link, &port.default) {
                    (Some(l), _) => {
                        Source::Signal(by_port[&(l.from.block.as_str(), l.from.port.as_str())])
                    }
                    (None, Some(value)) => Source::Default(value.clone()),
                    (None, None) => unreachable!("unconnected inputs checked"),
                };
                inputs.insert(
                    port.name.as_str(),
                    Input {
                        source,
                        ty: port.ty,
                    },
                );
            }
            let outputs = def
                .active_outputs(&block.params)
                .map(|o| {
                    (
                        o.name.as_str(),
                        by_port[&(block.id.as_str(), o.name.as_str())],
                    )
                })
                .collect();
            // A parameter switched off by its condition may have no value.
            let params = def
                .params
                .iter()
                .filter_map(|p| {
                    let value = block.params.get(&p.name).or(p.default.as_ref())?;
                    Some((p.name.as_str(), value.clone()))
                })
                .collect();
            blocks.push(Instance {
                block,
                lib,
                inputs,
                outputs,
                params,
            });
        }

        Ok(Self {
            project,
            blocks,
            signals,
        })
    }
}
