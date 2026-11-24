// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PathNormalizer.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! A conic as a path: `GeoConicND.pointChanged`, `pathChanged` and the
//! parameter range, for circles and general conics.

use super::conic_nd::{my_length, ConicData, ConicType};
use super::path::{
    inf_function, inverse_inf_function, line_path_changed, line_point_changed, PathState,
};
use std::f64::consts::PI;

/// The number GeoGebra gives each conic type (`GeoConicNDConstants`), which
/// a path parameter records as its path type.
pub(crate) fn type_number(ty: ConicType) -> i32 {
    ty as i32
}

/// `GeoConicND.getMinParameter` and `getMaxParameter`.
pub(crate) fn range(d: &ConicData) -> (f64, f64) {
    match d.ty {
        ConicType::DoubleLine | ConicType::Parabola => (f64::NEG_INFINITY, f64::INFINITY),
        ConicType::Circle | ConicType::Ellipse => (-PI, PI),
        ConicType::Hyperbola | ConicType::IntersectingLines | ConicType::ParallelLines => {
            (-1.0, 3.0)
        }
        _ => (0.0, 0.0),
    }
}

/// `GeoConicND.coordsRWtoEV`: real world to eigenvector coordinates.
pub(crate) fn rw_to_ev(d: &ConicData, p: &mut PathState) {
    let (mut x, mut y, z) = p.coords();
    let [mx, my] = d.midpoint;
    x -= z * mx;
    y -= z * my;
    let [e0, e1] = d.eigenvec;
    p.set_coords_2d(x * e0[0] + y * e0[1], x * e1[0] + y * e1[1], z);
}

/// `GeoConicND.coordsEVtoRW`.
pub(crate) fn ev_to_rw(d: &ConicData, p: &mut PathState) {
    let (x, y, z) = p.coords();
    let [e0, e1] = d.eigenvec;
    let rx = x * e0[0] + y * e1[0];
    let ry = x * e0[1] + y * e1[1];
    let [mx, my] = d.midpoint;
    p.set_coords_2d(rx + z * mx, ry + z * my, z);
}

/// `GeoLine.distanceHom(Coords)`.
fn distance_hom(g: [f64; 3], (x, y, z): (f64, f64, f64)) -> f64 {
    ((g[0] * x / z + g[1] * y / z + g[2]) / my_length(g[0], g[1])).abs()
}

fn line(g: [f64; 3]) -> (f64, f64, f64) {
    (g[0], g[1], g[2])
}

/// The point a line of a degenerate conic measures its parameter from:
/// `setStartPointsForLines`, which resets the line's start point and takes
/// `GeoLine.getPointOnLine` — an axis point, `-z/x` on the x-axis when `x`
/// is the larger coefficient, else `-z/y` on the y-axis.
fn axis_start([x, y, z]: [f64; 3]) -> (f64, f64, f64) {
    if x.abs() > y.abs() {
        (-z / x, 0.0, 1.0)
    } else {
        (0.0, -z / y, 1.0)
    }
}

/// `GeoConicND.pointChanged`, which is `pointChangedUnlimited` for a full
/// conic.
pub(crate) fn point_changed(d: &ConicData, p: &mut PathState) {
    p.set_type(type_number(d.ty));
    match d.ty {
        ConicType::SinglePoint => {
            let [mx, my] = d.midpoint;
            p.set_coords_2d(mx, my, 1.0);
        }
        ConicType::IntersectingLines | ConicType::ParallelLines => {
            // the closer line
            let first =
                distance_hom(d.lines[0], p.coords()) <= distance_hom(d.lines[1], p.coords());
            let g = if first { d.lines[0] } else { d.lines[1] };
            line_point_changed(line(g), Some(axis_start(g)), p);
            // the line parameter in (-1, 1), or in (1, 3) for the second line
            let t = inverse_inf_function(p.parameter());
            p.set_t(t);
            if !first {
                p.set_t(p.parameter() + 2.0);
            }
        }
        ConicType::DoubleLine => {
            line_point_changed(line(d.lines[0]), Some(axis_start(d.lines[0])), p)
        }
        ConicType::Circle => {
            rw_to_ev(d, p);
            let (x, y, z) = p.coords();
            let (px, py) = (x / z, y / z);
            let [ha, hb] = d.half_axes;
            p.set_t((ha * py).atan2(hb * px));
            let t = p.parameter();
            p.set_coords_2d(ha * t.cos(), hb * t.sin(), 1.0);
            ev_to_rw(d, p);
        }
        ConicType::Ellipse | ConicType::Hyperbola | ConicType::Parabola => {
            rw_to_ev(d, p);
            let (x, y, z) = p.coords();
            let (x, y) = (x / z, y / z);
            let t = match d.ty {
                ConicType::Ellipse => ellipse_parameter(d.half_axes, x, y),
                ConicType::Hyperbola => hyperbola_parameter(d.half_axes, x, y),
                _ => parabola_parameter(d.p, x, y),
            };
            p.set_t(t);
            curved_at(d, p);
        }
        _ => p.set_coords_2d(f64::NAN, f64::NAN, f64::NAN),
    }
}

