//! Direct rebuild: when only the body of the sketch changed, compile it and link the
//! firmware with the commands arduino-cli ran last time, without starting arduino-cli.
//!
//! A full build saves the commands (the "recipe") in [`RECIPE`]: the sketch compile
//! command from `compile_commands.json` and the commands printed after the link stage
//! of the verbose log. Its key covers everything but the sketch body, so a new
//! `#include`, another board option or an edited library falls back to arduino-cli.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{board_options, build_dir, sketch_dir, uses_web_ui, BoardConfig, CompileOptions};
use crate::{CliReason, Fnv64, WEBUI_RESPONSE_DEFINE};

/// File in the build folder with the recipe of the last full build.
pub const RECIPE: &str = "flp-direct.json";

/// Stage line of the verbose log after which the link and image commands come.
const LINK_STAGE: &str = "Linking everything together...";

/// Commands longer than this go to the compiler through a response file
/// (the Windows command line is limited to 32K characters).
const MAX_COMMAND_LINE: usize = 8000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Recipe {
    pub key: RecipeKey,
    /// Compiles `sketch/sketch.ino.cpp` into its object file.
    pub compile: Vec<String>,
    /// Link, firmware image, partition table, size.
    pub post: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Step {
    /// A program with its arguments.
    Run(Vec<String>),
    /// A `cmd /c` command line (the part after `/c`).
    Shell(String),
}

impl Recipe {
    pub fn load(work_dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(build_dir(work_dir).join(RECIPE)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, work_dir: &Path) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            // Only a missed shortcut next time if this fails.
            let _ = std::fs::write(build_dir(work_dir).join(RECIPE), text);
        }
    }

    pub fn remove(work_dir: &Path) {
        let _ = std::fs::remove_file(build_dir(work_dir).join(RECIPE));
    }

    /// The commands, ready to start; long ones get a response file in the build folder.
    pub fn commands(&self, work_dir: &Path) -> std::io::Result<Vec<Command>> {
        let mut commands = vec![run_command(&self.compile, work_dir, 0)?];
        for (i, step) in self.post.iter().enumerate() {
            commands.push(match step {
                Step::Run(args) => run_command(args, work_dir, i + 1)?,
                Step::Shell(line) => shell_command(line)?,
            });
        }
        Ok(commands)
    }
}

fn run_command(args: &[String], work_dir: &Path, n: usize) -> std::io::Result<Command> {
    let (exe, rest) = args
        .split_first()
        .ok_or_else(|| std::io::Error::other("пустая команда в рецепте сборки"))?;
    let mut cmd = Command::new(exe);
    if args.iter().map(|a| a.len() + 3).sum::<usize>() > MAX_COMMAND_LINE {
        let rsp = build_dir(work_dir).join(format!("flp-direct-{n}.rsp"));
        std::fs::write(&rsp, response_file(rest))?;
        let mut arg = OsString::from("@");
        arg.push(rsp);
        cmd.arg(arg);
    } else {
        cmd.args(rest);
    }
    Ok(cmd)
}

#[cfg(windows)]
fn shell_command(line: &str) -> std::io::Result<Command> {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("cmd");
    cmd.raw_arg(format!("/c {line}"));
    Ok(cmd)
}

#[cfg(not(windows))]
fn shell_command(_line: &str) -> std::io::Result<Command> {
    Err(std::io::Error::other("cmd /c есть только в Windows"))
}

/// Arguments for a gcc response file: each one quoted, `\` and `"` escaped.
fn response_file(args: &[String]) -> String {
    args.iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collects the commands of the verbose log that follow the link stage.
#[derive(Debug, Default)]
pub(crate) struct Capture {
    linking: bool,
    pub post: Vec<Step>,
}

impl Capture {
    pub fn feed(&mut self, line: &str) {
        if line.trim() == LINK_STAGE {
            self.linking = true;
            self.post.clear();
        } else if self.linking {
            // Tool output (esptool's "Creating esp32 image...") is not a command.
            if let Some(step) = parse_step(line, &|exe| is_program(exe)) {
                self.post.push(step);
            }
        }
    }
}

fn is_program(exe: &str) -> bool {
    let path = Path::new(exe);
    path.is_absolute()
        && (path.is_file() || (cfg!(windows) && path.with_extension("exe").is_file()))
}

/// One command line of the verbose log: `cmd /c ...`, or a program (checked with
/// `is_program`) and its arguments. Arguments are quoted the Go way (`"C:\\x"`).
fn parse_step(line: &str, is_program: &dyn Fn(&str) -> bool) -> Option<Step> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("cmd /c ") {
        let rest: Vec<String> = split_go(rest)?
            .into_iter()
            .map(|(word, quoted)| if quoted { format!("\"{word}\"") } else { word })
            .collect();
        return Some(Step::Shell(rest.join(" ")));
    }
    let words = split_go(line)?;
    if !is_program(&words.first()?.0) {
        return None;
    }
    Some(Step::Run(words.into_iter().map(|(w, _)| w).collect()))
}

