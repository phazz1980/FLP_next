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
}
