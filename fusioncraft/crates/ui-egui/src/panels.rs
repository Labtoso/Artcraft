//! The window chrome around the viewport: menu bar, toolbar, browser, timeline and status bar.

use crate::dialogs::{self, NewKind};
use crate::icons::{self, Icon};
use crate::sketching::{self, Tool};
use crate::{files, theme, FusionCraftApp};
use egui::{vec2, Color32, CornerRadius, RichText, Sense, Stroke};
use fusioncraft_doc::FeatureKind;
use fusioncraft_render::{DisplayMode, StdView};
use serde_json::{json, Value};

/// A square button with an icon drawn in code and a caption below it.
fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, active: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::proportional(10.5), theme::TEXT_DIM);
    let size = vec2((galley.size().x + 10.0).max(58.0), 54.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter_at(rect);
    let bg = if active {
        theme::ACCENT.gamma_multiply(0.45)
    } else if enabled && resp.hovered() {
        theme::CONTROL_HOVER
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), bg);
    let col = if enabled { theme::ICON } else { theme::TEXT_FAINT.gamma_multiply(0.6) };
    let icon_rect = egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 19.0), vec2(26.0, 26.0));
    icons::paint(&painter, icon_rect, icon, col);
    let text_col = if enabled { theme::TEXT_DIM } else { theme::TEXT_FAINT.gamma_multiply(0.6) };
    painter.text(rect.center_bottom() - vec2(0.0, 7.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.5), text_col);
    resp.on_hover_text(label)
}

/// A small icon-only button for bars and tree rows.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tip: &str, enabled: bool) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), sense);
    let painter = ui.painter_at(rect);
    if enabled && resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(4), theme::CONTROL_HOVER);
    }
    let col = if enabled { theme::ICON } else { theme::TEXT_FAINT.gamma_multiply(0.6) };
    icons::paint(&painter, rect.shrink(3.0), icon, col);
    resp.on_hover_text(tip)
}

fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let row = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui);
        });
        let w = row.response.rect.width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, title, egui::FontId::proportional(10.0), theme::TEXT_FAINT);
    });
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 70.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, theme::BORDER));
    ui.add_space(4.0);
}

pub fn menu_bar(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    egui::Panel::top("menu").frame(egui::Frame::new().fill(theme::CHROME_DARK).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Design        Ctrl+N").clicked() {
                    files::new(app);
                    ui.close();
                }
                if ui.button("Open…             Ctrl+O").clicked() {
                    files::open(app);
                    ui.close();
                }
                ui.separator();
                if ui.button("Save              Ctrl+S").clicked() {
                    files::save(app, false);
                    ui.close();
                }
                if ui.button("Save As…").clicked() {
                    files::save(app, true);
                    ui.close();
                }
                ui.separator();
                if ui.button("Export STL…       Ctrl+E").clicked() {
                    files::export(app, "stl");
                    ui.close();
                }
                if ui.button("Export OBJ…").clicked() {
                    files::export(app, "obj");
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    app.quit_requested = true;
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = app.engine.undo_label().map(|l| format!("Undo {l}")).unwrap_or_else(|| "Undo".into());
                if ui.add_enabled(app.engine.can_undo(), egui::Button::new(format!("{undo}   Ctrl+Z"))).clicked() {
                    app.run("edit.undo", &Value::Null);
                    ui.close();
                }
                let redo = app.engine.redo_label().map(|l| format!("Redo {l}")).unwrap_or_else(|| "Redo".into());
                if ui.add_enabled(app.engine.can_redo(), egui::Button::new(format!("{redo}   Ctrl+Y"))).clicked() {
                    app.run("edit.redo", &Value::Null);
                    ui.close();
                }
                ui.separator();
                if ui.button("Change Parameters…").clicked() {
                    app.params_open = true;
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                for (label, v) in [("Home  (Home)", StdView::Home), ("Top", StdView::Top), ("Front", StdView::Front), ("Right", StdView::Right), ("Left", StdView::Left), ("Back", StdView::Back), ("Bottom", StdView::Bottom)] {
                    if ui.button(label).clicked() {
                        app.set_view(v);
                        ui.close();
                    }
                }
                if ui.button("Fit  (F6)").clicked() {
                    app.fit_all(true);
                    ui.close();
                }
                ui.separator();
                for (label, m) in [("Shaded with Visible Edges", DisplayMode::ShadedWithEdges), ("Shaded", DisplayMode::Shaded), ("Wireframe", DisplayMode::Wireframe)] {
                    if ui.radio(app.view.mode == m, label).clicked() {
                        app.view.mode = m;
                        ui.close();
                    }
                }
                ui.separator();
                ui.checkbox(&mut app.view.grid, "Layout Grid");
                ui.checkbox(&mut app.view.show_origin, "Origin Planes");
                ui.checkbox(&mut app.cam.ortho, "Orthographic");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    app.shortcuts_open = true;
                    ui.close();
                }
                if ui.button("About FusionCraft").clicked() {
                    app.about_open = true;
                    ui.close();
                }
            });
            ui.add_space(16.0);
            if icon_button(ui, Icon::Save, "Save (Ctrl+S)", true).clicked() {
                files::save(app, false);
            }
            if icon_button(ui, Icon::Undo, "Undo (Ctrl+Z)", app.engine.can_undo()).clicked() {
                app.run("edit.undo", &Value::Null);
            }
            if icon_button(ui, Icon::Redo, "Redo (Ctrl+Y)", app.engine.can_redo()).clicked() {
                app.run("edit.redo", &Value::Null);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dirty = if app.engine.dirty { " •" } else { "" };
                ui.label(RichText::new(format!("{}{dirty}", app.engine.title())).color(theme::TEXT_DIM));
            });
        });
    });
}

