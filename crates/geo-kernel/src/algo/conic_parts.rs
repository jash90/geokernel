// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSingle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCenterConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCenterQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoVertexConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAxes.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAsymptote.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAsymptote.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAxis.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAxisLength.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoEccentricity.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoExcentricity.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDirectrix.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoParabolaParameter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLineND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolarLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolarPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDiameterLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDiameterVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIntersect.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/Commands.java
//   source/shared/common/src/main/java/org/geogebra/common/util/GgbMat.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Objects computed from a general conic ([`Kind::Conic`]): its
//! intersection points with a line, its tangents, polar lines and
//! diameters, and what is read off its record (centre, foci, vertices,
//! axes, directrices, lengths and eccentricities).
//!
//! They read the conic's [`ConicData`] record through
//! [`Construction::conic`]. An undefined parent makes every one of them
//! undefined. GeoGebra differs there: a conic that became undefined keeps
//! the type and the fields of its last classification, and `Vertex`, the
//! axes, the numbers and the diameters are still read from them (see
//! `DIFFERENCES.md`).

use super::conic_nd::{ConicData, ConicType};
use super::intersect::{helper_line_conic_points, on_general_conic_by_construction, Pt};
use super::{line_value, Group, Parents, CONIC_OBJECTS, LINE, POINT};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

const LINE_LIKE_OR_RAY: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray];
const DIRECTION: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray, Kind::Vector];

const CONIC: &[Kind] = &[Kind::Conic];
const CIRCLE_OR_CONIC: &[Kind] = &[Kind::Circle, Kind::Conic];

/// Objects computed from a general conic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicPartDef {
    /// `Intersect(g, k, n)` and `Intersect(k, g, n)`: `which` is 0 or 1,
    /// GeoGebra's intersection 1 or 2 of a line and a conic.
    IntersectLine { line: Id, conic: Id, which: u8 },
    /// `Center(k)` and `Midpoint(k)`.
    Center(Id),
    /// Output `which` (0 or 1) of `Focus(k)`.
    Focus { conic: Id, which: u8 },
    /// Output `which` (0 to 3) of `Vertex(k)`.
    Vertex { conic: Id, which: u8 },
    /// Output `which` of `Axes(k)`; `MajorAxis(k)` (`FirstAxis`) is axis 0
    /// and `MinorAxis(k)` (`SecondAxis`) axis 1.
    Axis { conic: Id, which: u8 },
    /// Output `which` (0 or 1) of `Asymptote(k)`.
    Asymptote { conic: Id, which: u8 },
    /// Output `which` (0 or 1) of `Directrix(k)`.
    Directrix { conic: Id, which: u8 },
    /// `FirstAxisLength(k)` (`SemiMajorAxisLength`) for 0,
    /// `SecondAxisLength(k)` (`SemiMinorAxisLength`) for 1.
    AxisLength { conic: Id, which: u8 },
    /// `Eccentricity(k)`.
    Eccentricity(Id),
    /// `LinearEccentricity(k)` (`Excentricity`).
    LinearEccentricity(Id),
    /// `Parameter(k)` of a parabola.
    Parameter(Id),
    /// Output `which` (0 or 1) of `Tangent(P, k)`.
    TangentPoint { point: Id, conic: Id, which: u8 },
    /// Output `which` (0 or 1) of `Tangent(g, k)`: the tangents parallel
    /// to a line, segment or ray.
    TangentLine { line: Id, conic: Id, which: u8 },
    /// `Polar(P, k)`.
    Polar { point: Id, conic: Id },
    /// `Polar(g, k)`: the pole of a line with respect to a conic.
    Pole { line: Id, conic: Id },
    /// `Diameter(g, k)` and `Diameter(v, k)` (`ConjugateDiameter`): the
    /// diameter conjugate to the direction of a line, segment, ray or vector.
    Diameter { direction: Id, conic: Id },
}

impl ConicPartDef {
    fn conic(&self) -> Id {
        match *self {
            ConicPartDef::IntersectLine { conic, .. }
            | ConicPartDef::Focus { conic, .. }
            | ConicPartDef::Vertex { conic, .. }
            | ConicPartDef::Axis { conic, .. }
            | ConicPartDef::Asymptote { conic, .. }
            | ConicPartDef::Directrix { conic, .. }
            | ConicPartDef::AxisLength { conic, .. }
            | ConicPartDef::TangentPoint { conic, .. }
            | ConicPartDef::TangentLine { conic, .. }
            | ConicPartDef::Polar { conic, .. }
            | ConicPartDef::Pole { conic, .. }
            | ConicPartDef::Diameter { conic, .. } => conic,
            ConicPartDef::Center(conic)
            | ConicPartDef::Eccentricity(conic)
            | ConicPartDef::LinearEccentricity(conic)
            | ConicPartDef::Parameter(conic) => conic,
        }
    }

