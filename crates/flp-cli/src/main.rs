//! `flp` — command line of FLP_next.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use flp_build::{ArduinoCli, BuildMode, CompileOptions, Compiled};
use flp_codegen::{ArduinoCpp, Backend, GenError, Program};
use flp_core::{check_project, Diagnostic, Library, LocalValues, Project, Severity};

/// Exit code when the schema has errors.
const EXIT_SCHEMA_ERRORS: u8 = 1;
/// Exit code when the project or library cannot be loaded.
const EXIT_LOAD_FAILED: u8 = 2;
/// Exit code when code generation fails for another reason (templates, files).
const EXIT_GEN_FAILED: u8 = 3;
/// Exit code when arduino-cli fails to build or upload.
const EXIT_BUILD_FAILED: u8 = 4;

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
        /// Файл проекта .flpn (или папка с ним)
        project: PathBuf,
    },
    /// Сгенерировать скетч Arduino в .flp/<проект>/sketch/sketch.ino рядом с проектом
    Gen {
        /// Файл проекта .flpn (или папка с ним)
        project: PathBuf,
        /// Вывести скетч в консоль вместо записи в файл
        #[arg(long)]
        stdout: bool,
        /// Встроить модуль онлайн-отладки
        #[arg(long)]
        debug: bool,
    },
    /// Сгенерировать скетч и собрать прошивку через arduino-cli
    Build {
        /// Файл проекта .flpn (или папка с ним)
        project: PathBuf,
        /// Отладочная сборка: модуль онлайн-отладки, без сильной оптимизации
        #[arg(long)]
        debug: bool,
        /// Собрать заново, даже если прошивка уже собрана из того же скетча
        #[arg(long)]
        force: bool,
    },
    /// Показывать живые значения с платы (прошивка собрана с --debug)
    Monitor {
        /// Файл проекта .flpn (или папка с ним)
        project: PathBuf,
        /// Порт платы, например COM3 (по умолчанию из настроек проекта)
        #[arg(long, short)]
        port: Option<String>,
    },
    /// Собрать прошивку и залить её на плату
    Upload {
        /// Файл проекта .flpn (или папка с ним)
        project: PathBuf,
        /// Порт платы, например COM3 (по умолчанию из настроек проекта или единственный найденный)
        #[arg(long, short)]
        port: Option<String>,
        /// Отладочная сборка: модуль онлайн-отладки, без сильной оптимизации
        #[arg(long)]
        debug: bool,
        /// Собрать заново, даже если прошивка уже собрана из того же скетча
        #[arg(long)]
        force: bool,
    },
    /// Перевести проект из старого формата (папка с project.toml, logic.toml,
    /// layout.toml) в один файл <папка>/<папка>.flpn; старые файлы не удаляются
    Convert {
        /// Папка проекта в старом формате
        project: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { project } => run_check(&project, cli.library.as_deref()),
        Command::Gen {
            project,
            stdout,
            debug,
        } => run_gen(&project, cli.library.as_deref(), stdout, debug),
        Command::Build {
            project,
            debug,
            force,
        } => run_build(&project, cli.library.as_deref(), debug, force, None),
        Command::Upload {
            project,
            port,
            debug,
            force,
        } => run_build(&project, cli.library.as_deref(), debug, force, Some(port)),
        Command::Monitor { project, port } => run_monitor(&project, cli.library.as_deref(), port),
        Command::Convert { project } => run_convert(&project, cli.library.as_deref()),
    }
}

fn run_convert(dir: &Path, library: Option<&Path>) -> ExitCode {
    let result = Project::load(dir).and_then(|p| {
        if p.path.exists() {
            println!("Проект уже в одном файле: {}", p.path.display());
            return Ok(());
        }
        // Without the library the archive is written without the PDF of sheets.
        let warning = match load_library(dir, library) {
            Some(lib) => flp_pdf::save_project(&p, &lib)?,
            None => p.save().map(|()| None)?,
        };
        println!("Записан {}", p.path.display());
        if let Some(w) = warning {
            println!("предупреждение: {w}");
        }
        Ok(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ошибка: {e}");
            ExitCode::from(EXIT_LOAD_FAILED)
        }
    }
}

fn run_check(project_dir: &Path, library: Option<&Path>) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };

    let report = check_project(&project, &lib);
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

fn run_gen(project_dir: &Path, library: Option<&Path>, stdout: bool, debug: bool) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let code = match generate(&project, &lib, debug) {
        Ok((code, _)) => code,
        Err(exit) => return exit,
    };
    if stdout {
        print!("{code}");
        return ExitCode::SUCCESS;
    }
    match write_sketch(&project, &code) {
        Ok(()) => ExitCode::SUCCESS,
        Err(exit) => exit,
    }
}