pub fn toolbar(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    egui::Panel::top("toolbar").frame(egui::Frame::new().fill(theme::CHROME).inner_margin(egui::Margin::symmetric(10, 4))).show(ui, |ui| {
        let sketching = app.sketch.is_some();
        ui.horizontal(|ui| {
            let tab = |ui: &mut egui::Ui, label: &str, on: bool| {
                let t = RichText::new(label).size(11.5).strong().color(if on { theme::TEXT } else { theme::TEXT_FAINT });
                let r = ui.add(egui::Label::new(t).sense(Sense::hover()));
                if on {
                    ui.painter().hline(r.rect.x_range(), r.rect.bottom() + 2.0, Stroke::new(2.0, theme::ACCENT));
                }
            };
            ui.label(RichText::new("DESIGN").size(11.5).color(theme::TEXT_DIM));
            ui.add_space(12.0);
            tab(ui, "SOLID", !sketching);
            ui.add_space(8.0);
            tab(ui, "SKETCH", sketching);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if sketching {
                sketch_tools(app, ui);
            } else {
                solid_tools(app, ui);
            }
        });
    });
}

fn solid_tools(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    let free = app.dialog.is_none();
    let has_body = !app.engine.model.bodies.is_empty();
    group(ui, "CREATE", |ui| {
        if tool_button(ui, Icon::Sketch, "Sketch", free, app.picking_plane).clicked() {
            app.picking_plane = !app.picking_plane;
            if app.picking_plane {
                app.set_status("Select a plane or a planar face to sketch on", false);
            }
        }
        for (icon, label, kind) in [
            (Icon::Extrude, "Extrude", NewKind::Extrude),
            (Icon::Revolve, "Revolve", NewKind::Revolve),
            (Icon::Box, "Box", NewKind::Box),
            (Icon::Cylinder, "Cylinder", NewKind::Cylinder),
            (Icon::Sphere, "Sphere", NewKind::Sphere),
            (Icon::Torus, "Torus", NewKind::Torus),
            (Icon::Cone, "Cone", NewKind::Cone),
        ] {
            if tool_button(ui, icon, label, free, false).clicked() {
                app.picking_plane = false;
                dialogs::open_new(app, kind);
            }
        }
        for (icon, label, kind) in [(Icon::Mirror, "Mirror", NewKind::Mirror), (Icon::PatternRect, "Rect Pattern", NewKind::PatternRect), (Icon::PatternCirc, "Circ Pattern", NewKind::PatternCirc)] {
            if tool_button(ui, icon, label, free && has_body, false).clicked() {
                dialogs::open_new(app, kind);
            }
        }
    });
    group(ui, "MODIFY", |ui| {
        if tool_button(ui, Icon::Move, "Move", free && has_body, false).clicked() {
            dialogs::open_new(app, NewKind::Move);
        }
        if tool_button(ui, Icon::Shell, "Shell", free && has_body, false).clicked() {
            dialogs::open_new(app, NewKind::Shell);
        }
        if tool_button(ui, Icon::Parameters, "Parameters", true, app.params_open).clicked() {
            app.params_open = !app.params_open;
        }
    });
    group(ui, "INSPECT", |ui| {
        if tool_button(ui, Icon::Properties, "Properties", true, false).clicked() {
            let m = &app.engine.model;
            let text = if m.bodies.is_empty() {
                "No bodies yet".to_owned()
            } else {
                let bb = m.aabb();
                let s = bb.max - bb.min;
                format!("{} bodies · volume {:.1} mm³ · size {:.1} × {:.1} × {:.1} mm", m.bodies.len(), m.volume(), s.x, s.y, s.z)
            };
            app.set_status(text, false);
        }
        if tool_button(ui, Icon::Display, "Display", true, false).clicked() {
            app.view.mode = match app.view.mode {
                DisplayMode::ShadedWithEdges => DisplayMode::Shaded,
                DisplayMode::Shaded => DisplayMode::Wireframe,
                DisplayMode::Wireframe => DisplayMode::ShadedWithEdges,
            };
        }
    });
}

