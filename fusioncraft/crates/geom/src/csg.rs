//! Mesh booleans with binary space partitioning trees.
//!
//! Both operands are split by each other's face planes; the pieces that lie inside (or outside)
//! the other solid are kept or dropped. Inputs must be closed, outward-facing meshes. The work is
//! bounded: a tree deeper than [`MAX_DEPTH`] or with more than [`MAX_POLYGONS`] pieces reports
//! [`GeomError::TooComplex`] instead of exhausting the stack or memory.

use crate::mesh::{Mesh, Triangle};
use crate::vec::Vec3;
use crate::GeomError;

pub const MAX_DEPTH: usize = 2_000;
pub const MAX_POLYGONS: usize = 400_000;
const PLANE_EPS: f64 = 1e-6;

#[derive(Debug, Clone, Copy)]
struct CPlane {
    n: Vec3,
    w: f64,
}

#[derive(Debug, Clone)]
struct Poly {
    pts: Vec<Vec3>,
    plane: CPlane,
}

impl CPlane {
    fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Option<Self> {
        let n = (b - a).cross(c - a).normalized()?;
        Some(Self { n, w: n.dot(a) })
    }
    fn flipped(self) -> Self {
        Self { n: -self.n, w: -self.w }
    }
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

struct Buckets<'a> {
    co_front: &'a mut Vec<Poly>,
    co_back: Option<&'a mut Vec<Poly>>,
    front: &'a mut Vec<Poly>,
    back: &'a mut Vec<Poly>,
}

