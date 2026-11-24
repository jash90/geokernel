// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Lines, segments, midpoints and distances built from points.
//!
//! Lines carry GeoGebra's raw coefficients (see [`Value::Line`]). A free
//! point is `(x, y, 1)` in GeoGebra's homogeneous coordinates.

use super::{join, line_value, normalized_coefficients};
use crate::kernel::{Construction, Id, Value};

/// `AlgoJoinPoints`: `GeoVec3D.lineThroughPoints(P, Q)`.
pub(crate) fn line(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (x, y, z) = raw_line_through(c, p, q)?;
    line_value(x, y, z)
}

pub(crate) fn raw_line_through(c: &Construction, p: Id, q: Id) -> Option<(f64, f64, f64)> {
    match (c.point_at_infinity(p), c.point_at_infinity(q)) {
        (Some(_), Some(_)) => None,
        (Some((x, y, _)), None) => {
            let (qx, qy) = c.point(q)?;
            Some((y, -x, x * qy - y * qx))
        }
        (None, Some((x, y, _))) => {
            let (px, py) = c.point(p)?;
            Some((-y, x, px * y - py * x))
        }
        (None, None) => Some(join(c.point(p)?, c.point(q)?)),
    }
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
    let (x1, y1) = c.cartesian_point(p)?;
    let (x2, y2) = c.cartesian_point(q)?;
    Some(Value::Number(super::measure::my_length(x2 - x1, y2 - y1)))
}

/// Cross a point with the homogeneous direction of a line.
pub(crate) fn cross_direction(
    (px, py, weight): (f64, f64, f64),
    (vx, vy): (f64, f64),
) -> (f64, f64, f64) {
    (
        py * 0.0 - weight * vy,
        weight * vx - px * 0.0,
        px * vy - py * vx,
    )
}

pub(crate) fn raw_perpendicular(
    c: &Construction,
    through: Id,
    (lx, ly, _): (f64, f64, f64),
) -> Option<(f64, f64, f64)> {
    let p = c.stored_value(through).point_coords()?;
    Some(normalized_coefficients(
        cross_direction(p, (lx, ly)),
        10e6,
        1e-2,
    ))
}

pub(crate) fn raw_parallel(
    c: &Construction,
    through: Id,
    (lx, ly, _): (f64, f64, f64),
) -> Option<(f64, f64, f64)> {
    let p = c.stored_value(through).point_coords()?;
    Some(cross_direction(p, (ly, -lx)))
}

/// `AlgoOrthoLinePointLine`: the cross product of `P` with the normal of
/// the line, rescaled by powers of two into `[1e-2, 1e7]`.
pub(crate) fn perpendicular(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (x, y, z) = raw_perpendicular(c, through, c.raw_carrier(to)?)?;
    line_value(x, y, z)
}

/// `AlgoLinePointLine`: the cross product of `P` with the direction of the line.
pub(crate) fn parallel(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (x, y, z) = raw_parallel(c, through, c.raw_carrier(to)?)?;
    line_value(x, y, z)
}
