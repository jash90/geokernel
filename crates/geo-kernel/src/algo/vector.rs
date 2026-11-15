// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/BasicCommandProcessorFactory.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/Commands.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdTranslate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOrthogonalLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdUnitVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOrthogonalVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdUnitOrthogonalVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLength.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoVectorPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTranslate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTranslateVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPointsRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRayPointVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnitVector2D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnitVectorLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnitVectorVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoVectorLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoVectorVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnitOrthoVectorLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnitOrthoVectorVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLengthVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVector.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Vectors and rays: `Vector`, free vectors, `Ray`, `Translate`, lines
//! through a point with a direction vector, `Direction`, `UnitVector`,
//! `OrthogonalVector`, `UnitOrthogonalVector` and `Length` of a vector.
//!
//! GeoGebra makes a line or a ray from raw coefficients `(x, y, z)` of
//! `x*X + y*Y + z = 0` whose scale the algorithm decides, and calls the
//! line undefined when both `x` and `y` are zero within
//! `STANDARD_PRECISION`. [`stored_line`] turns such raw coefficients into
//! what `Value::Line` and `Value::Ray` store.
//!
//! `Direction(l)` is `(y, -x)` and `OrthogonalVector(l)` is `(x, y)` of the
//! raw coefficients, so their length depends on the raw scale, and GeoGebra
//! still computes them when the line is undefined (a zero vector).
//! [`raw_normal`] rebuilds the raw `(x, y)` from the definition for the lines
//! whose scale is known; other lines are used with their stored coefficients.

use super::{Group, Parents, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

const VECTOR: &[Kind] = &[Kind::Vector];
/// Objects with a direction: GeoGebra's `GeoLine` and its subclasses.
const DIRECTED: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray];
const VECTOR_OR_DIRECTED: &[Kind] = &[Kind::Vector, Kind::Line, Kind::Segment, Kind::Ray];
/// Kinds of object `Translate` moves, restricted to those supported here.
const TRANSLATABLE: &[Kind] = &[
    Kind::Point,
    Kind::Line,
    Kind::Segment,
    Kind::Circle,
    Kind::Ray,
    Kind::Vector,
];

fn only(kind: Kind) -> &'static [Kind] {
    match kind {
        Kind::Point => &[Kind::Point],
        Kind::Line => &[Kind::Line],
        Kind::Segment => &[Kind::Segment],
        Kind::Circle => &[Kind::Circle],
        Kind::Number => &[Kind::Number],
        Kind::Vector => &[Kind::Vector],
        Kind::Ray => &[Kind::Ray],
    }
}

/// Vectors, rays and the objects made from them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VectorDef {
    /// `v = (x, y)` with a lower-case label: a vector the user can change.
    Free { x: f64, y: f64 },
    /// `Vector(A, B)`: from `A` to `B`.
    Between(Id, Id),
    /// `Vector(A)`: the position vector of `A`.
    Position(Id),
    /// `Ray(A, B)`: from `A` through `B`.
    RayPoints(Id, Id),
    /// `Ray(A, v)`: from `A` in the direction of the vector `v`.
    RayVector(Id, Id),
    /// `Line(A, v)`: through `A` with direction `v`.
    LineVector(Id, Id),
    /// `OrthogonalLine(A, v)` and `PerpendicularLine(A, v)`.
    OrthoLineVector(Id, Id),
    /// `Translate(object, v)`; `kind` is the kind of `src` and so of the image.
    Translate { kind: Kind, src: Id, by: Id },
    /// `Translate(v, A)`: the vector `v` drawn from `A`.
    TranslateTo { vector: Id, start: Id },
    /// `Direction(l)` of a line, segment or ray.
    Direction(Id),
    /// `UnitVector(v)` of a vector, line, segment or ray.
    Unit(Id),
    /// `OrthogonalVector(v)` and `PerpendicularVector(v)`.
    Ortho(Id),
    /// `UnitOrthogonalVector(v)` and `UnitPerpendicularVector(v)`.
    UnitOrtho(Id),
    /// `Length(v)` of a vector.
    Length(Id),
}

