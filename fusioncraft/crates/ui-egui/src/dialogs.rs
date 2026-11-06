//! Feature dialogs (with live preview) and the other windows.

use crate::{theme, FusionCraftApp};
use egui::{Color32, RichText};
use fusioncraft_doc::feature::WorldAxis;
use fusioncraft_doc::{Axis, Entity, Expr, ExtentDirection, Feature, FeatureKind, Id, Operation, PlaneRef};
use fusioncraft_geom::Vec3;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Extrude,
    Revolve,
    Box,
    Cylinder,
    Sphere,
    Torus,
    Cone,
    Mirror,
    PatternRect,
    PatternCirc,
    Move,
    Shell,
}

pub struct Dialog {
    pub kind: FeatureKind,
    /// The feature being edited, or `None` for a new one.
    pub editing: Option<Id>,
    pub dirty: bool,
    pub error: Option<String>,
}

impl Dialog {
    /// Profile selection of extrude/revolve, for viewport picking.
    pub fn profiles_mut(&mut self) -> Option<(Id, &mut Vec<usize>)> {
        match &mut self.kind {
            FeatureKind::Extrude { sketch, profiles, .. } | FeatureKind::Revolve { sketch, profiles, .. } => Some((*sketch, profiles)),
            _ => None,
        }
    }
    pub fn sketch(&self) -> Option<Id> {
        match &self.kind {
            FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } => Some(*sketch),
            _ => None,
        }
    }
}

/// The most recent sketch before the marker that has a closed profile.
fn default_sketch(app: &FusionCraftApp) -> Option<Id> {
    app.engine
        .doc
        .timeline
        .iter()
        .take(app.engine.doc.active_len())
        .rev()
        .filter_map(|f| match f.kind {
            FeatureKind::Sketch { sketch } => Some(sketch),
            _ => None,
        })
        .find(|s| !fusioncraft_model::sketch_profiles(&app.engine.doc, *s).is_empty())
}

pub fn open_new(app: &mut FusionCraftApp, what: NewKind) {
    if app.sketch.is_some() {
        crate::sketching::finish(app);
    }
    let has_body = !app.engine.model.bodies.is_empty();
    let op = if has_body { Operation::Join } else { Operation::NewBody };
    let e = |s: &str| Expr::new(s);
    let kind = match what {
        NewKind::Extrude | NewKind::Revolve => {
            let Some(sketch) = default_sketch(app) else {
                app.set_status("Create a sketch with a closed profile first", true);
                return;
            };
            if what == NewKind::Extrude {
                FeatureKind::Extrude { sketch, profiles: Vec::new(), distance: e("10 mm"), direction: ExtentDirection::OneSide, operation: op }
            } else {
                FeatureKind::Revolve { sketch, profiles: Vec::new(), axis: Axis::SketchY, angle: e("360 deg"), operation: op }
            }
        }
        NewKind::Box => FeatureKind::Box { base: Vec3::ZERO, width: e("20 mm"), depth: e("20 mm"), height: e("20 mm"), operation: op },
        NewKind::Cylinder => FeatureKind::Cylinder { base: Vec3::ZERO, radius: e("10 mm"), height: e("20 mm"), operation: op },
        NewKind::Sphere => FeatureKind::Sphere { center: Vec3::ZERO, radius: e("10 mm"), operation: op },
        NewKind::Torus => FeatureKind::Torus { center: Vec3::ZERO, major: e("20 mm"), minor: e("5 mm"), operation: op },
        NewKind::Cone => FeatureKind::Cone { base: Vec3::ZERO, radius: e("10 mm"), top_radius: e("0 mm"), height: e("20 mm"), operation: op },
        NewKind::Mirror | NewKind::PatternRect | NewKind::PatternCirc | NewKind::Move | NewKind::Shell if !has_body => {
            app.set_status("Create a body first", true);
            return;
        }
        NewKind::Mirror => FeatureKind::Mirror { plane: PlaneRef::Yz },
        NewKind::PatternRect => FeatureKind::PatternRect { count_x: 3, count_y: 1, spacing_x: e("30 mm"), spacing_y: e("30 mm") },
        NewKind::PatternCirc => FeatureKind::PatternCirc { count: 6, angle: e("360 deg"), axis: WorldAxis::Z },
        NewKind::Move => FeatureKind::Move { dx: e("0 mm"), dy: e("0 mm"), dz: e("10 mm") },
        NewKind::Shell => FeatureKind::Shell { thickness: e("2 mm") },
    };
    app.picking_plane = false;
    app.dialog = Some(Dialog { kind, editing: None, dirty: true, error: None });
}

