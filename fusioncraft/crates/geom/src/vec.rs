//! Small `f64` vector types.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y
    }
    /// The z component of the 3D cross product.
    pub fn cross(self, o: Self) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn distance(self, o: Self) -> f64 {
        (self - o).length()
    }
    pub fn normalized(self) -> Option<Self> {
        let l = self.length();
        (l > crate::EPS && l.is_finite()).then(|| self / l)
    }
    pub fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }
    pub fn lerp(self, o: Self, t: f64) -> Self {
        self + (o - self) * t
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    pub fn rotated(self, angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn distance(self, o: Self) -> f64 {
        (self - o).length()
    }
    pub fn normalized(self) -> Option<Self> {
        let l = self.length();
        (l > crate::EPS && l.is_finite()).then(|| self / l)
    }
    /// Normalized, or `fallback` for a zero vector.
    pub fn normalized_or(self, fallback: Self) -> Self {
        self.normalized().unwrap_or(fallback)
    }
    pub fn lerp(self, o: Self, t: f64) -> Self {
        self + (o - self) * t
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    pub fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    pub fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    /// A unit vector perpendicular to this one.
    pub fn any_perpendicular(self) -> Self {
        let a = if self.x.abs() < 0.9 { Self::X } else { Self::Y };
        self.cross(a).normalized_or(Self::Z)
    }
    /// Rotate around the unit `axis` through the origin by `angle` radians (Rodrigues).
    pub fn rotated_about(self, axis: Self, angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        self * c + axis.cross(self) * s + axis * (axis.dot(self) * (1.0 - c))
    }
    pub fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

macro_rules! ops {
    ($t:ident, $($f:ident),+) => {
        impl Add for $t { type Output = Self; fn add(self, o: Self) -> Self { Self { $($f: self.$f + o.$f),+ } } }
        impl Sub for $t { type Output = Self; fn sub(self, o: Self) -> Self { Self { $($f: self.$f - o.$f),+ } } }
        impl Mul<f64> for $t { type Output = Self; fn mul(self, s: f64) -> Self { Self { $($f: self.$f * s),+ } } }
        impl Div<f64> for $t { type Output = Self; fn div(self, s: f64) -> Self { Self { $($f: self.$f / s),+ } } }
        impl Neg for $t { type Output = Self; fn neg(self) -> Self { Self { $($f: -self.$f),+ } } }
        impl AddAssign for $t { fn add_assign(&mut self, o: Self) { $(self.$f += o.$f;)+ } }
        impl SubAssign for $t { fn sub_assign(&mut self, o: Self) { $(self.$f -= o.$f;)+ } }
    };
}
ops!(Vec2, x, y);
ops!(Vec3, x, y, z);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_and_rotate() {
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        let r = Vec3::X.rotated_about(Vec3::Z, std::f64::consts::FRAC_PI_2);
        assert!(r.distance(Vec3::Y) < 1e-12);
        assert!(Vec3::ZERO.normalized().is_none());
    }
}