/// `DoubleUtil.isZero`.
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `MyMath.length(x, y)`: scales by the larger component.
fn ggb_length(x: f64, y: f64) -> f64 {
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

/// The coefficients that `Value::Line` and `Value::Ray` store for GeoGebra's
/// raw coefficients `(x, y, z)`; `None` where GeoGebra's `GeoLine.isDefined`
/// is false. `Value::Line` holds coefficients normalised so that `(a, b)` has
/// length 1, which keeps the direction `(b, -a)`. Every line and ray value
/// of this group comes from here.
pub(crate) fn stored_line(x: f64, y: f64, z: f64) -> Option<(f64, f64, f64)> {
    if !(x.is_finite() && y.is_finite() && z.is_finite()) || is_zero(x) && is_zero(y) {
        return None;
    }
    let len = (x * x + y * y).sqrt();
    Some((x / len, y / len, z / len))
}

fn point(c: &Construction, id: Id) -> Option<(f64, f64)> {
    match c.value(id) {
        Value::Point { x, y } => Some((x, y)),
        _ => None,
    }
}

fn vector(c: &Construction, id: Id) -> Option<(f64, f64)> {
    match c.value(id) {
        Value::Vector { x, y } => Some((x, y)),
        _ => None,
    }
}

/// `GeoVec3D.lineThroughPoints` for two finite points: directed from `p` to `q`.
fn raw_through(p: (f64, f64), q: (f64, f64)) -> (f64, f64, f64) {
    (p.1 - q.1, q.0 - p.0, p.0 * q.1 - p.1 * q.0)
}

/// `GeoVec3D.lineThroughPointVector`: through `p` with direction `v`.
fn raw_along(p: (f64, f64), v: (f64, f64)) -> (f64, f64, f64) {
    (-v.1, v.0, p.0 * v.1 - p.1 * v.0)
}

/// `AlgoOrthoLinePointVector`: `GeoVec3D.cross(P, -v.y, v.x, 0)` for a point
/// with homogeneous coordinate 1.
fn raw_ortho(p: (f64, f64), v: (f64, f64)) -> (f64, f64, f64) {
    (-v.0, -v.1, p.0 * v.0 + p.1 * v.1)
}

/// `GeoLine.getNormalizedCoefficients`: scales by powers of two until some
/// coefficient is at least `min` and some coefficient at most `max`.
fn rescale(v: (f64, f64, f64), max: f64, min: f64) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = v;
    if is_zero(x) && is_zero(y) && is_zero(z) || !(x.is_finite() && y.is_finite() && z.is_finite())
    {
        return v;
    }
    while x.abs() < min && y.abs() < min && z.abs() < min {
        (x, y, z) = (x * 2.0, y * 2.0, z * 2.0);
    }
    while x.abs() > max && y.abs() > max && z.abs() > max {
        (x, y, z) = (x / 2.0, y / 2.0, z / 2.0);
    }
    (x, y, z)
}

/// The raw `(x, y)` that GeoGebra holds for the line, segment or ray `id`,
/// also when it is undefined because they vanish.
///
/// Rebuilt from the definition for `Line(A, B)`, `Segment(A, B)`, the
/// parallel `Line(P, l)` (the parent's coefficients), `PerpendicularLine(P, l)`
/// (the parent's normal turned and rescaled), the lines and rays of this
/// group and their translations (a translation keeps `x` and `y`). The point
/// `P` is taken with homogeneous coordinate 1, which holds for free points.
/// Any other line is used with its stored coefficients, which equal
/// GeoGebra's only up to a positive or negative factor.
fn raw_normal(c: &Construction, mut id: Id) -> Option<(f64, f64)> {
    loop {
        match c.def(id) {
            Def::Line(p, q) | Def::Segment(p, q) | Def::Vector(VectorDef::RayPoints(p, q)) => {
                let (x, y, _) = raw_through(point(c, p)?, point(c, q)?);
                return Some((x, y));
            }
            Def::Vector(VectorDef::RayVector(p, v) | VectorDef::LineVector(p, v)) => {
                let (x, y, _) = raw_along(point(c, p)?, vector(c, v)?);
                return Some((x, y));
            }
            Def::Vector(VectorDef::OrthoLineVector(p, v)) => {
                let (x, y, _) = raw_ortho(point(c, p)?, vector(c, v)?);
                return Some((x, y));
            }
            Def::Parallel { through, to } => {
                point(c, through)?;
                id = to;
            }
            Def::Perpendicular { through, to } => {
                // `AlgoOrthoLinePointLine`: `GeoVec3D.cross(P, l.x, l.y, 0)`,
                // then rescaled by `GeoLine.getNormalizedCoefficients`.
                let (lx, ly) = raw_normal(c, to)?;
                let (px, py) = point(c, through)?;
                let (x, y, _) = rescale((-ly, lx, px * ly - py * lx), 10e6, 1e-2);
                return Some((x, y));
            }
            Def::Vector(VectorDef::Translate { src, by, .. }) => {
                vector(c, by)?;
                id = src;
            }
            _ => {
                return match c.value(id) {
                    Value::Line { a, b, .. } | Value::Ray { a, b, .. } => Some((a, b)),
                    Value::Segment { x1, y1, x2, y2 } => Some((y1 - y2, x2 - x1)),
                    _ => None,
                }
            }
        }
    }
}