pub fn open_edit(app: &mut FusionCraftApp, id: Id) {
    let Some(f) = app.engine.doc.feature(id) else { return };
    if let FeatureKind::Sketch { sketch } = f.kind {
        crate::sketching::edit(app, sketch);
        return;
    }
    let kind = f.kind.clone();
    if app.sketch.is_some() {
        crate::sketching::finish(app);
    }
    app.dialog = Some(Dialog { kind, editing: Some(id), dirty: true, error: None });
}

fn refresh_preview(app: &mut FusionCraftApp) {
    let Some(d) = app.dialog.as_mut() else { return };
    if !d.dirty {
        return;
    }
    d.dirty = false;
    let mut doc = app.engine.doc.clone();
    let id = match d.editing {
        Some(id) => {
            if let Some(f) = doc.feature_mut(id) {
                f.kind = d.kind.clone();
            }
            id
        }
        None => {
            let id = doc.alloc_id();
            let name = doc.next_name(d.kind.label());
            doc.insert_feature(Feature { id, name, suppressed: false, kind: d.kind.clone() });
            id
        }
    };
    let model = fusioncraft_model::evaluate(&doc);
    d.error = model.errors.get(&id).cloned();
    app.preview = Some(model);
    app.model_version += 1;
}

fn commit(app: &mut FusionCraftApp) {
    let Some(d) = app.dialog.take() else { return };
    app.preview = None;
    app.model_version += 1;
    let kind = match serde_json::to_value(&d.kind) {
        Ok(v) => v,
        Err(e) => {
            app.set_status(e.to_string(), true);
            return;
        }
    };
    let ok = match d.editing {
        Some(id) => app.run("feature.edit", &json!({ "id": id, "kind": kind })).is_some(),
        None => app.run("feature.add", &json!({ "kind": kind })).is_some(),
    };
    if !ok {
        // Keep the dialog open with the error so nothing typed is lost.
        let err = app.status.as_ref().map(|s| s.text.clone());
        app.dialog = Some(Dialog { error: err, dirty: true, ..d });
    }
}

pub fn cancel(app: &mut FusionCraftApp) {
    app.dialog = None;
    app.preview = None;
    app.model_version += 1;
}

/// An expression field with its evaluated value.
fn expr_field(ui: &mut egui::Ui, label: &str, e: &mut Expr, params: &fusioncraft_doc::Params, unit: &str) -> bool {
    let mut changed = false;
    ui.label(RichText::new(label).color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut e.0).desired_width(120.0));
        changed |= r.changed();
        match params.eval(e) {
            Ok(v) => ui.label(RichText::new(format!("{} {unit}", fusioncraft_doc::expr::format_number((v * 1000.0).round() / 1000.0))).color(theme::TEXT_FAINT)),
            Err(err) => ui.label(RichText::new("!").color(theme::ERROR)).on_hover_text(err.to_string()),
        };
    });
    ui.end_row();
    changed
}

fn op_field(ui: &mut egui::Ui, op: &mut Operation) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Operation").color(theme::TEXT_DIM));
    egui::ComboBox::from_id_salt("op").selected_text(op.label()).show_ui(ui, |ui| {
        for o in Operation::ALL {
            changed |= ui.selectable_value(op, o, o.label()).changed();
        }
    });
    ui.end_row();
    changed
}

fn vec3_field(ui: &mut egui::Ui, label: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.label(RichText::new(label).color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        for c in [&mut v.x, &mut v.y, &mut v.z] {
            changed |= ui.add(egui::DragValue::new(c).speed(0.5).max_decimals(3)).changed();
        }
    });
    ui.end_row();
    changed
}

fn count_field(ui: &mut egui::Ui, label: &str, n: &mut u32) -> bool {
    ui.label(RichText::new(label).color(theme::TEXT_DIM));
    let c = ui.add(egui::DragValue::new(n).range(1..=fusioncraft_model::MAX_COPIES)).changed();
    ui.end_row();
    c
}

fn plane_combo(ui: &mut egui::Ui, id: &str, p: &mut PlaneRef) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(format!("{} plane", p.label())).show_ui(ui, |ui| {
        for q in [PlaneRef::Xy, PlaneRef::Xz, PlaneRef::Yz] {
            changed |= ui.selectable_value(p, q, format!("{} plane", q.label())).changed();
        }
    });
    changed
}

