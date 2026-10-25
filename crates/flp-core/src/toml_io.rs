use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Error, Result};

pub(crate) fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    parse_toml(&text, path)
}

/// Parses TOML text; `path` is only for error messages.
pub(crate) fn parse_toml<T: DeserializeOwned>(text: &str, path: &Path) -> Result<T> {
    // Files may be edited in Notepad, which can add a BOM.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    toml::from_str(text).map_err(|source| Error::Parse {
        path: path.to_owned(),
        source: Box::new(source),
    })
}

/// Serializes to TOML text; `path` is only for error messages.
pub(crate) fn to_toml<T: Serialize>(value: &T, path: &Path) -> Result<String> {
    toml::to_string_pretty(value).map_err(|source| Error::Serialize {
        path: path.to_owned(),
        source,
    })
}