    /// The number of outputs of the command, and the largest valid `which`.
    fn outputs(&self) -> u8 {
        match self {
            ConicPartDef::IntersectLine { .. }
            | ConicPartDef::Focus { .. }
            | ConicPartDef::Axis { .. }
            | ConicPartDef::Asymptote { .. }
            | ConicPartDef::Directrix { .. }
            | ConicPartDef::AxisLength { .. }
            | ConicPartDef::TangentPoint { .. }
            | ConicPartDef::TangentLine { .. } => 2,
            ConicPartDef::Vertex { .. } => 4,
            _ => 1,
        }
    }

    fn which(&self) -> u8 {
        match *self {
            ConicPartDef::IntersectLine { which, .. }
            | ConicPartDef::Focus { which, .. }
            | ConicPartDef::Vertex { which, .. }
            | ConicPartDef::Axis { which, .. }
            | ConicPartDef::Asymptote { which, .. }
            | ConicPartDef::Directrix { which, .. }
            | ConicPartDef::AxisLength { which, .. }
            | ConicPartDef::TangentPoint { which, .. }
            | ConicPartDef::TangentLine { which, .. } => which,
            _ => 0,
        }
    }
}

impl Group for ConicPartDef {
    fn kind(&self) -> Kind {
        match self {
            ConicPartDef::IntersectLine { .. }
            | ConicPartDef::Center(_)
            | ConicPartDef::Focus { .. }
            | ConicPartDef::Pole { .. }
            | ConicPartDef::Vertex { .. } => Kind::Point,
            ConicPartDef::Axis { .. }
            | ConicPartDef::Asymptote { .. }
            | ConicPartDef::Directrix { .. }
            | ConicPartDef::TangentPoint { .. }
            | ConicPartDef::TangentLine { .. }
            | ConicPartDef::Polar { .. }
            | ConicPartDef::Diameter { .. } => Kind::Line,
            ConicPartDef::AxisLength { .. }
            | ConicPartDef::Eccentricity(_)
            | ConicPartDef::LinearEccentricity(_)
            | ConicPartDef::Parameter(_) => Kind::Number,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            ConicPartDef::IntersectLine { line, conic, .. }
            | ConicPartDef::TangentLine { line, conic, .. }
            | ConicPartDef::Pole { line, conic } => Parents::of(&[line, conic]),
            ConicPartDef::TangentPoint { point, conic, .. }
            | ConicPartDef::Polar { point, conic } => Parents::of(&[point, conic]),
            ConicPartDef::Diameter { direction, conic } => Parents::of(&[direction, conic]),
            other => Parents::of(&[other.conic()]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            ConicPartDef::IntersectLine { line, .. } => c.expect(line, LINE)?,
            ConicPartDef::TangentLine { line, .. } | ConicPartDef::Pole { line, .. } => {
                c.expect(line, LINE_LIKE_OR_RAY)?
            }
            ConicPartDef::TangentPoint { point, .. } | ConicPartDef::Polar { point, .. } => {
                c.expect(point, POINT)?
            }
            ConicPartDef::Diameter { direction, .. } => c.expect(direction, DIRECTION)?,
            _ => {}
        }
        let conics = match self {
            Self::Asymptote { .. } => CONIC_OBJECTS,
            Self::Pole { .. } => CIRCLE_OR_CONIC,
            _ => CONIC,
        };
        c.expect(self.conic(), conics)?;
        if self.which() < self.outputs() {
            Ok(())
        } else {
            Err(Error::BadIndex(self.which()))
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        // An undefined parent makes the object undefined, also where
        // GeoGebra would still read the stale record of an undefined conic
        // or take a point on both objects by construction.
        if self
            .parents()
            .as_slice()
            .iter()
            .any(|&p| !c.value(p).is_defined())
        {
            return None;
        }
        if let ConicPartDef::IntersectLine { line, conic, which } = *self {
            return super::intersect::line_general_conic(c, line, conic, which);
        }
        if let ConicPartDef::Asymptote { conic, which } = *self {
            return asymptote(&super::conic_object(c, conic)?, which);
        }
        let d = c.conic(self.conic())?;
        match *self {
            ConicPartDef::IntersectLine { .. } | ConicPartDef::Asymptote { .. } => None,
            ConicPartDef::TangentPoint {
                point,
                conic,
                which,
            } => {
                let (g, _) = tangent_from_point(c, point, conic, &d, which)?;
                line_value(g[0], g[1], g[2])
            }
            ConicPartDef::TangentLine { line, which, .. } => {
                let (g, _) = tangent_parallel(c, line, &d, which)?;
                line_value(g[0], g[1], g[2])
            }
            ConicPartDef::Polar { point, .. } => {
                let g = polar(&d, c.point(point)?);
                line_value(g[0], g[1], g[2])
            }
            ConicPartDef::Pole { line, .. } => pole(&d, c.raw_carrier(line)?),
            ConicPartDef::Diameter { direction, .. } => {
                let (vx, vy) = match c.value(direction) {
                    Value::Vector { x, y } => (x, y),
                    _ => {
                        // `GeoLine.getDirection`
                        let (a, b, _) = c.raw_carrier(direction)?;
                        (b, -a)
                    }
                };
                let g = diameter(&d, vx, vy);
                line_value(g[0], g[1], g[2])
            }
            ConicPartDef::Center(_) => center(&d),
            ConicPartDef::Focus { which, .. } => focus(&d, which),
            ConicPartDef::Vertex { which, .. } => vertex(&d, which),
            ConicPartDef::Axis { which, .. } => axis(&d, which),
            ConicPartDef::Directrix { which, .. } => directrix(&d, which),
            ConicPartDef::AxisLength { which, .. } => match d.ty {
                ConicType::Circle | ConicType::Hyperbola | ConicType::Ellipse => {
                    number(d.half_axes[usize::from(which)])
                }
                _ => None,
            },
            ConicPartDef::Eccentricity(_) => match d.ty {
                ConicType::Circle => number(0.0),
                ConicType::Parabola => number(1.0),
                ConicType::Hyperbola | ConicType::Ellipse => number(d.eccentricity),
                _ => None,
            },
            ConicPartDef::LinearEccentricity(_) => match d.ty {
                ConicType::Circle => number(0.0),
                ConicType::Hyperbola | ConicType::Ellipse | ConicType::Parabola => {
                    number(d.linear_eccentricity)
                }
                _ => None,
            },
            ConicPartDef::Parameter(_) => match d.ty {
                ConicType::Parabola => number(d.p),
                _ => None,
            },
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        if let ("Intersect", 2 | 3) = (func, args.len()) {
            let first = c.id(args[0])?;
            let second = c.id(args[1])?;
            let (line, conic) = match (c.kind(first), c.kind(second)) {
                (Kind::Line, Kind::Conic) => (first, second),
                (Kind::Conic, Kind::Line) => (second, first),
                _ => return None,
            };
            return Some(
                parse_index(args.get(2).copied()).map(|which| ConicPartDef::IntersectLine {
                    line,
                    conic,
                    which,
                }),
            );
        }
        if let ("Polar" | "Diameter" | "ConjugateDiameter", [first, second]) = (func, args) {
            let first = c.id(first)?;
            let conic = c.id(second)?;
            if func == "Polar"
                && CIRCLE_OR_CONIC.contains(&c.kind(conic))
                && LINE_LIKE_OR_RAY.contains(&c.kind(first))
            {
                return Some(Ok(Self::Pole { line: first, conic }));
            }
            if c.kind(conic) != Kind::Conic {
                return None;
            }
            return Some(match (func, c.kind(first)) {
                ("Polar", Kind::Point) => Ok(ConicPartDef::Polar {
                    point: first,
                    conic,
                }),
                (_, Kind::Line | Kind::Segment | Kind::Ray | Kind::Vector) if func != "Polar" => {
                    Ok(ConicPartDef::Diameter {
                        direction: first,
                        conic,
                    })
                }
                (_, k) => Err(format!("`{func}` does not take a {k} and a conic")),
            });
        }
        let [arg] = args else { return None };
        let conic = c.id(arg).filter(|&k| c.kind(k) == Kind::Conic)?;
        let def = match func {
            "Center" | "Midpoint" => ConicPartDef::Center(conic),
            "MajorAxis" | "FirstAxis" => ConicPartDef::Axis { conic, which: 0 },
            "MinorAxis" | "SecondAxis" => ConicPartDef::Axis { conic, which: 1 },
            "FirstAxisLength" | "SemiMajorAxisLength" => {
                ConicPartDef::AxisLength { conic, which: 0 }
            }
            "SecondAxisLength" | "SemiMinorAxisLength" => {
                ConicPartDef::AxisLength { conic, which: 1 }
            }
            "Eccentricity" => ConicPartDef::Eccentricity(conic),
            "LinearEccentricity" | "Excentricity" => ConicPartDef::LinearEccentricity(conic),
            "Parameter" => ConicPartDef::Parameter(conic),
            _ => return None,
        };
        Some(Ok(def))
    }

    fn parse_multi(
        c: &Construction,
        func: &str,
        args: &[&str],
    ) -> Option<Result<Vec<Self>, String>> {
        if let ("Tangent", [first, second]) = (func, args) {
            let (first, second) = (c.id(first)?, c.id(second)?);
            return match (c.kind(first), c.kind(second)) {
                (Kind::Point, Kind::Conic) => Some(Ok(tangents_from_point(c, first, second))),
                (Kind::Conic, Kind::Point) => Some(Ok(tangents_from_point(c, second, first))),
                (Kind::Line | Kind::Segment | Kind::Ray, Kind::Conic) => Some(Ok((0..2)
                    .map(|which| ConicPartDef::TangentLine {
                        line: first,
                        conic: second,
                        which,
                    })
                    .collect())),
                (_, Kind::Conic) | (Kind::Conic, _) => Some(Err(
                    "this form of `Tangent` of a conic is not supported yet".into(),
                )),
                _ => None,
            };
        }
        let [arg] = args else { return None };
        let kinds = if func == "Asymptote" {
            CONIC_OBJECTS
        } else {
            CONIC
        };
        let conic = c.id(arg).filter(|&k| kinds.contains(&c.kind(k)))?;
        let (n, make): (u8, fn(Id, u8) -> Self) = match func {
            "Focus" => (2, |conic, which| ConicPartDef::Focus { conic, which }),
            "Vertex" => (4, |conic, which| ConicPartDef::Vertex { conic, which }),
            "Axes" => (2, |conic, which| ConicPartDef::Axis { conic, which }),
            "Directrix" => (2, |conic, which| ConicPartDef::Directrix { conic, which }),
            "Asymptote" => (2, |conic, which| ConicPartDef::Asymptote { conic, which }),
            _ => return None,
        };
        Some(Ok((0..n).map(|which| make(conic, which)).collect()))
    }
}

/// `AlgoAsymptote.compute`: the two lines through the centre in the half-axis directions.
fn asymptote(d: &ConicData, which: u8) -> Option<Value> {
    if d.ty != ConicType::Hyperbola {
        return None;
    }
    let [e0, e1] = d.eigenvec;
    let [ax, bx] = d.half_axes;
    let (vx, vy) = (ax * e0[0], ax * e0[1]);
    let (wx, wy) = (bx * e1[0], bx * e1[1]);
    let (a, b) = if which == 0 {
        (-(wy + vy), wx + vx)
    } else {
        (-(wy - vy), wx - vx)
    };
    line_value(a, b, -(a * d.midpoint[0] + b * d.midpoint[1]))
}

/// The tangents from `point` to `conic`. As in the constructor of
/// `AlgoTangentPointND`, when both tangents are the same line at creation
/// (the point is on the conic) the second one is made undefined; the kernel
/// then creates only the first tangent.
fn tangents_from_point(c: &Construction, point: Id, conic: Id) -> Vec<ConicPartDef> {
    let defs: Vec<ConicPartDef> = (0..2)
        .map(|which| ConicPartDef::TangentPoint {
            point,
            conic,
            which,
        })
        .collect();
    match (defs[0].compute(c), defs[1].compute(c)) {
        (Some(t0), Some(t1)) if super::lines::lin_dep(t0, t1) => defs[..1].to_vec(),
        _ => defs,
    }
}

/// `GeoConicND.polarLine` for the point `(px, py, 1)`; NaN when the conic
/// is undefined.
fn polar(d: &ConicData, (px, py): (f64, f64)) -> [f64; 3] {
    if !d.defined {
        return [f64::NAN; 3];
    }
    let m = &d.matrix;
    [
        m[0] * px + m[3] * py + m[4],
        m[3] * px + m[1] * py + m[5],
        m[4] * px + m[5] * py + m[2],
    ]
}

/// `GeoConicND.polarPoint`: solve the conic matrix against the line's
/// coefficients. As in `GgbMat.inverseImmediate`, use partial pivoting
/// and treat a pivot below the standard precision as singular.
fn pole(d: &ConicData, (a, b, c): (f64, f64, f64)) -> Option<Value> {
    let [xx, yy, constant, xy, x, y] = d.matrix;
    let mut rows = [[xx, xy, x, a], [xy, yy, y, b], [x, y, constant, c]];
    for col in 0..3 {
        let mut pivot = col;
        for row in col + 1..3 {
            if rows[row][col].abs() > rows[pivot][col].abs() {
                pivot = row;
            }
        }
        if rows[pivot][col].abs() < STANDARD_PRECISION {
            return None;
        }
        rows.swap(col, pivot);
        let leading = rows[col];
        for row in &mut rows[col + 1..] {
            let factor = row[col] / leading[col];
            for j in col + 1..4 {
                row[j] -= factor * leading[j];
            }
        }
    }
    let z = rows[2][3] / rows[2][2];
    let y = (rows[1][3] - rows[1][2] * z) / rows[1][1];
    let x = (rows[0][3] - rows[0][1] * y - rows[0][2] * z) / rows[0][0];
    if super::conic::is_epsilon(z, x, y) {
        return None;
    }
    let (x, y) = (x / z, y / z);
    (x.is_finite() && y.is_finite()).then_some(Value::Point { x, y })
}

/// `GeoConicND.diameterLine(vx, vy)`; it does not check whether the conic
/// is defined.
fn diameter(d: &ConicData, vx: f64, vy: f64) -> [f64; 3] {
    let m = &d.matrix;
    [
        m[0] * vx + m[3] * vy,
        m[3] * vx + m[1] * vy,
        m[4] * vx + m[5] * vy,
    ]
}

/// `AlgoTangentPointND.compute` with `AlgoTangentPoint`: tangent `which`
/// from `point` and the point where it touches the conic. `None` for a
/// degenerate conic.
fn tangent_from_point(
    c: &Construction,
    point: Id,
    conic: Id,
    d: &ConicData,
    which: u8,
) -> Option<([f64; 3], Pt)> {
    if d.ty.is_degenerate() {
        return None;
    }
    let p = c.point(point)?;
    let g = polar(d, p);
    // isIntersectionPointIncident: on the conic, or on it by construction
    if d.on_full_conic(p, STANDARD_PRECISION) || on_general_conic_by_construction(c, point, conic) {
        // the polar of a point on the conic is its tangent
        return Some((g, Some(p)));
    }
    let t = helper_line_conic_points(g, d)[usize::from(which)];
    // `GeoVec3D.lineThroughPoints(P, T)`
    let line = match t {
        Some((tx, ty)) => [p.1 - ty, tx - p.0, p.0 * ty - p.1 * tx],
        None => [f64::NAN; 3],
    };
    Some((line, t))
}

/// `AlgoTangentLineND.compute` with `AlgoTangentLine`: tangent `which`
/// parallel to `line` and the point where it touches the conic. A parabola
/// has one such tangent. `None` for a degenerate conic.
fn tangent_parallel(
    c: &Construction,
    line: Id,
    d: &ConicData,
    which: u8,
) -> Option<([f64; 3], Pt)> {
    if d.ty.is_degenerate() {
        return None;
    }
    let (gx, gy, _) = c.raw_carrier(line)?;
    // the diameter conjugate to the direction (gy, -gx)
    let points = helper_line_conic_points(diameter(d, gy, -gx), d);
    let z = |t: Pt| t.map_or(f64::NAN, |(tx, ty)| -(tx * gx + ty * gy));
    let line = if d.ty == ConicType::Parabola {
        match which {
            0 if z(points[1]).is_nan() => [gx, gy, z(points[0])],
            0 => [gx, gy, z(points[1])],
            _ => [f64::NAN; 3],
        }
    } else {
        [gx, gy, z(points[usize::from(which)])]
    };
    Some((line, points[usize::from(which)]))
}

/// The point where the tangent `d` touches `conic`, as
/// `TangentAlgo.getTangentPoint` gives it to `AlgoIntersectLineConic`:
/// `None` when `d` is not a tangent of `conic`, `Some(None)` when the point
/// is undefined.
pub(crate) fn tangent_point_on(c: &Construction, d: ConicPartDef, conic: Id) -> Option<Pt> {
    match d {
        ConicPartDef::TangentPoint {
            point,
            conic: k,
            which,
        } if k == conic => Some(
            c.conic(k)
                .and_then(|data| tangent_from_point(c, point, k, &data, which))
                .and_then(|(_, t)| t),
        ),
        ConicPartDef::TangentLine {
            line,
            conic: k,
            which,
        } if k == conic => Some(
            c.conic(k)
                .and_then(|data| tangent_parallel(c, line, &data, which))
                .and_then(|(_, t)| t),
        ),
        _ => None,
    }
}

fn number(v: f64) -> Option<Value> {
    Some(Value::Number(v))
}

fn point(x: f64, y: f64) -> Option<Value> {
    Some(Value::Point { x, y })
}

/// `AlgoCenterConic`: the midpoint of a central conic.
fn center(d: &ConicData) -> Option<Value> {
    if !d.defined {
        return None;
    }
    match d.ty {
        ConicType::Circle
        | ConicType::Ellipse
        | ConicType::Hyperbola
        | ConicType::SinglePoint
        | ConicType::IntersectingLines => point(d.midpoint[0], d.midpoint[1]),
        _ => None,
    }
}

/// `AlgoFocus`.
fn focus(d: &ConicData, which: u8) -> Option<Value> {
    if !d.defined {
        return None;
    }
    let [bx, by] = d.midpoint;
    let e0 = d.eigenvec[0];
    match d.ty {
        ConicType::Circle => point(bx, by),
        ConicType::Ellipse | ConicType::Hyperbola => {
            let temp1 = d.linear_eccentricity * e0[0];
            let temp2 = d.linear_eccentricity * e0[1];
            if which == 0 {
                point(bx - temp1, by - temp2)
            } else {
                point(bx + temp1, by + temp2)
            }
        }
        ConicType::Parabola if which == 0 => {
            let temp1 = d.p / 2.0;
            point(bx + temp1 * e0[0], by + temp1 * e0[1])
        }
        _ => None,
    }
}

/// `AlgoVertexConic`; it does not check whether the conic is defined.
fn vertex(d: &ConicData, which: u8) -> Option<Value> {
    let [bx, by] = d.midpoint;
    let along = |axis: usize, sign: f64| {
        let e = d.eigenvec[axis];
        let temp1 = d.half_axes[axis] * e[0];
        let temp2 = d.half_axes[axis] * e[1];
        if sign < 0.0 {
            point(bx - temp1, by - temp2)
        } else {
            point(bx + temp1, by + temp2)
        }
    };
    match (d.ty, which) {
        (ConicType::Circle | ConicType::Ellipse, _) | (ConicType::Hyperbola, 0 | 1) => along(
            usize::from(which / 2),
            if which.is_multiple_of(2) { -1.0 } else { 1.0 },
        ),
        (ConicType::Parabola | ConicType::ParallelLines | ConicType::DoubleLine, 0) => {
            point(bx, by)
        }
        _ => None,
    }
}

/// `AlgoAxes` and `AlgoAxis`: the line through the midpoint along an
/// eigenvector; it does not check whether the conic is defined.
fn axis(d: &ConicData, which: u8) -> Option<Value> {
    let e = d.eigenvec[usize::from(which)];
    let [bx, by] = d.midpoint;
    let (x, y) = (-e[1], e[0]);
    line_value(x, y, -(x * bx + y * by))
}

/// `AlgoDirectrix`: one directrix of a parabola, two of an ellipse or a
/// hyperbola.
fn directrix(d: &ConicData, which: u8) -> Option<Value> {
    if !d.defined {
        return None;
    }
    let [bx, by] = d.midpoint;
    let [e0, e1] = d.eigenvec;
    let (x, y) = (-e1[1], e1[0]);
    match d.ty {
        ConicType::Parabola if which == 0 => {
            let px = bx - d.p / 2.0 * e0[0];
            let py = by - d.p / 2.0 * e0[1];
            line_value(x, y, -(x * px + y * py))
        }
        ConicType::Ellipse | ConicType::Hyperbola => {
            let e = d.eccentricity;
            let a = d.half_axes[0];
            let (px, py) = if which == 0 {
                (bx + e0[0] * a / e, by + e0[1] * a / e)
            } else {
                (bx - e0[0] * a / e, by - e0[1] * a / e)
            };
            line_value(x, y, -(x * px + y * py))
        }
        _ => None,
    }
}

/// The optional intersection number `1` or `2` as an index 0 or 1.
fn parse_index(arg: Option<&str>) -> Result<u8, String> {
    match arg {
        None | Some("1") => Ok(0),
        Some("2") => Ok(1),
        Some(other) => Err(format!("intersection number must be 1 or 2, got `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use crate::kernel::{Construction, Value};
    use crate::script::load;

    fn xy(c: &Construction, label: &str) -> Option<(f64, f64)> {
        match c.value(c.id(label).unwrap()) {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }
    }

    fn close(p: Option<(f64, f64)>, x: f64, y: f64) -> bool {
        p.is_some_and(|(px, py)| (px - x).abs() < 1e-9 && (py - y).abs() < 1e-9)
    }

    const ELLIPSE: &str = "F = (-3, 0)\nG = (3, 0)\nP = (0, 4)\ne = Ellipse(F, G, P)\n";

    #[test]
    fn a_line_meets_an_ellipse_in_two_points() {
        let c = load(&format!(
            "{ELLIPSE}A = (0, -10)\nB = (0, 10)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(e, g, 2)\n"
        ))
        .unwrap();
        let (s, t) = (xy(&c, "S"), xy(&c, "T"));
        assert!(close(s, 0.0, 4.0) || close(s, 0.0, -4.0));
        assert!(close(t, 0.0, 4.0) || close(t, 0.0, -4.0));
        assert_ne!(s, t);
    }

    #[test]
    fn a_tangent_point_is_only_intersection_one_and_a_passing_line_none() {
        let mut c = load(&format!(
            "{ELLIPSE}A = (-1, 4)\nB = (1, 4)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n"
        ))
        .unwrap();
        assert!(close(xy(&c, "S"), 0.0, 4.0));
        assert_eq!(xy(&c, "T"), None);
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        c.move_point(a, -1.0, 5.0).unwrap();
        c.move_point(b, 1.0, 5.0).unwrap();
        assert_eq!(xy(&c, "S"), None);
        assert_eq!(xy(&c, "T"), None);
    }

    #[test]
    fn a_line_parallel_to_the_axis_meets_a_parabola_once_as_intersection_one() {
        // y = x^2 / 4: focus (0, 1), directrix y = -1
        let c = load(
            "F = (0, 1)\nA = (0, -1)\nB = (1, -1)\nd = Line(A, B)\np = Parabola(F, d)\nC = (2, 0)\nD = (2, 5)\ng = Line(C, D)\nS = Intersect(g, p, 1)\nT = Intersect(g, p, 2)\nE = (2, 5)\nH = (2, 0)\nh = Line(E, H)\nU = Intersect(h, p, 1)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 2.0, 1.0));
        assert_eq!(xy(&c, "T"), None);
        assert!(close(xy(&c, "U"), 2.0, 1.0));
    }

    #[test]
    fn a_secant_of_a_double_line_meets_it_once() {
        let c = load(
            "F = (-3, 0)\nG = (3, 0)\na = 3\ne = Ellipse(F, G, a)\nA = (1, -2)\nB = (1, 2)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 1.0, 0.0));
        assert_eq!(xy(&c, "T"), None);
    }

    #[test]
    fn a_point_on_line_and_conic_by_construction_comes_first() {
        for (first, second) in [("A", "X"), ("X", "A")] {
            let mut c = load(&format!(
                "A = (1, 0)\nB = (0, 1)\nC = (-1, 0)\nD = (0, -1)\nE = (0.6, 0.8)\nk = Conic(A, B, C, D, E)\nX = (-2, 1)\ng = Line({first}, {second})\nS = Intersect(g, k, 1)\nT = Intersect(g, k, 2)\n"
            ))
            .unwrap();
            assert!(close(xy(&c, "S"), 1.0, 0.0), "{first} {second}");
            assert!(xy(&c, "T").is_some());
            assert!(!close(xy(&c, "T"), 1.0, 0.0));
            // the point stays first when the line turns
            let x = c.id("X").unwrap();
            c.move_point(x, 3.0, 2.0).unwrap();
            assert!(close(xy(&c, "S"), 1.0, 0.0), "{first} {second}");
        }
    }

    #[test]
    fn rebuilt_intersections_have_the_same_values() {
        let script = format!(
            "{ELLIPSE}A = (-6, -1)\nB = (5, 2)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n"
        );
        let mut c = load(&script).unwrap();
        let a = c.id("A").unwrap();
        c.move_point(a, -4.0, 3.0).unwrap();
        c.move_point(a, -6.0, -1.0).unwrap();
        let fresh = load(&script).unwrap();
        for label in ["S", "T"] {
            assert_eq!(xy(&c, label), xy(&fresh, label), "{label}");
        }
    }

    #[test]
    fn an_undefined_conic_makes_the_intersections_undefined() {
        let mut c = load(
            "F = (-3, 0)\nG = (3, 0)\na = 5\ne = Ellipse(F, G, a)\nf = 2\nk = Dilate(e, f)\nA = (0, -20)\nB = (0, 20)\ng = Line(A, B)\nS = Intersect(g, k, 1)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 0.0, 8.0) || close(xy(&c, "S"), 0.0, -8.0));
        let f = c.id("f").unwrap();
        c.set_number(f, 0.0).unwrap();
        assert_eq!(xy(&c, "S"), None);
        c.set_number(f, 2.0).unwrap();
        assert!(xy(&c, "S").is_some());
    }

    fn num(c: &Construction, label: &str) -> Option<f64> {
        match c.value(c.id(label).unwrap()) {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }

    #[test]
    fn an_ellipse_has_its_centre_foci_vertices_and_numbers() {
        let c = load(&format!(
            "{ELLIPSE}M = Center(e)\nW = Focus(e)\nX = Vertex(e)\nla = FirstAxisLength(e)\nlb = SecondAxisLength(e)\nec = Eccentricity(e)\nli = LinearEccentricity(e)\npa = Parameter(e)\ndir = Directrix(e)\n"
        ))
        .unwrap();
        assert!(close(xy(&c, "M"), 0.0, 0.0));
        let foci = [xy(&c, "W_{1}"), xy(&c, "W_{2}")];
        assert!(
            foci.iter().any(|&p| close(p, -3.0, 0.0)) && foci.iter().any(|&p| close(p, 3.0, 0.0))
        );
        for label in ["X_{1}", "X_{2}", "X_{3}", "X_{4}"] {
            assert!(xy(&c, label).is_some(), "{label}");
        }
        assert_eq!(num(&c, "la"), Some(5.0));
        assert_eq!(num(&c, "lb"), Some(4.0));
        assert!((num(&c, "ec").unwrap() - 0.6).abs() < 1e-12);
        assert!((num(&c, "li").unwrap() - 3.0).abs() < 1e-12);
        assert_eq!(num(&c, "pa"), None);
        // the directrices x = +-25/3
        for label in ["dir", "dir_{1}"] {
            let Value::Line { a, b, c: cc } = c.value(c.id(label).unwrap()) else {
                panic!("{label}")
            };
            assert!(
                b.abs() < 1e-12 && ((cc / a).abs() - 25.0 / 3.0).abs() < 1e-9,
                "{label}"
            );
        }
    }

    #[test]
    fn a_hyperbola_labels_its_two_vertices_and_a_parabola_one_focus() {
        let c = load(
            "F = (-3, 0)\nG = (3, 0)\nP = (2, 1)\nh = Hyperbola(F, G, P)\nX = Vertex(h)\nQ = (0, 1)\nA = (0, -1)\nB = (1, -1)\nd = Line(A, B)\np = Parabola(Q, d)\nW = Focus(p)\nV = Vertex(p)\nr = Parameter(p)\n",
        )
        .unwrap();
        assert!(xy(&c, "X_{1}").is_some() && xy(&c, "X_{2}").is_some());
        assert!(c.id("X_{3}").is_none());
        assert!(close(xy(&c, "W"), 0.0, 1.0));
        assert!(close(xy(&c, "V"), 0.0, 0.0));
        assert_eq!(num(&c, "r"), Some(2.0));
    }

    fn line(c: &Construction, label: &str) -> Option<(f64, f64, f64)> {
        match c.value(c.id(label).unwrap()) {
            Value::Line { a, b, c } => Some((a, b, c)),
            _ => None,
        }
    }

    #[test]
    fn tangents_from_a_point_touch_the_ellipse_where_the_intersection_says() {
        let c = load(&format!(
            "{ELLIPSE}Q = (7, 5)\nt = Tangent(Q, e)\nS = Intersect(t_{{1}}, e)\nT = Intersect(t_{{1}}, e, 2)\n"
        ))
        .unwrap();
        let s = xy(&c, "S").unwrap();
        assert!((s.0 * s.0 / 25.0 + s.1 * s.1 / 16.0 - 1.0).abs() < 1e-9);
        let (a, b, cc) = line(&c, "t_{1}").unwrap();
        assert!((a * s.0 + b * s.1 + cc).abs() < 1e-9 && (a * 7.0 + b * 5.0 + cc).abs() < 1e-9);
        assert_eq!(xy(&c, "T"), None);
        assert!(line(&c, "t_{2}").is_some());
    }

    #[test]
    fn a_point_on_the_conic_has_one_tangent_its_polar() {
        let c = load(&format!("{ELLIPSE}t = Tangent(P, e)\np = Polar(P, e)\n")).unwrap();
        assert!(c.id("t_{1}").is_none());
        let (a, b, cc) = line(&c, "t").unwrap();
        // y = 4
        assert!(a.abs() < 1e-12 && (cc / b + 4.0).abs() < 1e-12);
        assert_eq!(line(&c, "t"), line(&c, "p"));
    }

    #[test]
    fn a_parabola_has_one_tangent_parallel_to_a_line_and_diameters_along_its_axis() {
        let c = load(
            "F = (0, 1)\nA = (0, -1)\nB = (1, -1)\nd = Line(A, B)\np = Parabola(F, d)\nU = (0, 0)\nV = (1, 1)\ng = Line(U, V)\nt = Tangent(g, p)\nm = Diameter(g, p)\n",
        )
        .unwrap();
        // y = x^2 / 4 touches a line of slope 1 at (2, 1)
        let (a, b, cc) = line(&c, "t").unwrap();
        assert!((a * 2.0 + b + cc).abs() < 1e-9 && (a + b).abs() < 1e-12);
        assert!(c.id("t_{2}").is_none());
        // the diameter is the vertical line x = 2
        let (a, b, cc) = line(&c, "m").unwrap();
        assert!(b.abs() < 1e-12 && (cc / a + 2.0).abs() < 1e-9);
    }
}
