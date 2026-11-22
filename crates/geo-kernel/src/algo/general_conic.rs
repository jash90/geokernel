// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicFivePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicFromCoeffList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoEllipseHyperbolaFociPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicFociLengthND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoParabolaPointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTransformation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDilate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRotate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRotatePoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTranslate.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoMirror.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdEllipseHyperbola.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdParabola.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Objects whose state is a [`ConicData`] record: the general conics
//! (`Conic`, `Ellipse`, `Hyperbola`, `Parabola`) and the transformations of
//! a conic or a circle. `Dilate` of a circle is computed on the circle's
//! matrix and classified again; `Translate`, `Rotate` and `Mirror` move the
//! circle's axes with it (`GeoConicND.rotate`, `GeoConic.mirror` turn and
//! mirror the eigenvectors by hand), and points on the image are placed by
//! those axes.
//!
//! These definitions do not compute a [`Value`] from their parents alone:
//! the classification depends on the previous state of the object (see
//! [`crate::algo::conic_nd`]). The construction keeps one record per object
//! and asks [`GeneralConicDef::record`] for the next one, and the value is
//! derived from the record by [`value`].
//!
//! A transformation of a conic starts from a copy of the source's record
//! (`AlgoTransformation.setOutGeo`), so its result depends on the source
//! only. A transformed circle stays a [`Kind::Circle`]: its value is the
//! circle of its record, and it is undefined when GeoGebra's image is no
//! longer a circle or a single point.
//!
//! Not supported: `Ellipse` and `Hyperbola` with a segment as the axis
//! length, the special case of `Conic` through five points that lie on a
//! parabola by construction, and points with a homogeneous coordinate
//! other than 1 (every point of the kernel has `z = 1`).

use super::conic_nd::{ConicData, ConicType};
use super::transform::{center_of, shear_axis, Map, TransformOf};
use super::{Group, Parents, NUMBER, NUMBER_OR_ANGLE, POINT};
use crate::expr::{Element, ExprDef, List};
use crate::kernel::{Construction, Error, Id, IdList, Kind, Value};
use crate::script::object;

const VECTOR: &[Kind] = &[Kind::Vector];
const CIRCLE: &[Kind] = &[Kind::Circle];
const LINE: &[Kind] = &[Kind::Line];

/// General conics and the transformations of conics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeneralConicDef {
    /// `Conic(A, B, C, D, E)`: the conic through five points, read from a
    /// stored list.
    FivePoints(IdList),
    /// `Conic(list)` with a named list of coefficients or matrix rows.
    CoefficientList(Id),
    /// `Conic(list)` with an inline list, or `Conic` of six numbers.
    /// As with a standalone expression, the handle belongs to its construction.
    CoefficientExpr { expression: ExprDef, reads: IdList },
    /// `Ellipse(F, G, P)` or `Hyperbola(F, G, P)`: foci and a point.
    FociPoint {
        f1: Id,
        f2: Id,
        through: Id,
        hyperbola: bool,
    },
    /// `Ellipse(F, G, a)` and `Hyperbola(F, G, a)`: foci and the length of
    /// the semi-major axis. Both commands make the same conic; its type is
    /// decided by the matrix.
    FociLength { f1: Id, f2: Id, length: Id },
    /// `Parabola(F, g)`: focus and directrix (a line, segment or ray).
    Parabola { focus: Id, directrix: Id },
    /// `ApplyMatrix(matrix, conic)`; a circle can become a general conic.
    ApplyMatrix { matrix: Id, src: Id },
    /// `Translate(k, v)`.
    Translate { kind: Kind, src: Id, by: Id },
    /// `Rotate(k, angle)` and `Rotate(k, angle, Q)`.
    Rotate {
        kind: Kind,
        src: Id,
        angle: Id,
        center: Option<Id>,
    },
    /// `Dilate(k, factor)` and `Dilate(k, factor, Q)`.
    Dilate {
        kind: Kind,
        src: Id,
        factor: Id,
        center: Option<Id>,
    },
    /// `Mirror(k, Q)`.
    MirrorPoint { kind: Kind, src: Id, center: Id },
    /// `Mirror(k, g)` with a line, segment or ray.
    MirrorLine { kind: Kind, src: Id, mirror: Id },
    /// `Shear(k, g, f)` and `Stretch(k, g, f)` of a conic or a circle.
    ShearStretch {
        kind: Kind,
        src: Id,
        axis: Id,
        ratio: Id,
        shear: bool,
    },
    /// `Stretch(k, v)`: stretched along the vector `v` by its length.
    StretchVector { kind: Kind, src: Id, vector: Id },
    /// `Mirror(k, c)`: the inversion of a conic or circle in a circle.
    Invert { kind: Kind, src: Id, circle: Id },
    /// `Mirror(g, c)`: the inversion of a line in a circle, which is a
    /// circle through the centre of `c` (or the line itself when it passes
    /// through the centre, which the kernel reports as undefined).
    InvertLine { src: Id, circle: Id },
}

