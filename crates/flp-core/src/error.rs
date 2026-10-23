use std::path::PathBuf;

/// Errors of the core. Messages are user-facing, hence in Russian.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("не удалось прочитать {}: {}", .path.display(), .source)]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("не удалось записать {}: {}", .path.display(), .source)]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("ошибка в {}: {}", .path.display(), .source)]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("не удалось сохранить {}: {}", .path.display(), .source)]
    Serialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    #[error("неверное описание блока {}: {}", .path.display(), .message)]
    InvalidBlockDef { path: PathBuf, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
