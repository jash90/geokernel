// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoConicFivePoints.java
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
//! a conic, among them `Dilate` of a circle, which GeoGebra computes on the
//! circle's matrix and classifies again.
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
//! Not supported: `Conic` of a list or of six coefficients, `Ellipse` and
//! `Hyperbola` with a segment as the axis length, the special case of
//! `Conic` through five points that lie on a parabola by construction, and
//! points with a homogeneous coordinate other than 1 (every point of the
//! kernel has `z = 1`).

use super::conic_nd::{ConicData, ConicType};
use super::{Group, Parents, NUMBER, POINT};
use crate::kernel::{Construction, Error, Id, IdList, Kind, Value};
use crate::script::object;

const VECTOR: &[Kind] = &[Kind::Vector];
const CONIC: &[Kind] = &[Kind::Conic];

/// General conics and the transformations of conics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeneralConicDef {
    /// `Conic(A, B, C, D, E)`: the conic through five points, read from a
    /// stored list.
    FivePoints(IdList),
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
                let phi = c.number(angle).unwrap_or(f64::NAN);
                match center {
                    None => d.rotate(phi, None),
                    Some(q) => d.rotate(phi, Some(pt(q))),
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
                let r = c.number(factor).unwrap_or(f64::NAN);
                let s = match center {
                    None => (0.0, 0.0),
                    Some(q) => pt(q),
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
        }
        d
    }
}

/// `AlgoTransformation.setOutGeo`: the image starts as a copy of the
/// source's record. Returns whether the copy is defined; the transformation
/// stops when it is not.
fn set_out(c: &Construction, src: Id, d: &mut ConicData) -> bool {
    match c.conic(src) {
        Some(s) => {
            *d = s;
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

impl Group for GeneralConicDef {
    fn kind(&self) -> Kind {
        match *self {
            GeneralConicDef::FivePoints(_)
            | GeneralConicDef::FociPoint { .. }
            | GeneralConicDef::FociLength { .. }
            | GeneralConicDef::Parabola { .. } => Kind::Conic,
            GeneralConicDef::Translate { kind, .. }
            | GeneralConicDef::Rotate { kind, .. }
            | GeneralConicDef::Dilate { kind, .. }
            | GeneralConicDef::MirrorPoint { kind, .. }
            | GeneralConicDef::MirrorLine { kind, .. } => kind,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            GeneralConicDef::FivePoints(_) => Parents::none(),
            GeneralConicDef::FociPoint {
                f1, f2, through, ..
            } => Parents::of(&[f1, f2, through]),
            GeneralConicDef::FociLength { f1, f2, length } => Parents::of(&[f1, f2, length]),
            GeneralConicDef::Parabola { focus, directrix } => Parents::of(&[focus, directrix]),
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
        }
    }

    fn list(&self) -> Option<IdList> {
        match *self {
            GeneralConicDef::FivePoints(list) => Some(list),
            _ => None,
        }
    }

    fn with_list(self, list: IdList) -> Self {
        match self {
            GeneralConicDef::FivePoints(_) => GeneralConicDef::FivePoints(list),
            other => other,
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        let kind_of = |kind: Kind| match kind {
            Kind::Circle => Ok(&[Kind::Circle][..]),
            Kind::Conic => Ok(CONIC),
            _ => Err(Error::UnknownObject),
        };
        match *self {
            GeneralConicDef::FivePoints(list) => {
                if c.list(list).len() != 5 {
                    return Err(Error::UnknownObject);
                }
                c.expect_list(list, POINT)
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
                c.expect(second, NUMBER)?;
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
            "Translate" | "Rotate" | "Dilate" | "Mirror" | "Reflect" => {
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

/// Whether the object `id` keeps a conic record of its own: a general
/// conic, or a circle made by a transformation of one.
pub(crate) fn has_record(c: &Construction, id: Id) -> bool {
    matches!(c.def(id), crate::kernel::Def::GeneralConic(_))
}

/// A transformation of a conic, or of a circle when GeoGebra computes the
/// image on the matrix: `Dilate` of any circle, and every transformation of
/// a circle that keeps a record. `None` leaves the line to the other groups.
fn parse_transform(
    c: &Construction,
    func: &str,
    args: &[&str],
) -> Option<Result<GeneralConicDef, String>> {
    let src = c.id(args.first()?)?;
    let kind = c.kind(src);
    let claim = match kind {
        Kind::Conic => true,
        Kind::Circle => func == "Dilate" || has_record(c, src),
        _ => false,
    };
    if !claim {
        return None;
    }
    let arity_ok = match func {
        "Mirror" | "Reflect" | "Translate" => args.len() == 2,
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
            _ => Err(format!(
                "`{}` is a {second_kind}, expected a point or a line (a conic cannot be inverted)",
                args[1]
            )),
        },
        _ => {
            if second_kind != Kind::Number {
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

/// Runs `name = Conic(A, B, C, D, E)`, which needs a stored list; `None`
/// for any other line.
pub(crate) fn exec(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (func, args) = crate::script::split_call(rhs)?;
    if func != "Conic" {
        return None;
    }
    if args.len() != 5 {
        return Some(Err(format!(
            "`Conic` takes five points, not {} argument(s)",
            args.len()
        )));
    }
    let mut ids = Vec::with_capacity(5);
    for arg in &args {
        match point_arg(c, arg) {
            Ok(id) => ids.push(id),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(
        c.add_with_list(name, &ids, |l| {
            crate::kernel::Def::GeneralConic(GeneralConicDef::FivePoints(l))
        })
        .map(Some)
        .map_err(|e| e.to_string()),
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
