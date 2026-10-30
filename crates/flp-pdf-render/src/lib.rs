//! Compiles the Typst markup made by `flp-pdf` into a PDF.
//!
//! Typst is large (~35 MB), so it lives only in the `crablogic-pdf` executable;
//! the other programs start it as a helper. Fonts are not bundled with Typst:
//! the system fonts with Cyrillic are used (Windows always has Segoe UI).

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use typst_embed::{FontSet, RenderEnvironment, RenderError};

/// Placeholder of the font list in the markup, see `flp_pdf::source`.
const FONTS_TOKEN: &str = "font: FONTS";

/// Fonts of this computer that have Cyrillic: (family, regular file, bold file).
const SYSTEM_FONTS: [(&str, &str, &str); 2] = [
    ("Tahoma", "tahoma.ttf", "tahomabd.ttf"),
    ("Segoe UI", "segoeui.ttf", "segoeuib.ttf"),
];

/// Renders the markup to PDF bytes; the error is a short text for the user.
pub fn render(source: &str) -> Result<Vec<u8>, String> {
    let source = source.replace(FONTS_TOKEN, &format!("font: {}", font_list()));
    let project = typst_embed::Project::from_source(source);
    let pdf = typst_embed::render_pdf(&project, environment()).map_err(describe)?;
    Ok(pdf.bytes().to_vec())
}

/// Typst reports errors as a list; the first lines are enough to find the cause.
fn describe(e: RenderError) -> String {
    match e {
        RenderError::Diagnostics(list) => list
            .iter()
            .take(3)
            .map(|d| d.message().to_owned())
            .collect::<Vec<_>>()
            .join("; "),
        other => other.to_string(),
    }
}

/// System fonts found on this computer, as (family, file contents).
fn system_fonts() -> &'static [(&'static str, Vec<u8>)] {
    static FONTS: OnceLock<Vec<(&'static str, Vec<u8>)>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let dir = std::env::var_os("WINDIR")
            .map(|w| PathBuf::from(w).join("Fonts"))
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\Fonts"));
        let mut found = Vec::new();
        for (family, regular, bold) in SYSTEM_FONTS {
            // Typst fails on an unknown family, so only fonts that were read are listed.
            if let Ok(data) = fs::read(dir.join(regular)) {
                found.push((family, data));
                found.extend(fs::read(dir.join(bold)).ok().map(|d| (family, d)));
            }
        }
        found
    })
}

/// Typst array of the font families that were found.
fn font_list() -> String {
    let mut names: Vec<&str> = Vec::new();
    for (family, _) in system_fonts() {
        if !names.contains(family) {
            names.push(family);
        }
    }
    let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    format!("({})", quoted.join(", "))
}

fn environment() -> &'static RenderEnvironment {
    static ENV: OnceLock<RenderEnvironment> = OnceLock::new();
    ENV.get_or_init(|| {
        let files: Vec<Vec<u8>> = system_fonts().iter().map(|(_, d)| d.clone()).collect();
        RenderEnvironment::builder()
            .font_set(FontSet::from_font_files(files))
            .build()
            .expect("render environment without packages is always valid")
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use flp_core::{Library, Project};

    #[test]
    fn renders_pump_example() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (lib, _) = Library::load(root.join("library")).unwrap();
        let project = Project::load(root.join("examples/pump")).unwrap();
        let pdf = super::render(&flp_pdf::source(&project, &lib)).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        if let Some(out) = std::env::var_os("FLP_PDF_OUT") {
            std::fs::write(out, &pdf).unwrap();
        }
    }
}
