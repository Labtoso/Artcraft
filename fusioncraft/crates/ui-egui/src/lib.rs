//! FusionCraft user interface (layer L5). The UI is thin: it reads engine state and changes the
//! design only through `Engine::execute`, the same commands the command line uses.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod dialogs;
mod files;
pub mod icons;
mod panels;
mod sketching;
pub mod theme;
mod viewport;

use dialogs::Dialog;
use fusioncraft_doc::Id;
use fusioncraft_engine::Engine;
use fusioncraft_model::Model;
use fusioncraft_render::{Camera, DisplayMode};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

pub use viewport::ViewportCache;

/// A short message at the bottom of the window.
pub struct Status {
    pub text: String,
    pub error: bool,
    pub at: f64,
}

/// Camera animation between two states.
pub struct CamAnim {
    pub from: Camera,
    pub to: Camera,
    pub start: f64,
    pub duration: f64,
}

pub struct ViewState {
    pub mode: DisplayMode,
    pub grid: bool,
    pub show_origin: bool,
    pub hidden_bodies: HashSet<usize>,
}

pub struct FusionCraftApp {
    pub engine: Engine,
    pub cam: Camera,
    pub cam_anim: Option<CamAnim>,
    pub view: ViewState,
    pub selected_bodies: Vec<usize>,
    pub selected_feature: Option<Id>,
    pub hover_body: Option<usize>,
    pub sketch: Option<sketching::SketchSession>,
    /// "Create Sketch" is waiting for a plane or face click.
    pub picking_plane: bool,
    pub dialog: Option<Dialog>,
    pub preview: Option<Model>,
    pub params_open: bool,
    pub about_open: bool,
    pub shortcuts_open: bool,
    pub rename: Option<(Id, String)>,
    pub status: Option<Status>,
    pub cache: ViewportCache,
    /// Bumped whenever the model shown in the viewport changes.
    pub model_version: u64,
    pub confirm_new: bool,
    pub quit_requested: bool,
}

impl FusionCraftApp {
    pub fn new(ctx: &egui::Context, open: Option<PathBuf>) -> Self {
        theme::apply(ctx);
        let mut app = Self {
            engine: Engine::new(),
            cam: Camera::default(),
            cam_anim: None,
            view: ViewState { mode: DisplayMode::ShadedWithEdges, grid: true, show_origin: true, hidden_bodies: HashSet::new() },
            selected_bodies: Vec::new(),
            selected_feature: None,
            hover_body: None,
            sketch: None,
            picking_plane: false,
            dialog: None,
            preview: None,
            params_open: false,
            about_open: false,
            shortcuts_open: false,
            rename: None,
            status: None,
            cache: ViewportCache::default(),
            model_version: 1,
            confirm_new: false,
            quit_requested: false,
        };
        if let Some(p) = open {
            app.run("file.open", &serde_json::json!({ "path": p.display().to_string() }));
            app.fit_all(false);
        }
        app
    }

