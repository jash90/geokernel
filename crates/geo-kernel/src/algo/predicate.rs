// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreCollinear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreParallel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdArePerpendicular.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreConcurrent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreConcyclic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreEqual.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAreCongruent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdIsTangent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreCollinear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreParallel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoArePerpendicular.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreConcurrent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreConcyclic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreEqual.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreCongruent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoIsTangent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoRay.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoBoolean.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/Coords.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Boolean tests on objects: `AreCollinear`, `AreParallel`,
//! `ArePerpendicular`, `AreConcurrent`, `AreConcyclic`, `AreEqual`,
//! `AreCongruent` and `IsTangent`.
//!
//! Only GeoGebra's numeric `compute()` is ported, with the same tolerance
//! tests (`DoubleUtil.isZero`, `isEqual`, `Kernel.MIN_PRECISION`). The
//! symbolic prover is not part of the kernel.
//!
//! An input that is undefined in the kernel is read the way GeoGebra holds
//! an undefined object: NaN coordinates, so that every test fails, except a
//! line that degenerated to all-zero coefficients (see [`raw_line`]).

use super::vector::VectorDef;
use super::{normalized_coefficients, Group, Parents, CIRCLE, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};

/// Everything a `GeoLine` can be: a line, a segment or a ray.
const LINE_LIKE: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray];
/// Every kind of object `AreEqual` and `AreCongruent` compare.
const ANY: &[Kind] = &[
    Kind::Point,
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::Circle,
    Kind::Vector,
    Kind::Number,
    Kind::Boolean,
];

/// Boolean tests on objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PredicateDef {
    /// `AreCollinear(A, B, C)`.
    Collinear(Id, Id, Id),
    /// `AreParallel(g, h)` for lines, segments and rays.
    Parallel(Id, Id),
    /// `ArePerpendicular(g, h)` for lines, segments and rays.
    Perpendicular(Id, Id),
    /// `AreConcurrent(g, h, i)` for lines, segments and rays.
    Concurrent(Id, Id, Id),
    /// `AreConcyclic(A, B, C, D)`.
    Concyclic(Id, Id, Id, Id),
    /// `AreEqual(a, b)` for objects of any kind.
    Equal(Id, Id),
    /// `AreCongruent(a, b)` for objects of any kind.
    Congruent(Id, Id),
    /// `IsTangent(line, circle)`.
    Tangent { line: Id, circle: Id },
}

/// `-eps < v < eps` (`DoubleUtil.isZero`).
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `DoubleUtil.isZero(v, eps)`.
fn is_zero_eps(v: f64, eps: f64) -> bool {
    -eps < v && v < eps
}

/// `DoubleUtil.isEqual(a, b)`.
fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal_eps(x: f64, y: f64, eps: f64) -> bool {
    x == y || ((x - eps) < y && y < (x + eps))
}

/// `Math.min` and `Math.max` of Java propagate NaN, unlike `f64::min`/`max`.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

type Line3 = (f64, f64, f64);
const NAN3: Line3 = (f64::NAN, f64::NAN, f64::NAN);

/// A point as GeoGebra holds it: NaN coordinates when it is undefined.
fn point(c: &Construction, id: Id) -> (f64, f64) {
    c.point(id).unwrap_or((f64::NAN, f64::NAN))
}

/// The raw coefficients `(x, y, z)` of a line, segment or ray, as the
/// `GeoLine` holds them. A line that degenerated to all-zero coefficients is
/// undefined in the kernel, but GeoGebra keeps the zeros, and so do the
/// parallel and perpendicular lines made from it. They are rebuilt here from
/// the points and lines they are made from; any other undefined line has NaN
/// coefficients.
fn raw_line(c: &Construction, id: Id) -> Line3 {
    super::vector::raw_or_undefined(c, id).unwrap_or(NAN3)
}

