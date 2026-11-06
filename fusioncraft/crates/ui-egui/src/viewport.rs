//! The 3D canvas: rendering, camera navigation, picking, the ViewCube and overlays.

use crate::{sketching, theme, FusionCraftApp};
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke};
use fusioncraft_doc::{PlaneRef, SketchPlane};
use fusioncraft_geom::{Plane, Vec2, Vec3};
use fusioncraft_render::{render, Camera, DisplayMode, Frame, Scene, StdView, Style};
use serde_json::json;
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub struct ViewportCache {
    texture: Option<egui::TextureHandle>,
    key: u64,
    ssaa: usize,
    frame: Option<Frame>,
    last_motion: f64,
}

/// Screen ↔ world mapping for the current frame.
#[derive(Clone, Copy)]
pub struct View {
    pub rect: Rect,
    pub cam: Camera,
}

impl View {
    pub fn to_screen(self, p: Vec3) -> Option<Pos2> {
        let (x, y, _) = self.cam.project(p, f64::from(self.rect.width()), f64::from(self.rect.height()))?;
        Some(pos2(self.rect.left() + x as f32, self.rect.top() + y as f32))
    }
    pub fn ray(self, p: Pos2) -> (Vec3, Vec3) {
        self.cam.ray(f64::from(p.x - self.rect.left()), f64::from(p.y - self.rect.top()), f64::from(self.rect.width()), f64::from(self.rect.height()))
    }
    /// Where a screen point lands on a plane, in the plane's 2D coordinates.
    pub fn on_plane(self, plane: &Plane, p: Pos2) -> Option<Vec2> {
        let (o, d) = self.ray(p);
        plane.intersect_ray(o, d).map(|w| plane.to_local(w)).or({
            // Looking exactly along the plane: no hit.
            None
        })
    }
    pub fn units_per_point(self) -> f64 {
        self.cam.units_per_pixel(f64::from(self.rect.height()))
    }
    /// A polyline in the plane, clipped at the near plane.
    pub fn plane_polyline(self, plane: &Plane, pts: &[Vec2]) -> Vec<Pos2> {
        pts.iter().filter_map(|p| self.to_screen(plane.to_world(*p))).collect()
    }
}

fn hash_f(h: &mut std::collections::hash_map::DefaultHasher, v: f64) {
    v.to_bits().hash(h);
}

pub fn show(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::CANVAS)).show(ui, |ui| {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let view = View { rect, cam: app.cam };
        navigate(app, ui, &resp, view);
        let view = View { rect, cam: app.cam };
        draw_scene(app, ui, rect);
        let painter = ui.painter_at(rect);
        let hover_pos = resp.hover_pos();

        // Overlays: sketches, profiles, origin planes, tool previews.
        sketching::draw_sketches(app, &painter, view);
        if let Some(d) = &app.dialog
            && let Some(s) = d.sketch() {
                draw_profiles(app, &painter, view, s, hover_pos);
            }
        let plane_hover = if app.picking_plane { pick_plane(app, view, hover_pos) } else { None };
        if app.picking_plane || (app.view.show_origin && app.sketch.is_none()) {
            draw_origin(&painter, view, app.picking_plane, plane_hover.as_ref());
        }
        if app.sketch.is_some() {
            sketching::viewport_input(app, ui, &resp, &painter, view);
        } else if resp.clicked() {
            click(app, view, hover_pos, plane_hover, ui.input(|i| i.modifiers.shift));
        }
        if !app.picking_plane && app.sketch.is_none() {
            hover_body(app, view, hover_pos);
        }
        hint(app, &painter, rect);
        view_cube(app, ui, rect);
        nav_bar(app, ui, rect);
    });
}

