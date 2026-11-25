//! Drawing primitives for property-controlled points and strokes.

use eframe::egui::{vec2, Color32, Painter, Pos2, Shape, Stroke, Vec2};

pub(super) fn point(painter: &Painter, pos: Pos2, color: Color32, radius: f32, style: i32) {
    let stroke = Stroke::new(1.0, color);
    let p = |x, y| pos + vec2(x, y) * radius;
    let polygon = |vertices: Vec<Pos2>, fill| {
        painter.add(Shape::convex_polygon(vertices, fill, stroke));
    };
    match style {
        1 => {
            painter.line_segment([p(-0.7, -0.7), p(0.7, 0.7)], stroke);
            painter.line_segment([p(-0.7, 0.7), p(0.7, -0.7)], stroke);
        }
        2 => {
            painter.circle_stroke(pos, radius, stroke);
        }
        3 => {
            painter.line_segment([p(-1.0, 0.0), p(1.0, 0.0)], stroke);
            painter.line_segment([p(0.0, -1.0), p(0.0, 1.0)], stroke);
        }
        4 | 5 => polygon(
            vec![p(0.0, -1.0), p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0)],
            if style == 4 {
                color
            } else {
                Color32::TRANSPARENT
            },
        ),
        6..=9 => {
            let rotate = |x: f32, y: f32| match style {
                7 => p(-x, -y),
                8 => p(-y, x),
                9 => p(y, -x),
                _ => p(x, y),
            };
            polygon(
                vec![rotate(0.0, -1.0), rotate(0.87, 0.5), rotate(-0.87, 0.5)],
                color,
            );
        }
        10 => {
            painter.circle_filled(pos, radius, color);
        }
        _ => super::point_shape(painter, pos, color, radius),
    }
}

pub(super) fn line(painter: &Painter, points: [Pos2; 2], stroke: Stroke, style: i32) {
    let (dash, gap) = match style {
        10 => (5.0, 4.0),
        15 => (11.0, 6.0),
        20 => (1.0, 4.0),
        30 => (9.0, 4.0),
        _ => {
            painter.line_segment(points, stroke);
            return;
        }
    };
    let [a, b] = points;
    let delta = b - a;
    if !delta.is_finite() || delta == Vec2::ZERO {
        return;
    }
    let rect = painter.clip_rect();
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for (start, step, min, max) in [
        (a.x, delta.x, rect.min.x, rect.max.x),
        (a.y, delta.y, rect.min.y, rect.max.y),
    ] {
        if step == 0.0 {
            if !(min..=max).contains(&start) {
                return;
            }
        } else {
            let first = (min - start) / step;
            let last = (max - start) / step;
            lo = lo.max(first.min(last));
            hi = hi.min(first.max(last));
        }
    }
    if lo >= hi {
        return;
    }
    let range = [a + delta * lo, a + delta * hi];
    let length = range[0].distance(range[1]);
    if !length.is_finite() {
        return;
    }
    // Keep even a very large view within a fixed subdivision allowance.
    let scale = (length / (128.0 * (dash + gap))).max(1.0);
    if style == 30 {
        let mut shapes = Vec::new();
        Shape::dashed_line_many_with_offset(
            &range,
            stroke,
            &[dash * scale, scale],
            &[gap * scale],
            0.0,
            &mut shapes,
        );
        painter.extend(shapes);
    } else {
        painter.extend(Shape::dashed_line(
            &range,
            stroke,
            dash * scale,
            gap * scale,
        ));
    }
}

pub(super) fn segment_marks(
    painter: &Painter,
    a: Pos2,
    b: Pos2,
    stroke: Stroke,
    ends: [Option<u8>; 2],
    decoration: u8,
) {
    let d = b - a;
    let length = d.length();
    if length == 0.0 || !length.is_finite() {
        return;
    }
    let unit = d / length;
    let normal = vec2(-unit.y, unit.x);
    let radius = stroke.width.clamp(1.0, 5.0) + 3.0;
    for (style, point, forward) in [(ends[0], a, unit), (ends[1], b, -unit)] {
        let Some(style) = style else { continue };
        let left = point + normal * radius;
        let right = point - normal * radius;
        let base = forward * (1.5 * radius);
        match style {
            1 => {
                painter.line_segment([left, right], stroke);
            }
            2 => {
                painter.add(Shape::line(vec![left + base, point, right + base], stroke));
            }
            3 => {
                painter.add(Shape::line(vec![left, point + base, right], stroke));
            }
            4 | 5 => {
                painter.add(Shape::convex_polygon(
                    vec![point, left + base, right + base],
                    if style == 5 {
                        stroke.color
                    } else {
                        Color32::TRANSPARENT
                    },
                    stroke,
                ));
            }
            6 => {
                painter.circle_stroke(point, radius, stroke);
            }
            7 => {
                painter.circle_filled(point, radius, stroke.color);
            }
            _ => {}
        }
    }
    let count = match decoration {
        1..=3 => decoration,
        4..=6 => decoration - 3,
        _ => 0,
    };
    for i in 0..count {
        let mid = a + d * 0.5 + unit * ((f32::from(i) - 0.5 * f32::from(count - 1)) * 6.0);
        let left = mid + normal * radius;
        let right = mid - normal * radius;
        if decoration < 4 {
            painter.line_segment([left, right], stroke);
        } else {
            painter.add(Shape::line(vec![left, mid + unit * radius, right], stroke));
        }
    }
}