/// The value of an object of kind `kind` with the record `d`.
///
/// A circle is a [`Value::Circle`] while its record is a circle or a
/// single point; a conic is a [`Value::Conic`]. Undefined when the record
/// is, or when a number the golden files record for its type is not finite
/// (GeoGebra's harness counts such an object as undefined).
pub(crate) fn value(kind: Kind, d: &ConicData) -> Value {
    if !d.defined || !recorded_finite(d) {
        return Value::Undefined;
    }
    match kind {
        Kind::Circle => match d.ty {
            ConicType::Circle | ConicType::SinglePoint => Value::Circle {
                cx: d.midpoint[0],
                cy: d.midpoint[1],
                r: d.half_axes[0],
            },
            _ => Value::Undefined,
        },
        _ => Value::Conic { ty: d.ty },
    }
}

/// Whether the numbers recorded for the type of `d` are finite: the matrix,
/// and the parts of its type (`golden/SCHEMA.md`).
fn recorded_finite(d: &ConicData) -> bool {
    let fin = |v: &[f64]| v.iter().all(|x| x.is_finite());
    let ev = [
        d.eigenvec[0][0],
        d.eigenvec[0][1],
        d.eigenvec[1][0],
        d.eigenvec[1][1],
    ];
    fin(&d.matrix)
        && match d.ty {
            ConicType::Circle | ConicType::Ellipse | ConicType::Hyperbola => {
                fin(&d.midpoint) && fin(&d.half_axes) && fin(&ev)
            }
            ConicType::Parabola => fin(&d.midpoint) && d.p.is_finite() && fin(&ev),
            ConicType::SinglePoint => fin(&d.midpoint),
            ConicType::IntersectingLines | ConicType::ParallelLines | ConicType::DoubleLine => {
                fin(&d.lines[0]) && fin(&d.lines[1])
            }
            ConicType::Empty | ConicType::NotClassified => true,
        }
}

