//! Sketches: 2D curves on a plane.

use crate::feature::PlaneRef;
use crate::Id;
use fusioncraft_geom::{Plane, Vec2};
use serde::{Deserialize, Serialize};

/// Where a sketch lies. Origin planes are fixed; a face plane is captured when the sketch is
/// created on a body face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SketchPlane {
    Origin { plane: PlaneRef },
    Custom { plane: Plane },
}

impl SketchPlane {
    pub fn plane(&self) -> Plane {
        match self {
            SketchPlane::Origin { plane } => plane.plane(),
            SketchPlane::Custom { plane } => *plane,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Entity {
    Line {
        a: Vec2,
        b: Vec2,
        #[serde(default)]
        construction: bool,
    },
    /// Two-point rectangle (opposite corners).
    Rect { a: Vec2, b: Vec2 },
    Circle { center: Vec2, radius: f64 },
    /// Regular polygon by circumscribed radius.
    Polygon {
        center: Vec2,
        radius: f64,
        sides: u32,
        #[serde(default)]
        rotation: f64,
    },
    /// Arc through three points: start, a point on the arc, end.
    Arc { start: Vec2, mid: Vec2, end: Vec2 },
    Point { p: Vec2 },
}

/// Segments used for circles and arcs when turning curves into profiles.
pub const CIRCLE_SEGMENTS: usize = 96;

impl Entity {
    pub fn kind(&self) -> &'static str {
        match self {
            Entity::Line { .. } => "line",
            Entity::Rect { .. } => "rect",
            Entity::Circle { .. } => "circle",
            Entity::Polygon { .. } => "polygon",
            Entity::Arc { .. } => "arc",
            Entity::Point { .. } => "point",
        }
    }

    pub fn is_finite(&self) -> bool {
        match self {
            Entity::Line { a, b, .. } | Entity::Rect { a, b } => a.is_finite() && b.is_finite(),
            Entity::Circle { center, radius } => center.is_finite() && radius.is_finite(),
            Entity::Polygon { center, radius, rotation, .. } => center.is_finite() && radius.is_finite() && rotation.is_finite(),
            Entity::Arc { start, mid, end } => start.is_finite() && mid.is_finite() && end.is_finite(),
            Entity::Point { p } => p.is_finite(),
        }
    }

    /// A closed outline for entities that are closed on their own.
    pub fn closed_loop(&self) -> Option<Vec<Vec2>> {
        match self {
            Entity::Rect { a, b } => {
                let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
                let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
                ((x1 - x0) > 1e-9 && (y1 - y0) > 1e-9)
                    .then(|| vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)])
            }
            Entity::Circle { center, radius } => (*radius > 1e-9).then(|| fusioncraft_geom::poly::circle(*center, *radius, CIRCLE_SEGMENTS)),
            Entity::Polygon { center, radius, sides, rotation } => {
                let n = (*sides).clamp(3, 1024) as usize;
                (*radius > 1e-9).then(|| {
                    (0..n)
                        .map(|i| {
                            let a = rotation.to_radians() + std::f64::consts::TAU * i as f64 / n as f64 + std::f64::consts::FRAC_PI_2;
                            *center + Vec2::new(a.cos(), a.sin()) * *radius
                        })
                        .collect()
                })
            }
            _ => None,
        }
    }

    /// An open polyline for chainable curves (lines and arcs).
    pub fn open_path(&self) -> Option<Vec<Vec2>> {
        match self {
            Entity::Line { a, b, construction: false } => Some(vec![*a, *b]),
            Entity::Arc { start, mid, end } => Some(arc_points(*start, *mid, *end)),
            _ => None,
        }
    }

    /// Points for drawing the entity.
    pub fn display_points(&self) -> Vec<Vec2> {
        if let Some(mut l) = self.closed_loop() {
            if let Some(f) = l.first().copied() {
                l.push(f);
            }
            return l;
        }
        match self {
            Entity::Line { a, b, .. } => vec![*a, *b],
            Entity::Arc { start, mid, end } => arc_points(*start, *mid, *end),
            Entity::Point { p } => vec![*p],
            _ => Vec::new(),
        }
    }

    /// Snap points: end points, centres and corners.
    pub fn snap_points(&self) -> Vec<Vec2> {
        match self {
            Entity::Line { a, b, .. } => vec![*a, *b, (*a + *b) * 0.5],
            Entity::Rect { .. } => self.closed_loop().unwrap_or_default(),
            Entity::Circle { center, .. } => vec![*center],
            Entity::Polygon { center, .. } => {
                let mut v = self.closed_loop().unwrap_or_default();
                v.push(*center);
                v
            }
            Entity::Arc { start, end, .. } => vec![*start, *end],
            Entity::Point { p } => vec![*p],
        }
    }

    /// Distance from a point to the entity's curve.
    pub fn distance_to(&self, p: Vec2) -> f64 {
        let pts = self.display_points();
        if let [only] = pts.as_slice() {
            return only.distance(p);
        }
        pts.windows(2)
            .filter_map(|w| match w {
                [a, b] => Some(segment_distance(p, *a, *b)),
                _ => None,
            })
            .fold(f64::MAX, f64::min)
    }
}

pub fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    if l2 < 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
    p.distance(a + d * t)
}

/// Polyline of the circular arc through three points (a straight line if collinear).
pub fn arc_points(s: Vec2, m: Vec2, e: Vec2) -> Vec<Vec2> {
    let Some(c) = circumcenter(s, m, e) else { return vec![s, e] };
    let r = c.distance(s);
    let ang = |p: Vec2| (p.y - c.y).atan2(p.x - c.x);
    let a0 = ang(s);
    let tau = std::f64::consts::TAU;
    let norm = |a: f64| ((a % tau) + tau) % tau;
    let sweep_e = norm(ang(e) - a0);
    let sweep_m = norm(ang(m) - a0);
    // Go the way that passes through m.
    let sweep = if sweep_m <= sweep_e { sweep_e } else { sweep_e - tau };
    let n = ((sweep.abs() / tau * CIRCLE_SEGMENTS as f64).ceil() as usize).clamp(2, 1024);
    (0..=n)
        .map(|i| {
            let a = a0 + sweep * i as f64 / n as f64;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

fn circumcenter(a: Vec2, b: Vec2, c: Vec2) -> Option<Vec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.dot(a), b.dot(b), c.dot(c));
    let x = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let y = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let p = Vec2::new(x, y);
    p.is_finite().then_some(p)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    pub id: Id,
    pub name: String,
    pub plane: SketchPlane,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default = "yes")]
    pub visible: bool,
}

fn yes() -> bool {
    true
}

impl Sketch {
    pub fn new(id: Id, name: String, plane: SketchPlane) -> Self {
        Self { id, name, plane, entities: Vec::new(), visible: true }
    }
}
