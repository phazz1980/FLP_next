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
/// Parameter of a `var_set` block adding the bool input `en`: the value is written
/// only while it is true ("разрешение записи"). Set per block, not per variable.
pub const WRITE_ENABLE: &str = "write_enable";

/// Arduino storage type of a variable. On the sheets it carries the signal of
/// [`VarType::signal`]; the sketch stores it in the declared C++ type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VarType {
    #[serde(rename = "bool", alias = "boolean")]
    Bool,
    #[serde(rename = "char")]
    Char,
    #[serde(rename = "byte")]
    Byte,
    #[serde(rename = "short")]
    Short,
    #[serde(rename = "word")]
    Word,
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "unsigned int")]
    UnsignedInt,
    #[serde(rename = "long")]
    Long,
    #[serde(rename = "unsigned long")]
    UnsignedLong,
    #[serde(rename = "long long")]
    LongLong,
    #[serde(rename = "unsigned long long")]
    UnsignedLongLong,
    #[serde(rename = "int8_t")]
    Int8,
    #[serde(rename = "uint8_t")]
    Uint8,
    #[serde(rename = "int16_t")]
    Int16,
    #[serde(rename = "uint16_t")]
    Uint16,
    #[serde(rename = "int32_t")]
    Int32,
    #[serde(rename = "uint32_t")]
    Uint32,
    #[serde(rename = "int64_t")]
    Int64,
    #[serde(rename = "uint64_t")]
    Uint64,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "double")]
    Double,
}

impl VarType {
    pub const ALL: [VarType; 21] = [
        VarType::Bool,
        VarType::Char,
        VarType::Byte,
        VarType::Short,
        VarType::Word,
        VarType::Int,
        VarType::UnsignedInt,
        VarType::Long,
        VarType::UnsignedLong,
        VarType::LongLong,
        VarType::UnsignedLongLong,
        VarType::Int8,
        VarType::Uint8,
        VarType::Int16,
        VarType::Uint16,
        VarType::Int32,
        VarType::Uint32,
        VarType::Int64,
        VarType::Uint64,
        VarType::Float,
        VarType::Double,
    ];