fn sketch_tools(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    let current = app.sketch.as_ref().map(|s| s.tool);
    let construction = app.sketch.as_ref().is_some_and(|s| s.construction);
    group(ui, "CREATE", |ui| {
        for (icon, label, tool) in [
            (Icon::Line, "Line", Tool::Line),
            (Icon::Rect, "Rectangle", Tool::Rect),
            (Icon::Circle, "Circle", Tool::Circle),
            (Icon::Polygon, "Polygon", Tool::Polygon),
            (Icon::Arc, "Arc", Tool::Arc),
            (Icon::Point, "Point", Tool::Point),
        ] {
            if tool_button(ui, icon, label, true, current == Some(tool)).clicked() {
                let next = if current == Some(tool) { Tool::Select } else { tool };
                sketching::set_tool(app, next);
            }
        }
    });
    group(ui, "MODIFY", |ui| {
        if tool_button(ui, Icon::Construction, "Construction", true, construction).clicked()
            && let Some(s) = app.sketch.as_mut() {
                s.construction = !s.construction;
            }
        let sel = app.sketch.as_ref().and_then(|s| s.selected.map(|i| (s.id, i)));
        if tool_button(ui, Icon::Delete, "Delete", sel.is_some(), false).clicked()
            && let Some((id, i)) = sel {
                app.run("sketch.delete_entity", &json!({ "sketch": id, "index": i }));
                if let Some(s) = app.sketch.as_mut() {
                    s.selected = None;
                }
            }
    });
    group(ui, "FINISH", |ui| {
        if tool_button(ui, Icon::FinishSketch, "Finish Sketch", true, false).clicked() {
            sketching::finish(app);
        }
    });
}

pub fn status_bar(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    egui::Panel::bottom("status").frame(egui::Frame::new().fill(theme::CHROME_DARK).inner_margin(egui::Margin::symmetric(10, 3))).show(ui, |ui| {
        ui.horizontal(|ui| {
            match &app.status {
                Some(s) if crate::now() - s.at < 8.0 || s.error => {
                    let c = if s.error { theme::ERROR } else { theme::TEXT_DIM };
                    ui.label(RichText::new(&s.text).size(11.5).color(c));
                }
                _ => {
                    let hint = if app.picking_plane {
                        "Select a plane or a planar face to sketch on (Esc cancels)".to_owned()
                    } else if let Some(s) = &app.sketch {
                        sketching::hint(s)
                    } else {
                        "Right drag to orbit, middle drag to pan, wheel to zoom".to_owned()
                    };
                    ui.label(RichText::new(hint).size(11.5).color(theme::TEXT_FAINT));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let m = &app.engine.model;
                ui.label(RichText::new(format!("{} bodies · {} features", m.bodies.len(), app.engine.doc.timeline.len())).size(11.5).color(theme::TEXT_FAINT));
                if !m.errors.is_empty() {
                    ui.label(RichText::new(format!("{} errors", m.errors.len())).size(11.5).color(theme::ERROR));
                }
            });
        });
    });
    if app.picking_plane && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        app.picking_plane = false;
    }
}

fn feature_icon(k: &FeatureKind) -> Icon {
    match k {
        FeatureKind::Sketch { .. } => Icon::Sketch,
        FeatureKind::Extrude { .. } => Icon::Extrude,
        FeatureKind::Revolve { .. } => Icon::Revolve,
        FeatureKind::Box { .. } => Icon::Box,
        FeatureKind::Cylinder { .. } => Icon::Cylinder,
        FeatureKind::Sphere { .. } => Icon::Sphere,
        FeatureKind::Torus { .. } => Icon::Torus,
        FeatureKind::Cone { .. } => Icon::Cone,
        FeatureKind::Mirror { .. } => Icon::Mirror,
        FeatureKind::PatternRect { .. } => Icon::PatternRect,
        FeatureKind::PatternCirc { .. } => Icon::PatternCirc,
        FeatureKind::Move { .. } => Icon::Move,
        FeatureKind::Shell { .. } => Icon::Shell,
    }
}

