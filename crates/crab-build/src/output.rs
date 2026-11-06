//! Turning verbose `arduino-cli compile -v` output into short build stages.

/// One line of compiler output, as the user should see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputLine {
    /// A build stage, already translated.
    Stage(String),
    /// Firmware size report.
    Size(String),
    Error(String),
    Warning(String),
    /// Notes and source context around a diagnostic.
    Context(String),
    /// Compiler command lines and other noise; hidden in the short view.
    Detail,
}

/// Classifies one line of `arduino-cli compile -v` output.
pub fn classify(line: &str) -> OutputLine {
    let trimmed = line.trim();
    let stage = |s: &str| OutputLine::Stage(s.to_owned());
    if trimmed.is_empty() {
        return OutputLine::Detail;
    }
    if let Some(name) = trimmed
        .strip_prefix("Compiling library \"")
        .and_then(|s| s.strip_suffix('"'))
    {
        return OutputLine::Stage(format!("Компиляция библиотеки {name}..."));
    }
    match trimmed {
        "Detecting libraries used..." => return stage("Поиск используемых библиотек..."),
        "Generating function prototypes..." => return stage("Генерация прототипов функций..."),
        "Compiling sketch..." => return stage("Компиляция скетча..."),
        "Compiling libraries..." => return stage("Компиляция библиотек..."),
        "Compiling core..." => return stage("Компиляция ядра..."),
        "Linking everything together..." => return stage("Линковка..."),
        _ => {}
    }
    if trimmed.starts_with("Using precompiled core") {
        return stage("Ядро: из кэша");
    }
    if let Some(size) = size_line(trimmed) {
        return OutputLine::Size(size);
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("error:")
        || lower.contains("fatal error")
        || lower.contains("undefined reference")
        || lower.starts_with("error ")
        || lower.starts_with("exit status")
    {
        return OutputLine::Error(trimmed.to_owned());
    }
    if lower.contains("warning:") {
        return OutputLine::Warning(trimmed.to_owned());
    }
    if lower.contains("note:")
        || trimmed.starts_with("In file included from")
        || trimmed.starts_with("from ")
        || line.contains(": In function")
        || line.contains(": In member function")
        || is_source_context(trimmed)
    {
        return OutputLine::Context(line.trim_end().to_owned());
    }
    OutputLine::Detail
}

/// Position in `sketch.ino` and message of a compiler diagnostic
/// (`C:\p\sketch.ino:12:5: error: ...`).
pub fn parse_diagnostic(line: &str) -> Option<(usize, String)> {
    let rest = &line[line.find("sketch.ino:")? + "sketch.ino:".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let number = digits.parse().ok()?;
    let message = rest.split_once("error:")?.1.trim();
    Some((number, message.to_owned()))
}

/// Block whose generated code covers `line` (1-based) of the sketch: the nearest
/// `// id: kind` comment above it that names a block of the schema.
pub fn locate_block<'a>(
    sketch: &str,
    line: usize,
    blocks: &'a [(String, String)],
) -> Option<&'a (String, String)> {
    sketch
        .lines()
        .take(line)
        .collect::<Vec<_>>()
        .iter()
        .rev()
        .find_map(|l| {
            let (id, kind) = l.trim().strip_prefix("// ")?.split_once(": ")?;
            blocks.iter().find(|(i, k)| i == id && k == kind.trim())
        })
}

/// Plain-language explanation of a common compiler error.
pub fn explain_compile(message: &str) -> Option<&'static str> {
    let m = message.to_lowercase();
    let hint = if m.contains("no such file or directory") {
        "не найден заголовочный файл: скорее всего, не установлена Arduino-библиотека блока"
    } else if m.contains("was not declared in this scope") {
        "имя не объявлено: опечатка в шаблоне блока или не подключена нужная библиотека"
    } else if m.contains("does not name a type") {
        "неизвестный тип: не подключён заголовок библиотеки или опечатка в типе"
    } else if m.contains("undefined reference") {
        "функция объявлена, но нигде не определена: не хватает библиотеки или кода в шаблоне"
    } else if m.contains("multiple definition")
        || m.contains("redefinition of")
        || m.contains("redeclared")
    {
        "имя объявлено дважды: два блока создают одинаковую переменную или функцию"
    } else if m.contains("no matching function")
        || m.contains("invalid conversion")
        || m.contains("cannot convert")
    {
        "типы не совпадают: проверьте типы входов и параметров блока"
    } else if m.contains("expected") {
        "синтаксическая ошибка в сгенерированном коде: проверьте шаблон блока (скобки, «;»)"
    } else if m.contains("overflowed") || m.contains("will not fit in region") {
        "прошивка не помещается в память платы: выберите большую схему разделов или уберите блоки"
    } else {
        return None;
    };
    Some(hint)
}