/// Generates, writes and compiles the sketch; with `upload = Some(port)` also uploads it.
fn run_build(
    project_dir: &Path,
    library: Option<&Path>,
    debug: bool,
    force: bool,
    upload: Option<Option<String>>,
) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let libraries = match generate(&project, &lib, debug) {
        Ok((code, libraries)) => match write_sketch(&project, &code) {
            Ok(()) => libraries,
            Err(exit) => return exit,
        },
        Err(exit) => return exit,
    };
    let build_failed = |e: flp_build::BuildError| {
        eprintln!("ошибка: {e}");
        ExitCode::from(EXIT_BUILD_FAILED)
    };
    let cli = match ArduinoCli::locate() {
        Ok(cli) => cli,
        Err(e) => return build_failed(e),
    };
    let board = &project.config.board;
    let opts = CompileOptions {
        mode: if debug {
            BuildMode::Debug
        } else {
            BuildMode::Release
        },
        force,
        ..Default::default()
    };
    let mut print = |line: &str| println!("{line}");

    let required: Vec<&str> = libraries.iter().map(String::as_str).collect();
    match cli.ensure_libraries(&required, &mut print) {
        Ok(installed) if !installed.is_empty() => {
            println!("Установлены библиотеки: {}", installed.join(", "));
        }
        Ok(_) => {}
        Err(e) => return build_failed(e),
    }
    println!("Сборка для {}...", board.fqbn);
    match cli.compile(&project.work_dir(), board, &opts, &mut print) {
        Ok(Compiled::UpToDate) => println!("Прошивка уже собрана, скетч не менялся"),
        Ok(Compiled::Built) => {}
        Err(e) => return build_failed(e),
    }
    let Some(port) = upload else {
        println!(
            "Сборка готова: {}",
            flp_build::build_dir(&project.work_dir()).display()
        );
        return ExitCode::SUCCESS;
    };
    let port = match cli.choose_port(port.as_deref(), board) {
        Ok(port) => port,
        Err(e) => return build_failed(e),
    };
    println!("Заливка на {port}...");
    if let Err(e) = cli.upload(&project.work_dir(), board, &port, &mut print) {
        return build_failed(e);
    }
    println!("Прошивка залита на {port}");
    ExitCode::SUCCESS
}

/// Generates the sketch, printing schema errors. Also returns the Arduino libraries it needs.
fn generate(
    project: &Project,
    lib: &Library,
    debug: bool,
) -> Result<(String, Vec<String>), ExitCode> {
    let backend = ArduinoCpp { debug };
    // Wi-Fi passwords and other `local` values are kept by the program, not in the file.
    let store = LocalValues::open_default().unwrap_or_else(|e| {
        eprintln!("предупреждение: {e}");
        LocalValues::default()
    });
    let project = &project.with_local(lib, &store);
    let result = Program::build(project, lib).and_then(|program| {
        let code = backend.generate(&program)?;
        let libraries = program.libraries().into_iter().map(String::from).collect();
        Ok((code, libraries))
    });
    match result {
        Ok(generated) => Ok(generated),
        Err(GenError::Schema(diagnostics)) => {
            print_diagnostics(&diagnostics);
            eprintln!("Генерация отменена: в схеме есть ошибки (подробнее — flp check)");
            Err(ExitCode::from(EXIT_SCHEMA_ERRORS))
        }
        Err(e) => {
            eprintln!("ошибка: {e}");
            Err(ExitCode::from(EXIT_GEN_FAILED))
        }
    }
}

fn write_sketch(project: &Project, code: &str) -> Result<(), ExitCode> {
    match flp_codegen::write_sketch(&project.work_dir(), code) {
        Ok((path, true)) => println!("Скетч записан: {}", path.display()),
        Ok((path, false)) => println!("Скетч не изменился: {}", path.display()),
        Err(e) => {
            eprintln!("ошибка: {e}");
            return Err(ExitCode::from(EXIT_GEN_FAILED));
        }
    }
    Ok(())
}

/// Prints live values every half second until Ctrl+C.
fn run_monitor(project_dir: &Path, library: Option<&Path>, port: Option<String>) -> ExitCode {
    let (project, lib) = match load(project_dir, library) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let program = match Program::build(&project, &lib) {
        Ok(p) => p,
        Err(GenError::Schema(d)) => {
            print_diagnostics(&d);
            return ExitCode::from(EXIT_SCHEMA_ERRORS);
        }
        Err(e) => {
            eprintln!("ошибка: {e}");
            return ExitCode::from(EXIT_GEN_FAILED);
        }
    };
    let Some(port) = port.or_else(|| project.config.board.port.clone()) else {
        eprintln!(
            "ошибка: укажите порт через --port или в файле проекта ([board] port = \"COM3\")"
        );
        return ExitCode::from(EXIT_BUILD_FAILED);
    };
    let table = flp_codegen::debug::debug_table(&program);
    let names: Vec<String> = table
        .entries
        .iter()
        .map(|e| match &e.source {
            flp_debug::Source::Output(p) => p.to_string(),
            flp_debug::Source::Var(v) => format!("${v}"),
        })
        .collect();
    let session = flp_debug::Session::start(table, flp_debug::transport::serial(&port), || {});
    println!("Монитор {port}, Ctrl+C — выход");
    let mut printed_console = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let live = session.snapshot();
        for line in live.console_after(printed_console) {
            println!("> {line}");
        }
        printed_console = live.console_total;
        let values: Vec<String> = names
            .iter()
            .zip(&live.values)
            .zip(&live.forced)
            .map(|((name, v), f)| {
                let v = v.map_or("—".to_owned(), |v| v.to_string());
                let mark = if f.is_some() { " (F)" } else { "" };
                format!("{name}={v}{mark}")
            })
            .collect();
        println!("[{}] {}", live.status, values.join("  "));
    }
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
    // User blocks packed into the project file but missing here are restored first.
    if flp_core::project::is_project_file(project_dir) {
        if let Ok((lib, _)) = Library::load_with_user(&dir) {
            let user = flp_core::user_library_dir(&dir);
            let written = flp_core::packed::unpack_missing_blocks(project_dir, &user, &lib);
            if !written.is_empty() {
                println!("из проекта добавлены блоки: {}", written.join(", "));
            }
        }
    }
    match Library::load_with_user(&dir) {
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
    let project_dir = std::path::absolute(project_dir).ok();
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