impl GeneralConicDef {
    /// The record of the object after an update, from its record `prev`
    /// before it (a new object starts from [`ConicData::new`]).
    pub(crate) fn record(&self, c: &Construction, prev: &ConicData) -> ConicData {
        let nan = (f64::NAN, f64::NAN);
        let pt = |id: Id| c.point(id).unwrap_or(nan);
        let mut d = *prev;
        match *self {
            GeneralConicDef::FivePoints(list) => {
                let ids = c.list(list);
                let mut p = [nan; 5];
                for (slot, &id) in p.iter_mut().zip(ids) {
                    *slot = pt(id);
                }
                d.set_five_points(p);
            }
            GeneralConicDef::CoefficientList(id) => {
                set_coefficient_matrix(&mut d, c.elements(id).and_then(coefficient_matrix));
            }
            GeneralConicDef::CoefficientExpr { expression, .. } => {
                set_coefficient_matrix(&mut d, Self::coefficient_expression_matrix(c, expression));
            }
            GeneralConicDef::FociPoint {
                f1,
                f2,
                through,
                hyperbola,
            } => {
                let (a, b, p) = (pt(f1), pt(f2), pt(through));
                let length1 = ((a.0 - p.0) * (a.0 - p.0) + (a.1 - p.1) * (a.1 - p.1)).sqrt();
                let length2 = ((b.0 - p.0) * (b.0 - p.0) + (b.1 - p.1) * (b.1 - p.1)).sqrt();
                let length = if hyperbola {
                    (length1 - length2).abs()
                } else {
                    length1 + length2
                };
                d.set_ellipse_hyperbola(a, b, length / 2.0);
            }
            GeneralConicDef::FociLength { f1, f2, length } => {
                let a = c.number(length).unwrap_or(f64::NAN);
                d.set_ellipse_hyperbola(pt(f1), pt(f2), a);
            }
            GeneralConicDef::Parabola { focus, directrix } => {
                // `setParabola` leaves the record alone when an input is undefined
                match (c.point(focus), c.raw_carrier(directrix)) {
                    (Some(f), Some((a, b, z))) if super::line_value(a, b, z).is_some() => {
                        d.set_parabola(f, [a, b, z]);
                    }
                    _ => d.defined = false,
                }
            }
            GeneralConicDef::ApplyMatrix { matrix, src } => {
                if set_out(c, src, &mut d) {
                    if let Some(m) = c
                        .elements(matrix)
                        .and_then(super::vector::matrix::Matrix::read)
                    {
                        m.conic(&mut d);
                    } else {
                        d.defined = false;
                    }
                }
            }
            GeneralConicDef::Translate { src, by, .. } => {
                let Some((vx, vy)) = vector(c, by) else {
                    d.defined = false;
                    return d;
                };
                if !set_out(c, src, &mut d) {
                    return d;
                }
                d.translate(vx, vy);
            }
            GeneralConicDef::Rotate {
                src, angle, center, ..
            } => {
                if !set_out(c, src, &mut d) {
                    return d;
                }
                // an undefined angle or centre fills the record with NaN, which
                // the golden files count as an undefined object
                let Some((phi, center)) = rotated_by(c, angle, center) else {
                    d.defined = false;
                    return d;
                };
                match center {
                    None => d.rotate(phi, None),
                    Some(q) => d.rotate(phi, Some(q)),
                }
            }
            GeneralConicDef::Dilate {
                src,
                factor,
                center,
                ..
            } => {
                if !set_out(c, src, &mut d) {
                    return d;
                }
                let Some(r) = c.number(factor) else {
                    d.defined = false;
                    return d;
                };
                let s = match center {
                    None => (0.0, 0.0),
                    Some(q) => match c.point(q) {
                        Some(p) => p,
                        None => {
                            d.defined = false;
                            return d;
                        }
                    },
                };
                d.dilate(r, s);
            }
            GeneralConicDef::MirrorPoint { src, center, .. } => {
                // `AlgoMirror`: an undefined mirror makes the image undefined
                let Some(q) = c.point(center) else {
                    d.defined = false;
                    return d;
                };
                if !set_out(c, src, &mut d) {
                    return d;
                }
                d.mirror_point(q);
            }
            GeneralConicDef::MirrorLine { src, mirror, .. } => {
                let Some((a, b, z)) = c.raw_carrier(mirror) else {
                    d.defined = false;
                    return d;
                };
                if super::line_value(a, b, z).is_none() {
                    d.defined = false;
                    return d;
                }
                if !set_out(c, src, &mut d) {
                    return d;
                }
                d.mirror_line([a, b, z]);
            }
            GeneralConicDef::ShearStretch {
                src,
                axis,
                ratio,
                shear,
                ..
            } => {
                if !set_out(c, src, &mut d) {
                    return d;
                }
                let def = if shear {
                    TransformOf::Shear { axis, ratio }
                } else {
                    TransformOf::Stretch { axis, ratio }
                };
                transform_conic(c, &def, &mut d);
            }
            GeneralConicDef::StretchVector { src, vector, .. } => {
                if !set_out(c, src, &mut d) {
                    return d;
                }
                transform_conic(c, &TransformOf::StretchVector(vector), &mut d);
            }
            GeneralConicDef::Invert { src, circle, .. } => {
                let Some(k) = c.conic(circle) else {
                    d.defined = false;
                    return d;
                };
                if !set_out(c, src, &mut d) {
                    return d;
                }
                d.mirror_conic(&k, false);
            }
            GeneralConicDef::InvertLine { src, circle } => {
                let (Some(k), Some((a, b, z))) = (c.conic(circle), c.raw_carrier(src)) else {
                    d.defined = false;
                    return d;
                };
                d = ConicData::new();
                d.from_line([a, b, z], None);
                d.mirror_conic(&k, true);
            }
        }
        d
    }
}

fn set_coefficient_matrix(d: &mut ConicData, matrix: Option<[f64; 6]>) {
    if let Some(matrix) = matrix {
        d.set_matrix(matrix);
    } else {
        d.defined = false;
    }
}

/// `AlgoConicFromCoeffList` and `GeoConicND.setCoeffs`. A flat list is
/// ordered as x², y², constant, xy, x, y. A matrix contributes its diagonal
/// and the average of each pair of opposite off-diagonal entries.
pub(crate) fn coefficient_matrix(list: List<'_>) -> Option<[f64; 6]> {
    fn number(element: Option<Element<'_>>) -> f64 {
        match element {
            Some(Element::Value(_, Value::Number(v))) => v,
            Some(Element::Value(_, Value::Bool(v))) => f64::from(u8::from(v)),
            _ => f64::NAN,
        }
    }

    match list.len() {
        6 => Some(std::array::from_fn(|i| {
            let v = number(list.get(i));
            if i < 3 {
                v
            } else {
                v / 2.0
            }
        })),
        3 => {
            let get = |row, col| match list.get(row) {
                Some(Element::List(row)) => number(row.get(col)),
                _ => f64::NAN,
            };
            Some([
                get(0, 0),
                get(1, 1),
                get(2, 2),
                (get(0, 1) + get(1, 0)) / 2.0,
                (get(0, 2) + get(2, 0)) / 2.0,
                (get(1, 2) + get(2, 1)) / 2.0,
            ])
        }
        _ => None,
    }
}

/// `AlgoTransformation.setOutGeo`: the image starts as a copy of the
/// source's record. Returns whether the copy is defined; the transformation
/// stops when it is not. A source whose record is defined but whose value
/// is not (a part that is not finite) counts as undefined, so that the
/// image of an undefined conic is undefined.
fn set_out(c: &Construction, src: Id, d: &mut ConicData) -> bool {
    match c.conic(src) {
        Some(s) => {
            *d = s;
            d.defined = s.defined && c.value(src).is_defined();
            d.defined
        }
        None => {
            d.defined = false;
            false
        }
    }
}