/// The feature dialog, floating at the right of the viewport like Fusion's command dialogs.
pub fn show(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    refresh_preview(app);
    let params = app.engine.doc.params();
    let sketches: Vec<(Id, String)> = app.engine.doc.sketches.iter().map(|s| (s.id, s.name.clone())).collect();
    let doc = &app.engine.doc;
    let lines: Vec<(usize, String)> = app
        .dialog
        .as_ref()
        .and_then(Dialog::sketch)
        .and_then(|s| doc.sketch(s))
        .map(|s| {
            s.entities
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e, Entity::Line { .. }))
                .map(|(i, e)| (i, format!("Line {}{}", i + 1, if matches!(e, Entity::Line { construction: true, .. }) { " (construction)" } else { "" })))
                .collect()
        })
        .unwrap_or_default();
    let profile_count = app.dialog.as_ref().and_then(Dialog::sketch).map_or(0, |s| fusioncraft_model::sketch_profiles(doc, s).len());
    let Some(d) = app.dialog.as_mut() else { return };
    let title = format!("{} {}", if d.editing.is_some() { "EDIT" } else { "" }, d.kind.label().to_uppercase()).trim().to_owned();
    let mut ok = false;
    let mut cancel_it = false;
    let mut changed = false;
    egui::Window::new(RichText::new(title).strong())
        .id(egui::Id::unique("feature-dialog"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 92.0))
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.set_min_width(300.0);
            egui::Grid::new("dlg").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                match &mut d.kind {
                    FeatureKind::Extrude { sketch, profiles, distance, direction, operation } => {
                        changed |= sketch_combo(ui, sketch, &sketches, profiles);
                        profiles_row(ui, profiles, profile_count, &mut changed);
                        ui.label(RichText::new("Direction").color(theme::TEXT_DIM));
                        egui::ComboBox::from_id_salt("dir")
                            .selected_text(if *direction == ExtentDirection::Symmetric { "Symmetric" } else { "One Side" })
                            .show_ui(ui, |ui| {
                                changed |= ui.selectable_value(direction, ExtentDirection::OneSide, "One Side").changed();
                                changed |= ui.selectable_value(direction, ExtentDirection::Symmetric, "Symmetric").changed();
                            });
                        ui.end_row();
                        changed |= expr_field(ui, "Distance", distance, &params, "mm");
                        ui.label("");
                        if ui.button("Flip").on_hover_text("Extrude to the other side").clicked() {
                            let t = distance.0.trim().to_owned();
                            distance.0 = match t.strip_prefix('-') {
                                Some(rest) => rest.trim().trim_start_matches('(').trim_end_matches(')').to_owned(),
                                None => format!("-({t})"),
                            };
                            changed = true;
                        }
                        ui.end_row();
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Revolve { sketch, profiles, axis, angle, operation } => {
                        changed |= sketch_combo(ui, sketch, &sketches, profiles);
                        profiles_row(ui, profiles, profile_count, &mut changed);
                        ui.label(RichText::new("Axis").color(theme::TEXT_DIM));
                        let axis_label = match axis {
                            Axis::SketchX => "Sketch X axis".to_owned(),
                            Axis::SketchY => "Sketch Y axis".to_owned(),
                            Axis::Line { index } => format!("Line {}", *index + 1),
                        };
                        egui::ComboBox::from_id_salt("axis").selected_text(axis_label).show_ui(ui, |ui| {
                            changed |= ui.selectable_value(axis, Axis::SketchX, "Sketch X axis").changed();
                            changed |= ui.selectable_value(axis, Axis::SketchY, "Sketch Y axis").changed();
                            for (i, l) in &lines {
                                changed |= ui.selectable_value(axis, Axis::Line { index: *i }, l).changed();
                            }
                        });
                        ui.end_row();
                        changed |= expr_field(ui, "Angle", angle, &params, "deg");
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Box { base, width, depth, height, operation } => {
                        changed |= expr_field(ui, "Width (X)", width, &params, "mm");
                        changed |= expr_field(ui, "Depth (Y)", depth, &params, "mm");
                        changed |= expr_field(ui, "Height (Z)", height, &params, "mm");
                        changed |= vec3_field(ui, "Base centre", base);
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Cylinder { base, radius, height, operation } => {
                        changed |= expr_field(ui, "Radius", radius, &params, "mm");
                        changed |= expr_field(ui, "Height", height, &params, "mm");
                        changed |= vec3_field(ui, "Base centre", base);
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Sphere { center, radius, operation } => {
                        changed |= expr_field(ui, "Radius", radius, &params, "mm");
                        changed |= vec3_field(ui, "Centre", center);
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Torus { center, major, minor, operation } => {
                        changed |= expr_field(ui, "Ring radius", major, &params, "mm");
                        changed |= expr_field(ui, "Tube radius", minor, &params, "mm");
                        changed |= vec3_field(ui, "Centre", center);
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Cone { base, radius, top_radius, height, operation } => {
                        changed |= expr_field(ui, "Base radius", radius, &params, "mm");
                        changed |= expr_field(ui, "Top radius", top_radius, &params, "mm");
                        changed |= expr_field(ui, "Height", height, &params, "mm");
                        changed |= vec3_field(ui, "Base centre", base);
                        changed |= op_field(ui, operation);
                    }
                    FeatureKind::Mirror { plane } => {
                        ui.label(RichText::new("Mirror plane").color(theme::TEXT_DIM));
                        changed |= plane_combo(ui, "mirror-plane", plane);
                        ui.end_row();
                    }
                    FeatureKind::PatternRect { count_x, count_y, spacing_x, spacing_y } => {
                        changed |= count_field(ui, "Quantity X", count_x);
                        changed |= expr_field(ui, "Spacing X", spacing_x, &params, "mm");
                        changed |= count_field(ui, "Quantity Y", count_y);
                        changed |= expr_field(ui, "Spacing Y", spacing_y, &params, "mm");
                    }
                    FeatureKind::PatternCirc { count, angle, axis } => {
                        changed |= count_field(ui, "Quantity", count);
                        changed |= expr_field(ui, "Total angle", angle, &params, "deg");
                        ui.label(RichText::new("Axis").color(theme::TEXT_DIM));
                        ui.horizontal(|ui| {
                            for (a, l) in [(WorldAxis::X, "X"), (WorldAxis::Y, "Y"), (WorldAxis::Z, "Z")] {
                                changed |= ui.selectable_value(axis, a, l).changed();
                            }
                        });
                        ui.end_row();
                    }
                    FeatureKind::Move { dx, dy, dz } => {
                        changed |= expr_field(ui, "X distance", dx, &params, "mm");
                        changed |= expr_field(ui, "Y distance", dy, &params, "mm");
                        changed |= expr_field(ui, "Z distance", dz, &params, "mm");
                    }
                    FeatureKind::Shell { thickness } => {
                        changed |= expr_field(ui, "Thickness", thickness, &params, "mm");
                    }
                    FeatureKind::Sketch { .. } => {}
                }
            });
            if let Some(err) = &d.error {
                ui.add_space(4.0);
                ui.colored_label(theme::ERROR, err);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Cancel"))).clicked() {
                        cancel_it = true;
                    }
                    let okb = egui::Button::new(RichText::new("  OK  ").color(Color32::WHITE)).fill(theme::ACCENT);
                    if ui.add(okb).clicked() {
                        ok = true;
                    }
                });
            });
        });
    if changed {
        d.dirty = true;
    }
    let (enter, esc) = ui.ctx().input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if ok || enter {
        commit(app);
    } else if cancel_it || esc {
        cancel(app);
    }
}

