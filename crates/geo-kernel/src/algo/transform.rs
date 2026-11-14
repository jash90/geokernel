// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdMirror.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRotate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdDilate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdShear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdStretch.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMirror.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRotate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRotatePoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDilate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoShearOrStretch.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Transform.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Images of points, lines, segments and circles under reflection in a
//! point or a line, inversion of a point in a circle, rotation and dilation,
//! and of points, lines and segments under shear and stretch.
//!
//! GeoGebra transforms a line by transforming its raw coefficients, and a
//! circle by transforming its centre and scaling its radius. A segment whose
//! end points have labels is transformed through its end points, so the
//! image keeps the order of the end points. The formulas below follow
//! `GeoPoint`, `GeoLine` and `GeoConic`, including the sign flips of the
//! line coefficients, so that an image line has the same orientation.
//!
//! Not covered: inversion of lines, segments and circles in a circle (the
//! image is a circle, a line or a conic depending on the position) and the
//! forms that need a vector (`Translate`, `Shear` and `Stretch` along a
//! vector), and shear and stretch of a circle (the image is an ellipse).

use super::{Group, Parents, CIRCLE, LINE, LINE_LIKE, NUMBER, POINT};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

const SEGMENT: &[Kind] = &[Kind::Segment];
/// Kinds of object GeoGebra can transform, restricted to those supported here.
const TRANSFORMABLE: &[Kind] = &[Kind::Point, Kind::Line, Kind::Segment, Kind::Circle];
/// Kinds that stay of the same kind under shear and stretch.
const AFFINE_ONLY: &[Kind] = &[Kind::Point, Kind::Line, Kind::Segment];

fn of_kind(kind: Kind) -> &'static [Kind] {
    match kind {
        Kind::Point => POINT,
        Kind::Line => LINE,
        Kind::Segment => SEGMENT,
        Kind::Circle => CIRCLE,
        Kind::Number => NUMBER,
    }
}

/// Images of objects under reflection, rotation and dilation. `kind` is the
/// kind of `src` and so of the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformDef {
    /// `Mirror(object, point)`.
    MirrorPoint { kind: Kind, src: Id, center: Id },
    /// `Mirror(object, line or segment)`.
    MirrorLine { kind: Kind, src: Id, mirror: Id },
    /// `Mirror(point, circle)`: inversion.
    Invert { src: Id, circle: Id },
    /// `Rotate(object, angle)` and `Rotate(object, angle, point)`.
    Rotate {
        kind: Kind,
        src: Id,
        angle: Id,
        center: Option<Id>,
    },
    /// `Dilate(object, factor)` and `Dilate(object, factor, point)`.
    Dilate {
        kind: Kind,
        src: Id,
        factor: Id,
        center: Option<Id>,
    },
    /// `Shear(object, line, ratio)` and `Stretch(object, line, ratio)`; the
    /// object is a point, a line or a segment and `axis` a line or a segment.
    ShearStretch {
        kind: Kind,
        src: Id,
        axis: Id,
        ratio: Id,
        shear: bool,
    },
}

/// A map of the plane that moves points, lines and circle centres.
#[derive(Clone, Copy)]
enum Map {
    /// Reflection in the point `(qx, qy)`.
    Point { qx: f64, qy: f64 },
    /// Reflection in the line `a*x + b*y + c = 0`.
    Line { a: f64, b: f64, c: f64 },
    /// Rotation by `phi` about `(qx, qy)`.
    Rotate { phi: f64, qx: f64, qy: f64 },
    /// Dilation by `r` from `(sx, sy)`.
    Dilate { r: f64, sx: f64, sy: f64 },
    /// The matrix `[[m0, m1], [m2, m3]]` applied after translating by
    /// `(qx, qy)` and followed by the translation back (shear and stretch).
    Matrix { qx: f64, qy: f64, m: [f64; 4] },
}

/// The point `(x, y)` on the mirror line that GeoGebra translates by.
fn mirror_origin(a: f64, b: f64, c: f64) -> (f64, f64) {
    if a.abs() > b.abs() {
        (-c / a, 0.0)
    } else {
        (0.0, -c / b)
    }
}