/// The start point of a ray, also when the kernel holds the ray as
/// undefined (`Ray(A, A)`): GeoGebra keeps the start point.
fn ray_start(c: &Construction, id: Id) -> Option<(f64, f64)> {
    if let Value::Ray { x, y, .. } = c.value(id) {
        return Some((x, y));
    }
    match c.def(id) {
        Def::Vector(VectorDef::RayPoints(p, _) | VectorDef::RayVector(p, _)) => c.point(p),
        _ => None,
    }
}

/// Objects that a predicate reads besides its arguments: for a line the
/// kernel holds as undefined, the objects its zero coefficients come from.
pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Vec<Id> {
    let mut ids = Vec::new();
    if let Def::Predicate(p) = *def {
        let args: Vec<Id> = p.parents().as_slice().to_vec();
        for id in args {
            if LINE_LIKE.contains(&c.kind(id)) {
                super::vector::line_hidden(c, id, &mut ids);
            }
        }
    }
    ids
}

/// `GeoPoint.collinearND`: the cross product of `A - B` and `A - C` vanishes.
fn collinear(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let (bx, by) = (a.0 - b.0, a.1 - b.1);
    let (cx, cy) = (a.0 - c.0, a.1 - c.1);
    is_zero(bx * cy - by * cx)
}

/// `GeoLine.concurrent`: the determinant of the three coefficient triples.
fn concurrent(l1: Line3, l2: Line3, l3: Line3) -> bool {
    let det = l1.0 * l2.1 * l3.2 + l2.0 * l3.1 * l1.2 + l3.0 * l1.1 * l2.2
        - l3.0 * l2.1 * l1.2
        - l2.0 * l1.1 * l3.2
        - l1.0 * l3.1 * l2.2;
    is_zero(det)
}

/// `GeoPoint.concyclic`: Ptolemy's theorem, with the loosest tolerance, for
/// points whose homogeneous `z` is 1.
fn concyclic(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let (ax, ay, az) = (a.0, a.1, 1.0);
    let (bx, by, bz) = (b.0, b.1, 1.0);
    let (cx, cy, cz) = (c.0, c.1, 1.0);
    let (dx, dy, dz) = (d.0, d.1, 1.0);
    let ab = (bx * az - ax * bz).hypot(by * az - ay * bz);
    let ac = (cx * az - ax * cz).hypot(cy * az - ay * cz);
    let ad = (dx * az - ax * dz).hypot(dy * az - ay * dz);
    let bc = (cx * bz - bx * cz).hypot(cy * bz - by * cz);
    let bd = (dx * bz - bx * dz).hypot(dy * bz - by * dz);
    let cd = (dx * cz - cx * dz).hypot(dy * cz - cy * dz);
    is_zero_eps(ab * cd + bc * ad - ac * bd, MIN_PRECISION)
        || is_zero_eps(ab * cd + ac * bd - bc * ad, MIN_PRECISION)
        || is_zero_eps(bc * ad + ac * bd - ab * cd, MIN_PRECISION)
}

/// `GeoPoint.isEqualPoint2D` for finite points: both coordinates agree.
fn same_point(p: (f64, f64), q: (f64, f64)) -> bool {
    is_equal(p.0, q.0) && is_equal(p.1, q.1)
}

/// `GeoVec3D.linDep`: the coefficient triples are proportional.
fn lin_dep(g: Line3, h: Line3) -> bool {
    let n1 = java_max(g.0.abs(), java_max(g.1.abs(), g.2.abs()));
    let n2 = java_max(h.0.abs(), java_max(h.1.abs(), h.2.abs()));
    let (x1, y1, z1) = (g.0 / n1, g.1 / n1, g.2 / n1);
    let (x2, y2, z2) = (h.0 / n2, h.1 / n2, h.2 / n2);
    is_equal(x1 * y2, x2 * y1) && is_equal(z1 * y2, z2 * y1) && is_equal(x1 * z2, x2 * z1)
}

/// `GeoLine.isParallel`: `this` is `g`, the argument `h`.
fn parallel(g: Line3, h: Line3) -> bool {
    is_equal(h.0 * g.1, h.1 * g.0)
}

