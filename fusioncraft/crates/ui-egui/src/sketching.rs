//! Sketch mode: drawing tools, snapping, typed dimensions and the sketch palette.

use crate::viewport::View;
use crate::{theme, FusionCraftApp};
use egui::{pos2, vec2, Color32, Pos2, RichText, Stroke};
use fusioncraft_doc::{Entity, Expr, Id, SketchPlane};
use fusioncraft_geom::{Plane, Vec2};
use fusioncraft_render::grid_step;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    Rect,
    Circle,
    Polygon,
    Arc,
    Point,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line",
            Tool::Rect => "2-Point Rectangle",
            Tool::Circle => "Center Diameter Circle",
            Tool::Polygon => "Polygon",
            Tool::Arc => "3-Point Arc",
            Tool::Point => "Point",
        }
    }
}

pub struct SketchSession {
    pub id: Id,
    pub tool: Tool,
    pub clicks: Vec<Vec2>,
    pub sides: u32,
    pub construction: bool,
    pub snap_grid: bool,
    /// Digits typed while drawing (exact length/diameter/size).
    pub typed: String,
    pub cursor: Option<Vec2>,
    pub snapped: bool,
    pub selected: Option<usize>,
    pub hover: Option<usize>,
}

pub fn start(app: &mut FusionCraftApp, plane: SketchPlane) {
    let params = match &plane {
        SketchPlane::Origin { plane } => json!({ "plane": plane.label().to_ascii_lowercase() }),
        SketchPlane::Custom { plane } => json!({ "origin": plane.origin.to_array(), "normal": plane.normal.to_array(), "x_axis": plane.x_axis.to_array() }),
    };
    if let Some(v) = app.run("sketch.create", &params)
        && let Some(id) = v["sketch"].as_u64() {
            enter(app, id);
        }
}

pub fn edit(app: &mut FusionCraftApp, id: Id) {
    app.dialog = None;
    app.preview = None;
    enter(app, id);
}

fn enter(app: &mut FusionCraftApp, id: Id) {
    let Some(s) = app.engine.doc.sketch(id) else { return };
    let plane = s.plane.plane();
    app.status = None;
    app.engine.active_sketch = Some(id);
    app.sketch = Some(SketchSession {
        id,
        tool: Tool::Line,
        clicks: Vec::new(),
        sides: 6,
        construction: false,
        snap_grid: true,
        typed: String::new(),
        cursor: None,
        snapped: false,
        selected: None,
        hover: None,
    });
    let mut to = app.cam;
    to.look_along(plane.normal);
    to.target = plane.to_world(plane.to_local(app.cam.target));
    app.animate_to(to);
}

pub fn finish(app: &mut FusionCraftApp) {
    app.sketch = None;
    app.engine.active_sketch = None;
    let mut to = app.cam;
    to.set_view(fusioncraft_render::StdView::Home);
    app.animate_to(to);
}

pub fn hint(s: &SketchSession) -> String {
    let typed = if s.typed.is_empty() { String::new() } else { format!("   Typed: {}", s.typed) };
    let what = match (s.tool, s.clicks.len()) {
        (Tool::Select, _) => "Click a curve to select it; Delete removes it",
        (Tool::Line, 0) => "Line: click the start point",
        (Tool::Line, _) => "Line: click the next point (type a length + Enter). Esc ends the chain",
        (Tool::Rect, 0) => "Rectangle: click the first corner",
        (Tool::Rect, _) => "Rectangle: click the opposite corner (type 30x20 + Enter)",
        (Tool::Circle, 0) => "Circle: click the centre",
        (Tool::Circle, _) => "Circle: click to set the size (type a diameter + Enter)",
        (Tool::Polygon, 0) => "Polygon: click the centre",
        (Tool::Polygon, _) => "Polygon: click a corner (type a radius + Enter)",
        (Tool::Arc, 0) => "Arc: click the start point",
        (Tool::Arc, 1) => "Arc: click the end point",
        (Tool::Arc, _) => "Arc: click a point on the arc",
        (Tool::Point, _) => "Point: click to place",
    };
    format!("{what}{typed}")
}

