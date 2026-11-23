// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectFunctions.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectFunctionsNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectFunctionLineNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolynomialConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSimpleRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIntersect.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::boundary::Carrier;
use super::conics::graph_roots;
use super::function_roots::{check_root, interval_roots, root_near};
use crate::algo::conic_nd::ConicType;
use crate::algo::{curve, Group, Parents, CARRIERS, CONIC_OBJECTS};
use crate::expr::{IntersectionWork, Raw};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const FUNCTION: &[Kind] = &[Kind::Function];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct FunctionIntersection {
    function: Id,
    other: Id,
    method: Method,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Method {
    Polynomial(u32),
    Indexed(u32),
    ConstantIndex(u32),
    Seed {
        point: u32,
        line_first: bool,
    },
    /// The most recent successful x coordinate, stored as float bits.
    Automatic(u64),
    Conic(u32),
    Interval {
        left: u32,
        right: u32,
        slot: u32,
    },
}

type Point = (f64, f64);
type Vertices = Arc<[Point]>;

/// Root lists shared by output slots until a dependency update or rollback.
#[derive(Default)]
pub(crate) struct FunctionRootCache {
    graphs: Mutex<HashMap<FunctionIntersection, Vec<Point>>>,
    cartesian_pairs: Mutex<HashMap<[Id; 2], super::curves::CurveRoots>>,
    inequalities: Mutex<HashMap<Id, Vertices>>,
    implicit_pairs: Mutex<HashMap<[Id; 2], super::curves::CurveRoots>>,
    implicit_tangents: Mutex<HashMap<[Id; 2], Vec<Value>>>,
}

impl FunctionRootCache {
    pub(crate) fn clear(&mut self) {
        self.graphs.get_mut().expect("function root cache").clear();
        self.implicit_tangents
            .get_mut()
            .expect("implicit tangent cache")
            .clear();
        self.implicit_pairs
            .get_mut()
            .expect("implicit intersection cache")
            .clear();
        self.cartesian_pairs
            .get_mut()
            .expect("curve root cache")
            .clear();
        self.inequalities
            .get_mut()
            .expect("inequality vertex cache")
            .clear();
    }

    pub(crate) fn inequality(&self, key: Id, solve: impl FnOnce() -> Vertices) -> Vertices {
        if let Some(points) = self
            .inequalities
            .lock()
            .expect("inequality vertex cache")
            .get(&key)
        {
            return points.clone();
        }
        let points = solve();
        self.inequalities
            .lock()
            .expect("inequality vertex cache")
            .insert(key, points.clone());
        points
    }

    pub(super) fn cartesian<T>(
        &self,
        key: [Id; 2],
        solve: impl FnOnce() -> super::curves::CurveRoots,
        read: impl FnOnce(&[Option<Point>]) -> T,
    ) -> T {
        if let Some(points) = self
            .cartesian_pairs
            .lock()
            .expect("curve root cache")
            .get(&key)
        {
            return read(points);
        }
        let points = solve();
        let result = read(&points);
        self.cartesian_pairs
            .lock()
            .expect("curve root cache")
            .insert(key, points);
        result
    }

    pub(in crate::algo) fn implicit<T>(
        &self,
        pair: [Id; 2],
        solve: impl FnOnce() -> super::curves::CurveRoots,
        read: impl FnOnce(&[Option<Point>]) -> T,
    ) -> T {
        if let Some(points) = self
            .implicit_pairs
            .lock()
            .expect("implicit intersection cache")
            .get(&pair)
        {
            return read(points);
        }
        let points = solve();
        let result = read(&points);
        self.implicit_pairs
            .lock()
            .expect("implicit intersection cache")
            .insert(pair, points);
        result
    }

    pub(in crate::algo) fn implicit_tangents<T>(
        &self,
        inputs: [Id; 2],
        solve: impl FnOnce() -> Vec<Value>,
        read: impl FnOnce(&[Value]) -> T,
    ) -> T {
        if let Some(lines) = self
            .implicit_tangents
            .lock()
            .expect("implicit tangent cache")
            .get(&inputs)
        {
            return read(lines);
        }
        let lines = solve();
        let result = read(&lines);
        self.implicit_tangents
            .lock()
            .expect("implicit tangent cache")
            .insert(inputs, lines);
        result
    }

    fn get<R>(
        &self,
        key: FunctionIntersection,
        solve: impl FnOnce() -> Vec<Point>,
        read: impl FnOnce(&[Point]) -> R,
    ) -> R {
        if let Some(points) = self.graphs.lock().expect("function root cache").get(&key) {
            return read(points);
        }
        let points = solve();
        let result = read(&points);
        self.graphs
            .lock()
            .expect("function root cache")
            .insert(key, points);
        result
    }
}

impl FunctionIntersection {
    pub(super) fn labels_indexed(self) -> bool {
        matches!(self.method, Method::Conic(_))
    }

    fn pair(c: &Construction, first: &str, second: &str) -> Option<(Id, Id)> {
        let (a, b) = (c.id(first)?, c.id(second)?);
        let graph_partner = |kind| {
            kind == Kind::Function || CARRIERS.contains(&kind) || CONIC_OBJECTS.contains(&kind)
        };
        match (c.kind(a), c.kind(b)) {
            (Kind::Function, kind) if graph_partner(kind) => Some((a, b)),
            (kind, Kind::Function) if graph_partner(kind) => Some((b, a)),
            _ => None,
        }
    }

    /// The first slot of `Intersect(f, g, a, b)`, which owns the outputs
    /// that later edits reveal.
    pub(super) fn is_interval_anchor(self) -> bool {
        matches!(self.method, Method::Interval { slot: 0, .. })
    }

    pub(super) fn root_count(self, c: &Construction) -> usize {
        self.with_points(c, |points| points.len())
    }

    pub(super) fn interval_slot(self, slot: usize) -> Option<Self> {
        let slot = u32::try_from(slot).ok()?;
        matches!(self.method, Method::Interval { .. }).then(|| self.with_slot(slot))
    }

    fn with_slot(self, slot: u32) -> Self {
        let method = match self.method {
            Method::Conic(_) => Method::Conic(slot),
            Method::Interval { left, right, .. } => Method::Interval { left, right, slot },
            _ => Method::Polynomial(slot),
        };
        Self { method, ..self }
    }

    fn outputs(self, c: &Construction) -> Vec<Self> {
        if matches!(self.method, Method::Automatic(_)) {
            return vec![self];
        }
        let count = self.with_points(c, |points| points.len()).max(1) as u32;
        (0..count).map(|slot| self.with_slot(slot)).collect()
    }

    pub(super) fn multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<Self>> {
        let ("Intersect", [first, second]) = (name, args) else {
            return None;
        };
        let (function, other) = Self::pair(c, first, second)?;
        let work = IntersectionWork::new();
        let method = if CONIC_OBJECTS.contains(&c.kind(other)) {
            Method::Conic(0)
        } else if work.fraction(c, function).is_some()
            && (c.kind(other) != Kind::Function || work.fraction(c, other).is_some())
        {
            Method::Polynomial(0)
        } else {
            Method::Automatic(0)
        };
        Some(
            Self {
                function,
                other,
                method,
            }
            .outputs(c),
        )
    }

    /// Selector expressions become ordinary dependencies of the point.
    pub(super) fn selected(
        c: &mut Construction,
        args: &[&str],
    ) -> Option<Result<Vec<Self>, String>> {
        let [first, second, selector, tail @ ..] = args else {
            return None;
        };
        let (function, other) = Self::pair(c, first, second)?;
        let method = match tail {
            [] => {
                if let Some(value) = crate::expr::plain_number(selector) {
                    Ok(Method::ConstantIndex(index(value)))
                } else {
                    c.store_intersection_argument(selector)
                        .and_then(|(id, kind)| match kind {
                            Kind::Point if !CONIC_OBJECTS.contains(&c.kind(other)) => {
                                Ok(Method::Seed {
                                    point: id,
                                    line_first: c.id(first) == Some(other),
                                })
                            }
                            kind if kind.is_number() => Ok(Method::Indexed(id)),
                            _ => Err(format!("invalid intersection selector `{selector}`")),
                        })
                }
            }
            [right] if c.kind(other) == Kind::Function => (|| {
                let (left, left_kind) = c.store_intersection_argument(selector)?;
                let (right, right_kind) = c.store_intersection_argument(right)?;
                if !(left_kind.is_number() && right_kind.is_number()) {
                    return Err("intersection interval bounds must be numbers".into());
                }
                Ok(Method::Interval {
                    left,
                    right,
                    slot: 0,
                })
            })(),
            _ => Err("invalid function intersection arguments".into()),
        };
        Some(method.and_then(|method| {
            let def = Self {
                function,
                other,
                method,
            };
            if matches!(method, Method::ConstantIndex(_) | Method::Indexed(_))
                && !CONIC_OBJECTS.contains(&c.kind(other))
                && (!polynomial(c, function)
                    || c.kind(other) == Kind::Function && !polynomial(c, other))
            {
                return Err("an intersection index requires polynomial functions".into());
            }
            Ok(if matches!(method, Method::Interval { .. }) {
                def.outputs(c)
            } else {
                vec![def]
            })
        }))
    }

    fn arguments(self) -> impl Iterator<Item = u32> {
        match self.method {
            Method::Seed { point, .. } | Method::Indexed(point) => [Some(point), None],
            Method::Interval { left, right, .. } => [Some(left), Some(right)],
            _ => [None, None],
        }
        .into_iter()
        .flatten()
    }

    pub(super) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        self.arguments()
            .filter_map(|arg| c.intersection_argument_parents(arg))
            .flatten()
            .copied()
            .collect()
    }

    pub(super) fn with_root(self, value: Value) -> Option<Self> {
        let (Method::Automatic(previous), Value::Point { x, .. }) = (self.method, value) else {
            return None;
        };
        let bits = x.to_bits();
        (bits != previous).then_some(Self {
            method: Method::Automatic(bits),
            ..self
        })
    }

    fn with_points<R>(self, c: &Construction, read: impl FnOnce(&[(f64, f64)]) -> R) -> R {
        let key = match self.method {
            Method::Seed { .. } | Method::Automatic(_) => self,
            _ if CONIC_OBJECTS.contains(&c.kind(self.other)) => Self {
                method: Method::Conic(0),
                ..self
            },
            _ => self.with_slot(0),
        };
        c.function_roots.get(key, || self.points(c), read)
    }

    fn points(self, c: &Construction) -> Vec<(f64, f64)> {
        let work = IntersectionWork::new();
        let points = self.solve(c, &work);
        if work.exhausted() {
            Vec::new()
        } else {
            points
        }
    }

    fn solve(self, c: &Construction, work: &IntersectionWork) -> Vec<(f64, f64)> {
        let other = c.kind(self.other);
        if CONIC_OBJECTS.contains(&other) {
            return self.conic_points(c, work);
        }
        let f = |x| work.sample(c, self.function, x);
        if other == Kind::Function {
            let g = |x| work.sample(c, self.other, x);
            let difference = |x| f(x) - g(x);
            match self.method {
                Method::Interval { left, right, .. } => {
                    let (Some(left), Some(right)) = (number(c, left), number(c, right)) else {
                        return Vec::new();
                    };
                    interval_roots(
                        &difference,
                        left,
                        right,
                        crate::kernel::locus::HEADLESS_VIEW[4],
                        || work.exhausted(),
                    )
                }
                Method::Seed { .. } | Method::Automatic(_) => self
                    .seed(c)
                    .and_then(|seed| {
                        root_near(
                            &difference,
                            |x| {
                                work.derivative(c, self.function, x)
                                    .zip(work.derivative(c, self.other, x))
                                    .map_or_else(|| curve::slope(difference, x), |(a, b)| a - b)
                            },
                            seed,
                            || {
                                if let Some(target) = work.constant(c, self.other) {
                                    work.inverse(c, self.function, target)
                                } else {
                                    let target = work.constant(c, self.function)?;
                                    work.inverse(c, self.other, target)
                                }
                            },
                        )
                    })
                    .into_iter()
                    .collect(),
                _ => match (
                    work.fraction(c, self.function),
                    work.fraction(c, self.other),
                ) {
                    (Some((p, q)), Some((r, s)))
                        if work.charge((p.len() * s.len() + r.len() * q.len()) as u64) =>
                    {
                        graph_roots(&subtract(product(&p, &s), product(&r, &q)), &|units| {
                            work.charge(units)
                        })
                        .into_iter()
                        .filter(|&x| difference(x).abs() < MIN_PRECISION)
                        .collect()
                    }
                    _ => Vec::new(),
                },
            }
            .into_iter()
            .filter_map(|x| finite_point(x, f(x)))
            .collect()
        } else {
            self.line_points(c, &f, work)
        }
    }

    fn seed(self, c: &Construction) -> Option<f64> {
        match self.method {
            Method::Seed { point, .. } => match c.intersection_argument_value(point)? {
                Raw::Vec(x, y) if x.is_finite() && y.is_finite() => Some(x),
                _ => None,
            },
            Method::Automatic(bits) => Some(f64::from_bits(bits)),
            _ => None,
        }
    }

    fn line_points(
        self,
        c: &Construction,
        f: &impl Fn(f64) -> f64,
        work: &IntersectionWork,
    ) -> Vec<(f64, f64)> {
        let Some(line) = Carrier::of(c, self.other) else {
            return Vec::new();
        };
        let (a, b, d) = line.line;
        let vertical = b.abs() < STANDARD_PRECISION;
        let numeric = matches!(self.method, Method::Seed { .. } | Method::Automatic(_));
        let line_value = |x: f64| -(a * x + d) / b;
        let difference = |x| f(x) - line_value(x);
        let roots = if numeric {
            let Some(seed) = self.seed(c) else {
                return Vec::new();
            };
            let root = if vertical {
                check_root(-d / a, f)
            } else {
                root_near(
                    &difference,
                    |x| {
                        work.derivative(c, self.function, x)
                            .map_or_else(|| curve::slope(difference, x), |v| v + a / b)
                    },
                    seed,
                    || {
                        (a == 0.0)
                            .then(|| work.inverse(c, self.function, -d / b))
                            .flatten()
                    },
                )
                .and_then(|x| {
                    // CmdIntersect reaches function/function before the
                    // reversed line form for a line that is a function of x.
                    if matches!(
                        self.method,
                        Method::Seed {
                            line_first: true,
                            ..
                        }
                    ) {
                        Some(x)
                    } else {
                        check_root(x, &difference)
                    }
                })
            };
            root.into_iter().collect()
        } else if vertical {
            vec![-d / a]
        } else if let Some((p, q)) = work.fraction(c, self.function) {
            if !work.charge((2 * q.len()) as u64) {
                return Vec::new();
            }
            graph_roots(&subtract(p, product(&[-d / b, -a / b], &q)), &|units| {
                work.charge(units)
            })
        } else {
            return Vec::new();
        };
        roots
            .into_iter()
            .filter_map(|x| {
                let y = f(x);
                if !line.incident((x, y), MIN_PRECISION) {
                    return None;
                }
                finite_point(
                    x,
                    if numeric && !vertical {
                        line_value(x)
                    } else {
                        y
                    },
                )
            })
            .collect()
    }

    fn conic_points(self, c: &Construction, work: &IntersectionWork) -> Vec<(f64, f64)> {
        let part = c.arc(self.other);
        let Some(conic) = part.map(|p| p.conic).or_else(|| c.conic(self.other)) else {
            return Vec::new();
        };
        if !conic.defined || part.is_some_and(|p| !p.defined) {
            return Vec::new();
        }
        let [xx, yy, constant, xy, linear_x, linear_y] = conic.matrix;
        let f = |x| work.sample(c, self.function, x);
        let roots = if let Some((p, q)) = work
            .fraction(c, self.function)
            .filter(|(_, q)| q.len() == 1)
        {
            if !work.charge((p.len() * (p.len() + 4)) as u64) {
                return Vec::new();
            }
            let y: Vec<_> = p.into_iter().map(|a| a / q[0]).collect();
            let mut residual = vec![constant, 2.0 * linear_x, xx];
            add(&mut residual, product(&y, &[2.0 * linear_y, 2.0 * xy]));
            add(&mut residual, product(&y, &product(&y, &[yy])));
            let mut roots = graph_roots(&residual, &|units| work.charge(units));
            roots.dedup_by(|a, b| (*a - *b).abs() <= MIN_PRECISION);
            roots
        } else {
            let (left, right) = if matches!(conic.ty, ConicType::Circle | ConicType::Ellipse) {
                let radius = 1.05 * conic.half_axes[0].max(conic.half_axes[1]);
                (conic.midpoint[0] - radius, conic.midpoint[0] + radius)
            } else {
                // GeoFunction.getMinParameter/getMaxParameter in the headless view.
                (-4.3 * 1.5, 11.700000000000001 * 1.5)
            };
            interval_roots(
                &|x| {
                    let y = f(x);
                    xx * x * x
                        + yy * y * y
                        + constant
                        + 2.0 * (xy * x * y + linear_x * x + linear_y * y)
                },
                left,
                right,
                crate::kernel::locus::HEADLESS_VIEW[4],
                || work.exhausted(),
            )
        };
        // The solver uses the full conic even when the input is a conic part.
        let mut points = Vec::with_capacity(roots.len());
        for (x, y) in roots.into_iter().filter_map(|x| finite_point(x, f(x))) {
            if !work.charge(points.len() as u64 + 1) {
                return Vec::new();
            }
            if points
                .iter()
                .all(|&(a, b): &(f64, f64)| (x - a).powi(2) + (y - b).powi(2) >= STANDARD_PRECISION)
            {
                points.push((x, y));
            }
        }
        points
    }
}

