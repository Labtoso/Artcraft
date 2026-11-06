//! Toolbar icons, drawn in code (no image assets).

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Sketch,
    FinishSketch,
    Extrude,
    Revolve,
    Box,
    Cylinder,
    Sphere,
    Torus,
    Cone,
    Mirror,
    PatternRect,
    PatternCirc,
    Move,
    Shell,
    Parameters,
    Properties,
    Line,
    Rect,
    Circle,
    Polygon,
    Arc,
    Point,
    Construction,
    Delete,
    Undo,
    Redo,
    Save,
    Open,
    New,
    Home,
    Fit,
    Orbit,
    Pan,
    Grid,
    Display,
    Perspective,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + r.width() * x, r.top() + r.height() * y)
}

fn poly(painter: &Painter, pts: Vec<Pos2>, fill: Color32, stroke: Stroke) {
    painter.add(Shape::convex_polygon(pts, fill, stroke));
}

fn line(painter: &Painter, pts: &[Pos2], stroke: Stroke) {
    painter.add(Shape::line(pts.to_vec(), stroke));
}

/// An isometric box outline inside `r` (fractions x0..x1, y0..y1).
fn iso_box(painter: &Painter, r: Rect, top: f32, bottom: f32, fill: Color32, side: Color32, s: Stroke) {
    let (l, c, rr) = (0.18, 0.5, 0.82);
    let front = vec![p(r, l, top + 0.12), p(r, c, top + 0.24), p(r, c, bottom), p(r, l, bottom - 0.12)];
    let right = vec![p(r, c, top + 0.24), p(r, rr, top + 0.12), p(r, rr, bottom - 0.12), p(r, c, bottom)];
    let lid = vec![p(r, l, top + 0.12), p(r, c, top), p(r, rr, top + 0.12), p(r, c, top + 0.24)];
    poly(painter, front, side, s);
    poly(painter, right, side.linear_multiply(0.8), s);
    poly(painter, lid, fill, s);
}

