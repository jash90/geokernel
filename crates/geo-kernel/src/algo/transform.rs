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
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec2D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Images of points, lines, segments, rays, vectors, polygons and
//! polylines under reflection in a point or a line, rotation, dilation,
//! shear and stretch, and inversion of a point in a circle. Images of
//! circles and conics keep a conic record with the circle's axes and are
//! made by [`crate::algo::general_conic`]; the inversion of a line is a
//! conic and is made there too, and the parts of conics (arcs and sectors)
//! keep their record in [`crate::algo::arc`].
//!
//! GeoGebra transforms a line by transforming its raw coefficients. A
//! segment whose end points have labels is transformed through its end
//! points, so the image keeps the order of the end points; the same holds
//! for a ray and for the vertices of a polygon, whose image also gets the
//! images of its sides. The formulas below follow `GeoPoint`, `GeoLine`,
//! `GeoVector` and `GeoConic`, including the sign flips of the line
//! coefficients, so that an image line has the same orientation.
//!
//! Not covered: the inversion of a ray in a circle (GeoGebra labels the
//! image after the ray instead of the command) and `Shear` or `Stretch` of
//! a polyline (GeoGebra rejects it too).

use super::vector::VectorDef;
use super::{join, line_value, Group, Parents, CIRCLE, LINE_LIKE, NUMBER, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;

/// Kinds of object this group transforms. Circles are transformed by
/// [`crate::algo::general_conic::GeneralConicDef`], which keeps their axes.
const TRANSFORMABLE: &[Kind] = &[
    Kind::Point,
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::Vector,
    Kind::Polygon,
    Kind::PolyLine,
];
const VECTOR: &[Kind] = &[Kind::Vector];
/// What `Shear` and `Stretch` accept: no polyline (`isMatrixTransformable`).
const SHEARABLE: &[Kind] = &[
    Kind::Point,
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::Vector,
    Kind::Polygon,
];

fn of_kind(kind: Kind) -> &'static [Kind] {
    match kind {
        Kind::Point => POINT,
        Kind::Line => &[Kind::Line],
        Kind::Segment => &[Kind::Segment],
        Kind::Circle => &[Kind::Circle],
        Kind::Number | Kind::Angle => NUMBER,
        Kind::Vector => &[Kind::Vector],
        Kind::Ray => &[Kind::Ray],
        Kind::Boolean => &[Kind::Boolean],
        Kind::Function => &[Kind::Function],
        Kind::Polygon => &[Kind::Polygon],
        Kind::PolyLine => &[Kind::PolyLine],
        Kind::List => &[Kind::List],
        Kind::Conic => &[Kind::Conic],
        Kind::Arc => &[Kind::Arc],
        Kind::Sector => &[Kind::Sector],
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
    /// `Stretch(object, vector)`: stretched along the vector by its length.
    StretchVector { kind: Kind, src: Id, vector: Id },
}

/// The parameters of a transformation, without the transformed object and
/// its kind, which the definitions that hold a [`TransformDef`] inside
/// themselves carry separately (an [`crate::algo::arc::ArcDef`] image keeps
/// its own record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformOf {
    /// Reflection in a point.
    MirrorPoint(Id),
    /// Reflection in a line.
    MirrorLine(Id),
    /// Rotation by `angle` about `center`, or the origin. The centre is
    /// kept as an index so the whole enum stays small enough for `Def`
    /// (see the `expr_sizes` test).
    Rotate { angle: Id, center: Option<u32> },
    /// Dilation by `factor` from `center`, or the origin (as `Rotate`).
    Dilate { factor: Id, center: Option<u32> },
    /// Shear along `axis` by `ratio`.
    Shear { axis: Id, ratio: Id },
    /// Stretch along `axis` by `ratio`.
    Stretch { axis: Id, ratio: Id },
    /// Stretch along a vector by its length.
    StretchVector(Id),
    /// Inversion in a circle.
    Invert(Id),
}

/// The centre a [`TransformOf`] rotation or dilation runs about: kept as an
/// index, `None` meaning the origin.
pub(crate) fn center_of(center: Option<u32>) -> Option<Id> {
    center.map(|index| Id(index as usize))
}