fn vector(c: &Construction, id: Id) -> Option<(f64, f64)> {
    match c.value(id) {
        Value::Vector { x, y } => Some((x, y)),
        _ => None,
    }
}

/// The angle a rotation runs by and the centre it runs about, as one tuple
/// (`None` for the origin); `None` when either is undefined.
fn rotated_by(
    c: &Construction,
    angle: Id,
    center: Option<Id>,
) -> Option<(f64, Option<(f64, f64)>)> {
    let phi = c.number(angle)?;
    let center = match center {
        None => None,
        Some(q) => Some(c.point(q)?),
    };
    Some((phi, center))
}

/// Applies the transformation `def` of [`record`](GeneralConicDef::record) to
/// `d`, which starts as a copy of the source's record (`setOutGeo`); an
/// undefined input leaves `d` undefined. The parts of conics call this with
/// the conic of their own record, which starts the same way.
pub(crate) fn transform_conic(c: &Construction, def: &TransformOf, d: &mut ConicData) {
    if !d.defined {
        return;
    }
    match *def {
        TransformOf::MirrorPoint(center) => {
            // `AlgoMirror`: an undefined mirror makes the image undefined
            match c.point(center) {
                Some(q) => d.mirror_point(q),
                None => d.defined = false,
            }
        }
        TransformOf::MirrorLine(mirror) => match c.raw_carrier(mirror) {
            Some((a, b, z)) if super::line_value(a, b, z).is_some() => {
                d.mirror_line([a, b, z]);
            }
            _ => d.defined = false,
        },
        TransformOf::Rotate { angle, center } => {
            // an undefined angle or centre leaves no image (the parts of
            // conics read this record, so the flag ends their parameters)
            match rotated_by(c, angle, center_of(center)) {
                Some((phi, None)) => d.rotate(phi, None),
                Some((phi, Some(q))) => d.rotate(phi, Some(q)),
                None => d.defined = false,
            }
        }
        TransformOf::Dilate { factor, center } => {
            let (r, s) = match (c.number(factor), center_of(center).map(|q| c.point(q))) {
                (Some(r), None) => (r, (0.0, 0.0)),
                (Some(r), Some(Some(q))) => (r, q),
                _ => {
                    d.defined = false;
                    return;
                }
            };
            d.dilate(r, s);
        }
        TransformOf::Shear { .. } | TransformOf::Stretch { .. } | TransformOf::StretchVector(_) => {
            if let Some(Map::Matrix {
                qx,
                qy,
                m: [m00, m01, m10, m11],
            }) = def.map(c)
            {
                d.translate(qx, qy);
                d.matrix_transform(m00, m01, m10, m11);
                d.translate(-qx, -qy);
            } else {
                d.defined = false;
            }
        }
        // the inversion of a part is computed by the part itself
        // (`ArcDef::InvertPart`); a plain point has no conic record
        TransformOf::Invert(_) => d.defined = false,
    }
}

impl Group for GeneralConicDef {
    fn kind(&self) -> Kind {
        match *self {
            GeneralConicDef::FivePoints(_)
            | GeneralConicDef::CoefficientList(_)
            | GeneralConicDef::CoefficientExpr { .. }
            | GeneralConicDef::FociPoint { .. }
            | GeneralConicDef::FociLength { .. }
            | GeneralConicDef::Parabola { .. } => Kind::Conic,
            GeneralConicDef::Translate { kind, .. }
            | GeneralConicDef::Rotate { kind, .. }
            | GeneralConicDef::Dilate { kind, .. }
            | GeneralConicDef::MirrorPoint { kind, .. }
            | GeneralConicDef::MirrorLine { kind, .. }
            | GeneralConicDef::ShearStretch { kind, .. }
            | GeneralConicDef::StretchVector { kind, .. }
            | GeneralConicDef::Invert { kind, .. } => kind,
            GeneralConicDef::InvertLine { .. } | GeneralConicDef::ApplyMatrix { .. } => Kind::Conic,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            GeneralConicDef::FivePoints(_) | GeneralConicDef::CoefficientExpr { .. } => {
                Parents::none()
            }
            GeneralConicDef::CoefficientList(id) => Parents::of(&[id]),
            GeneralConicDef::FociPoint {
                f1, f2, through, ..
            } => Parents::of(&[f1, f2, through]),
            GeneralConicDef::FociLength { f1, f2, length } => Parents::of(&[f1, f2, length]),
            GeneralConicDef::Parabola { focus, directrix } => Parents::of(&[focus, directrix]),
            GeneralConicDef::ApplyMatrix { matrix, src } => Parents::of(&[matrix, src]),
            GeneralConicDef::Translate { src, by, .. } => Parents::of(&[src, by]),
            GeneralConicDef::Rotate {
                src,
                angle: second,
                center,
                ..
            }
            | GeneralConicDef::Dilate {
                src,
                factor: second,
                center,
                ..
            } => match center {
                Some(q) => Parents::of(&[src, second, q]),
                None => Parents::of(&[src, second]),
            },
            GeneralConicDef::MirrorPoint { src, center, .. } => Parents::of(&[src, center]),
            GeneralConicDef::MirrorLine { src, mirror, .. } => Parents::of(&[src, mirror]),
            GeneralConicDef::ShearStretch {
                src, axis, ratio, ..
            } => Parents::of(&[src, axis, ratio]),
            GeneralConicDef::StretchVector { src, vector, .. } => Parents::of(&[src, vector]),
            GeneralConicDef::Invert { src, circle, .. }
            | GeneralConicDef::InvertLine { src, circle } => Parents::of(&[src, circle]),
        }
    }