fn session_plane(app: &FusionCraftApp) -> Option<(Plane, Id)> {
    let s = app.sketch.as_ref()?;
    let sk = app.engine.doc.sketch(s.id)?;
    Some((sk.plane.plane(), s.id))
}

/// Snap a plane point: sketch points first (within 10 px), then the grid.
fn snap(app: &FusionCraftApp, view: View, plane: &Plane, raw: Vec2) -> (Vec2, bool) {
    let Some(s) = &app.sketch else { return (raw, false) };
    let tol = view.units_per_point() * 10.0;
    let mut best: Option<(f64, Vec2)> = None;
    let mut consider = |p: Vec2| {
        let d = p.distance(raw);
        if d < tol && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, p));
        }
    };
    consider(Vec2::ZERO);
    if let Some(sk) = app.engine.doc.sketch(s.id) {
        for e in &sk.entities {
            for p in e.snap_points() {
                consider(p);
            }
        }
    }
    for p in &s.clicks {
        consider(*p);
    }
    // The origin of the world projected on a face plane.
    consider(plane.to_local(fusioncraft_geom::Vec3::ZERO));
    if let Some((_, p)) = best {
        return (p, true);
    }
    if s.snap_grid {
        let step = grid_step(view.cam.distance) / 2.0;
        if step > 0.0 {
            return (Vec2::new((raw.x / step).round() * step, (raw.y / step).round() * step), false);
        }
    }
    (raw, false)
}

fn line_params(sketch: Id, a: Vec2, b: Vec2, construction: bool) -> Value {
    json!({ "sketch": sketch, "a": [a.x, a.y], "b": [b.x, b.y], "construction": construction })
}

/// Commit the current tool with the point `p`. Returns true when something was added.
fn place(app: &mut FusionCraftApp, p: Vec2) {
    let Some(s) = app.sketch.as_mut() else { return };
    let id = s.id;
    let construction = s.construction;
    s.typed.clear();
    match s.tool {
        Tool::Select => {}
        Tool::Point => {
            app.run("sketch.point", &json!({ "sketch": id, "p": [p.x, p.y] }));
        }
        Tool::Line => {
            if let Some(&last) = s.clicks.last() {
                if last.distance(p) < 1e-9 {
                    return;
                }
                let first = s.clicks.first().copied();
                let closes = first.is_some_and(|f| f.distance(p) < 1e-9);
                s.clicks = if closes { Vec::new() } else { vec![p] };
                // Remember the chain start for closing.
                if !closes
                    && let Some(f) = first {
                        s.clicks.insert(0, f);
                        s.clicks.truncate(1);
                        s.clicks.push(p);
                    }
                app.run("sketch.line", &line_params(id, last, p, construction));
            } else {
                s.clicks.push(p);
            }
        }
        Tool::Rect => {
            if let Some(&a) = s.clicks.first() {
                s.clicks.clear();
                if (a.x - p.x).abs() > 1e-9 && (a.y - p.y).abs() > 1e-9 {
                    if construction {
                        let c = [a, Vec2::new(p.x, a.y), p, Vec2::new(a.x, p.y)];
                        for k in 0..4 {
                            app.run("sketch.line", &line_params(id, c[k], c[(k + 1) % 4], true));
                        }
                    } else {
                        app.run("sketch.rect", &json!({ "sketch": id, "a": [a.x, a.y], "b": [p.x, p.y] }));
                    }
                }
            } else {
                s.clicks.push(p);
            }
        }
        Tool::Circle | Tool::Polygon => {
            if let Some(&c) = s.clicks.first() {
                s.clicks.clear();
                let r = c.distance(p);
                if r > 1e-9 {
                    if s.tool == Tool::Circle {
                        app.run("sketch.circle", &json!({ "sketch": id, "center": [c.x, c.y], "radius": r }));
                    } else {
                        let rot = (p.y - c.y).atan2(p.x - c.x).to_degrees() - 90.0;
                        let sides = s.sides;
                        app.run("sketch.polygon", &json!({ "sketch": id, "center": [c.x, c.y], "radius": r, "sides": sides, "rotation": rot }));
                    }
                }
            } else {
                s.clicks.push(p);
            }
        }
        Tool::Arc => {
            if s.clicks.len() >= 2 {
                let (a, b) = (s.clicks[0], s.clicks[1]);
                s.clicks.clear();
                app.run("sketch.arc", &json!({ "sketch": id, "start": [a.x, a.y], "mid": [p.x, p.y], "end": [b.x, b.y] }));
            } else {
                s.clicks.push(p);
            }
        }
    }
}

