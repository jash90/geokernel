//! Hit testing of any visible object under a screen position, with no UI types.

use crate::interact::ViewTransform;
use geo_kernel::algo::vector;
use geo_kernel::kernel::{Construction, Id, Value};

/// Distance from `p` to the segment `a`-`b`, all in screen pixels.
fn dist_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// Distance in screen pixels from `p` to the object's drawn shape, or `None`
/// for objects that are not drawn (numbers, undefined, unknown kinds).
fn distance(c: &Construction, id: Id, t: &ViewTransform, p: (f64, f64)) -> Option<f64> {
    let to_screen = |x: f64, y: f64| t.world_to_screen(x, y);
    match c.value(id) {
        Value::Point { x, y } => {
            let s = to_screen(x, y);
            Some((s.0 - p.0).hypot(s.1 - p.1))
        }
        v @ Value::Line { .. } => {
            let (a, b, k) = v.unit_line()?;
            let (wx, wy) = t.screen_to_world(p.0, p.1);
            Some((a * wx + b * wy + k).abs() * t.scale())
        }
        Value::Segment { x1, y1, x2, y2 } => {
            Some(dist_segment(p, to_screen(x1, y1), to_screen(x2, y2)))
        }
        Value::Ray { x, y, a, b, .. } => {
            let len = a.hypot(b);
            if len == 0.0 {
                return None;
            }
            let start = to_screen(x, y);
            // A point far along the ray stands in for infinity.
            let far = 1e7 / t.scale().max(1e-9);
            let end = to_screen(x + far * b / len, y - far * a / len);
            Some(dist_segment(p, start, end))
        }
        Value::Circle { cx, cy, r } => {
            let s = to_screen(cx, cy);
            Some(((s.0 - p.0).hypot(s.1 - p.1) - r * t.scale()).abs())
        }
        Value::Vector { x, y } => {
            let (x0, y0) = vector::start(c, id)?;
            Some(dist_segment(
                p,
                to_screen(x0, y0),
                to_screen(x0 + x, y0 + y),
            ))
        }
        Value::Polygon { .. } | Value::PolyLine { .. } => {
            let closed = matches!(c.value(id), Value::Polygon { .. });
            let pts = screen_vertices(c, id, t)?;
            let n = pts.len();
            let edges = if closed { n } else { n.saturating_sub(1) };
            (0..edges)
                .map(|i| dist_segment(p, pts[i], pts[(i + 1) % n]))
                .min_by(f64::total_cmp)
        }
        _ => None,
    }
}

/// The vertices of a polygon or polyline on the screen, `None` when one of
/// them is undefined.
pub fn screen_vertices(c: &Construction, id: Id, t: &ViewTransform) -> Option<Vec<(f64, f64)>> {
    let pts: Option<Vec<(f64, f64)>> = c
        .vertices(id)
        .map(|v| v.map(|(x, y)| t.world_to_screen(x, y)))
        .collect();
    pts.filter(|p| p.len() >= 2)
}

