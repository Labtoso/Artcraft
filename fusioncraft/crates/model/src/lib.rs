//! Timeline evaluation (layer L2): replays the active features of a document and produces the
//! bodies. A failing feature is recorded in [`Model::errors`] (shown red in the timeline) and
//! skipped; it never stops the rest of the timeline.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use fusioncraft_doc::{
    profile, Axis, Document, Entity, Expr, ExtentDirection, Feature, FeatureKind, Id, Operation, Params, Profile,
};
use fusioncraft_geom::mesh::Edge;
use fusioncraft_geom::{csg, poly, solid, GeomError, Mesh, Plane, Vec2, Vec3};
use std::collections::HashMap;

/// A copy transform used by mirror and patterns.
type Transform = Box<dyn Fn(&Mesh) -> Mesh>;

/// Segments for curved primitives.
pub const SEGMENTS: usize = 64;
/// Upper bound for pattern copies.
pub const MAX_COPIES: u32 = 400;
/// Drawing edges: faces meeting at more than this angle get a line.
pub const CREASE_DEG: f64 = 25.0;

#[derive(Debug, Clone)]
pub struct Body {
    /// Stable per evaluation: creation order.
    pub index: usize,
    pub name: String,
    pub mesh: Mesh,
    pub edges: Vec<Edge>,
    /// The feature that created the body.
    pub source: Id,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub bodies: Vec<Body>,
    /// Feature id → error message.
    pub errors: HashMap<Id, String>,
    /// Feature id → warning (the feature ran, but not fully).
    pub warnings: HashMap<Id, String>,
}

impl Model {
    pub fn volume(&self) -> f64 {
        self.bodies.iter().map(|b| b.mesh.volume()).sum()
    }
    pub fn aabb(&self) -> fusioncraft_geom::Aabb {
        self.bodies.iter().fold(fusioncraft_geom::Aabb::empty(), |a, b| a.union(&b.mesh.aabb()))
    }
}

/// Evaluate on a worker thread with a large stack; a panic becomes an error, never a crash.
pub fn evaluate(doc: &Document) -> Model {
    let doc = doc.clone();
    let handle = std::thread::Builder::new()
        .name("fusioncraft-eval".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || evaluate_here(&doc));
    match handle {
        Ok(h) => match h.join() {
            Ok(m) => m,
            Err(_) => {
                log::error!("timeline evaluation panicked");
                let mut m = Model::default();
                m.errors.insert(0, "internal error while computing the model".into());
                m
            }
        },
        Err(e) => {
            log::error!("could not start evaluation thread: {e}");
            Model::default()
        }
    }
}

/// Evaluate on the calling thread.
pub fn evaluate_here(doc: &Document) -> Model {
    let params = doc.params();
    let mut ev = Evaluator { doc, params, bodies: Vec::new(), model: Model::default(), counter: 0 };
    for f in doc.timeline.iter().take(doc.active_len()) {
        if f.suppressed {
            continue;
        }
        if let Err(e) = ev.feature(f) {
            ev.model.errors.insert(f.id, e);
        }
    }
    let mut model = ev.model;
    model.bodies = ev
        .bodies
        .into_iter()
        .filter(|b| !b.mesh.is_empty())
        .map(|mut b| {
            b.edges = b.mesh.feature_edges(CREASE_DEG);
            b
        })
        .collect();
    model
}

/// The profiles of a sketch (for the UI's profile picker).
pub fn sketch_profiles(doc: &Document, sketch: Id) -> Vec<Profile> {
    doc.sketch(sketch).map(profile::profiles).unwrap_or_default()
}

struct Evaluator<'a> {
    doc: &'a Document,
    params: Params,
    bodies: Vec<Body>,
    model: Model,
    counter: usize,
}

fn geom(e: GeomError) -> String {
    e.to_string()
}

