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

use super::{conic::ConicDef, length};
use crate::kernel::{Construction, Def, Id, Value};
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
    with_radius_value(c, center, c.number(radius_id)?)
}

pub(crate) fn with_radius_value(c: &Construction, center: Id, r: f64) -> Option<Value> {
    let (cx, cy) = c.point(center)?;
    let r = radius(r).filter(|r| r.is_finite())?;
    Some(Value::Circle { cx, cy, r })
}

/// Whether the circle command behind `def` leaves the circle's last record
/// in place for the current inputs: `GeoQuadricND.setSphereND` returns
/// before it writes the matrix when the centre is undefined or the radius
/// is negative, and `GeoConicND.setCircle(M, P)` when either point is
/// undefined. Only the flag `defined` changes, so the half axes, midpoint
/// and type stay those of the last circle written. An undefined radius
/// (`NaN`) with a defined centre is written, as `NaN`. `None` for any other
/// definition.
pub(crate) fn keeps_record(c: &Construction, def: Def) -> Option<bool> {
    match def {
        Def::CircleRadius { center, radius } => Some(
            c.point(center).is_none() || c.number(radius).is_some_and(|r| r <= -STANDARD_PRECISION),
        ),
        Def::CircleThrough { center, through } => {
            Some(c.point(center).is_none() || c.point(through).is_none())
        }
        Def::Conic(ConicDef::LiteralRadius { center, .. }) => Some(c.point(center).is_none()),
        // GeoConicND.java (setCircle with a segment) checks both inputs
        // before it writes the circle matrix.
        Def::Conic(ConicDef::SegmentRadius { center, segment }) => {
            Some(c.point(center).is_none() || !c.value(segment).is_defined())
        }
        _ => None,
    }
}

/// The matrix setter runs for an undefined numeric radius with a defined
/// centre. Only the radius-dependent matrix entry is NaN.
pub(crate) fn undefined_radius_record(c: &Construction, def: Def) -> Option<(f64, f64, f64)> {
    let Def::CircleRadius { center, radius } = def else {
        return None;
    };
    let (cx, cy) = c.point(center)?;
    (c.number(radius).is_none() && c.infinite_number(radius).is_none()).then_some((
        cx,
        cy,
        f64::NAN,
    ))
}

#[cfg(test)]
mod tests {
    use crate::kernel::Value;
    use crate::script::load;

    #[test]
    fn a_radius_near_zero_is_zero_and_a_negative_one_undefined() {
        let mut c = load("A = (1, 2)\nr = 3\nk = Circle(A, r)\n").unwrap();
        let (r, k) = (c.id("r").unwrap(), c.id("k").unwrap());
        for (v, expected) in [
            (1e-9, Some(0.0)),
            (-1e-9, Some(0.0)),
            (0.0, Some(0.0)),
            (2e-8, Some(2e-8)),
            (-2e-8, None),
            (-1.5, None),
        ] {
            c.set_number(r, v).unwrap();
            let got = match c.value(k) {
                Value::Circle { r, .. } => Some(r),
                _ => None,
            };
            assert_eq!(got, expected, "radius {v}");
        }
    }
}