    /// Run a command and report failures in the status bar. Returns the result on success.
    pub fn run(&mut self, id: &str, params: &Value) -> Option<Value> {
        let before = self.engine.doc.clone();
        let r = self.engine.execute(id, params);
        if self.engine.doc != before || matches!(id, "file.open" | "file.new" | "edit.undo" | "edit.redo") {
            self.model_version += 1;
            self.after_change();
        }
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.set_status(e.to_string(), true);
                None
            }
        }
    }

    fn after_change(&mut self) {
        // Drop selections that no longer exist.
        let bodies: HashSet<usize> = self.engine.model.bodies.iter().map(|b| b.index).collect();
        self.selected_bodies.retain(|b| bodies.contains(b));
        if self.selected_feature.is_some_and(|f| self.engine.doc.feature(f).is_none()) {
            self.selected_feature = None;
        }
        if let Some(s) = &self.sketch
            && self.engine.doc.sketch(s.id).is_none() {
                self.sketch = None;
            }
    }

    pub fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(Status { text: text.into(), error, at: now() });
    }

    /// The model to draw: the dialog preview if there is one.
    pub fn shown_model(&self) -> &Model {
        self.preview.as_ref().unwrap_or(&self.engine.model)
    }

    pub fn animate_to(&mut self, to: Camera) {
        self.cam_anim = Some(CamAnim { from: self.cam, to, start: now(), duration: 0.35 });
    }

    pub fn fit_all(&mut self, animate: bool) {
        let mut to = self.cam;
        let bb = self.shown_model().aabb();
        to.fit(&bb);
        if bb.is_empty() {
            to.distance = 200.0;
        }
        if animate { self.animate_to(to) } else { self.cam = to }
    }

    pub fn set_view(&mut self, v: fusioncraft_render::StdView) {
        let mut to = self.cam;
        to.set_view(v);
        if v == fusioncraft_render::StdView::Home {
            to.fit(&self.shown_model().aabb());
        }
        self.animate_to(to);
    }

    pub fn title(&self) -> String {
        format!("{} - FusionCraft", self.engine.title())
    }
}

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl eframe::App for FusionCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.animate(&ctx);
        self.shortcuts(&ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        panels::menu_bar(self, ui);
        panels::toolbar(self, ui);
        panels::status_bar(self, ui);
        panels::timeline(self, ui);
        panels::browser(self, ui);
        if self.dialog.is_some() {
            dialogs::show(self, ui);
        }
        if self.sketch.is_some() {
            sketching::palette(self, ui);
        }
        viewport::show(self, ui);
        dialogs::windows(self, &ctx);
        if self.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl FusionCraftApp {
    fn animate(&mut self, ctx: &egui::Context) {
        let Some(a) = &self.cam_anim else { return };
        let t = ((now() - a.start) / a.duration).clamp(0.0, 1.0);
        let e = t * t * (3.0 - 2.0 * t);
        let (from, to) = (a.from, a.to);
        let mut dyaw = (to.yaw - from.yaw) % std::f64::consts::TAU;
        if dyaw > std::f64::consts::PI {
            dyaw -= std::f64::consts::TAU;
        } else if dyaw < -std::f64::consts::PI {
            dyaw += std::f64::consts::TAU;
        }
        self.cam.yaw = from.yaw + dyaw * e;
        self.cam.pitch = from.pitch + (to.pitch - from.pitch) * e;
        self.cam.distance = from.distance + (to.distance - from.distance) * e;
        self.cam.target = from.target.lerp(to.target, e);
        self.cam.ortho = to.ortho;
        if t >= 1.0 {
            self.cam = to;
            self.cam_anim = None;
        }
        ctx.request_repaint();
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let typing = ctx.egui_wants_keyboard_input();
        let fire = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        if fire(Modifiers::COMMAND, Key::Z) && !typing {
            self.run("edit.undo", &Value::Null);
        }
        if (fire(Modifiers::COMMAND, Key::Y) || fire(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)) && !typing {
            self.run("edit.redo", &Value::Null);
        }
        if fire(Modifiers::COMMAND, Key::S) {
            files::save(self, false);
        }
        if fire(Modifiers::COMMAND, Key::O) {
            files::open(self);
        }
        if fire(Modifiers::COMMAND, Key::N) {
            files::new(self);
        }
        if fire(Modifiers::COMMAND, Key::E) {
            files::export(self, "stl");
        }
        if typing {
            return;
        }
        if fire(Modifiers::NONE, Key::F6) {
            self.fit_all(true);
        }
        if fire(Modifiers::NONE, Key::Home) {
            self.set_view(fusioncraft_render::StdView::Home);
        }
        if self.sketch.is_some() && self.dialog.is_none() && fire(Modifiers::NONE, Key::E) {
            // Like Fusion: E in a sketch finishes it and starts Extrude on it.
            sketching::finish(self);
            dialogs::open_new(self, dialogs::NewKind::Extrude);
        }
        if self.sketch.is_none() && self.dialog.is_none() {
            if fire(Modifiers::NONE, Key::E) {
                dialogs::open_new(self, dialogs::NewKind::Extrude);
            }
            if fire(Modifiers::NONE, Key::Delete)
                && let Some(f) = self.selected_feature {
                    self.run("feature.delete", &serde_json::json!({ "id": f }));
                }
        }
    }
}

/// Native options and the app factory for the desktop binary.
pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FusionCraft")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(std::sync::Arc::new(app_icon())),
        ..Default::default()
    }
}

/// The window icon, drawn in code: an extruded block with the accent arrow.
pub fn app_icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let inside = |x: f32, y: f32, pts: &[(f32, f32)]| {
        let mut c = false;
        let m = pts.len();
        for i in 0..m {
            let (xi, yi) = pts[i];
            let (xj, yj) = pts[(i + m - 1) % m];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                c = !c;
            }
        }
        c
    };
    let lid = [(10.0, 26.0), (32.0, 15.0), (54.0, 26.0), (32.0, 37.0)];
    let left = [(10.0, 26.0), (32.0, 37.0), (32.0, 58.0), (10.0, 47.0)];
    let right = [(32.0, 37.0), (54.0, 26.0), (54.0, 47.0), (32.0, 58.0)];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let col = if (29.0..35.0).contains(&fx) && (3.0..24.0).contains(&fy) || (fy < 12.0 && (fx - 32.0).abs() < (12.0 - fy) * 0.9 + 0.5 && fy > 2.0) {
                Some([0x3d, 0x8b, 0xfd])
            } else if inside(fx, fy, &lid) {
                Some([0xd7, 0xdb, 0xe0])
            } else if inside(fx, fy, &left) {
                Some([0x9a, 0xa2, 0xae])
            } else if inside(fx, fy, &right) {
                Some([0x6c, 0x75, 0x84])
            } else {
                None
            };
            if let Some(c) = col {
                let o = (y * n + x) * 4;
                if let Some(px) = rgba.get_mut(o..o + 4) {
                    px.copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}