/// Apply typed digits: exact length, diameter, radius or "w x h".
fn apply_typed(app: &mut FusionCraftApp) {
    let params = app.engine.doc.params();
    let Some(s) = app.sketch.as_ref() else { return };
    let typed = s.typed.clone();
    let cursor = s.cursor.unwrap_or(Vec2::new(1.0, 0.0));
    let eval = |t: &str| params.eval(&Expr::new(t.trim())).ok();
    let target = match (s.tool, s.clicks.last().copied()) {
        (Tool::Line, Some(last)) => eval(&typed).map(|len| {
            let dir = (cursor - last).normalized().unwrap_or(Vec2::new(1.0, 0.0));
            last + dir * len
        }),
        (Tool::Circle, Some(c)) => eval(&typed).map(|d| c + Vec2::new(d / 2.0, 0.0)),
        (Tool::Polygon, Some(c)) => eval(&typed).map(|r| c + (cursor - c).normalized().unwrap_or(Vec2::new(0.0, 1.0)) * r),
        (Tool::Rect, Some(a)) => {
            let parts: Vec<&str> = typed.split(['x', 'X', '*', ';']).collect();
            let (w, h) = match parts.as_slice() {
                [w, h] => (eval(w), eval(h)),
                [w] => (eval(w), eval(w)),
                _ => (None, None),
            };
            w.zip(h).map(|(w, h)| {
                let sx = if cursor.x >= a.x { 1.0 } else { -1.0 };
                let sy = if cursor.y >= a.y { 1.0 } else { -1.0 };
                a + Vec2::new(w * sx, h * sy)
            })
        }
        _ => None,
    };
    match target {
        Some(p) => place(app, p),
        None => app.set_status(format!("'{typed}' is not a size"), true),
    }
    if let Some(s) = app.sketch.as_mut() {
        s.typed.clear();
    }
}

