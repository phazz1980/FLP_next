/// Checks an identifier of a block, port, parameter or block type.
///
/// Any alphabet is allowed (`насос`, `pump_1`): a letter or `_` first, then
/// letters, digits or `_`. No spaces or dots, since links are written
/// `block.port`. Generated C++ uses [`c_ident`] of the name.
pub fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Identifier made from a human name, for the type of a new block: lower case,
/// runs of spaces and punctuation become one `_` (`Мой насос` → `мой_насос`).
/// A name that gives nothing usable becomes `block`.
pub fn ident_from_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_');
    match out.chars().next() {
        None => "block".to_owned(),
        Some(c) if c.is_alphabetic() => out.to_owned(),
        Some(_) => format!("block_{out}"),
    }
}

/// Whether `s` is already a C identifier: ASCII letters, digits and `_`, not
/// starting with a digit.
pub fn is_c_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// C++ name for any text. A C identifier stays as it is (`pump`); anything else
/// is transliterated and gets a hash of the full text, so different names never
/// meet (`nasos_1a2b3c`).
pub fn c_ident(s: &str) -> String {
    if is_c_ident(s) {
        return s.to_owned();
    }
    let mut out = String::new();
    for c in s.chars() {
        match translit(c) {
            Some(t) => out.push_str(t),
            None if c.is_ascii_alphanumeric() => out.push(c),
            None if !out.is_empty() && !out.ends_with('_') => out.push('_'),
            None => {}
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert_str(0, "n_");
    }
    if !out.ends_with('_') {
        out.push('_');
    }
    let mut hash: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        hash = (hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    format!("{out}{:06x}", hash & 0x00ff_ffff)
}

pub(crate) fn translit(c: char) -> Option<&'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idents() {
        assert!(is_valid_ident("pump"));
        assert!(is_valid_ident("_x1"));
        assert!(is_valid_ident("timer_on"));
        assert!(is_valid_ident("насос"));
        assert!(is_valid_ident("ро_r1"));
        assert!(is_valid_ident("Насос_2"));
        assert!(!is_valid_ident(""));
        assert!(!is_valid_ident("1x"));
        assert!(!is_valid_ident("a-b"));
        assert!(!is_valid_ident("два слова"));
        assert!(!is_valid_ident("a.b"));
    }

    #[test]
    fn idents_from_names() {
        assert_eq!(ident_from_name("Мой насос"), "мой_насос");
        assert_eq!(ident_from_name("  PID (v2)! "), "pid_v2");
        assert_eq!(ident_from_name("3 реле"), "block_3_реле");
        assert_eq!(ident_from_name("—"), "block");
        for s in ["Мой насос", "  PID (v2)! ", "3 реле", "—"] {
            assert!(is_valid_ident(&ident_from_name(s)), "{s}");
        }
    }

    #[test]
    fn c_idents() {
        assert_eq!(c_ident("pump"), "pump");
        assert!(c_ident("насос").starts_with("nasos_"));
        for s in ["насос", "ро_r1", "ъ1", "Давление в баке", "%", "1-й"] {
            assert!(is_c_ident(&c_ident(s)), "{s} → {}", c_ident(s));
        }
        assert_ne!(c_ident("a b"), c_ident("a-b"));
    }
}