impl Group for FunctionIntersection {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        Parents::of(&[self.function, self.other])
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.function, FUNCTION)?;
        for argument in self.arguments() {
            c.intersection_argument_parents(argument)
                .ok_or(Error::UnknownObject)?;
        }
        match c.kind(self.other) {
            Kind::Function => c.expect(self.other, FUNCTION),
            kind if CARRIERS.contains(&kind) => c.expect(self.other, CARRIERS),
            _ => c.expect(self.other, CONIC_OBJECTS),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let slot = match self.method {
            Method::Indexed(id) => index(number(c, id).unwrap_or(f64::NAN)),
            Method::ConstantIndex(slot)
            | Method::Interval { slot, .. }
            | Method::Polynomial(slot)
            | Method::Conic(slot) => slot,
            Method::Seed { .. } | Method::Automatic(_) => 0,
        };
        let (x, y) = self.with_points(c, |points| points.get(slot as usize).copied())?;
        Some(Value::Point { x, y })
    }

    fn parse(_: &Construction, _: &str, _: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}

fn number(c: &Construction, selector: u32) -> Option<f64> {
    match c.intersection_argument_value(selector)? {
        Raw::Num(n) => Some(n),
        _ => None,
    }
}

fn index(n: f64) -> u32 {
    (n as i32).wrapping_sub(1).max(0) as u32
}

