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
