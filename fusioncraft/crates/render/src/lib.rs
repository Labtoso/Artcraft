//! Software renderer (layer L2). Draws bodies with a depth buffer, their edges with hidden
//! lines removed, the ground grid and extra lines, and fills a pick buffer that tells which body
//! and triangle is under each pixel. It needs no GPU, so it runs everywhere and in tests.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod camera;
pub mod png;

pub use camera::{Camera, StdView};

use fusioncraft_geom::{Mesh, Vec3};
use fusioncraft_model::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }
    #[allow(clippy::self_named_constructors)]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }
    fn scaled(self, k: f64) -> [f64; 3] {
        [f64::from(self.0[0]) * k, f64::from(self.0[1]) * k, f64::from(self.0[2]) * k]
    }
}

/// Colours of the viewport (dark Craft-suite canvas).
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub background: Rgba,
    pub body: Rgba,
    pub body_selected: Rgba,
    pub body_hover: Rgba,
    pub edge: Rgba,
    pub grid_minor: Rgba,
    pub grid_major: Rgba,
    pub axis_x: Rgba,
    pub axis_y: Rgba,
    pub axis_z: Rgba,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Rgba::rgb(0x22, 0x28, 0x2f),
            body: Rgba::rgb(0xb8, 0xbe, 0xc6),
            body_selected: Rgba::rgb(0x7d, 0xb0, 0xff),
            body_hover: Rgba::rgb(0xa8, 0xc8, 0xf0),
            edge: Rgba::rgba(0x14, 0x18, 0x1d, 235),
            grid_minor: Rgba::rgba(0x3a, 0x43, 0x4f, 255),
            grid_major: Rgba::rgba(0x4a, 0x55, 0x63, 255),
            axis_x: Rgba::rgb(0xd0, 0x4a, 0x4a),
            axis_y: Rgba::rgb(0x52, 0xb0, 0x5c),
            axis_z: Rgba::rgb(0x4a, 0x7c, 0xd8),
        }
    }
}

/// A line drawn on top of the scene.
#[derive(Debug, Clone, Copy)]
pub struct Line3 {
    pub a: Vec3,
    pub b: Vec3,
    pub color: Rgba,
    /// true: hidden behind bodies; false: always visible.
    pub depth_test: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    ShadedWithEdges,
    Shaded,
    Wireframe,
}

pub struct Scene<'a> {
    pub model: &'a Model,
    pub camera: &'a Camera,
    pub style: Style,
    pub mode: DisplayMode,
    /// Body index (Body::index) to highlight.
    pub selected: &'a [usize],
    pub hover: Option<usize>,
    pub hidden: &'a [usize],
    pub grid: bool,
    pub lines: &'a [Line3],
}

/// What is under a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pick {
    /// Position in `Model::bodies`.
    pub body: usize,
    pub triangle: usize,
}

pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// RGBA8, row-major, top row first.
    pub rgba: Vec<u8>,
    pick: Vec<u32>,
    tri: Vec<u32>,
    depth: Vec<f32>,
}

impl Frame {
    pub fn pick(&self, x: usize, y: usize) -> Option<Pick> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = y * self.width + x;
        let b = *self.pick.get(i)?;
        if b == 0 {
            return None;
        }
        Some(Pick { body: (b - 1) as usize, triangle: *self.tri.get(i)? as usize })
    }
    /// Depth key under a pixel (larger is closer), if a body is there.
    pub fn depth_at(&self, x: usize, y: usize) -> Option<f32> {
        (x < self.width && y < self.height).then(|| self.depth.get(y * self.width + x).copied()).flatten().filter(|d| *d > f32::MIN)
    }
}

const MAX_PIXELS: usize = 8192 * 8192;

struct Target {
    w: usize,
    h: usize,
    color: Vec<[f32; 3]>,
    depth: Vec<f32>,
    pick: Vec<u32>,
    tri: Vec<u32>,
}

