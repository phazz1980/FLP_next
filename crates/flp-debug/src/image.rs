//! Memory image of the watched values.
//!
//! The firmware copies every watched value into one packed structure at the end
//! of a scan; the editor reads that image by offsets. Fields are sorted from the
//! largest type to the smallest, so each one is naturally aligned without
//! padding. The order is stable: equal sizes keep the table order (execution
//! order), so the layout is a pure function of the table.

use flp_core::SignalType;

use crate::table::{Entry, Value};

/// How a watched value is stored in the firmware. The numbers are the type
/// codes of the generated sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool = 0,
    I32 = 1,
    F32 = 2,
    I8 = 3,
    U8 = 4,
    I16 = 5,
    U16 = 6,
    U32 = 7,
    I64 = 8,
    U64 = 9,
    F64 = 10,
}

impl Kind {
    pub fn of(e: &Entry) -> Self {
        match e.cpp_type {
            "char" | "int8_t" => Kind::I8,
            "byte" | "uint8_t" => Kind::U8,
            "short" | "int16_t" => Kind::I16,
            "word" | "uint16_t" => Kind::U16,
            "unsigned int" | "unsigned long" | "uint32_t" => Kind::U32,
            "long long" | "int64_t" => Kind::I64,
            "unsigned long long" | "uint64_t" => Kind::U64,
            "double" => Kind::F64,
            _ => match e.ty {
                SignalType::Bool => Kind::Bool,
                SignalType::Int => Kind::I32,
                SignalType::Float => Kind::F32,
                SignalType::String | SignalType::Any | SignalType::Number | SignalType::Same => {
                    unreachable!("strings are not in the debug table")
                }
            },
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    /// Size in the image, bytes.
    pub fn size(self) -> usize {
        match self {
            Kind::Bool | Kind::I8 | Kind::U8 => 1,
            Kind::I16 | Kind::U16 => 2,
            Kind::I32 | Kind::F32 | Kind::U32 => 4,
            Kind::I64 | Kind::U64 | Kind::F64 => 8,
        }
    }

    /// C++ type of the field in the image structure.
    pub fn cpp(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::I32 => "int32_t",
            Kind::F32 => "float",
            Kind::I8 => "int8_t",
            Kind::U8 => "uint8_t",
            Kind::I16 => "int16_t",
            Kind::U16 => "uint16_t",
            Kind::U32 => "uint32_t",
            Kind::I64 => "int64_t",
            Kind::U64 => "uint64_t",
            Kind::F64 => "double",
        }
    }

    /// The value as the editor shows it: like the 4-byte wire form of protocol v2
    /// (wide integers are cut to 32 bits, a double becomes a float).
    /// `bytes` is exactly [`Kind::size`] bytes, little-endian.
    pub fn decode(self, bytes: &[u8]) -> Value {
        let le = |n: usize| {
            let mut b = [0u8; 8];
            b[..n].copy_from_slice(&bytes[..n]);
            u64::from_le_bytes(b)
        };
        match self {
            Kind::Bool => Value::Bool(bytes[0] != 0),
            Kind::I8 => Value::Int(i32::from(bytes[0] as i8)),
            Kind::U8 => Value::Int(i32::from(bytes[0])),
            Kind::I16 => Value::Int(i32::from(le(2) as u16 as i16)),
            Kind::U16 => Value::Int(i32::from(le(2) as u16)),
            Kind::I32 | Kind::U32 => Value::Int(le(4) as u32 as i32),
            Kind::I64 | Kind::U64 => Value::Int(le(8) as u32 as i32),
            Kind::F32 => Value::Float(f32::from_bits(le(4) as u32)),
            Kind::F64 => Value::Float(f64::from_bits(le(8)) as f32),
        }
    }
}

/// Place of one watched value in the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub offset: usize,
    pub kind: Kind,
}

/// Offsets of all table entries in the image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageLayout {
    /// By table index.
    pub slots: Vec<Slot>,
    /// Table indices in image order (largest type first).
    pub order: Vec<usize>,
    /// Size of the image, bytes.
    pub size: usize,
}

impl ImageLayout {
    pub fn new(entries: &[Entry]) -> Self {
        let kinds: Vec<Kind> = entries.iter().map(Kind::of).collect();
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(kinds[i].size()));
        let mut slots = vec![
            Slot {
                offset: 0,
                kind: Kind::Bool
            };
            entries.len()
        ];
        let mut size = 0;
        for &i in &order {
            slots[i] = Slot {
                offset: size,
                kind: kinds[i],
            };
            size += kinds[i].size();
        }
        ImageLayout { slots, order, size }
    }

    /// Value of table entry `index` in `image`; `None` when the image is too short.
    pub fn read(&self, image: &[u8], index: usize) -> Option<Value> {
        let slot = self.slots.get(index)?;
        let bytes = image.get(slot.offset..slot.offset + slot.kind.size())?;
        Some(slot.kind.decode(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Source;

    fn entry(name: &str, ty: SignalType, cpp_type: &'static str) -> Entry {
        Entry {
            source: Source::Var(name.into()),
            ty,
            storage: name.into(),
            cpp_type,
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("a", SignalType::Bool, ""),
            entry("b", SignalType::Float, ""),
            entry("c", SignalType::Int, "int16_t"),
            entry("d", SignalType::Float, "double"),
            entry("e", SignalType::Bool, ""),
            entry("f", SignalType::Int, ""),
        ]
    }

    #[test]
    fn largest_first_without_gaps() {
        let layout = ImageLayout::new(&sample());
        // d(8) b(4) f(4) c(2) a(1) e(1): equal sizes keep the table order.
        assert_eq!(layout.order, vec![3, 1, 5, 2, 0, 4]);
        let offsets: Vec<usize> = layout.slots.iter().map(|s| s.offset).collect();
        assert_eq!(offsets, vec![18, 8, 16, 0, 19, 12]);
        assert_eq!(layout.size, 20);
        // Every field is aligned to its own size.
        for s in &layout.slots {
            assert_eq!(s.offset % s.kind.size(), 0);
        }
    }

    #[test]
    fn reads_by_offsets() {
        let layout = ImageLayout::new(&sample());
        let mut image = vec![0u8; layout.size];
        let put = |image: &mut Vec<u8>, i: usize, bytes: &[u8]| {
            let o = layout.slots[i].offset;
            image[o..o + bytes.len()].copy_from_slice(bytes);
        };
        put(&mut image, 0, &[1]);
        put(&mut image, 1, &2.5f32.to_le_bytes());
        put(&mut image, 2, &(-7i16).to_le_bytes());
        put(&mut image, 3, &1.5f64.to_le_bytes());
        put(&mut image, 5, &(-100_000i32).to_le_bytes());
        assert_eq!(layout.read(&image, 0), Some(Value::Bool(true)));
        assert_eq!(layout.read(&image, 1), Some(Value::Float(2.5)));
        assert_eq!(layout.read(&image, 2), Some(Value::Int(-7)));
        assert_eq!(layout.read(&image, 3), Some(Value::Float(1.5)));
        assert_eq!(layout.read(&image, 4), Some(Value::Bool(false)));
        assert_eq!(layout.read(&image, 5), Some(Value::Int(-100_000)));
        assert_eq!(layout.read(&image[..5], 5), None);
        assert_eq!(layout.read(&image, 6), None);
    }

    #[test]
    fn layout_is_deterministic() {
        assert_eq!(ImageLayout::new(&sample()), ImageLayout::new(&sample()));
        assert_eq!(ImageLayout::new(&[]).size, 0);
    }
}