/// A map of the plane that moves points, lines and circle centres.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Map {
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
    pub(crate) fn point(&self, x: f64, y: f64) -> (f64, f64) {
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

    /// Image of a direction `(x, y)` (`GeoVector.mirror`, `GeoVector.rotate`
    // and friends): a translation does not turn it and the others are
    // applied without their centre.
    pub(crate) fn vector(&self, x: f64, y: f64) -> (f64, f64) {
        match *self {
            Map::Point { .. } => (-x, -y),
            Map::Line { a, b, .. } => mirror_xy(x, y, 2.0 * (-a).atan2(b)),
            // `GeoVec3D.rotateXY`: a plain rotation, without `MyMath.cos`
            Map::Rotate { phi, .. } => {
                let (sin, cos) = phi.sin_cos();
                (x * cos - y * sin, x * sin + y * cos)
            }
            Map::Dilate { r, .. } => (r * x, r * y),
            Map::Matrix { m, .. } => (m[0] * x + m[1] * y, m[2] * x + m[3] * y),
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
                // undefined like `GeoLine.isDefined`
                line_value(a, b, c)
            }
            Value::Circle { .. }
            | Value::Number(_)
            | Value::Vector { .. }
            | Value::Ray { .. }
            | Value::Bool(_)
            | Value::Function
            | Value::Polygon { .. }
            | Value::PolyLine { .. }
            | Value::List { .. }
            | Value::Conic { .. }
            | Value::Arc { .. }
            | Value::Sector { .. }
            | Value::Undefined => None,
        }
    }
}

