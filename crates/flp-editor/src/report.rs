//! "Report a bug": a small form in the editor; "Send" opens a prefilled new-issue page in
//! the browser. Nothing is sent by the editor itself; the user reviews the text and submits it.

use eframe::egui;

/// Log lines put into the report.
const LOG_LINES: usize = 12;
/// Characters of one log line.
const LINE_LEN: usize = 200;

/// State of the report window.
pub struct Form {
    pub open: bool,
    what: String,
    expected: String,
    with_env: bool,
    with_log: bool,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            open: false,
            what: String::new(),
            expected: String::new(),
            with_env: true,
            with_log: true,
        }
    }
}

impl Form {
    /// Draws the window; returns the issue URL when "Send" was pressed.
    pub fn show(&mut self, ctx: &egui::Context, facts: &Context) -> Option<String> {
        let mut send = false;
        let mut open = self.open;
        egui::Window::new(crate::i18n::tr("Сообщить об ошибке"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr("Что произошло"));
                ui.add(
                    egui::TextEdit::multiline(&mut self.what)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                ui.label(crate::i18n::tr("Что ожидалось"));
                ui.add(
                    egui::TextEdit::multiline(&mut self.expected)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(&mut self.with_env, crate::i18n::tr("Приложить версию, ОС и плату"));
                ui.checkbox(&mut self.with_log, crate::i18n::tr("Приложить последние сообщения"));
                ui.weak(crate::i18n::tr("Откроется страница GitHub с этим текстом: проверьте его и отправьте. Файл проекта .flpn можно перетащить в поле описания."));
                send = ui
                    .add_enabled(
                        !self.what.trim().is_empty(),
                        egui::Button::new(crate::i18n::tr("Отправить на GitHub")),
                    )
                    .clicked();
            });
        self.open = open;
        if !send {
            return None;
        }
        let url = issue_url(
            facts,
            &Fields {
                what: &self.what,
                expected: &self.expected,
                with_env: self.with_env,
                with_log: self.with_log,
            },
        );
        *self = Self::default();
        Some(url)
    }
}

/// What the user typed and chose.
pub struct Fields<'a> {
    pub what: &'a str,
    pub expected: &'a str,
    pub with_env: bool,
    pub with_log: bool,
}

/// Facts about the running editor and the open project.
pub struct Context<'a> {
    pub repo: &'a str,
    pub version: &'a str,
    /// Board name and FQBN of the open project, if any.
    pub board: Option<(&'a str, &'a str)>,
    /// Newest log lines, oldest first.
    pub log: Vec<String>,
}

/// URL of the new-issue form with the body filled in.
pub fn issue_url(c: &Context, f: &Fields) -> String {
    let mut body = crate::i18n::fmt(
        "## Что произошло\n{0}\n\n## Что ожидалось\n{1}\n",
        &[&f.what.trim(), &f.expected.trim()],
    );
    if f.with_env {
        body.push_str("\n## Данные\n");
        body.push_str(&format!("- Версия: {}\n", c.version));
        body.push_str(&format!("- ОС: {}\n", std::env::consts::OS));
        match c.board {
            Some((name, fqbn)) => body.push_str(&format!("- Плата: {name} (`{fqbn}`)\n")),
            None => body.push_str("- Плата: проект не открыт\n"),
        }
    }
    body.push_str("\nПроект `.flpn` можно приложить к issue перетаскиванием.\n");
    if f.with_log && !c.log.is_empty() {
        body.push_str("\n## Последние сообщения\n```\n");
        let skip = c.log.len().saturating_sub(LOG_LINES);
        for line in c.log.iter().skip(skip) {
            let line: String = line.chars().take(LINE_LEN).collect();
            body.push_str(&line);
            body.push('\n');
        }
        body.push_str("```\n");
    }
    format!(
        "https://github.com/{}/issues/new?labels=bug&title={}&body={}",
        c.repo,
        encode(&title(f.what)),
        encode(&body)
    )
}

/// Title: "Ошибка: " and the first line of the description.
fn title(what: &str) -> String {
    let first = what.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let short: String = first.trim().chars().take(80).collect();
    crate::i18n::fmt("Ошибка: {0}", &[&short])
}

/// Percent-encoding of everything but unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(what: &str) -> Fields<'_> {
        Fields {
            what,
            expected: "",
            with_env: true,
            with_log: true,
        }
    }

    #[test]
    fn url_is_encoded_and_has_the_facts() {
        let url = issue_url(
            &Context {
                repo: "o/r",
                version: "1.2.3",
                board: Some(("ESP32", "esp32:esp32:esp32")),
                log: vec!["a b&c".into()],
            },
            &fields("не работает"),
        );
        assert!(url.starts_with("https://github.com/o/r/issues/new?labels=bug&title="));
        assert!(!url.contains(' ') && !url.contains('\n'));
        assert!(url.contains("1.2.3") && url.contains("a%20b%26c"));
    }

    #[test]
    fn only_the_newest_lines_go_in() {
        let log: Vec<String> = (0..50).map(|i| format!("line{i:02}x")).collect();
        let url = issue_url(
            &Context {
                repo: "o/r",
                version: "1",
                board: None,
                log,
            },
            &fields("x"),
        );
        assert!(url.contains("line49x") && !url.contains("line00x"));
    }

    #[test]
    fn title_comes_from_the_description_and_options_are_honoured() {
        let c = Context {
            repo: "o/r",
            version: "9.9.9",
            board: None,
            log: vec!["logline".into()],
        };
        let f = Fields {
            what: "\nпадает\nещё",
            expected: "",
            with_env: false,
            with_log: false,
        };
        let url = issue_url(&c, &f);
        assert!(url.contains("title=%D0%9E%D1%88%D0%B8%D0%B1%D0%BA%D0%B0%3A%20"));
        assert!(!url.contains("9.9.9") && !url.contains("logline"));
    }
}