enum TimelineAction {
    Edit(u64),
    Rename(u64),
    Suppress(u64),
    Delete(u64),
    Marker(Value),
}

pub fn timeline(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    let mut action = None;
    egui::Panel::bottom("timeline").frame(egui::Frame::new().fill(theme::CHROME).inner_margin(egui::Margin::symmetric(10, 6))).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if icon_button(ui, Icon::Home, "Roll to start", true).clicked() {
                action = Some(TimelineAction::Marker(json!(0)));
            }
            let active = app.engine.doc.active_len();
            let total = app.engine.doc.timeline.len();
            if icon_button(ui, Icon::Undo, "Step back", active > 0).clicked() {
                action = Some(TimelineAction::Marker(json!(active.saturating_sub(1))));
            }
            if icon_button(ui, Icon::Redo, "Step forward", active < total).clicked() {
                let next = active + 1;
                action = Some(TimelineAction::Marker(if next >= total { json!("end") } else { json!(next) }));
            }
            if icon_button(ui, Icon::Fit, "Roll to end", active < total).clicked() {
                action = Some(TimelineAction::Marker(json!("end")));
            }
            ui.add_space(8.0);
            egui::ScrollArea::horizontal().auto_shrink([false, true]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, f) in app.engine.doc.timeline.iter().enumerate() {
                        let rolled = i >= active;
                        let err = app.engine.model.errors.get(&f.id);
                        let selected = app.selected_feature == Some(f.id);
                        let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                        let p = ui.painter_at(rect);
                        let fill = if selected {
                            theme::ACCENT.gamma_multiply(0.55)
                        } else if resp.hovered() {
                            theme::CONTROL_HOVER
                        } else {
                            theme::CONTROL
                        };
                        p.rect_filled(rect, CornerRadius::same(4), if rolled { fill.gamma_multiply(0.35) } else { fill });
                        if err.is_some() {
                            p.rect_stroke(rect.shrink(0.5), CornerRadius::same(4), Stroke::new(1.5, theme::ERROR), egui::StrokeKind::Inside);
                        }
                        let col = if f.suppressed || rolled { theme::TEXT_FAINT.gamma_multiply(0.6) } else if err.is_some() { theme::ERROR } else { theme::ICON };
                        icons::paint(&p, rect.shrink(6.0), feature_icon(&f.kind), col);
                        if f.suppressed {
                            p.line_segment([rect.left_bottom() + vec2(5.0, -5.0), rect.right_top() + vec2(-5.0, 5.0)], Stroke::new(1.5, theme::TEXT_FAINT));
                        }
                        let tip = match err {
                            Some(e) => format!("{}\n{e}", f.name),
                            None if f.suppressed => format!("{} (suppressed)", f.name),
                            None => f.name.clone(),
                        };
                        let resp = resp.on_hover_text(tip);
                        if resp.clicked() {
                            app.selected_feature = Some(f.id);
                        }
                        if resp.double_clicked() {
                            action = Some(TimelineAction::Edit(f.id));
                        }
                        let id = f.id;
                        resp.context_menu(|ui| {
                            if ui.button("Edit Feature").clicked() {
                                action = Some(TimelineAction::Edit(id));
                                ui.close();
                            }
                            if ui.button("Rename").clicked() {
                                action = Some(TimelineAction::Rename(id));
                                ui.close();
                            }
                            if ui.button(if f.suppressed { "Unsuppress Features" } else { "Suppress Features" }).clicked() {
                                action = Some(TimelineAction::Suppress(id));
                                ui.close();
                            }
                            if ui.button("Roll History Marker Here").clicked() {
                                let pos = i + 1;
                                action = Some(TimelineAction::Marker(if pos >= total { json!("end") } else { json!(pos) }));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(RichText::new("Delete").color(theme::ERROR)).clicked() {
                                action = Some(TimelineAction::Delete(id));
                                ui.close();
                            }
                        });
                        if i + 1 == active && active < total {
                            marker(ui);
                        }
                    }
                    if active == total && total > 0 {
                        marker(ui);
                    }
                });
            });
        });
    });
    match action {
        Some(TimelineAction::Edit(id)) => dialogs::open_edit(app, id),
        Some(TimelineAction::Rename(id)) => {
            let name = app.engine.doc.feature(id).map(|f| f.name.clone()).unwrap_or_default();
            app.rename = Some((id, name));
        }
        Some(TimelineAction::Suppress(id)) => {
            app.run("feature.suppress", &json!({ "id": id }));
        }
        Some(TimelineAction::Delete(id)) => {
            app.run("feature.delete", &json!({ "id": id }));
        }
        Some(TimelineAction::Marker(pos)) => {
            app.run("timeline.marker", &json!({ "position": pos }));
        }
        None => {}
    }
}