    fn list(&self) -> Option<IdList> {
        match *self {
            GeneralConicDef::FivePoints(list)
            | GeneralConicDef::CoefficientExpr { reads: list, .. } => Some(list),
            _ => None,
        }
    }

    fn with_list(self, list: IdList) -> Self {
        match self {
            GeneralConicDef::FivePoints(_) => GeneralConicDef::FivePoints(list),
            GeneralConicDef::CoefficientExpr { expression, .. } => {
                GeneralConicDef::CoefficientExpr {
                    expression,
                    reads: list,
                }
            }
            other => other,
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        let kind_of = |kind: Kind| match kind {
            Kind::Circle => Ok(&[Kind::Circle][..]),
            // a sheared or stretched circle is held as a conic, whose source
            // is a circle or a conic
            Kind::Conic => Ok(&[Kind::Conic, Kind::Circle][..]),
            _ => Err(Error::UnknownObject),
        };
        match *self {
            GeneralConicDef::FivePoints(list) => {
                if c.list(list).len() != 5 {
                    return Err(Error::UnknownObject);
                }
                c.expect_list(list, POINT)
            }
            GeneralConicDef::CoefficientList(id) => c.expect(id, &[Kind::List]),
            GeneralConicDef::CoefficientExpr { expression, .. } => {
                crate::expr::check_handle(c, expression)
            }
            GeneralConicDef::FociPoint {
                f1, f2, through, ..
            } => {
                c.expect(f1, POINT)?;
                c.expect(f2, POINT)?;
                c.expect(through, POINT)
            }
            GeneralConicDef::FociLength { f1, f2, length } => {
                c.expect(f1, POINT)?;
                c.expect(f2, POINT)?;
                c.expect(length, NUMBER)
            }
            GeneralConicDef::Parabola { focus, directrix } => {
                c.expect(focus, POINT)?;
                c.expect(directrix, LINE_OR_RAY)
            }
            GeneralConicDef::ApplyMatrix { matrix, src } => {
                c.expect(matrix, &[Kind::List])?;
                c.expect(src, &[Kind::Conic, Kind::Circle])
            }
            GeneralConicDef::Translate { kind, src, by } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(by, VECTOR)
            }
            GeneralConicDef::Rotate {
                kind,
                src,
                angle: second,
                center,
            }
            | GeneralConicDef::Dilate {
                kind,
                src,
                factor: second,
                center,
            } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(second, NUMBER_OR_ANGLE)?;
                match center {
                    Some(q) => c.expect(q, POINT),
                    None => Ok(()),
                }
            }
            GeneralConicDef::MirrorPoint { kind, src, center } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(center, POINT)
            }
            GeneralConicDef::MirrorLine { kind, src, mirror } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(mirror, LINE_OR_RAY)
            }
            GeneralConicDef::ShearStretch {
                kind,
                src,
                axis,
                ratio,
                shear,
            } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(axis, shear_axis(shear))?;
                c.expect(ratio, NUMBER_OR_ANGLE)
            }
            GeneralConicDef::StretchVector { kind, src, vector } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(vector, VECTOR)
            }
            GeneralConicDef::Invert { kind, src, circle } => {
                c.expect(src, kind_of(kind)?)?;
                c.expect(circle, CIRCLE)
            }
            GeneralConicDef::InvertLine { src, circle } => {
                c.expect(src, LINE)?;
                c.expect(circle, CIRCLE)
            }
        }
    }

    /// The value from a fresh record. The construction does not call this
    /// for its objects: it keeps their records (see the module docs).
    fn compute(&self, c: &Construction) -> Option<Value> {
        Some(value(self.kind(), &self.record(c, &ConicData::new())))
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        match func {
            "Ellipse" | "Hyperbola" => Some(parse_foci(c, func, args)),
            "Parabola" => Some(parse_parabola(c, args)),
            "Translate" | "Rotate" | "Dilate" | "Mirror" | "Reflect" | "Shear" | "Stretch" => {
                parse_transform(c, func, args)
            }
            _ => None,
        }
    }
}