fn sketch_combo(ui: &mut egui::Ui, sketch: &mut Id, sketches: &[(Id, String)], profiles: &mut Vec<usize>) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Sketch").color(theme::TEXT_DIM));
    let name = sketches.iter().find(|(i, _)| i == sketch).map(|(_, n)| n.clone()).unwrap_or_default();
    egui::ComboBox::from_id_salt("sketch").selected_text(name).show_ui(ui, |ui| {
        for (i, n) in sketches {
            if ui.selectable_value(sketch, *i, n).changed() {
                profiles.clear();
                changed = true;
            }
        }
    });
    ui.end_row();
    changed
}

fn profiles_row(ui: &mut egui::Ui, profiles: &mut Vec<usize>, count: usize, changed: &mut bool) {
    ui.label(RichText::new("Profiles").color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        let text = if profiles.is_empty() { format!("All ({count})") } else { format!("{} of {count} selected", profiles.len()) };
        ui.label(text).on_hover_text("Click profiles in the canvas to choose them");
        if !profiles.is_empty() && ui.small_button("All").clicked() {
            profiles.clear();
            *changed = true;
        }
    });
    ui.end_row();
}

/// Parameters, rename, about, shortcuts and confirmation windows.
pub fn windows(app: &mut FusionCraftApp, ctx: &egui::Context) {
    if app.params_open {
        parameters(app, ctx);
    }
    if let Some((id, mut name)) = app.rename.take() {
        let mut open = true;
        let mut done = false;
        egui::Window::new("Rename").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).open(&mut open).show(ctx, |ui| {
            let r = ui.text_edit_singleline(&mut name);
            r.request_focus();
            if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = true;
            }
        });
        if done {
            app.run("feature.rename", &json!({ "id": id, "name": name }));
        } else if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.rename = Some((id, name));
        }
    }
    if app.confirm_new {
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
            ui.label("The design has unsaved changes. Start a new design anyway?");
            ui.horizontal(|ui| {
                if ui.button("Save first").clicked() {
                    app.confirm_new = false;
                    crate::files::save(app, false);
                }
                if ui.button("Discard and start new").clicked() {
                    app.confirm_new = false;
                    crate::files::really_new(app);
                }
                if ui.button("Cancel").clicked() {
                    app.confirm_new = false;
                }
            });
        });
    }
    if app.about_open {
        egui::Window::new("About FusionCraft").collapsible(false).resizable(false).open(&mut app.about_open).show(ctx, |ui| {
            ui.heading("FusionCraft");
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("Parametric 3D design in pure Rust: sketches, extrude, revolve, primitives, booleans, patterns and a feature timeline.");
            ui.label("License: MIT OR Apache-2.0");
            ui.label("© LabtosoStudios");
        });
    }
    if app.shortcuts_open {
        egui::Window::new("Keyboard and mouse").collapsible(false).resizable(false).open(&mut app.shortcuts_open).show(ctx, |ui| {
            egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                for (k, v) in [
                    ("Middle mouse drag", "Pan"),
                    ("Shift + middle drag / right drag", "Orbit"),
                    ("Mouse wheel", "Zoom at the cursor"),
                    ("F6", "Fit all"),
                    ("Home", "Home view"),
                    ("E", "Extrude"),
                    ("L / R / C / P / A", "Sketch line, rectangle, circle, polygon, arc"),
                    ("X", "Construction on/off (sketch)"),
                    ("Type a number + Enter", "Exact length, radius or angle while drawing"),
                    ("Esc", "Stop the tool, then finish the sketch"),
                    ("Delete", "Delete the selected curve or feature"),
                    ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
                    ("Ctrl+S / Ctrl+O / Ctrl+N", "Save / Open / New"),
                    ("Ctrl+E", "Export STL"),
                ] {
                    ui.label(RichText::new(k).strong());
                    ui.label(v);
                    ui.end_row();
                }
            });
        });
    }
}

