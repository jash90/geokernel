//! Lines, segments, midpoints and distances built from points.

use super::{length, line_through};
use crate::kernel::{Construction, Id, Value};

pub(crate) fn line(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (a, b, c) = line_through(c.point(p)?, c.point(q)?)?;
    Some(Value::Line { a, b, c })
}

pub(crate) fn segment(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (x1, y1) = c.point(p)?;
    let (x2, y2) = c.point(q)?;
    Some(Value::Segment { x1, y1, x2, y2 })
}

pub(crate) fn midpoint(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (x1, y1) = c.point(p)?;
    let (x2, y2) = c.point(q)?;
    Some(Value::Point {
        x: 0.5 * (x1 + x2),
        y: 0.5 * (y1 + y2),
    })
}

pub(crate) fn distance(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (x1, y1) = c.point(p)?;
    let (x2, y2) = c.point(q)?;
    Some(Value::Number(length(x2 - x1, y2 - y1)))
}

pub(crate) fn perpendicular(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (px, py) = c.point(through)?;
    let (a, b, _) = c.carrier(to)?;
    // The new line runs along the old normal (a, b).
    let (na, nb) = (-b, a);
    Some(Value::Line {
        a: na,
        b: nb,
        c: -(na * px + nb * py),
    })
}

pub(crate) fn parallel(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (px, py) = c.point(through)?;
    let (a, b, _) = c.carrier(to)?;
    Some(Value::Line {
        a,
        b,
        c: -(a * px + b * py),
    })
}