/// Render the scene at `width × height` with `ssaa × ssaa` supersampling.
pub fn render(scene: &Scene<'_>, width: usize, height: usize, ssaa: usize) -> Frame {
    let width = width.max(1);
    let height = height.max(1);
    let mut s = ssaa.clamp(1, 4);
    while width * height * s * s > MAX_PIXELS && s > 1 {
        s -= 1;
    }
    let (w, h) = (width * s, height * s);
    let bg = scene.style.background.scaled(1.0);
    let bg = [bg[0] as f32, bg[1] as f32, bg[2] as f32];
    let n = w.saturating_mul(h).min(MAX_PIXELS);
    let mut t = Target { w, h, color: vec![bg; n], depth: vec![f32::MIN; n], pick: vec![0; n], tri: vec![0; n] };
    let cam = scene.camera;
    let lw = s as f64;

    if scene.grid {
        draw_grid(&mut t, cam, &scene.style, lw);
    }
    let light_dir = (cam.forward() * -1.0 + cam.up() * 0.6 + cam.right() * 0.35).normalized_or(Vec3::Z);
    for (bi, body) in scene.model.bodies.iter().enumerate() {
        if scene.hidden.contains(&body.index) {
            continue;
        }
        let base = if scene.selected.contains(&body.index) {
            scene.style.body_selected
        } else if scene.hover == Some(body.index) {
            scene.style.body_hover
        } else {
            scene.style.body
        };
        if scene.mode != DisplayMode::Wireframe {
            draw_mesh(&mut t, cam, &body.mesh, base, light_dir, (bi + 1) as u32);
        }
    }
    // Grid after bodies would be hidden correctly too, but drawing it first keeps it behind
    // translucent edges; redraw the axes on top with depth test.
    for (bi, body) in scene.model.bodies.iter().enumerate() {
        let _ = bi;
        if scene.hidden.contains(&body.index) || scene.mode == DisplayMode::Shaded {
            continue;
        }
        let test = scene.mode != DisplayMode::Wireframe;
        let color = if scene.mode == DisplayMode::Wireframe { scene.style.body } else { scene.style.edge };
        for e in &body.edges {
            draw_line(&mut t, cam, e[0], e[1], color, test, lw);
        }
    }
    for l in scene.lines {
        draw_line(&mut t, cam, l.a, l.b, l.color, l.depth_test, lw * 1.5);
    }
    resolve(t, width, height, s)
}

fn resolve(t: Target, width: usize, height: usize, s: usize) -> Frame {
    let mut rgba = vec![0u8; width * height * 4];
    let mut pick = vec![0u32; width * height];
    let mut tri = vec![0u32; width * height];
    let mut depth = vec![f32::MIN; width * height];
    let inv = 1.0 / (s * s) as f32;
    for y in 0..height {
        for x in 0..width {
            let mut acc = [0f32; 3];
            for sy in 0..s {
                for sx in 0..s {
                    let i = (y * s + sy) * t.w + (x * s + sx);
                    if let Some(c) = t.color.get(i) {
                        acc[0] += c[0];
                        acc[1] += c[1];
                        acc[2] += c[2];
                    }
                }
            }
            let o = (y * width + x) * 4;
            if let Some(px) = rgba.get_mut(o..o + 4) {
                px[0] = (acc[0] * inv).clamp(0.0, 255.0) as u8;
                px[1] = (acc[1] * inv).clamp(0.0, 255.0) as u8;
                px[2] = (acc[2] * inv).clamp(0.0, 255.0) as u8;
                px[3] = 255;
            }
            let c = (y * s + s / 2) * t.w + (x * s + s / 2);
            let j = y * width + x;
            if let (Some(p), Some(d), Some(tr)) = (pick.get_mut(j), depth.get_mut(j), tri.get_mut(j)) {
                *p = t.pick.get(c).copied().unwrap_or(0);
                *d = t.depth.get(c).copied().unwrap_or(f32::MIN);
                *tr = t.tri.get(c).copied().unwrap_or(0);
            }
        }
    }
    let _ = t.h;
    Frame { width, height, rgba, pick, tri, depth }
}

/// A vertex after projection: screen x, y and the depth key (larger is closer).
#[derive(Clone, Copy)]
struct Sv {
    x: f64,
    y: f64,
    d: f64,
}

