//! Solid builders: extrusion, revolution and primitives. Every result is a closed mesh with
//! outward-facing triangles.

use crate::mesh::{Mesh, Triangle};
use crate::plane::Plane;
use crate::poly;
use crate::vec::{Vec2, Vec3};
use crate::{GeomError, EPS};

/// Extrude a simple polygon from offset `start` to offset `end` along the plane normal.
pub fn extrude(profile: &[Vec2], plane: &Plane, start: f64, end: f64) -> Result<Mesh, GeomError> {
    let pts = poly::ccw(&poly::cleaned(profile));
    if pts.len() < 3 {
        return Err(GeomError::DegenerateProfile);
    }
    if !(start.is_finite() && end.is_finite()) || (end - start).abs() < EPS {
        return Err(GeomError::OutOfRange(format!("extrude distance {}", end - start)));
    }
    let (lo, hi) = if start < end { (start, end) } else { (end, start) };
    let tris = poly::triangulate(&pts)?;
    let at = |p: Vec2, h: f64| plane.to_world(p) + plane.normal * h;
    let mut out = Vec::with_capacity(tris.len() * 2 + pts.len() * 2);
    for [a, b, c] in &tris {
        let (Some(pa), Some(pb), Some(pc)) = (pts.get(*a), pts.get(*b), pts.get(*c)) else { continue };
        out.push(Triangle::new(at(*pa, hi), at(*pb, hi), at(*pc, hi)));
        out.push(Triangle::new(at(*pa, lo), at(*pc, lo), at(*pb, lo)));
    }
    let n = pts.len();
    for i in 0..n {
        let (Some(p), Some(q)) = (pts.get(i), pts.get((i + 1) % n)) else { continue };
        let (a, b, c, d) = (at(*p, lo), at(*q, lo), at(*q, hi), at(*p, hi));
        out.push(Triangle::new(a, b, c));
        out.push(Triangle::new(a, c, d));
    }
    let mut m = Mesh::new(out).cleaned();
    m.orient_outward();
    Ok(m)
}

/// Revolve a simple polygon around an axis lying in the sketch plane.
/// `axis_origin`/`axis_dir` are in plane coordinates; `angle` in radians (sign = direction).
pub fn revolve(profile: &[Vec2], plane: &Plane, axis_origin: Vec2, axis_dir: Vec2, angle: f64, segments: usize) -> Result<Mesh, GeomError> {
    let pts = poly::ccw(&poly::cleaned(profile));
    if pts.len() < 3 {
        return Err(GeomError::DegenerateProfile);
    }
    let dir2 = axis_dir.normalized().ok_or(GeomError::DegenerateProfile)?;
    if !angle.is_finite() || angle.abs() < 1e-6 {
        return Err(GeomError::OutOfRange(format!("revolve angle {angle}")));
    }
    // All points must be on one side of the axis.
    let side: Vec<f64> = pts.iter().map(|p| dir2.cross(*p - axis_origin)).collect();
    let has_pos = side.iter().any(|s| *s > EPS);
    let has_neg = side.iter().any(|s| *s < -EPS);
    if has_pos && has_neg {
        return Err(GeomError::CrossesAxis);
    }
    let full = angle.abs() >= std::f64::consts::TAU - 1e-9;
    let angle = angle.clamp(-std::f64::consts::TAU, std::f64::consts::TAU);
    let steps = ((segments.clamp(8, 512) as f64) * angle.abs() / std::f64::consts::TAU).ceil().max(2.0) as usize;
    let o = plane.to_world(axis_origin);
    let axis = (plane.to_world(axis_origin + dir2) - o).normalized_or(Vec3::Z);
    let base: Vec<Vec3> = pts.iter().map(|p| plane.to_world(*p)).collect();
    let ring = |k: usize| -> Vec<Vec3> {
        let a = if full && k == steps { 0.0 } else { angle * k as f64 / steps as f64 };
        base.iter().map(|p| o + (*p - o).rotated_about(axis, a)).collect()
    };
    // Sweep direction at angle 0 relative to the plane normal decides the winding: walls are
    // built as edge × sweep, which faces outward when the sweep runs along +normal.
    let sweep_sign = base
        .iter()
        .map(|p| axis.cross(*p - o).dot(plane.normal) * angle.signum())
        .fold(0.0_f64, |acc, v| if v.abs() > acc.abs() { v } else { acc });
    let walls_ok = sweep_sign > 0.0;
    let n = base.len();
    let mut out = Vec::new();
    let mut prev = ring(0);
    for k in 1..=steps {
        let next = ring(k);
        for i in 0..n {
            let j = (i + 1) % n;
            let (Some(a), Some(b), Some(c), Some(d)) = (prev.get(i), prev.get(j), next.get(j), next.get(i)) else { continue };
            if walls_ok {
                out.push(Triangle::new(*a, *b, *c));
                out.push(Triangle::new(*a, *c, *d));
            } else {
                out.push(Triangle::new(*a, *c, *b));
                out.push(Triangle::new(*a, *d, *c));
            }
        }
        prev = next;
    }
    if !full {
        let tris = poly::triangulate(&pts)?;
        let first = ring(0);
        let last = ring(steps);
        for [a, b, c] in tris {
            let (Some(pa), Some(pb), Some(pc)) = (first.get(a), first.get(b), first.get(c)) else { continue };
            let (Some(qa), Some(qb), Some(qc)) = (last.get(a), last.get(b), last.get(c)) else { continue };
            if walls_ok {
                out.push(Triangle::new(*pa, *pc, *pb));
                out.push(Triangle::new(*qa, *qb, *qc));
            } else {
                out.push(Triangle::new(*pa, *pb, *pc));
                out.push(Triangle::new(*qa, *qc, *qb));
            }
        }
    }
    let mut m = Mesh::new(out).cleaned();
    m.orient_outward();
    Ok(m)
}

