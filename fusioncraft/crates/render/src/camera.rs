//! Orbit camera, Z up (like Fusion with "Z up" set).

use fusioncraft_geom::{Aabb, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub target: Vec3,
    /// Around Z, radians. 0 = looking from +X.
    pub yaw: f64,
    /// Elevation, radians, in (-π/2, π/2].
    pub pitch: f64,
    pub distance: f64,
    pub fov_deg: f64,
    pub ortho: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdView {
    Home,
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
}

impl StdView {
    pub fn angles(self) -> (f64, f64) {
        use std::f64::consts::FRAC_PI_2;
        match self {
            StdView::Home => ((-60f64).to_radians(), 28f64.to_radians()),
            StdView::Top => (-FRAC_PI_2, FRAC_PI_2),
            StdView::Bottom => (-FRAC_PI_2, -FRAC_PI_2),
            StdView::Front => (-FRAC_PI_2, 0.0),
            StdView::Back => (FRAC_PI_2, 0.0),
            StdView::Right => (0.0, 0.0),
            StdView::Left => (std::f64::consts::PI, 0.0),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            StdView::Home => "Home",
            StdView::Top => "Top",
            StdView::Bottom => "Bottom",
            StdView::Front => "Front",
            StdView::Back => "Back",
            StdView::Left => "Left",
            StdView::Right => "Right",
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        let (yaw, pitch) = StdView::Home.angles();
        Self { target: Vec3::ZERO, yaw, pitch, distance: 200.0, fov_deg: 35.0, ortho: false }
    }
}

const PITCH_LIMIT: f64 = std::f64::consts::FRAC_PI_2;

impl Camera {
    pub fn eye(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        self.target + Vec3::new(cp * cy, cp * sy, sp) * self.distance
    }
    pub fn forward(&self) -> Vec3 {
        (self.target - self.eye()).normalized_or(-Vec3::Z)
    }
    pub fn right(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), self.yaw.cos(), 0.0)
    }
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalized_or(Vec3::Z)
    }
    /// Direction from the eye to a point (for perspective), or the view direction (ortho).
    pub fn view_dir_to(&self, p: Vec3) -> Vec3 {
        if self.ortho { self.forward() } else { (p - self.eye()).normalized_or(self.forward()) }
    }
    fn near(&self) -> f64 {
        (self.distance * 0.002).max(1e-4)
    }
    fn half_height(&self) -> f64 {
        (self.fov_deg.to_radians() * 0.5).tan()
    }
    /// View-space coordinates: (right, up, depth along forward).
    fn view(&self, p: Vec3) -> (f64, f64, f64) {
        let d = p - self.eye();
        (d.dot(self.right()), d.dot(self.up()), d.dot(self.forward()))
    }
    /// Screen position and depth key (larger = closer). `None` behind the near plane.
    pub fn project(&self, p: Vec3, w: f64, h: f64) -> Option<(f64, f64, f64)> {
        let (x, y, z) = self.view(p);
        let hh = self.half_height();
        let s = h * 0.5;
        let (sx, sy, key) = if self.ortho {
            let k = 1.0 / (self.distance * hh);
            (x * k, y * k, -z)
        } else {
            if z < self.near() * 0.999 {
                return None;
            }
            let k = 1.0 / (z * hh);
            (x * k, y * k, 1.0 / z)
        };
        let out = (w * 0.5 + sx * s, h * 0.5 - sy * s, key);
        (out.0.is_finite() && out.1.is_finite() && out.2.is_finite()).then_some(out)
    }
    /// The ray under a screen point: origin and unit direction.
    pub fn ray(&self, px: f64, py: f64, w: f64, h: f64) -> (Vec3, Vec3) {
        let s = h.max(1.0) * 0.5;
        let nx = (px - w * 0.5) / s;
        let ny = (h * 0.5 - py) / s;
        let hh = self.half_height();
        if self.ortho {
            let o = self.eye() + self.right() * (nx * hh * self.distance) + self.up() * (ny * hh * self.distance);
            (o, self.forward())
        } else {
            let d = (self.forward() + self.right() * (nx * hh) + self.up() * (ny * hh)).normalized_or(self.forward());
            (self.eye(), d)
        }
    }
    /// World units per pixel at the target distance.
    pub fn units_per_pixel(&self, h: f64) -> f64 {
        2.0 * self.distance * self.half_height() / h.max(1.0)
    }

    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw = (self.yaw + dyaw) % std::f64::consts::TAU;
        self.pitch = (self.pitch + dpitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }
    pub fn pan(&mut self, dx_px: f64, dy_px: f64, h: f64) {
        let u = self.units_per_pixel(h);
        self.target = self.target - self.right() * (dx_px * u) + self.up() * (dy_px * u);
    }
    /// Zoom by a factor towards a world point under the cursor.
    pub fn zoom_at(&mut self, factor: f64, focus: Option<Vec3>) {
        let f = factor.clamp(0.05, 20.0);
        let new_d = (self.distance * f).clamp(1e-3, 1e7);
        if let Some(p) = focus {
            let k = 1.0 - new_d / self.distance;
            self.target = self.target + (p - self.target) * k;
        }
        self.distance = new_d;
    }
    pub fn set_view(&mut self, v: StdView) {
        let (y, p) = v.angles();
        self.yaw = y;
        self.pitch = p;
    }
    /// Look straight at a plane with the given normal and in-plane up.
    pub fn look_along(&mut self, normal: Vec3) {
        let n = normal.normalized_or(Vec3::Z);
        self.pitch = n.z.clamp(-1.0, 1.0).asin();
        if n.x.abs() > 1e-9 || n.y.abs() > 1e-9 {
            self.yaw = n.y.atan2(n.x);
        } else {
            self.yaw = -std::f64::consts::FRAC_PI_2;
        }
    }
    pub fn fit(&mut self, b: &Aabb) {
        if b.is_empty() {
            self.target = Vec3::ZERO;
            self.distance = 200.0;
            return;
        }
        self.target = b.center();
        let r = (b.size().length() * 0.5).max(1.0);
        self.distance = (r / (self.fov_deg.to_radians() * 0.5).sin() * 1.1).clamp(1.0, 1e7);
    }

    /// Clip a triangle against the near plane (perspective only).
    pub fn clip_triangle(&self, v: [Vec3; 3]) -> Vec<Vec<Vec3>> {
        if self.ortho {
            return vec![v.to_vec()];
        }
        let near = self.near();
        let z: Vec<f64> = v.iter().map(|p| self.view(*p).2).collect();
        if z.iter().all(|z| *z >= near) {
            return vec![v.to_vec()];
        }
        if z.iter().all(|z| *z < near) {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            let (Some(&zi), Some(&zj), Some(&pi), Some(&pj)) = (z.get(i), z.get(j), v.get(i), v.get(j)) else { continue };
            if zi >= near {
                out.push(pi);
            }
            if (zi >= near) != (zj >= near) {
                let t = (near - zi) / (zj - zi);
                out.push(pi.lerp(pj, t));
            }
        }
        vec![out]
    }

    pub fn clip_segment(&self, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
        if self.ortho {
            return Some((a, b));
        }
        let near = self.near();
        let (za, zb) = (self.view(a).2, self.view(b).2);
        match (za >= near, zb >= near) {
            (true, true) => Some((a, b)),
            (false, false) => None,
            (true, false) => Some((a, a.lerp(b, (near - za) / (zb - za)))),
            (false, true) => Some((a.lerp(b, (near - za) / (zb - za)), b)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_and_projection() {
        let mut c = Camera::default();
        c.set_view(StdView::Front);
        assert!(c.right().distance(Vec3::X) < 1e-9);
        assert!(c.up().distance(Vec3::Z) < 1e-9);
        c.set_view(StdView::Top);
        assert!(c.up().distance(Vec3::Y) < 1e-9);
        let (x, y, _) = c.project(Vec3::ZERO, 200.0, 100.0).unwrap_or_default();
        assert!((x - 100.0).abs() < 1e-6 && (y - 50.0).abs() < 1e-6);
        let (o, d) = c.ray(100.0, 50.0, 200.0, 100.0);
        assert!((o + d * c.distance).distance(Vec3::ZERO) < 1e-6);
    }
}
