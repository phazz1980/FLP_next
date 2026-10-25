//! `flp-editor` — graphical FBD editor of FLP_next.
//!
//! Layout follows FLProg: menu and toolbar on top, project tree and tags on the left,
//! block library and the AI tab on the right, numbered sheets in the centre,
//! messages at the bottom.

mod ai;
mod app;
mod assoc;
mod blockedit;
mod boardcfg;
mod canvas;
mod debugview;
mod dialogs;
mod doc;
mod job;
mod panels;
mod theme;
mod themeedit;

use std::path::PathBuf;

fn main() -> eframe::Result {
    // A project file given on the command line, e.g. by double click in Explorer.
    let project = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("FLP_next")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FLP_next",
        options,
        Box::new(move |cc| Ok(Box::new(app::EditorApp::new(cc, project)))),
    )
}
