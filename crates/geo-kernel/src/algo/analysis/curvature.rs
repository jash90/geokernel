// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoCurvatureCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoCurvatureVectorCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoOsculatingCircle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::algo::{
    curve::{slope, Cartesian},
    path::PathState,
};
use crate::expr::derivative::FunctionDerivative;
use crate::tol::MIN_PRECISION;
use crate::{Construction, Id, Kind, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurvatureKind {
    Scalar,
    Vector,
    Circle,
}

impl CurvatureKind {
    pub(super) fn kind(self) -> Kind {
        match self {
            Self::Scalar => Kind::Number,
            Self::Vector => Kind::Vector,
            Self::Circle => Kind::Circle,
        }
    }
}

pub(super) fn compute(
    c: &Construction,
    point: Id,
    source: Id,
    output: CurvatureKind,
) -> Option<Value> {
    let (px, py) = c.point(point)?;
    let (curvature, vector) = match c.kind(source) {
        Kind::Function => {
            let f = FunctionDerivative::new(c, source)?;
            let on_path = (f.value(px) - py).abs() < MIN_PRECISION;
            if output == CurvatureKind::Circle && !on_path {
                return None;
            }
            let first = f.slope(px);
            let second = slope(|x| f.slope(x), px);
            let s2 = 1.0 + first * first;
            (
                second / s2.powf(1.5),
                (-first * second / (s2 * s2), second / (s2 * s2)),
            )
        }
        Kind::Curve => {
            let curve = Cartesian::new(c, source)?;
            let t = curve.closest((px, py));
            let (x, y) = curve.value(t);
            if output == CurvatureKind::Circle && (px - x).hypot(py - y) >= MIN_PRECISION {
                return None;
            }
            let (dx, dy) = curve.first(t);
            let (ddx, ddy) = curve.second(t);
            let s2 = dx * dx + dy * dy;
            let cross = dx * ddy - dy * ddx;
            (
                cross / s2.powf(1.5),
                (-dy * cross / (s2 * s2), dx * cross / (s2 * s2)),
            )
        }
        _ => {
            let conic = c
                .conic(source)
                .or_else(|| c.arc(source).map(|arc| arc.conic))?;
            if output == CurvatureKind::Circle && !conic.on_full_conic((px, py), MIN_PRECISION) {
                return None;
            }
            let state = PathState::saved(c, source, px, py, None);
            let (x, y, z) = state.coords();
            let (x, y) = (x / z, y / z);
            let [a, b, _, d, e, f] = conic.matrix;
            let (fx, fy) = (a * x + d * y + e, b * y + d * x + f);
            let k =
                (-fy * fy * a + 2.0 * fx * fy * d - fx * fx * b) / (fx * fx + fy * fy).powf(1.5);
            let norm = fx.hypot(fy);
            (k.abs(), (fx * k / norm, fy * k / norm))
        }
    };
    let (vx, vy) = vector;
    match output {
        CurvatureKind::Scalar => curvature.is_finite().then_some(Value::Number(curvature)),
        CurvatureKind::Vector => {
            (vx.is_finite() && vy.is_finite()).then_some(Value::Vector { x: vx, y: vy })
        }
        CurvatureKind::Circle => {
            let r = vx.hypot(vy).recip();
            let radius_squared = r * r;
            let (cx, cy) = (px + radius_squared * vx, py + radius_squared * vy);
            (r.is_finite() && cx.is_finite() && cy.is_finite()).then_some(Value::Circle {
                cx,
                cy,
                r,
            })
        }
    }
}