fn polynomial(c: &Construction, id: Id) -> bool {
    IntersectionWork::new()
        .fraction(c, id)
        .is_some_and(|(_, q)| q.len() == 1)
}

fn finite_point(x: f64, y: f64) -> Option<(f64, f64)> {
    (x.is_finite() && y.is_finite()).then_some((x + 0.0, y + 0.0))
}

fn product(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            out[i + j] += a * b;
        }
    }
    out
}

fn add(a: &mut Vec<f64>, b: Vec<f64>) {
    a.resize(a.len().max(b.len()), 0.0);
    for (a, b) in a.iter_mut().zip(b) {
        *a += b;
    }
}

fn subtract(mut a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
    add(&mut a, b.into_iter().map(|b| -b).collect());
    a
}

#[cfg(test)]
mod tests {
    use crate::{expr::work_done, script};

    #[test]
    fn intersects_freehand_functions_with_graphs_lines_and_conics() {
        let c = script::load(concat!(
            "f = Function({-2, 4, 1, 5, 2})\ng(x) = x\n",
            "S = Intersect(f, g, 0, 3)\nl: y = 3\nT = Intersect(f, l)\n",
            "h = Function({-2, 2, -2, 2})\nO = (0, 0)\nk = Circle(O, sqrt(2))\n",
            "Q = Intersect(h, k)"
        ))
        .unwrap();
        for (label, expected) in [
            ("S", (3.0, 3.0)),
            ("T", (-0.5, 3.0)),
            ("Q_{1}", (-1.0, -1.0)),
            ("Q_{2}", (1.0, 1.0)),
        ] {
            let (x, y) = c.id(label).and_then(|id| c.point(id)).unwrap_or_else(|| {
                panic!("expected a defined point {label}");
            });
            assert!((x - expected.0).abs() < 1e-9, "{label}: {x}");
            assert!((y - expected.1).abs() < 1e-9, "{label}: {y}");
        }
    }