/// The object under the screen position `(sx, sy)` that `accept` allows.
/// Points win over curves when one is within `point_radius`; otherwise the
/// nearest curve within `curve_radius` (both in pixels) is returned, and
/// failing that a polygon whose inside holds the position.
pub fn pick(
    c: &Construction,
    t: &ViewTransform,
    (sx, sy): (f64, f64),
    point_radius: f64,
    curve_radius: f64,
    accept: impl Fn(Id) -> bool,
) -> Option<Id> {
    let mut best_point: Option<(Id, f64)> = None;
    let mut best_curve: Option<(Id, f64)> = None;
    // The last polygon whose inside holds the position, as the one drawn on top.
    let mut inside: Option<Id> = None;
    for id in c.ids() {
        if !accept(id) {
            continue;
        }
        if matches!(c.value(id), Value::Polygon { .. })
            && screen_vertices(c, id, t).is_some_and(|pts| crate::shape::contains(&pts, (sx, sy)))
        {
            inside = Some(id);
        }
        let Some(d) = distance(c, id, t, (sx, sy)) else {
            continue;
        };
        let is_point = matches!(c.value(id), Value::Point { .. });
        let (best, limit) = if is_point {
            (&mut best_point, point_radius)
        } else {
            (&mut best_curve, curve_radius)
        };
        if d <= limit && best.is_none_or(|(_, bd)| d < bd) {
            *best = Some((id, d));
        }
    }
    best_point.or(best_curve).map(|(id, _)| id).or(inside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_kernel::svg::View;

    /// 100 x 100 px area showing the square [-5, 5]^2: 10 px per unit.
    fn setup() -> (Construction, ViewTransform) {
        let c = geo_kernel::script::load(
            "A = (0, 0)\nB = (4, 0)\nf = Line(A, B)\nk = Circle(A, B)\ns = Segment(A, B)\nN = 3\n",
        )
        .unwrap();
        let t = ViewTransform::new(
            View {
                xmin: -5.0,
                ymin: -5.0,
                xmax: 5.0,
                ymax: 5.0,
            },
            100.0,
            100.0,
        );
        (c, t)
    }

    fn label(c: &Construction, id: Option<Id>) -> Option<String> {
        id.map(|i| c.label(i).to_owned())
    }

    #[test]
    fn a_point_wins_over_curves_through_it() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        let id = pick(&c, &t, (sx + 2.0, sy), 8.0, 6.0, |_| true);
        assert_eq!(label(&c, id).as_deref(), Some("A"));
    }

    #[test]
    fn curves_are_found_within_the_radius() {
        let (c, t) = setup();
        // Just above the line f (y = 0) between A and B, outside both points.
        let (sx, sy) = t.world_to_screen(2.0, 0.0);
        let on_line = pick(&c, &t, (sx, sy - 3.0), 8.0, 6.0, |id| c.label(id) == "f");
        assert_eq!(label(&c, on_line).as_deref(), Some("f"));
        let too_far = pick(&c, &t, (sx, sy - 9.0), 8.0, 6.0, |id| c.label(id) == "f");
        assert_eq!(too_far, None);
        // The circle of radius 4 around the origin.
        let (cx, cy) = t.world_to_screen(0.0, 4.0);
        let on_circle = pick(&c, &t, (cx + 1.0, cy), 8.0, 6.0, |id| c.label(id) == "k");
        assert_eq!(label(&c, on_circle).as_deref(), Some("k"));
    }

    #[test]
    fn segments_end_at_their_end_points() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(4.0, 0.0);
        let beyond = pick(&c, &t, (sx + 10.0, sy), 2.0, 6.0, |id| c.label(id) == "s");
        assert_eq!(beyond, None);
        let inside = pick(&c, &t, (sx - 10.0, sy), 2.0, 6.0, |id| c.label(id) == "s");
        assert_eq!(label(&c, inside).as_deref(), Some("s"));
    }

    #[test]
    fn accept_filters_and_numbers_are_never_hit() {
        let (c, t) = setup();
        let (sx, sy) = t.world_to_screen(0.0, 0.0);
        assert_eq!(pick(&c, &t, (sx, sy), 8.0, 6.0, |_| false), None);
        let id = pick(&c, &t, (sx, sy), 8.0, 6.0, |id| c.label(id) == "N");
        assert_eq!(id, None);
    }

    #[test]
    fn the_nearest_curve_wins() {
        let (c, t) = setup();
        // Near the crossing of f and s the closer one is chosen: put the
        // pointer 1 px from the line and 5 px from the circle.
        let (sx, sy) = t.world_to_screen(-4.5, 0.0);
        let id = pick(&c, &t, (sx, sy + 1.0), 2.0, 6.0, |id| {
            matches!(c.label(id), "f" | "k")
        });
        assert_eq!(label(&c, id).as_deref(), Some("f"));
    }
}
