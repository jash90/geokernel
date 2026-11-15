//! Draws the objects of a construction with `egui::Painter`.

use crate::highlight::{self, Highlight};
use crate::interact::ViewTransform;
use crate::labelpos;
use crate::theme;
use eframe::egui::{vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};
use geo_kernel::algo::vector;
use geo_kernel::kernel::{Construction, Id, Value};

pub const FREE_POINT_RADIUS: f32 = 5.5;
const DEPENDENT_POINT_RADIUS: f32 = 5.0;
const CURVE_WIDTH: f32 = 2.0;
const LABEL_FONT: f32 = 16.0;

/// What the picture needs besides the construction.
pub struct Scene<'a> {
    pub visible: &'a dyn Fn(Id) -> bool,
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
}

/// Colours of an object's marker in the algebra list: the fill and the ring.
pub fn marker_colors(c: &Construction, id: Id) -> (Color32, Color32) {
    let ring = Color32::from_rgb(0x70, 0x70, 0x70);
    match c.value(id) {
        Value::Point { .. } if c.def(id).is_free() => (Color32::from_rgb(0x9e, 0xc4, 0xf2), ring),
        Value::Point { .. } => (Color32::from_rgb(0xc4, 0xc4, 0xc4), ring),
        _ => (Color32::from_rgb(0x70, 0x70, 0x70), ring),
    }
}

