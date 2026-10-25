use serde::{Deserialize, Serialize};

/// Value of a block parameter as written in the project file (or JSON for AI).
///
/// The meaning (pin, duration, ...) comes from the parameter kind in `block.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl std::fmt::Display for ParamValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamValue::Bool(b) => write!(f, "{b}"),
            ParamValue::Int(i) => write!(f, "{i}"),
            ParamValue::Float(x) => write!(f, "{x}"),
            ParamValue::Text(s) => f.write_str(s),
        }
    }
}

/// Parses a duration like `500ms`, `5s`, `1.5s`, `2m`, `1h` (also `мс`, `с`, `мин`, `ч`)
/// into whole milliseconds.
///
/// Returns `None` for malformed input, fractional milliseconds or values that do not
/// fit into 32 bits (`millis()` on ESP32 is 32-bit).
pub fn parse_duration_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (num, unit) = s.split_at(split);
    let num: f64 = num.parse().ok()?;
    let factor = match unit.trim() {
        "ms" | "мс" => 1.0,
        "s" | "с" => 1_000.0,
        "m" | "min" | "мин" => 60_000.0,
        "h" | "ч" => 3_600_000.0,
        _ => return None,
    };
    let ms = num * factor;
    let valid = ms.is_finite() && ms >= 0.0 && ms.fract() == 0.0 && ms <= f64::from(u32::MAX);
    valid.then_some(ms as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration_ms("5s"), Some(5_000));
        assert_eq!(parse_duration_ms("500ms"), Some(500));
        assert_eq!(parse_duration_ms("1.5s"), Some(1_500));
        assert_eq!(parse_duration_ms("2m"), Some(120_000));
        assert_eq!(parse_duration_ms("1h"), Some(3_600_000));
        assert_eq!(parse_duration_ms("3 с"), Some(3_000));
        assert_eq!(parse_duration_ms("10мин"), Some(600_000));
        assert_eq!(parse_duration_ms("5"), None);
        assert_eq!(parse_duration_ms("s"), None);
        assert_eq!(parse_duration_ms("0.5ms"), None);
        assert_eq!(parse_duration_ms("5 days"), None);
        assert_eq!(parse_duration_ms("2000h"), None);
    }
}
