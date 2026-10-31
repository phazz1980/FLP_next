//! `crablogic-editor` — graphical FBD editor of CrabLogic.
//!
//! Layout: menu and toolbar on top, project tree and tags on the left,
//! block library and the AI tab on the right, numbered sheets in the centre,
//! messages and the port monitor at the bottom.

// Release builds are GUI-only: no console window behind the editor.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod ai;
mod aiconf;
mod app;
mod assoc;
mod blockedit;
mod blockio;
mod boardcfg;
mod boardedit;
mod boardio;
mod canvas;
mod codeview;
mod console;
mod cpphl;
mod debugview;
mod dialogs;
mod doc;
mod help;
mod i18n;
mod job;
mod markdown;
mod mcp_host;
mod netpanel;
mod panels;
mod pictures;
mod report;
mod selfupdate;
mod splash;
mod textmenu;
mod theme;
mod themeedit;
mod update;
mod userlib;

use std::path::PathBuf;

fn main() -> eframe::Result {
    // The copy of this exe that installs a downloaded update after the editor exits.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == selfupdate::APPLY_FLAG) {
        selfupdate::run_helper(&args[1..]);
        return Ok(());
    }
    // A project file given on the command line, e.g. by double click in Explorer.
    let project = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CrabLogic")
            .with_decorations(false)
            .with_icon(eframe::egui::IconData {
                rgba: include_bytes!("../assets/icon64.rgba").to_vec(),
                width: 64,
                height: 64,
            })
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CrabLogic",
        options,
        Box::new(move |cc| Ok(Box::new(app::EditorApp::new(cc, project)))),
    )
}