pub fn paint(painter: &Painter, rect: Rect, c: &Construction, t: &ViewTransform, scene: &Scene) {
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    let origin = rect.min;
    let p = |x: f64, y: f64| -> Pos2 {
        let (sx, sy) = t.world_to_screen(x, y);
        origin + vec2(sx as f32, sy as f32)
    };
    let screen = |x: f64, y: f64| t.world_to_screen(x, y);
    let in_rect = |s: (f64, f64)| s.0 >= 0.0 && s.0 <= w && s.1 >= 0.0 && s.1 <= h;
    let curve = Stroke::new(CURVE_WIDTH, theme::CURVE);

    // Used to draw an "infinite" line as a segment longer than the view.
    let v = t.view();
    let (vw, vh) = (v.xmax - v.xmin, v.ymax - v.ymin);
    let (mx, my) = (0.5 * (v.xmin + v.xmax), 0.5 * (v.ymin + v.ymax));
    let reach = (w / t.scale()).hypot(h / t.scale()).max(vw.hypot(vh));

    let ids: Vec<Id> = c.ids().filter(|&id| (scene.visible)(id)).collect();
    let mut labels: Vec<(String, (f64, f64), Color32, Align2)> = Vec::new();

    // Highlights go under the objects.
    for (id, strong) in [(scene.hovered, false), (scene.selected, true)] {
        let Some(id) = id.filter(|i| (scene.visible)(*i)) else {
            continue;
        };
        if let Some(hl) = highlight::of_value(&c.value(id), t, w, h) {
            paint_highlight(painter, rect, hl, strong);
        }
    }

    for &id in &ids {
        if let Some((a, b, k)) = c.value(id).unit_line() {
            let d = a * mx + b * my + k;
            let (fx, fy) = (mx - a * d, my - b * d);
            painter.line_segment(
                [
                    p(fx - reach * b, fy + reach * a),
                    p(fx + reach * b, fy - reach * a),
                ],
                curve,
            );
            if let Some(entry) = labelpos::line_entry(screen(fx, fy), (b, a), w, h) {
                let pos = labelpos::line_label(entry, w, h, 24.0, 12.0);
                labels.push((
                    c.label(id).to_owned(),
                    pos,
                    theme::LABEL,
                    Align2::CENTER_CENTER,
                ));
            }
        }
    }
    for &id in &ids {
        // A ray: a half-line from its start point along (b, -a).
        if let Value::Ray { x, y, a, b, .. } = c.value(id) {
            let len = a.hypot(b);
            if !(len > 0.0 && x.is_finite() && y.is_finite()) {
                continue;
            }
            let far = (reach + (x - mx).hypot(y - my)) / len;
            painter.line_segment([p(x, y), p(x + far * b, y - far * a)], curve);
            let s = screen(x, y);
            let entry = if in_rect(s) {
                Some(labelpos::Entry {
                    x: s.0,
                    y: s.1,
                    dx: b / len,
                    dy: a / len,
                })
            } else {
                labelpos::line_entry(s, (b, a), w, h)
            };
            if let Some(e) = entry {
                let pos = labelpos::line_label(e, w, h, 24.0, 12.0);
                labels.push((
                    c.label(id).to_owned(),
                    pos,
                    theme::LABEL,
                    Align2::CENTER_CENTER,
                ));
            }
        }
    }
    for &id in &ids {
        if let Value::Circle { cx, cy, r } = c.value(id) {
            let radius = r * t.scale();
            if !(radius.is_finite() && radius < 1e5 && cx.is_finite() && cy.is_finite()) {
                continue;
            }
            painter.circle_stroke(p(cx, cy), radius as f32, curve);
            let (sx, sy) = screen(cx, cy);
            // Prefer the upper right of the circle, then the other diagonals.
            for deg in [-45.0f64, -135.0, 45.0, 135.0] {
                let a = deg.to_radians();
                let pos = (
                    sx + (radius + 12.0) * a.cos(),
                    sy + (radius + 12.0) * a.sin(),
                );
                if in_rect(pos) {
                    labels.push((
                        c.label(id).to_owned(),
                        pos,
                        theme::LABEL,
                        Align2::CENTER_CENTER,
                    ));
                    break;
                }
            }
        }
    }
    for &id in &ids {
        if let Value::Segment { x1, y1, x2, y2 } = c.value(id) {
            if ![x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                continue;
            }
            painter.line_segment([p(x1, y1), p(x2, y2)], curve);
            let pos = labelpos::segment_label(screen(x1, y1), screen(x2, y2), 12.0);
            labels.push((
                c.label(id).to_owned(),
                pos,
                theme::LABEL,
                Align2::CENTER_CENTER,
            ));
        }
    }
    for &id in &ids {
        // A vector: an arrow from its start point (the origin if it has none).
        let (Value::Vector { x, y }, Some((x0, y0))) = (c.value(id), vector::start(c, id)) else {
            continue;
        };
        let (from, to) = (p(x0, y0), p(x0 + x, y0 + y));
        painter.line_segment([from, to], curve);
        let d = to - from;
        if d.length() > 0.0 {
            let u = d / d.length();
            let n = vec2(-u.y, u.x);
            let base = to - 10.0 * u;
            painter.add(Shape::convex_polygon(
                vec![to, base + 4.0 * n, base - 4.0 * n],
                theme::CURVE,
                Stroke::NONE,
            ));
        }
        let pos = labelpos::segment_label(screen(x0, y0), screen(x0 + x, y0 + y), 12.0);
        labels.push((
            c.label(id).to_owned(),
            pos,
            theme::LABEL,
            Align2::CENTER_CENTER,
        ));
    }
    for &id in &ids {
        if let Value::Point { x, y } = c.value(id) {
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            let free = c.def(id).is_free();
            let (fill, ring, radius, label_color) = if free {
                (
                    theme::FREE_POINT_FILL,
                    theme::FREE_POINT_RING,
                    FREE_POINT_RADIUS,
                    theme::FREE_POINT_LABEL,
                )
            } else {
                (
                    theme::DEPENDENT_FILL,
                    theme::DEPENDENT_RING,
                    DEPENDENT_POINT_RADIUS,
                    theme::LABEL,
                )
            };
            painter.circle(p(x, y), radius, fill, Stroke::new(1.3, ring));
            let (sx, sy) = screen(x, y);
            labels.push((
                c.label(id).to_owned(),
                (sx + 8.0, sy - 8.0),
                label_color,
                Align2::LEFT_BOTTOM,
            ));
        }
    }
    let font = FontId::proportional(LABEL_FONT);
    for (text, (sx, sy), color, align) in labels {
        if sx < -20.0 || sy < -20.0 || sx > w + 20.0 || sy > h + 20.0 {
            continue;
        }
        painter.text(
            origin + vec2(sx as f32, sy as f32),
            align,
            text,
            font.clone(),
            color,
        );
    }
}

fn paint_highlight(painter: &Painter, rect: Rect, hl: Highlight, strong: bool) {
    let p = |x: f64, y: f64| rect.min + vec2(x as f32, y as f32);
    let alpha = if strong { 0.45 } else { 0.25 };
    let glow = Stroke::new(8.0, theme::ACCENT.gamma_multiply(alpha));
    match hl {
        Highlight::Point { x, y } => {
            painter.circle_filled(p(x, y), 11.0, theme::ACCENT.gamma_multiply(alpha));
        }
        Highlight::Stroke { x1, y1, x2, y2 } => {
            painter.line_segment([p(x1, y1), p(x2, y2)], glow);
        }
        Highlight::Circle { x, y, r } => {
            painter.circle_stroke(p(x, y), r as f32, glow);
        }
    }
}