/// Lines, segments and rays: what GeoGebra's `isGeoLine` accepts.
const LINE_OR_RAY: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray];

fn point_arg(c: &Construction, label: &str) -> Result<Id, String> {
    let id = object(c, label)?;
    if c.kind(id) != Kind::Point {
        return Err(format!("`{label}` is a {}, expected a point", c.kind(id)));
    }
    Ok(id)
}

fn parse_foci(c: &Construction, func: &str, args: &[&str]) -> Result<GeneralConicDef, String> {
    if args.len() != 3 {
        return Err(format!("`{func}` does not take {} argument(s)", args.len()));
    }
    let f1 = point_arg(c, args[0])?;
    let f2 = point_arg(c, args[1])?;
    let third = object(c, args[2])?;
    match c.kind(third) {
        Kind::Point => Ok(GeneralConicDef::FociPoint {
            f1,
            f2,
            through: third,
            hyperbola: func == "Hyperbola",
        }),
        Kind::Number => Ok(GeneralConicDef::FociLength {
            f1,
            f2,
            length: third,
        }),
        other => Err(format!(
            "`{}` is a {other}, expected a point or a number",
            args[2]
        )),
    }
}

fn parse_parabola(c: &Construction, args: &[&str]) -> Result<GeneralConicDef, String> {
    if args.len() != 2 {
        return Err(format!(
            "`Parabola` does not take {} argument(s)",
            args.len()
        ));
    }
    let focus = point_arg(c, args[0])?;
    let directrix = object(c, args[1])?;
    if !LINE_OR_RAY.contains(&c.kind(directrix)) {
        return Err(format!(
            "`{}` is a {}, expected a line",
            args[1],
            c.kind(directrix)
        ));
    }
    Ok(GeneralConicDef::Parabola { focus, directrix })
}

/// A transformation of a conic or of a circle: every image of a circle
/// keeps a record too (see below). The group also claims the inversion of a
/// line in a circle, whose image is a conic. `None` leaves the line to the
/// other groups.
fn parse_transform(
    c: &Construction,
    func: &str,
    args: &[&str],
) -> Option<Result<GeneralConicDef, String>> {
    let src = c.id(args.first()?)?;
    let kind = c.kind(src);
    // whether the command mirrors in a circle (an inversion)
    let in_a_circle = matches!(func, "Mirror" | "Reflect")
        && matches!(c.id(args.get(1)?).map(|id| c.kind(id)), Some(Kind::Circle));
    let claim = match kind {
        Kind::Conic => true,
        // every image of a circle keeps a record: GeoGebra copies the
        // circle's axes and turns or mirrors them with it, and the axes
        // carry the path parameter of points on the image
        Kind::Circle => true,
        // the inversion of a line is a conic (a circle through the centre
        // of the mirror)
        Kind::Line => in_a_circle,
        _ => false,
    };
    if !claim {
        return None;
    }
    let arity_ok = match func {
        "Mirror" | "Reflect" | "Translate" => args.len() == 2,
        "Shear" => args.len() == 3,
        "Stretch" => args.len() == 2 || args.len() == 3,
        _ => args.len() == 2 || args.len() == 3,
    };
    if !arity_ok {
        return Some(Err(format!(
            "`{func}` does not take {} argument(s)",
            args.len()
        )));
    }
    Some(parse_transform_args(c, func, args, kind, src))
}

