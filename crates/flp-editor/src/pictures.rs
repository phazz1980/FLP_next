//! Pictures of a description (`docs.md`) kept in the `images/` folder next to it: added
//! from files while editing, copied into the folder on save. Used by the block editor
//! and the board editor.

use std::fs;
use std::path::{Path, PathBuf};

/// Name of the folder with the pictures.
pub const IMAGES_DIR: &str = "images";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pictures {
    /// Pictures not in the folder yet: name in `images/` → file to copy.
    pub added: Vec<(String, PathBuf)>,
    /// The saved folder (the block or board folder), where the saved pictures are.
    pub home: Option<PathBuf>,
}

impl Pictures {
    /// Pictures of the saved folder `dir`.
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            added: Vec::new(),
            home: Some(dir.to_owned()),
        }
    }

    /// The same pictures for a copy: every file of `dir/images` is copied on save.
    pub fn copied_from(dir: &Path) -> Self {
        let mut pictures = Self::default();
        for e in fs::read_dir(dir.join(IMAGES_DIR))
            .into_iter()
            .flatten()
            .flatten()
        {
            if e.path().is_file() {
                let name = e.file_name().to_string_lossy().into_owned();
                pictures.added.push((name, e.path()));
            }
        }
        pictures
    }

    /// File of the picture a docs link `images/<name>` points to.
    pub fn path(&self, target: &str) -> Option<PathBuf> {
        let name = target.strip_prefix("images/")?;
        if name.contains('/') || !crate::markdown::is_local_image(target) {
            return None;
        }
        if let Some((_, src)) = self.added.iter().find(|(n, _)| n == name) {
            return Some(src.clone());
        }
        let home = self.home.as_ref()?;
        Some(home.join(IMAGES_DIR).join(name))
    }

    /// Takes a picture file into the docs; returns its target `images/<name>`.
    pub fn add(&mut self, src: PathBuf) -> String {
        // A plain ASCII name: the target stays easy to type and safe in the docs and the archive.
        let mut stem = String::new();
        for c in src
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
        {
            if c.is_ascii_alphanumeric() || c == '-' {
                stem.push(c);
            } else if !stem.ends_with('_') {
                stem.push('_');
            }
        }
        let stem = stem.trim_matches('_');
        let stem = if stem.is_empty() { "image" } else { stem };
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| "png".into());
        let mut name = format!("{stem}.{ext}");
        let mut n = 1;
        while self
            .path(&format!("images/{name}"))
            .is_some_and(|p| p.is_file() || self.added.iter().any(|(i, _)| *i == name))
        {
            n += 1;
            name = format!("{stem}_{n}.{ext}");
        }
        self.added.push((name.clone(), src));
        format!("images/{name}")
    }

    /// Makes `dir/images` hold exactly the pictures `docs` uses, and `dir` the saved folder.
    pub fn apply(&mut self, dir: &Path, docs: &str) -> Result<(), String> {
        let root = dir.join(IMAGES_DIR);
        let used: Vec<String> = crate::markdown::image_targets(docs)
            .iter()
            .filter_map(|t| t.strip_prefix("images/").map(str::to_owned))
            .collect();
        for (name, src) in std::mem::take(&mut self.added) {
            if !used.contains(&name) || src == root.join(&name) {
                continue;
            }
            let err = |e: std::io::Error| format!("{}: {e}", src.display());
            fs::create_dir_all(&root).map_err(err)?;
            fs::copy(&src, root.join(&name)).map_err(err)?;
        }
        for e in fs::read_dir(&root).into_iter().flatten().flatten() {
            if !used.contains(&e.file_name().to_string_lossy().into_owned()) {
                let _ = fs::remove_file(e.path());
            }
        }
        let _ = fs::remove_dir(&root);
        self.home = Some(dir.to_owned());
        Ok(())
    }
}
