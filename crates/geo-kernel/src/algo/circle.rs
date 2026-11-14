//! Circles given by a centre and a point or a radius.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCirclePointRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSphereNDPointRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::length;
use crate::kernel::{Construction, Id, Value};
use crate::tol::STANDARD_PRECISION;

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
    // GeoQuadricND.setSphereND: a radius that is zero up to the standard
    // precision becomes 0 (a single-point circle), any other negative radius
    // makes the circle undefined.
    let r = if -STANDARD_PRECISION < r && r < STANDARD_PRECISION {
        0.0
    } else if r < 0.0 {
        return None;
    } else {
        r
    };
    Some(Value::Circle { cx, cy, r })
}
