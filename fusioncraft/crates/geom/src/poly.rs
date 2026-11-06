//! Simple polygons in 2D: area, containment and ear-clipping triangulation.

use crate::vec::Vec2;
use crate::{EPS, GeomError};

/// Twice the signed area is avoided; this is the real signed area (CCW positive).
pub fn signed_area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        if let (Some(p), Some(q)) = (pts.get(i), pts.get((i + 1) % n)) {
            a += p.cross(*q);
        }
    }
    a * 0.5
}

/// Remove consecutive duplicates (also last == first) and collinear points.
pub fn cleaned(pts: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for &p in pts {
        if !p.is_finite() {
            continue;
        }
        if out.last().is_none_or(|l| l.distance(p) > EPS) {
            out.push(p);
        }
    }
    while out.len() > 1 && out.first().zip(out.last()).is_some_and(|(a, b)| a.distance(*b) <= EPS) {
        out.pop();
    }
    // Drop collinear vertices.
    let mut changed = true;
    while changed && out.len() > 3 {
        changed = false;
        let n = out.len();
        for i in 0..n {
            let (Some(a), Some(b), Some(c)) = (out.get((i + n - 1) % n), out.get(i), out.get((i + 1) % n)) else {
                continue;
            };
            if (*b - *a).cross(*c - *b).abs() < EPS * (a.distance(*c) + 1.0) {
                out.remove(i);
                changed = true;
                break;
            }
        }
    }
    out
}

/// Counter-clockwise copy of a polygon.
pub fn ccw(pts: &[Vec2]) -> Vec<Vec2> {
    let mut v = pts.to_vec();
    if signed_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Even-odd point-in-polygon test.
pub fn contains(pts: &[Vec2], p: Vec2) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (Some(a), Some(b)) = (pts.get(i), pts.get((i + n - 1) % n)) else { continue };
        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// A point strictly inside the polygon (used to test nesting of loops).
pub fn interior_point(pts: &[Vec2]) -> Option<Vec2> {
    let tris = triangulate(pts).ok()?;
    let mut best: Option<(f64, Vec2)> = None;
    for [a, b, c] in tris {
        let (Some(a), Some(b), Some(c)) = (pts.get(a), pts.get(b), pts.get(c)) else { continue };
        let area = (*b - *a).cross(*c - *a).abs();
        if best.is_none_or(|(ba, _)| area > ba) {
            best = Some((area, (*a + *b + *c) / 3.0));
        }
    }
    best.map(|(_, p)| p)
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (b - a).cross(p - a);
    let d2 = (c - b).cross(p - b);
    let d3 = (a - c).cross(p - c);
    d1 >= -EPS && d2 >= -EPS && d3 >= -EPS
}

/// Ear-clipping triangulation of a simple polygon. Returns index triples into `pts`, each
/// counter-clockwise. The polygon may be in either orientation.
pub fn triangulate(pts: &[Vec2]) -> Result<Vec<[usize; 3]>, GeomError> {
    let n = pts.len();
    if n < 3 || signed_area(pts).abs() < EPS {
        return Err(GeomError::DegenerateProfile);
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(pts) < 0.0 {
        idx.reverse();
    }
    let at = |i: usize| pts.get(i).copied().unwrap_or(Vec2::ZERO);
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0usize;
    while idx.len() > 3 {
        guard += 1;
        if guard > n * n + 16 {
            // Self-intersecting input: fall back to a fan so the caller still gets a cap.
            break;
        }
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (Some(&ia), Some(&ib), Some(&ic)) = (idx.get((i + m - 1) % m), idx.get(i), idx.get((i + 1) % m)) else {
                continue;
            };
            let (a, b, c) = (at(ia), at(ib), at(ic));
            if (b - a).cross(c - b) <= EPS {
                continue; // reflex or flat
            }
            let blocked = idx.iter().any(|&j| {
                j != ia && j != ib && j != ic && {
                    let p = at(j);
                    p.distance(a) > EPS && p.distance(b) > EPS && p.distance(c) > EPS && point_in_triangle(p, a, b, c)
                }
            });
            if !blocked {
                out.push([ia, ib, ic]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Only flat/reflex corners left: drop one flat vertex if any, otherwise fan.
            let flat = (0..m).find(|&i| {
                let (Some(&ia), Some(&ib), Some(&ic)) = (idx.get((i + m - 1) % m), idx.get(i), idx.get((i + 1) % m)) else {
                    return false;
                };
                (at(ib) - at(ia)).cross(at(ic) - at(ib)).abs() <= EPS
            });
            match flat {
                Some(i) => {
                    idx.remove(i);
                }
                None => break,
            }
        }
    }
    if idx.len() >= 3 {
        if let Some(&first) = idx.first() {
            for w in idx.windows(2).skip(1) {
                if let [b, c] = *w {
                    out.push([first, b, c]);
                }
            }
        }
    }
    Ok(out)
}

/// A regular polygon approximating a circle.
pub fn circle(center: Vec2, radius: f64, segments: usize) -> Vec<Vec2> {
    let n = segments.clamp(8, 512);
    (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            center + Vec2::new(a.cos(), a.sin()) * radius
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_shape_triangulates() {
        let l = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let t = triangulate(&l).unwrap_or_default();
        assert_eq!(t.len(), 4);
        let area: f64 = t
            .iter()
            .map(|[a, b, c]| (l[*b] - l[*a]).cross(l[*c] - l[*a]) * 0.5)
            .sum();
        assert!((area - 3.0).abs() < 1e-9);
        assert!(contains(&l, Vec2::new(0.5, 1.5)));
        assert!(!contains(&l, Vec2::new(1.5, 1.5)));
    }

    #[test]
    fn degenerate_is_error() {
        assert!(triangulate(&[Vec2::ZERO, Vec2::new(1.0, 0.0)]).is_err());
        assert!(triangulate(&[Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)]).is_err());
    }
}