    #[test]
    fn interval_slots_share_charged_root_search() {
        use super::FunctionIntersection;
        use crate::algo::intersect::boundary::BoundaryDef;

        fn update_work(slots: u32) -> u64 {
            let mut c = script::load("a = 0\nf(x) = sin(x - a)\nh(x) = 0").unwrap();
            let defs = FunctionIntersection::selected(&mut c, &["f", "h", "0", "60"])
                .unwrap()
                .unwrap();
            assert!(defs.len() >= slots as usize);
            for slot in 0..slots {
                c.add(
                    &format!("R{slot}"),
                    BoundaryDef::function(defs[slot as usize]),
                )
                .unwrap();
            }
            let a = c.id("a").unwrap();
            let before = work_done();
            assert_eq!(c.set_number(a, 0.3).unwrap(), slots as usize + 1);
            work_done() - before
        }

        let single = update_work(1);
        assert!(single >= 400, "interval probes must charge work: {single}");
        assert_eq!(
            update_work(16),
            single,
            "a slot must reuse the update's roots"
        );
    }

    #[test]
    fn rebuilding_an_undefined_automatic_point_keeps_its_last_root() {
        let mut c = script::load(concat!(
            "a = 0\nb = 0\nf(x) = sin(x - a)\nh(x) = b\n",
            "R = Intersect(f, h)"
        ))
        .unwrap();
        let a = c.id("a").unwrap();
        let b = c.id("b").unwrap();
        let r = c.id("R").unwrap();
        for x in 1..=4 {
            c.set_number(a, f64::from(x)).unwrap();
        }
        c.set_number(b, 2.0).unwrap();
        assert!(c.point(r).is_none());
        let mut rebuilt = script::load("a = 4\nb = 2\nf(x) = sin(x - a)\nh(x) = b").unwrap();
        rebuilt.add("R", c.def(r)).unwrap();
        for c in [&mut c, &mut rebuilt] {
            c.set_number(b, 0.0).unwrap();
            let (x, _) = c.point(r).unwrap();
            assert!((x - 4.0).abs() < 1e-9, "{x}");
        }
    }
}