fn navigate(app: &mut FusionCraftApp, ui: &egui::Ui, resp: &egui::Response, view: View) {
    let (shift, scroll, zoom, pointer) = ui.input(|i| (i.modifiers.shift, i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
    let mut moved = false;
    let d = resp.drag_delta();
    let orbit = resp.dragged_by(egui::PointerButton::Secondary) || (resp.dragged_by(egui::PointerButton::Middle) && shift);
    if orbit && d != egui::Vec2::ZERO {
        app.cam_anim = None;
        app.cam.orbit(-f64::from(d.x) * 0.008, f64::from(d.y) * 0.008);
        moved = true;
    } else if resp.dragged_by(egui::PointerButton::Middle) && d != egui::Vec2::ZERO {
        app.cam_anim = None;
        app.cam.pan(f64::from(d.x), f64::from(d.y), f64::from(view.rect.height()));
        moved = true;
    }
    if resp.hovered() && (scroll != 0.0 || zoom != 1.0) {
        app.cam_anim = None;
        let factor = if zoom != 1.0 { 1.0 / f64::from(zoom) } else { (-f64::from(scroll) * 0.0015).exp() };
        let focus = pointer.and_then(|p| focus_point(app, view, p));
        app.cam.zoom_at(factor, focus);
        moved = true;
    }
    if moved {
        app.cache.last_motion = crate::now();
    }
}

/// The point under the cursor for zooming: on a body, else on the view plane through the target.
fn focus_point(app: &FusionCraftApp, view: View, p: Pos2) -> Option<Vec3> {
    let (o, d) = view.ray(p);
    let hit = app.cache.frame.as_ref().and_then(|f| {
        let ppp = f.width as f32 / view.rect.width().max(1.0);
        let px = ((p.x - view.rect.left()) * ppp) as usize;
        let py = ((p.y - view.rect.top()) * ppp) as usize;
        let pick = f.pick(px, py)?;
        let tri = app.shown_model().bodies.get(pick.body)?.mesh.tris.get(pick.triangle)?;
        let plane = Plane::new(tri.v[0], tri.normal(), Vec3::X);
        plane.intersect_ray(o, d)
    });
    hit.or_else(|| Plane::new(view.cam.target, -view.cam.forward(), view.cam.right()).intersect_ray(o, d))
}

fn draw_scene(app: &mut FusionCraftApp, ui: &egui::Ui, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let w = (rect.width() * ppp).round().max(1.0) as usize;
    let h = (rect.height() * ppp).round().max(1.0) as usize;
    let busy = app.cam_anim.is_some() || crate::now() - app.cache.last_motion < 0.25 || ui.input(|i| i.pointer.any_down());
    let ssaa = if busy { 1 } else { 2 };
    let hover = app.cache_hover();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (w, h, app.model_version, app.view.mode as u8, app.view.grid && app.sketch.is_none()).hash(&mut hasher);
    for v in [app.cam.yaw, app.cam.pitch, app.cam.distance, app.cam.target.x, app.cam.target.y, app.cam.target.z] {
        hash_f(&mut hasher, v);
    }
    app.cam.ortho.hash(&mut hasher);
    app.selected_bodies.hash(&mut hasher);
    hover.hash(&mut hasher);
    let mut hidden: Vec<usize> = app.view.hidden_bodies.iter().copied().collect();
    hidden.sort_unstable();
    hidden.hash(&mut hasher);
    let key = hasher.finish();
    let need = key != app.cache.key || app.cache.texture.is_none() || (ssaa > app.cache.ssaa);
    if need {
        let model = app.preview.as_ref().unwrap_or(&app.engine.model);
        let scene = Scene {
            model,
            camera: &app.cam,
            style: Style::default(),
            mode: app.view.mode,
            selected: &app.selected_bodies,
            hover,
            hidden: &hidden,
            grid: app.view.grid && sketch_grid_hidden(app),
            lines: &[],
        };
        let frame = render(&scene, w, h, ssaa);
        let img = egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.rgba);
        match &mut app.cache.texture {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => app.cache.texture = Some(ui.ctx().load_texture("viewport", img, egui::TextureOptions::LINEAR)),
        }
        app.cache.frame = Some(frame);
        app.cache.key = key;
        app.cache.ssaa = ssaa;
    }
    if busy {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
    }
    if let Some(t) = &app.cache.texture {
        ui.painter().image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// The ground grid shows unless a sketch on another plane is open.
fn sketch_grid_hidden(app: &FusionCraftApp) -> bool {
    app.sketch.is_none()
}

impl FusionCraftApp {
    fn cache_hover(&self) -> Option<usize> {
        self.cache.frame.as_ref().and(self.hover_body)
    }
}

fn hover_body(app: &mut FusionCraftApp, view: View, p: Option<Pos2>) {
    let new = p.and_then(|p| {
        let f = app.cache.frame.as_ref()?;
        let ppp = f.width as f32 / view.rect.width().max(1.0);
        let pick = f.pick(((p.x - view.rect.left()) * ppp) as usize, ((p.y - view.rect.top()) * ppp) as usize)?;
        app.shown_model().bodies.get(pick.body).map(|b| b.index)
    });
    app.hover_body = new;
}

/// What a click on the canvas picks in "Create Sketch" mode.
#[derive(Clone)]
pub enum PlanePick {
    Origin(PlaneRef),
    Face(Plane),
}

fn origin_quads(view: View) -> [(PlaneRef, [Vec3; 4]); 3] {
    let s = view.cam.distance * 0.18;
    [
        (PlaneRef::Xy, [Vec3::ZERO, Vec3::new(s, 0.0, 0.0), Vec3::new(s, s, 0.0), Vec3::new(0.0, s, 0.0)]),
        (PlaneRef::Xz, [Vec3::ZERO, Vec3::new(s, 0.0, 0.0), Vec3::new(s, 0.0, s), Vec3::new(0.0, 0.0, s)]),
        (PlaneRef::Yz, [Vec3::ZERO, Vec3::new(0.0, s, 0.0), Vec3::new(0.0, s, s), Vec3::new(0.0, 0.0, s)]),
    ]
}

fn pick_plane(app: &FusionCraftApp, view: View, p: Option<Pos2>) -> Option<PlanePick> {
    let p = p?;
    let (o, d) = view.ray(p);
    let mut best: Option<(f64, PlanePick)> = None;
    let s = view.cam.distance * 0.18;
    for (r, _) in origin_quads(view) {
        let pl = r.plane();
        if let Some(hit) = pl.intersect_ray(o, d) {
            let in_quad = match r {
                PlaneRef::Xy => (0.0..=s).contains(&hit.x) && (0.0..=s).contains(&hit.y),
                PlaneRef::Xz => (0.0..=s).contains(&hit.x) && (0.0..=s).contains(&hit.z),
                PlaneRef::Yz => (0.0..=s).contains(&hit.y) && (0.0..=s).contains(&hit.z),
            };
            let t = (hit - o).length();
            if in_quad && best.as_ref().is_none_or(|(bt, _)| t < *bt) {
                best = Some((t, PlanePick::Origin(r)));
            }
        }
    }
    // A planar body face under the cursor.
    if let Some(f) = app.cache.frame.as_ref() {
        let ppp = f.width as f32 / view.rect.width().max(1.0);
        if let Some(pick) = f.pick(((p.x - view.rect.left()) * ppp) as usize, ((p.y - view.rect.top()) * ppp) as usize)
            && let Some(tri) = app.engine.model.bodies.get(pick.body).and_then(|b| b.mesh.tris.get(pick.triangle)) {
                let n = tri.normal();
                let tri_plane = Plane::new(tri.v[0], n, Vec3::X);
                if let Some(hit) = tri_plane.intersect_ray(o, d) {
                    let t = (hit - o).length();
                    if best.as_ref().is_none_or(|(bt, _)| t < *bt) {
                        let x_hint = if n.z.abs() > 0.9 { Vec3::X } else { Vec3::Z.cross(n) };
                        // Origin: the world origin projected onto the face plane.
                        let origin = Vec3::ZERO - n * tri_plane.signed_distance(Vec3::ZERO);
                        best = Some((t, PlanePick::Face(Plane::new(origin, n, x_hint))));
                    }
                }
            }
    }
    best.map(|(_, p)| p)
}

fn draw_origin(painter: &egui::Painter, view: View, picking: bool, hover: Option<&PlanePick>) {
    if picking {
        for (r, q) in origin_quads(view) {
            let pts: Vec<Pos2> = q.iter().filter_map(|p| view.to_screen(*p)).collect();
            if pts.len() != 4 {
                continue;
            }
            let hot = matches!(hover, Some(PlanePick::Origin(h)) if *h == r);
            let fill = if hot { theme::PLANE_HOVER } else { theme::PLANE };
            painter.add(Shape::convex_polygon(pts.clone(), fill, Stroke::new(1.0, theme::HOVER.linear_multiply(0.6))));
            if let Some(c) = view.to_screen((q[0] + q[2]) * 0.5) {
                painter.text(c, egui::Align2::CENTER_CENTER, r.label(), egui::FontId::proportional(13.0), theme::TEXT);
            }
        }
    }
    // Origin axes (short).
    let len = view.cam.distance * 0.08;
    if let Some(o) = view.to_screen(Vec3::ZERO) {
        for (a, c) in [(Vec3::X, Color32::from_rgb(0xd0, 0x4a, 0x4a)), (Vec3::Y, Color32::from_rgb(0x52, 0xb0, 0x5c)), (Vec3::Z, Color32::from_rgb(0x4a, 0x7c, 0xd8))] {
            if let Some(e) = view.to_screen(a * len) {
                painter.line_segment([o, e], Stroke::new(2.0, c));
            }
        }
        painter.circle_filled(o, 3.0, theme::TEXT);
    }
}

fn click(app: &mut FusionCraftApp, view: View, p: Option<Pos2>, plane: Option<PlanePick>, shift: bool) {
    let Some(p) = p else { return };
    if app.picking_plane {
        if let Some(pick) = plane {
            app.picking_plane = false;
            let sp = match pick {
                PlanePick::Origin(r) => SketchPlane::Origin { plane: r },
                PlanePick::Face(pl) => SketchPlane::Custom { plane: pl },
            };
            sketching::start(app, sp);
        }
        return;
    }
    // Profile picking for extrude / revolve.
    if let Some(sid) = app.dialog.as_ref().and_then(|d| d.sketch())
        && let Some(i) = profile_at(app, view, sid, p) {
            let n = fusioncraft_model::sketch_profiles(&app.engine.doc, sid).len();
            if let Some((_, profiles)) = app.dialog.as_mut().and_then(|d| d.profiles_mut()) {
                if profiles.is_empty() && !shift {
                    // From "all" to just this one.
                    profiles.push(i);
                } else if let Some(k) = profiles.iter().position(|x| *x == i) {
                    profiles.remove(k);
                } else {
                    profiles.push(i);
                    profiles.sort_unstable();
                }
                if profiles.len() == n {
                    profiles.clear();
                }
            }
            if let Some(d) = app.dialog.as_mut() {
                d.dirty = true;
            }
            return;
        }
    match app.hover_body {
        Some(b) => {
            if shift {
                if let Some(k) = app.selected_bodies.iter().position(|x| *x == b) {
                    app.selected_bodies.remove(k);
                } else {
                    app.selected_bodies.push(b);
                }
            } else {
                app.selected_bodies = vec![b];
            }
        }
        None => app.selected_bodies.clear(),
    }
}

fn profile_at(app: &FusionCraftApp, view: View, sketch: fusioncraft_doc::Id, p: Pos2) -> Option<usize> {
    let s = app.engine.doc.sketch(sketch)?;
    let plane = s.plane.plane();
    let local = view.on_plane(&plane, p)?;
    let profs = fusioncraft_model::sketch_profiles(&app.engine.doc, sketch);
    // The smallest region containing the point.
    profs
        .iter()
        .enumerate()
        .filter(|(_, pr)| pr.contains(local))
        .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
        .map(|(i, _)| i)
}

fn draw_profiles(app: &FusionCraftApp, painter: &egui::Painter, view: View, sketch: fusioncraft_doc::Id, hover: Option<Pos2>) {
    let Some(s) = app.engine.doc.sketch(sketch) else { return };
    let plane = s.plane.plane();
    let profs = fusioncraft_model::sketch_profiles(&app.engine.doc, sketch);
    let selected: Vec<usize> = app.dialog.as_ref().and_then(|d| match &d.kind {
        fusioncraft_doc::FeatureKind::Extrude { profiles, .. } | fusioncraft_doc::FeatureKind::Revolve { profiles, .. } => Some(profiles.clone()),
        _ => None,
    }).unwrap_or_default();
    let hot = hover.and_then(|p| profile_at(app, view, sketch, p));
    for (i, pr) in profs.iter().enumerate() {
        let on = selected.is_empty() || selected.contains(&i);
        let color = if hot == Some(i) { theme::PLANE_HOVER } else if on { theme::PROFILE_SELECTED } else { theme::PROFILE.linear_multiply(0.5) };
        fill_region(painter, view, &plane, pr, color);
    }
}

/// Fill a profile region (outer minus holes) by triangulating it with its holes bridged:
/// the holes are drawn by overpainting only where needed, so we triangulate outer and holes
/// separately and use an even-odd friendly approach: draw outer triangles that are not inside
/// a hole.
fn fill_region(painter: &egui::Painter, view: View, plane: &Plane, pr: &fusioncraft_doc::Profile, color: Color32) {
    let Ok(tris) = fusioncraft_geom::poly::triangulate(&pr.outer) else { return };
    let mut mesh = egui::Mesh::default();
    let pts: Vec<Option<Pos2>> = pr.outer.iter().map(|p| view.to_screen(plane.to_world(*p))).collect();
    for [a, b, c] in tris {
        let (Some(Some(pa)), Some(Some(pb)), Some(Some(pc))) = (pts.get(a), pts.get(b), pts.get(c)) else { continue };
        if !pr.holes.is_empty() {
            // Subdivide triangles that touch holes coarsely: skip those whose centroid is inside one.
            let (Some(la), Some(lb), Some(lc)) = (pr.outer.get(a), pr.outer.get(b), pr.outer.get(c)) else { continue };
            let cen = (*la + *lb + *lc) / 3.0;
            if pr.holes.iter().any(|h| fusioncraft_geom::poly::contains(h, cen)) {
                continue;
            }
            fill_with_holes(&mut mesh, view, plane, [*la, *lb, *lc], &pr.holes, color, 0);
            continue;
        }
        let i = mesh.vertices.len() as u32;
        for q in [pa, pb, pc] {
            mesh.colored_vertex(*q, color);
        }
        mesh.add_triangle(i, i + 1, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// Split a triangle into four until pieces are clear of the holes (bounded depth).
fn fill_with_holes(mesh: &mut egui::Mesh, view: View, plane: &Plane, t: [Vec2; 3], holes: &[Vec<Vec2>], color: Color32, depth: u32) {
    let cen = (t[0] + t[1] + t[2]) / 3.0;
    let touches = holes.iter().any(|h| h.iter().any(|p| fusioncraft_geom::poly::contains(&t, *p)) || t.iter().any(|p| fusioncraft_geom::poly::contains(h, *p)));
    if !touches || depth >= 5 {
        if holes.iter().any(|h| fusioncraft_geom::poly::contains(h, cen)) {
            return;
        }
        let pts: Vec<Pos2> = t.iter().filter_map(|p| view.to_screen(plane.to_world(*p))).collect();
        if pts.len() == 3 {
            let i = mesh.vertices.len() as u32;
            for q in pts {
                mesh.colored_vertex(q, color);
            }
            mesh.add_triangle(i, i + 1, i + 2);
        }
        return;
    }
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, bc, ca) = ((a + b) * 0.5, (b + c) * 0.5, (c + a) * 0.5);
    for s in [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]] {
        fill_with_holes(mesh, view, plane, s, holes, color, depth + 1);
    }
}

fn hint(app: &FusionCraftApp, painter: &egui::Painter, rect: Rect) {
    let text = if app.picking_plane {
        Some("Select a plane or a planar face to sketch on. Esc cancels.".to_owned())
    } else if let Some(s) = &app.sketch {
        Some(sketching::hint(s))
    } else if app.dialog.as_ref().is_some_and(|d| d.sketch().is_some()) {
        Some("Click profiles to choose them (Shift adds). OK or Enter applies.".to_owned())
    } else if app.engine.doc.timeline.is_empty() {
        Some("Start with Create Sketch (S) or a Box, Cylinder or Sphere from CREATE.".to_owned())
    } else {
        None
    };
    if let Some(t) = text {
        let pos = pos2(rect.center().x, rect.top() + 16.0);
        let galley = painter.layout_no_wrap(t, egui::FontId::proportional(13.0), theme::TEXT);
        let r = Rect::from_center_size(pos + vec2(0.0, galley.size().y * 0.5), galley.size() + vec2(20.0, 10.0));
        painter.rect_filled(r, 8.0, theme::CHROME_DARK.linear_multiply(0.92));
        painter.galley(r.min + vec2(10.0, 5.0), galley, theme::TEXT);
    }
}

/// The ViewCube in the top-right corner: click a face to look at it.
fn view_cube(app: &mut FusionCraftApp, ui: &mut egui::Ui, rect: Rect) {
    let size = 46.0;
    let center = pos2(rect.right() - 70.0, rect.top() + 70.0);
    let area = Rect::from_center_size(center, vec2(size * 2.6, size * 2.6));
    let resp = ui.interact(area, egui::Id::unique("viewcube"), Sense::click_and_drag());
    let painter = ui.painter_at(area.expand(20.0));
    let cam = app.cam;
    let (r, u, f) = (cam.right(), cam.up(), cam.forward());
    let proj = |v: Vec3| center + vec2(v.dot(r) as f32, -v.dot(u) as f32) * size;
    let faces: [(Vec3, StdView, &str); 6] = [
        (Vec3::Z, StdView::Top, "TOP"),
        (-Vec3::Z, StdView::Bottom, "BOTTOM"),
        (-Vec3::Y, StdView::Front, "FRONT"),
        (Vec3::Y, StdView::Back, "BACK"),
        (Vec3::X, StdView::Right, "RIGHT"),
        (-Vec3::X, StdView::Left, "LEFT"),
    ];
    let hover = resp.hover_pos();
    let mut clicked_view = None;
    let mut drawn: Vec<(f64, Vec<Pos2>, &str, StdView, Vec3)> = Vec::new();
    for (n, v, label) in faces {
        if n.dot(f) >= -1e-6 {
            continue;
        }
        let a = n.any_perpendicular();
        let b = n.cross(a);
        let quad: Vec<Pos2> = [a + b, b - a, -a - b, a - b].iter().map(|q| proj((n + *q) * 0.62)).collect();
        drawn.push((n.dot(f), quad, label, v, n));
    }
    drawn.sort_by(|x, y| y.0.total_cmp(&x.0));
    for (_, quad, label, v, n) in &drawn {
        let inside = hover.is_some_and(|h| point_in_quad(h, quad));
        let shade = (0.55 + 0.45 * (-n.dot(f)) as f32).clamp(0.0, 1.0);
        let base = Color32::from_rgb((0x5c as f32 * shade) as u8 + 20, (0x67 as f32 * shade) as u8 + 20, (0x7c as f32 * shade) as u8 + 24);
        let fill = if inside { theme::ACCENT } else { base };
        painter.add(Shape::convex_polygon(quad.clone(), fill, Stroke::new(1.0, theme::BORDER)));
        let c = quad.iter().fold(egui::Vec2::ZERO, |acc, p| acc + p.to_vec2()) / 4.0;
        let font = egui::FontId::proportional(if label.len() > 5 { 9.0 } else { 10.5 });
        if -n.dot(f) > 0.25 {
            painter.text(c.to_pos2(), egui::Align2::CENTER_CENTER, *label, font, theme::TEXT);
        }
        if inside && resp.clicked() {
            clicked_view = Some(*v);
        }
    }
    // Axis triad below the cube.
    for (a, c) in [(Vec3::X, Color32::from_rgb(0xd0, 0x4a, 0x4a)), (Vec3::Y, Color32::from_rgb(0x52, 0xb0, 0x5c)), (Vec3::Z, Color32::from_rgb(0x4a, 0x7c, 0xd8))] {
        let o = proj(Vec3::new(-0.62, -0.62, -0.62));
        let e = proj(Vec3::new(-0.62, -0.62, -0.62) + a * 0.5);
        painter.line_segment([o, e], Stroke::new(2.0, c));
    }
    // Home button.
    let home = Rect::from_center_size(pos2(area.left() + 4.0, area.top() + 4.0), vec2(20.0, 20.0));
    let hr = ui.interact(home, egui::Id::unique("cube-home"), Sense::click());
    crate::icons::paint(&ui.painter_at(home.expand(2.0)), home.shrink(2.0), crate::icons::Icon::Home, if hr.hovered() { theme::ACCENT } else { theme::ICON });
    if hr.on_hover_text("Home view").clicked() {
        app.set_view(StdView::Home);
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        app.cam_anim = None;
        app.cam.orbit(-f64::from(d.x) * 0.012, f64::from(d.y) * 0.012);
        app.cache.last_motion = crate::now();
    } else if let Some(v) = clicked_view {
        app.set_view(v);
    }
}

fn point_in_quad(p: Pos2, q: &[Pos2]) -> bool {
    let n = q.len();
    let mut inside = false;
    for i in 0..n {
        let (Some(a), Some(b)) = (q.get(i), q.get((i + n - 1) % n)) else { continue };
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Navigation bar at the bottom of the canvas.
fn nav_bar(app: &mut FusionCraftApp, ui: &mut egui::Ui, rect: Rect) {
    use crate::icons::Icon;
    let w = 230.0;
    let bar = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 24.0), vec2(w, 34.0));
    ui.painter().rect_filled(bar, 10.0, theme::CHROME_DARK.linear_multiply(0.95));
    let mut x = bar.left() + 8.0;
    let mut button = |ui: &mut egui::Ui, icon: Icon, tip: &str, on: bool| -> bool {
        let r = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(26.0, 26.0));
        x += 30.0;
        let resp = ui.interact(r, egui::Id::unique(("nav", tip)), Sense::click());
        if resp.hovered() || on {
            ui.painter().rect_filled(r, 6.0, if on { theme::ACCENT.linear_multiply(0.5) } else { theme::CONTROL_HOVER });
        }
        crate::icons::paint(ui.painter(), r.shrink(5.0), icon, theme::ICON);
        resp.on_hover_text(tip).clicked()
    };
    if button(ui, Icon::Home, "Home view (Home)", false) {
        app.set_view(StdView::Home);
    }
    if button(ui, Icon::Fit, "Fit (F6)", false) {
        app.fit_all(true);
    }
    if button(ui, Icon::Orbit, "Orbit: right drag or Shift + middle drag", false) {
        app.set_status("Orbit: right mouse drag or Shift + middle mouse drag. Pan: middle drag. Zoom: wheel.", false);
    }
    let modes = [(DisplayMode::ShadedWithEdges, "Shaded with visible edges"), (DisplayMode::Shaded, "Shaded"), (DisplayMode::Wireframe, "Wireframe")];
    if button(ui, Icon::Display, "Display style (click to change)", false) {
        let i = modes.iter().position(|(m, _)| *m == app.view.mode).unwrap_or(0);
        let next = modes.get((i + 1) % modes.len()).copied().unwrap_or(modes[0]);
        app.view.mode = next.0;
        app.set_status(next.1, false);
    }
    let grid = app.view.grid;
    if button(ui, Icon::Grid, "Layout grid", grid) {
        app.view.grid = !app.view.grid;
    }
    let ortho = app.cam.ortho;
    if button(ui, Icon::Perspective, if ortho { "Orthographic (click for perspective)" } else { "Perspective (click for orthographic)" }, ortho) {
        app.cam.ortho = !app.cam.ortho;
    }
    if button(ui, Icon::Properties, "Properties of the design", false)
        && let Some(v) = app.run("model.info", &json!({})) {
            let vol = v["volume"].as_f64().unwrap_or(0.0);
            let n = v["bodies"].as_array().map_or(0, Vec::len);
            app.set_status(format!("{n} bodies, volume {:.2} cm³ ({:.1} mm³)", vol / 1000.0, vol), false);
        }
}