fn split(plane: CPlane, poly: Poly, out: &mut Buckets<'_>) {
    let mut kind = 0u8;
    let types: Vec<u8> = poly
        .pts
        .iter()
        .map(|p| {
            let t = plane.n.dot(*p) - plane.w;
            let k = if t < -PLANE_EPS {
                BACK
            } else if t > PLANE_EPS {
                FRONT
            } else {
                COPLANAR
            };
            kind |= k;
            k
        })
        .collect();
    match kind {
        COPLANAR => {
            if plane.n.dot(poly.plane.n) > 0.0 {
                out.co_front.push(poly);
            } else if let Some(b) = out.co_back.as_deref_mut() {
                b.push(poly);
            } else {
                out.co_front.push(poly);
            }
        }
        FRONT => out.front.push(poly),
        BACK => out.back.push(poly),
        _ => {
            let n = poly.pts.len();
            let mut f = Vec::with_capacity(n + 1);
            let mut b = Vec::with_capacity(n + 1);
            for i in 0..n {
                let j = (i + 1) % n;
                let (Some(&ti), Some(&tj), Some(&vi), Some(&vj)) =
                    (types.get(i), types.get(j), poly.pts.get(i), poly.pts.get(j))
                else {
                    continue;
                };
                if ti != BACK {
                    f.push(vi);
                }
                if ti != FRONT {
                    b.push(vi);
                }
                if (ti | tj) == SPANNING {
                    let denom = plane.n.dot(vj - vi);
                    if denom.abs() > 1e-15 {
                        let t = (plane.w - plane.n.dot(vi)) / denom;
                        let v = vi.lerp(vj, t.clamp(0.0, 1.0));
                        f.push(v);
                        b.push(v);
                    }
                }
            }
            if f.len() >= 3 {
                out.front.push(Poly { pts: f, plane: poly.plane });
            }
            if b.len() >= 3 {
                out.back.push(Poly { pts: b, plane: poly.plane });
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<CPlane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polys: Vec<Poly>,
}

struct Budget {
    pieces: usize,
}

impl Budget {
    fn spend(&mut self, n: usize) -> Result<(), GeomError> {
        self.pieces = self.pieces.saturating_add(n);
        if self.pieces > MAX_POLYGONS * 8 {
            return Err(GeomError::TooComplex(MAX_POLYGONS));
        }
        Ok(())
    }
}

impl Node {
    fn invert(&mut self) {
        let mut stack: Vec<&mut Node> = vec![self];
        while let Some(n) = stack.pop() {
            for p in &mut n.polys {
                p.pts.reverse();
                p.plane = p.plane.flipped();
            }
            n.plane = n.plane.map(CPlane::flipped);
            std::mem::swap(&mut n.front, &mut n.back);
            if let Some(f) = n.front.as_deref_mut() {
                stack.push(f);
            }
            if let Some(b) = n.back.as_deref_mut() {
                stack.push(b);
            }
        }
    }

    fn clip_polygons(&self, polys: Vec<Poly>, depth: usize, budget: &mut Budget) -> Result<Vec<Poly>, GeomError> {
        if depth > MAX_DEPTH {
            return Err(GeomError::TooComplex(MAX_DEPTH));
        }
        let Some(plane) = self.plane else { return Ok(polys) };
        budget.spend(polys.len())?;
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polys {
            let mut co = Vec::new();
            let mut co_b = Vec::new();
            split(plane, p, &mut Buckets { co_front: &mut co, co_back: Some(&mut co_b), front: &mut front, back: &mut back });
            front.append(&mut co);
            back.append(&mut co_b);
        }
        let mut front = match &self.front {
            Some(f) => f.clip_polygons(front, depth + 1, budget)?,
            None => front,
        };
        let back = match &self.back {
            Some(b) => b.clip_polygons(back, depth + 1, budget)?,
            None => Vec::new(),
        };
        front.extend(back);
        Ok(front)
    }

    fn clip_to(&mut self, other: &Node, budget: &mut Budget) -> Result<(), GeomError> {
        let mut stack: Vec<&mut Node> = vec![self];
        while let Some(n) = stack.pop() {
            let polys = std::mem::take(&mut n.polys);
            n.polys = other.clip_polygons(polys, 0, budget)?;
            if let Some(f) = n.front.as_deref_mut() {
                stack.push(f);
            }
            if let Some(b) = n.back.as_deref_mut() {
                stack.push(b);
            }
        }
        Ok(())
    }

    fn all_polygons(&self) -> Vec<Poly> {
        let mut out = Vec::new();
        let mut stack: Vec<&Node> = vec![self];
        while let Some(n) = stack.pop() {
            out.extend(n.polys.iter().cloned());
            if let Some(f) = n.front.as_deref() {
                stack.push(f);
            }
            if let Some(b) = n.back.as_deref() {
                stack.push(b);
            }
        }
        out
    }

    fn build(&mut self, polys: Vec<Poly>, budget: &mut Budget) -> Result<(), GeomError> {
        // Iterative: a work list of (node, polygons) pairs walks down the tree.
        let mut work: Vec<(&mut Node, Vec<Poly>, usize)> = vec![(self, polys, 0)];
        while let Some((node, polys, depth)) = work.pop() {
            if polys.is_empty() {
                continue;
            }
            if depth > MAX_DEPTH {
                return Err(GeomError::TooComplex(MAX_DEPTH));
            }
            budget.spend(polys.len())?;
            let plane = match node.plane {
                Some(p) => p,
                None => {
                    let Some(first) = polys.first() else { continue };
                    node.plane = Some(first.plane);
                    first.plane
                }
            };
            let mut front = Vec::new();
            let mut back = Vec::new();
            let mut co = Vec::new();
            for p in polys {
                split(plane, p, &mut Buckets { co_front: &mut co, co_back: None, front: &mut front, back: &mut back });
            }
            node.polys.extend(co);
            let Node { front: nf, back: nb, .. } = node;
            if !front.is_empty() {
                let child = nf.get_or_insert_with(Box::default);
                work.push((child.as_mut(), front, depth + 1));
            }
            if !back.is_empty() {
                let child = nb.get_or_insert_with(Box::default);
                work.push((child.as_mut(), back, depth + 1));
            }
        }
        Ok(())
    }
}

fn to_polys(m: &Mesh) -> Result<Vec<Poly>, GeomError> {
    if m.tris.len() > MAX_POLYGONS {
        return Err(GeomError::TooComplex(MAX_POLYGONS));
    }
    Ok(m.tris
        .iter()
        .filter_map(|t| {
            let [a, b, c] = t.v;
            CPlane::from_points(a, b, c).map(|plane| Poly { pts: vec![a, b, c], plane })
        })
        .collect())
}

fn to_mesh(polys: Vec<Poly>) -> Mesh {
    let mut tris = Vec::new();
    for p in polys {
        if let Some(&a) = p.pts.first() {
            for w in p.pts.windows(2).skip(1) {
                if let [b, c] = *w {
                    tris.push(Triangle::new(a, b, c));
                }
            }
        }
    }
    Mesh::new(tris).cleaned()
}

fn tree(m: &Mesh, budget: &mut Budget) -> Result<Node, GeomError> {
    let mut n = Node::default();
    n.build(to_polys(m)?, budget)?;
    Ok(n)
}

/// A ∪ B.
pub fn union(a: &Mesh, b: &Mesh) -> Result<Mesh, GeomError> {
    if !a.aabb().overlaps(&b.aabb(), 1e-9) {
        let mut m = a.clone();
        m.append(b);
        return Ok(m);
    }
    let mut budget = Budget { pieces: 0 };
    let mut ta = tree(a, &mut budget)?;
    let mut tb = tree(b, &mut budget)?;
    ta.clip_to(&tb, &mut budget)?;
    tb.clip_to(&ta, &mut budget)?;
    tb.invert();
    tb.clip_to(&ta, &mut budget)?;
    tb.invert();
    ta.build(tb.all_polygons(), &mut budget)?;
    Ok(to_mesh(ta.all_polygons()))
}

/// A − B.
pub fn subtract(a: &Mesh, b: &Mesh) -> Result<Mesh, GeomError> {
    if !a.aabb().overlaps(&b.aabb(), 1e-9) {
        return Ok(a.clone());
    }
    let mut budget = Budget { pieces: 0 };
    let mut ta = tree(a, &mut budget)?;
    let mut tb = tree(b, &mut budget)?;
    ta.invert();
    ta.clip_to(&tb, &mut budget)?;
    tb.clip_to(&ta, &mut budget)?;
    tb.invert();
    tb.clip_to(&ta, &mut budget)?;
    tb.invert();
    ta.build(tb.all_polygons(), &mut budget)?;
    ta.invert();
    Ok(to_mesh(ta.all_polygons()))
}

/// A ∩ B.
pub fn intersect(a: &Mesh, b: &Mesh) -> Result<Mesh, GeomError> {
    if !a.aabb().overlaps(&b.aabb(), 1e-9) {
        return Ok(Mesh::default());
    }
    let mut budget = Budget { pieces: 0 };
    let mut ta = tree(a, &mut budget)?;
    let mut tb = tree(b, &mut budget)?;
    ta.invert();
    tb.clip_to(&ta, &mut budget)?;
    tb.invert();
    ta.clip_to(&tb, &mut budget)?;
    tb.clip_to(&ta, &mut budget)?;
    ta.build(tb.all_polygons(), &mut budget)?;
    ta.invert();
    Ok(to_mesh(ta.all_polygons()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid;

    fn cube(c: Vec3, s: f64) -> Mesh {
        solid::cuboid(c - Vec3::new(s, s, s) * 0.5, Vec3::new(s, s, s))
    }

    #[test]
    fn volumes() {
        let a = cube(Vec3::ZERO, 2.0);
        let b = cube(Vec3::new(1.0, 1.0, 1.0), 2.0);
        let u = union(&a, &b).unwrap_or_default();
        let s = subtract(&a, &b).unwrap_or_default();
        let i = intersect(&a, &b).unwrap_or_default();
        assert!((u.volume() - 15.0).abs() < 1e-6, "union {}", u.volume());
        assert!((s.volume() - 7.0).abs() < 1e-6, "subtract {}", s.volume());
        assert!((i.volume() - 1.0).abs() < 1e-6, "intersect {}", i.volume());
    }

    #[test]
    fn hole_through_plate() {
        let plate = solid::cuboid(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(10.0, 10.0, 2.0));
        let pin = solid::cylinder(Vec3::new(0.0, 0.0, -1.0), Vec3::Z, 2.0, 4.0, 48);
        let r = subtract(&plate, &pin).unwrap_or_default();
        let expected = 200.0 - pin.volume() / 2.0;
        assert!((r.volume() - expected).abs() < 1e-3, "{} vs {}", r.volume(), expected);
    }
}