fn parameters(app: &mut FusionCraftApp, ctx: &egui::Context) {
    let mut open = true;
    let params = app.engine.doc.params();
    let mut list: Vec<(String, String, String)> = app.engine.doc.parameters.iter().map(|p| (p.name.clone(), p.expr.0.clone(), p.comment.clone())).collect();
    let mut set: Vec<(String, String, String)> = Vec::new();
    let mut delete: Option<String> = None;
    let mut add = false;
    egui::Window::new("Parameters").id(egui::Id::unique("params-window")).open(&mut open).default_width(520.0).show(ctx, |ui| {
        ui.label(RichText::new("User parameters can be used in every distance, angle and size field, e.g. width / 2.").color(theme::TEXT_DIM));
        ui.add_space(4.0);
        egui::Grid::new("params").num_columns(5).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            for h in ["Name", "Expression", "Value", "Comment", ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (name, expr, comment) in &mut list {
                ui.label(name.as_str());
                let r1 = ui.add(egui::TextEdit::singleline(expr).desired_width(140.0));
                match params.eval(&Expr::new(expr.clone())) {
                    Ok(v) => ui.label(fusioncraft_doc::expr::format_number((v * 1000.0).round() / 1000.0)),
                    Err(e) => ui.colored_label(theme::ERROR, e.to_string()),
                };
                let r2 = ui.add(egui::TextEdit::singleline(comment).desired_width(140.0));
                if r1.lost_focus() || r2.lost_focus() {
                    set.push((name.clone(), expr.clone(), comment.clone()));
                }
                if ui.small_button("Delete").clicked() {
                    delete = Some(name.clone());
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if ui.button("+ Add parameter").clicked() {
            add = true;
        }
    });
    for (name, expr, comment) in set {
        let unchanged = app.engine.doc.parameters.iter().any(|p| p.name == name && p.expr.0 == expr && p.comment == comment);
        if !unchanged {
            app.run("param.set", &json!({ "name": name, "expr": expr, "comment": comment }));
        }
    }
    if let Some(n) = delete {
        app.run("param.delete", &json!({ "name": n }));
    }
    if add {
        let name = (1..).map(|i| format!("d{i}")).find(|n| !app.engine.doc.parameters.iter().any(|p| &p.name == n)).unwrap_or_else(|| "d".into());
        app.run("param.set", &json!({ "name": name, "expr": "10 mm" }));
    }
    app.params_open = open;
}