/// The `(x, y)` of a vector, or the raw direction `(y, -x)` of a line,
/// segment or ray (`AlgoUnitVectorVector`, `AlgoUnitVectorLine`).
fn direction_of(c: &Construction, id: Id) -> Option<(f64, f64)> {
    if c.kind(id) == Kind::Vector {
        vector(c, id)
    } else {
        let (x, y) = raw_normal(c, id)?;
        Some((y, -x))
    }
}

/// `(x, y)` divided by its length; `None` for a zero vector.
fn unit(x: f64, y: f64) -> Option<Value> {
    let len = ggb_length(x, y);
    let (ux, uy) = (x / len, y / len);
    (ux.is_finite() && uy.is_finite()).then_some(Value::Vector { x: ux, y: uy })
}

fn line_value((x, y, z): (f64, f64, f64)) -> Option<Value> {
    let (a, b, c) = stored_line(x, y, z)?;
    Some(Value::Line { a, b, c })
}

fn ray_value(start: (f64, f64), (x, y, z): (f64, f64, f64)) -> Option<Value> {
    let (a, b, c) = stored_line(x, y, z)?;
    Some(Value::Ray {
        x: start.0,
        y: start.1,
        a,
        b,
        c,
    })
}

/// `AlgoTranslate`: the image of the value `v` under the translation by
/// `(dx, dy)`. GeoGebra moves a line by changing its constant
/// (`GeoLine.translate`), a circle by moving its centre, a segment and a ray
/// by moving the points they are made from, and leaves a vector unchanged
/// (`GeoVector.translate` does nothing).
fn translate(v: Value, dx: f64, dy: f64) -> Option<Value> {
    Some(match v {
        Value::Point { x, y } => Value::Point {
            x: x + dx,
            y: y + dy,
        },
        Value::Line { a, b, c } => Value::Line {
            a,
            b,
            c: c - (a * dx + b * dy),
        },
        Value::Segment { x1, y1, x2, y2 } => Value::Segment {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
        },
        Value::Circle { cx, cy, r } => Value::Circle {
            cx: cx + dx,
            cy: cy + dy,
            r,
        },
        Value::Ray { x, y, a, b, c } => Value::Ray {
            x: x + dx,
            y: y + dy,
            a,
            b,
            c: c - (a * dx + b * dy),
        },
        Value::Vector { .. } => v,
        Value::Number(_) | Value::Undefined => return None,
    })
}

