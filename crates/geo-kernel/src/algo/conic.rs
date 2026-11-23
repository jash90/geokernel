// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircleThreePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIncircle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMidpointSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCenterConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDiameterLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSphereNDPointRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Circles and their parts: circle through three points, circle with a
//! literal or segment as radius, incircle, centre, midpoint of a segment, diameter.

use super::{length, line_value, Group, Parents, CIRCLE, LINE_LIKE, POINT};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

/// Circles and their parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicDef {
    /// `Circle(A, B, C)`: the circle through three points.
    ThroughThree(Id, Id, Id),
    /// `Circle(P, s)`: centre `P`, radius the length of the segment `s`.
    SegmentRadius { center: Id, segment: Id },
    /// `Circle(P, 2)`: centre `P`, radius stored with the circle definition.
    LiteralRadius { center: Id, radius: f64 },
    /// `Incircle(A, B, C)`.
    Incircle(Id, Id, Id),
    /// `Center(k)` and `Midpoint(k)` of a circle.
    Center(Id),
    /// `Center(f)` or `Midpoint(f)` of a boolean function of one variable.
    IntervalCenter(Id),
    /// `Midpoint(s)` of a segment.
    MidpointSegment(Id),
    /// `Diameter(l, k)`: the diameter of the circle `k` conjugate to the
    /// direction of the line or segment `l`.
    Diameter { line: Id, circle: Id },
}

const CIRCLE_OR_SEGMENT: &[Kind] = &[Kind::Circle, Kind::Segment];

