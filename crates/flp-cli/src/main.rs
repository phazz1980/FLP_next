//! `flp` — command line of FLP_next.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use flp_core::{check, Library, Project, Severity};

/// Exit code when the schema has errors.
const EXIT_SCHEMA_ERRORS: u8 = 1;
/// Exit code when the project or library cannot be loaded.
const EXIT_LOAD_FAILED: u8 = 2;

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { project } => run_check(&project, cli.library.as_deref()),
    }
}

fn run_check(project_dir: &Path, library: Option<&Path>) -> ExitCode {
    let project = match Project::load(project_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ошибка: {e}");
            return ExitCode::from(EXIT_LOAD_FAILED);
        }
    };
    let Some(lib) = load_library(project_dir, library) else {
        return ExitCode::from(EXIT_LOAD_FAILED);
    };

    let report = check(&project.logic, &lib, Some(&project.layout));
    for d in &report.diagnostics {
        match d.severity {
            Severity::Error => eprintln!("{d}"),
            Severity::Warning => println!("{d}"),
        }
    }

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
