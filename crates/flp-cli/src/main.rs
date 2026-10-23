//! `flp` — command line of FLP_next.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use flp_codegen::{ArduinoCpp, GenError};
use flp_core::{check, Diagnostic, Library, Project, Severity};

/// Exit code when the schema has errors.
const EXIT_SCHEMA_ERRORS: u8 = 1;
/// Exit code when the project or library cannot be loaded.
const EXIT_LOAD_FAILED: u8 = 2;
/// Exit code when code generation fails for another reason (templates, files).
const EXIT_GEN_FAILED: u8 = 3;

#[derive(Parser)]
#[command(
    name = "flp",
    version,
    about = "FLP_next: визуальное программирование микроконтроллеров"
)]
struct Cli {
    /// Папка библиотеки блоков (по умолчанию ищется `library/` рядом с проектом
    /// или программой; можно задать переменной FLP_LIBRARY)
    #[arg(long, global = true, env = "FLP_LIBRARY")]
    library: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Проверить схему проекта
    Check {
        /// Папка проекта
        project: PathBuf,
    },
    /// Сгенерировать скетч Arduino в <проект>/.flp/sketch/sketch.ino
    Gen {
        /// Папка проекта
        project: PathBuf,
        /// Вывести скетч в консоль вместо записи в файл
        #[arg(long)]
        stdout: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { project } => run_check(&project, cli.library.as_deref()),
        Command::Gen { project, stdout } => run_gen(&project, cli.library.as_deref(), stdout),
    }
}

fn run_check(project_dir: &Path, library: Option<&Path>) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };

    let report = check(&project.logic, &lib, Some(&project.layout));
    print_diagnostics(&report.diagnostics);

    let errors = report.errors().count();
    if errors > 0 {
        eprintln!("Проверка {}: ошибок — {errors}", project_dir.display());
        return ExitCode::from(EXIT_SCHEMA_ERRORS);
    }
    let order: Vec<&str> = report
        .order
        .iter()
        .map(|&i| project.logic.blocks[i].id.as_str())
        .collect();
    println!("Проверка {}: ошибок нет", project_dir.display());
    println!("Порядок выполнения: {}", order.join(" → "));
    if !report.feedback.is_empty() {
        let links: Vec<String> = report
            .feedback
            .iter()
            .map(|&i| {
                let l = &project.logic.links[i];
                format!("{} → {}", l.from, l.to)
            })
            .collect();
        println!(
            "Обратные связи (значение с прошлого цикла): {}",
            links.join(", ")
        );
    }
    ExitCode::SUCCESS
}

fn run_gen(project_dir: &Path, library: Option<&Path>, stdout: bool) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let code = match flp_codegen::generate(&project, &lib, &ArduinoCpp) {
        Ok(code) => code,
        Err(GenError::Schema(diagnostics)) => {
            print_diagnostics(&diagnostics);
            eprintln!("Генерация отменена: в схеме есть ошибки (подробнее — flp check)");
            return ExitCode::from(EXIT_SCHEMA_ERRORS);
        }
        Err(e) => {
            eprintln!("ошибка: {e}");
            return ExitCode::from(EXIT_GEN_FAILED);
        }
    };
    if stdout {
        print!("{code}");
        return ExitCode::SUCCESS;
    }
    match flp_codegen::write_sketch(&project.root, &code) {
        Ok((path, true)) => println!("Скетч записан: {}", path.display()),
        Ok((path, false)) => println!("Скетч не изменился: {}", path.display()),
        Err(e) => {
            eprintln!("ошибка: {e}");
            return ExitCode::from(EXIT_GEN_FAILED);
        }
    }
    ExitCode::SUCCESS
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        match d.severity {
            Severity::Error => eprintln!("{d}"),
            Severity::Warning => println!("{d}"),
        }
    }
}

/// Loads the project and the block library, printing errors.
fn load(project_dir: &Path, library: Option<&Path>) -> Result<(Project, Library), ExitCode> {
    let project = Project::load(project_dir).map_err(|e| {
        eprintln!("ошибка: {e}");
        ExitCode::from(EXIT_LOAD_FAILED)
    })?;
    let lib = load_library(project_dir, library).ok_or(ExitCode::from(EXIT_LOAD_FAILED))?;
    Ok((project, lib))
}

/// Loads the block library; library problems are printed as warnings.
fn load_library(project_dir: &Path, explicit: Option<&Path>) -> Option<Library> {
    let Some(dir) = explicit
        .map(Path::to_path_buf)
        .or_else(|| find_library(project_dir))
    else {
        eprintln!(
            "ошибка: не найдена папка библиотеки блоков `library/`; укажите её через --library"
        );
        return None;
    };
    match Library::load(&dir) {
        Ok((lib, errors)) => {
            for e in errors {
                println!("предупреждение: блок пропущен: {e}");
            }
            Some(lib)
        }
        Err(e) => {
            eprintln!("ошибка: {e}");
            None
        }
    }
}

/// Looks for `library/` in the project folder and its parents, then next to the executable.
fn find_library(project_dir: &Path) -> Option<PathBuf> {
    let project_dir = project_dir.canonicalize().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    [project_dir, exe_dir]
        .into_iter()
        .flatten()
        .flat_map(|start| {
            start
                .ancestors()
                .map(|a| a.join("library"))
                .collect::<Vec<_>>()
        })
        .find(|dir| dir.is_dir())
}