/// Explains an error of the upload tool (esptool) in terms of the project: a wrong
/// board, a busy port, a chip that does not answer.
pub fn explain_upload(line: &str, fqbn: &str) -> Option<String> {
    let l = line.trim();
    if let Some(rest) = l.split_once("This chip is ").map(|(_, r)| r) {
        let mut words = rest.split_whitespace();
        let found = words.next()?;
        let wanted = words.nth(1)?.trim_end_matches('.');
        return Some(format!(
            "Неверная плата: к порту подключён {found}, а в проекте выбрана плата для {wanted} ({fqbn}).              Смените плату в настройках платы."
        ));
    }
    let lower = l.to_lowercase();
    if lower.contains("could not open port") || lower.contains("access is denied") {
        Some(
            "Порт занят или недоступен: закройте монитор порта и отладку, отключите и подключите плату"
                .to_owned(),
        )
    } else if lower.contains("failed to connect")
        || lower.contains("no serial data received")
        || lower.contains("wrong boot mode")
    {
        Some(
            "Плата не отвечает: проверьте кабель и порт; удерживайте кнопку BOOT во время заливки"
                .to_owned(),
        )
    } else {
        None
    }
}

/// Translates the size report, keeping its numbers.
fn size_line(trimmed: &str) -> Option<String> {
    let numbers: Vec<&str> = trimmed
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();
    match (trimmed, numbers.as_slice()) {
        (t, [used, pct, max]) if t.starts_with("Sketch uses") => {
            Some(format!("Прошивка: {used} байт ({pct}%) из {max}"))
        }
        (t, [used, pct, free, max]) if t.starts_with("Global variables use") => Some(format!(
            "Глобальные переменные: {used} байт ({pct}%) из {max}, свободно {free}"
        )),
        _ => None,
    }
}

/// gcc source excerpts: `  12 |   foo();` and the caret line `     |   ^~~`.
fn is_source_context(trimmed: &str) -> bool {
    let rest = trimmed.trim_start_matches(|c: char| c.is_ascii_digit());
    rest.trim_start().starts_with('|')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_are_translated() {
        assert_eq!(
            classify("Compiling sketch..."),
            OutputLine::Stage("Компиляция скетча...".into())
        );
        assert_eq!(
            classify("Compiling library \"OneWire\""),
            OutputLine::Stage("Компиляция библиотеки OneWire...".into())
        );
        assert_eq!(
            classify("Linking everything together..."),
            OutputLine::Stage("Линковка...".into())
        );
    }

    #[test]
    fn diagnostics_and_size() {
        assert!(matches!(
            classify("C:\\p\\sketch.ino:5:3: error: 'foo' was not declared"),
            OutputLine::Error(_)
        ));
        assert!(matches!(
            classify("C:\\p\\sketch.ino:7:1: warning: unused variable 'x'"),
            OutputLine::Warning(_)
        ));
        assert!(matches!(
            classify("    5 |   foo();"),
            OutputLine::Context(_)
        ));
        assert!(matches!(classify("      |   ^~~"), OutputLine::Context(_)));
        assert!(matches!(
            classify("Sketch uses 270000 bytes (20%) of program storage space."),
            OutputLine::Detail
        ));
        assert_eq!(
            classify(
                "Sketch uses 242513 bytes (18%) of program storage space. Maximum is 1310720 bytes."
            ),
            OutputLine::Size("Прошивка: 242513 байт (18%) из 1310720".into())
        );
        assert_eq!(
            classify(
                "Global variables use 21232 bytes (6%) of dynamic memory, leaving 306448 bytes                  for local variables. Maximum is 327680 bytes."
            ),
            OutputLine::Size(
                "Глобальные переменные: 21232 байт (6%) из 327680, свободно 306448".into()
            )
        );
    }

    #[test]
    fn compiler_commands_are_hidden() {
        assert_eq!(
            classify("\"C:\\tools\\xtensa-esp32-elf-g++\" -c -Os -w sketch.cpp -o sketch.o"),
            OutputLine::Detail
        );
        assert_eq!(
            classify("Using previously compiled file: C:\\b\\core.a"),
            OutputLine::Detail
        );
    }

    #[test]
    fn diagnostics_are_located_in_blocks() {
        let line = r"C:\w\sketch.ino:5:3: error: 'foo' was not declared in this scope";
        let (n, msg) = parse_diagnostic(line).unwrap();
        assert_eq!(n, 5);
        assert!(explain_compile(&msg).is_some());
        let sketch = "// header: note
// pump: relay
int a;
  // led: digital_out
foo();
bar();
";
        let blocks = vec![
            ("pump".to_owned(), "relay".to_owned()),
            ("led".to_owned(), "digital_out".to_owned()),
        ];
        assert_eq!(locate_block(sketch, 5, &blocks).unwrap().0, "led");
        assert_eq!(locate_block(sketch, 3, &blocks).unwrap().0, "pump");
        assert!(locate_block(sketch, 1, &blocks).is_none());
    }

    #[test]
    fn wrong_chip_is_explained() {
        let hint = explain_upload(
            "A fatal error occurred: This chip is ESP32-C3 not ESP32. Wrong --chip argument?",
            "esp32:esp32:esp32",
        )
        .unwrap();
        assert!(hint.contains("ESP32-C3") && hint.contains("ESP32"));
        assert!(explain_upload("Writing at 0x1000...", "x").is_none());
    }
}