/// `GeoLine.isPerpendicular`.
fn perpendicular(g: Line3, h: Line3) -> bool {
    is_equal(h.0 * g.0, -h.1 * g.1)
}

/// `GeoLine.isDegenerate`: all-zero direction with a finite offset.
fn degenerate(g: Line3) -> bool {
    g.0 == 0.0 && g.1 == 0.0 && g.2.is_finite()
}

/// `GeoLine.isEqualExtended` for two lines (not rays or segments).
fn lines_equal(c: &Construction, a: Id, b: Id) -> bool {
    let (ga, gb) = (raw_line(c, a), raw_line(c, b));
    if !c.value(a).is_defined() || !c.value(b).is_defined() {
        return degenerate(ga) && degenerate(gb);
    }
    lin_dep(ga, gb)
}

/// The flat matrix of a circle as a GeoGebra conic holds it
/// (`GeoQuadricND.setSphereNDMatrix`).
fn circle_matrix(cx: f64, cy: f64, r: f64) -> [f64; 6] {
    let mut m2 = -r * r;
    m2 += cx * cx;
    m2 += cy * cy;
    [1.0, 1.0, m2, 0.0, -cx, -cy]
}

/// `GeoQuadricND.hasEqualMatrix`: the matrices are proportional.
fn equal_matrix(a: &[f64; 6], b: &[f64; 6]) -> bool {
    let mut lambda1 = 0.0;
    let mut equal = true;
    for i in 0..6 {
        let a_zero = is_zero(a[i]);
        let b_zero = is_zero(b[i]);
        if a_zero != b_zero {
            equal = false;
        } else if !a_zero && !b_zero {
            if lambda1 == 0.0 {
                lambda1 = a[i] / b[i];
            } else {
                equal = is_equal(a[i], lambda1 * b[i]);
            }
        }
        if !equal {
            break;
        }
    }
    equal
}

/// `GeoConicND.isEqualExtended` for two circles.
fn circles_equal(c: &Construction, a: Id, b: Id) -> bool {
    match (c.circle(a), c.circle(b)) {
        // Both conics are empty.
        (None, None) => true,
        (Some(p), Some(q)) => {
            equal_matrix(&circle_matrix(p.0, p.1, p.2), &circle_matrix(q.0, q.1, q.2))
        }
        // The matrix of an undefined conic is NaN.
        _ => false,
    }
}

/// Length of a segment as `GeoSegment` holds it: below the tolerance is 0.
fn segment_length(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let (dx, dy) = (x2 - x1, y2 - y1);
    let (ax, ay) = (dx.abs(), dy.abs());
    let length = if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    };
    if is_zero(length) {
        0.0
    } else {
        length
    }
}

fn truth(v: Value) -> bool {
    matches!(v, Value::Bool(true))
}

