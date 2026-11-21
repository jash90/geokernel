// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra).
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Angle commands and their drawing geometry.
//!
//! Rules follow GeoGebra's CmdAngle, AlgoAnglePoints, AlgoAngleLines,
//! AlgoAngleVector, AlgoAngleVectors, AlgoAngleConic and DrawAngle at
//! 112919f2d450ec7430a5cd3b65dba226fdc43b74 (EUPL 1.2).

use crate::algo::polygon::PolygonDef;
use crate::algo::{self, Group, Parents, NUMBER, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI, TAU};

/// The interval applied to an angle's raw counter-clockwise sweep.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AngleStyle {
    /// [0, 2π], the default for measurements.
    #[default]
    Anticlockwise,
    /// [0, π].
    NotReflex,
    /// [π, 2π].
    Reflex,
    /// A numeric angle such as an angle literal or a sum of angles.
    Unbounded,
}

impl AngleStyle {
    pub const fn interval(self) -> (f64, f64) {
        match self {
            Self::Anticlockwise => (0.0, TAU),
            Self::NotReflex => (0.0, PI),
            Self::Reflex => (PI, TAU),
            Self::Unbounded => (f64::NEG_INFINITY, f64::INFINITY),
        }
    }

    pub(crate) fn value(self, raw: f64) -> f64 {
        if self == Self::Unbounded {
            return raw;
        }
        let bounded = normalize(raw);
        if self.reverses(bounded) {
            TAU - bounded
        } else {
            bounded
        }
    }

    fn reverses(self, bounded: f64) -> bool {
        (self == Self::NotReflex && bounded > PI) || (self == Self::Reflex && bounded < PI)
    }
}

const DIRECTION: &[Kind] = &[Kind::Point, Kind::Vector];
const CONIC: &[Kind] = &[Kind::Circle, Kind::Conic, Kind::Arc, Kind::Sector];
pub const DEFAULT_ARC_SIZE: f64 = 30.0;

pub(crate) fn normalize(value: f64) -> f64 {
    if value > STANDARD_PRECISION && value < TAU {
        return value;
    }
    let remainder = value % TAU;
    if remainder.abs() < STANDARD_PRECISION {
        if value < 1.0 {
            0.0
        } else {
            TAU
        }
    } else if remainder < 0.0 {
        remainder + TAU
    } else {
        remainder
    }
}

type XY = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Radians {
    Literal(f64),
    Object(Id),
}

impl Radians {
    fn value(self, c: &Construction) -> Option<f64> {
        match self {
            Self::Literal(v) => Some(v),
            Self::Object(id) => c.number(id),
        }
    }