fn parse_transform_args(
    c: &Construction,
    func: &str,
    args: &[&str],
    kind: Kind,
    src: Id,
) -> Result<GeneralConicDef, String> {
    let second = object(c, args[1])?;
    let second_kind = c.kind(second);
    // `Shear` and `Stretch` of a circle give a conic (an ellipse usually),
    // whose kind is a conic whatever the image turns out to be
    let kind = if matches!(func, "Shear" | "Stretch") && kind == Kind::Circle {
        Kind::Conic
    } else {
        kind
    };
    match func {
        "Translate" => {
            if second_kind != Kind::Vector {
                return Err(format!(
                    "`{}` is a {second_kind}, expected a vector",
                    args[1]
                ));
            }
            Ok(GeneralConicDef::Translate {
                kind,
                src,
                by: second,
            })
        }
        "Mirror" | "Reflect" => match second_kind {
            Kind::Point => Ok(GeneralConicDef::MirrorPoint {
                kind,
                src,
                center: second,
            }),
            k if LINE_OR_RAY.contains(&k) => Ok(GeneralConicDef::MirrorLine {
                kind,
                src,
                mirror: second,
            }),
            Kind::Circle if kind == Kind::Line => Ok(GeneralConicDef::InvertLine {
                src,
                circle: second,
            }),
            Kind::Circle => Ok(GeneralConicDef::Invert {
                kind,
                src,
                circle: second,
            }),
            _ => Err(format!(
                "`{}` is a {second_kind}, expected a point or a line (a conic cannot be inverted)",
                args[1]
            )),
        },
        "Shear" | "Stretch" => {
            if args.len() == 2 {
                // `Stretch(k, v)`: stretched along a vector by its length
                if second_kind != Kind::Vector {
                    return Err(format!(
                        "`{}` is a {second_kind}, expected a vector",
                        args[1]
                    ));
                }
                return Ok(GeneralConicDef::StretchVector {
                    kind,
                    src,
                    vector: second,
                });
            }
            let shear = func == "Shear";
            if !shear_axis(shear).contains(&second_kind) {
                let expected = if shear {
                    "a line or a vector"
                } else {
                    "a line"
                };
                return Err(format!(
                    "`{}` is a {second_kind}, expected {expected}",
                    args[1]
                ));
            }
            let ratio = object(c, args[2])?;
            if !c.kind(ratio).is_number() {
                return Err(format!(
                    "`{}` is a {}, expected a number",
                    args[2],
                    c.kind(ratio)
                ));
            }
            Ok(GeneralConicDef::ShearStretch {
                kind,
                src,
                axis: second,
                ratio,
                shear,
            })
        }
        _ => {
            // a `GeoNumberValue`, which an angle is too
            if !second_kind.is_number() {
                return Err(format!(
                    "`{}` is a {second_kind}, expected a number",
                    args[1]
                ));
            }
            let center = match args.get(2) {
                None => None,
                Some(label) => Some(point_arg(c, label)?),
            };
            Ok(if func == "Rotate" {
                GeneralConicDef::Rotate {
                    kind,
                    src,
                    angle: second,
                    center,
                }
            } else {
                GeneralConicDef::Dilate {
                    kind,
                    src,
                    factor: second,
                    center,
                }
            })
        }
    }
}

