//! Interface languages.
//!
//! The Russian text in the source is the key: `tr("Сохранить")` returns the translation from
//! the dictionary of the current language (`i18n/<code>.json`), or the key itself when there is
//! no translation, so a missing entry only leaves one phrase in Russian.

use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use eframe::egui;

/// Languages of the interface: the source one plus the most spoken native languages.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    Zh,
    Es,
    En,
    Hi,
    Bn,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::Ru, Lang::En, Lang::Zh, Lang::Es, Lang::Hi, Lang::Bn];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::Zh => "zh",
            Lang::Es => "es",
            Lang::En => "en",
            Lang::Hi => "hi",
            Lang::Bn => "bn",
        }
    }

    /// Name of the language in itself, shown in the language menu.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Ru => "Русский",
            Lang::Zh => "中文",
            Lang::Es => "Español",
            Lang::En => "English",
            Lang::Hi => "हिन्दी",
            Lang::Bn => "বাংলা",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }

    fn dictionary_source(self) -> &'static str {
        match self {
            Lang::Ru => "{}",
            Lang::Zh => include_str!("../i18n/zh.json"),
            Lang::Es => include_str!("../i18n/es.json"),
            Lang::En => include_str!("../i18n/en.json"),
            Lang::Hi => include_str!("../i18n/hi.json"),
            Lang::Bn => include_str!("../i18n/bn.json"),
        }
    }

    fn index(self) -> usize {
        Lang::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) as usize % Lang::ALL.len()]
}

pub fn set(lang: Lang) {
    CURRENT.store(lang.index() as u8, Ordering::Relaxed);
}

type Dictionary = HashMap<&'static str, &'static str>;

fn dictionary(lang: Lang) -> &'static Dictionary {
    static CELLS: [OnceLock<Dictionary>; 6] = [const { OnceLock::new() }; 6];
    CELLS[lang.index()].get_or_init(|| {
        let parsed: HashMap<String, String> =
            serde_json::from_str(lang.dictionary_source()).unwrap_or_default();
        // The dictionaries live as long as the program, so strings can be handed out as 'static.
        parsed
            .into_iter()
            .map(|(k, v)| {
                let k: &'static str = Box::leak(k.into_boxed_str());
                let v: &'static str = Box::leak(v.into_boxed_str());
                (k, v)
            })
            .collect()
    })
}

/// Translation of a Russian phrase into the current language.
pub fn tr(text: &str) -> &str {
    match current() {
        Lang::Ru => text,
        lang => dictionary(lang).get(text).copied().unwrap_or(text),
    }
}

/// Translates `template` and fills its `{0}`, `{1}`... placeholders with `args`.
/// `{{` and `}}` stand for literal braces, as in `format!`.
pub fn fmt(template: &str, args: &[&dyn Display]) -> String {
    let template = tr(template);
    let mut out = String::with_capacity(template.len() + 16);
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut index = String::new();
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    index.push(d);
                }
                match index.trim().parse::<usize>().ok().and_then(|i| args.get(i)) {
                    Some(arg) => out.push_str(&arg.to_string()),
                    None => {
                        out.push('{');
                        out.push_str(&index);
                        out.push('}');
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// A message kept as its Russian template and arguments, translated when it is shown, so a
/// line written before a language switch follows the switch. Arguments that are themselves
/// phrases of the dictionary are translated too.
#[derive(Debug, Clone, PartialEq)]
pub struct Msg {
    template: String,
    args: Vec<String>,
}

impl Display for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let translated: Vec<&str> = self.args.iter().map(|a| tr(a)).collect();
        let args: Vec<&dyn Display> = translated.iter().map(|a| a as &dyn Display).collect();
        f.write_str(&fmt(&self.template, &args))
    }
}

/// Builds a [`Msg`] from a template and its arguments (see [`fmt`]).
pub fn msg(template: &str, args: &[&dyn Display]) -> Msg {
    Msg {
        template: template.to_owned(),
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

/// Text of a log line: either ready (dynamic text, shown as is) or a [`Msg`].
#[derive(Debug, Clone, PartialEq)]
pub enum LogText {
    Plain(String),
    Msg(Msg),
}

impl Display for LogText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogText::Plain(s) => f.write_str(s),
            LogText::Msg(m) => m.fmt(f),
        }
    }
}

impl From<String> for LogText {
    fn from(s: String) -> Self {
        LogText::Plain(s)
    }
}

impl From<&str> for LogText {
    fn from(s: &str) -> Self {
        LogText::Msg(msg(s, &[]))
    }
}

impl From<Msg> for LogText {
    fn from(m: Msg) -> Self {
        LogText::Msg(m)
    }
}

/// Adds system fonts with the scripts the Russian/Latin default fonts of egui lack.
pub fn install_fonts(ctx: &egui::Context) {
    // Chinese, then Devanagari and Bengali (Nirmala UI covers both).
    const CANDIDATES: [(&str, &str); 4] = [
        ("cjk", r"C:\Windows\Fonts\msyh.ttc"),
        ("cjk-fallback", r"C:\Windows\Fonts\simsun.ttc"),
        ("indic", r"C:\Windows\Fonts\Nirmala.ttc"),
        ("indic-fallback", r"C:\Windows\Fonts\Nirmala.ttf"),
    ];
    let mut fonts = egui::FontDefinitions::default();
    for (name, path) in CANDIDATES {
        let Ok(bytes) = fs::read(path) else { continue };
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push(name.to_owned());
            }
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_text_is_filled_from_its_template() {
        let line = LogText::from(msg("Открыт проект {0}", &[&"x"]));
        set(Lang::Ru);
        assert_eq!(line.to_string(), "Открыт проект x");
    }

    #[test]
    fn fmt_fills_placeholders_and_keeps_braces() {
        set(Lang::Ru);
        assert_eq!(fmt("a {1} b {0} {{c}}", &[&1, &"x"]), "a x b 1 {c}");
    }

    #[test]
    fn dictionaries_parse_and_hold_only_known_placeholders() {
        for lang in Lang::ALL {
            let dict = dictionary(lang);
            for (key, value) in dict {
                for n in 0..8 {
                    let p = format!("{{{n}}}");
                    assert_eq!(
                        key.contains(&p),
                        value.contains(&p),
                        "{}: placeholder {p} in {key:?}",
                        lang.code()
                    );
                }
            }
        }
    }
}