pub fn paint(painter: &Painter, r: Rect, icon: Icon, color: Color32) {
    let s = Stroke::new((r.width() / 14.0).clamp(1.2, 2.2), color);
    let thin = Stroke::new(s.width * 0.7, color);
    let fill = color.linear_multiply(0.35);
    let accent = crate::theme::ACCENT;
    let c = r.center();
    let w = r.width();
    match icon {
        Icon::Sketch => {
            poly(painter, vec![p(r, 0.1, 0.7), p(r, 0.45, 0.5), p(r, 0.9, 0.7), p(r, 0.55, 0.9)], fill, thin);
            line(painter, &[p(r, 0.3, 0.72), p(r, 0.75, 0.2)], Stroke::new(s.width * 1.6, accent));
            line(painter, &[p(r, 0.3, 0.72), p(r, 0.26, 0.8)], s);
        }
        Icon::FinishSketch => {
            line(painter, &[p(r, 0.2, 0.52), p(r, 0.42, 0.74), p(r, 0.82, 0.28)], Stroke::new(s.width * 1.8, crate::theme::SNAP));
        }
        Icon::Extrude => {
            iso_box(painter, r, 0.38, 0.92, fill, color.linear_multiply(0.55), thin);
            line(painter, &[p(r, 0.5, 0.42), p(r, 0.5, 0.06)], Stroke::new(s.width * 1.4, accent));
            poly(painter, vec![p(r, 0.36, 0.2), p(r, 0.5, 0.04), p(r, 0.64, 0.2)], accent, Stroke::NONE);
        }
        Icon::Revolve => {
            painter.circle_stroke(c + vec2(0.0, w * 0.06), w * 0.32, s);
            poly(painter, vec![p(r, 0.72, 0.12), p(r, 0.9, 0.3), p(r, 0.66, 0.34)], accent, Stroke::NONE);
            line(painter, &[p(r, 0.5, 0.04), p(r, 0.5, 0.96)], Stroke::new(thin.width, accent));
        }
        Icon::Box => iso_box(painter, r, 0.12, 0.9, fill, color.linear_multiply(0.55), thin),
        Icon::Cylinder => {
            let rx = w * 0.32;
            let top = r.top() + r.height() * 0.24;
            let bot = r.top() + r.height() * 0.78;
            painter.rect_filled(Rect::from_min_max(pos2(c.x - rx, top), pos2(c.x + rx, bot)), 0.0, color.linear_multiply(0.45));
            painter.add(Shape::ellipse_filled(pos2(c.x, bot), vec2(rx, w * 0.11), color.linear_multiply(0.45)));
            painter.add(Shape::ellipse_filled(pos2(c.x, top), vec2(rx, w * 0.11), fill.linear_multiply(1.6)));
            painter.add(Shape::ellipse_stroke(pos2(c.x, top), vec2(rx, w * 0.11), thin));
            line(painter, &[pos2(c.x - rx, top), pos2(c.x - rx, bot)], thin);
            line(painter, &[pos2(c.x + rx, top), pos2(c.x + rx, bot)], thin);
        }
        Icon::Sphere => {
            painter.circle_filled(c, w * 0.36, color.linear_multiply(0.45));
            painter.add(Shape::ellipse_stroke(c, vec2(w * 0.36, w * 0.12), thin));
            painter.circle_stroke(c, w * 0.36, thin);
            painter.circle_filled(c - vec2(w * 0.12, w * 0.12), w * 0.07, color.linear_multiply(0.9));
        }
        Icon::Torus => {
            painter.add(Shape::ellipse_stroke(c, vec2(w * 0.38, w * 0.22), Stroke::new(w * 0.14, color.linear_multiply(0.55))));
            painter.add(Shape::ellipse_stroke(c, vec2(w * 0.38, w * 0.22), thin));
        }
        Icon::Cone => {
            poly(painter, vec![p(r, 0.5, 0.1), p(r, 0.82, 0.78), p(r, 0.18, 0.78)], color.linear_multiply(0.45), thin);
            painter.add(Shape::ellipse_stroke(p(r, 0.5, 0.78), vec2(w * 0.32, w * 0.1), thin));
        }
        Icon::Mirror => {
            poly(painter, vec![p(r, 0.1, 0.3), p(r, 0.4, 0.2), p(r, 0.4, 0.8), p(r, 0.1, 0.7)], fill, thin);
            poly(painter, vec![p(r, 0.6, 0.2), p(r, 0.9, 0.3), p(r, 0.9, 0.7), p(r, 0.6, 0.8)], Color32::TRANSPARENT, thin);
            line(painter, &[p(r, 0.5, 0.06), p(r, 0.5, 0.94)], Stroke::new(thin.width, accent));
        }
        Icon::PatternRect => {
            for (x, y) in [(0.1, 0.1), (0.55, 0.1), (0.1, 0.55), (0.55, 0.55)] {
                painter.rect(Rect::from_min_size(p(r, x, y), vec2(w * 0.35, w * 0.35)), 2.0, fill, thin, egui::StrokeKind::Inside);
            }
        }
        Icon::PatternCirc => {
            for k in 0..6 {
                let a = std::f32::consts::TAU * k as f32 / 6.0;
                painter.circle(c + vec2(a.cos(), a.sin()) * w * 0.32, w * 0.1, fill, thin);
            }
            painter.circle_filled(c, w * 0.05, accent);
        }
        Icon::Move => {
            let a = w * 0.4;
            for d in [vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0)] {
                line(painter, &[c, c + d * a], s);
                let tip = c + d * a;
                let n = vec2(-d.y, d.x) * w * 0.1;
                poly(painter, vec![tip + d * w * 0.06, tip - d * w * 0.1 + n, tip - d * w * 0.1 - n], color, Stroke::NONE);
            }
        }
        Icon::Shell => {
            iso_box(painter, r, 0.12, 0.9, fill, color.linear_multiply(0.55), thin);
            poly(painter, vec![p(r, 0.34, 0.24), p(r, 0.5, 0.18), p(r, 0.66, 0.24), p(r, 0.5, 0.3)], crate::theme::PANEL, thin);
        }
        Icon::Parameters => {
            painter.text(c, egui::Align2::CENTER_CENTER, "fx", egui::FontId::proportional(w * 0.62), color);
        }
        Icon::Properties => {
            painter.circle_stroke(c, w * 0.38, s);
            painter.text(c, egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(w * 0.55), color);
        }
        Icon::Line => {
            line(painter, &[p(r, 0.15, 0.85), p(r, 0.85, 0.15)], s);
            painter.circle_filled(p(r, 0.15, 0.85), w * 0.07, accent);
            painter.circle_filled(p(r, 0.85, 0.15), w * 0.07, accent);
        }
        Icon::Rect => {
            painter.rect_stroke(Rect::from_min_max(p(r, 0.14, 0.24), p(r, 0.86, 0.76)), 0.0, s, egui::StrokeKind::Middle);
            painter.circle_filled(p(r, 0.14, 0.76), w * 0.07, accent);
            painter.circle_filled(p(r, 0.86, 0.24), w * 0.07, accent);
        }
        Icon::Circle => {
            painter.circle_stroke(c, w * 0.36, s);
            painter.circle_filled(c, w * 0.06, accent);
        }
        Icon::Polygon => {
            let pts: Vec<Pos2> = (0..6)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / 6.0 + std::f32::consts::FRAC_PI_6;
                    c + vec2(a.cos(), a.sin()) * w * 0.38
                })
                .collect();
            poly(painter, pts, Color32::TRANSPARENT, s);
        }
        Icon::Arc => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let a = std::f32::consts::PI * (1.0 + k as f32 / 16.0);
                    c + vec2(a.cos(), a.sin()) * w * 0.36 + vec2(0.0, w * 0.15)
                })
                .collect();
            line(painter, &pts, s);
            for q in [pts.first(), pts.get(8), pts.last()].into_iter().flatten() {
                painter.circle_filled(*q, w * 0.06, accent);
            }
        }
        Icon::Point => {
            painter.circle_filled(c, w * 0.12, accent);
            painter.circle_stroke(c, w * 0.24, thin);
        }
        Icon::Construction => {
            for k in 0..4 {
                let t0 = 0.12 + k as f32 * 0.2;
                line(painter, &[p(r, t0, 1.0 - t0), p(r, t0 + 0.12, 0.88 - t0)], Stroke::new(s.width, crate::theme::CONSTRUCTION));
            }
        }
        Icon::Delete => {
            line(painter, &[p(r, 0.22, 0.22), p(r, 0.78, 0.78)], Stroke::new(s.width * 1.3, crate::theme::ERROR));
            line(painter, &[p(r, 0.78, 0.22), p(r, 0.22, 0.78)], Stroke::new(s.width * 1.3, crate::theme::ERROR));
        }
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Redo { -1.0 } else { 1.0 };
            let q = |x: f32, y: f32| pos2(c.x + (x - 0.5) * w * flip, r.top() + y * r.height());
            let pts: Vec<Pos2> = (0..=12)
                .map(|k| {
                    let a = std::f32::consts::PI * (k as f32 / 12.0) - std::f32::consts::FRAC_PI_2;
                    q(0.5 + 0.28 * a.cos(), 0.5 + 0.26 * a.sin())
                })
                .collect();
            line(painter, &pts, s);
            line(painter, &[q(0.5, 0.24), q(0.2, 0.24)], s);
            poly(painter, vec![q(0.1, 0.24), q(0.28, 0.1), q(0.28, 0.38)], color, Stroke::NONE);
        }
        Icon::Save => {
            painter.rect_stroke(Rect::from_min_max(p(r, 0.16, 0.16), p(r, 0.84, 0.84)), 2.0, s, egui::StrokeKind::Middle);
            painter.rect_filled(Rect::from_min_max(p(r, 0.3, 0.16), p(r, 0.7, 0.4)), 0.0, color);
            painter.rect_stroke(Rect::from_min_max(p(r, 0.28, 0.56), p(r, 0.72, 0.84)), 0.0, thin, egui::StrokeKind::Middle);
        }
        Icon::Open => {
            poly(painter, vec![p(r, 0.1, 0.3), p(r, 0.4, 0.3), p(r, 0.46, 0.38), p(r, 0.9, 0.38), p(r, 0.9, 0.82), p(r, 0.1, 0.82)], fill, s);
        }
        Icon::New => {
            poly(painter, vec![p(r, 0.24, 0.12), p(r, 0.62, 0.12), p(r, 0.78, 0.28), p(r, 0.78, 0.88), p(r, 0.24, 0.88)], fill, s);
        }
        Icon::Home => {
            poly(painter, vec![p(r, 0.5, 0.12), p(r, 0.88, 0.48), p(r, 0.12, 0.48)], color, Stroke::NONE);
            painter.rect_filled(Rect::from_min_max(p(r, 0.24, 0.46), p(r, 0.76, 0.86)), 1.0, color);
            painter.rect_filled(Rect::from_min_max(p(r, 0.42, 0.6), p(r, 0.58, 0.86)), 0.0, crate::theme::CHROME_DARK);
        }
        Icon::Fit => {
            for (a, b, d) in [(0.12, 0.12, (1.0, 1.0)), (0.88, 0.12, (-1.0, 1.0)), (0.12, 0.88, (1.0, -1.0)), (0.88, 0.88, (-1.0, -1.0))] {
                let o = p(r, a, b);
                line(painter, &[o + vec2(0.0, d.1 * w * 0.22), o, o + vec2(d.0 * w * 0.22, 0.0)], s);
            }
            painter.rect_stroke(Rect::from_center_size(c, vec2(w * 0.32, w * 0.32)), 1.0, thin, egui::StrokeKind::Middle);
        }
        Icon::Orbit => {
            painter.add(Shape::ellipse_stroke(c, vec2(w * 0.4, w * 0.18), s));
            painter.circle_filled(c, w * 0.12, color);
        }
        Icon::Pan => {
            paint(painter, r.shrink(w * 0.05), Icon::Move, color);
        }
        Icon::Grid => {
            for k in 0..4 {
                let t = 0.14 + k as f32 * 0.24;
                line(painter, &[p(r, t, 0.14), p(r, t, 0.86)], thin);
                line(painter, &[p(r, 0.14, t), p(r, 0.86, t)], thin);
            }
        }
        Icon::Display => {
            iso_box(painter, r, 0.14, 0.88, color.linear_multiply(0.7), color.linear_multiply(0.45), Stroke::new(thin.width, Color32::BLACK));
        }
        Icon::Perspective => {
            poly(painter, vec![p(r, 0.3, 0.2), p(r, 0.7, 0.2), p(r, 0.9, 0.82), p(r, 0.1, 0.82)], Color32::TRANSPARENT, s);
        }
    }
}