    /// Name as written in Arduino code and in `logic.toml`.
    pub fn name(self) -> &'static str {
        match self {
            VarType::Bool => "bool",
            VarType::Char => "char",
            VarType::Byte => "byte",
            VarType::Short => "short",
            VarType::Word => "word",
            VarType::Int => "int",
            VarType::UnsignedInt => "unsigned int",
            VarType::Long => "long",
            VarType::UnsignedLong => "unsigned long",
            VarType::LongLong => "long long",
            VarType::UnsignedLongLong => "unsigned long long",
            VarType::Int8 => "int8_t",
            VarType::Uint8 => "uint8_t",
            VarType::Int16 => "int16_t",
            VarType::Uint16 => "uint16_t",
            VarType::Int32 => "int32_t",
            VarType::Uint32 => "uint32_t",
            VarType::Int64 => "int64_t",
            VarType::Uint64 => "uint64_t",
            VarType::Float => "float",
            VarType::Double => "double",
        }
    }

    /// C++ type of the storage in the sketch. `int` stays 32-bit as before.
    pub fn cpp(self) -> &'static str {
        match self {
            VarType::Int => "int32_t",
            other => other.name(),
        }
    }

    /// Signal type of the variable blocks on the sheets.
    pub fn signal(self) -> SignalType {
        match self {
            VarType::Bool => SignalType::Bool,
            VarType::Float | VarType::Double => SignalType::Float,
            _ => SignalType::Int,
        }
    }

    /// Allowed range of an integer type.
    pub fn int_range(self) -> Option<(i64, i64)> {
        Some(match self {
            VarType::Char | VarType::Int8 => (i8::MIN.into(), i8::MAX.into()),
            VarType::Byte | VarType::Uint8 => (0, u8::MAX.into()),
            VarType::Short | VarType::Int16 => (i16::MIN.into(), i16::MAX.into()),
            VarType::Word | VarType::Uint16 => (0, u16::MAX.into()),
            VarType::Int | VarType::Long | VarType::Int32 => (i32::MIN.into(), i32::MAX.into()),
            VarType::UnsignedInt | VarType::UnsignedLong | VarType::Uint32 => (0, u32::MAX.into()),
            VarType::LongLong | VarType::Int64 | VarType::UnsignedLongLong | VarType::Uint64 => {
                (i64::MIN, i64::MAX)
            }
            _ => return None,
        })
    }

    /// Whether `value` fits as the initial value.
    pub fn accepts(self, value: &ParamValue) -> bool {
        match (self.int_range(), value) {
            (Some((lo, hi)), ParamValue::Int(v)) => (lo..=hi).contains(v),
            (Some(_), _) => false,
            (None, _) => self.signal().accepts(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Var {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: VarType,
    /// Value after power-on; false / 0 when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init: Option<ParamValue>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Kept in flash (EEPROM on ESP32 is emulated in NVS the same way): restored
    /// after power-on, saved when the value changes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retain: bool,
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
    let var = block.var_name().and_then(|name| logic.var(name));
    let ty = var.map_or(SignalType::Float, |v| v.ty.signal());
    let en = block.kind == VAR_SET
        && matches!(block.params.get(WRITE_ENABLE), Some(ParamValue::Bool(true)));
    Some(format!(
        "{}_{}{}",
        block.kind,
        type_suffix(ty),
        if en { "_en" } else { "" }
    ))
}

fn type_suffix(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool",
        SignalType::Int => "int",
        SignalType::Float => "float",
    }
}

/// A variable name is free text (`Давление в баке`): not empty, without control
/// characters and leading or trailing spaces. The sketch uses [`storage_name`].
pub fn is_valid_var_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && !name.chars().any(char::is_control)
}

/// C++ name of the variable storage. A C identifier keeps its name (`var_t`);
/// any other name is transliterated and gets a hash of the full name, so
/// different names never meet (`var_Davlenie_v_bake_1a2b3c`).
pub fn storage_name(var: &str) -> String {
    if crate::ident::is_valid_ident(var) {
        return format!("var_{var}");
    }
    let mut out = String::from("var_");
    for c in var.chars() {
        match translit(c) {
            Some(t) => out.push_str(t),
            None if c.is_ascii_alphanumeric() => out.push(c),
            None if !out.ends_with('_') => out.push('_'),
            None => {}
        }
    }
    if !out.ends_with('_') {
        out.push('_');
    }
    let mut hash: u32 = 0x811c_9dc5;
    for b in var.bytes() {
        hash = (hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    format!("{out}{:06x}", hash & 0x00ff_ffff)
}

fn translit(c: char) -> Option<&'static str> {
    const LOWER: [&str; 32] = [
        "a", "b", "v", "g", "d", "e", "zh", "z", "i", "y", "k", "l", "m", "n", "o", "p", "r", "s",
        "t", "u", "f", "kh", "ts", "ch", "sh", "shch", "", "y", "", "e", "yu", "ya",
    ];
    const UPPER: [&str; 32] = [
        "A", "B", "V", "G", "D", "E", "Zh", "Z", "I", "Y", "K", "L", "M", "N", "O", "P", "R", "S",
        "T", "U", "F", "Kh", "Ts", "Ch", "Sh", "Shch", "", "Y", "", "E", "Yu", "Ya",
    ];
    match c {
        'а'..='я' => Some(LOWER[c as usize - 'а' as usize]),
        'А'..='Я' => Some(UPPER[c as usize - 'А' as usize]),
        'ё' => Some("e"),
        'Ё' => Some("E"),
        _ => None,
    }
}

pub(crate) fn builtin_blocks() -> BTreeMap<String, LibraryBlock> {
    let mut blocks = BTreeMap::new();
    for ty in [SignalType::Bool, SignalType::Int, SignalType::Float] {
        for (get, en) in [(true, false), (false, false), (false, true)] {
            let kind = if get { VAR_GET } else { VAR_SET };
            let port = PortDef {
                name: if get { "out" } else { "in" }.to_owned(),
                ty,
                default: None,
                when: None,
                label: None,
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
                inputs: match (get, en) {
                    (true, _) => Vec::new(),
                    (false, false) => vec![port.clone()],
                    (false, true) => vec![
                        port.clone(),
                        PortDef {
                            name: "en".to_owned(),
                            ty: SignalType::Bool,
                            default: Some(ParamValue::Bool(true)),
                            when: None,
                            label: None,
                            description: "Разрешение записи".to_owned(),
                        },
                    ],
                },
                outputs: if get { vec![port] } else { Vec::new() },
                params: {
                    let mut params = vec![ParamDef {
                        name: VAR_PARAM.to_owned(),
                        kind: ParamKind::Text,
                        default: None,
                        min: None,
                        max: None,
                        when: None,
                        label: None,
                        description: "Имя переменной".to_owned(),
                    }];
                    if !get {
                        params.push(ParamDef {
                            name: WRITE_ENABLE.to_owned(),
                            kind: ParamKind::Bool,
                            default: Some(ParamValue::Bool(false)),
                            min: None,
                            max: None,
                            when: None,
                            label: None,
                            description: "Вход разрешения записи (en)".to_owned(),
                        });
                    }
                    params
                },
            };
            let template = match (get, en) {
                (true, _) => "// @loop\n{{ output.out }} = {{ var }};\n",
                (false, false) => "// @loop\n{{ var }} = {{ input.in }};\n",
                (false, true) => {
                    "// @loop\nif ({{ input.en }}) {\n  {{ var }} = {{ input.in }};\n}\n"
                }
            };
            blocks.insert(
                format!("{kind}_{}{}", type_suffix(ty), if en { "_en" } else { "" }),
                LibraryBlock {
                    def,
                    dir: Default::default(),
                    inline_templates: [(ESP32_CPP_TEMPLATE, template.to_owned())].into(),
                },
            );
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_text_is_a_var_name() {
        assert!(is_valid_var_name("Давление в баке"));
        assert!(is_valid_var_name("t"));
        assert!(!is_valid_var_name(""));
        assert!(!is_valid_var_name(" t"));
        assert!(!is_valid_var_name("a\nb"));
    }

    #[test]
    fn storage_names_are_c_identifiers() {
        assert_eq!(storage_name("temp"), "var_temp");
        for name in ["Давление в баке", "Ёж", "1-й насос", "%", "давление в баке"]
        {
            let s = storage_name(name);
            assert!(crate::ident::is_valid_ident(&s), "{name} → {s}");
        }
        assert!(storage_name("Давление в баке").starts_with("var_Davlenie_v_bake_"));
        // Names differing only in symbols the transliteration drops stay apart.
        assert_ne!(storage_name("a b"), storage_name("a-b"));
    }
}