fn marker(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(8.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.vline(r.center().x, r.y_range(), Stroke::new(2.5, theme::ACCENT));
    p.circle_filled(r.center_top(), 3.5, theme::ACCENT);
}

fn tree_header(ui: &mut egui::Ui, label: &str, open: &mut bool) {
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::click());
        let c = r.center();
        let pts = if *open {
            vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)]
        } else {
            vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, theme::TEXT_DIM, Stroke::NONE));
        let l = ui.add(egui::Label::new(RichText::new(label).color(theme::TEXT)).sense(Sense::click()));
        if resp.clicked() || l.clicked() {
            *open = !*open;
        }
    });
}

pub fn browser(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    let mut toggle_sketch = None;
    let mut edit_sketch = None;
    egui::Panel::left("browser")
        .resizable(true)
        .default_size(230.0)
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10)))
        .show(ui, |ui| {
            ui.label(RichText::new("BROWSER").size(11.0).strong().color(theme::TEXT_FAINT));
            ui.add_space(6.0);
            let id = ui.scope_id().with("browser_open");
            let mut open: [bool; 3] = ui.data(|d| d.get_temp(id)).unwrap_or([true, true, true]);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(RichText::new(&app.engine.doc.name).strong().color(theme::TEXT));
                ui.indent("doc", |ui| {
                    tree_header(ui, "Origin", &mut open[0]);
                    if open[0] {
                        ui.indent("origin", |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut app.view.show_origin, "");
                                ui.label(RichText::new("XY · XZ · YZ").color(theme::TEXT_DIM));
                            });
                        });
                    }
                    tree_header(ui, &format!("Sketches ({})", app.engine.doc.sketches.len()), &mut open[1]);
                    if open[1] {
                        ui.indent("sketches", |ui| {
                            for s in &app.engine.doc.sketches {
                                ui.horizontal(|ui| {
                                    let eye = if s.visible { Icon::Display } else { Icon::Grid };
                                    if icon_button(ui, eye, if s.visible { "Hide" } else { "Show" }, true).clicked() {
                                        toggle_sketch = Some(s.id);
                                    }
                                    let active = app.sketch.as_ref().is_some_and(|a| a.id == s.id);
                                    let col = if active { theme::ACCENT } else if s.visible { theme::TEXT } else { theme::TEXT_FAINT };
                                    let r = ui.add(egui::Label::new(RichText::new(&s.name).color(col)).sense(Sense::click()));
                                    if r.double_clicked() {
                                        edit_sketch = Some(s.id);
                                    }
                                    r.context_menu(|ui| {
                                        if ui.button("Edit Sketch").clicked() {
                                            edit_sketch = Some(s.id);
                                            ui.close();
                                        }
                                    });
                                });
                            }
                        });
                    }
                    let bodies: Vec<(usize, String)> = app.engine.model.bodies.iter().map(|b| (b.index, b.name.clone())).collect();
                    tree_header(ui, &format!("Bodies ({})", bodies.len()), &mut open[2]);
                    if open[2] {
                        ui.indent("bodies", |ui| {
                            for (index, name) in bodies {
                                ui.horizontal(|ui| {
                                    let hidden = app.view.hidden_bodies.contains(&index);
                                    let eye = if hidden { Icon::Grid } else { Icon::Display };
                                    if icon_button(ui, eye, if hidden { "Show" } else { "Hide" }, true).clicked() {
                                        if hidden {
                                            app.view.hidden_bodies.remove(&index);
                                        } else {
                                            app.view.hidden_bodies.insert(index);
                                        }
                                    }
                                    let sel = app.selected_bodies.contains(&index);
                                    let col = if sel { theme::ACCENT } else if hidden { theme::TEXT_FAINT } else { theme::TEXT };
                                    if ui.add(egui::Label::new(RichText::new(name).color(col)).sense(Sense::click())).clicked() {
                                        app.selected_bodies = if sel { Vec::new() } else { vec![index] };
                                    }
                                });
                            }
                        });
                    }
                });
            });
            ui.data_mut(|d| d.insert_temp(id, open));
        });
    if let Some(id) = toggle_sketch {
        app.run("sketch.show", &json!({ "sketch": id }));
    }
    if let Some(id) = edit_sketch {
        sketching::edit(app, id);
    }
}