/// `GeoVec2D.mirror(phi)`: the mirror transform with angle `phi`.
fn mirror_xy(x: f64, y: f64, phi: f64) -> (f64, f64) {
    let (sin, cos) = phi.sin_cos();
    (x * cos + y * sin, x * sin - y * cos)
}

impl Map {
    /// Image of a point, or of the centre of a circle.
    fn point(&self, x: f64, y: f64) -> (f64, f64) {
        match *self {
            Map::Point { qx, qy } => (2.0 * qx - x, 2.0 * qy - y),
            Map::Line { a, b, c } => {
                let (qx, qy) = mirror_origin(a, b, c);
                let (x, y) = mirror_xy(x - qx, y - qy, 2.0 * (-a).atan2(b));
                (x + qx, y + qy)
            }
            Map::Rotate { phi, qx, qy } => {
                // `MyMath.cos` snaps a cosine of magnitude below 1e-16 to 0.
                let cos = phi.cos();
                let cos = if cos.abs() < 1e-16 { 0.0 } else { cos };
                let sin = phi.sin();
                (
                    (x - qx) * cos + (qy - y) * sin + qx,
                    (x - qx) * sin + (y - qy) * cos + qy,
                )
            }
            Map::Dilate { r, sx, sy } => {
                let temp = 1.0 - r;
                (r * x + temp * sx, r * y + temp * sy)
            }
            Map::Matrix { qx, qy, m } => {
                let (x, y) = (x + qx, y + qy);
                (m[0] * x + m[1] * y - qx, m[2] * x + m[3] * y - qy)
            }
        }
    }

    /// Image of the raw line `a*x + b*y + c = 0`, as GeoGebra computes it.
    fn line(&self, a: f64, b: f64, c: f64) -> (f64, f64, f64) {
        match *self {
            Map::Point { qx, qy } => (-a, -b, c + 2.0 * (a * qx + b * qy)),
            Map::Line {
                a: ga,
                b: gb,
                c: gc,
            } => {
                let (qx, qy) = mirror_origin(ga, gb, gc);
                let phi = 2.0 * (-ga).atan2(gb);
                let (sin, cos) = phi.sin_cos();
                let x0 = a * cos + b * sin;
                let y0 = a * sin - b * cos;
                let aqx = a * qx;
                let bqy = b * qy;
                let z = c + aqx + bqy + (bqy - aqx) * cos - (a * qy + b * qx) * sin;
                // The orientation of the line changes.
                (-x0, -y0, -z)
            }
            Map::Rotate { phi, qx, qy } => {
                let (sin, cos) = phi.sin_cos();
                let x0 = a * cos - b * sin;
                let y0 = a * sin + b * cos;
                let z = c + (a * qx + b * qy) * (1.0 - cos) + (b * qx - a * qy) * sin;
                (x0, y0, z)
            }
            Map::Dilate { r, sx, sy } => {
                let z = (r - 1.0) * (a * sx + b * sy) + r * c;
                (a * r, b * r, z * r)
            }
            Map::Matrix { qx, qy, m } => {
                // `GeoLine.matrixTransform` on the translated line.
                let z = c - (a * qx + b * qy);
                let (p, q, r, s) = (m[0], m[1], m[2], m[3]);
                let (x1, y1, z1) = if b.abs() < STANDARD_PRECISION {
                    (s * a, -q * a, -q * r * z + s * p * z)
                } else {
                    let x1 = r * b - s * a;
                    let y1 = q * a - p * b;
                    (x1 * b, y1 * b, q * z * x1 + s * z * y1)
                };
                (x1, y1, z1 + (x1 * qx + y1 * qy))
            }
        }
    }

    /// Radius of the image of a circle; `None` when the circle is undefined.
    fn radius(&self, r: f64) -> Option<f64> {
        // A circle of radius below the standard precision is a single point
        // in GeoGebra and has radius 0.
        let r = if r < STANDARD_PRECISION { 0.0 } else { r };
        match *self {
            Map::Dilate { r: factor, .. } => {
                if factor == 0.0 {
                    None
                } else {
                    Some(factor.abs() * r)
                }
            }
            Map::Matrix { .. } => None,
            _ => Some(r),
        }
    }

