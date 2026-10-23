//! Language-independent program: blocks in execution order with resolved
//! inputs, outputs and parameter values.

use std::collections::{BTreeMap, HashMap};

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
    /// Input port → source.
    pub inputs: BTreeMap<&'a str, Input>,
    /// Output port → index in [`Program::signals`].
    pub outputs: BTreeMap<&'a str, usize>,
    /// Every declared parameter, with defaults filled in.
    pub params: BTreeMap<&'a str, ParamValue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    /// Index in [`Program::signals`].
    pub signal: usize,
    /// Type of the input port (may differ from the signal type by `int` → `float`).
    pub ty: SignalType,
}

impl<'a> Program<'a> {
    /// Checks the schema and resolves it. Fails with [`GenError::Schema`] on check errors.
    pub fn build(project: &'a Project, lib: &'a Library) -> Result<Self, GenError> {
        let logic = &project.logic;
        let report = check(logic, lib, None);
        if report.has_errors() {
            return Err(GenError::Schema(report.diagnostics));
        }
        let lib_block = |b: &Block| lib.get(&b.kind).expect("type checked by flp_core::check");

        let mut signals = Vec::new();
        let mut by_port: HashMap<(&str, &str), usize> = HashMap::new();
        let mut names: HashMap<String, PortRef> = HashMap::new();
        for &bi in &report.order {
            let block = &logic.blocks[bi];
            for out in &lib_block(block).def.outputs {
                let name = format!("{}_{}", block.id, out.name);
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
            for link in logic.links.iter().filter(|l| l.to.block == block.id) {
                let port = def.input(&link.to.port).expect("port checked");
                let signal = by_port[&(link.from.block.as_str(), link.from.port.as_str())];
                inputs.insert(
                    port.name.as_str(),
                    Input {
                        signal,
                        ty: port.ty,
                    },
                );
            }
            let outputs = def
                .outputs
                .iter()
                .map(|o| {
                    (
                        o.name.as_str(),
                        by_port[&(block.id.as_str(), o.name.as_str())],
                    )
                })
                .collect();
            let params = def
                .params
                .iter()
                .map(|p| {
                    let value = block.params.get(&p.name).or(p.default.as_ref());
                    (
                        p.name.as_str(),
                        value.expect("required params checked").clone(),
                    )
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