/// GeoGebra's `MyMath.length(x, y)`, used where GeoGebra measures the
/// distance of two `GeoPoint`s: it scales by the larger component.
pub(super) fn scaled_length(x: f64, y: f64) -> f64 {
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

pub(super) fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// A circle radius as GeoGebra stores it: below the tolerance it is zero.
fn snap_radius(r: f64) -> f64 {
    if is_zero(r) {
        0.0
    } else {
        r
    }
}

/// Circle through three points (`AlgoCircleThreePoints`). A centre at
/// infinity (collinear points, or two equal points and a third) is
/// undefined.
fn through_three(c: &Construction, a: Id, b: Id, cc: Id) -> Option<Value> {
    match circumcircle(c.point(a)?, c.point(b)?, c.point(cc)?) {
        Circumcircle::Circle { cx, cy, r } => Some(Value::Circle { cx, cy, r }),
        Circumcircle::Lines | Circumcircle::Undefined => None,
    }
}

/// What `AlgoCircleThreePoints` makes of three finite points.
pub(crate) enum Circumcircle {
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    /// The centre is at infinity in the direction perpendicular to a pair
    /// of the points (collinear points, or two equal points and a third):
    /// GeoGebra makes the conic two parallel lines (`setCircle(M, P)`).
    Lines,
    /// The two bisectors meet at a point that `GeoPoint` counts as at
    /// infinity: `setCircle(M, r)` makes the conic undefined.
    Undefined,
}

/// `AlgoCircleThreePoints.compute` for three finite points.
pub(crate) fn circumcircle(
    (ax, ay): (f64, f64),
    (bx, by): (f64, f64),
    (cx, cy): (f64, f64),
) -> Circumcircle {
    if is_equal(ax, bx) && is_equal(ay, by) {
        if is_equal(ax, cx) && is_equal(ay, cy) {
            return Circumcircle::Circle {
                cx: ax,
                cy: ay,
                r: 0.0,
            };
        }
        return Circumcircle::Lines;
    } else if (is_equal(ax, cx) && is_equal(ay, cy)) || (is_equal(bx, cx) && is_equal(by, cy)) {
        return Circumcircle::Lines;
    }

    let (abx, aby) = (bx - ax, by - ay);
    let (acx, acy) = (cx - ax, cy - ay);
    let (bcx, bcy) = (cx - bx, cy - by);

    let len_ab = scaled_length(abx, aby);
    let len_ac = scaled_length(acx, acy);
    let len_bc = scaled_length(bcx, bcy);

    // The two bisectors with the largest intersection angle.
    let det0 = (abx * acy - aby * acx).abs() / (len_ab * len_ac);
    let det1 = (acx * bcy - acy * bcx).abs() / (len_ac * len_bc);
    let det2 = (abx * bcy - aby * bcx).abs() / (len_ab * len_bc);

    let mut max_det = det0;
    let mut case = 0;
    if det1 > max_det {
        case = 1;
        max_det = det1;
    }
    if det2 > max_det {
        case = 2;
        max_det = det2;
    }

    if is_zero(max_det) {
        return Circumcircle::Lines;
    }

    let bisector = |dx: f64, dy: f64, px: f64, py: f64, qx: f64, qy: f64| {
        (dx, dy, -((px + qx) * dx + (py + qy) * dy) / 2.0)
    };
    let (s0, s1) = match case {
        1 => (
            bisector(bcx, bcy, bx, by, cx, cy),
            bisector(acx, acy, ax, ay, cx, cy),
        ),
        2 => (
            bisector(abx, aby, ax, ay, bx, by),
            bisector(bcx, bcy, bx, by, cx, cy),
        ),
        _ => (
            bisector(abx, aby, ax, ay, bx, by),
            bisector(acx, acy, ax, ay, cx, cy),
        ),
    };

    // Intersection of the bisectors, with the point's own test for a point
    // at infinity (`GeoPoint.updateCoords`).
    let x = s0.1 * s1.2 - s0.2 * s1.1;
    let y = s0.2 * s1.0 - s0.0 * s1.2;
    let z = s0.0 * s1.1 - s0.1 * s1.0;
    if z.is_nan() || is_epsilon(z, x, y) {
        return Circumcircle::Undefined;
    }
    let (mx, my) = if z == 1.0 { (x, y) } else { (x / z, y / z) };

    let r = snap_radius(scaled_length(ax - mx, ay - my));
    Circumcircle::Circle { cx: mx, cy: my, r }
}

/// `DoubleUtil.isEpsilon(e, x, y)`: whether `e` is negligible against
/// the tolerance and against both `x` and `y`.
pub(super) fn is_epsilon(e: f64, x: f64, y: f64) -> bool {
    let e = e.abs();
    !(e > STANDARD_PRECISION
        || e > x.abs() * STANDARD_PRECISION
        || e > y.abs() * STANDARD_PRECISION)
}

/// Incircle of the triangle `ABC` (`AlgoIncircle`).
fn incircle(c: &Construction, a: Id, b: Id, cc: Id) -> Option<Value> {
    let (ax, ay) = c.point(a)?;
    let (bx, by) = c.point(b)?;
    let (cx, cy) = c.point(cc)?;

    let d_ab = length(bx - ax, by - ay);
    let d_ac = length(cx - ax, cy - ay);
    let d_bc = length(cx - bx, cy - by);
    let s = (d_ab + d_ac + d_bc) / 2.0;
    let w_a = d_bc / s / 2.0;
    let w_b = d_ac / s / 2.0;
    let w_c = d_ab / s / 2.0;

    // Barycentric combination of the homogeneous coordinates (x, y, 1).
    let hx = ax * w_a + w_b * bx + w_c * cx;
    let hy = ay * w_a + w_b * by + w_c * cy;
    let hz = 1.0 * w_a + w_b * 1.0 + w_c * 1.0;
    if hz.is_nan() || hx.is_nan() || hy.is_nan() {
        return None;
    }
    let (mx, my) = if hz == 1.0 {
        (hx, hy)
    } else {
        (hx / hz, hy / hz)
    };

    let radius = ((s - d_bc) * (s - d_ac) / s * (s - d_ab)).sqrt();
    if radius.is_nan() {
        return None;
    }
    Some(Value::Circle {
        cx: mx,
        cy: my,
        r: snap_radius(radius),
    })
}

impl Group for ConicDef {
    fn kind(&self) -> Kind {
        match *self {
            ConicDef::ThroughThree(..)
            | ConicDef::SegmentRadius { .. }
            | ConicDef::LiteralRadius { .. }
            | ConicDef::Incircle(..) => Kind::Circle,
            ConicDef::Center(_) | ConicDef::MidpointSegment(_) => Kind::Point,
            ConicDef::IntervalCenter(_) => Kind::Number,
            ConicDef::Diameter { .. } => Kind::Line,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            ConicDef::ThroughThree(a, b, c) | ConicDef::Incircle(a, b, c) => {
                Parents::of(&[a, b, c])
            }
            ConicDef::SegmentRadius { center, segment } => Parents::of(&[center, segment]),
            ConicDef::LiteralRadius { center, .. } => Parents::of(&[center]),
            ConicDef::Center(k) | ConicDef::MidpointSegment(k) | ConicDef::IntervalCenter(k) => {
                Parents::of(&[k])
            }
            ConicDef::Diameter { line, circle } => Parents::of(&[line, circle]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            ConicDef::ThroughThree(a, b, cc) | ConicDef::Incircle(a, b, cc) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(cc, POINT)
            }
            ConicDef::SegmentRadius { center, segment } => {
                c.expect(center, POINT)?;
                c.expect(segment, &[Kind::Segment])
            }
            ConicDef::LiteralRadius { center, .. } => c.expect(center, POINT),
            ConicDef::Center(k) => c.expect(k, CIRCLE),
            ConicDef::IntervalCenter(f) => c.expect(f, &[Kind::Function]),
            ConicDef::MidpointSegment(s) => c.expect(s, &[Kind::Segment]),
            ConicDef::Diameter { line, circle } => {
                c.expect(line, LINE_LIKE)?;
                c.expect(circle, CIRCLE)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            ConicDef::ThroughThree(a, b, cc) => through_three(c, a, b, cc),
            ConicDef::SegmentRadius { center, segment } => {
                let (cx, cy) = c.point(center)?;
                match c.value(segment) {
                    Value::Segment { x1, y1, x2, y2 } => Some(Value::Circle {
                        cx,
                        cy,
                        r: snap_radius(scaled_length(x2 - x1, y2 - y1)),
                    }),
                    _ => None,
                }
            }
            ConicDef::LiteralRadius { center, radius } => {
                super::circle::with_radius_value(c, center, radius)
            }
            ConicDef::Incircle(a, b, cc) => incircle(c, a, b, cc),
            ConicDef::Center(k) => {
                let (cx, cy, _) = c.circle(k)?;
                Some(Value::Point { x: cx, y: cy })
            }
            ConicDef::IntervalCenter(f) => Self::named_interval_midpoint(c, f),
            ConicDef::MidpointSegment(s) => match c.value(s) {
                Value::Segment { x1, y1, x2, y2 } => Some(Value::Point {
                    x: (x1 + x2) / 2.0,
                    y: (y1 + y2) / 2.0,
                }),
                _ => None,
            },
            ConicDef::Diameter { line, circle } => {
                // `GeoLine.getDirection` of the raw coefficients, then
                // `GeoConicND.diameterLine` with the circle matrix
                // (1, 1, cx^2 + cy^2 - r^2, 0, -cx, -cy).
                let (a, b, _) = c.raw_carrier(line)?;
                let (cx, cy, _) = c.circle(circle)?;
                let (vx, vy) = (b, -a);
                line_value(vx + 0.0 * vy, 0.0 * vx + vy, -cx * vx + -cy * vy)
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let n = args.len();
        let count_error = || Some(Err(format!("`{func}` does not take {n} argument(s)")));
        match func {
            "Circle" => match n {
                3 => Some(three_ids(c, args).map(|(a, b, d)| ConicDef::ThroughThree(a, b, d))),
                2 => {
                    if let Some(second) = c.id(args[1]) {
                        // Named numbers and points are handled by the built-in command.
                        return (c.kind(second) == Kind::Segment).then(|| {
                            object(c, args[0]).map(|center| ConicDef::SegmentRadius {
                                center,
                                segment: second,
                            })
                        });
                    }
                    let radius = crate::expr::plain_number(args[1])?;
                    Some(
                        object(c, args[0]).map(|center| ConicDef::LiteralRadius { center, radius }),
                    )
                }
                _ => None,
            },
            "Incircle" => match n {
                3 => Some(three_ids(c, args).map(|(a, b, d)| ConicDef::Incircle(a, b, d))),
                _ => count_error(),
            },
            "Center" | "Midpoint" => match n {
                1 => Some(object(c, args[0]).and_then(|id| {
                    match c.kind(id) {
                        Kind::Circle => Ok(ConicDef::Center(id)),
                        Kind::Segment => Ok(ConicDef::MidpointSegment(id)),
                        Kind::Function
                            if crate::expr::function_signature(c, id)
                                .is_some_and(|signature| signature.boolean) =>
                        {
                            Ok(ConicDef::IntervalCenter(id))
                        }
                        found => Err(Error::WrongKind {
                            label: args[0].to_string(),
                            expected: CIRCLE_OR_SEGMENT,
                            found,
                        }
                        .to_string()),
                    }
                })),
                2 => None,
                _ if func == "Midpoint" => None,
                _ => count_error(),
            },
            // `ConjugateDiameter` is GeoGebra's alias of `Diameter`.
            "Diameter" | "ConjugateDiameter" => match n {
                2 => Some(object(c, args[0]).and_then(|line| {
                    let circle = object(c, args[1])?;
                    Ok(ConicDef::Diameter { line, circle })
                })),
                _ => count_error(),
            },
            _ => None,
        }
    }
}

fn three_ids(c: &Construction, args: &[&str]) -> Result<(Id, Id, Id), String> {
    Ok((
        object(c, args[0])?,
        object(c, args[1])?,
        object(c, args[2])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::load;

    fn value(c: &Construction, label: &str) -> Value {
        c.value(c.id(label).unwrap())
    }

    fn err(src: &str) -> String {
        load(src).err().expect("script should fail").message
    }

    const TRI: &str = "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\n";

    #[test]
    fn parses_every_syntax() {
        let c = load(&format!(
            "{TRI}k = Circle(A, B, C)\ni1 = Incircle(A, B, C)\nO = Center(k)\nM = Midpoint(k)\n\
             s = Segment(A, B)\nm = Midpoint(s)\nn = Center(s)\nj = Circle(C, s)\n\
             f = Line(A, B)\nd = Diameter(f, k)\ne = Diameter(s, k)\n"
        ))
        .unwrap();
        let def = |l: &str| c.def(c.id(l).unwrap());
        use crate::kernel::Def;
        assert!(matches!(def("k"), Def::Conic(ConicDef::ThroughThree(..))));
        assert!(matches!(def("i1"), Def::Conic(ConicDef::Incircle(..))));
        assert!(matches!(def("O"), Def::Conic(ConicDef::Center(_))));
        assert!(matches!(def("M"), Def::Conic(ConicDef::Center(_))));
        assert!(matches!(def("m"), Def::Conic(ConicDef::MidpointSegment(_))));
        assert!(matches!(def("n"), Def::Conic(ConicDef::MidpointSegment(_))));
        assert!(matches!(
            def("j"),
            Def::Conic(ConicDef::SegmentRadius { .. })
        ));
        assert!(matches!(def("d"), Def::Conic(ConicDef::Diameter { .. })));
    }

    #[test]
    fn conjugate_diameter_is_diameter() {
        let c = load(&format!(
            "{TRI}k = Circle(A, B, C)\nf = Line(A, C)\nd = Diameter(f, k)\n\
             e = ConjugateDiameter(f, k)\n"
        ))
        .unwrap();
        let (d, e) = (c.id("d").unwrap(), c.id("e").unwrap());
        assert_eq!(c.def(d), c.def(e));
        // f is (-3, 0, 0); the diameter through the centre (2, 1.5) has the
        // direction (0, 3) of f as its normal.
        assert_eq!(
            c.value(e),
            Value::Line {
                a: 0.0,
                b: 3.0,
                c: -4.5
            }
        );
        assert!(err(&format!("{TRI}k = ConjugateDiameter(A)\n")).contains("does not take 1"));
    }

    #[test]
    fn built_in_shapes_are_left_alone() {
        let c = load(&format!(
            "{TRI}r = 2\nk = Circle(A, r)\nm = Circle(A, B)\nM = Midpoint(A, B)\n"
        ))
        .unwrap();
        use crate::kernel::Def;
        let def = |l: &str| c.def(c.id(l).unwrap());
        assert!(matches!(def("k"), Def::CircleRadius { .. }));
        assert!(matches!(def("m"), Def::CircleThrough { .. }));
        assert!(matches!(def("M"), Def::Midpoint(..)));
    }

    #[test]
    fn computes_the_circumcircle_and_incircle_of_a_345_triangle() {
        let c = load(&format!(
            "{TRI}k = Circle(A, B, C)\ni1 = Incircle(A, B, C)\nO = Center(k)\n"
        ))
        .unwrap();
        let Value::Circle { cx, cy, r } = value(&c, "k") else {
            panic!("not a circle");
        };
        assert!((cx - 2.0).abs() < 1e-12 && (cy - 1.5).abs() < 1e-12 && (r - 2.5).abs() < 1e-12);
        let Value::Circle { cx, cy, r } = value(&c, "i1") else {
            panic!("not a circle");
        };
        assert!((cx - 1.0).abs() < 1e-12 && (cy - 1.0).abs() < 1e-12 && (r - 1.0).abs() < 1e-12);
        assert_eq!(value(&c, "O"), Value::Point { x: 2.0, y: 1.5 });
    }

    #[test]
    fn degenerate_triples() {
        let mut c = load(&format!("{TRI}k = Circle(A, B, C)\n")).unwrap();
        let (a, b, cc) = (c.id("A").unwrap(), c.id("B").unwrap(), c.id("C").unwrap());
        let k = c.id("k").unwrap();
        c.move_point(cc, 2.0, 0.0).unwrap();
        assert_eq!(c.value(k), Value::Undefined);
        c.move_point(b, 0.0, 0.0).unwrap();
        assert_eq!(c.value(k), Value::Undefined);
        c.move_point(cc, 0.0, 0.0).unwrap();
        assert_eq!(
            c.value(k),
            Value::Circle {
                cx: 0.0,
                cy: 0.0,
                r: 0.0
            }
        );
        c.move_point(a, 1.0, 1.0).unwrap();
        assert_eq!(c.value(k), Value::Undefined);
    }

    #[test]
    fn rejects_wrong_arguments() {
        assert!(err(&format!("{TRI}r = 2\nk = Circle(A, B, r)\n")).contains("is a number"));
        assert!(err(&format!("{TRI}k = Incircle(A, B)\n")).contains("does not take 2"));
        assert!(err(&format!("{TRI}k = Center(A, B, C)\n")).contains("does not take 3"));
        assert!(err(&format!("{TRI}k = Center(A)\n")).contains("is a point"));
        assert!(err(&format!("{TRI}k = Midpoint(A)\n")).contains("is a point"));
        assert!(err(&format!("{TRI}k = Diameter(A)\n")).contains("does not take 1"));
        assert!(err(&format!("{TRI}k = Incircle(A, B, Z)\n")).contains("unknown object `Z`"));
        assert!(err(&format!("{TRI}k = Circle(A, B, C, A)\n")).contains("does not take 4"));
    }
}
