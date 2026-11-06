//! Profiles: the closed regions of a sketch that features extrude or revolve.
//!
//! Closed entities (rectangles, circles, polygons) are loops on their own; lines and arcs are
//! chained end to end into loops. A loop inside another loop is a hole of its parent region and
//! a region of its own, just like in Fusion where a circle inside a rectangle gives two
//! selectable profiles.

use crate::sketch::Sketch;
use fusioncraft_geom::{poly, Vec2};

/// Join tolerance for chaining end points (mm).
pub const JOIN_TOL: f64 = 1e-4;

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Outer boundary, counter-clockwise.
    pub outer: Vec<Vec2>,
    /// Holes, each counter-clockwise (they are subtracted).
    pub holes: Vec<Vec<Vec2>>,
}

impl Profile {
    pub fn area(&self) -> f64 {
        poly::signed_area(&self.outer).abs() - self.holes.iter().map(|h| poly::signed_area(h).abs()).sum::<f64>()
    }
    /// Is the point inside the region (inside outer, outside every hole)?
    pub fn contains(&self, p: Vec2) -> bool {
        poly::contains(&self.outer, p) && !self.holes.iter().any(|h| poly::contains(h, p))
    }
}

/// All closed loops of a sketch.
pub fn loops(sketch: &Sketch) -> Vec<Vec<Vec2>> {
    let mut out: Vec<Vec<Vec2>> = Vec::new();
    let mut paths: Vec<Vec<Vec2>> = Vec::new();
    for e in &sketch.entities {
        if !e.is_finite() {
            continue;
        }
        if let Some(l) = e.closed_loop() {
            out.push(l);
        } else if let Some(p) = e.open_path() {
            if p.len() >= 2 {
                paths.push(p);
            }
        }
    }
    out.extend(chain(paths));
    out.into_iter()
        .map(|l| poly::ccw(&poly::cleaned(&l)))
        .filter(|l| l.len() >= 3 && poly::signed_area(l).abs() > 1e-9)
        .collect()
}

/// Chain open paths into closed loops. Each path is used at most once; loops are found by
/// walking from a path's end to a path that starts (or ends) at the same point.
fn chain(mut paths: Vec<Vec<Vec2>>) -> Vec<Vec<Vec2>> {
    let mut loops = Vec::new();
    let near = |a: Vec2, b: Vec2| a.distance(b) <= JOIN_TOL;
    let mut guard = 0usize;
    while let Some(first) = paths.pop() {
        guard += 1;
        if guard > 100_000 {
            break;
        }
        let Some(&start) = first.first() else { continue };
        let mut current = first;
        let mut closed = current.last().is_some_and(|l| near(*l, start)) && current.len() > 2;
        let mut progress = true;
        while !closed && progress {
            progress = false;
            let Some(&end) = current.last() else { break };
            if let Some(i) = paths.iter().position(|p| p.first().is_some_and(|f| near(*f, end)) || p.last().is_some_and(|l| near(*l, end))) {
                let mut next = paths.remove(i);
                if !next.first().is_some_and(|f| near(*f, end)) {
                    next.reverse();
                }
                current.extend(next.into_iter().skip(1));
                progress = true;
                closed = current.last().is_some_and(|l| near(*l, start));
            }
        }
        if closed {
            current.pop();
            loops.push(current);
        }
    }
    loops
}

/// The selectable regions of a sketch, in a stable order (largest first).
pub fn profiles(sketch: &Sketch) -> Vec<Profile> {
    let ls = loops(sketch);
    let inner_pts: Vec<Option<Vec2>> = ls.iter().map(|l| poly::interior_point(l)).collect();
    // parent[i] = smallest loop containing loop i.
    let area = |l: &Vec<Vec2>| poly::signed_area(l).abs();
    let mut parent: Vec<Option<usize>> = vec![None; ls.len()];
    for (i, li) in ls.iter().enumerate() {
        let Some(p) = inner_pts.get(i).copied().flatten() else { continue };
        let mut best: Option<(usize, f64)> = None;
        for (j, lj) in ls.iter().enumerate() {
            if i == j || area(lj) <= area(li) {
                continue;
            }
            // Containment of the whole loop: test its vertices too, not just one point.
            if poly::contains(lj, p) && li.iter().all(|q| poly::contains(lj, *q) || lj.iter().any(|v| v.distance(*q) < JOIN_TOL)) {
                let a = area(lj);
                if best.is_none_or(|(_, ba)| a < ba) {
                    best = Some((j, a));
                }
            }
        }
        if let Some(slot) = parent.get_mut(i) {
            *slot = best.map(|(j, _)| j);
        }
    }
    let mut out: Vec<Profile> = ls
        .iter()
        .enumerate()
        .map(|(i, l)| Profile {
            outer: l.clone(),
            holes: ls
                .iter()
                .enumerate()
                .filter(|(j, _)| parent.get(*j).copied().flatten() == Some(i))
                .map(|(_, h)| h.clone())
                .collect(),
        })
        .collect();
    out.sort_by(|a, b| b.area().total_cmp(&a.area()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::PlaneRef;
    use crate::sketch::{Entity, SketchPlane};

    fn sketch(entities: Vec<Entity>) -> Sketch {
        let mut s = Sketch::new(1, "Sketch1".into(), SketchPlane::Origin { plane: PlaneRef::Xy });
        s.entities = entities;
        s
    }

    #[test]
    fn rect_with_circle_gives_two_regions() {
        let s = sketch(vec![
            Entity::Rect { a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 10.0) },
            Entity::Circle { center: Vec2::new(5.0, 5.0), radius: 2.0 },
        ]);
        let p = profiles(&s);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].holes.len(), 1);
        assert!(p[0].contains(Vec2::new(1.0, 1.0)));
        assert!(!p[0].contains(Vec2::new(5.0, 5.0)));
        assert!(p[1].contains(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn lines_chain_into_triangle() {
        let (a, b, c) = (Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(0.0, 3.0));
        let s = sketch(vec![
            Entity::Line { a, b, construction: false },
            Entity::Line { a: a, b: c, construction: false },
            Entity::Line { a: b, b: c, construction: false },
        ]);
        let p = profiles(&s);
        assert_eq!(p.len(), 1);
        assert!((p[0].area() - 6.0).abs() < 1e-9);
        // An open chain is no profile; construction lines never are.
        let open = sketch(vec![Entity::Line { a, b, construction: false }, Entity::Line { a: b, b: c, construction: false }]);
        assert!(profiles(&open).is_empty());
    }

    #[test]
    fn arc_closes_a_slot() {
        let s = sketch(vec![
            Entity::Line { a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), construction: false },
            Entity::Arc { start: Vec2::new(10.0, 0.0), mid: Vec2::new(12.0, 2.0), end: Vec2::new(10.0, 4.0) },
            Entity::Line { a: Vec2::new(10.0, 4.0), b: Vec2::new(0.0, 4.0), construction: false },
            Entity::Line { a: Vec2::new(0.0, 4.0), b: Vec2::new(0.0, 0.0), construction: false },
        ]);
        let p = profiles(&s);
        assert_eq!(p.len(), 1);
        let expected = 40.0 + std::f64::consts::PI * 4.0 / 2.0;
        assert!((p[0].area() - expected).abs() < 0.05, "{}", p[0].area());
    }
}