/// Where a vector is drawn from: the start point GeoGebra gives it, or the
/// origin when it has none. `None` when `id` is not a vector or its start
/// point is undefined.
pub fn start(c: &Construction, id: Id) -> Option<(f64, f64)> {
    if c.kind(id) != Kind::Vector {
        return None;
    }
    let from = |p: Id| match c.value(p) {
        Value::Point { x, y } => Some((x, y)),
        Value::Ray { x, y, .. } => Some((x, y)),
        Value::Segment { x1, y1, .. } => Some((x1, y1)),
        _ => None,
    };
    match c.def(id) {
        Def::Vector(VectorDef::Between(p, _)) => from(p),
        Def::Vector(VectorDef::TranslateTo { start, .. }) => from(start),
        Def::Vector(VectorDef::Translate { src, .. }) => start(c, src),
        Def::Vector(
            VectorDef::Direction(l)
            | VectorDef::Unit(l)
            | VectorDef::Ortho(l)
            | VectorDef::UnitOrtho(l),
        ) => match (c.kind(l), c.def(l)) {
            (Kind::Vector, _) => start(c, l),
            (_, Def::Line(p, _)) => from(p),
            (Kind::Segment | Kind::Ray, _) => from(l),
            _ => Some((0.0, 0.0)),
        },
        _ => Some((0.0, 0.0)),
    }
}

