//! Intersection points of lines and circles.

use super::length;
use crate::kernel::{Construction, Id, Value};
use crate::tol::{EPS, TANGENT_TOL};

pub(crate) fn lines(c: &Construction, l: Id, m: Id) -> Option<Value> {
    let (a1, b1, c1) = c.line(l)?;
    let (a2, b2, c2) = c.line(m)?;
    let det = a1 * b2 - a2 * b1;
    if det.abs() < EPS {
        return None; // parallel
    }
    Some(Value::Point {
        x: (b1 * c2 - b2 * c1) / det,
        y: (a2 * c1 - a1 * c2) / det,
    })
}

/// `which` is 0 or 1: the first or second point along the line's direction.
pub(crate) fn line_circle(c: &Construction, line: Id, circle: Id, which: u8) -> Option<Value> {
    let (a, b, cc) = c.line(line)?;
    let (cx, cy, r) = c.circle(circle)?;
    // Signed distance from the centre to the line.
    let d = a * cx + b * cy + cc;
    let h2 = r * r - d * d;
    if h2 < -TANGENT_TOL * (r * r).max(1.0) {
        return None;
    }
    let h = h2.max(0.0).sqrt();
    let (fx, fy) = (cx - a * d, cy - b * d);
    let s = if which == 0 { -h } else { h };
    Some(Value::Point {
        x: fx + s * b,
        y: fy - s * a,
    })
}

/// `which` is 0 or 1: the point to the left or to the right of the
/// direction from the first centre to the second.
pub(crate) fn circles(c: &Construction, c1: Id, c2: Id, which: u8) -> Option<Value> {
    let (x1, y1, r1) = c.circle(c1)?;
    let (x2, y2, r2) = c.circle(c2)?;
    let (dx, dy) = (x2 - x1, y2 - y1);
    let d = length(dx, dy);
    if d < EPS {
        return None; // concentric
    }
    let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h2 = r1 * r1 - along * along;
    if h2 < -TANGENT_TOL * (r1 * r1).max(1.0) {
        return None;
    }
    let h = h2.max(0.0).sqrt();
    let (ux, uy) = (dx / d, dy / d);
    let (bx, by) = (x1 + along * ux, y1 + along * uy);
    let s = if which == 0 { h } else { -h };
    Some(Value::Point {
        x: bx - s * uy,
        y: by + s * ux,
    })
}