/// `MyMath.length`: no overflow, no underflow.
fn my_length(x: f64, y: f64) -> f64 {
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

/// The transformed object of a definition (every variant holds it).
pub(crate) fn source_of(d: &TransformDef) -> Id {
    match *d {
        TransformDef::MirrorPoint { src, .. }
        | TransformDef::MirrorLine { src, .. }
        | TransformDef::Invert { src, .. }
        | TransformDef::Rotate { src, .. }
        | TransformDef::Dilate { src, .. }
        | TransformDef::ShearStretch { src, .. }
        | TransformDef::StretchVector { src, .. } => src,
    }
}

/// A ray value from a start point and raw coefficients, undefined like
/// `GeoRay.isDefined`.
fn ray_value(start: (f64, f64), (x, y, z): (f64, f64, f64)) -> Option<Value> {
    let (a, b, c) = super::vector::stored_line(x, y, z)?;
    Some(Value::Ray {
        x: start.0,
        y: start.1,
        a,
        b,
        c,
    })
}

/// The image of a ray: GeoGebra rebuilds it from the images of the points
/// it was made from (`GeoRay.createTransformedObject`), so the image of
/// `Ray(A, B)` is `Ray(A', B')` and the image of `Ray(A, v)` starts at `A'`
/// and points along the image of `v`.
fn ray_image(c: &Construction, def: &TransformDef, map: &Map) -> Option<Value> {
    match c.def(source_of(def)) {
        Def::Vector(VectorDef::RayPoints(p, q)) => {
            let (pp, qq) = (c.point(p)?, c.point(q)?);
            let s = map.point(pp.0, pp.1);
            let e = map.point(qq.0, qq.1);
            ray_value(s, join(s, e))
        }
        Def::Vector(VectorDef::RayVector(p, v)) => {
            let pp = c.point(p)?;
            let Value::Vector { x: vx, y: vy } = c.value(v) else {
                return None;
            };
            // `AlgoUnitVectorLine` reads the ray's raw direction, which the
            // transformation turns; the image ray is made from it
            let d = map.vector(vx, vy);
            let s = map.point(pp.0, pp.1);
            ray_value(s, (-d.1, d.0, s.0 * d.1 - s.1 * d.0))
        }
        _ => None,
    }
}

/// `AlgoPolygon.calcAreaWithSign` over the moved vertices: half the sum of
/// `GeoPoint.det` over the closed sequence. `None` for fewer than two
/// points (an image of an empty polygon, which GeoGebra holds undefined).
fn moved_area(points: &[(f64, f64)]) -> Option<f64> {
    let (&first, rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut prev = first;
    let mut sum = 0.0;
    for &p in rest {
        sum += prev.0 * p.1 - p.0 * prev.1;
        prev = p;
    }
    sum += prev.0 * first.1 - first.0 * prev.1;
    Some(sum / 2.0)
}

impl TransformDef {
    fn center(c: &Construction, center: Option<Id>) -> Option<(f64, f64)> {
        match center {
            None => Some((0.0, 0.0)),
            Some(id) => c.point(id),
        }
    }

    /// The kind and source of the transformed object and the parameters of
    /// the transformation.
    pub(crate) fn parts(self) -> (Kind, Id, TransformOf) {
        match self {
            TransformDef::MirrorPoint { kind, src, center } => {
                (kind, src, TransformOf::MirrorPoint(center))
            }
            TransformDef::MirrorLine { kind, src, mirror } => {
                (kind, src, TransformOf::MirrorLine(mirror))
            }
            TransformDef::Invert { src, circle } => (Kind::Point, src, TransformOf::Invert(circle)),
            TransformDef::Rotate {
                kind,
                src,
                angle,
                center,
            } => (
                kind,
                src,
                TransformOf::Rotate {
                    angle,
                    center: center.map(|id| id.index() as u32),
                },
            ),
            TransformDef::Dilate {
                kind,
                src,
                factor,
                center,
            } => (
                kind,
                src,
                TransformOf::Dilate {
                    factor,
                    center: center.map(|id| id.index() as u32),
                },
            ),
            TransformDef::ShearStretch {
                kind,
                src,
                axis,
                ratio,
                shear,
            } => (
                kind,
                src,
                if shear {
                    TransformOf::Shear { axis, ratio }
                } else {
                    TransformOf::Stretch { axis, ratio }
                },
            ),
            TransformDef::StretchVector { kind, src, vector } => {
                (kind, src, TransformOf::StretchVector(vector))
            }
        }
    }

    /// The same transformation of another object: the sides of a transformed
    /// polygon are transformations of its sides.
    pub(crate) fn of(self, kind: Kind, src: Id) -> TransformDef {
        match self {
            TransformDef::MirrorPoint { center, .. } => {
                TransformDef::MirrorPoint { kind, src, center }
            }
            TransformDef::MirrorLine { mirror, .. } => {
                TransformDef::MirrorLine { kind, src, mirror }
            }
            TransformDef::Invert { circle, .. } => TransformDef::Invert { src, circle },
            TransformDef::Rotate { angle, center, .. } => TransformDef::Rotate {
                kind,
                src,
                angle,
                center,
            },
            TransformDef::Dilate { factor, center, .. } => TransformDef::Dilate {
                kind,
                src,
                factor,
                center,
            },
            TransformDef::ShearStretch {
                axis, ratio, shear, ..
            } => TransformDef::ShearStretch {
                kind,
                src,
                axis,
                ratio,
                shear,
            },
            TransformDef::StretchVector { vector, .. } => {
                TransformDef::StretchVector { kind, src, vector }
            }
        }
    }

    /// The map of the plane this definition is: the image of every point it
    /// moves. `None` when an input is undefined (the image then is too) or
    /// for the inversion, which parts compute themselves.
    pub(crate) fn map(&self, c: &Construction) -> Option<Map> {
        self.parts().2.map(c)
    }
}

impl TransformOf {
    /// The map of the plane this transformation is (see
    /// [`TransformDef::map`]).
    pub(crate) fn map(&self, c: &Construction) -> Option<Map> {
        match *self {
            TransformOf::MirrorPoint(center) => {
                let (qx, qy) = c.point(center)?;
                Some(Map::Point { qx, qy })
            }
            TransformOf::MirrorLine(mirror) => {
                let (a, b, c) = c.raw_carrier(mirror)?;
                Some(Map::Line { a, b, c })
            }
            TransformOf::Rotate { angle, center } => {
                let (qx, qy) = TransformDef::center(c, center_of(center))?;
                let phi = c.number(angle)?;
                Some(Map::Rotate { phi, qx, qy })
            }
            TransformOf::Dilate { factor, center } => {
                let (sx, sy) = TransformDef::center(c, center_of(center))?;
                let r = c.number(factor)?;
                Some(Map::Dilate { r, sx, sy })
            }
            TransformOf::Shear { axis, ratio } => {
                let (la, lb, lc) = c.raw_carrier(axis)?;
                let n = c.number(ratio)?;
                shear_matrix(la, lb, lc, n, true)
            }
            TransformOf::Stretch { axis, ratio } => {
                let (la, lb, lc) = c.raw_carrier(axis)?;
                let n = c.number(ratio)?;
                shear_matrix(la, lb, lc, n, false)
            }
            TransformOf::StretchVector(vector) => {
                let Value::Vector { x: vx, y: vy } = c.value(vector) else {
                    return None;
                };
                let (qx, qy) = match crate::algo::vector::start(c, vector) {
                    Some((sx, sy)) => (-sx, -sy),
                    None => (0.0, 0.0),
                };
                // the point of the vector moves to the origin and the
                // stretch is by the vector's length along it
                let len = my_length(vx, vy);
                let (cos, s) = (-vy / len, vx / len);
                let m = [
                    cos * cos + s * s * len,
                    cos * s * (1.0 - len),
                    cos * s * (1.0 - len),
                    s * s + cos * cos * len,
                ];
                Some(Map::Matrix { qx, qy, m })
            }
            TransformOf::Invert(_) => None,
        }
    }
}

/// The matrix of `Shear` and `Stretch` along the line `(la, lb, lc)` by the
/// ratio `n` (`AlgoShearOrStretch.compute`), which moves a point of the line
/// to the origin first and back afterwards.
fn shear_matrix(la: f64, lb: f64, lc: f64, n: f64, shear: bool) -> Option<Map> {
    let (qx, qy) = if la.abs() > lb.abs() {
        (lc / la, 0.0)
    } else {
        (0.0, lc / lb)
    };
    let len = my_length(la, lb);
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
    Some(Map::Matrix { qx, qy, m })
}

/// Inversion of the point `p` in the circle `(cx, cy, r)`
/// (`GeoPointND.mirror(GeoConicND)`): the point itself for a circle of
/// radius 0, `None` for the centre, whose image is at infinity.
pub(crate) fn invert_point(p: (f64, f64), (cx, cy, r): (f64, f64, f64)) -> Option<(f64, f64)> {
    // A circle of radius 0 is a single point and leaves the point alone.
    if r < STANDARD_PRECISION {
        return Some(p);
    }
    let (dx, dy) = (p.0 - cx, p.1 - cy);
    let scale = r * r / (dx * dx + dy * dy);
    let out = (cx + scale * dx, cy + scale * dy);
    (out.0.is_finite() && out.1.is_finite()).then_some(out)
}

impl Group for TransformDef {
    fn kind(&self) -> Kind {
        match *self {
            TransformDef::MirrorPoint { kind, .. }
            | TransformDef::MirrorLine { kind, .. }
            | TransformDef::Rotate { kind, .. }
            | TransformDef::Dilate { kind, .. }
            | TransformDef::ShearStretch { kind, .. }
            | TransformDef::StretchVector { kind, .. } => kind,
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
            TransformDef::StretchVector { src, vector, .. } => Parents::of(&[src, vector]),
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
                c.expect(src, SHEARABLE)?;
                c.expect(axis, LINE_LIKE)?;
                c.expect(ratio, NUMBER)
            }
            TransformDef::StretchVector { kind, src, vector } => {
                c.expect(src, of_kind(kind))?;
                c.expect(src, SHEARABLE)?;
                c.expect(vector, VECTOR)
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
        if let TransformDef::Invert { src, circle } = *self {
            let p = c.point(src)?;
            let (x, y) = invert_point(p, c.circle(circle)?)?;
            return Some(Value::Point { x, y });
        }
        let map = self.map(c);
        match self.kind() {
            Kind::Ray => ray_image(c, self, &map?),
            Kind::Vector => match (c.value(source_of(self)), map) {
                (Value::Vector { x, y }, Some(map)) => {
                    let (x, y) = map.vector(x, y);
                    Some(Value::Vector { x, y })
                }
                _ => None,
            },
            Kind::Polygon => {
                let map = map?;
                let mut points = Vec::new();
                for v in c.vertices(source_of(self)) {
                    let (x, y) = v?;
                    points.push(map.point(x, y));
                }
                Some(Value::Polygon {
                    area: moved_area(&points)?,
                })
            }
            Kind::PolyLine => {
                // the image runs through the points like the source
                // (`transformPoly`): an undefined input leaves a hole rather
                // than undefining the polyline (GeoPolyLine.isDefined)
                let mut length = 0.0;
                let mut prev: Option<(f64, f64)> = None;
                for v in c.vertices(source_of(self)) {
                    let p = v.and_then(|(x, y)| map.as_ref().map(|m| m.point(x, y)));
                    if let (Some(a), Some(b)) = (prev, p) {
                        let (dx, dy) = (a.0 - b.0, a.1 - b.1);
                        length += (dx * dx + dy * dy).sqrt();
                    }
                    prev = p;
                }
                Some(Value::PolyLine { length })
            }
            _ => map?.value(c.value(source_of(self))),
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        match func {
            "Mirror" | "Reflect" | "Rotate" | "Dilate" | "Shear" | "Stretch" => {}
            _ => return None,
        }
        Some(parse_transform(c, func, args))
    }
}

/// Parses a transformation of one of `kinds`; the parts of conics call
/// this with their own kinds and read the definition themselves.
pub(crate) fn parse_transform_for(
    c: &Construction,
    func: &str,
    args: &[&str],
    kinds: &[Kind],
) -> Result<TransformDef, String> {
    let arity_ok = match func {
        "Mirror" | "Reflect" => args.len() == 2,
        "Shear" => args.len() == 3,
        "Stretch" => args.len() == 2 || args.len() == 3,
        _ => args.len() == 2 || args.len() == 3,
    };
    if !arity_ok {
        return Err(format!("`{func}` does not take {} argument(s)", args.len()));
    }
    let src = object(c, args[0])?;
    let kind = c.kind(src);
    if !kinds.contains(&kind) {
        return Err(format!(
            "`{}` is a {kind}, which `{func}` does not transform",
            args[0]
        ));
    }
    if matches!(func, "Shear" | "Stretch") && kind == Kind::PolyLine {
        return Err(format!(
            "`{func}` of a polyline is not supported (GeoGebra rejects it too)"
        ));
    }
    let second = object(c, args[1])?;
    let second_kind = c.kind(second);

    if func == "Stretch" && args.len() == 2 {
        // `Stretch(object, vector)`: stretched along the vector by its length
        if second_kind != Kind::Vector {
            return Err(format!(
                "`{}` is a {second_kind}, expected a vector",
                args[1]
            ));
        }
        return Ok(TransformDef::StretchVector {
            kind,
            src,
            vector: second,
        });
    }

    if matches!(func, "Shear" | "Stretch") {
        if !matches!(second_kind, Kind::Line | Kind::Segment | Kind::Ray) {
            return Err(format!("`{}` is a {second_kind}, expected a line", args[1]));
        }
        let ratio = object(c, args[2])?;
        if !c.kind(ratio).is_number() {
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
            Kind::Line | Kind::Segment | Kind::Ray => Ok(TransformDef::MirrorLine {
                kind,
                src,
                mirror: second,
            }),
            Kind::Circle if kind == Kind::Point => Ok(TransformDef::Invert {
                src,
                circle: second,
            }),
            Kind::Circle => Err(format!(
                "inversion of a {kind} in a circle is not supported (only a point, a line, a \
                 segment, a circle, a conic, an arc and a sector can be inverted)"
            )),
            _ => Err(format!(
                "`{}` is a {second_kind}, expected a mirror",
                args[1]
            )),
        };
    }

    if !second_kind.is_number() {
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

fn parse_transform(c: &Construction, func: &str, args: &[&str]) -> Result<TransformDef, String> {
    parse_transform_for(c, func, args, TRANSFORMABLE)
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
        let mut c = load(
            "A = (1, 0)\nB = (3, 1)\nw = 0.5\nl = Line(A, B)\nk = Circle(A, B)\n\
             r = Ray(A, B)\nt = PolyLine(A, B)\n",
        )
        .unwrap();
        let mut err = |line: &str| exec(&mut c, line).unwrap_err();
        assert!(err("m = Mirror(A)").contains("`Mirror` does not take 1 argument(s)"));
        assert!(err("m = Reflect(A, B, l)").contains("does not take 3"));
        assert!(err("m = Rotate(A)").contains("`Rotate` does not take 1"));
        assert!(err("m = Dilate(A, w, B, B)").contains("`Dilate` does not take 4"));
        assert!(err("m = Rotate(A, B)").contains("expected a number"));
        assert!(err("m = Rotate(A, w, l)").contains("expected a point"));
        assert!(err("m = Mirror(w, A)").contains("does not transform"));
        assert!(err("m = Mirror(A, w)").contains("expected a mirror"));
        // the inversion of a ray is not supported (its ends are labelled
        // like a ray's in GeoGebra); a line, a segment, a circle, a conic,
        // an arc and a sector can be inverted
        assert!(err("m = Mirror(r, k)").contains("not supported"));
        assert!(err("m = Mirror(t, k)").contains("not supported"));
        assert!(err("m = Dilate(Z, w)").contains("unknown object"));
        assert!(err("m = Shear(A, l)").contains("`Shear` does not take 2"));
        // `Stretch(A, l)` with a line is the vector form's error: `l` is
        // not a vector
        assert!(err("m = Stretch(A, l)").contains("expected a vector"));
        assert!(err("m = Shear(t, l, w)").contains("polyline is not supported"));
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
        // l is y = 0 with GeoGebra's raw coefficients (0, 4, 0); the mirror
        // in a point flips it.
        assert_eq!(
            value(&c, "m"),
            Value::Line {
                a: 0.0,
                b: -4.0,
                c: 8.0
            }
        );
        assert_eq!(
            value(&c, "d"),
            Value::Line {
                a: -0.0,
                b: -4.0,
                c: -0.0
            }
        );
        // Mirroring a line in itself keeps the line, its scale and its orientation.
        let Value::Line { a, b, c: z } = value(&c, "n") else {
            panic!()
        };
        assert!(close(a, 0.0) && close(b, 4.0) && close(z, 0.0));
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
