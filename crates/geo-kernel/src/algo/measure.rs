// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLength.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLengthSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLengthVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoLengthFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoLengthFunction2Points.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPerimeter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdCircumference.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircumferenceConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdArea.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSlope.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSlope.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdDistance.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoClosestPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistanceLineLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAffineRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAffineRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdCrossRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoCrossRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/Coords.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAnglePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoConstructionStep.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoStepObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAxisStep.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoAngle.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Numbers measured on objects: length, radius, circumference, area, slope,
//! distances, affine and cross ratio, and the angle of three points.
//!
//! Zero tests on lines use GeoGebra's raw coefficients, which the kernel
//! keeps (see [`Value::Line`]).
//!
//! GeoGebra decides "is this zero" with the absolute tolerance
//! [`STANDARD_PRECISION`]; the same tests are used here so that degenerate
//! cases switch at the same threshold.

use super::conic_nd::ConicType;
use super::point::{closest_coords, CLOSEST_PATH};
use super::polygon::{count_arg, Count};
use super::{conic_object, Group, Parents, CARRIERS, CONIC_OBJECTS, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;
use std::f64::consts::PI;

/// Numbers measured on objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureDef {
    /// `Length(segment)`, `Length(point)` or the sample count of a locus.
    Length(Id),
    TextLength(usize),
    /// A Cartesian curve with two coordinate functions and an interval.
    CartesianCurve {
        coordinates: [u32; 2],
        bounds: [u32; 2],
    },
    /// The summed distances between consecutive points of a locus.
    LocusPerimeter(Id),
    /// Graph or curve length between numeric bounds; the id is packed.
    FunctionLength {
        function: u32,
        start: Count,
        end: Count,
    },
    /// Graph or curve length between endpoints given as points.
    FunctionLengthBetweenPoints {
        function: Id,
        start: Id,
        end: Id,
    },
    /// `Radius(conic)` of a circle or single-point conic.
    Radius(Id),
    /// `Circumference(conic)` and `Perimeter(conic)` of a full conic.
    Circumference(Id),
    /// `Area(conic)` of a bounded full conic.
    CircleArea(Id),
    /// `Area(P, Q, R)`: area of the triangle.
    Area3(Id, Id, Id),
    /// `Area(P, Q, R, S)`: area of the quadrilateral through the points in order.
    Area4(Id, Id, Id, Id),
    /// `Slope(line)` or the derivative of a function at zero.
    Slope(Id),
    /// `Distance(point, path)` in either order of the arguments.
    DistancePointPath {
        point: Id,
        path: Id,
    },
    /// `Distance(line, line)`; segments count as the lines they lie on.
    DistanceLines(Id, Id),
    /// `AffineRatio(A, B, C)`.
    AffineRatio(Id, Id, Id),
    /// `CrossRatio(A, B, C, D)`.
    CrossRatio(Id, Id, Id, Id),
    /// `Angle(A, B, C)`: the angle at `B`, counter-clockwise from `BA` to `BC`.
    Angle(Id, Id, Id),
    /// `ConstructionStep()`, or of one object (`AlgoConstructionStep`,
    /// `AlgoStepObject`): the step counter, or the object's construction
    /// index, plus one.
    ConstructionStep(Option<Id>),
    /// `AxisStepX()` and `AxisStepY()` (`AlgoAxisStep`): the grid distance
    /// of the graphics view the kernel reads.
    AxisStep {
        y: bool,
    },
}

/// `-eps < v < eps` (GeoGebra's `DoubleUtil.isZero`).
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// GeoGebra's `DoubleUtil.isEqual(a, b)`.
fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

/// GeoGebra's `MyMath.length`: no overflow, no underflow.
pub(super) fn my_length(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    }
}

/// Radius of a circle as GeoGebra reports it: a radius below the tolerance
/// makes the conic a single point of radius 0.
fn circle_radius(c: &Construction, id: Id) -> Option<f64> {
    let (_, _, r) = c.circle(id)?;
    Some(if is_zero(r) { 0.0 } else { r })
}

/// Length of a segment; GeoGebra sets lengths below the tolerance to 0.
pub(crate) fn segment_length(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let length = my_length(x2 - x1, y2 - y1);
    if is_zero(length) {
        0.0
    } else {
        length
    }
}

/// `GeoLine.getSlope`: undefined for a vertical line.
fn slope(x: f64, y: f64) -> Option<f64> {
    if is_zero(y) {
        None
    } else {
        Some(-x / y)
    }
}

/// `GeoPoint.collinearND` for points of the plane: the cross product of
/// `A - B` and `A - C` vanishes.
fn collinear(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let (bx, by) = (a.0 - b.0, a.1 - b.1);
    let (cx, cy) = (a.0 - c.0, a.1 - c.1);
    is_zero(bx * cy - by * cx)
}