/// `isEqualExtended` of the first object, called with the second. Kinds added
/// later are equal to nothing until they are handled here.
#[allow(unreachable_patterns)]
fn equal(c: &Construction, a: Id, b: Id) -> bool {
    let (ka, kb) = (c.kind(a), c.kind(b));
    match ka {
        Kind::Point => ka == kb && same_point(point(c, a), point(c, b)),
        Kind::Line => match kb {
            Kind::Line => lines_equal(c, a, b),
            // A segment or ray is never equal to a line, but a line that
            // is undefined is equal to every degenerate line-like object.
            Kind::Segment | Kind::Ray => {
                (!c.value(a).is_defined() || !c.value(b).is_defined())
                    && degenerate(raw_line(c, a))
                    && degenerate(raw_line(c, b))
            }
            _ => false,
        },
        Kind::Segment => match (c.value(a), c.value(b)) {
            (
                Value::Segment { x1, y1, x2, y2 },
                Value::Segment {
                    x1: u1,
                    y1: v1,
                    x2: u2,
                    y2: v2,
                },
            ) => {
                let (s, e, s2, e2) = ((x1, y1), (x2, y2), (u1, v1), (u2, v2));
                (same_point(s, s2) && same_point(e, e2)) || (same_point(s, e2) && same_point(e, s2))
            }
            _ => false,
        },
        // `GeoRay.isEqualExtended` has no check that the rays are defined.
        Kind::Ray => {
            kb == Kind::Ray && {
                let (g, h) = (raw_line(c, a), raw_line(c, b));
                // `GeoLine.isSameDirection`
                let same_direction = h.0 * g.0 >= 0.0 && h.1 * g.1 >= 0.0 && parallel(g, h);
                same_direction
                    && match (ray_start(c, a), ray_start(c, b)) {
                        (Some(s), Some(t)) => same_point(t, s),
                        _ => false,
                    }
            }
        }
        Kind::Circle => ka == kb && circles_equal(c, a, b),
        Kind::Vector => match (c.value(a), c.value(b)) {
            (Value::Vector { x, y }, Value::Vector { x: u, y: v }) => {
                x.is_finite() && y.is_finite() && is_equal(x, u) && is_equal(y, v)
            }
            _ => false,
        },
        Kind::Number => match (c.value(a), c.value(b)) {
            (Value::Number(p), Value::Number(q)) => is_equal(p, q),
            _ => false,
        },
        Kind::Boolean => kb == Kind::Boolean && truth(c.value(a)) == truth(c.value(b)),
        _ => false,
    }
}

/// `isCongruent` of the first object, called with the second. `None` is the
/// prover's "unknown", which no kind here produces.
fn congruent(c: &Construction, a: Id, b: Id) -> Option<bool> {
    let (ka, kb) = (c.kind(a), c.kind(b));
    Some(match ka {
        Kind::Point => kb == Kind::Point,
        Kind::Line | Kind::Ray => LINE_LIKE.contains(&kb),
        Kind::Segment => match (c.value(a), c.value(b)) {
            (
                Value::Segment { x1, y1, x2, y2 },
                Value::Segment {
                    x1: u1,
                    y1: v1,
                    x2: u2,
                    y2: v2,
                },
            ) => is_equal(
                segment_length(x1, y1, x2, y2),
                segment_length(u1, v1, u2, v2),
            ),
            _ => false,
        },
        Kind::Circle => match (c.value(a), c.value(b)) {
            // Circles are congruent when their radii agree; a circle of
            // radius 0 is a single point, not a circle.
            (Value::Circle { r, .. }, Value::Circle { r: s, .. }) => {
                r >= STANDARD_PRECISION && s >= STANDARD_PRECISION && is_equal(r, s)
            }
            _ => false,
        },
        _ => equal(c, a, b),
    })
}

/// `AlgoIsTangent.compute`: a tangent of the circle by construction, or a
/// line that meets it in exactly one point (`AlgoIntersectLineConic`).
fn tangent(c: &Construction, line: Id, circle: Id) -> bool {
    if let Def::Lines(d) = c.def(line) {
        if super::lines::tangent_point_on(c, d, circle).is_some() {
            return true;
        }
    }
    let Some((cx, cy, r)) = c.circle(circle) else {
        return false;
    };
    let g = raw_line(c, line);
    let (x, y, z) = normalized_coefficients(g, 2.0, 0.5);
    let m = circle_matrix(cx, cy, r);
    // An arbitrary point of the line.
    let (px, py) = if x.abs() > y.abs() {
        (-z / x, 0.0)
    } else {
        (0.0, -z / y)
    };
    let sv_x = m[0] * y - m[3] * x;
    let sv_y = m[3] * y - m[1] * x;
    let u = y * sv_x - x * sv_y;
    let d = px * sv_x + py * sv_y + m[4] * y - m[5] * x;
    let w = m[2]
        + m[4] * px
        + m[5] * py
        + py * (m[5] + m[3] * px + m[1] * py)
        + px * (m[4] + m[0] * px + m[3] * py);
    let delta = java_min(
        MIN_PRECISION,
        java_max(1.0, (2.0 * d).abs() + u.abs() + w.abs()) * STANDARD_PRECISION,
    );
    if is_zero(u) {
        // A line meeting the circle once is not a tangent.
        return false;
    }
    is_equal_eps(d * d - u * w, 0.0, delta)
}

