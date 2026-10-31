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
    /// Settings of the UARTs: `[[uart]]` of the board and settings blocks.
    pub uarts: Vec<flp_core::UartConfig>,
}

impl Program<'_> {
    /// Language of the comments in the sketch.
    pub fn lang(&self) -> crate::Lang {
        crate::Lang::from_code(&self.project.config.project.comment_lang)
    }
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
    /// Output port → variable it is copied to after the block ran.
    pub output_vars: BTreeMap<&'a str, &'a str>,
    /// Parameter → variable whose value replaces the constant.
    pub param_vars: BTreeMap<&'a str, &'a str>,
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
    /// Type of the input port (may differ from the signal type by `int` → `float`);
    /// for an `any` port, the type of its source.
    pub ty: SignalType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Index in [`Program::signals`]; `invert` for an inverted logical output.
    Signal { index: usize, invert: bool },
    /// Constant on the input, or the default value of an unconnected input.
    Default(ParamValue),
    /// Input bound to a variable of the project.
    Var(String),
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

    /// Folders with the Arduino libraries attached to the blocks (`lib/` of a block
    /// folder), for `--libraries`; they need no installation.
    pub fn attached_libraries(&self) -> Vec<std::path::PathBuf> {
        let mut dirs: Vec<_> = self
            .blocks
            .iter()
            .filter_map(|b| flp_core::blockfile::library_dirs(&b.lib.dir))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    /// Checks the schema and resolves it. Fails with [`GenError::Schema`] on check errors.
    pub fn build(project: &'a Project, lib: &'a Library) -> Result<Self, GenError> {
        let logic = &project.logic;
        let report = check(logic, lib, None);
        let uarts = flp_core::uart::check_project_uarts(project, lib);
        if report.has_errors() || uarts.has_errors() {
            let mut diagnostics = report.diagnostics;
            diagnostics.extend(uarts.diagnostics);
            return Err(GenError::Schema(diagnostics));
        }
        let lib_block = |b: &Block| {
            lib.resolve(logic, b)
                .expect("type checked by flp_core::check")
        };

        let defs: Vec<_> = logic
            .blocks
            .iter()
            .map(|b| Some(&lib_block(b).def))
            .collect();
        let resolved = flp_core::check::resolved_outputs(logic, &defs, &report.order);
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
                let ty = resolved
                    .get(&(bi, out.name.as_str()))
                    .copied()
                    .unwrap_or(out.ty);
                signals.push(Signal { name, ty });
            }
        }

        let mut blocks = Vec::with_capacity(report.order.len());
        for &bi in &report.order {
            let block = &logic.blocks[bi];
            let lib = lib_block(block);
            let def = &lib.def;
            let mut inputs = BTreeMap::new();
            let same = check::same_type(logic, &defs, &resolved, block, def)
                .expect("same types checked by flp_core::check");
            for port in def.active_inputs(&block.params) {
                let link = logic
                    .links
                    .iter()
                    .find(|l| l.to.block == block.id && l.to.port == port.name);
                let constant = block.consts.get(&port.name).or(port.default.as_ref());
                let bound = block.bound.get(&port.name);
                let source = match (bound, link, constant) {
                    (Some(var), _, _) => Source::Var(var.clone()),
                    (None, Some(l), _) => Source::Signal {
                        index: by_port[&(l.from.block.as_str(), l.from.port.as_str())],
                        invert: logic
                            .block(&l.from.block)
                            .is_some_and(|b| b.inverted.contains(&l.from.port)),
                    },
                    // A default of another type on a `same` input becomes zero of the shared one.
                    (None, None, Some(value))
                        if port.ty == SignalType::Same
                            && !block.consts.contains_key(&port.name)
                            && !same.accepts(value) =>
                    {
                        Source::Default(zero_value(same))
                    }
                    (None, None, Some(value)) => Source::Default(value.clone()),
                    (None, None, None) => unreachable!("unconnected inputs checked"),
                };
                let ty = match (&source, port.ty) {
                    (_, SignalType::Same) => same,
                    (Source::Var(name), SignalType::Any | SignalType::Number) => {
                        logic.var(name).map_or(SignalType::Any, |v| v.ty.signal())
                    }
                    (Source::Signal { index, .. }, SignalType::Any | SignalType::Number) => {
                        signals[*index].ty
                    }
                    (Source::Default(value), SignalType::Any | SignalType::Number) => {
                        SignalType::of_value(value)
                    }
                    (_, ty) => ty,
                };
                inputs.insert(port.name.as_str(), Input { source, ty });
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
                    let value = block.params.get(&p.name).or(p.default.as_ref());
                    let value = match value {
                        Some(v) => v.clone(),
                        // Taken from a variable: the constant is not needed.
                        None if block.bound.contains_key(&p.name) => match p.kind {
                            flp_core::ParamKind::Int => ParamValue::Int(0),
                            flp_core::ParamKind::Float => ParamValue::Float(0.0),
                            _ => ParamValue::Text(String::new()),
                        },
                        None => return None,
                    };
                    Some((p.name.as_str(), value))
                })
                .collect();
            let output_vars = def
                .active_outputs(&block.params)
                .filter_map(|o| Some((o.name.as_str(), block.bound.get(&o.name)?.as_str())))
                .collect();
            let param_vars = def
                .active_params(&block.params)
                .filter_map(|p| Some((p.name.as_str(), block.bound.get(&p.name)?.as_str())))
                .collect();
            blocks.push(Instance {
                block,
                lib,
                inputs,
                outputs,
                params,
                output_vars,
                param_vars,
            });
        }

        Ok(Self {
            project,
            blocks,
            signals,
            uarts: flp_core::uart::project_uarts(project, lib),
        })
    }
}

/// Zero constant of a concrete signal type.
fn zero_value(ty: SignalType) -> ParamValue {
    match ty {
        SignalType::Bool => ParamValue::Bool(false),
        SignalType::Int => ParamValue::Int(0),
        SignalType::String => ParamValue::Text(String::new()),
        _ => ParamValue::Float(0.0),
    }
}
