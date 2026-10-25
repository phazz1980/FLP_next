//! Blocks and links of the schema, without coordinates.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ident::is_valid_ident;
use crate::ParamValue;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Logic {
    #[serde(rename = "var", default, skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<crate::vars::Var>,
    #[serde(rename = "block", default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    #[serde(rename = "link", default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
}

impl Logic {
    pub fn block(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }
}

/// Block instance in the schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// Human-readable unique id (`pressure`, `pump`).
    pub id: String,
    /// Library block type (`analog_in`, `timer_on`).
    #[serde(rename = "type")]
    pub kind: String,
    /// All other keys of the table are parameter values.
    #[serde(flatten)]
    pub params: BTreeMap<String, ParamValue>,
}

/// Connection `from = "block.port"` → `to = "block.port"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: PortRef,
    pub to: PortRef,
}

/// Reference to a block port, written as `block.port`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PortRef {
    pub block: String,
    pub port: String,
}

impl PortRef {
    pub fn new(block: impl Into<String>, port: impl Into<String>) -> Self {
        Self {
            block: block.into(),
            port: port.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("неверная ссылка на порт «{0}»: ожидается «блок.порт»")]
pub struct PortRefError(pub String);

impl FromStr for PortRef {
    type Err = PortRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || PortRefError(s.to_owned());
        let (block, port) = s.split_once('.').ok_or_else(err)?;
        if !is_valid_ident(block) || !is_valid_ident(port) {
            return Err(err());
        }
        Ok(Self::new(block, port))
    }
}

impl TryFrom<String> for PortRef {
    type Error = PortRefError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<PortRef> for String {
    fn from(r: PortRef) -> Self {
        r.to_string()
    }
}

impl fmt::Display for PortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.block, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_ref_parse() {
        assert_eq!("low.a".parse(), Ok(PortRef::new("low", "a")));
        assert_eq!(PortRef::new("low", "out").to_string(), "low.out");
        for bad in ["low", "low.", ".a", "a.b.c", "1x.out", "a b.c"] {
            assert!(bad.parse::<PortRef>().is_err(), "{bad}");
        }
    }

    #[test]
    fn block_params_flatten() {
        let logic: Logic = toml::from_str(
            r#"
            [[block]]
            id = "t"
            type = "timer_on"
            time = "5s"
            pin = 4
            k = 2.5
            on = true

            [[link]]
            from = "a.out"
            to = "t.in"
            "#,
        )
        .unwrap();
        let b = logic.block("t").unwrap();
        assert_eq!(b.kind, "timer_on");
        assert_eq!(b.params["time"], ParamValue::Text("5s".into()));
        assert_eq!(b.params["pin"], ParamValue::Int(4));
        assert_eq!(b.params["k"], ParamValue::Float(2.5));
        assert_eq!(b.params["on"], ParamValue::Bool(true));
        assert_eq!(logic.links[0].to, PortRef::new("t", "in"));
    }

    #[test]
    fn bad_link_is_parse_error() {
        let res: Result<Logic, _> = toml::from_str("[[link]]\nfrom = \"a\"\nto = \"b.in\"\n");
        assert!(res.is_err());
    }
}
