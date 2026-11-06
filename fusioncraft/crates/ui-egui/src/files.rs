//! File menu actions with native dialogs.

use crate::FusionCraftApp;
use serde_json::json;

pub fn new(app: &mut FusionCraftApp) {
    if app.engine.dirty {
        app.confirm_new = true;
        return;
    }
    really_new(app);
}

pub fn really_new(app: &mut FusionCraftApp) {
    app.run("file.new", &json!({}));
    app.sketch = None;
    app.dialog = None;
    app.preview = None;
    app.cam = fusioncraft_render::Camera::default();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open(app: &mut FusionCraftApp) {
    let Some(path) = rfd::FileDialog::new().add_filter("FusionCraft design", &[fusioncraft_io::EXTENSION]).pick_file() else { return };
    if app.run("file.open", &json!({ "path": path.display().to_string() })).is_some() {
        app.sketch = None;
        app.dialog = None;
        app.preview = None;
        app.fit_all(false);
        app.set_status(format!("Opened {}", path.display()), false);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(app: &mut FusionCraftApp, save_as: bool) {
    let path = match (&app.engine.path, save_as) {
        (Some(p), false) => p.clone(),
        _ => {
            let Some(p) = rfd::FileDialog::new()
                .add_filter("FusionCraft design", &[fusioncraft_io::EXTENSION])
                .set_file_name(format!("{}.{}", app.engine.doc.name, fusioncraft_io::EXTENSION))
                .save_file()
            else {
                return;
            };
            p
        }
    };
    if let Some(v) = app.run("file.save", &json!({ "path": path.display().to_string() })) {
        app.set_status(format!("Saved {}", v["path"].as_str().unwrap_or_default()), false);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export(app: &mut FusionCraftApp, format: &str) {
    let ext = if format == "obj" { "obj" } else { "stl" };
    let Some(path) = rfd::FileDialog::new()
        .add_filter(if ext == "obj" { "Wavefront OBJ" } else { "STL mesh" }, &[ext])
        .set_file_name(format!("{}.{ext}", app.engine.doc.name))
        .save_file()
    else {
        return;
    };
    if let Some(v) = app.run("file.export", &json!({ "path": path.display().to_string(), "format": format })) {
        app.set_status(format!("Exported {} triangles to {}", v["triangles"], path.display()), false);
    }
}

#[cfg(target_arch = "wasm32")]
pub fn open(app: &mut FusionCraftApp) {
    app.set_status("Opening files is not available in the browser yet", true);
}
#[cfg(target_arch = "wasm32")]
pub fn save(app: &mut FusionCraftApp, _save_as: bool) {
    app.set_status("Saving is not available in the browser yet", true);
}
#[cfg(target_arch = "wasm32")]
pub fn export(app: &mut FusionCraftApp, _format: &str) {
    app.set_status("Export is not available in the browser yet", true);
}