fn draw_mesh(t: &mut Target, cam: &Camera, mesh: &Mesh, base: Rgba, light: Vec3, id: u32) {
    let (w, h) = (t.w as f64, t.h as f64);
    for (ti, tri) in mesh.tris.iter().enumerate() {
        let n = tri.normal();
        // Two-sided lighting: back faces of open meshes are still lit.
        let facing = n.dot(cam.view_dir_to(tri.v[0]));
        let ln = if facing > 0.0 { -n } else { n };
        let diffuse = ln.dot(light).max(0.0);
        let rim = 0.12 * (1.0 - ln.dot(-cam.view_dir_to(tri.v[0])).abs());
        let k = 0.38 + 0.62 * diffuse + rim;
        let c = base.scaled(k);
        let color = [c[0].min(255.0) as f32, c[1].min(255.0) as f32, c[2].min(255.0) as f32];
        for poly in cam.clip_triangle(tri.v) {
            let pts: Vec<Sv> = poly.iter().filter_map(|p| cam.project(*p, w, h).map(|(x, y, d)| Sv { x, y, d })).collect();
            if pts.len() < 3 {
                continue;
            }
            if let Some(&a) = pts.first() {
                for wnd in pts.windows(2).skip(1) {
                    if let [b, c2] = *wnd {
                        raster_tri(t, a, b, c2, color, id, ti as u32);
                    }
                }
            }
        }
    }
}

fn raster_tri(t: &mut Target, a: Sv, b: Sv, c: Sv, color: [f32; 3], id: u32, tri: u32) {
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-12 || !area.is_finite() {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let max_x = (a.x.max(b.x).max(c.x).ceil().min(t.w as f64 - 1.0)).max(0.0) as usize;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
    let max_y = (a.y.max(b.y).max(c.y).ceil().min(t.h as f64 - 1.0)).max(0.0) as usize;
    if min_x > max_x || min_y > max_y {
        return;
    }
    let inv = 1.0 / area;
    for y in min_y..=max_y {
        let py = y as f64 + 0.5;
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let w0 = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) * inv;
            let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                continue;
            }
            let d = (w0 * a.d + w1 * b.d + w2 * c.d) as f32;
            let i = y * t.w + x;
            if let Some(slot) = t.depth.get_mut(i)
                && d > *slot {
                    *slot = d;
                    if let Some(px) = t.color.get_mut(i) {
                        *px = color;
                    }
                    if let Some(p) = t.pick.get_mut(i) {
                        *p = id;
                    }
                    if let Some(p) = t.tri.get_mut(i) {
                        *p = tri;
                    }
                }
        }
    }
}

fn blend(dst: &mut [f32; 3], c: Rgba, alpha: f32) {
    let a = f32::from(c.0[3]) / 255.0 * alpha;
    for (d, s) in dst.iter_mut().zip(c.0) {
        *d = *d * (1.0 - a) + f32::from(s) * a;
    }
}

fn draw_line(t: &mut Target, cam: &Camera, a: Vec3, b: Vec3, color: Rgba, depth_test: bool, width: f64) {
    let Some((a, b)) = cam.clip_segment(a, b) else { return };
    let (w, h) = (t.w as f64, t.h as f64);
    let (Some(pa), Some(pb)) = (cam.project(a, w, h), cam.project(b, w, h)) else { return };
    let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
    let len = dx.abs().max(dy.abs());
    if !len.is_finite() || len > 1e6 {
        return;
    }
    let steps = len.ceil().max(1.0) as usize;
    let half = (width * 0.5).max(0.5);
    let r = half.ceil() as i64;
    // Depth tolerance: edges lie exactly on faces, so accept a small band in front.
    for i in 0..=steps {
        let s = i as f64 / steps as f64;
        let x = pa.0 + dx * s;
        let y = pa.1 + dy * s;
        let d = pa.2 + (pb.2 - pa.2) * s;
        let tol = d.abs() * 2e-3 + 1e-6;
        for oy in -r..=r {
            for ox in -r..=r {
                let fx = x + ox as f64;
                let fy = y + oy as f64;
                let dist = ((ox * ox + oy * oy) as f64).sqrt();
                if dist > half + 0.5 {
                    continue;
                }
                if fx < 0.0 || fy < 0.0 || fx >= w || fy >= h {
                    continue;
                }
                let idx = fy as usize * t.w + fx as usize;
                if depth_test {
                    let Some(&bd) = t.depth.get(idx) else { continue };
                    if f64::from(bd) > d + tol {
                        continue;
                    }
                }
                let cover = (half + 0.5 - dist).clamp(0.0, 1.0) as f32;
                if let Some(px) = t.color.get_mut(idx) {
                    blend(px, color, cover);
                }
            }
        }
    }
}

