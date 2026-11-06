//! Triangle meshes: the representation of bodies.

use crate::vec::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub v: [Vec3; 3],
}

impl Triangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { v: [a, b, c] }
    }
    pub fn normal(&self) -> Vec3 {
        let [a, b, c] = self.v;
        (b - a).cross(c - a).normalized_or(Vec3::Z)
    }
    pub fn area(&self) -> f64 {
        let [a, b, c] = self.v;
        (b - a).cross(c - a).length() * 0.5
    }
    pub fn flipped(&self) -> Self {
        let [a, b, c] = self.v;
        Self::new(a, c, b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn empty() -> Self {
        Self { min: Vec3::new(f64::MAX, f64::MAX, f64::MAX), max: Vec3::new(f64::MIN, f64::MIN, f64::MIN) }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    pub fn include(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn union(&self, o: &Self) -> Self {
        Self { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn overlaps(&self, o: &Self, tol: f64) -> bool {
        !(self.is_empty() || o.is_empty())
            && self.min.x <= o.max.x + tol
            && o.min.x <= self.max.x + tol
            && self.min.y <= o.max.y + tol
            && o.min.y <= self.max.y + tol
            && self.min.z <= o.max.z + tol
            && o.min.z <= self.max.z + tol
    }
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub tris: Vec<Triangle>,
}

/// A line segment of the mesh's outline drawing.
pub type Edge = [Vec3; 2];

impl Mesh {
    pub fn new(tris: Vec<Triangle>) -> Self {
        Self { tris }
    }
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }
    pub fn aabb(&self) -> Aabb {
        let mut b = Aabb::empty();
        for t in &self.tris {
            for p in t.v {
                b.include(p);
            }
        }
        b
    }
    /// Signed volume (positive for outward-facing triangles).
    pub fn volume(&self) -> f64 {
        self.tris
            .iter()
            .map(|t| {
                let [a, b, c] = t.v;
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }
    pub fn area(&self) -> f64 {
        self.tris.iter().map(Triangle::area).sum()
    }
    /// Flip every triangle if the mesh is inside out.
    pub fn orient_outward(&mut self) {
        if self.volume() < 0.0 {
            for t in &mut self.tris {
                *t = t.flipped();
            }
        }
    }
    pub fn map_points(&self, f: impl Fn(Vec3) -> Vec3, flip: bool) -> Self {
        Self::new(
            self.tris
                .iter()
                .map(|t| {
                    let n = Triangle::new(f(t.v[0]), f(t.v[1]), f(t.v[2]));
                    if flip { n.flipped() } else { n }
                })
                .collect(),
        )
    }
    pub fn translated(&self, d: Vec3) -> Self {
        self.map_points(|p| p + d, false)
    }
    /// Mirror across the plane through `origin` with unit `normal`.
    pub fn mirrored(&self, origin: Vec3, normal: Vec3) -> Self {
        self.map_points(|p| p - normal * (2.0 * (p - origin).dot(normal)), true)
    }
    pub fn rotated(&self, origin: Vec3, axis: Vec3, angle: f64) -> Self {
        self.map_points(|p| origin + (p - origin).rotated_about(axis, angle), false)
    }
    pub fn append(&mut self, other: &Mesh) {
        self.tris.extend_from_slice(&other.tris);
    }
    /// Remove zero-area and non-finite triangles.
    pub fn cleaned(mut self) -> Self {
        self.tris.retain(|t| t.v.iter().all(|p| p.is_finite()) && t.area() > 1e-12);
        self
    }

    /// The drawing edges of the mesh: boundaries between faces whose normals differ by more
    /// than `crease_deg`. Edges inside flat faces (also across T-junctions left by booleans)
    /// and between the facets of smooth curved faces are left out.
    pub fn feature_edges(&self, crease_deg: f64) -> Vec<Edge> {
        let scale = self.aabb().size().length().max(1.0);
        let q = 1e6 / scale;
        let key = |p: Vec3| ((p.x * q).round() as i64, (p.y * q).round() as i64, (p.z * q).round() as i64);
        // Group triangles by plane.
        let mut groups: HashMap<(i64, i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, t) in self.tris.iter().enumerate() {
            let n = t.normal();
            let d = n.dot(t.v[0]);
            let k = ((n.x * 1e4).round() as i64, (n.y * 1e4).round() as i64, (n.z * 1e4).round() as i64, (d * q / 10.0).round() as i64);
            groups.entry(k).or_default().push(i);
        }
        // Every vertex, for splitting edges at T-junctions.
        let mut verts: Vec<Vec3> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for t in &self.tris {
            for p in t.v {
                if seen.insert(key(p)) {
                    verts.push(p);
                }
            }
        }
        let grid = VertexGrid::new(&verts, scale / 48.0);
        let cos_limit = crease_deg.to_radians().cos();
        // Split boundary segments of each group, keyed by endpoints, with the group's normal.
        let mut boundary: HashMap<((i64, i64, i64), (i64, i64, i64)), (Edge, Vec<Vec3>)> = HashMap::new();
        for tris in groups.values() {
            let normal = self.tris.get(tris.first().copied().unwrap_or(0)).map(Triangle::normal).unwrap_or(Vec3::Z);
            let mut count: HashMap<((i64, i64, i64), (i64, i64, i64)), (Edge, i32)> = HashMap::new();
            for &ti in tris {
                let Some(t) = self.tris.get(ti) else { continue };
                for e in 0..3 {
                    let a = t.v[e];
                    let b = t.v[(e + 1) % 3];
                    for (p, r) in grid.split(a, b) {
                        let (ka, kb) = (key(p), key(r));
                        if ka == kb {
                            continue;
                        }
                        let k = if ka < kb { (ka, kb) } else { (kb, ka) };
                        let entry = count.entry(k).or_insert(([p, r], 0));
                        entry.1 += 1;
                    }
                }
            }
            for (k, (seg, c)) in count {
                if c % 2 == 1 {
                    boundary.entry(k).or_insert((seg, Vec::new())).1.push(normal);
                }
            }
        }
        boundary
            .into_values()
            .filter(|(_, normals)| match normals.as_slice() {
                [a, b] => a.dot(*b) < cos_limit,
                _ => true,
            })
            .map(|(e, _)| e)
            .collect()
    }
}

/// Uniform grid of points for "which vertices lie on this segment" queries.
struct VertexGrid<'a> {
    verts: &'a [Vec3],
    cell: f64,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
}

impl<'a> VertexGrid<'a> {
    fn new(verts: &'a [Vec3], cell: f64) -> Self {
        let cell = cell.max(1e-6);
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in verts.iter().enumerate() {
            cells.entry(Self::cell_of(*p, cell)).or_default().push(i);
        }
        Self { verts, cell, cells }
    }
    fn cell_of(p: Vec3, cell: f64) -> (i64, i64, i64) {
        ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64)
    }
    /// The segment a–b split at every vertex lying on it, in order.
    fn split(&self, a: Vec3, b: Vec3) -> Vec<(Vec3, Vec3)> {
        let d = b - a;
        let len2 = d.dot(d);
        if len2 < 1e-18 {
            return Vec::new();
        }
        let lo = Self::cell_of(a.min(b), self.cell);
        let hi = Self::cell_of(a.max(b), self.cell);
        let span = (hi.0 - lo.0 + 1) * (hi.1 - lo.1 + 1) * (hi.2 - lo.2 + 1);
        let mut ts: Vec<f64> = Vec::new();
        let tol = 1e-6 * len2.sqrt().max(1.0);
        let mut test = |i: usize| {
            if let Some(p) = self.verts.get(i) {
                let t = (*p - a).dot(d) / len2;
                if t > 1e-6 && t < 1.0 - 1e-6 && (a + d * t).distance(*p) < tol {
                    ts.push(t);
                }
            }
        };
        if span > 4096 {
            for i in 0..self.verts.len() {
                test(i);
            }
        } else {
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        if let Some(list) = self.cells.get(&(x, y, z)) {
                            for &i in list {
                                test(i);
                            }
                        }
                    }
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        let mut out = Vec::with_capacity(ts.len() + 1);
        let mut prev = a;
        for t in ts {
            let p = a + d * t;
            out.push((prev, p));
            prev = p;
        }
        out.push((prev, b));
        out
    }
}