/// Splits a command line into words; a word in double quotes has Go escapes
/// (`\\`, `\"`) and is marked as quoted.
fn split_go(line: &str) -> Option<Vec<(String, bool)>> {
    let mut words = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars.next_if(|c| *c == ' ').is_some() {}
        let Some(&first) = chars.peek() else {
            return Some(words);
        };
        let mut word = String::new();
        if first == '"' {
            chars.next();
            loop {
                match chars.next()? {
                    '"' => break,
                    '\\' => word.push(chars.next()?),
                    c => word.push(c),
                }
            }
            words.push((word, true));
        } else {
            while let Some(c) = chars.next_if(|c| *c != ' ') {
                word.push(c);
            }
            words.push((word, false));
        }
    }
}

/// The command that compiled `sketch.ino.cpp`, from the compilation database
/// arduino-cli writes into the build folder.
pub(crate) fn sketch_compile_command(work_dir: &Path) -> Option<Vec<String>> {
    #[derive(Deserialize)]
    struct Entry {
        file: String,
        arguments: Vec<String>,
    }
    let text = std::fs::read_to_string(build_dir(work_dir).join("compile_commands.json")).ok()?;
    let entries: Vec<Entry> = serde_json::from_str(&text).ok()?;
    entries
        .into_iter()
        .find(|e| {
            e.file
                .replace('\\', "/")
                .ends_with("/sketch/sketch.ino.cpp")
        })
        .map(|e| e.arguments)
}

/// Fingerprint of everything but the sketch body, in parts so a mismatch can say what
/// changed. Only the sketch file names and its body are left out of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecipeKey {
    /// arduino-cli: its path and file, so an update shows.
    pub tool: String,
    /// FQBN and board options.
    pub board: String,
    /// Library folders passed to arduino-cli.
    pub libraries: String,
    /// The `#include` lines and the flags chosen from the sketch.
    pub sketch: String,
    /// Files in the include folders of the compile command (core and used libraries).
    pub files: String,
}

impl RecipeKey {
    /// The first part that differs in `now`, as the reason to run arduino-cli.
    pub fn mismatch(&self, now: &RecipeKey) -> Option<CliReason> {
        [
            (&self.tool, &now.tool, CliReason::Tool),
            (&self.board, &now.board, CliReason::Board),
            (&self.libraries, &now.libraries, CliReason::Libraries),
            (&self.sketch, &now.sketch, CliReason::Includes),
            (&self.files, &now.files, CliReason::LibraryFiles),
        ]
        .into_iter()
        .find(|(old, new, _)| old != new)
        .map(|(_, _, reason)| reason)
    }
}

/// Key of the sketch written to `work_dir`. `None` when the sketch is not a single
/// `sketch.ino`.
pub(crate) fn recipe_key(
    exe: &Path,
    work_dir: &Path,
    board: &BoardConfig,
    opts: &CompileOptions,
    compile: &[String],
) -> Option<RecipeKey> {
    let names: Vec<OsString> = std::fs::read_dir(sketch_dir(work_dir))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .collect();
    if names != ["sketch.ino"] {
        return None;
    }
    let sketch = std::fs::read_to_string(sketch_dir(work_dir).join("sketch.ino")).ok()?;
    let hex = |h: Fnv64| format!("{:016x}", h.0);

    let mut tool = Fnv64::new();
    tool.write(exe.as_os_str().as_encoded_bytes());
    write_meta(&mut tool, exe);

    let mut boards = Fnv64::new();
    boards.write(board.fqbn.as_bytes());
    for (k, v) in board_options(board) {
        boards.write(k.as_bytes());
        boards.write(v.as_bytes());
    }

    let mut libraries = Fnv64::new();
    for dir in &opts.libraries {
        libraries.write(dir.as_os_str().as_encoded_bytes());
    }

    let mut includes = Fnv64::new();
    for line in include_lines(&sketch) {
        includes.write(line.as_bytes());
    }
    if uses_web_ui(work_dir) {
        includes.write(WEBUI_RESPONSE_DEFINE.as_bytes());
    }

    let mut files = Fnv64::new();
    for dir in include_dirs(compile) {
        let mut found = Vec::new();
        list_files(&dir, &mut found);
        found.sort();
        for file in found {
            files.write(file.as_os_str().as_encoded_bytes());
            write_meta(&mut files, &file);
        }
    }
    Some(RecipeKey {
        tool: hex(tool),
        board: hex(boards),
        libraries: hex(libraries),
        sketch: hex(includes),
        files: hex(files),
    })
}