    fn value(&self, src: Value) -> Option<Value> {
        match src {
            Value::Point { x, y } => {
                let (x, y) = self.point(x, y);
                Some(Value::Point { x, y })
            }
            Value::Segment { x1, y1, x2, y2 } => {
                let (x1, y1) = self.point(x1, y1);
                let (x2, y2) = self.point(x2, y2);
                Some(Value::Segment { x1, y1, x2, y2 })
            }
            Value::Line { a, b, c } => {
                let (a, b, c) = self.line(a, b, c);
                // GeoGebra's `GeoLine.isDefined`: both coefficients zero.
                if a.abs() < STANDARD_PRECISION && b.abs() < STANDARD_PRECISION {
                    return None;
                }
                let len = (a * a + b * b).sqrt();
                Some(Value::Line {
                    a: a / len,
                    b: b / len,
                    c: c / len,
                })
            }
            Value::Circle { cx, cy, r } => {
                let r = self.radius(r)?;
                let (cx, cy) = self.point(cx, cy);
                Some(Value::Circle { cx, cy, r })
            }
            Value::Number(_) | Value::Undefined => None,
        }
    }
}

/// Inversion of a point in a circle (`GeoPointND.mirror(GeoConicND)`).
fn invert(p: (f64, f64), circle: (f64, f64, f64)) -> (f64, f64) {
    let (cx, cy, r) = circle;
    // A circle of radius 0 is a single point and leaves the point alone.
    if r < STANDARD_PRECISION {
        return p;
    }
    let (dx, dy) = (p.0 - cx, p.1 - cy);
    let scale = r * r / (dx * dx + dy * dy);
    (cx + scale * dx, cy + scale * dy)
}

impl TransformDef {
    fn center(c: &Construction, center: Option<Id>) -> Option<(f64, f64)> {
        match center {
            None => Some((0.0, 0.0)),
            Some(id) => c.point(id),
        }
    }
}

