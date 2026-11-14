//! Circles given by a centre and a point or a radius.

use super::length;
use crate::kernel::{Construction, Id, Value};

pub(crate) fn through(c: &Construction, center: Id, through: Id) -> Option<Value> {
    let (cx, cy) = c.point(center)?;
    let (px, py) = c.point(through)?;
    Some(Value::Circle {
        cx,
        cy,
        r: length(px - cx, py - cy),
    })
}

pub(crate) fn with_radius(c: &Construction, center: Id, radius: Id) -> Option<Value> {
    let (cx, cy) = c.point(center)?;
    let r = c.number(radius)?;
    if r < 0.0 {
        return None;
    }
    Some(Value::Circle { cx, cy, r })
}
