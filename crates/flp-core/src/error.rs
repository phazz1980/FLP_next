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

    #[error("в папке {} нет проекта (файла .flpn)", .path.display())]
    NoProject { path: PathBuf },

    #[error("{} — не архив проекта: {}", .path.display(), .source)]
    Archive {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("в архиве {} нет {}", .path.display(), .entry)]
    MissingEntry { path: PathBuf, entry: &'static str },

    #[error("неверное описание блока {}: {}", .path.display(), .message)]
    InvalidBlockDef { path: PathBuf, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
