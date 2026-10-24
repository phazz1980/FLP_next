//! Variables (tags): named values declared in `logic.toml` as `[[var]]`.
//!
//! On the sheets a variable is used through blocks `var_get` (read, output `out`)
//! and `var_set` (write, input `in`) with parameter `var = "<name>"`. A variable can
//! be read in any number of places and on any sheet, and written in one place.
//! A variable nobody writes keeps its initial value, which makes it a setpoint.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::library::ESP32_CPP_TEMPLATE;
use crate::library::{BlockDef, BlockInfo, LibraryBlock, ParamDef, ParamKind, PortDef};
use crate::{Block, Logic, ParamValue, SignalType};

pub const VAR_GET: &str = "var_get";
pub const VAR_SET: &str = "var_set";
/// Parameter of variable blocks naming the variable.
pub const VAR_PARAM: &str = "var";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Var {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: SignalType,
    /// Value after power-on; false / 0 when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init: Option<ParamValue>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Logic {
    pub fn var(&self, name: &str) -> Option<&Var> {
        self.vars.iter().find(|v| v.name == name)
    }
}

impl Block {
    /// Name of the variable a `var_get` / `var_set` block accesses.
    pub fn var_name(&self) -> Option<&str> {
        if self.kind != VAR_GET && self.kind != VAR_SET {
            return None;
        }
        match self.params.get(VAR_PARAM) {
            Some(ParamValue::Text(name)) => Some(name),
            _ => None,
        }
    }
}

/// Internal library type of a variable block (`var_get_float`), or `None` for
/// ordinary blocks.
pub(crate) fn builtin_kind(logic: &Logic, block: &Block) -> Option<String> {
    if block.kind != VAR_GET && block.kind != VAR_SET {
        return None;
    }
    let ty = block
        .var_name()
        .and_then(|name| logic.var(name))
        .map_or(SignalType::Float, |v| v.ty);
    Some(format!("{}_{}", block.kind, type_suffix(ty)))
}

fn type_suffix(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int",
        SignalType::Float => "float",
    }
}

/// C++-safe name of the variable storage.
pub fn storage_name(var: &str) -> String {
    format!("var_{var}")
}

pub(crate) fn builtin_blocks() -> BTreeMap<String, LibraryBlock> {
    let mut blocks = BTreeMap::new();
    for ty in [SignalType::Bool, SignalType::Int, SignalType::Float] {
        for get in [true, false] {
            let kind = if get { VAR_GET } else { VAR_SET };
            let port = PortDef {
                name: if get { "out" } else { "in" }.to_owned(),
                ty,
                default: None,
                when: None,
                description: String::new(),
            };
            let def = BlockDef {
                info: BlockInfo {
                    kind: kind.to_owned(),
                    symbol: None,
                    name: if get {
                        "Чтение переменной"
                    } else {
                        "Запись переменной"
                    }
                    .to_owned(),
                    group: "Переменные".to_owned(),
                    description: if get {
                        "Выдаёт текущее значение переменной."
                    } else {
                        "Записывает значение в переменную."
                    }
                    .to_owned(),
                    // Reading takes the value written on this or the previous scan,
                    // so loops through variables are allowed.
                    memory: get,
                    includes: Vec::new(),
                    libraries: Vec::new(),
                    single: false,
                },
                inputs: if get { Vec::new() } else { vec![port.clone()] },
                outputs: if get { vec![port] } else { Vec::new() },
                params: vec![ParamDef {
                    name: VAR_PARAM.to_owned(),
                    kind: ParamKind::Text,
                    default: None,
                    min: None,
                    max: None,
                    when: None,
                    description: "Имя переменной".to_owned(),
                }],
            };
            let template = if get {
                "// @loop\n{{ output.out }} = var_{{ value.var }};\n"
            } else {
                "// @loop\nvar_{{ value.var }} = {{ input.in }};\n"
            };
            blocks.insert(
                format!("{kind}_{}", type_suffix(ty)),
                LibraryBlock {
                    def,
                    dir: Default::default(),
                    inline_templates: [(ESP32_CPP_TEMPLATE, template)].into(),
                },
            );
        }
    }
    blocks
}
