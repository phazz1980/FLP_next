/// Checks an identifier of a block, port or parameter.
///
/// Identifiers end up in generated C++ names, so they must be C-compatible:
/// an ASCII letter or `_` first, then ASCII letters, digits or `_`.
pub fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idents() {
        assert!(is_valid_ident("pump"));
        assert!(is_valid_ident("_x1"));
        assert!(is_valid_ident("timer_on"));
        assert!(!is_valid_ident(""));
        assert!(!is_valid_ident("1x"));
        assert!(!is_valid_ident("a-b"));
        assert!(!is_valid_ident("насос"));
    }
}
