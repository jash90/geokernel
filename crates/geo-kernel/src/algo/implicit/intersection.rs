// Licensed under the EUPL 1.2.

use super::polynomial::{evaluate, roots, substitute};
use crate::algo::intersect::boundary::BoundaryDef;
use crate::algo::intersect::function_roots::interval_roots;
use crate::algo::{polygon, Group, Parents};
use crate::expr::{IntersectionWork, Raw};
use crate::kernel::locus::HEADLESS_VIEW;
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Error, Id, Kind, Value};

type Point = (f64, f64);
type Points = Vec<Option<Point>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::algo) struct Intersection {
    pair: [Id; 2],
    method: Method,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Slot(u32),
    ConstantIndex(u32),
    Index(u32),
}

impl Intersection {
    fn pair(c: &Construction, first: &str, second: &str) -> Option<[Id; 2]> {
        let (a, b) = (c.id(first)?, c.id(second)?);
        let partner = |kind| {
            matches!(
                kind,
                Kind::Line | Kind::PolyLine | Kind::Function | Kind::Conic | Kind::Circle
            )
        };
        match (c.kind(a), c.kind(b)) {
            (Kind::ImplicitCurve, Kind::ImplicitCurve) => Some([a, b]),
            (Kind::ImplicitCurve, kind) if partner(kind) => Some([a, b]),
            (kind, Kind::ImplicitCurve) if partner(kind) => Some([b, a]),
            _ => None,
        }
    }

    pub(in crate::algo) fn multi(
        c: &Construction,
        command: &str,
        args: &[&str],
    ) -> Option<Vec<Self>> {
        let ("Intersect", [a, b]) = (command, args) else {
            return None;
        };
        let pair = Self::pair(c, a, b)?;
        let def = Self {
            pair,
            method: Method::Slot(0),
        };
        let count = def
            .count(c)
            .max(usize::from(c.kind(pair[1]) == Kind::PolyLine));
        Some((0..count).filter_map(|slot| def.slot(slot)).collect())
    }

    pub(in crate::algo) fn exec_selected(
        c: &mut Construction,
        label: &str,
        args: &[&str],
    ) -> Option<Result<Option<Id>, String>> {
        let [a, b, tail @ ..] = args else { return None };
        let pair = Self::pair(c, a, b)?;
        if tail.is_empty() {
            let defs = Self::multi(c, "Intersect", args)?;
            if defs.is_empty() {
                return Some(Ok(None));
            }
            let defs = defs
                .into_iter()
                .map(|d| Def::IntersectBoundary(BoundaryDef::implicit(d)))
                .collect();
            return Some(crate::script::add_outputs(c, label, defs).map(Some));
        }
        let [selector] = tail else { return None };
        let partner = c.kind(pair[1]);
        if matches!(partner, Kind::PolyLine | Kind::ImplicitCurve)
            || (matches!(partner, Kind::Conic | Kind::Circle) && c.id(a) != Some(pair[0]))
        {
            return Some(Err("no indexed intersection for those arguments".into()));
        }
        if partner == Kind::Function
            && polynomial_graph(c, pair[1], &IntersectionWork::new()).is_none()
        {
            return Some(Err(
                "an indexed implicit intersection needs a polynomial graph".into(),
            ));
        }
        let checkpoint = c.checkpoint();
        let result = (|| {
            let method = if let Some(n) = crate::expr::plain_number(selector) {
                Method::ConstantIndex(index(n))
            } else {
                let (selector, kind) = c.store_intersection_argument(selector)?;
                if !kind.is_number() {
                    return Err("an intersection index must be a number".into());
                }
                Method::Index(selector)
            };
            let def = Def::IntersectBoundary(BoundaryDef::implicit(Self { pair, method }));
            c.add(label, def).map(Some).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            c.discard_since(checkpoint);
        }
        Some(result)
    }

    pub(in crate::algo) fn can_grow(self) -> bool {
        self.method == Method::Slot(0)
    }

    pub(in crate::algo) fn slot(self, slot: usize) -> Option<Self> {
        self.can_grow().then_some(Self {
            method: Method::Slot(u32::try_from(slot).ok()?),
            ..self
        })
    }

    pub(in crate::algo) fn labels_indexed(self) -> bool {
        matches!(self.method, Method::Slot(_))
    }

    pub(in crate::algo) fn retains_output(self, c: &Construction) -> bool {
        self.labels_indexed()
            && (matches!(c.kind(self.pair[1]), Kind::Line | Kind::Function)
                && !c.value(self.pair[0]).is_defined()
                || c.kind(self.pair[1]) == Kind::Function && !c.value(self.pair[1]).is_defined())
    }

    pub(in crate::algo) fn count(self, c: &Construction) -> usize {
        self.with_points(c, <[Option<Point>]>::len)
    }

    pub(in crate::algo) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        let mut parents = if c.kind(self.pair[1]) == Kind::PolyLine {
            polygon::inputs(c, self.pair[1])
        } else {
            Vec::new()
        };
        if let Method::Index(index) = self.method {
            if let Some(ids) = c.intersection_argument_parents(index) {
                parents.extend_from_slice(ids);
            }
        }
        parents
    }

    fn with_points<T>(self, c: &Construction, read: impl FnOnce(&[Option<Point>]) -> T) -> T {
        c.function_roots
            .implicit(self.pair, || solve(c, self.pair).unwrap_or_default(), read)
    }
}

