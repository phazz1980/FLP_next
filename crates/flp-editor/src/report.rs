//! "Report a bug": opens a prefilled new-issue page in the browser.
//! Nothing is sent by the editor itself; the user reviews the text and submits it.

/// Log lines put into the report.
const LOG_LINES: usize = 12;
/// Characters of one log line.
const LINE_LEN: usize = 200;

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
pub fn issue_url(c: &Context) -> String {
    let mut body = String::from("## Что произошло\n\n\n## Что ожидалось\n\n\n## Данные\n");
    body.push_str(&format!("- Версия: {}\n", c.version));
    body.push_str(&format!("- ОС: {}\n", std::env::consts::OS));
    match c.board {
        Some((name, fqbn)) => body.push_str(&format!("- Плата: {name} (`{fqbn}`)\n")),
        None => body.push_str("- Плата: проект не открыт\n"),
    }
    body.push_str("\nПроект `.flpn` можно приложить к issue перетаскиванием.\n");
    if !c.log.is_empty() {
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
        encode("Ошибка: "),
        encode(&body)
    )
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

    #[test]
    fn url_is_encoded_and_has_the_facts() {
        let url = issue_url(&Context {
            repo: "o/r",
            version: "1.2.3",
            board: Some(("ESP32", "esp32:esp32:esp32")),
            log: vec!["a b&c".into()],
        });
        assert!(url.starts_with("https://github.com/o/r/issues/new?labels=bug&title="));
        assert!(!url.contains(' ') && !url.contains('\n'));
        assert!(url.contains("1.2.3") && url.contains("a%20b%26c"));
    }

    #[test]
    fn only_the_newest_lines_go_in() {
        let log: Vec<String> = (0..50).map(|i| format!("line{i:02}x")).collect();
        let url = issue_url(&Context {
            repo: "o/r",
            version: "1",
            board: None,
            log,
        });
        assert!(url.contains("line49x") && !url.contains("line00x"));
    }
}