pub fn viewport_input(app: &mut FusionCraftApp, ui: &egui::Ui, resp: &egui::Response, painter: &egui::Painter, view: View) {
    let Some((plane, sid)) = session_plane(app) else {
        app.sketch = None;
        return;
    };
    // Keyboard: tools, typed sizes, Esc, Delete.
    let mut typed_enter = false;
    let mut esc = false;
    let mut delete = false;
    let mut tool_key = None;
    let mut toggle_construction = false;
    if !ui.ctx().egui_wants_keyboard_input() {
        ui.input(|i| {
            for ev in &i.events {
                match ev {
                    egui::Event::Text(t) => {
                        for ch in t.chars() {
                            let drawing = app.sketch.as_ref().is_some_and(|s| !s.clicks.is_empty());
                            if drawing && (ch.is_ascii_digit() || ".,-+*/()x;".contains(ch) || (ch.is_alphabetic() && app.sketch.as_ref().is_some_and(|s| !s.typed.is_empty()))) {
                                if let Some(s) = app.sketch.as_mut() {
                                    s.typed.push(ch);
                                }
                            } else {
                                match ch.to_ascii_lowercase() {
                                    'l' => tool_key = Some(Tool::Line),
                                    'r' => tool_key = Some(Tool::Rect),
                                    'c' => tool_key = Some(Tool::Circle),
                                    'p' => tool_key = Some(Tool::Polygon),
                                    'a' => tool_key = Some(Tool::Arc),
                                    'x' => toggle_construction = true,
                                    _ => {}
                                }
                            }
                        }
                    }
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => typed_enter = true,
                    egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => esc = true,
                    egui::Event::Key { key: egui::Key::Delete | egui::Key::Backspace, pressed: true, .. } => delete = true,
                    _ => {}
                }
            }
        });
    }
    if let Some(t) = tool_key {
        set_tool(app, t);
    }
    if toggle_construction
        && let Some(s) = app.sketch.as_mut() {
            s.construction = !s.construction;
        }
    if delete {
        let backspace_typed = app.sketch.as_mut().is_some_and(|s| s.typed.pop().is_some());
        if !backspace_typed
            && let Some(i) = app.sketch.as_mut().and_then(|s| s.selected.take()) {
                app.run("sketch.delete_entity", &json!({ "sketch": sid, "index": i }));
            }
    }
    if typed_enter && app.sketch.as_ref().is_some_and(|s| !s.typed.is_empty()) {
        apply_typed(app);
    }
    if esc {
        let busy = app.sketch.as_ref().is_some_and(|s| !s.clicks.is_empty() || !s.typed.is_empty());
        if busy {
            if let Some(s) = app.sketch.as_mut() {
                s.clicks.clear();
                s.typed.clear();
            }
        } else if app.sketch.as_ref().is_some_and(|s| s.tool != Tool::Select) {
            set_tool(app, Tool::Select);
        } else {
            finish(app);
            return;
        }
    }

    // Cursor on the plane.
    let raw = resp.hover_pos().and_then(|p| view.on_plane(&plane, p));
    let snapped = raw.map(|r| snap(app, view, &plane, r));
    let hover_entity = raw.and_then(|r| {
        let sk = app.engine.doc.sketch(sid)?;
        let tol = view.units_per_point() * 6.0;
        sk.entities
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.distance_to(r)))
            .filter(|(_, d)| *d < tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    if let Some(s) = app.sketch.as_mut() {
        s.cursor = snapped.map(|(p, _)| p);
        s.snapped = snapped.is_some_and(|(_, k)| k);
        s.hover = if s.tool == Tool::Select { hover_entity } else { None };
    }
    if resp.clicked() {
        let tool = app.sketch.as_ref().map(|s| s.tool);
        if tool == Some(Tool::Select) {
            if let Some(s) = app.sketch.as_mut() {
                s.selected = hover_entity;
            }
        } else if let Some((p, _)) = snapped {
            place(app, p);
        }
    }
    if resp.secondary_clicked()
        && let Some(s) = app.sketch.as_mut() {
            s.clicks.clear();
        }
    draw_grid(painter, view, &plane);
    draw_preview(app, painter, view, &plane);
}

pub fn set_tool(app: &mut FusionCraftApp, t: Tool) {
    if let Some(s) = app.sketch.as_mut() {
        s.tool = t;
        s.clicks.clear();
        s.typed.clear();
        s.selected = None;
    }
}

fn draw_grid(painter: &egui::Painter, view: View, plane: &Plane) {
    let step = grid_step(view.cam.distance);
    let r = view.rect;
    let corners: Vec<Vec2> = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].iter().filter_map(|p| view.on_plane(plane, *p)).collect();
    if corners.len() < 4 {
        return;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for c in &corners {
        x0 = x0.min(c.x);
        x1 = x1.max(c.x);
        y0 = y0.min(c.y);
        y1 = y1.max(c.y);
    }
    let (i0, i1) = ((x0 / step).floor() as i64, (x1 / step).ceil() as i64);
    let (j0, j1) = ((y0 / step).floor() as i64, (y1 / step).ceil() as i64);
    if (i1 - i0) > 400 || (j1 - j0) > 400 {
        return;
    }
    let minor = Color32::from_rgba_unmultiplied(0x5a, 0x66, 0x76, 50);
    let major = Color32::from_rgba_unmultiplied(0x6a, 0x78, 0x8a, 95);
    let seg = |a: Vec2, b: Vec2| -> Option<[Pos2; 2]> { Some([view.to_screen(plane.to_world(a))?, view.to_screen(plane.to_world(b))?]) };
    for i in i0..=i1 {
        let x = i as f64 * step;
        let c = if i == 0 { theme::ERROR.linear_multiply(0.5) } else if i % 10 == 0 { major } else { minor };
        if let Some(s) = seg(Vec2::new(x, y0), Vec2::new(x, y1)) {
            painter.line_segment(s, Stroke::new(1.0, c));
        }
    }
    for j in j0..=j1 {
        let y = j as f64 * step;
        let c = if j == 0 { theme::SNAP.linear_multiply(0.5) } else if j % 10 == 0 { major } else { minor };
        if let Some(s) = seg(Vec2::new(x0, y), Vec2::new(x1, y)) {
            painter.line_segment(s, Stroke::new(1.0, c));
        }
    }
}

fn fmt(v: f64) -> String {
    fusioncraft_doc::expr::format_number((v * 100.0).round() / 100.0)
}

fn label(painter: &egui::Painter, at: Pos2, text: String) {
    let galley = painter.layout_no_wrap(text, egui::FontId::proportional(12.0), theme::TEXT);
    let r = egui::Rect::from_min_size(at + vec2(14.0, 10.0), galley.size() + vec2(10.0, 6.0));
    painter.rect_filled(r, 5.0, theme::CHROME_DARK);
    painter.galley(r.min + vec2(5.0, 3.0), galley, theme::TEXT);
}

fn draw_preview(app: &FusionCraftApp, painter: &egui::Painter, view: View, plane: &Plane) {
    let Some(s) = &app.sketch else { return };
    let Some(cur) = s.cursor else { return };
    let color = if s.construction { theme::CONSTRUCTION } else { theme::ACCENT };
    let stroke = Stroke::new(1.8, color);
    let poly = |pts: &[Vec2]| view.plane_polyline(plane, pts);
    if let Some(c) = view.to_screen(plane.to_world(cur)) {
        if s.snapped {
            painter.rect_stroke(egui::Rect::from_center_size(c, vec2(10.0, 10.0)), 1.0, Stroke::new(1.5, theme::SNAP), egui::StrokeKind::Middle);
        } else if s.tool != Tool::Select {
            painter.circle_stroke(c, 3.5, Stroke::new(1.2, theme::TEXT));
        }
        let typed = if s.typed.is_empty() { String::new() } else { format!("  [{}]", s.typed) };
        let dim = match (s.tool, s.clicks.last()) {
            (Tool::Line, Some(last)) => {
                let d = cur - *last;
                Some(format!("{} mm  {}°{typed}", fmt(d.length()), fmt(d.y.atan2(d.x).to_degrees())))
            }
            (Tool::Rect, Some(a)) => Some(format!("{} × {} mm{typed}", fmt((cur.x - a.x).abs()), fmt((cur.y - a.y).abs()))),
            (Tool::Circle, Some(a)) => Some(format!("Ø {} mm{typed}", fmt(a.distance(cur) * 2.0))),
            (Tool::Polygon, Some(a)) => Some(format!("R {} mm, {} sides{typed}", fmt(a.distance(cur)), s.sides)),
            (Tool::Select, _) => None,
            _ => Some(format!("{}, {}", fmt(cur.x), fmt(cur.y))),
        };
        if let Some(t) = dim {
            label(painter, c, t);
        }
    }
    match (s.tool, s.clicks.as_slice()) {
        (Tool::Line, [.., last]) => {
            painter.add(egui::Shape::line(poly(&[*last, cur]), stroke));
        }
        (Tool::Rect, [a]) => {
            let pts = [*a, Vec2::new(cur.x, a.y), cur, Vec2::new(a.x, cur.y), *a];
            painter.add(egui::Shape::line(poly(&pts), stroke));
        }
        (Tool::Circle, [c]) => {
            let pts = fusioncraft_geom::poly::circle(*c, c.distance(cur).max(1e-9), 96);
            let mut p = poly(&pts);
            if let Some(f) = p.first().copied() {
                p.push(f);
            }
            painter.add(egui::Shape::line(p, stroke));
        }
        (Tool::Polygon, [c]) => {
            let e = Entity::Polygon { center: *c, radius: c.distance(cur), sides: s.sides, rotation: (cur.y - c.y).atan2(cur.x - c.x).to_degrees() - 90.0 };
            painter.add(egui::Shape::line(poly(&e.display_points()), stroke));
        }
        (Tool::Arc, [a]) => {
            painter.add(egui::Shape::dashed_line(&poly(&[*a, cur]), stroke, 6.0, 4.0));
        }
        (Tool::Arc, [a, b, ..]) => {
            let pts = fusioncraft_doc::sketch::arc_points(*a, cur, *b);
            painter.add(egui::Shape::line(poly(&pts), stroke));
        }
        _ => {}
    }
    for p in &s.clicks {
        if let Some(q) = view.to_screen(plane.to_world(*p)) {
            painter.circle_filled(q, 3.0, color);
        }
    }
}

/// Draw every visible sketch of the active timeline (and the open one brighter).
pub fn draw_sketches(app: &FusionCraftApp, painter: &egui::Painter, view: View) {
    let doc = &app.engine.doc;
    let active: Vec<Id> = doc
        .timeline
        .iter()
        .take(doc.active_len())
        .filter(|f| !f.suppressed)
        .filter_map(|f| f.kind.sketch_id().filter(|_| matches!(f.kind, fusioncraft_doc::FeatureKind::Sketch { .. })))
        .collect();
    let open = app.sketch.as_ref().map(|s| s.id);
    let consumed: Vec<Id> = doc.timeline.iter().take(doc.active_len()).filter_map(|f| match f.kind {
        fusioncraft_doc::FeatureKind::Extrude { sketch, .. } | fusioncraft_doc::FeatureKind::Revolve { sketch, .. } => Some(sketch),
        _ => None,
    }).collect();
    for s in &doc.sketches {
        let is_open = open == Some(s.id);
        if !is_open && (!s.visible || !active.contains(&s.id)) {
            continue;
        }
        // Like Fusion, a sketch used by a feature fades once the body exists.
        let faded = !is_open && consumed.contains(&s.id);
        let plane = s.plane.plane();
        let session = app.sketch.as_ref().filter(|x| x.id == s.id);
        for (i, e) in s.entities.iter().enumerate() {
            let construction = matches!(e, Entity::Line { construction: true, .. });
            let mut color = if construction { theme::CONSTRUCTION } else if is_open { theme::SKETCH_ACTIVE } else { theme::SKETCH };
            if faded {
                color = color.linear_multiply(0.35);
            }
            if session.is_some_and(|x| x.selected == Some(i)) {
                color = theme::ACCENT;
            } else if session.is_some_and(|x| x.hover == Some(i)) {
                color = theme::HOVER;
            }
            let width = if is_open { 1.8 } else { 1.3 };
            let pts = view.plane_polyline(&plane, &e.display_points());
            if let Entity::Point { .. } = e {
                if let Some(p) = pts.first() {
                    painter.circle_filled(*p, 3.0, color);
                }
                continue;
            }
            if construction {
                painter.add(egui::Shape::dashed_line(&pts, Stroke::new(width, color), 7.0, 4.0));
            } else {
                painter.add(egui::Shape::line(pts, Stroke::new(width, color)));
            }
            if is_open {
                for p in e.snap_points().iter().take(8) {
                    if let Some(q) = view.to_screen(plane.to_world(*p)) {
                        painter.circle_filled(q, 2.0, color.linear_multiply(0.8));
                    }
                }
            }
        }
    }
}

/// The SKETCH PALETTE at the right while a sketch is open.
pub fn palette(app: &mut FusionCraftApp, ui: &mut egui::Ui) {
    let Some(s) = app.sketch.as_ref() else { return };
    let sid = s.id;
    let sel = s.selected;
    let mut construction = s.construction;
    let mut snap = s.snap_grid;
    let mut sides = s.sides;
    let tool = s.tool;
    let entity = sel.and_then(|i| app.engine.doc.sketch(sid).and_then(|sk| sk.entities.get(i)).cloned());
    let name = app.engine.doc.sketch(sid).map(|s| s.name.clone()).unwrap_or_default();
    let count = app.engine.doc.sketch(sid).map_or(0, |s| s.entities.len());
    let profiles = fusioncraft_model::sketch_profiles(&app.engine.doc, sid).len();
    let mut finish_now = false;
    let mut edited: Option<Entity> = None;
    let mut delete = false;
    egui::Window::new(RichText::new("SKETCH PALETTE").strong())
        .id(egui::Id::unique("sketch-palette"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 92.0))
        .resizable(false)
        .collapsible(true)
        .show(ui.ctx(), |ui| {
            ui.set_min_width(260.0);
            ui.label(RichText::new(format!("{name}: {count} curves, {profiles} profiles")).color(theme::TEXT_DIM));
            ui.label(RichText::new(format!("Tool: {}", tool.label())).color(theme::TEXT_DIM));
            ui.checkbox(&mut construction, "Construction (X)");
            ui.checkbox(&mut snap, "Snap to grid");
            if tool == Tool::Polygon {
                ui.horizontal(|ui| {
                    ui.label("Sides");
                    ui.add(egui::DragValue::new(&mut sides).range(3..=64));
                });
            }
            if let Some(mut e) = entity {
                ui.separator();
                ui.label(RichText::new(format!("Selected: {}", e.kind())).strong());
                let mut changed = false;
                egui::Grid::new("ent").num_columns(2).show(ui, |ui| {
                    let mut v2 = |ui: &mut egui::Ui, l: &str, v: &mut Vec2| {
                        ui.label(l);
                        ui.horizontal(|ui| {
                            changed |= ui.add(egui::DragValue::new(&mut v.x).speed(0.1).max_decimals(3)).changed();
                            changed |= ui.add(egui::DragValue::new(&mut v.y).speed(0.1).max_decimals(3)).changed();
                        });
                        ui.end_row();
                    };
                    match &mut e {
                        Entity::Line { a, b, construction } => {
                            v2(ui, "Start", a);
                            v2(ui, "End", b);
                            ui.label("Construction");
                            changed |= ui.checkbox(construction, "").changed();
                            ui.end_row();
                        }
                        Entity::Rect { a, b } => {
                            v2(ui, "Corner 1", a);
                            v2(ui, "Corner 2", b);
                        }
                        Entity::Circle { center, radius } => {
                            v2(ui, "Centre", center);
                            ui.label("Diameter");
                            let mut d = *radius * 2.0;
                            if ui.add(egui::DragValue::new(&mut d).speed(0.1).range(0.001..=1e6).max_decimals(3)).changed() {
                                *radius = d / 2.0;
                                changed = true;
                            }
                            ui.end_row();
                        }
                        Entity::Polygon { center, radius, sides, rotation } => {
                            v2(ui, "Centre", center);
                            ui.label("Radius");
                            changed |= ui.add(egui::DragValue::new(radius).speed(0.1).range(0.001..=1e6)).changed();
                            ui.end_row();
                            ui.label("Sides");
                            changed |= ui.add(egui::DragValue::new(sides).range(3..=64)).changed();
                            ui.end_row();
                            ui.label("Rotation");
                            changed |= ui.add(egui::DragValue::new(rotation).speed(1.0).suffix("°")).changed();
                            ui.end_row();
                        }
                        Entity::Arc { start, mid, end } => {
                            v2(ui, "Start", start);
                            v2(ui, "Through", mid);
                            v2(ui, "End", end);
                        }
                        Entity::Point { p } => v2(ui, "Position", p),
                    }
                });
                if changed {
                    edited = Some(e);
                }
                if ui.button("Delete curve").clicked() {
                    delete = true;
                }
            }
            ui.separator();
            let b = egui::Button::new(RichText::new("Finish Sketch").color(Color32::WHITE)).fill(theme::ACCENT);
            if ui.add(b).clicked() {
                finish_now = true;
            }
        });
    if let Some(s) = app.sketch.as_mut() {
        s.construction = construction;
        s.snap_grid = snap;
        s.sides = sides;
    }
    if let (Some(e), Some(i)) = (edited, sel)
        && let Ok(v) = serde_json::to_value(&e) {
            app.run("sketch.edit_entity", &json!({ "sketch": sid, "index": i, "entity": v }));
        }
    if delete
        && let Some(i) = sel {
            app.run("sketch.delete_entity", &json!({ "sketch": sid, "index": i }));
            if let Some(s) = app.sketch.as_mut() {
                s.selected = None;
            }
        }
    if finish_now {
        finish(app);
    }
    let _ = pos2(0.0, 0.0);
}