impl Group for VectorDef {
    fn kind(&self) -> Kind {
        match *self {
            VectorDef::Free { .. }
            | VectorDef::Between(..)
            | VectorDef::Position(_)
            | VectorDef::TranslateTo { .. }
            | VectorDef::Direction(_)
            | VectorDef::Unit(_)
            | VectorDef::Ortho(_)
            | VectorDef::UnitOrtho(_) => Kind::Vector,
            VectorDef::RayPoints(..) | VectorDef::RayVector(..) => Kind::Ray,
            VectorDef::LineVector(..) | VectorDef::OrthoLineVector(..) => Kind::Line,
            VectorDef::Translate { kind, .. } => kind,
            VectorDef::Length(_) => Kind::Number,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            VectorDef::Free { .. } => Parents::none(),
            VectorDef::Between(p, q)
            | VectorDef::RayPoints(p, q)
            | VectorDef::RayVector(p, q)
            | VectorDef::LineVector(p, q)
            | VectorDef::OrthoLineVector(p, q) => Parents::of(&[p, q]),
            VectorDef::Translate { src, by, .. } => Parents::of(&[src, by]),
            VectorDef::TranslateTo { vector, start } => Parents::of(&[vector, start]),
            VectorDef::Position(p)
            | VectorDef::Direction(p)
            | VectorDef::Unit(p)
            | VectorDef::Ortho(p)
            | VectorDef::UnitOrtho(p)
            | VectorDef::Length(p) => Parents::of(&[p]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            VectorDef::Free { x, y } => {
                if x.is_finite() && y.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            VectorDef::Between(p, q) | VectorDef::RayPoints(p, q) => {
                c.expect(p, POINT)?;
                c.expect(q, POINT)
            }
            VectorDef::Position(p) => c.expect(p, POINT),
            VectorDef::RayVector(p, v)
            | VectorDef::LineVector(p, v)
            | VectorDef::OrthoLineVector(p, v) => {
                c.expect(p, POINT)?;
                c.expect(v, VECTOR)
            }
            VectorDef::Translate { kind, src, by } => {
                c.expect(src, TRANSLATABLE)?;
                c.expect(src, only(kind))?;
                c.expect(by, VECTOR)
            }
            VectorDef::TranslateTo { vector, start } => {
                c.expect(vector, VECTOR)?;
                c.expect(start, POINT)
            }
            VectorDef::Direction(l) => c.expect(l, DIRECTED),
            VectorDef::Unit(v) | VectorDef::Ortho(v) | VectorDef::UnitOrtho(v) => {
                c.expect(v, VECTOR_OR_DIRECTED)
            }
            VectorDef::Length(v) => c.expect(v, VECTOR),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            VectorDef::Free { x, y } => Some(Value::Vector { x, y }),
            VectorDef::Between(p, q) => {
                // `AlgoVector`: `P.vectorTo(Q)`.
                let (p, q) = (point(c, p)?, point(c, q)?);
                Some(Value::Vector {
                    x: q.0 - p.0,
                    y: q.1 - p.1,
                })
            }
            VectorDef::Position(p) => {
                let (x, y) = point(c, p)?;
                Some(Value::Vector { x, y })
            }
            VectorDef::RayPoints(p, q) => {
                let (p, q) = (point(c, p)?, point(c, q)?);
                ray_value(p, raw_through(p, q))
            }
            VectorDef::RayVector(p, v) => {
                let p = point(c, p)?;
                ray_value(p, raw_along(p, vector(c, v)?))
            }
            VectorDef::LineVector(p, v) => line_value(raw_along(point(c, p)?, vector(c, v)?)),
            VectorDef::OrthoLineVector(p, v) => line_value(raw_ortho(point(c, p)?, vector(c, v)?)),
            VectorDef::Translate { src, by, .. } => {
                let (dx, dy) = vector(c, by)?;
                translate(c.value(src), dx, dy)
            }
            // `AlgoTranslateVector` copies the coordinates and does not read
            // the start point.
            VectorDef::TranslateTo { vector: v, .. } => {
                let (x, y) = vector(c, v)?;
                Some(Value::Vector { x, y })
            }
            VectorDef::Direction(l) => {
                let (x, y) = direction_of(c, l)?;
                Some(Value::Vector { x, y })
            }
            VectorDef::Unit(v) => {
                let (x, y) = direction_of(c, v)?;
                unit(x, y)
            }
            VectorDef::Ortho(v) => {
                // `AlgoOrthoVectorVector` turns a vector by +90 degrees;
                // `AlgoOrthoVectorLine` takes the raw normal of a line.
                let (x, y) = if c.kind(v) == Kind::Vector {
                    let (x, y) = vector(c, v)?;
                    (-y, x)
                } else {
                    raw_normal(c, v)?
                };
                Some(Value::Vector { x, y })
            }
            VectorDef::UnitOrtho(v) => {
                let (x, y) = if c.kind(v) == Kind::Vector {
                    let (x, y) = vector(c, v)?;
                    (-y, x)
                } else {
                    raw_normal(c, v)?
                };
                unit(x, y)
            }
            VectorDef::Length(v) => {
                let (x, y) = vector(c, v)?;
                Some(Value::Number(ggb_length(x, y)))
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        // Commands shared with other groups and the built-in commands: only
        // the forms whose last argument is a vector are handled here.
        let vector_arg = |label: &str| c.id(label).filter(|&id| c.kind(id) == Kind::Vector);
        match func {
            "Line" | "OrthogonalLine" | "PerpendicularLine" => {
                if args.len() != 2 {
                    return None;
                }
                let v = vector_arg(args[1])?;
                let p = c.id(args[0])?;
                return Some(Ok(if func == "Line" {
                    VectorDef::LineVector(p, v)
                } else {
                    VectorDef::OrthoLineVector(p, v)
                }));
            }
            "Length" => {
                if args.len() != 1 {
                    return None;
                }
                return Some(Ok(VectorDef::Length(vector_arg(args[0])?)));
            }
            _ => {}
        }
        let arity = match func {
            "Vector" => 1..=2,
            "Ray" | "Translate" => 2..=2,
            "Direction"
            | "UnitVector"
            | "OrthogonalVector"
            | "PerpendicularVector"
            | "UnitOrthogonalVector"
            | "UnitPerpendicularVector" => 1..=1,
            _ => return None,
        };
        if !arity.contains(&args.len()) {
            return Some(Err(format!(
                "`{func}` does not take {} argument(s)",
                args.len()
            )));
        }
        let ids: Result<Vec<Id>, String> = args.iter().map(|a| object(c, a)).collect();
        let ids = match ids {
            Ok(ids) => ids,
            Err(e) => return Some(Err(e)),
        };
        Some(Ok(match (func, ids.as_slice()) {
            ("Vector", &[p]) => VectorDef::Position(p),
            ("Vector", &[p, q]) => VectorDef::Between(p, q),
            ("Ray", &[p, q]) => {
                if c.kind(q) == Kind::Vector {
                    VectorDef::RayVector(p, q)
                } else {
                    VectorDef::RayPoints(p, q)
                }
            }
            ("Translate", &[src, by]) => {
                // `CmdTranslate`: a vector and a point give the vector drawn
                // from the point; otherwise the first object is moved by the
                // second, which must be a vector here.
                if c.kind(src) == Kind::Vector && c.kind(by) == Kind::Point {
                    VectorDef::TranslateTo {
                        vector: src,
                        start: by,
                    }
                } else {
                    VectorDef::Translate {
                        kind: c.kind(src),
                        src,
                        by,
                    }
                }
            }
            ("Direction", &[l]) => VectorDef::Direction(l),
            ("UnitVector", &[v]) => VectorDef::Unit(v),
            ("OrthogonalVector" | "PerpendicularVector", &[v]) => VectorDef::Ortho(v),
            ("UnitOrthogonalVector" | "UnitPerpendicularVector", &[v]) => VectorDef::UnitOrtho(v),
            _ => unreachable!("argument count checked above"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{exec, load};

    const BASE: &str = "A = (1, 2)\nB = (4, 6)\nC = (0, 1)\nu = (3, -1)\nr = 2\n\
                        l = Line(A, B)\ns = Segment(A, B)\nk = Circle(A, r)\n";

    fn base() -> Construction {
        load(BASE).unwrap()
    }

    fn value(c: &Construction, label: &str) -> Value {
        c.value(c.id(label).unwrap())
    }

    fn def(c: &Construction, label: &str) -> Def {
        c.def(c.id(label).unwrap())
    }

    fn vec_of(c: &Construction, label: &str) -> (f64, f64) {
        match value(c, label) {
            Value::Vector { x, y } => (x, y),
            other => panic!("{label} is {other:?}"),
        }
    }

    fn close(p: (f64, f64), q: (f64, f64)) {
        assert!(
            (p.0 - q.0).abs() < 1e-12 && (p.1 - q.1).abs() < 1e-12,
            "{p:?} != {q:?}"
        );
    }

    fn err(c: &mut Construction, line: &str) -> String {
        exec(c, line).unwrap_err()
    }

    #[test]
    fn lower_case_coordinates_make_a_free_vector() {
        let mut c = base();
        assert_eq!(c.kind(c.id("u").unwrap()), Kind::Vector);
        assert_eq!(c.kind(c.id("A").unwrap()), Kind::Point);
        assert!(def(&c, "u").is_free());
        exec(&mut c, "_p = (1, 1)").unwrap();
        assert_eq!(c.kind(c.id("_p").unwrap()), Kind::Point);
        exec(&mut c, "w = Vector(A)").unwrap();
        let u = c.id("u").unwrap();
        c.move_point(u, 0.5, 0.25).unwrap();
        assert_eq!(value(&c, "u"), Value::Vector { x: 0.5, y: 0.25 });
        let w = c.id("w").unwrap();
        assert!(c.move_point(w, 1.0, 1.0).is_err());
    }

    #[test]
    fn every_syntax_parses() {
        let mut c = base();
        for line in [
            "v = Vector(A, B)",
            "w = Vector(A)",
            "g = Ray(A, B)",
            "h = Ray(A, u)",
            "m = Line(A, u)",
            "n = OrthogonalLine(A, u)",
            "n2 = PerpendicularLine(A, u)",
            "d = Direction(l)",
            "d2 = Direction(s)",
            "d3 = Direction(g)",
            "q = UnitVector(u)",
            "q2 = UnitVector(l)",
            "o = OrthogonalVector(u)",
            "o2 = PerpendicularVector(s)",
            "t = UnitOrthogonalVector(u)",
            "t2 = UnitPerpendicularVector(g)",
            "len = Length(u)",
            "P = Translate(A, u)",
            "tl = Translate(l, u)",
            "ts = Translate(s, u)",
            "tk = Translate(k, u)",
            "tg = Translate(g, u)",
            "tu = Translate(u, v)",
            "tv = Translate(u, A)",
        ] {
            exec(&mut c, line).unwrap_or_else(|e| panic!("{line}: {e}"));
        }
        let kind = |l: &str| c.kind(c.id(l).unwrap());
        assert_eq!(kind("g"), Kind::Ray);
        assert_eq!(kind("m"), Kind::Line);
        assert_eq!(kind("len"), Kind::Number);
        assert_eq!(kind("P"), Kind::Point);
        assert_eq!(kind("ts"), Kind::Segment);
        assert_eq!(kind("tk"), Kind::Circle);
        assert_eq!(kind("tg"), Kind::Ray);
        assert_eq!(kind("tv"), Kind::Vector);
        assert!(matches!(
            def(&c, "tv"),
            Def::Vector(VectorDef::TranslateTo { .. })
        ));
        assert!(matches!(
            def(&c, "n2"),
            Def::Vector(VectorDef::OrthoLineVector(..))
        ));
    }

    #[test]
    fn shared_command_names_keep_their_old_handlers() {
        let mut c = base();
        exec(&mut c, "P = (5, 1)").unwrap();
        exec(&mut c, "a = Line(A, B)").unwrap();
        exec(&mut c, "b = Line(P, l)").unwrap();
        exec(&mut c, "p = PerpendicularLine(P, l)").unwrap();
        exec(&mut c, "p2 = OrthogonalLine(P, s)").unwrap();
        exec(&mut c, "len = Length(s)").unwrap();
        assert!(matches!(def(&c, "a"), Def::Line(..)));
        assert!(matches!(def(&c, "b"), Def::Parallel { .. }));
        assert!(matches!(def(&c, "p"), Def::Perpendicular { .. }));
        assert!(matches!(def(&c, "p2"), Def::Perpendicular { .. }));
        assert!(matches!(def(&c, "len"), Def::Measure(..)));
        // Errors of the built-in commands are unchanged.
        assert!(err(&mut c, "x1 = Line(A)").contains("`Line` does not take 1"));
        assert!(err(&mut c, "x2 = Line(A, Z)").contains("unknown object `Z`"));
        assert!(err(&mut c, "x3 = Line(A, u, B)").contains("does not take 3"));
    }

    #[test]
    fn wrong_counts_and_kinds_are_rejected() {
        let mut c = base();
        assert!(err(&mut c, "a = Vector(A, B, C)").contains("`Vector` does not take 3"));
        assert!(err(&mut c, "a = Ray(A)").contains("`Ray` does not take 1"));
        assert!(err(&mut c, "a = Translate(A)").contains("`Translate` does not take 1"));
        assert!(err(&mut c, "a = Direction(l, s)").contains("`Direction` does not take 2"));
        assert!(err(&mut c, "a = UnitVector(Z)").contains("unknown object `Z`"));
        assert!(err(&mut c, "a = Vector(l)").contains("expected a point"));
        assert!(err(&mut c, "a = Direction(u)").contains("expected a line"));
        assert!(err(&mut c, "a = UnitVector(A)").contains("expected a vector"));
        assert!(err(&mut c, "a = Translate(A, B)").contains("expected a vector"));
        assert!(err(&mut c, "a = Translate(r, u)").contains("`r` is a number"));
        assert!(err(&mut c, "a = Ray(A, l)").contains("expected a point"));
        // `Length` of a point is not handled here and not by any other group.
        assert!(exec(&mut c, "a = Length(A)").is_err());
        // Nothing was added by the failed lines.
        assert_eq!(c.len(), load(BASE).unwrap().len());
    }

    #[test]
    fn vectors_follow_geogebra() {
        let c = load(&format!(
            "{BASE}v = Vector(A, B)\nw = Vector(A)\nd = Direction(l)\no = OrthogonalVector(l)\n\
             q = UnitVector(l)\nt = UnitOrthogonalVector(u)\nn = Length(v)\nov = OrthogonalVector(u)\n"
        ))
        .unwrap();
        close(vec_of(&c, "v"), (3.0, 4.0));
        close(vec_of(&c, "w"), (1.0, 2.0));
        close(vec_of(&c, "d"), (3.0, 4.0));
        close(vec_of(&c, "o"), (-4.0, 3.0));
        close(vec_of(&c, "q"), (0.6, 0.8));
        close(vec_of(&c, "ov"), (1.0, 3.0));
        let s10 = 10f64.sqrt();
        close(vec_of(&c, "t"), (1.0 / s10, 3.0 / s10));
        assert_eq!(value(&c, "n"), Value::Number(5.0));
    }

    #[test]
    fn degenerate_lines_give_zero_directions_but_no_unit_vectors() {
        let mut c = load(&format!(
            "{BASE}g = Ray(A, B)\nd = Direction(g)\nq = UnitVector(g)\no = OrthogonalVector(l)\n"
        ))
        .unwrap();
        let b = c.id("B").unwrap();
        c.move_point(b, 1.0, 2.0).unwrap();
        assert_eq!(value(&c, "g"), Value::Undefined);
        close(vec_of(&c, "d"), (0.0, 0.0));
        close(vec_of(&c, "o"), (0.0, 0.0));
        assert_eq!(value(&c, "q"), Value::Undefined);
    }

    #[test]
    fn rays_point_along_their_direction() {
        let c = load(&format!(
            "{BASE}g = Ray(A, B)\nh = Ray(A, u)\nm = Line(A, u)\n"
        ))
        .unwrap();
        let Value::Ray { x, y, a, b, .. } = value(&c, "g") else {
            panic!()
        };
        assert_eq!((x, y), (1.0, 2.0));
        close((b, -a), (0.6, 0.8));
        let Value::Ray { a, b, c: k, .. } = value(&c, "h") else {
            panic!()
        };
        let s10 = 10f64.sqrt();
        close((b, -a), (3.0 / s10, -1.0 / s10));
        // The ray and the line through A with direction u share coefficients.
        assert_eq!(value(&c, "m"), Value::Line { a, b, c: k });
    }

    #[test]
    fn translation_moves_each_kind_and_keeps_vectors() {
        let c = load(&format!(
            "{BASE}g = Ray(A, B)\nP = Translate(A, u)\ntl = Translate(l, u)\nts = Translate(s, u)\n\
             tk = Translate(k, u)\ntg = Translate(g, u)\nv = Vector(A, B)\ntv = Translate(v, u)\n\
             tw = Translate(v, C)\n"
        ))
        .unwrap();
        assert_eq!(value(&c, "P"), Value::Point { x: 4.0, y: 1.0 });
        assert_eq!(
            value(&c, "ts"),
            Value::Segment {
                x1: 4.0,
                y1: 1.0,
                x2: 7.0,
                y2: 5.0
            }
        );
        assert_eq!(
            value(&c, "tk"),
            Value::Circle {
                cx: 4.0,
                cy: 1.0,
                r: 2.0
            }
        );
        let Value::Line { a, b, c: k } = value(&c, "tl") else {
            panic!()
        };
        assert!((a * 4.0 + b * 1.0 + k).abs() < 1e-12);
        let Value::Ray { x, y, .. } = value(&c, "tg") else {
            panic!()
        };
        assert_eq!((x, y), (4.0, 1.0));
        assert_eq!(value(&c, "tv"), value(&c, "v"));
        assert_eq!(value(&c, "tw"), value(&c, "v"));
        assert_eq!(start(&c, c.id("tw").unwrap()), Some((0.0, 1.0)));
        assert_eq!(start(&c, c.id("v").unwrap()), Some((1.0, 2.0)));
        assert_eq!(start(&c, c.id("u").unwrap()), Some((0.0, 0.0)));
        assert_eq!(start(&c, c.id("A").unwrap()), None);
    }

    #[test]
    fn rays_and_vectors_are_drawn() {
        let c = load(&format!("{BASE}g = Ray(A, B)\nv = Vector(A, B)\n")).unwrap();
        let view = crate::svg::View::fit(&c);
        let svg = crate::svg::render(&c, view, 400, 300);
        // An arrow head for each of the vectors u and v.
        assert_eq!(svg.matches("<polygon").count(), 2);
        assert!(view.xmax >= 4.0 && view.ymax >= 6.0);
    }

    #[test]
    fn directions_of_translated_lines_keep_the_raw_scale() {
        let c = load(&format!(
            "{BASE}g = Ray(A, u)\ntg = Translate(g, u)\nd = Direction(tg)\nP = (5, 1)\n\
             p = Line(P, l)\ndp = Direction(p)\n"
        ))
        .unwrap();
        close(vec_of(&c, "d"), (3.0, -1.0));
        close(vec_of(&c, "dp"), (3.0, 4.0));
    }
}
