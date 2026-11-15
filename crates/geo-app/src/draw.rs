//! Draws a construction with `egui::Painter`.

use crate::interact::ViewTransform;
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};
use geo_kernel::algo::vector;
use geo_kernel::kernel::{Construction, Value};

const BACKGROUND: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const LINE: Color32 = Color32::from_rgb(0x9a, 0xa0, 0xa6);
const CIRCLE: Color32 = Color32::from_rgb(0x1a, 0x73, 0xe8);
const SEGMENT: Color32 = Color32::from_rgb(0x20, 0x21, 0x24);
const FREE_POINT: Color32 = Color32::from_rgb(0x1a, 0x73, 0xe8);
const DEPENDENT_POINT: Color32 = Color32::from_rgb(0x5f, 0x63, 0x68);
const LABEL: Color32 = Color32::from_rgb(0x20, 0x21, 0x24);

/// Radius in pixels of a free point; used by hit testing too.
pub const FREE_POINT_RADIUS: f32 = 5.0;
const DEPENDENT_POINT_RADIUS: f32 = 4.0;

/// Draws everything into `rect`. `t` works in coordinates relative to the top-left of `rect`.
pub fn paint(painter: &Painter, rect: Rect, c: &Construction, t: &ViewTransform) {
    painter.rect_filled(rect, 0.0, BACKGROUND);
    let origin = rect.min;
    let p = |x: f64, y: f64| -> Pos2 {
        let (sx, sy) = t.world_to_screen(x, y);
        origin + eframe::egui::vec2(sx as f32, sy as f32)
    };

    // Used to draw an "infinite" line as a segment longer than the view.
    let v = t.view();
    let (vw, vh) = (v.xmax - v.xmin, v.ymax - v.ymin);
    let (mx, my) = (0.5 * (v.xmin + v.xmax), 0.5 * (v.ymin + v.ymax));
    let reach = (rect.width() as f64 / t.scale())
        .hypot(rect.height() as f64 / t.scale())
        .max(vw.hypot(vh));

    // Draw in layers: lines, circles, segments, then points on top.
    for id in c.ids() {
        if let Value::Line { a, b, c: k } = c.value(id) {
            let d = a * mx + b * my + k;
            let (fx, fy) = (mx - a * d, my - b * d);
            painter.line_segment(
                [
                    p(fx - reach * b, fy + reach * a),
                    p(fx + reach * b, fy - reach * a),
                ],
                Stroke::new(1.0, LINE),
            );
        }
    }
    for id in c.ids() {
        // A ray: a half-line from its start point along (b, -a).
        if let Value::Ray { x, y, a, b, .. } = c.value(id) {
            let far = (reach + (x - mx).hypot(y - my)) / a.hypot(b);
            painter.line_segment(
                [p(x, y), p(x + far * b, y - far * a)],
                Stroke::new(1.0, LINE),
            );
        }
    }
    for id in c.ids() {
        if let Value::Circle { cx, cy, r } = c.value(id) {
            painter.circle_stroke(p(cx, cy), (r * t.scale()) as f32, Stroke::new(1.5, CIRCLE));
        }
    }
    for id in c.ids() {
        if let Value::Segment { x1, y1, x2, y2 } = c.value(id) {
            painter.line_segment([p(x1, y1), p(x2, y2)], Stroke::new(2.0, SEGMENT));
        }
    }
    for id in c.ids() {
        // A vector: an arrow from its start point (the origin if it has none).
        let (Value::Vector { x, y }, Some((x0, y0))) = (c.value(id), vector::start(c, id)) else {
            continue;
        };
        let (from, to) = (p(x0, y0), p(x0 + x, y0 + y));
        painter.line_segment([from, to], Stroke::new(2.0, SEGMENT));
        let d = to - from;
        if d.length() > 0.0 {
            let u = d / d.length();
            let n = eframe::egui::vec2(-u.y, u.x);
            let base = to - 10.0 * u;
            painter.add(eframe::egui::Shape::convex_polygon(
                vec![to, base + 4.0 * n, base - 4.0 * n],
                SEGMENT,
                Stroke::NONE,
            ));
        }
    }
    for id in c.ids() {
        if let Value::Point { x, y } = c.value(id) {
            let (fill, radius) = if c.def(id).is_free() {
                (FREE_POINT, FREE_POINT_RADIUS)
            } else {
                (DEPENDENT_POINT, DEPENDENT_POINT_RADIUS)
            };
            let pos = p(x, y);
            painter.circle(pos, radius, fill, Stroke::new(1.0, Color32::WHITE));
            painter.text(
                pos + eframe::egui::vec2(8.0, -8.0),
                Align2::LEFT_BOTTOM,
                c.label(id),
                FontId::proportional(14.0),
                LABEL,
            );
        }
    }
}
