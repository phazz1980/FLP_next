//! `crablogic-pdf`: reads Typst markup from stdin, writes the PDF to stdout.
//! Errors go to stderr with exit code 1.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut source = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut source) {
        eprintln!("не удалось прочитать разметку: {e}");
        return ExitCode::FAILURE;
    }
    match flp_pdf_render::render(&source) {
        Ok(pdf) => match std::io::stdout().write_all(&pdf) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("не удалось записать PDF: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