impl Group for Intersection {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        Parents::of(&self.pair)
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.pair[0], &[Kind::ImplicitCurve])?;
        c.expect(
            self.pair[1],
            &[
                Kind::Line,
                Kind::PolyLine,
                Kind::Function,
                Kind::Conic,
                Kind::Circle,
                Kind::ImplicitCurve,
            ],
        )?;
        if let Method::Index(index) = self.method {
            c.intersection_argument_parents(index)
                .ok_or(Error::UnknownObject)?;
        }
        Ok(())
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let slot = match self.method {
            Method::Slot(slot) | Method::ConstantIndex(slot) => slot,
            Method::Index(arg) => {
                let Raw::Num(n) = c.intersection_argument_value(arg)? else {
                    return None;
                };
                index(n)
            }
        };
        let (x, y) = self.with_points(c, |points| points.get(slot as usize).copied().flatten())?;
        Some(Value::Point { x, y })
    }

    fn parse(_: &Construction, _: &str, _: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

fn index(n: f64) -> u32 {
    (n as i32).wrapping_sub(1).max(0) as u32
}

fn solve(c: &Construction, [first, other]: [Id; 2]) -> Option<Points> {
    let matrix = c.implicit_coefficients(first)?;
    let work = IntersectionWork::new();
    let points = match c.kind(other) {
        Kind::Line => {
            let (a, b, d) = c.raw_line(other)?;
            let (x, y) = match c.def(other) {
                Def::Line(start, _)
                | Def::Parallel { through: start, .. }
                | Def::Perpendicular { through: start, .. } => c.point(start)?,
                _ if a.abs() > b.abs() => (-d / a, 0.0),
                _ => (0.0, -d / b),
            };
            parametric(
                matrix,
                &[x, b],
                &[y, -a],
                [f64::NEG_INFINITY, f64::INFINITY],
                &work,
            )?
        }
        Kind::PolyLine => polyline(c, other, matrix, &work)?,
        Kind::Function => graph(c, other, matrix, &work)?,
        Kind::Conic | Kind::Circle if c.value(other).is_defined() => {
            super::elimination::conic(matrix, &c.conic(other)?, &work)
                .into_iter()
                .map(Some)
                .collect()
        }
        Kind::ImplicitCurve => {
            super::elimination::pair(matrix, c.implicit_coefficients(other)?, &work)?
                .into_iter()
                .map(Some)
                .collect()
        }
        _ => return None,
    };
    (!work.exhausted()).then_some(points)
}

fn polynomial_graph(c: &Construction, id: Id, work: &IntersectionWork) -> Option<Vec<f64>> {
    let (mut numerator, denominator) = work.fraction(c, id)?;
    let &[scale] = denominator.as_slice() else {
        return None;
    };
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    for v in &mut numerator {
        *v /= scale;
    }
    Some(numerator)
}

fn graph(c: &Construction, id: Id, matrix: &[Vec<f64>], work: &IntersectionWork) -> Option<Points> {
    if !c.value(id).is_defined() {
        return None;
    }
    let [mut left, mut right, _, _, scale, _] = c.locus_view().unwrap_or(HEADLESS_VIEW);
    if let Some(bounds) = crate::expr::function_interval(c, id) {
        let [lo, hi] = bounds?;
        left = left.max(lo);
        right = right.min(hi);
    }
    if right < left {
        return None;
    }
    if let Some(y) = polynomial_graph(c, id, work) {
        return parametric(matrix, &[0.0, 1.0], &y, [left, right], work);
    }
    let f = |x| {
        if !work.charge((matrix.len() * matrix.first().map_or(0, Vec::len)) as u64) {
            return f64::NAN;
        }
        crate::plot::implicit::evaluate(matrix, x, work.sample(c, id, x))
    };
    Some(
        interval_roots(&f, left, right, scale, || work.exhausted())
            .into_iter()
            .map(|x| finite(x, work.sample(c, id, x)))
            .collect(),
    )
}

fn polyline(
    c: &Construction,
    id: Id,
    matrix: &[Vec<f64>],
    work: &IntersectionWork,
) -> Option<Points> {
    const MAX_VERTICES: usize = 512;
    let points: Vec<_> = c.vertices(id).take(MAX_VERTICES + 1).collect();
    if points.len() > MAX_VERTICES {
        return None;
    }
    let mut out = Vec::new();
    for edge in points.windows(2) {
        let [Some((x, y)), Some((end_x, end_y))] = edge else {
            continue;
        };
        if x == end_x && y == end_y {
            continue;
        }
        out.extend(
            parametric(matrix, &[*x, end_x - x], &[*y, end_y - y], [0.0, 1.0], work)?
                .into_iter()
                .flatten()
                .map(Some),
        );
    }
    Some(out)
}

fn parametric(
    matrix: &[Vec<f64>],
    x: &[f64],
    y: &[f64],
    [lo, hi]: [f64; 2],
    work: &IntersectionWork,
) -> Option<Points> {
    let residual = substitute(matrix, x, y, work)?;
    let mut points = Points::new();
    for t in roots(&residual, work) {
        if t < lo - STANDARD_PRECISION || t > hi + STANDARD_PRECISION {
            points.push(None);
        } else if let Some(p) = finite(evaluate(x, t), evaluate(y, t)) {
            if points
                .iter()
                .flatten()
                .all(|&(a, b)| (a - p.0).powi(2) + (b - p.1).powi(2) >= STANDARD_PRECISION)
            {
                points.push(Some(p));
            }
        }
    }
    Some(points)
}

fn finite(x: f64, y: f64) -> Option<Point> {
    (x.is_finite() && y.is_finite()).then_some((x, y))
}
