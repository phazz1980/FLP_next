//! What the board reports: block outputs and variables, in a fixed order.
//!
//! The table is built by the code generator from the checked schema; its
//! fingerprint is compiled into the firmware so the editor can tell whether the
//! board runs the schema it shows.

use flp_core::{PortRef, SignalType};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Output of a block.
    Output(PortRef),
    /// Variable (tag).
    Var(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub source: Source,
    pub ty: SignalType,
    /// C++ name of the storage in the sketch.
    pub storage: String,
    /// C++ type of the storage; empty for the default of `ty` (bool, int32_t, float).
    pub cpp_type: &'static str,
}

/// A string the board reports: travels in its own `TEXT` frames, only on change.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEntry {
    pub source: Source,
    /// C++ name of the `String` in the sketch.
    pub storage: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebugTable {
    pub entries: Vec<Entry>,
    /// Strings: block outputs, then variables.
    pub texts: Vec<TextEntry>,
}

impl DebugTable {
    /// FNV-1a over names and types: changes whenever the reported set changes.
    pub fn fingerprint(&self) -> u32 {
        let mut hash: u32 = 0x811C_9DC5;
        for e in &self.entries {
            let tag = match e.ty {
                SignalType::Bool => 'b',
                SignalType::Int => 'i',
                SignalType::Float => 'f',
                SignalType::String => 's',
            };
            for b in format!("{}:{tag};", e.storage).bytes() {
                hash ^= u32::from(b);
                hash = hash.wrapping_mul(0x0100_0193);
            }
        }
        for t in &self.texts {
            for b in format!("{}:s;", t.storage).bytes() {
                hash ^= u32::from(b);
                hash = hash.wrapping_mul(0x0100_0193);
            }
        }
        hash
    }

    pub fn text_output_index(&self, port: &PortRef) -> Option<usize> {
        self.texts
            .iter()
            .position(|t| matches!(&t.source, Source::Output(p) if p == port))
    }

    pub fn text_var_index(&self, name: &str) -> Option<usize> {
        self.texts
            .iter()
            .position(|t| matches!(&t.source, Source::Var(v) if v == name))
    }

    pub fn output_index(&self, port: &PortRef) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| matches!(&e.source, Source::Output(p) if p == port))
    }

    pub fn var_index(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| matches!(&e.source, Source::Var(v) if v == name))
    }
}

/// A live value. Strings are not reported (values travel as 4 bytes), so the
/// table never holds a string entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Float(f32),
}

impl Value {
    pub fn from_raw(ty: SignalType, raw: u32) -> Self {
        match ty {
            SignalType::Bool => Value::Bool(raw != 0),
            SignalType::Int => Value::Int(raw as i32),
            SignalType::Float => Value::Float(f32::from_bits(raw)),
            SignalType::String => unreachable!("strings are not in the debug table"),
        }
    }

    pub fn to_raw(self) -> u32 {
        match self {
            Value::Bool(b) => u32::from(b),
            Value::Int(i) => i as u32,
            Value::Float(f) => f.to_bits(),
        }
    }

    /// Converts to the type of the slot (e.g. a typed-in number for a bool tag).
    pub fn cast(self, ty: SignalType) -> Self {
        let f = match self {
            Value::Bool(b) => f64::from(u8::from(b)),
            Value::Int(i) => f64::from(i),
            Value::Float(f) => f64::from(f),
        };
        match ty {
            SignalType::Bool => Value::Bool(f != 0.0),
            SignalType::Int => Value::Int(f.round() as i32),
            SignalType::Float => Value::Float(f as f32),
            SignalType::String => unreachable!("strings are not in the debug table"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{}", if *b { "1" } else { "0" }),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => {
                let s = format!("{x:.2}");
                let s = s.trim_end_matches('0').trim_end_matches('.');
                write!(f, "{s}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(storage: &str, ty: SignalType) -> Entry {
        Entry {
            source: Source::Var(storage.into()),
            ty,
            storage: storage.into(),
            cpp_type: "",
        }
    }

    #[test]
    fn fingerprint_follows_content() {
        let a = DebugTable {
            entries: vec![entry("x", SignalType::Bool)],
            ..Default::default()
        };
        let b = DebugTable {
            entries: vec![entry("x", SignalType::Int)],
            ..Default::default()
        };
        assert_ne!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint(), a.clone().fingerprint());
    }

    #[test]
    fn values() {
        assert_eq!(
            Value::from_raw(SignalType::Int, (-5i32) as u32),
            Value::Int(-5)
        );
        assert_eq!(
            Value::from_raw(SignalType::Float, 2.5f32.to_bits()),
            Value::Float(2.5)
        );
        assert_eq!(Value::Float(2.5).to_string(), "2.5");
        assert_eq!(Value::Float(3.0).to_string(), "3");
        assert_eq!(Value::Float(7.0).cast(SignalType::Bool), Value::Bool(true));
    }
}