impl Evaluator<'_> {
    fn num(&self, e: &Expr, what: &str) -> Result<f64, String> {
        self.params.eval(e).map_err(|err| format!("{what}: {err}"))
    }
    fn positive(&self, e: &Expr, what: &str) -> Result<f64, String> {
        let v = self.num(e, what)?;
        if v <= 0.0 {
            return Err(format!("{what} must be greater than 0"));
        }
        if v > 1e7 {
            return Err(format!("{what} is too large"));
        }
        Ok(v)
    }

    fn feature(&mut self, f: &Feature) -> Result<(), String> {
        match &f.kind {
            FeatureKind::Sketch { sketch } => {
                self.doc.sketch(*sketch).map(|_| ()).ok_or_else(|| "the sketch is missing".to_owned())
            }
            FeatureKind::Extrude { sketch, profiles, distance, direction, operation } => {
                let (plane, profs) = self.selected_profiles(*sketch, profiles)?;
                let d = self.num(distance, "Distance")?;
                if d.abs() < 1e-9 || d.abs() > 1e7 {
                    return Err("Distance must not be 0".into());
                }
                let (s, e) = match direction {
                    ExtentDirection::OneSide => (0.0, d),
                    ExtentDirection::Symmetric => (-d.abs(), d.abs()),
                };
                let tool = self.profile_tool(&profs, |outer| solid::extrude(outer, &plane, s, e))?;
                self.apply(f, tool, *operation)
            }
            FeatureKind::Revolve { sketch, profiles, axis, angle, operation } => {
                let (plane, profs) = self.selected_profiles(*sketch, profiles)?;
                let a = self.num(angle, "Angle")?.to_radians();
                let (ao, ad) = self.revolve_axis(*sketch, *axis)?;
                let tool = self.profile_tool(&profs, |outer| solid::revolve(outer, &plane, ao, ad, a, SEGMENTS))?;
                self.apply(f, tool, *operation)
            }
            FeatureKind::Box { base, width, depth, height, operation } => {
                let (w, d, h) = (self.positive(width, "Width")?, self.positive(depth, "Depth")?, self.positive(height, "Height")?);
                let tool = solid::cuboid(*base - Vec3::new(w / 2.0, d / 2.0, 0.0), Vec3::new(w, d, h));
                self.apply(f, tool, *operation)
            }
            FeatureKind::Cylinder { base, radius, height, operation } => {
                let (r, h) = (self.positive(radius, "Radius")?, self.num(height, "Height")?);
                if h.abs() < 1e-9 {
                    return Err("Height must not be 0".into());
                }
                let start = if h < 0.0 { *base + Vec3::Z * h } else { *base };
                let tool = solid::cylinder(start, Vec3::Z, r, h.abs(), SEGMENTS);
                self.apply(f, tool, *operation)
            }
            FeatureKind::Sphere { center, radius, operation } => {
                let r = self.positive(radius, "Radius")?;
                self.apply(f, solid::sphere(*center, r, SEGMENTS), *operation)
            }
            FeatureKind::Torus { center, major, minor, operation } => {
                let (ma, mi) = (self.positive(major, "Major radius")?, self.positive(minor, "Minor radius")?);
                if mi >= ma {
                    return Err("The minor radius must be smaller than the major radius".into());
                }
                self.apply(f, solid::torus(*center, Vec3::Z, ma, mi, SEGMENTS), *operation)
            }
            FeatureKind::Cone { base, radius, top_radius, height, operation } => {
                let (r0, h) = (self.positive(radius, "Radius")?, self.positive(height, "Height")?);
                let r1 = self.num(top_radius, "Top radius")?.max(0.0);
                self.apply(f, solid::cone(*base, Vec3::Z, r0, r1, h, SEGMENTS), *operation)
            }
            FeatureKind::Mirror { plane } => {
                let p = plane.plane();
                self.transform_copies(&[Box::new(move |m: &Mesh| m.mirrored(p.origin, p.normal))])
            }
            FeatureKind::PatternRect { count_x, count_y, spacing_x, spacing_y } => {
                let (cx, cy) = ((*count_x).max(1), (*count_y).max(1));
                if cx.saturating_mul(cy) > MAX_COPIES {
                    return Err(format!("At most {MAX_COPIES} copies"));
                }
                let (sx, sy) = (self.num(spacing_x, "Spacing X")?, self.num(spacing_y, "Spacing Y")?);
                let mut ts: Vec<Transform> = Vec::new();
                for i in 0..cx {
                    for j in 0..cy {
                        if i == 0 && j == 0 {
                            continue;
                        }
                        let d = Vec3::new(sx * f64::from(i), sy * f64::from(j), 0.0);
                        ts.push(Box::new(move |m: &Mesh| m.translated(d)));
                    }
                }
                self.transform_copies(&ts)
            }
            FeatureKind::PatternCirc { count, angle, axis } => {
                let n = (*count).clamp(1, MAX_COPIES);
                let total = self.num(angle, "Angle")?.to_radians();
                let full = (total.abs() - std::f64::consts::TAU).abs() < 1e-6;
                let step = if full || n == 1 { total / f64::from(n) } else { total / f64::from(n - 1) };
                let ax = axis.vector();
                let ts: Vec<Transform> = (1..n)
                    .map(|k| {
                        let a = step * f64::from(k);
                        Box::new(move |m: &Mesh| m.rotated(Vec3::ZERO, ax, a)) as Box<dyn Fn(&Mesh) -> Mesh>
                    })
                    .collect();
                self.transform_copies(&ts)
            }
            FeatureKind::Move { dx, dy, dz } => {
                let d = Vec3::new(self.num(dx, "X")?, self.num(dy, "Y")?, self.num(dz, "Z")?);
                if self.bodies.is_empty() {
                    return Err("There is no body to move".into());
                }
                for b in &mut self.bodies {
                    b.mesh = b.mesh.translated(d);
                }
                Ok(())
            }
            FeatureKind::Shell { thickness } => {
                let t = self.positive(thickness, "Thickness")?;
                if self.bodies.is_empty() {
                    return Err("There is no body to shell".into());
                }
                for b in &mut self.bodies {
                    let bb = b.mesh.aabb();
                    let c = bb.center();
                    let s = bb.size();
                    let k = |v: f64| if v > 2.0 * t { (v - 2.0 * t) / v } else { 0.0 };
                    let (kx, ky, kz) = (k(s.x), k(s.y), k(s.z));
                    if kx <= 0.0 || ky <= 0.0 || kz <= 0.0 {
                        return Err("The wall is thicker than the body".into());
                    }
                    let inner = b.mesh.map_points(|p| c + Vec3::new((p.x - c.x) * kx, (p.y - c.y) * ky, (p.z - c.z) * kz), false);
                    b.mesh = csg::subtract(&b.mesh, &inner).map_err(geom)?;
                }
                Ok(())
            }
        }
    }

    fn selected_profiles(&self, sketch: Id, sel: &[usize]) -> Result<(Plane, Vec<Profile>), String> {
        let s = self.doc.sketch(sketch).ok_or("the sketch is missing")?;
        let plane = s.plane.plane();
        if !plane.is_valid() {
            return Err("the sketch plane is invalid".into());
        }
        let all = profile::profiles(s);
        if all.is_empty() {
            return Err("the sketch has no closed profile".into());
        }
        if sel.is_empty() {
            return Ok((plane, all));
        }
        let mut out = Vec::new();
        for &i in sel {
            out.push(all.get(i).cloned().ok_or_else(|| format!("profile {} no longer exists", i + 1))?);
        }
        Ok((plane, out))
    }

    fn revolve_axis(&self, sketch: Id, axis: Axis) -> Result<(Vec2, Vec2), String> {
        match axis {
            Axis::SketchX => Ok((Vec2::ZERO, Vec2::new(1.0, 0.0))),
            Axis::SketchY => Ok((Vec2::ZERO, Vec2::new(0.0, 1.0))),
            Axis::Line { index } => {
                let s = self.doc.sketch(sketch).ok_or("the sketch is missing")?;
                match s.entities.get(index) {
                    Some(Entity::Line { a, b, .. }) if a.distance(*b) > 1e-9 => Ok((*a, *b - *a)),
                    _ => Err("the axis line no longer exists".into()),
                }
            }
        }
    }

    /// Build the tool body for the selected profiles. A hole that is itself selected is filled.
    fn profile_tool(&self, profs: &[Profile], build: impl Fn(&[Vec2]) -> Result<Mesh, GeomError>) -> Result<Mesh, String> {
        let same = |a: &[Vec2], b: &[Vec2]| a.len() == b.len() && (poly::signed_area(a) - poly::signed_area(b)).abs() < 1e-6 && a.iter().zip(b).all(|(p, q)| p.distance(*q) < 1e-6);
        let mut tool: Option<Mesh> = None;
        for p in profs {
            let mut m = build(&p.outer).map_err(geom)?;
            for h in &p.holes {
                if profs.iter().any(|o| same(&o.outer, h)) {
                    continue;
                }
                let hm = build(h).map_err(geom)?;
                m = csg::subtract(&m, &hm).map_err(geom)?;
            }
            tool = Some(match tool {
                None => m,
                Some(t) => {
                    // Selected regions nested in each other are already covered by the outer one.
                    if profs.iter().any(|o| o.holes.iter().any(|h| same(h, &p.outer))) {
                        t
                    } else {
                        csg::union(&t, &m).map_err(geom)?
                    }
                }
            });
        }
        tool.ok_or_else(|| "no profile selected".to_owned())
    }

    fn new_body(&mut self, f: &Feature, mesh: Mesh) {
        self.counter += 1;
        self.bodies.push(Body { index: self.counter, name: format!("Body{}", self.counter), mesh, edges: Vec::new(), source: f.id });
    }

    fn apply(&mut self, f: &Feature, tool: Mesh, op: Operation) -> Result<(), String> {
        if tool.is_empty() {
            return Err("the feature produced no geometry".into());
        }
        let bb = tool.aabb();
        let touching: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| self.bodies.get(i).is_some_and(|b| b.mesh.aabb().overlaps(&bb, 1e-6)))
            .collect();
        match op {
            Operation::NewBody => {
                self.new_body(f, tool);
                Ok(())
            }
            Operation::Join => {
                let Some((&first, rest)) = touching.split_first() else {
                    self.new_body(f, tool);
                    return Ok(());
                };
                let mut merged = match self.bodies.get(first) {
                    Some(b) => csg::union(&b.mesh, &tool).map_err(geom)?,
                    None => tool,
                };
                for &i in rest {
                    if let Some(b) = self.bodies.get(i) {
                        merged = csg::union(&merged, &b.mesh).map_err(geom)?;
                    }
                }
                if let Some(b) = self.bodies.get_mut(first) {
                    b.mesh = merged;
                }
                for &i in rest.iter().rev() {
                    if i < self.bodies.len() {
                        self.bodies.remove(i);
                    }
                }
                Ok(())
            }
            Operation::Cut => {
                if touching.is_empty() {
                    return Err("the cut does not touch any body".into());
                }
                for &i in &touching {
                    if let Some(b) = self.bodies.get_mut(i) {
                        b.mesh = csg::subtract(&b.mesh, &tool).map_err(geom)?;
                    }
                }
                self.bodies.retain(|b| !b.mesh.is_empty());
                Ok(())
            }
            Operation::Intersect => {
                if touching.is_empty() {
                    return Err("the intersection does not touch any body".into());
                }
                for &i in &touching {
                    if let Some(b) = self.bodies.get_mut(i) {
                        b.mesh = csg::intersect(&b.mesh, &tool).map_err(geom)?;
                    }
                }
                self.bodies.retain(|b| !b.mesh.is_empty());
                Ok(())
            }
        }
    }

    /// Join transformed copies of every body into it.
    fn transform_copies(&mut self, transforms: &[Transform]) -> Result<(), String> {
        if self.bodies.is_empty() {
            return Err("There is no body to copy".into());
        }
        for b in &mut self.bodies {
            let original = b.mesh.clone();
            let mut acc = original.clone();
            for t in transforms {
                acc = csg::union(&acc, &t(&original)).map_err(geom)?;
            }
            b.mesh = acc;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusioncraft_doc::{PlaneRef, Sketch, SketchPlane};

    fn doc_with_plate() -> (Document, Id) {
        let mut d = Document::new("t");
        let sid = d.alloc_id();
        let mut s = Sketch::new(sid, "Sketch1".into(), SketchPlane::Origin { plane: PlaneRef::Xy });
        s.entities.push(Entity::Rect { a: Vec2::new(-20.0, -10.0), b: Vec2::new(20.0, 10.0) });
        s.entities.push(Entity::Circle { center: Vec2::new(0.0, 0.0), radius: 5.0 });
        d.sketches.push(s);
        let fid = d.alloc_id();
        d.insert_feature(Feature { id: fid, name: "Sketch1".into(), suppressed: false, kind: FeatureKind::Sketch { sketch: sid } });
        (d, sid)
    }

    fn extrude(d: &mut Document, sketch: Id, profiles: Vec<usize>, dist: &str, op: Operation) -> Id {
        let id = d.alloc_id();
        d.insert_feature(Feature {
            id,
            name: d.next_name("Extrude"),
            suppressed: false,
            kind: FeatureKind::Extrude { sketch, profiles, distance: Expr::new(dist), direction: ExtentDirection::OneSide, operation: op },
        });
        id
    }

    #[test]
    fn plate_with_hole_and_parameters() {
        let (mut d, sid) = doc_with_plate();
        d.parameters.push(fusioncraft_doc::Parameter { name: "t".into(), expr: Expr::new("5 mm"), comment: String::new() });
        extrude(&mut d, sid, vec![0], "t", Operation::NewBody);
        let m = evaluate(&d);
        assert!(m.errors.is_empty(), "{:?}", m.errors);
        assert_eq!(m.bodies.len(), 1);
        let hole = std::f64::consts::PI * 25.0 * 5.0;
        assert!((m.volume() - (800.0 * 5.0 - hole)).abs() < 2.0, "{}", m.volume());
        assert!(!m.bodies[0].edges.is_empty());
        // Both regions = full plate.
        extrude(&mut d, sid, vec![1], "t", Operation::Join);
        let m = evaluate(&d);
        assert!((m.volume() - 4000.0).abs() < 1.0, "{}", m.volume());
        // Changing the parameter updates everything.
        d.parameters[0].expr = Expr::new("10");
        assert!((evaluate(&d).volume() - 8000.0).abs() < 1.0);
    }

    #[test]
    fn cut_errors_and_rollback() {
        let (mut d, sid) = doc_with_plate();
        let cut = extrude(&mut d, sid, vec![], "3", Operation::Cut);
        let m = evaluate(&d);
        assert!(m.errors.contains_key(&cut));
        let (mut d, sid) = doc_with_plate();
        extrude(&mut d, sid, vec![], "10", Operation::NewBody);
        let id = d.alloc_id();
        d.insert_feature(Feature {
            id,
            name: "Cyl".into(),
            suppressed: false,
            kind: FeatureKind::Cylinder { base: Vec3::new(15.0, 0.0, -1.0), radius: Expr::new("3"), height: Expr::new("20"), operation: Operation::Cut },
        });
        let full = evaluate(&d).volume();
        assert!(full < 8000.0 - 200.0, "{full} {:?}", evaluate(&d).errors);
        d.marker = Some(2);
        assert!((evaluate(&d).volume() - 8000.0).abs() < 1.0);
        d.marker = None;
        d.timeline[2].suppressed = true;
        assert!((evaluate(&d).volume() - 8000.0).abs() < 1.0);
        // A bad expression is an error on that feature only.
        d.timeline[2].suppressed = false;
        if let FeatureKind::Cylinder { radius, .. } = &mut d.timeline[2].kind {
            *radius = Expr::new("nope *");
        }
        let m = evaluate(&d);
        assert_eq!(m.errors.len(), 1);
        assert_eq!(m.bodies.len(), 1);
    }

    #[test]
    fn patterns_and_mirror() {
        let mut d = Document::new("p");
        let id = d.alloc_id();
        d.insert_feature(Feature {
            id,
            name: "Box1".into(),
            suppressed: false,
            kind: FeatureKind::Box { base: Vec3::new(10.0, 0.0, 0.0), width: "2".into(), depth: "2".into(), height: "2".into(), operation: Operation::NewBody },
        });
        let id = d.alloc_id();
        d.insert_feature(Feature { id, name: "Mirror1".into(), suppressed: false, kind: FeatureKind::Mirror { plane: PlaneRef::Yz } });
        assert!((evaluate(&d).volume() - 16.0).abs() < 1e-6);
        let id = d.alloc_id();
        d.insert_feature(Feature { id, name: "Pat".into(), suppressed: false, kind: FeatureKind::PatternCirc { count: 4, angle: "360".into(), axis: Default::default() } });
        let m = evaluate(&d);
        assert!((m.volume() - 32.0).abs() < 1e-6, "{}", m.volume());
    }
}
