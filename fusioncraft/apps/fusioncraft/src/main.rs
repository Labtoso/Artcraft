//! FusionCraft desktop app. Usage: `fusioncraft [design.fcraft]`.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use fusioncraft_ui_egui::{native_options, FusionCraftApp};
use std::path::PathBuf;

fn main() -> eframe::Result {
    let open = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "FusionCraft",
        native_options(),
        Box::new(move |cc| Ok(Box::new(FusionCraftApp::new(&cc.egui_ctx, open)))),
    )
}