fn same_point(p: (f64, f64), q: (f64, f64)) -> bool {
    is_equal(p.0, q.0) && is_equal(p.1, q.1)
}

/// `GeoPoint.affineRatio`: `lambda` with `pt2 = start + lambda * (pt1 - start)`.
fn affine_ratio(start: (f64, f64), pt1: (f64, f64), pt2: (f64, f64)) -> f64 {
    let abx = pt1.0 - start.0;
    let aby = pt1.1 - start.1;
    if abx.abs() > aby.abs() {
        (pt2.0 - start.0) / abx
    } else if aby.abs() > 0.0 {
        (pt2.1 - start.1) / aby
    } else {
        f64::NAN
    }
}

/// `DoubleUtil.convertToAngleValue`: an angle in `[0, 2 pi]`.
fn angle_value(val: f64) -> f64 {
    const TWO_PI: f64 = 2.0 * PI;
    if val > STANDARD_PRECISION && val < TWO_PI {
        return val;
    }
    let mut value = val % TWO_PI;
    if is_zero(value) {
        value = if val < 1.0 { 0.0 } else { TWO_PI };
    } else if value < 0.0 {
        value += TWO_PI;
    }
    value
}

/// Twice the signed area of the polygon through `pts` (`AlgoPolygon.calcAreaWithSign`).
fn polygon_area(pts: &[(f64, f64)]) -> f64 {
    let last = pts.len() - 1;
    let mut sum = 0.0;
    for i in 0..last {
        sum += pts[i].0 * pts[i + 1].1 - pts[i + 1].0 * pts[i].1;
    }
    sum += pts[last].0 * pts[0].1 - pts[0].0 * pts[last].1;
    sum / 2.0
}

/// `AlgoAnglePoints`: the angle at `pb`, counter-clockwise from `pa - pb`
/// to `pd - pb`, in `[0, 2 pi]`; `None` when either vector is zero.
pub(crate) fn angle_of_points(pa: (f64, f64), pb: (f64, f64), pd: (f64, f64)) -> Option<f64> {
    let (vx, vy) = (pa.0 - pb.0, pa.1 - pb.1);
    let (wx, wy) = (pd.0 - pb.0, pd.1 - pb.1);
    if (is_zero(vx) && is_zero(vy)) || (is_zero(wx) && is_zero(wy)) {
        return None;
    }
    let det = vx * wy - vy * wx;
    let prod = vx * wx + vy * wy;
    Some(angle_value(det.atan2(prod)))
}

/// Distance of a point to a segment: to the closest point of the segment.
fn point_segment_distance(p: (f64, f64), x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let length = segment_length(x1, y1, x2, y2);
    if length == 0.0 {
        return my_length(p.0 - x1, p.1 - y1);
    }
    let (dx, dy) = (x2 - x1, y2 - y1);
    let (px, py) = (p.0 - x1, p.1 - y1);
    let t = (px * dx + py * dy) / (dx * dx + dy * dy);
    if t < 0.0 {
        my_length(px, py)
    } else if t > 1.0 {
        my_length(p.0 - x2, p.1 - y2)
    } else {
        (px * dy - py * dx).abs() / my_length(dx, dy)
    }
}

