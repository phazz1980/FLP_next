use std::path::PathBuf;

use flp_core::{Diagnostic, PortRef};

/// Code generation errors. Messages are user-facing, hence in Russian.
#[derive(Debug, thiserror::Error)]
pub enum GenError {
    /// The schema did not pass the check; diagnostics include warnings too.
    #[error("в схеме есть ошибки, генерация невозможна")]
    Schema(Vec<Diagnostic>),

    #[error("у блока «{kind}» нет шаблона {file} для выбранной платы")]
    MissingTemplate { kind: String, file: &'static str },

    #[error("ошибка в шаблоне {}: {message}", .path.display())]
    BadTemplate { path: PathBuf, message: String },

    #[error("блок «{block}»: ошибка шаблона: {source}")]
    Render {
        block: String,
        #[source]
        source: minijinja::Error,
    },

    #[error("выходы {a} и {b} дают одно имя переменной «{name}»; переименуйте блок")]
    NameClash {
        name: String,
        a: PortRef,
        b: PortRef,
    },

    /// Blocks of different tasks (main and background) share something that is not thread-safe.
    #[error("фоновая задача: {0}")]
    Task(String),

    #[error("{}: {}", .path.display(), .source)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