impl Group for TransformDef {
    fn kind(&self) -> Kind {
        match *self {
            TransformDef::MirrorPoint { kind, .. }
            | TransformDef::MirrorLine { kind, .. }
            | TransformDef::Rotate { kind, .. }
            | TransformDef::Dilate { kind, .. }
            | TransformDef::ShearStretch { kind, .. } => kind,
            TransformDef::Invert { .. } => Kind::Point,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            TransformDef::MirrorPoint { src, center, .. } => Parents::of(&[src, center]),
            TransformDef::MirrorLine { src, mirror, .. } => Parents::of(&[src, mirror]),
            TransformDef::Invert { src, circle } => Parents::of(&[src, circle]),
            TransformDef::ShearStretch {
                src, axis, ratio, ..
            } => Parents::of(&[src, axis, ratio]),
            TransformDef::Rotate {
                src,
                angle: second,
                center,
                ..
            }
            | TransformDef::Dilate {
                src,
                factor: second,
                center,
                ..
            } => match center {
                Some(q) => Parents::of(&[src, second, q]),
                None => Parents::of(&[src, second]),
            },
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            TransformDef::MirrorPoint { kind, src, center } => {
                c.expect(src, of_kind(kind))?;
                c.expect(src, TRANSFORMABLE)?;
                c.expect(center, POINT)
            }
            TransformDef::MirrorLine { kind, src, mirror } => {
                c.expect(src, of_kind(kind))?;
                c.expect(src, TRANSFORMABLE)?;
                c.expect(mirror, LINE_LIKE)
            }
            TransformDef::Invert { src, circle } => {
                c.expect(src, POINT)?;
                c.expect(circle, CIRCLE)
            }
            TransformDef::ShearStretch {
                kind,
                src,
                axis,
                ratio,
                ..
            } => {
                c.expect(src, of_kind(kind))?;
                c.expect(src, AFFINE_ONLY)?;
                c.expect(axis, LINE_LIKE)?;
                c.expect(ratio, NUMBER)
            }
            TransformDef::Rotate {
                kind,
                src,
                angle: second,
                center,
            }
            | TransformDef::Dilate {
                kind,
                src,
                factor: second,
                center,
            } => {
                c.expect(src, of_kind(kind))?;
                c.expect(src, TRANSFORMABLE)?;
                c.expect(second, NUMBER)?;
                match center {
                    Some(q) => c.expect(q, POINT),
                    None => Ok(()),
                }
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let (src, map) = match *self {
            TransformDef::MirrorPoint { src, center, .. } => {
                let (qx, qy) = c.point(center)?;
                (src, Map::Point { qx, qy })
            }
            TransformDef::MirrorLine { src, mirror, .. } => {
                let (a, b, c) = c.carrier(mirror)?;
                (src, Map::Line { a, b, c })
            }
            TransformDef::ShearStretch {
                src,
                axis,
                ratio,
                shear,
                ..
            } => {
                let (la, lb, lc) = c.carrier(axis)?;
                let n = c.number(ratio)?;
                // `AlgoShearOrStretch.compute`: a point on the axis moves to
                // the origin, then the matrix of the shear or stretch is applied.
                let (qx, qy) = if la.abs() > lb.abs() {
                    (lc / la, 0.0)
                } else {
                    (0.0, lc / lb)
                };
                let len = (la * la + lb * lb).sqrt();
                let (s, cos) = (-la / len, lb / len);
                let m = if shear {
                    [
                        1.0 - cos * s * n,
                        cos * cos * n,
                        -s * s * n,
                        1.0 + s * cos * n,
                    ]
                } else {
                    [
                        cos * cos + s * s * n,
                        cos * s * (1.0 - n),
                        cos * s * (1.0 - n),
                        s * s + cos * cos * n,
                    ]
                };
                (src, Map::Matrix { qx, qy, m })
            }
            TransformDef::Invert { src, circle } => {
                let p = c.point(src)?;
                let (x, y) = invert(p, c.circle(circle)?);
                return Some(Value::Point { x, y });
            }
            TransformDef::Rotate {
                src, angle, center, ..
            } => {
                let (qx, qy) = Self::center(c, center)?;
                let phi = c.number(angle)?;
                (src, Map::Rotate { phi, qx, qy })
            }
            TransformDef::Dilate {
                src,
                factor,
                center,
                ..
            } => {
                let (sx, sy) = Self::center(c, center)?;
                let r = c.number(factor)?;
                (src, Map::Dilate { r, sx, sy })
            }
        };
        map.value(c.value(src))
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        match func {
            "Mirror" | "Reflect" | "Rotate" | "Dilate" | "Shear" | "Stretch" => {}
            _ => return None,
        }
        Some(parse_transform(c, func, args))
    }
}

fn parse_transform(c: &Construction, func: &str, args: &[&str]) -> Result<TransformDef, String> {
    if func == "Stretch" && args.len() == 2 {
        return Err("`Stretch` along a vector is not supported (there are no vectors)".into());
    }
    let arity_ok = match func {
        "Mirror" | "Reflect" => args.len() == 2,
        "Shear" | "Stretch" => args.len() == 3,
        _ => args.len() == 2 || args.len() == 3,
    };
    if !arity_ok {
        return Err(format!("`{func}` does not take {} argument(s)", args.len()));
    }
    let src = object(c, args[0])?;
    let kind = c.kind(src);
    if !TRANSFORMABLE.contains(&kind) {
        return Err(format!(
            "`{}` is a {kind}, which `{func}` does not transform",
            args[0]
        ));
    }
    let second = object(c, args[1])?;
    let second_kind = c.kind(second);

    if matches!(func, "Shear" | "Stretch") {
        if kind == Kind::Circle {
            return Err(format!(
                "`{func}` of a circle is not supported (the image is an ellipse)"
            ));
        }
        if !matches!(second_kind, Kind::Line | Kind::Segment) {
            return Err(format!("`{}` is a {second_kind}, expected a line", args[1]));
        }
        let ratio = object(c, args[2])?;
        if c.kind(ratio) != Kind::Number {
            return Err(format!(
                "`{}` is a {}, expected a number",
                args[2],
                c.kind(ratio)
            ));
        }
        return Ok(TransformDef::ShearStretch {
            kind,
            src,
            axis: second,
            ratio,
            shear: func == "Shear",
        });
    }

    if matches!(func, "Mirror" | "Reflect") {
        return match second_kind {
            Kind::Point => Ok(TransformDef::MirrorPoint {
                kind,
                src,
                center: second,
            }),
            Kind::Line | Kind::Segment => Ok(TransformDef::MirrorLine {
                kind,
                src,
                mirror: second,
            }),
            Kind::Circle if kind == Kind::Point => Ok(TransformDef::Invert {
                src,
                circle: second,
            }),
            Kind::Circle => Err(format!(
                "inversion of a {kind} in a circle is not supported (only a point can be inverted)"
            )),
            Kind::Number => Err(format!("`{}` is a number, expected a mirror", args[1])),
        };
    }

    if second_kind != Kind::Number {
        return Err(format!(
            "`{}` is a {second_kind}, expected a number",
            args[1]
        ));
    }
    let center = match args.get(2) {
        None => None,
        Some(label) => {
            let q = object(c, label)?;
            if c.kind(q) != Kind::Point {
                return Err(format!("`{label}` is a {}, expected a point", c.kind(q)));
            }
            Some(q)
        }
    };
    Ok(if func == "Rotate" {
        TransformDef::Rotate {
            kind,
            src,
            angle: second,
            center,
        }
    } else {
        TransformDef::Dilate {
            kind,
            src,
            factor: second,
            center,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Def;
    use crate::script::{exec, load};

    fn value(c: &Construction, label: &str) -> Value {
        c.value(c.id(label).unwrap())
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn parses_every_syntax() {
        let c = load(
            "A = (1, 0)\nB = (3, 1)\nQ = (0, 1)\nw = 0.5\nf = 2\n\
             l = Line(A, B)\ns = Segment(A, B)\nk = Circle(A, B)\n\
             p1 = Mirror(A, Q)\np2 = Reflect(l, Q)\np3 = Mirror(s, l)\np4 = Mirror(k, s)\n\
             p5 = Mirror(A, k)\np6 = Rotate(l, w)\np7 = Rotate(s, w, Q)\n\
             p8 = Dilate(k, f)\np9 = Dilate(A, f, Q)\nq1 = Shear(l, s, f)\nq2 = Stretch(A, l, w)\n",
        )
        .unwrap();
        let kind = |l: &str| c.kind(c.id(l).unwrap());
        assert_eq!(kind("p1"), Kind::Point);
        assert_eq!(kind("p2"), Kind::Line);
        assert_eq!(kind("p3"), Kind::Segment);
        assert_eq!(kind("p4"), Kind::Circle);
        assert_eq!(kind("p5"), Kind::Point);
        assert_eq!(kind("p8"), Kind::Circle);
        assert_eq!(kind("q1"), Kind::Line);
        assert_eq!(kind("q2"), Kind::Point);
        assert!(matches!(
            c.def(c.id("p5").unwrap()),
            Def::Transform(TransformDef::Invert { .. })
        ));
        assert!(matches!(
            c.def(c.id("p6").unwrap()),
            Def::Transform(TransformDef::Rotate { center: None, .. })
        ));
        assert!(matches!(
            c.def(c.id("p9").unwrap()),
            Def::Transform(TransformDef::Dilate {
                center: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn rejects_wrong_arguments() {
        let mut c =
            load("A = (1, 0)\nB = (3, 1)\nw = 0.5\nl = Line(A, B)\nk = Circle(A, B)\n").unwrap();
        let mut err = |line: &str| exec(&mut c, line).unwrap_err();
        assert!(err("m = Mirror(A)").contains("`Mirror` does not take 1 argument(s)"));
        assert!(err("m = Reflect(A, B, l)").contains("does not take 3"));
        assert!(err("m = Rotate(A)").contains("`Rotate` does not take 1"));
        assert!(err("m = Dilate(A, w, B, B)").contains("`Dilate` does not take 4"));
        assert!(err("m = Rotate(A, B)").contains("expected a number"));
        assert!(err("m = Rotate(A, w, l)").contains("expected a point"));
        assert!(err("m = Mirror(w, A)").contains("does not transform"));
        assert!(err("m = Mirror(A, w)").contains("expected a mirror"));
        assert!(err("m = Mirror(l, k)").contains("not supported"));
        assert!(err("m = Dilate(Z, w)").contains("unknown object"));
        assert!(err("m = Shear(A, l)").contains("`Shear` does not take 2"));
        assert!(err("m = Stretch(A, l)").contains("not supported"));
        assert!(err("m = Shear(k, l, w)").contains("ellipse"));
        assert!(err("m = Stretch(A, w, w)").contains("expected a line"));
        assert!(err("m = Stretch(A, l, B)").contains("expected a number"));
    }

    #[test]
    fn point_images() {
        let c = load(
            "A = (3, 1)\nQ = (1, 1)\nO = (0, 0)\nB = (0, 4)\nl = Line(O, B)\n\
             w = 1.5707963267948966\nf = -2\n\
             m = Mirror(A, Q)\nn = Mirror(A, l)\nr = Rotate(A, w, Q)\nd = Dilate(A, f, Q)\n",
        )
        .unwrap();
        let at = |l: &str| match value(&c, l) {
            Value::Point { x, y } => (x, y),
            v => panic!("{v:?}"),
        };
        assert_eq!(at("m"), (-1.0, 1.0));
        assert!(close(at("n").0, -3.0) && close(at("n").1, 1.0));
        assert!(close(at("r").0, 1.0) && close(at("r").1, 3.0));
        assert_eq!(at("d"), (-3.0, 1.0));
    }

    #[test]
    fn inversion_cases() {
        let mut c =
            load("O = (1, 2)\nR = (4, 2)\nk = Circle(O, R)\nP = (3, 3)\nm = Mirror(P, k)\n")
                .unwrap();
        assert_eq!(value(&c, "m"), Value::Point { x: 4.6, y: 3.8 });
        let p = c.id("P").unwrap();
        c.move_point(p, 1.0, 2.0).unwrap();
        assert_eq!(value(&c, "m"), Value::Undefined);
        // A circle of radius 0 leaves the point where it is.
        let r = c.id("R").unwrap();
        c.move_point(p, 3.0, 3.0).unwrap();
        c.move_point(r, 1.0, 2.0).unwrap();
        assert_eq!(value(&c, "m"), Value::Point { x: 3.0, y: 3.0 });
    }

    #[test]
    fn line_images_keep_geogebra_orientation() {
        let c = load(
            "A = (0, 0)\nB = (4, 0)\nQ = (0, 1)\nf = -1\nl = Line(A, B)\n\
             m = Mirror(l, Q)\nd = Dilate(l, f)\nn = Mirror(l, l)\n",
        )
        .unwrap();
        // l is y = 0 with normal (0, 1); the mirror in a point flips it.
        assert_eq!(
            value(&c, "m"),
            Value::Line {
                a: 0.0,
                b: -1.0,
                c: 2.0
            }
        );
        assert_eq!(
            value(&c, "d"),
            Value::Line {
                a: -0.0,
                b: -1.0,
                c: -0.0
            }
        );
        // Mirroring a line in itself keeps the line and its orientation.
        let Value::Line { a, b, c: z } = value(&c, "n") else {
            panic!()
        };
        assert!(close(a, 0.0) && close(b, 1.0) && close(z, 0.0));
    }

    #[test]
    fn dilation_by_zero() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 1)\nl = Line(A, B)\ns = Segment(A, B)\nk = Circle(A, B)\n\
             f = 2\nm = Dilate(l, f)\nn = Dilate(s, f)\no = Dilate(k, f)\n",
        )
        .unwrap();
        let f = c.id("f").unwrap();
        c.set_number(f, 0.0).unwrap();
        assert_eq!(value(&c, "m"), Value::Undefined);
        assert_eq!(value(&c, "o"), Value::Undefined);
        assert_eq!(
            value(&c, "n"),
            Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0
            }
        );
        c.set_number(f, -2.0).unwrap();
        assert_eq!(
            value(&c, "o"),
            Value::Circle {
                cx: -0.0,
                cy: -0.0,
                r: 2.0 * 17f64.sqrt()
            }
        );
    }

    #[test]
    fn undefined_parents_give_undefined_images() {
        let mut c =
            load("A = (0, 0)\nB = (4, 1)\nP = (2, 2)\nl = Line(A, B)\nm = Mirror(P, l)\n").unwrap();
        let b = c.id("B").unwrap();
        c.move_point(b, 0.0, 0.0).unwrap();
        assert_eq!(value(&c, "m"), Value::Undefined);
    }
}
