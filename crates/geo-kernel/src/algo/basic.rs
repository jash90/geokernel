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

use super::{join, length, line_value, normalized_coefficients};
use crate::kernel::{Construction, Id, Value};

/// `AlgoJoinPoints`: `GeoVec3D.lineThroughPoints(P, Q)`.
pub(crate) fn line(c: &Construction, p: Id, q: Id) -> Option<Value> {
    let (x, y, z) = join(c.point(p)?, c.point(q)?);
    line_value(x, y, z)
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

/// `GeoVec3D.cross(P, vx, vy, vz)` for the point `(px, py, 1)`.
fn cross_point(px: f64, py: f64, vx: f64, vy: f64, vz: f64) -> (f64, f64, f64) {
    (py * vz - vy, vx - px * vz, px * vy - py * vx)
}

/// `AlgoOrthoLinePointLine`: the cross product of `P` with the normal of
/// the line, rescaled by powers of two into `[1e-2, 1e7]` so that chains of
/// perpendiculars neither overflow nor underflow.
pub(crate) fn perpendicular(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (px, py) = c.point(through)?;
    let (lx, ly, _) = c.raw_carrier(to)?;
    let (x, y, z) = normalized_coefficients(cross_point(px, py, lx, ly, 0.0), 10e6, 1e-2);
    line_value(x, y, z)
}

/// `AlgoLinePointLine`: the cross product of `P` with the direction of the line.
pub(crate) fn parallel(c: &Construction, through: Id, to: Id) -> Option<Value> {
    let (px, py) = c.point(through)?;
    let (lx, ly, _) = c.raw_carrier(to)?;
    let (x, y, z) = cross_point(px, py, ly, -lx, 0.0);
    line_value(x, y, z)
}