/// `AlgoDistancePointObject` for a segment whose line coefficients
/// `(x, y, z)` were copied from another line: the distance to its
/// `AlgoClosestPoint`. That algorithm sets the closest point to `p`, which
/// already runs `GeoSegment.pointChanged` because the point is on the path,
/// and then runs `pointChanged` again from the result. Each pass projects on
/// the copied line, reads the parameter from the start point along
/// `(y, -x)` (`GeoLine.projectCoordsAndComputePathParam`) and clamps it to an
/// end point. With copied coefficients the second pass can move the point
/// again; for a segment of length zero both passes give the start point.
fn carried_segment_distance(
    p: (f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    (x, y, z): (f64, f64, f64),
) -> f64 {
    let pass = |q: (f64, f64)| {
        if start == end {
            return start;
        }
        let t = -(z + x * q.0 + y * q.1) / (x * x + y * y);
        let (px, py) = (q.0 + t * x, q.1 + t * y);
        let parameter = if x.abs() <= y.abs() {
            (px - start.0) / y
        } else {
            (start.1 - py) / x
        };
        if parameter < 0.0 {
            start
        } else if parameter > 1.0 {
            end
        } else {
            (px, py)
        }
    };
    let closest = pass(pass(p));
    my_length(p.0 - closest.0, p.1 - closest.1)
}

/// `GeoLine.distance(GeoLine)` for `this` = `g` and the argument `h`:
/// zero unless the lines are parallel.
fn line_line_distance(g: (f64, f64, f64), h: (f64, f64, f64)) -> f64 {
    if is_zero(h.0 * g.1 - h.1 * g.0) {
        // A point of h.
        let (px, py) = if h.0.abs() > h.1.abs() {
            (-h.2 / h.0, 0.0)
        } else {
            (0.0, -h.2 / h.1)
        };
        ((g.0 * px + g.1 * py + g.2) / my_length(g.0, g.1)).abs()
    } else {
        0.0
    }
}

/// The construction index of `id`: its place in the construction, or -1
/// for the built-in axes (`GeoElement.getConstructionIndex`).
fn id_index(c: &Construction, id: Id) -> i32 {
    if c.label(id).is_empty() {
        -1
    } else {
        c.element_of(id)
    }
}

/// `ConstructionStep()` and `ConstructionStep(object)` (`AlgoConstructionStep`,
/// `AlgoStepObject`); `AxisStepX()` and `AxisStepY()` (`AlgoAxisStep`).
fn parse_construction_step(
    c: &Construction,
    func: &str,
    args: &[&str],
) -> Result<MeasureDef, String> {
    // An empty pair of parentheses is no argument, not one empty one.
    let args = if args == [""] { &[][..] } else { args };
    match func {
        "AxisStepX" | "AxisStepY" => {
            if !args.is_empty() {
                return Err(format!("`{func}` does not take {} argument(s)", args.len()));
            }
            Ok(MeasureDef::AxisStep {
                y: func == "AxisStepY",
            })
        }
        _ => match args {
            [] => Ok(MeasureDef::ConstructionStep(None)),
            [object] => Ok(MeasureDef::ConstructionStep(Some(crate::script::object(
                c, object,
            )?))),
            _ => Err(format!("`{func}` does not take {} argument(s)", args.len())),
        },
    }
}

/// `EuclidianView.setAxesIntervals` with the automatic grid of the default
/// configuration: the axis numbering step at `scale` pixels per unit, one
/// number per 100 pixels, cleaned of binary noise; the grid distance is
/// the same (`DEFAULT_GRID_DIST_FACTOR` 1). The y axis copies the x step
/// when both scales agree.
fn axis_grid_step(view: [f64; 6], y: bool) -> f64 {
    let [_xmin, _xmax, _ymin, _ymax, xscale, yscale] = view;
    // each axis has its own grid distance, from its own scale
    // (`getGridDistances`)
    if y {
        numbering_step(yscale)
    } else {
        numbering_step(xscale)
    }
}

/// `setAxesIntervals`: with `units = maxPix / scale`, `pot` its power of
/// ten and `n = units / pot`, the step is `5 pot` for `n > 5`, `2 pot` for
/// `n > 2` and `pot` otherwise.
fn numbering_step(scale: f64) -> f64 {
    let units = 100.0 / scale;
    if !(units.is_finite() && units > 0.0) {
        return 1.0;
    }
    let exp = units.log10().floor();
    // "see #2682": both are cleaned of binary noise.
    let pot = check_decimal_fraction(10f64.powf(exp));
    let n = check_decimal_fraction(units / pot);
    if n > 5.0 {
        5.0 * pot
    } else if n > 2.0 {
        2.0 * pot
    } else {
        pot
    }
}

/// `DoubleUtil.checkDecimalFraction`: a value within `1e-16` of a
/// multiple of `1e-5` snaps to it.
fn check_decimal_fraction(val: f64) -> f64 {
    let frac = val * 1e5;
    let round = crate::expr::math::java_round(frac) as f64;
    if is_equal(frac, round) || (frac - round).abs() <= 1e-16 {
        round / 1e5
    } else {
        val
    }
}

impl Group for MeasureDef {
    fn kind(&self) -> Kind {
        if matches!(self, MeasureDef::CartesianCurve { .. }) {
            Kind::Curve
        } else {
            Kind::Number
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            MeasureDef::TextLength(_) => Parents::none(),
            MeasureDef::CartesianCurve {
                coordinates: [x, y],
                bounds: [a, b],
            } => Parents::of(&[x, y, a, b].map(|id| Id(id as usize))),
            MeasureDef::Length(a)
            | MeasureDef::LocusPerimeter(a)
            | MeasureDef::Radius(a)
            | MeasureDef::Circumference(a)
            | MeasureDef::CircleArea(a)
            | MeasureDef::Slope(a) => Parents::of(&[a]),
            MeasureDef::FunctionLength {
                function,
                start,
                end,
            } => {
                let function = Id(function as usize);
                match (start.id(), end.id()) {
                    (Some(a), Some(b)) => Parents::of(&[function, a, b]),
                    (Some(id), None) | (None, Some(id)) => Parents::of(&[function, id]),
                    (None, None) => Parents::of(&[function]),
                }
            }
            MeasureDef::FunctionLengthBetweenPoints {
                function,
                start,
                end,
            } => Parents::of(&[function, start, end]),
            MeasureDef::DistancePointPath { point, path } => Parents::of(&[point, path]),
            MeasureDef::DistanceLines(g, h) => Parents::of(&[g, h]),
            MeasureDef::Area3(a, b, c)
            | MeasureDef::AffineRatio(a, b, c)
            | MeasureDef::Angle(a, b, c) => Parents::of(&[a, b, c]),
            MeasureDef::Area4(a, b, c, d) | MeasureDef::CrossRatio(a, b, c, d) => {
                Parents::of(&[a, b, c, d])
            }
            MeasureDef::ConstructionStep(object) => {
                object.map_or_else(Parents::none, |id| Parents::of(&[id]))
            }
            // The view is not an object; `set_view` recomputes these.
            MeasureDef::AxisStep { .. } => Parents::none(),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            MeasureDef::TextLength(_) => Ok(()),
            MeasureDef::CartesianCurve {
                coordinates: [x, y],
                bounds: [a, b],
            } => {
                for id in [x, y] {
                    c.expect(Id(id as usize), &[Kind::Function])?;
                }
                for id in [a, b] {
                    c.expect(Id(id as usize), &[Kind::Number])?;
                }
                Ok(())
            }
            MeasureDef::Length(s) => c.expect(s, &[Kind::Segment, Kind::Point, Kind::Locus]),
            MeasureDef::LocusPerimeter(locus) => c.expect(locus, &[Kind::Locus]),
            MeasureDef::FunctionLength {
                function,
                start,
                end,
            } => {
                c.expect(Id(function as usize), &[Kind::Function, Kind::Curve])?;
                start.check(c)?;
                end.check(c)
            }
            MeasureDef::FunctionLengthBetweenPoints {
                function,
                start,
                end,
            } => {
                c.expect(function, &[Kind::Function, Kind::Curve])?;
                c.expect(start, POINT)?;
                c.expect(end, POINT)
            }
            MeasureDef::Radius(k) | MeasureDef::CircleArea(k) => c.expect(k, CONIC_OBJECTS),
            MeasureDef::Circumference(k) => c.expect(k, &[Kind::Circle, Kind::Conic]),
            MeasureDef::Slope(l) => {
                c.expect(l, &[Kind::Line, Kind::Segment, Kind::Ray, Kind::Function])
            }
            MeasureDef::DistancePointPath { point, path } => {
                c.expect(point, POINT)?;
                c.expect(path, CLOSEST_PATH)
            }
            MeasureDef::ConstructionStep(object) => {
                let _ = object;
                Ok(())
            }
            MeasureDef::AxisStep { .. } => Ok(()),
            MeasureDef::DistanceLines(g, h) => {
                c.expect(g, CARRIERS)?;
                c.expect(h, CARRIERS)
            }
            MeasureDef::Area3(a, b, d) | MeasureDef::AffineRatio(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            MeasureDef::Angle(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            MeasureDef::Area4(a, b, d, e) | MeasureDef::CrossRatio(a, b, d, e) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)?;
                c.expect(e, POINT)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let number = |v: f64| Some(Value::Number(v));
        match *self {
            MeasureDef::TextLength(n) => number(n as f64),
            MeasureDef::ConstructionStep(object) => {
                let step = match object {
                    // `AlgoStepObject`: the construction index of the
                    // object's own entry.
                    Some(id) => id_index(c, id),
                    None => c.step(),
                };
                number(f64::from(step + 1))
            }
            MeasureDef::AxisStep { y } => number(axis_grid_step(c.view(), y)),
            MeasureDef::CartesianCurve {
                coordinates,
                bounds: [a, b],
            } => {
                if !coordinates
                    .iter()
                    .all(|&id| c.value(Id(id as usize)).is_defined())
                {
                    return None;
                }
                let (start, end) = (c.number(Id(a as usize))?, c.number(Id(b as usize))?);
                (start <= end).then_some(Value::Curve { start, end })
            }
            MeasureDef::Length(s) => match c.value(s) {
                Value::Segment { x1, y1, x2, y2 } => number(segment_length(x1, y1, x2, y2)),
                Value::Locus { len, .. } => number(f64::from(len)),
                _ => {
                    let (x, y) = c.point(s)?;
                    number(my_length(x, y))
                }
            },
            MeasureDef::LocusPerimeter(id) => {
                let locus = c.locus(id)?;
                if !locus.defined || locus.points.is_empty() {
                    return None;
                }
                number(
                    locus
                        .points
                        .windows(2)
                        .map(|ends| {
                            let dx = ends[1].x - ends[0].x;
                            let dy = ends[1].y - ends[0].y;
                            (dx * dx + dy * dy).sqrt()
                        })
                        .sum(),
                )
            }
            MeasureDef::FunctionLength {
                function,
                start,
                end,
            } => {
                let id = Id(function as usize);
                let (a, b) = (start.value(c), end.value(c));
                if c.kind(id) == Kind::Curve {
                    number(super::curve::Cartesian::new(c, id)?.length(a, b))
                } else {
                    number(super::curve::length(c, id, a, b)?)
                }
            }
            MeasureDef::FunctionLengthBetweenPoints {
                function,
                start,
                end,
            } => {
                let (a, b) = (c.point(start)?, c.point(end)?);
                if c.kind(function) == Kind::Curve {
                    let curve = super::curve::Cartesian::new(c, function)?;
                    number(curve.length(curve.closest(a), curve.closest(b)))
                } else {
                    number(super::curve::length(c, function, a.0, b.0)?)
                }
            }
            MeasureDef::Radius(k) => {
                if c.kind(k) == Kind::Circle {
                    return number(circle_radius(c, k).or_else(|| {
                        super::circle::infinite_center_record(c, c.def(k)).map(|(_, _, r)| r)
                    })?);
                }
                let d = conic_object(c, k)?;
                match d.ty {
                    ConicType::Circle => number(d.half_axes[0]),
                    ConicType::SinglePoint => number(0.0),
                    _ => None,
                }
            }
            MeasureDef::Circumference(k) => {
                if c.kind(k) == Kind::Circle {
                    let r = circle_radius(c, k)?;
                    return (r != 0.0).then_some(Value::Number(2.0 * r * PI));
                }
                let d = conic_object(c, k)?;
                match d.ty {
                    ConicType::Circle => number(2.0 * d.half_axes[0] * PI),
                    ConicType::Ellipse => number(super::elliptic::circumference(d.half_axes)),
                    _ => None,
                }
            }
            MeasureDef::CircleArea(k) => {
                let d = c.conic(k)?;
                if !d.defined {
                    return None;
                }
                let [a, b] = d.half_axes;
                match d.ty {
                    ConicType::Circle | ConicType::SinglePoint => number(a * a * PI),
                    ConicType::Ellipse => number(a * b * PI),
                    _ => None,
                }
            }
            MeasureDef::Area3(a, b, d) => {
                let pts = [c.point(a)?, c.point(b)?, c.point(d)?];
                number(polygon_area(&pts).abs())
            }
            MeasureDef::Area4(a, b, d, e) => {
                let pts = [c.point(a)?, c.point(b)?, c.point(d)?, c.point(e)?];
                number(polygon_area(&pts).abs())
            }
            MeasureDef::Slope(l) => {
                if c.kind(l) == Kind::Function {
                    let f = |x| c.function_value(l, x).unwrap_or(f64::NAN);
                    let derivative = super::curve::slope(f, 0.0);
                    return derivative.is_finite().then_some(Value::Number(derivative));
                }
                let (x, y, _) = c.raw_carrier(l)?;
                number(slope(x, y)?)
            }
            MeasureDef::DistancePointPath { point, path } => {
                let p = c.point(point)?;
                match c.value(path) {
                    // `GeoLine.distance(x0, y0)`
                    Value::Line { a, b, c: z } => {
                        number(((a * p.0 + b * p.1 + z) / my_length(a, b)).abs())
                    }
                    Value::Segment { x1, y1, x2, y2 } => match c.segment_carrier(path) {
                        Some(line) => number(carried_segment_distance(p, (x1, y1), (x2, y2), line)),
                        None => number(point_segment_distance(p, x1, y1, x2, y2)),
                    },
                    _ => {
                        let q = closest_coords(c, path, p)?;
                        number(my_length(p.0 - q.0, p.1 - q.1))
                    }
                }
            }
            MeasureDef::DistanceLines(g, h) => {
                number(line_line_distance(c.raw_carrier(g)?, c.raw_carrier(h)?))
            }
            MeasureDef::AffineRatio(a, b, d) => {
                let (pa, pb, pd) = (c.point(a)?, c.point(b)?, c.point(d)?);
                if !collinear(pa, pb, pd) {
                    return None;
                }
                if same_point(pb, pd) {
                    return number(1.0);
                }
                number(affine_ratio(pa, pb, pd))
            }
            MeasureDef::CrossRatio(a, b, d, e) => {
                let (pa, pb, pd, pe) = (c.point(a)?, c.point(b)?, c.point(d)?, c.point(e)?);
                if same_point(pa, pe)
                    || same_point(pb, pd)
                    || !collinear(pb, pd, pe)
                    || !collinear(pa, pd, pe)
                {
                    return None;
                }
                number(affine_ratio(pb, pd, pe) / affine_ratio(pa, pd, pe))
            }
            MeasureDef::Angle(a, b, d) => {
                number(angle_of_points(c.point(a)?, c.point(b)?, c.point(d)?)?)
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let n = args.len();
        if func == "Distance" {
            return parse_distance(c, args);
        }
        if let ("Length", [f, a, b]) = (func, args) {
            return Some(parse_function_length(c, f, a, b));
        }
        if let ("ConstructionStep" | "AxisStepX" | "AxisStepY", _) = (func, args) {
            return Some(parse_construction_step(c, func, args));
        }
        let known = matches!(
            func,
            "Length"
                | "Radius"
                | "Circumference"
                | "Perimeter"
                | "Area"
                | "Slope"
                | "AffineRatio"
                | "CrossRatio"
                | "Angle"
        );
        if !known {
            return None;
        }
        let wanted = match func {
            "Area" => matches!(n, 1 | 3 | 4),
            "AffineRatio" | "Angle" => n == 3,
            "CrossRatio" => n == 4,
            _ => n == 1,
        };
        if !wanted {
            return Some(Err(format!("`{func}` does not take {n} argument(s)")));
        }
        let mut ids = [Id(0); 4];
        for (slot, arg) in ids.iter_mut().zip(args) {
            match object(c, arg) {
                Ok(id) => *slot = id,
                Err(e) => return Some(Err(e)),
            }
        }
        let [a, b, d, e] = ids;
        Some(Ok(match (func, n) {
            ("Length", _) => MeasureDef::Length(a),
            ("Radius", _) => MeasureDef::Radius(a),
            ("Circumference" | "Perimeter", _) if c.kind(a) == Kind::Locus => {
                MeasureDef::LocusPerimeter(a)
            }
            ("Circumference" | "Perimeter", _) => MeasureDef::Circumference(a),
            ("Area", 1) => MeasureDef::CircleArea(a),
            ("Area", 3) => MeasureDef::Area3(a, b, d),
            ("Area", _) => MeasureDef::Area4(a, b, d, e),
            ("Slope", _) => MeasureDef::Slope(a),
            ("AffineRatio", _) => MeasureDef::AffineRatio(a, b, d),
            ("CrossRatio", _) => MeasureDef::CrossRatio(a, b, d, e),
            _ => MeasureDef::Angle(a, b, d),
        }))
    }
}

pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Vec<Id> {
    let curve = match *def {
        Def::Measure(MeasureDef::FunctionLength { function, .. }) => Id(function as usize),
        Def::Measure(MeasureDef::FunctionLengthBetweenPoints { function, .. }) => function,
        _ => return Vec::new(),
    };
    if let Def::Measure(d @ MeasureDef::CartesianCurve { .. }) = c.def(curve) {
        return d.parents().as_slice().to_vec();
    }
    Vec::new()
}

fn parse_function_length(
    c: &Construction,
    f: &str,
    a: &str,
    b: &str,
) -> Result<MeasureDef, String> {
    let function = object(c, f)?;
    c.expect(function, &[Kind::Function, Kind::Curve])
        .map_err(|err| err.to_string())?;
    if let (Some(start), Some(end)) = (c.id(a), c.id(b)) {
        if c.kind(start) == Kind::Point && c.kind(end) == Kind::Point {
            return Ok(MeasureDef::FunctionLengthBetweenPoints {
                function,
                start,
                end,
            });
        }
    }
    Ok(MeasureDef::FunctionLength {
        function: u32::try_from(function.0).map_err(|_| "function id is out of range")?,
        start: count_arg(c, a)?,
        end: count_arg(c, b)?,
    })
}

/// `Distance(point, path)`, either argument order, and `Distance(line, line)`.
/// The built-in command handles the distance between two points.
fn parse_distance(c: &Construction, args: &[&str]) -> Option<Result<MeasureDef, String>> {
    if args.len() != 2 {
        return None;
    }
    let (first, second) = (c.id(args[0])?, c.id(args[1])?);
    let line_like = |id: Id| CARRIERS.contains(&c.kind(id));
    let path = |id: Id| CLOSEST_PATH.contains(&c.kind(id));
    let def = match (c.kind(first), c.kind(second)) {
        (Kind::Point, _) if path(second) => MeasureDef::DistancePointPath {
            point: first,
            path: second,
        },
        (_, Kind::Point) if path(first) => MeasureDef::DistancePointPath {
            point: second,
            path: first,
        },
        _ if line_like(first) && line_like(second) => MeasureDef::DistanceLines(first, second),
        _ => return None,
    };
    Some(Ok(def))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{exec, load};

    fn num(c: &Construction, label: &str) -> Option<f64> {
        match c.value(c.id(label).unwrap()) {
            Value::Number(v) => Some(v),
            Value::Undefined => None,
            other => panic!("{label} is {other:?}"),
        }
    }

    fn close(a: Option<f64>, b: f64) {
        let a = a.expect("defined");
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }

    const BASE: &str = "A = (0, 0)\nB = (4, 0)\nP = (1, 3)\nC = (0, 3)\n\
                        s = Segment(A, B)\nl = Line(A, B)\nk = Circle(A, B)\n";

    #[test]
    fn point_length_measures_from_the_origin() {
        let mut c = load("A = (3, 4)\nn = Length((3, 4))\nd = Length(A)").unwrap();
        close(num(&c, "n"), 5.0);
        close(num(&c, "d"), 5.0);
        c.move_point(c.id("A").unwrap(), -5.0, 12.0).unwrap();
        close(num(&c, "d"), 13.0);
    }

    #[test]
    fn every_syntax_parses() {
        let mut src = String::from(BASE);
        for line in [
            "d1 = Length(s)",
            "d2 = Radius(k)",
            "d3 = Circumference(k)",
            "d4 = Perimeter(k)",
            "d5 = Area(k)",
            "d6 = Area(A, B, P)",
            "d7 = Area(A, B, P, C)",
            "d8 = Slope(l)",
            "d9 = Slope(s)",
            "e1 = Distance(P, l)",
            "e2 = Distance(P, s)",
            "e3 = Distance(l, P)",
            "e4 = Distance(l, s)",
            "e5 = AffineRatio(A, B, A)",
            "e6 = CrossRatio(A, B, A, B)",
            "e7 = Angle(B, A, C)",
        ] {
            src.push_str(line);
            src.push('\n');
        }
        let c = load(&src).unwrap();
        assert!(matches!(c.def(c.id("d1").unwrap()), Def::Measure(_)));
        close(num(&c, "d1"), 4.0);
        close(num(&c, "d2"), 4.0);
        close(num(&c, "d3"), 8.0 * PI);
        close(num(&c, "d4"), 8.0 * PI);
        close(num(&c, "d5"), 16.0 * PI);
        close(num(&c, "d6"), 6.0);
        close(num(&c, "d7"), 7.5);
        close(num(&c, "d8"), 0.0);
        close(num(&c, "d9"), 0.0);
        close(num(&c, "e1"), 3.0);
        close(num(&c, "e2"), 3.0);
        close(num(&c, "e3"), 3.0);
        close(num(&c, "e4"), 0.0);
        close(num(&c, "e7"), PI / 2.0);
    }

    #[test]
    fn distance_of_two_points_is_left_to_the_built_in_command() {
        let c = load("A = (0, 0)\nB = (3, 4)\nd = Distance(A, B)\n").unwrap();
        assert!(matches!(c.def(c.id("d").unwrap()), Def::Distance(..)));
        close(num(&c, "d"), 5.0);
    }

    #[test]
    fn wrong_argument_counts_and_kinds_are_errors() {
        let mut c = load(BASE).unwrap();
        exec(&mut c, "r = 2").unwrap();
        for (line, text) in [
            ("d = Length(s, s)", "`Length` does not take 2 argument(s)"),
            ("d = Radius(k, k)", "`Radius` does not take 2 argument(s)"),
            ("d = Area(A, B)", "`Area` does not take 2 argument(s)"),
            (
                "d = Area(A, B, P, C, k)",
                "`k` is a circle, expected a point",
            ),
            ("d = Slope(l, l)", "`Slope` does not take 2 argument(s)"),
            ("d = AffineRatio(A, B)", "`AffineRatio` does not take 2"),
            ("d = CrossRatio(A, B, P)", "`CrossRatio` does not take 3"),
            (
                "d = Angle(A, B, P, C)",
                "`Angle` does not take 4 argument(s)",
            ),
            ("d = Distance(A, B, P)", "`Distance` does not take 3"),
            ("d = Length(l)", "`l` is a line, expected a segment"),
            ("d = Radius(s)", "`s` is a segment, expected a circle"),
            ("d = Area(A)", "`A` is a point, expected a circle"),
            (
                "d = Slope(k)",
                "`k` is a circle, expected a line or segment",
            ),
            ("d = Area(A, B, k)", "`k` is a circle, expected a point"),
            ("d = Angle(A, l, B)", "`l` is a line, expected a point"),
            ("d = Distance(P, r)", "`r` is a number"),
            ("d = Length(Z)", "unknown object `Z`"),
        ] {
            let err = exec(&mut c, line).unwrap_err();
            assert!(err.contains(text), "{line}: {err}");
        }
    }

    #[test]
    fn degenerate_inputs() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = 2\n\
             s = Segment(A, B)\nl = Line(A, B)\nk = Circle(A, r)\n\
             sl = Slope(l)\nss = Slope(s)\nci = Circumference(k)\nar = Area(k)\n\
             tri = Area(A, B, C)\nang = Angle(B, A, C)\nlen = Length(s)\n",
        )
        .unwrap();
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        let r = c.id("r").unwrap();

        // Vertical line and segment have no slope.
        c.move_point(b, 0.0, 5.0).unwrap();
        assert_eq!(num(&c, "sl"), None);
        assert_eq!(num(&c, "ss"), None);
        close(num(&c, "len"), 5.0);

        // Coincident end points: zero length, no line, no angle at the vertex.
        c.move_point(b, 0.0, 0.0).unwrap();
        assert_eq!(num(&c, "sl"), None);
        assert_eq!(num(&c, "ss"), None);
        close(num(&c, "len"), 0.0);
        assert_eq!(num(&c, "ang"), None);
        close(num(&c, "tri"), 0.0);
        c.move_point(b, 4.0, 0.0).unwrap();
        c.move_point(a, 4.0, 0.0).unwrap();
        assert_eq!(num(&c, "ang"), None);
        c.move_point(a, 0.0, 0.0).unwrap();

        // A circle of radius 0 is a point: no circumference, area 0.
        c.set_number(r, 0.0).unwrap();
        assert_eq!(num(&c, "ci"), None);
        close(num(&c, "ar"), 0.0);
        c.set_number(r, 1.0).unwrap();
        close(num(&c, "ci"), 2.0 * PI);
    }

    #[test]
    fn a_radius_below_the_tolerance_is_a_single_point() {
        let c = load(
            "A = (0, 0)\nr = 0.000000005\nk = Circle(A, r)\n\
             d = Radius(k)\np = Circumference(k)\na = Area(k)\n",
        )
        .unwrap();
        close(num(&c, "d"), 0.0);
        assert_eq!(num(&c, "p"), None);
        close(num(&c, "a"), 0.0);
    }

    #[test]
    fn angle_is_counter_clockwise_in_zero_to_two_pi() {
        let mut c =
            load("A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nw = Angle(A, B, C)\nu = Angle(C, B, A)\n")
                .unwrap();
        close(num(&c, "w"), PI / 2.0);
        close(num(&c, "u"), 1.5 * PI);
        let ptc = c.id("C").unwrap();
        c.move_point(ptc, 1.0, 0.0).unwrap();
        close(num(&c, "w"), 0.0);
        c.move_point(ptc, -1.0, 0.0).unwrap();
        close(num(&c, "w"), PI);
        c.move_point(ptc, 1.0, -1.0).unwrap();
        close(num(&c, "w"), 1.75 * PI);
    }

    #[test]
    fn ratios_need_collinear_points() {
        let mut c = load(
            "A = (0, 0)\nB = (2, 0)\nC = (6, 0)\nD = (3, 0)\n\
             r = AffineRatio(A, B, C)\nq = CrossRatio(A, B, C, D)\n",
        )
        .unwrap();
        close(num(&c, "r"), 3.0);
        let (ptc, ptd) = (c.id("C").unwrap(), c.id("D").unwrap());
        // (B, C, D) / (A, C, D) = ((3-2)/(6-2)) / ((3-0)/(6-0)).
        close(num(&c, "q"), 0.25 / 0.5);
        c.move_point(ptc, 6.0, 1.0).unwrap();
        assert_eq!(num(&c, "r"), None);
        assert_eq!(num(&c, "q"), None);
        c.move_point(ptc, 2.0, 0.0).unwrap();
        close(num(&c, "r"), 1.0);
        assert_eq!(num(&c, "q"), None);
        c.move_point(ptc, 6.0, 0.0).unwrap();
        c.move_point(ptd, 0.0, 0.0).unwrap();
        assert_eq!(num(&c, "q"), None);
    }

    #[test]
    fn zero_tests_use_the_raw_scale_of_the_lines() {
        // sin(angle) is 1e-10: parallel for unit normals, but the raw cross
        // product is about 1e-2, so GeoGebra sees intersecting lines.
        let c = load(
            "A = (0, 0)\nB = (10000, 0)\nC = (0, 1)\nD = (10000, 1.000001)\n\
             l = Line(A, B)\nm = Line(C, D)\nd = Distance(l, m)\n\
             F = (0.001, 1000000)\nv = Line(A, F)\nt = Slope(v)\n",
        )
        .unwrap();
        close(num(&c, "d"), 0.0);
        close(num(&c, "t"), 1e9);
    }
}