/// Adaptive grid on the XY plane plus the X and Y axes.
fn draw_grid(t: &mut Target, cam: &Camera, style: &Style, lw: f64) {
    let step = grid_step(cam.distance);
    let extent = step * 50.0;
    let c = cam.target;
    let x0 = ((c.x - extent) / step).floor() as i64;
    let x1 = ((c.x + extent) / step).ceil() as i64;
    let y0 = ((c.y - extent) / step).floor() as i64;
    let y1 = ((c.y + extent) / step).ceil() as i64;
    let (ya, yb) = (y0 as f64 * step, y1 as f64 * step);
    let (xa, xb) = (x0 as f64 * step, x1 as f64 * step);
    for i in x0..=x1 {
        let x = i as f64 * step;
        if i == 0 {
            continue;
        }
        let col = if i % 10 == 0 { style.grid_major } else { style.grid_minor };
        draw_line(t, cam, Vec3::new(x, ya, 0.0), Vec3::new(x, yb, 0.0), col, true, lw);
    }
    for j in y0..=y1 {
        let y = j as f64 * step;
        if j == 0 {
            continue;
        }
        let col = if j % 10 == 0 { style.grid_major } else { style.grid_minor };
        draw_line(t, cam, Vec3::new(xa, y, 0.0), Vec3::new(xb, y, 0.0), col, true, lw);
    }
    draw_line(t, cam, Vec3::new(xa.min(0.0), 0.0, 0.0), Vec3::new(xb.max(0.0), 0.0, 0.0), style.axis_x, true, lw * 1.5);
    draw_line(t, cam, Vec3::new(0.0, ya.min(0.0), 0.0), Vec3::new(0.0, yb.max(0.0), 0.0), style.axis_y, true, lw * 1.5);
}

/// Grid spacing for a camera distance: 1, 2 or 5 times a power of ten.
pub fn grid_step(distance: f64) -> f64 {
    let raw = (distance / 30.0).max(1e-3);
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    let k = if m < 2.0 { 1.0 } else if m < 5.0 { 2.0 } else { 5.0 };
    k * p
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusioncraft_doc::{Document, Expr, Feature, FeatureKind, Operation};

    #[test]
    fn renders_and_picks_a_box() {
        let mut d = Document::new("r");
        let id = d.alloc_id();
        d.insert_feature(Feature {
            id,
            name: "Box1".into(),
            suppressed: false,
            kind: FeatureKind::Box { base: Vec3::ZERO, width: Expr::new("20"), depth: Expr::new("20"), height: Expr::new("20"), operation: Operation::NewBody },
        });
        let model = fusioncraft_model::evaluate_here(&d);
        let mut cam = Camera::default();
        cam.fit(&model.aabb());
        let scene = Scene { model: &model, camera: &cam, style: Style::default(), mode: DisplayMode::ShadedWithEdges, selected: &[], hover: None, hidden: &[], grid: true, lines: &[] };
        let f = render(&scene, 160, 120, 2);
        assert_eq!(f.rgba.len(), 160 * 120 * 4);
        assert!(f.pick(80, 60).is_some(), "the box should cover the centre");
        assert!(f.pick(1, 1).is_none());
        // Zoomed far into the box must not panic (near-plane clipping).
        let mut close = cam;
        close.distance = 0.5;
        let scene = Scene { camera: &close, ..scene };
        let _ = render(&scene, 64, 48, 1);
    }

    #[test]
    fn grid_steps() {
        assert_eq!(grid_step(300.0), 10.0);
        assert_eq!(grid_step(90.0), 2.0);
    }
}