/// Runs the three input forms of `Conic`. Other commands use their own dispatch.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (func, args) = rhs.split_once('(')?;
    if func.trim() != "Conic" {
        return None;
    }
    Some(
        args.strip_suffix(')')
            .ok_or_else(|| "missing closing `)`".into())
            .and_then(|args| GeneralConicDef::add_from_arguments(c, name, args).map(Some)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::load;

    fn conic(c: &Construction, label: &str) -> ConicData {
        c.conic(c.id(label).unwrap()).unwrap()
    }

    #[test]
    fn inline_coefficients_create_one_object_and_roll_back_with_it() {
        let mut c = load("a = -4\n").unwrap();
        let start = c.checkpoint();
        let result = crate::script::exec(&mut c, "k = Conic(1, 1, a, 0, 0, 0)")
            .unwrap()
            .unwrap();
        assert_eq!(result.index(), 1);
        assert_eq!(c.len(), 2);
        assert!(crate::script::exec(&mut c, "k = Conic({1, 1, -9, 0, 0, 0})").is_err());
        assert_eq!(c.len(), 2);
        c.rollback(start).unwrap();

        let result = crate::script::exec(&mut c, "k = Conic({1, 4, a, 0, 0, 0})")
            .unwrap()
            .unwrap();
        assert_eq!(result.index(), 1);
        c.set_number(c.id("a").unwrap(), -9.0).unwrap();
        let record = c.conic(result).unwrap();
        assert_eq!(record.ty, ConicType::Ellipse);
        assert_eq!(record.matrix, [1.0, 4.0, -9.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn five_points_on_a_circle_make_a_circle() {
        let c = load(
            "A = (1, 0)\nB = (0, 1)\nC = (-1, 0)\nD = (0, -1)\nE = (0.6, 0.8)\nk = Conic(A, B, C, D, E)\n",
        )
        .unwrap();
        let d = conic(&c, "k");
        assert_eq!(d.ty, ConicType::Circle);
        assert!((d.half_axes[0] - 1.0).abs() < 1e-12);
        assert!(d.midpoint[0].abs() < 1e-12 && d.midpoint[1].abs() < 1e-12);
    }

    #[test]
    fn foci_and_a_point_make_an_ellipse_and_a_hyperbola() {
        let c = load(
            "F = (-3, 0)\nG = (3, 0)\nP = (0, 4)\ne = Ellipse(F, G, P)\nh = Hyperbola(F, G, (4, 0))\n",
        );
        // a point literal is not an object: the hyperbola needs a named point
        assert!(c.is_err());
        let c = load(
            "F = (-3, 0)\nG = (3, 0)\nP = (0, 4)\nQ = (4, 3)\ne = Ellipse(F, G, P)\nh = Hyperbola(F, G, Q)\n",
        )
        .unwrap();
        let e = conic(&c, "e");
        assert_eq!(e.ty, ConicType::Ellipse);
        assert!((e.half_axes[0] - 5.0).abs() < 1e-12);
        assert!((e.half_axes[1] - 4.0).abs() < 1e-12);
        assert!((e.linear_eccentricity - 3.0).abs() < 1e-12);
        let h = conic(&c, "h");
        assert_eq!(h.ty, ConicType::Hyperbola);
        let a = (58.0_f64.sqrt() - 10.0_f64.sqrt()) / 2.0;
        assert!((h.half_axes[0] - a).abs() < 1e-12);
        // a point on the focal axis outside the foci: a double line
        let c = load("F = (-3, 0)\nG = (3, 0)\nQ = (4, 0)\nh = Hyperbola(F, G, Q)\n").unwrap();
        assert_eq!(conic(&c, "h").ty, ConicType::DoubleLine);
    }

    #[test]
    fn coincident_foci_make_a_circle_and_a_short_axis_a_double_line() {
        let c = load("F = (1, 1)\na = 2\ne = Ellipse(F, F, a)\n").unwrap();
        assert_eq!(conic(&c, "e").ty, ConicType::Circle);
        // the semi-major axis is half the focal distance: a double line
        let c = load("F = (-3, 0)\nG = (3, 0)\na = 3\ne = Ellipse(F, G, a)\n").unwrap();
        assert_eq!(conic(&c, "e").ty, ConicType::DoubleLine);
    }

    #[test]
    fn a_parabola_has_its_vertex_between_focus_and_directrix() {
        let c = load("F = (0, 1)\nA = (0, -1)\nB = (1, -1)\ng = Line(A, B)\np = Parabola(F, g)\n")
            .unwrap();
        let d = conic(&c, "p");
        assert_eq!(d.ty, ConicType::Parabola);
        assert!(d.midpoint[0].abs() < 1e-12 && d.midpoint[1].abs() < 1e-12);
        assert!((d.p - 2.0).abs() < 1e-12);
        // the first eigenvector points from the directrix to the focus
        assert!(d.eigenvec[0][1] > 0.9);
    }

    #[test]
    fn a_tiny_dilated_circle_is_a_single_point() {
        // transform-dilate-020: the dilated matrix is classified again
        let c =
            load("A = (1, 1)\nr = 0.00004\nk = Circle(A, r)\nf = 2\nm = Dilate(k, f)\n").unwrap();
        assert_eq!(
            c.value(c.id("m").unwrap()),
            Value::Circle {
                cx: 2.0,
                cy: 2.0,
                r: 0.0
            }
        );
        assert_eq!(conic(&c, "m").ty, ConicType::SinglePoint);
    }

    #[test]
    fn a_conic_follows_its_points_and_undefined_inputs_spread() {
        let script = "A = (1, 0)\nB = (0, 1)\nC = (-1, 0)\nD = (0, -1)\nE = (0.6, 0.8)\nk = Conic(A, B, C, D, E)\nr = 2\nm = Dilate(k, r)\n";
        let mut c = load(script).unwrap();
        // a construction rebuilt from the definitions has the same records
        let mut d = Construction::new();
        for id in c.ids() {
            let def = c.def(id);
            let copy = match def.list() {
                Some(l) => {
                    let ids: Vec<Id> = c.list(l).to_vec();
                    d.add_with_list(c.label(id), &ids, |l| def.with_list(l))
                        .unwrap()
                }
                None => d.add(c.label(id), def).unwrap(),
            };
            assert!(conic_same(&c, &d, id, copy), "{}", c.label(id));
        }
        let e = c.id("E").unwrap();
        let m = c.id("m").unwrap();
        assert_eq!(conic(&c, "m").ty, ConicType::Circle);
        // a point that makes the conic a hyperbola
        c.move_point(e, 3.0, 0.5).unwrap();
        assert_eq!(conic(&c, "k").ty, ConicType::Hyperbola);
        assert_eq!(conic(&c, "m").ty, ConicType::Hyperbola);
        // back: the same visible state as before
        c.move_point(e, 0.6, 0.8).unwrap();
        let fresh = load(script).unwrap();
        assert_eq!(c.value(m), fresh.value(m));
        let (a, b) = (conic(&c, "m"), conic(&fresh, "m"));
        assert_eq!(a.matrix, b.matrix);
        assert_eq!(a.half_axes, b.half_axes);
        assert_eq!(a.eigenvec, b.eigenvec);
        // a factor 0 makes the image undefined, and it comes back
        let r = c.id("r").unwrap();
        c.set_number(r, 0.0).unwrap();
        assert!(!c.value(m).is_defined());
        c.set_number(r, 2.0).unwrap();
        assert_eq!(c.value(m), fresh.value(m));
    }

    fn conic_same(c: &Construction, d: &Construction, a: Id, b: Id) -> bool {
        match (c.conic(a), d.conic(b)) {
            (Some(x), Some(y)) => x.same(&y),
            (None, None) => true,
            _ => false,
        }
    }
}