/// Size and modification time; a missing file hashes as empty.
fn write_meta(h: &mut Fnv64, path: &Path) {
    let meta = std::fs::metadata(path).ok();
    h.write(&meta.as_ref().map_or(0, |m| m.len()).to_le_bytes());
    let modified = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    h.write(&modified.to_le_bytes());
}

/// Sorted, unique `#include` lines of the sketch.
fn include_lines(sketch: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = sketch
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("#include"))
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Folders of `-I<dir>` and `-I <dir>` arguments.
fn include_dirs(args: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.strip_prefix("-I") {
            Some("") => dirs.extend(iter.next().map(PathBuf::from)),
            Some(dir) => dirs.push(PathBuf::from(dir)),
            None => {}
        }
    }
    dirs
}

fn list_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => list_files(&path, out),
            Ok(_) => out.push(path),
            Err(_) => {}
        }
    }
}

/// Writes `sketch/sketch.ino.cpp` in the build folder the way arduino-cli does, minus
/// the generated prototypes (the generated code declares everything before use).
pub(crate) fn write_sketch_cpp(work_dir: &Path) -> std::io::Result<()> {
    let ino = sketch_dir(work_dir).join("sketch.ino");
    let text = std::fs::read_to_string(&ino)?;
    let cpp = build_dir(work_dir).join("sketch").join("sketch.ino.cpp");
    std::fs::write(cpp, sketch_cpp(&ino, &text))
}