/// Axis-aligned box from its minimum corner and size.
pub fn cuboid(min: Vec3, size: Vec3) -> Mesh {
    let plane = Plane::new(min, Vec3::Z, Vec3::X);
    let rect = [Vec2::new(0.0, 0.0), Vec2::new(size.x, 0.0), Vec2::new(size.x, size.y), Vec2::new(0.0, size.y)];
    extrude(&rect, &plane, 0.0, size.z).unwrap_or_default()
}

/// Cylinder with its base centre at `base`, along unit `axis`.
pub fn cylinder(base: Vec3, axis: Vec3, radius: f64, height: f64, segments: usize) -> Mesh {
    let x = axis.any_perpendicular();
    let plane = Plane::new(base, axis, x);
    extrude(&poly::circle(Vec2::ZERO, radius, segments), &plane, 0.0, height).unwrap_or_default()
}

/// Cone or frustum: base radius `r0`, top radius `r1` (may be 0).
pub fn cone(base: Vec3, axis: Vec3, r0: f64, r1: f64, height: f64, segments: usize) -> Mesh {
    let x = axis.any_perpendicular();
    let plane = Plane::new(base, x.cross(axis), x);
    let mut prof = vec![Vec2::new(0.0, 0.0), Vec2::new(r0.abs(), 0.0)];
    if r1.abs() > EPS {
        prof.push(Vec2::new(r1.abs(), height));
    }
    prof.push(Vec2::new(0.0, height));
    revolve(&prof, &plane, Vec2::ZERO, Vec2::new(0.0, 1.0), std::f64::consts::TAU, segments).unwrap_or_default()
}

pub fn sphere(center: Vec3, radius: f64, segments: usize) -> Mesh {
    let plane = Plane::new(center, -Vec3::Y, Vec3::X);
    let n = (segments / 2).clamp(6, 256);
    let prof: Vec<Vec2> = (0..=n)
        .map(|i| {
            let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / n as f64;
            Vec2::new(radius * a.cos(), radius * a.sin())
        })
        .collect();
    revolve(&prof, &plane, Vec2::ZERO, Vec2::new(0.0, 1.0), std::f64::consts::TAU, segments).unwrap_or_default()
}

pub fn torus(center: Vec3, axis: Vec3, major: f64, minor: f64, segments: usize) -> Mesh {
    let x = axis.any_perpendicular();
    let plane = Plane::new(center, x.cross(axis), x);
    let prof = poly::circle(Vec2::new(major, 0.0), minor.min(major * 0.999), segments / 2);
    revolve(&prof, &plane, Vec2::ZERO, Vec2::new(0.0, 1.0), std::f64::consts::TAU, segments).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn box_volume() {
        let b = cuboid(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0));
        assert!((b.volume() - 24.0).abs() < 1e-9);
        assert_eq!(b.tris.len(), 12);
    }

    #[test]
    fn negative_extrude_and_planes() {
        let sq = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        for plane in [Plane::xy(), Plane::xz(), Plane::yz()] {
            let m = extrude(&sq, &plane, 0.0, -5.0).unwrap_or_default();
            assert!((m.volume() - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn revolve_volumes() {
        // Rectangle 1..2 × 0..1 around the y axis: a ring, V = π (R² − r²) h.
        let rect = [Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(1.0, 1.0)];
        let full = revolve(&rect, &Plane::xz(), Vec2::ZERO, Vec2::new(0.0, 1.0), 2.0 * PI, 256).unwrap_or_default();
        assert!((full.volume() - 3.0 * PI).abs() < 0.01, "{}", full.volume());
        let half = revolve(&rect, &Plane::xz(), Vec2::ZERO, Vec2::new(0.0, 1.0), PI, 256).unwrap_or_default();
        assert!((half.volume() - 1.5 * PI).abs() < 0.01, "{}", half.volume());
        let neg = revolve(&rect, &Plane::xz(), Vec2::ZERO, Vec2::new(0.0, 1.0), -PI / 2.0, 256).unwrap_or_default();
        assert!((neg.volume() - 0.75 * PI).abs() < 0.01, "{}", neg.volume());
        let crossing = [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)];
        assert_eq!(revolve(&crossing, &Plane::xy(), Vec2::ZERO, Vec2::new(0.0, 1.0), PI, 32), Err(GeomError::CrossesAxis));
    }

    #[test]
    fn primitives() {
        let s = sphere(Vec3::ZERO, 1.0, 128);
        assert!((s.volume() - 4.0 / 3.0 * PI).abs() < 0.02, "{}", s.volume());
        let c = cylinder(Vec3::ZERO, Vec3::Z, 1.0, 2.0, 256);
        assert!((c.volume() - 2.0 * PI).abs() < 0.01);
        let t = torus(Vec3::ZERO, Vec3::Z, 3.0, 1.0, 128);
        assert!((t.volume() - 2.0 * PI * PI * 3.0).abs() < 0.5, "{}", t.volume());
        let k = cone(Vec3::ZERO, Vec3::Z, 1.0, 0.0, 3.0, 256);
        assert!((k.volume() - PI).abs() < 0.01, "{}", k.volume());
    }
}