    fn check(self, c: &Construction) -> Result<(), Error> {
        match self {
            Self::Literal(v) if v.is_finite() => Ok(()),
            Self::Literal(_) => Err(Error::NotFinite),
            Self::Object(id) => c.expect(id, NUMBER),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AngleSource {
    Free(f64),
    Numeric(Radians),
    Copy(Id),
    Direction(Id),
    Directions(Id, Id),
    Lines(Id, Id),
    Conic(Id),
    Points(Id, Id, Id),
    Polygon {
        poly: Id,
        index: u32,
        interior: bool,
    },
    Fixed {
        from: Id,
        center: Id,
        by: Radians,
        point: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleDef {
    pub(crate) source: AngleSource,
    pub(crate) style: AngleStyle,
}

impl AngleDef {
    pub(crate) fn new(source: AngleSource) -> Self {
        Self {
            source,
            style: AngleStyle::Anticlockwise,
        }
    }

    pub(crate) fn is_free(self) -> bool {
        matches!(self.source, AngleSource::Free(_))
    }
}

impl Group for AngleDef {
    fn kind(&self) -> Kind {
        match self.source {
            AngleSource::Fixed { point: true, .. } => Kind::Point,
            _ => Kind::Angle,
        }
    }

    fn parents(&self) -> Parents {
        match self.source {
            AngleSource::Free(_) | AngleSource::Numeric(Radians::Literal(_)) => Parents::none(),
            AngleSource::Numeric(Radians::Object(id))
            | AngleSource::Copy(id)
            | AngleSource::Direction(id)
            | AngleSource::Conic(id) => Parents::of(&[id]),
            AngleSource::Directions(a, b) | AngleSource::Lines(a, b) => Parents::of(&[a, b]),
            AngleSource::Points(a, b, d) => Parents::of(&[a, b, d]),
            AngleSource::Polygon { poly, .. } => Parents::of(&[poly]),
            AngleSource::Fixed {
                from,
                center,
                by: Radians::Object(id),
                ..
            } => Parents::of(&[from, center, id]),
            AngleSource::Fixed { from, center, .. } => Parents::of(&[from, center]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match self.source {
            AngleSource::Free(v) => Radians::Literal(v).check(c),
            AngleSource::Numeric(v) => v.check(c),
            AngleSource::Copy(id) => c.expect(id, &[Kind::Angle]),
            AngleSource::Direction(id) => c.expect(id, DIRECTION),
            AngleSource::Directions(a, b) => {
                c.expect(a, DIRECTION)?;
                c.expect(b, DIRECTION)
            }
            AngleSource::Lines(a, b) => {
                c.expect(a, CARRIERS)?;
                c.expect(b, CARRIERS)
            }
            AngleSource::Conic(id) => c.expect(id, CONIC),
            AngleSource::Points(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            AngleSource::Polygon { poly, .. } => c.expect(poly, &[Kind::Polygon]),
            AngleSource::Fixed {
                from, center, by, ..
            } => {
                c.expect(from, POINT)?;
                c.expect(center, POINT)?;
                by.check(c)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        if let AngleSource::Fixed {
            from,
            center,
            by,
            point: true,
        } = self.source
        {
            let (x, y) = rotate(c.point(from)?, c.point(center)?, by.value(c)?);
            return Some(Value::Point { x, y });
        }
        let raw = self.source.raw(c)?;
        Some(Value::Number(self.style.value(raw)))
    }

    fn parse(c: &Construction, command: &str, args: &[&str]) -> Option<Result<Self, String>> {
        (command == "Angle").then(|| parse(c, args))
    }
}

/// Sources whose public value can stay equal while the geometry read by an
/// angle changes: the vertices of a polygon and the defining points of
/// lines whose raw normals are kept even when the lines are undefined.
pub(crate) fn hidden_parents(c: &Construction, def: &Def) -> Vec<Id> {
    match *def {
        Def::AngleObject(AngleDef {
            source: AngleSource::Polygon { poly, .. },
            ..
        }) => algo::polygon::inputs(c, poly),
        Def::AngleObject(AngleDef {
            source: AngleSource::Lines(a, b),
            ..
        }) => {
            let mut parents = Vec::new();
            for mut line in [a, b] {
                loop {
                    match c.def(line) {
                        Def::Line(p, q) => {
                            parents.extend([p, q]);
                            break;
                        }
                        Def::Parallel { through, to } | Def::Perpendicular { through, to } => {
                            parents.push(through);
                            line = to;
                        }
                        _ => break,
                    }
                }
            }
            parents
        }
        _ => Vec::new(),
    }
}

const CARRIERS: &[Kind] = &[Kind::Line, Kind::Segment, Kind::Ray];

fn radians(c: &Construction, arg: &str) -> Result<Radians, String> {
    if let Some(id) = c.id(arg) {
        c.expect(id, NUMBER).map_err(|e| e.to_string())?;
        return Ok(Radians::Object(id));
    }
    crate::expr::constant_number(c, arg)
        .map(|(v, _)| Radians::Literal(v))
        .ok_or_else(|| format!("expected a number, got `{arg}`"))
}

pub(crate) fn fixed(c: &Construction, args: &[&str]) -> Option<Result<[AngleDef; 2], String>> {
    let [a, b, by] = *args else {
        return None;
    };
    if c.id(by).is_some_and(|id| c.kind(id) == Kind::Point) {
        return None;
    }
    let (from, center) = (c.id(a)?, c.id(b)?);
    if c.kind(from) != Kind::Point || c.kind(center) != Kind::Point {
        return None;
    }
    Some(radians(c, by).map(|by| {
        [false, true].map(|point| {
            AngleDef::new(AngleSource::Fixed {
                from,
                center,
                by,
                point,
            })
        })
    }))
}

fn parse(c: &Construction, args: &[&str]) -> Result<AngleDef, String> {
    let id = |arg: &str| crate::script::object(c, arg);
    let source = match *args {
        [a] if !a.is_empty() => {
            if let Some(i) = c.id(a) {
                match c.kind(i) {
                    Kind::Angle => {
                        return Ok(AngleDef {
                            source: AngleSource::Copy(i),
                            style: AngleStyle::Unbounded,
                        })
                    }
                    Kind::Point | Kind::Vector => AngleSource::Direction(i),
                    kind if CONIC.contains(&kind) => AngleSource::Conic(i),
                    Kind::Number => AngleSource::Numeric(Radians::Object(i)),
                    kind => return Err(format!("Angle does not take a {kind}")),
                }
            } else {
                let (value, angle) = crate::expr::constant_number(c, a)
                    .ok_or_else(|| format!("expected a number, got `{a}`"))?;
                if angle {
                    return Ok(AngleDef {
                        source: AngleSource::Free(value),
                        style: AngleStyle::Unbounded,
                    });
                }
                AngleSource::Numeric(Radians::Literal(value))
            }
        }
        [a, b] => {
            let (a, b) = (id(a)?, id(b)?);
            let (ka, kb) = (c.kind(a), c.kind(b));
            if DIRECTION.contains(&ka) && DIRECTION.contains(&kb) {
                AngleSource::Directions(a, b)
            } else if CARRIERS.contains(&ka) && CARRIERS.contains(&kb) {
                AngleSource::Lines(a, b)
            } else {
                return Err(format!("Angle does not take a {ka} and a {kb}"));
            }
        }
        _ => {
            let n = if args == [""] { 0 } else { args.len() };
            return Err(format!("`Angle` does not take {n} argument(s)"));
        }
    };
    Ok(AngleDef::new(source))
}

fn sub(a: XY, b: XY) -> XY {
    (a.0 - b.0, a.1 - b.1)
}

fn oriented(a: XY, b: XY) -> f64 {
    (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1)
}

fn direction(c: &Construction, id: Id) -> Option<XY> {
    match c.value(id) {
        Value::Point { x, y } | Value::Vector { x, y } => Some((x, y)),
        _ => None,
    }
}

fn origin(c: &Construction, id: Id) -> Option<XY> {
    if c.kind(id) == Kind::Vector {
        algo::vector::start(c, id)
    } else {
        Some((0.0, 0.0))
    }
}

/// Read the raw normal even when a finite construction produced a line
/// with a zero normal. The angle algorithm still takes the atan2 of its
/// dot product and determinant, including the signs of zero.
fn line_normal(c: &Construction, mut id: Id) -> Option<XY> {
    let mut derived = Vec::new();
    let (mut x, mut y) = loop {
        if let Some((a, b, _)) = c.raw_carrier(id) {
            break (a, b);
        }
        match c.def(id) {
            Def::Line(a, b) => {
                let (x, y, _) = algo::join(c.point(a)?, c.point(b)?);
                break (x, y);
            }
            Def::Parallel { through, to } | Def::Perpendicular { through, to } => {
                let p = c.point(through)?;
                let perpendicular = matches!(c.def(id), Def::Perpendicular { .. });
                derived.push((p, perpendicular));
                id = to;
            }
            _ => return None,
        }
    };
    for ((px, py), perpendicular) in derived.into_iter().rev() {
        let (a, b) = if perpendicular { (x, y) } else { (y, -x) };
        (x, y) = (py * 0.0 - b, a - px * 0.0);
    }
    Some((x, y))
}

fn rotate(p: XY, q: XY, by: f64) -> XY {
    let cos = by.cos();
    let cos = if cos.abs() < 1e-16 { 0.0 } else { cos };
    let sin = by.sin();
    (
        (p.0 - q.0) * cos + (q.1 - p.1) * sin + q.0,
        (p.0 - q.0) * sin + (p.1 - q.1) * cos + q.1,
    )
}

/// `GeoVec3D.linDep`: normalize all three coefficients before comparing
/// the cross products with the inclusive `DoubleUtil.isEqual` bounds.
fn linearly_dependent(g: (f64, f64, f64), h: (f64, f64, f64)) -> bool {
    let unit = |(a, b, c): (f64, f64, f64)| {
        let norm = a.abs().max(b.abs()).max(c.abs());
        (a / norm, b / norm, c / norm)
    };
    let (a, b, c) = unit(g);
    let (d, e, f) = unit(h);
    let equal =
        |x: f64, y: f64| x == y || (x - STANDARD_PRECISION <= y && y <= x + STANDARD_PRECISION);
    equal(a * e, d * b) && equal(c * e, f * b) && equal(a * f, d * c)
}

impl AngleSource {
    pub(crate) fn has_geometry(self) -> bool {
        !matches!(self, Self::Free(_) | Self::Numeric(_) | Self::Copy(_))
    }

    pub(crate) fn raw(self, c: &Construction) -> Option<f64> {
        match self {
            Self::Free(v) => Some(v),
            Self::Numeric(input) => input.value(c),
            Self::Copy(id) => c.number(id),
            Self::Direction(id) => {
                let (x, y) = direction(c, id)?;
                Some(y.atan2(x))
            }
            Self::Directions(a, b) => Some(oriented(direction(c, a)?, direction(c, b)?)),
            Self::Lines(a, b) => Some(oriented(line_normal(c, a)?, line_normal(c, b)?)),
            Self::Conic(id) => {
                let [x, y] = algo::conic_object(c, id)?.eigenvec[0];
                Some(y.atan2(x))
            }
            _ => {
                let [a, b, d] = self.points(c)?;
                algo::measure::angle_of_points(a, b, d)
            }
        }
    }

    pub(crate) fn points(self, c: &Construction) -> Option<[XY; 3]> {
        match self {
            Self::Points(a, b, d) => Some([c.point(a)?, c.point(b)?, c.point(d)?]),
            Self::Polygon {
                poly,
                index,
                interior,
            } => {
                let at = index as usize;
                let vertices = c.vertices(poly);
                let n = vertices.size_hint().0;
                if at >= n {
                    return None;
                }
                let get = |i| {
                    if let Def::Polygon(PolygonDef::Polygon(list)) = c.def(poly) {
                        return c.point(*c.list(list).get(i)?);
                    }
                    match (PolygonDef::VertexAt { poly, k: i as u32 }).compute(c)? {
                        Value::Point { x, y } => Some((x, y)),
                        _ => None,
                    }
                };
                let ccw = matches!(c.value(poly), Value::Polygon { area } if area > 0.0);
                let dir = if !interior || ccw { 1 } else { n - 1 };
                Some([get((at + dir) % n)?, get(at)?, get((at + n - dir) % n)?])
            }
            Self::Fixed {
                from,
                center,
                by,
                point: false,
            } => {
                let (p, q) = (c.point(from)?, c.point(center)?);
                Some([p, q, rotate(p, q, by.value(c)?)])
            }
            _ => None,
        }
    }

    fn geometry(self, c: &Construction) -> Option<DrawingBasis> {
        let (center, leg) = match self {
            Self::Direction(id) => (origin(c, id)?, (1.0, 0.0)),
            Self::Directions(a, _) => (origin(c, a)?, direction(c, a)?),
            Self::Conic(id) => (algo::conic_object(c, id)?.midpoint.into(), (1.0, 0.0)),
            Self::Lines(g, h) => {
                let g_line @ (a, b, k) = c.raw_carrier(g)?;
                let h_line @ (d, e, l) = c.raw_carrier(h)?;
                let center = if linearly_dependent(g_line, h_line) {
                    let start = |id| match c.def(id) {
                        Def::Line(p, _)
                        | Def::Parallel { through: p, .. }
                        | Def::Perpendicular { through: p, .. } => c.point(p),
                        _ => match c.value(id) {
                            Value::Ray { x, y, .. } => Some((x, y)),
                            Value::Segment { x1, y1, .. } => Some((x1, y1)),
                            _ => None,
                        },
                    };
                    start(h).or_else(|| start(g))?
                } else {
                    let det = a * e - b * d;
                    if det == 0.0 {
                        return None;
                    }
                    ((b * l - k * e) / det, (k * d - a * l) / det)
                };
                (center, (b, -a))
            }
            _ => {
                let [a, center, b] = self.points(c)?;
                let leg = sub(a, center);
                return Some(DrawingBasis {
                    center,
                    leg,
                    arms: Some([leg, sub(b, center)]),
                });
            }
        };
        Some(DrawingBasis {
            center,
            leg,
            arms: None,
        })
    }

    pub(crate) fn drawing(
        self,
        c: &Construction,
        sweep: f64,
        style: AngleStyle,
    ) -> Option<AngleGeometry> {
        if sweep.abs() < STANDARD_PRECISION {
            return None;
        }
        let DrawingBasis { center, leg, arms } = self.geometry(c)?;
        let mut start = leg.1.atan2(leg.0);
        if matches!(style, AngleStyle::NotReflex | AngleStyle::Reflex)
            && style.reverses(normalize(self.raw(c)?))
        {
            start -= sweep;
        }
        (center.0.is_finite() && center.1.is_finite() && start.is_finite()).then_some(
            AngleGeometry {
                center,
                start,
                sweep,
                arms,
            },
        )
    }
}

struct DrawingBasis {
    center: XY,
    leg: XY,
    arms: Option<[XY; 2]>,
}

/// An angle sector before the view transform. Its vertex stays in world
/// coordinates, while the radius is capped in pixels at each zoom level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleGeometry {
    pub center: XY,
    pub start: f64,
    pub sweep: f64,
    arms: Option<[XY; 2]>,
}

impl AngleGeometry {
    pub fn radius(self, scale: f64) -> f64 {
        let Some([a, b]) = self.arms else {
            return DEFAULT_ARC_SIZE;
        };
        let shortest_squared = (a.0 * a.0 + a.1 * a.1).min(b.0 * b.0 + b.1 * b.1);
        (0.5 * shortest_squared.sqrt() * scale)
            .floor()
            .min(DEFAULT_ARC_SIZE)
    }

    pub fn is_right(self) -> bool {
        (self.sweep - FRAC_PI_2).abs() < STANDARD_PRECISION
    }

    pub fn contains(self, (x, y): XY, radius: f64) -> bool {
        if radius <= 0.0 || x * x + y * y >= radius * radius {
            return false;
        }
        if self.is_right() {
            let (sin, cos) = self.start.sin_cos();
            let side = radius * FRAC_1_SQRT_2;
            let (u, v) = (x * cos + y * sin, -x * sin + y * cos);
            return u > 0.0 && u < side && v > 0.0 && v < side;
        }
        (y.atan2(x) - self.start).rem_euclid(TAU) < self.sweep
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{exec, load};

    #[test]
    fn dependent_lines_draw_at_the_second_line_start() {
        let c = load(
            "A = (0.011099999999999999, 0.025899999999999996)\n\
             B = (0.40109999999999996, 0.9359)\n\
             C = (-0.25889999999999996, -0.6041)\n\
             D = (0.6411, 1.4959)\n\
             f = Line(A, B)\ng = Line(D, C)\nang = Angle(f, g)\n",
        )
        .unwrap();
        let angle = c.angle_geometry(c.id("ang").unwrap()).unwrap();
        assert!((angle.sweep - PI).abs() < 1e-9);
        assert_eq!(Some(angle.center), c.point(c.id("D").unwrap()));
    }

    #[test]
    fn dependent_lines_use_the_first_start_when_the_second_is_a_polar() {
        let c = load(
            "A = (0.35, 1)\nB = (0.2, 2)\nO = (0, 0)\nT = (1, 0)\nP = (2, 0.3)\n\
             f = Line(A, B)\nk = Circle(O, T)\ng = Polar(P, k)\nang = Angle(f, g)\n",
        )
        .unwrap();
        let angle = c.angle_geometry(c.id("ang").unwrap()).unwrap();
        assert!((angle.sweep - PI).abs() < 1e-9);
        assert_eq!(Some(angle.center), c.point(c.id("A").unwrap()));
    }

    #[test]
    fn fixed_angle_outputs_can_be_named_explicitly_and_update_together() {
        let mut c =
            load("P = (2, 0)\nO = (0, 0)\nturn = 90°\n{a, Q} = Angle(P, O, turn)\n").unwrap();
        let (a, q, turn) = (
            c.id("a").unwrap(),
            c.id("Q").unwrap(),
            c.id("turn").unwrap(),
        );
        assert_eq!(
            (c.kind(a), c.kind(q), c.kind(turn)),
            (Kind::Angle, Kind::Point, Kind::Angle)
        );
        assert_eq!(c.point(q), Some((0.0, 2.0)));
        assert_eq!(c.number(a), Some(FRAC_PI_2));
        c.set_number(turn, -FRAC_PI_2).unwrap();
        assert_eq!(c.point(q), Some((0.0, -2.0)));
        assert_eq!(c.number(a), Some(3.0 * FRAC_PI_2));
        assert_eq!(exec(&mut c, "alias = Angle(a)").unwrap(), None);
        assert!(c.id("alias").is_none());
    }

    #[test]
    fn changing_interval_uses_the_original_orientation_and_recomputes_dependents() {
        let mut c =
            load("P = (2, 0)\nO = (0, 0)\nQ = (0, -1)\na = Angle(P, O, Q)\nn = sin(a)\n").unwrap();
        let (a, q, n) = (c.id("a").unwrap(), c.id("Q").unwrap(), c.id("n").unwrap());
        assert_eq!(c.angle_style(a).unwrap().interval(), (0.0, TAU));
        assert_eq!(c.number(a), Some(3.0 * FRAC_PI_2));
        assert_eq!(c.set_angle_style(a, AngleStyle::NotReflex).unwrap(), 1);
        assert_eq!(c.number(n), Some(1.0));
        assert_eq!(c.angle_geometry(a).unwrap().start, -FRAC_PI_2);
        c.move_point(q, 0.0, 1.0).unwrap();
        c.set_angle_style(a, AngleStyle::Reflex).unwrap();
        assert_eq!(c.number(a), Some(3.0 * FRAC_PI_2));
        assert_eq!(c.number(n), Some(-1.0));
        assert_eq!(
            c.angle_geometry(a).unwrap().start.rem_euclid(TAU),
            FRAC_PI_2
        );
    }

    #[test]
    fn numeric_angle_expressions_preserve_their_interval_when_rebuilt() {
        let mut c = load("a = 150°\nb = 2 * a\nsn = sin(b)\n").unwrap();
        let (a, b, z) = (c.id("a").unwrap(), c.id("b").unwrap(), c.id("sn").unwrap());
        assert_eq!(c.angle_style(b), Some(AngleStyle::Unbounded));
        c.set_angle_style(b, AngleStyle::NotReflex).unwrap();
        assert!((c.number(b).unwrap() - PI / 3.0).abs() < 1e-12);
        assert!(c.number(z).unwrap() > 0.0);
        let expression = c.expression(b).unwrap();
        let copy = c.add_expression("copy", &expression).unwrap();
        assert_eq!(c.angle_style(copy), Some(AngleStyle::NotReflex));
        c.set_number(a, PI * 3.0 / 4.0).unwrap();
        assert_eq!(c.number(copy), Some(FRAC_PI_2));
        assert_eq!(c.number(z), Some(1.0));
    }

    #[test]
    fn angle_kind_preserves_compact_kernel_storage() {
        assert_eq!(std::mem::size_of::<Value>(), 48);
        assert_eq!(std::mem::size_of::<Def>(), 48);
        assert_eq!(std::mem::size_of::<Kind>(), 1);
    }
}
