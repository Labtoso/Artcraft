//! A plane with a 2D coordinate frame, used for sketches.

use crate::vec::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: Vec3,
    /// Unit x direction of the plane's 2D frame.
    pub x_axis: Vec3,
    /// Unit y direction of the plane's 2D frame.
    pub y_axis: Vec3,
    /// Unit normal (`x_axis × y_axis`).
    pub normal: Vec3,
}

impl Plane {
    /// Build an orthonormal frame from an origin, a normal and a preferred x direction.
    pub fn new(origin: Vec3, normal: Vec3, x_hint: Vec3) -> Self {
        let normal = normal.normalized_or(Vec3::Z);
        let x = x_hint - normal * x_hint.dot(normal);
        let x_axis = x.normalized().unwrap_or_else(|| normal.any_perpendicular());
        let y_axis = normal.cross(x_axis).normalized_or(Vec3::Y);
        Self { origin, x_axis, y_axis, normal }
    }
    /// Top: the XY plane, seen from +Z.
    pub fn xy() -> Self {
        Self::new(Vec3::ZERO, Vec3::Z, Vec3::X)
    }
    /// Front: the XZ plane, seen from -Y (normal -Y so x right, z up).
    pub fn xz() -> Self {
        Self::new(Vec3::ZERO, -Vec3::Y, Vec3::X)
    }
    /// Right: the YZ plane, seen from +X.
    pub fn yz() -> Self {
        Self::new(Vec3::ZERO, Vec3::X, Vec3::Y)
    }
    pub fn to_world(&self, p: Vec2) -> Vec3 {
        self.origin + self.x_axis * p.x + self.y_axis * p.y
    }
    pub fn to_local(&self, p: Vec3) -> Vec2 {
        let d = p - self.origin;
        Vec2::new(d.dot(self.x_axis), d.dot(self.y_axis))
    }
    pub fn signed_distance(&self, p: Vec3) -> f64 {
        (p - self.origin).dot(self.normal)
    }
    /// Where the ray `origin + t·dir` meets the plane (t ≥ 0), if it does.
    pub fn intersect_ray(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let denom = dir.dot(self.normal);
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = (self.origin - origin).dot(self.normal) / denom;
        (t.is_finite() && t >= 0.0).then(|| origin + dir * t)
    }
    pub fn is_valid(&self) -> bool {
        self.origin.is_finite() && self.normal.is_finite() && (self.normal.length() - 1.0).abs() < 1e-6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_ray() {
        let p = Plane::xz();
        let w = p.to_world(Vec2::new(3.0, 4.0));
        assert!((w.z - 4.0).abs() < 1e-12 && (w.x - 3.0).abs() < 1e-12);
        assert!(p.to_local(w).distance(Vec2::new(3.0, 4.0)) < 1e-12);
        let hit = Plane::xy().intersect_ray(Vec3::new(1.0, 2.0, 10.0), -Vec3::Z);
        assert_eq!(hit, Some(Vec3::new(1.0, 2.0, 0.0)));
    }
}
