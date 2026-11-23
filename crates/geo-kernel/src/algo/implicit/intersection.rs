// Licensed under the EUPL 1.2.

use super::polynomial::{evaluate, roots, substitute};
use super::MAX_OUTPUTS;
use crate::algo::intersect::boundary::BoundaryDef;
use crate::algo::intersect::function_roots::interval_roots;
use crate::algo::{polygon, Group, Parents};
use crate::expr::{IntersectionWork, Raw};
use crate::kernel::locus::HEADLESS_VIEW;
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Error, Id, Kind, Value};

type Point = (f64, f64);
type Points = Vec<Option<Point>>;

// The reference sizes these outputs from the result itself. Use the
// construction's evaluation work and element limits for variable results.
const MAX_RESULT_SLOTS: usize = crate::expr::MAX_ELEMENTS as usize;

const PARTNERS: &[Kind] = &[
    Kind::Line,
    Kind::Segment,
    Kind::Ray,
    Kind::PolyLine,
    Kind::Function,
    Kind::Conic,
    Kind::Circle,
    Kind::Arc,
    Kind::Sector,
    Kind::ImplicitCurve,
];

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
        match (c.kind(a), c.kind(b)) {
            (Kind::ImplicitCurve, kind) if PARTNERS.contains(&kind) => Some([a, b]),
            (kind, Kind::ImplicitCurve) if PARTNERS.contains(&kind) => Some([b, a]),
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
            || (matches!(
                partner,
                Kind::Conic | Kind::Circle | Kind::Arc | Kind::Sector
            ) && c.id(a) != Some(pair[0]))
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
        (self.can_grow() && slot < MAX_RESULT_SLOTS).then_some(Self {
            method: Method::Slot(u32::try_from(slot).ok()?),
            ..self
        })
    }

    pub(in crate::algo) fn labels_indexed(self) -> bool {
        matches!(self.method, Method::Slot(_))
    }

    pub(in crate::algo) fn labels_clipped_slots(self, c: &Construction) -> bool {
        self.labels_indexed() && matches!(c.kind(self.pair[1]), Kind::Segment | Kind::Ray)
    }

    pub(in crate::algo) fn retains_output(self, c: &Construction) -> bool {
        self.labels_indexed()
            && (matches!(
                c.kind(self.pair[1]),
                Kind::Line | Kind::Segment | Kind::Ray | Kind::Function
            ) && !c.value(self.pair[0]).is_defined()
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
        c.expect(self.pair[1], PARTNERS)?;
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
    let partner = c.kind(other);
    let points = match partner {
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
        Kind::Segment | Kind::Ray => {
            let (a, b, _) = c.raw_carrier(other)?;
            let (x, y, max) = match c.value(other) {
                Value::Segment { x1, y1, .. } => (x1, y1, 1.0),
                Value::Ray { x, y, .. } => (x, y, f64::INFINITY),
                _ => return None,
            };
            parametric(matrix, &[x, b], &[y, -a], [0.0, max], &work)?
        }
        Kind::PolyLine => polyline(c, other, matrix, &work)?,
        Kind::Function => graph(c, other, matrix, &work)?,
        Kind::Conic | Kind::Circle | Kind::Arc | Kind::Sector if c.value(other).is_defined() => {
            let conic = c
                .arc(other)
                .map(|part| part.conic)
                .or_else(|| c.conic(other))?;
            let limited = matches!(c.kind(other), Kind::Arc | Kind::Sector);
            super::elimination::conic(matrix, &conic, &work, limited).map_or_else(
                || vec![None],
                |points| points.into_iter().map(Some).collect(),
            )
        }
        Kind::ImplicitCurve => {
            let second = c.implicit_coefficients(other)?;
            if matrix.len() == 1 && second.len() == 1 {
                vec![None]
            } else {
                super::elimination::pair(matrix, second, &work).map_or_else(
                    || vec![None],
                    |points| points.into_iter().map(Some).collect(),
                )
            }
        }
        _ => return None,
    };
    let limit = if matches!(partner, Kind::Function | Kind::PolyLine) {
        MAX_RESULT_SLOTS
    } else {
        MAX_OUTPUTS
    };
    if work.exhausted() || points.len() > limit {
        return Some(vec![None]);
    }
    Some(points)
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
        let points =
            parametric(matrix, &[0.0, 1.0], &y, [left, right], work).unwrap_or_else(|| vec![None]);
        return charge_slots(work, 0, points.len()).then_some(points);
    }
    let f = |x| {
        if !work.charge((matrix.len() * matrix.first().map_or(0, Vec::len)) as u64) {
            return f64::NAN;
        }
        crate::plot::implicit::evaluate(matrix, x, work.sample(c, id, x))
    };
    let roots = interval_roots(&f, left, right, scale, || work.exhausted());
    if !charge_slots(work, 0, roots.len()) {
        return None;
    }
    Some(
        roots
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
    let vertices = c.vertices(id);
    if !work.charge(vertices.size_hint().0 as u64) {
        return None;
    }
    let points: Vec<_> = vertices.collect();
    let mut out = Vec::new();
    for edge in points.windows(2) {
        let [Some((x, y)), Some((end_x, end_y))] = edge else {
            continue;
        };
        if x == end_x && y == end_y {
            continue;
        }
        let mut roots = parametric(matrix, &[*x, end_x - x], &[*y, end_y - y], [0.0, 1.0], work)?;
        roots.retain(Option::is_some);
        if !charge_slots(work, out.len(), roots.len()) {
            return None;
        }
        out.extend(roots);
    }
    Some(out)
}

fn charge_slots(work: &IntersectionWork, current: usize, added: usize) -> bool {
    added <= MAX_RESULT_SLOTS - current && work.charge(added as u64)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script;

    #[test]
    fn an_implicit_graph_grows_to_all_sixty_six_reference_slots() {
        let mut c =
            script::load("k = 0.1\nc: y^3 + y = 0\nf(x) = sin(k*x)\nP = Intersect(c, f)").unwrap();
        let k = c.id("k").unwrap();
        let first = c.id("P_{1}").unwrap();
        let Def::IntersectBoundary(boundary) = c.def(first) else {
            panic!("expected an implicit intersection");
        };
        let pair = [c.id("c").unwrap(), c.id("f").unwrap()];
        c.set_number(k, 13.0).unwrap();
        assert!(boundary.compute(&c).is_some());
        assert_eq!(solve(&c, pair).unwrap().len(), 66);
    }

    #[test]
    fn each_implicit_result_slot_consumes_work() {
        let work = IntersectionWork::new();
        assert!(work.charge(crate::expr::MAX_EVALUATION_WORK - 66));
        assert!(charge_slots(&work, 0, 66));
        assert!(!charge_slots(&work, 66, 1));
        assert!(work.exhausted());
    }

    #[test]
    fn the_implicit_polyline_command_creates_sixty_six_outputs() {
        let c = script::load(
            "c: y^3 + y = 0\nvertices = Sequence((j, (-1)^j), j, 0, 66)\npath = PolyLine(vertices)\nP = Intersect(c, path)",
        ).unwrap();
        for slot in 1..=66 {
            let output = c.id(&format!("P_{{{slot}}}")).unwrap();
            let (x, y) = c.point(output).unwrap();
            assert!((x - (f64::from(slot) - 0.5)).abs() < 1e-9 && y.abs() < 1e-9);
        }
        assert!(c.id("P_{67}").is_none());
    }

    #[test]
    fn translated_degree_seventy_two_elimination_finds_the_repeated_root() {
        let c =
            script::load("c: x^8 + x - (y - 1)^9 = 0\nd: x - (y - 1)^9 = 0\nP = Intersect(c, d)")
                .unwrap();
        assert_eq!(c.point(c.id("P_{1}").unwrap()), Some((0.0, 1.0)));
    }

    #[test]
    fn degree_seventy_two_elimination_keeps_the_reference_origin() {
        let c = script::load("c: x^8 - y = 0\nd: x - y^9 = 0\nP = Intersect(c, d)").unwrap();
        let p = c.id("P").or_else(|| c.id("P_{1}")).unwrap();
        assert_eq!(c.point(p), Some((0.0, 0.0)));
    }
}
