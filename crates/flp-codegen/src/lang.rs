//! Language of the comments in the generated sketch (`comment_lang` of the project).

/// Comment language. Only the text of comments (and of the debug report printed
/// by the firmware) changes; identifiers and code stay the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Ru,
    En,
}

impl Lang {
    /// `ru`/`en` (case-insensitive); anything else, including an empty string, is `ru`.
    pub fn from_code(code: &str) -> Self {
        if code.trim().eq_ignore_ascii_case("en") {
            Self::En
        } else {
            Self::Ru
        }
    }

    /// The text for this language.
    pub fn pick<'a>(self, ru: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::Ru => ru,
            Self::En => en,
        }
    }
}

/// Formats the Russian or the English template; both take the same arguments.
#[macro_export]
macro_rules! locf {
    ($lang:expr, $ru:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $lang {
            $crate::Lang::Ru => format!($ru $(, $arg)*),
            $crate::Lang::En => format!($en $(, $arg)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_picks_the_language() {
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code(" EN "), Lang::En);
        assert_eq!(Lang::from_code(""), Lang::Ru);
        assert_eq!(Lang::from_code("zh"), Lang::Ru);
    }

    #[test]
    fn macro_formats_both_templates() {
        let n = 3;
        assert_eq!(locf!(Lang::Ru, "лист {n}", "sheet {n}"), "лист 3");
        assert_eq!(locf!(Lang::En, "лист {}", "sheet {}", n), "sheet 3");
    }
}
