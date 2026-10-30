//! The whole chain: `flp_pdf::render` starts the `crablogic-pdf` executable.

use std::path::PathBuf;

use flp_core::{Library, Project};

#[test]
fn pdf_comes_back_from_the_helper() {
    std::env::set_var("CRABLOGIC_PDF", env!("CARGO_BIN_EXE_crablogic-pdf"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (lib, _) = Library::load(root.join("library")).unwrap();
    let project = Project::load(root.join("examples/pump")).unwrap();
    let pdf = flp_pdf::render(&project, &lib).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}