impl Group for PredicateDef {
    fn kind(&self) -> Kind {
        Kind::Boolean
    }

    fn parents(&self) -> Parents {
        match *self {
            PredicateDef::Collinear(a, b, d) | PredicateDef::Concurrent(a, b, d) => {
                Parents::of(&[a, b, d])
            }
            PredicateDef::Parallel(a, b)
            | PredicateDef::Perpendicular(a, b)
            | PredicateDef::Equal(a, b)
            | PredicateDef::Congruent(a, b) => Parents::of(&[a, b]),
            PredicateDef::Concyclic(a, b, d, e) => Parents::of(&[a, b, d, e]),
            PredicateDef::Tangent { line, circle } => Parents::of(&[line, circle]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            PredicateDef::Collinear(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            PredicateDef::Concyclic(a, b, d, e) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)?;
                c.expect(e, POINT)
            }
            PredicateDef::Parallel(a, b) | PredicateDef::Perpendicular(a, b) => {
                c.expect(a, LINE_LIKE)?;
                c.expect(b, LINE_LIKE)
            }
            PredicateDef::Concurrent(a, b, d) => {
                c.expect(a, LINE_LIKE)?;
                c.expect(b, LINE_LIKE)?;
                c.expect(d, LINE_LIKE)
            }
            PredicateDef::Equal(a, b) | PredicateDef::Congruent(a, b) => {
                c.expect(a, ANY)?;
                c.expect(b, ANY)
            }
            PredicateDef::Tangent { line, circle } => {
                c.expect(line, LINE_LIKE)?;
                c.expect(circle, CIRCLE)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let answer = |b: bool| Some(Value::Bool(b));
        match *self {
            PredicateDef::Collinear(a, b, d) => {
                answer(collinear(point(c, a), point(c, b), point(c, d)))
            }
            PredicateDef::Parallel(a, b) => answer(parallel(raw_line(c, a), raw_line(c, b))),
            PredicateDef::Perpendicular(a, b) => {
                answer(perpendicular(raw_line(c, a), raw_line(c, b)))
            }
            PredicateDef::Concurrent(a, b, d) => {
                answer(concurrent(raw_line(c, a), raw_line(c, b), raw_line(c, d)))
            }
            PredicateDef::Concyclic(a, b, d, e) => answer(concyclic(
                point(c, a),
                point(c, b),
                point(c, d),
                point(c, e),
            )),
            PredicateDef::Equal(a, b) => answer(equal(c, a, b)),
            PredicateDef::Congruent(a, b) => congruent(c, a, b).map(Value::Bool),
            PredicateDef::Tangent { line, circle } => answer(tangent(c, line, circle)),
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let wanted = match func {
            "AreCollinear" | "AreConcurrent" => 3,
            "AreConcyclic" => 4,
            "AreParallel" | "ArePerpendicular" | "AreEqual" | "AreCongruent" | "IsTangent" => 2,
            _ => return None,
        };
        let n = args.len();
        if n != wanted {
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
        Some(Ok(match func {
            "AreCollinear" => PredicateDef::Collinear(a, b, d),
            "AreConcurrent" => PredicateDef::Concurrent(a, b, d),
            "AreConcyclic" => PredicateDef::Concyclic(a, b, d, e),
            "AreParallel" => PredicateDef::Parallel(a, b),
            "ArePerpendicular" => PredicateDef::Perpendicular(a, b),
            "AreEqual" => PredicateDef::Equal(a, b),
            "AreCongruent" => PredicateDef::Congruent(a, b),
            _ => PredicateDef::Tangent { line: a, circle: b },
        }))
    }
}