/// `GeoConicND.pathChanged`: the point at its parameter, or projected when
/// the parameter was made for another type of conic.
pub(crate) fn path_changed(d: &ConicData, p: &mut PathState) {
    let ty = type_number(d.ty);
    // `compatibleType`: once the conic is back from a single point or an
    // empty conic, the old parameter is used
    let pt = p.path_type();
    let compatible = pt == ty
        || pt == type_number(ConicType::Empty)
        || pt == type_number(ConicType::SinglePoint);
    if !compatible || p.parameter().is_nan() {
        point_changed(d, p);
        return;
    }
    match d.ty {
        ConicType::SinglePoint => {
            let [mx, my] = d.midpoint;
            p.set_coords_2d(mx, my, 1.0);
        }
        ConicType::IntersectingLines | ConicType::ParallelLines => {
            let param = p.parameter();
            let second = param > 1.0;
            p.set_t(if second { param - 2.0 } else { param });
            // from (-1, 1) to the line parameter in (-inf, inf)
            let t = p.parameter();
            p.set_t(t / (1.0 - t.abs()));
            let g = d.lines[usize::from(second)];
            line_path_changed(line(g), Some(axis_start(g)), p);
            p.set_t(param);
        }
        ConicType::DoubleLine => {
            line_path_changed(line(d.lines[0]), Some(axis_start(d.lines[0])), p)
        }
        ConicType::Circle => {
            let t = p.parameter();
            let [ha, hb] = d.half_axes;
            p.set_coords_2d(ha * t.cos(), hb * t.sin(), 1.0);
            ev_to_rw(d, p);
        }
        ConicType::Ellipse | ConicType::Hyperbola | ConicType::Parabola => curved_at(d, p),
        _ => p.set_coords_2d(f64::NAN, f64::NAN, f64::NAN),
    }
}

/// The first-quadrant solution of the ellipse normal equation. Axis cases
/// choose the same one of the equal minima as GeoGebra.
fn ellipse_parameter([a, b]: [f64; 2], x: f64, y: f64) -> f64 {
    let (px, py) = (x.abs(), y.abs());
    let c2 = a * a - b * b;
    let mut t = if px < crate::tol::STANDARD_PRECISION {
        PI / 2.0
    } else if py < crate::tol::STANDARD_PRECISION {
        (a * px / c2).min(1.0).acos()
    } else if !px.is_finite() || !py.is_finite() {
        // The normal equation has no usable root for these coordinates.
        // A fresh projection keeps its initial first-quadrant parameter.
        0.0
    } else {
        super::curve::bisect(0.0, PI / 2.0, |t| {
            let (sin, cos) = t.sin_cos();
            a * px * sin - b * py * cos - c2 * sin * cos
        })
    };
    if x < 0.0 {
        t = PI - t;
    }
    if y < 0.0 {
        -t
    } else {
        t
    }
}

fn hyperbola_parameter([a, b]: [f64; 2], x: f64, y: f64) -> f64 {
    let (px, py) = (x.abs(), y.abs());
    let c2 = a * a + b * b;
    let eps = crate::tol::STANDARD_PRECISION;
    let u = if py < eps {
        (a * px / c2).max(1.0).acosh().sinh()
    } else if px < eps {
        b * py / c2
    } else {
        super::curve::bisect(0.0, (a * px + b * py) / c2, |u| {
            c2 * u - a * px * u / (1.0 + u * u).sqrt() - b * py
        })
    };
    let s = if y < 0.0 { -u.asinh() } else { u.asinh() };
    inverse_inf_function(s) + if x < 0.0 { 2.0 } else { 0.0 }
}

fn parabola_parameter(p: f64, x: f64, y: f64) -> f64 {
    let py = y.abs();
    if py < crate::tol::STANDARD_PRECISION {
        return (2.0 * (x - p) / p).max(0.0).sqrt();
    }
    let bound = (2.0 * x.abs() / p).sqrt() + (2.0 * py / p).cbrt() + 1.0;
    let t = super::curve::bisect(0.0, bound, |t| p * t * t * t / 2.0 + (p - x) * t - py);
    if y < 0.0 {
        -t
    } else {
        t
    }
}

fn curved_at(d: &ConicData, p: &mut PathState) {
    let t = p.parameter();
    let [a, b] = d.half_axes;
    let (x, y) = match d.ty {
        ConicType::Ellipse => (a * t.cos(), b * t.sin()),
        ConicType::Hyperbola if t > 1.0 => {
            let s = inf_function(t - 2.0);
            (-a * s.cosh(), b * s.sinh())
        }
        ConicType::Hyperbola => {
            let s = inf_function(t);
            (a * s.cosh(), b * s.sinh())
        }
        _ => (d.p * t * t / 2.0, d.p * t),
    };
    p.set_coords_2d(x, y, 1.0);
    ev_to_rw(d, p);
}
