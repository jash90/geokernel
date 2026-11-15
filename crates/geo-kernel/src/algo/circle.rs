// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Circles given by a centre and a point or a radius.

use super::length;
use crate::kernel::{Construction, Id, Value};
use crate::tol::STANDARD_PRECISION;

/// The radius GeoGebra keeps: `GeoQuadricND.setSphereND` turns a radius
/// that `DoubleUtil.isZero` accepts into 0, and `setSphereNDMatrix` makes a
/// radius below `STANDARD_PRECISION` a single point with half-axes 0
/// (`GeoConicND.singlePoint`). `None` for a negative radius.
fn radius(r: f64) -> Option<f64> {
    if -STANDARD_PRECISION < r && r < STANDARD_PRECISION {
        Some(0.0)
    } else if r < 0.0 {
        None
    } else {
        Some(r)
    }
}

pub(crate) fn through(c: &Construction, center: Id, through: Id) -> Option<Value> {
    let (cx, cy) = c.point(center)?;
    let (px, py) = c.point(through)?;
    let r = radius(length(px - cx, py - cy))?;
    Some(Value::Circle { cx, cy, r })
}

pub(crate) fn with_radius(c: &Construction, center: Id, radius_id: Id) -> Option<Value> {
    let (cx, cy) = c.point(center)?;
    let r = radius(c.number(radius_id)?)?;
    Some(Value::Circle { cx, cy, r })
}