fn sketch_cpp(ino: &Path, text: &str) -> String {
    let mut out = String::new();
    if !text.contains("#include <Arduino.h>") {
        out.push_str("#include <Arduino.h>\n");
    }
    let path = ino.to_string_lossy().replace('\\', "\\\\");
    out.push_str(&format!("#line 1 \"{path}\"\n"));
    out.push_str(text);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = r#""D:\\A15\\packages\\esp32\\tools\\gcc/bin/xtensa-esp32-elf-g++" "-Wl,--Map=C:\\p\\build/sketch.ino.map" -o "C:\\p\\build/sketch.ino.elf""#;

    #[test]
    fn go_quoted_words_are_unescaped() {
        assert_eq!(
            split_go(r#""C:\\a b\\x.exe" -q "say \"hi\"" plain"#).unwrap(),
            vec![
                (r"C:\a b\x.exe".to_owned(), true),
                ("-q".to_owned(), false),
                (r#"say "hi""#.to_owned(), true),
                ("plain".to_owned(), false),
            ]
        );
        assert_eq!(split_go(r#""unterminated"#), None);
    }

    #[test]
    fn link_log_lines_become_steps() {
        let any = |_: &str| true;
        assert_eq!(
            parse_step(LINK, &any),
            Some(Step::Run(vec![
                r"D:\A15\packages\esp32\tools\gcc/bin/xtensa-esp32-elf-g++".into(),
                r"-Wl,--Map=C:\p\build/sketch.ino.map".into(),
                "-o".into(),
                r"C:\p\build/sketch.ino.elf".into(),
            ]))
        );
        assert_eq!(
            parse_step(
                r#"cmd /c if exist "C:\\p\\build\\libraries\\Insights" "C:\\t\\gen.exe" x"#,
                &any
            ),
            Some(Step::Shell(
                r#"if exist "C:\p\build\libraries\Insights" "C:\t\gen.exe" x"#.into()
            ))
        );
        assert_eq!(parse_step("Creating esp32 image...", &|_| false), None);
    }

    #[test]
    fn capture_keeps_only_commands_after_the_link_stage() {
        let mut capture = Capture::default();
        capture.feed("Compiling sketch...");
        capture.feed(r#"cmd /c type nul > "C:\\p\\build/file_opts""#);
        capture.feed(LINK_STAGE);
        capture.feed(r#"cmd /c echo done"#);
        capture.feed("Successfully created esp32 image.");
        assert_eq!(capture.post, vec![Step::Shell("echo done".into())]);
    }

    #[test]
    fn include_set_ignores_order_and_body() {
        let a = "#include <A.h>\n#include <B.h>\nint x = 1;\n";
        let b = "#include <B.h>\nint x = 2;\n#include <A.h>\n";
        assert_eq!(include_lines(a), include_lines(b));
        assert_ne!(include_lines(a), include_lines("#include <A.h>\n"));
    }

    #[test]
    fn include_dirs_both_forms() {
        let args: Vec<String> = ["g++", "-IC:/core", "-I", "C:/lib/src", "-iprefix", "C:/sdk"]
            .map(String::from)
            .into();
        assert_eq!(
            include_dirs(&args),
            vec![PathBuf::from("C:/core"), PathBuf::from("C:/lib/src")]
        );
    }

    #[test]
    fn sketch_cpp_has_line_directive() {
        let cpp = sketch_cpp(
            Path::new(r"C:\p\sketch\sketch.ino"),
            "#include <Arduino.h>\nx\n",
        );
        assert_eq!(
            cpp,
            "#line 1 \"C:\\\\p\\\\sketch\\\\sketch.ino\"\n#include <Arduino.h>\nx\n"
        );
        assert!(sketch_cpp(Path::new("s.ino"), "x").starts_with("#include <Arduino.h>\n#line 1"));
    }

    #[test]
    fn recipe_key_ignores_only_the_sketch_body() {
        let root = std::env::temp_dir().join(format!("flp-direct-{}", std::process::id()));
        let sketch = sketch_dir(&root);
        let lib = root.join("lib");
        std::fs::create_dir_all(&sketch).unwrap();
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("lib.h"), "int a;").unwrap();
        let ino = sketch.join("sketch.ino");
        std::fs::write(&ino, "#include <A.h>\nvoid loop() {}").unwrap();
        let board = BoardConfig {
            fqbn: "esp32:esp32:esp32".into(),
            model: None,
            port: None,
            options: Default::default(),
        };
        let opts = CompileOptions::default();
        let compile = vec!["g++".to_owned(), format!("-I{}", lib.display())];
        let exe = Path::new("arduino-cli");
        let key = |board: &BoardConfig| recipe_key(exe, &root, board, &opts, &compile);
        let base = key(&board).unwrap();

        std::fs::write(&ino, "#include <A.h>\nvoid loop() { int x = 2; }").unwrap();
        assert_eq!(key(&board).unwrap(), base, "body edit keeps the key");
        assert_eq!(base.mismatch(&base), None);

        let mut optioned = board.clone();
        optioned.options.insert("FlashSize".into(), "8M".into());
        assert_eq!(
            base.mismatch(&key(&optioned).unwrap()),
            Some(CliReason::Board)
        );

        std::fs::write(&ino, "#include <A.h>\n#define FLP_WEBUI\n").unwrap();
        assert_eq!(
            base.mismatch(&key(&board).unwrap()),
            Some(CliReason::Includes),
            "web_ui"
        );

        std::fs::write(&ino, "#include <A.h>\n#include <B.h>\nvoid loop() {}").unwrap();
        assert_eq!(
            base.mismatch(&key(&board).unwrap()),
            Some(CliReason::Includes),
            "new include"
        );

        std::fs::write(&ino, "#include <A.h>\nvoid loop() {}").unwrap();
        std::fs::write(lib.join("lib.h"), "int a, b;").unwrap();
        assert_eq!(
            base.mismatch(&key(&board).unwrap()),
            Some(CliReason::LibraryFiles)
        );

        let more = CompileOptions {
            libraries: vec![lib.clone()],
            ..Default::default()
        };
        let with_lib = recipe_key(exe, &root, &board, &more, &compile).unwrap();
        assert_eq!(base.mismatch(&with_lib), Some(CliReason::Libraries));
        let other_cli = recipe_key(Path::new("arduino-cli-2"), &root, &board, &opts, &compile);
        assert_eq!(base.mismatch(&other_cli.unwrap()), Some(CliReason::Tool));

        std::fs::write(sketch.join("extra.h"), "").unwrap();
        assert_eq!(key(&board), None, "second sketch file");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn response_file_quotes_arguments() {
        let args = vec![r"C:\a b".to_owned(), r#"-D"x""#.to_owned()];
        assert_eq!(response_file(&args), "\"C:\\\\a b\"\n\"-D\\\"x\\\"\"");
    }
}
