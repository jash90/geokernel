// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLinePolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolyLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolyLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPolynomialPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectNpFunctionPolyLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootNewton.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Intersections with the edges of polygons and polylines.

use crate::algo::conic_nd::ConicType;
use crate::algo::{
    curve, intersect, join, polygon, Group, Parents, CARRIERS, CONIC_OBJECTS, POINT,
};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};

const POLYS: &[Kind] = &[Kind::Polygon, Kind::PolyLine];
const FUNCTION: &[Kind] = &[Kind::Function];

type Point = (f64, f64);
type Line = (f64, f64, f64);

use super::functions::FunctionIntersection;

/// One output slot of an intersection with a graph or polygonal boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundaryDef(Intersection);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intersection {
    Edges(EdgeIntersection),
    Function(FunctionIntersection),
}

impl BoundaryDef {
    pub(super) fn function(def: FunctionIntersection) -> Def {
        Def::IntersectBoundary(Self(Intersection::Function(def)))
    }

    pub(crate) fn labels_indexed(self) -> bool {
        match self.0 {
            Intersection::Edges(d) => d.labels_indexed(),
            Intersection::Function(d) => d.labels_indexed(),
        }
    }

    pub(crate) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        match self.0 {
            Intersection::Edges(d) => d.hidden_parents(c),
            Intersection::Function(d) => d.hidden_parents(c),
        }
    }
}

impl Group for BoundaryDef {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        match &self.0 {
            Intersection::Edges(d) => d.parents(),
            Intersection::Function(d) => d.parents(),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match &self.0 {
            Intersection::Edges(d) => d.check(c),
            Intersection::Function(d) => d.check(c),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match &self.0 {
            Intersection::Edges(d) => d.compute(c),
            Intersection::Function(d) => d.compute(c),
        }
    }

    fn parse(c: &Construction, name: &str, args: &[&str]) -> Option<Result<Self, String>> {
        EdgeIntersection::parse(c, name, args).map(|d| d.map(|d| Self(Intersection::Edges(d))))
    }
}

/// One output slot of an intersection with a polygonal boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EdgeIntersection {
    first: Id,
    second: Id,
    which: usize,
    family: Family,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Line,
    Pair,
    Conic,
    Polynomial,
    Seeded(Id),
}

impl EdgeIntersection {
    /// The edge algorithms label all slots through an output handler. The
    /// conic dispatcher labels its outputs through `LabelManager` instead.
    pub(crate) fn labels_indexed(self) -> bool {
        matches!(
            self.family,
            Family::Line | Family::Pair | Family::Polynomial
        )
    }

    pub(crate) fn hidden_parents(&self, c: &Construction) -> Vec<Id> {
        let mut ids = polygon::inputs(c, self.second);
        if self.family == Family::Pair {
            ids.extend(polygon::inputs(c, self.first));
        }
        ids
    }

    /// Visits the defined points in the order of the upstream algorithm.
    /// The visitor can stop once it has found the requested slot.
    fn points(self, c: &Construction, mut visit: impl FnMut(Point) -> bool) {
        match self.family {
            Family::Line => {
                let Some(carrier) = Carrier::of(c, self.first) else {
                    return;
                };
                let mut previous = None;
                while let Some((t, p)) = next_on_line(c, self.second, carrier, previous) {
                    if !visit(p) {
                        return;
                    }
                    previous = Some(t);
                }
            }
            Family::Pair => {
                for a in edges(c, self.first) {
                    for b in edges(c, self.second) {
                        if let Some(p) = a.crosses(b) {
                            if !visit(p) {
                                return;
                            }
                        }
                    }
                }
            }
            Family::Conic => {
                let part = c.arc(self.first);
                let data = part.map(|a| a.conic).or_else(|| c.conic(self.first));
                let Some(data) = data.filter(|d| d.defined && part.is_none_or(|a| a.defined))
                else {
                    return;
                };
                for edge in edges(c, self.second) {
                    let (a, b, d) = edge.line;
                    let raw = intersect::line_conic_points(
                        intersect::normalized_coefficients([a, b, d]),
                        &data.matrix,
                        data.ty == ConicType::DoubleLine,
                        MIN_PRECISION,
                    );
                    // Consecutive edges may each contribute the same vertex.
                    // A single edge may also contribute both roots at tangency.
                    for p in raw.into_iter().flatten() {
                        if edge.contains(p, MIN_PRECISION)
                            && part.as_ref().map_or_else(
                                || data.on_full_conic(p, MIN_PRECISION),
                                |a| super::conics::on_part(a, p),
                            )
                            && !visit(p)
                        {
                            return;
                        }
                    }
                }
            }
            Family::Seeded(seed) => {
                if let Some(p) = seeded(c, self.first, self.second, seed) {
                    visit(p);
                }
            }
            Family::Polynomial => {
                let Some(coefficients) = c.function_polynomial(self.first) else {
                    return;
                };
                for edge in edges(c, self.second) {
                    let (roots, count) = edge.polynomial_roots(coefficients);
                    for &x in &roots[..count] {
                        let Some(y) = c.function_value(self.first, x) else {
                            continue;
                        };
                        if edge.contains((x, y), MIN_PRECISION) && !visit((x, y)) {
                            return;
                        }
                    }
                }
            }
        }
    }
}

impl Group for EdgeIntersection {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        match self.family {
            Family::Seeded(seed) => Parents::of(&[self.first, self.second, seed]),
            _ => Parents::of(&[self.first, self.second]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.second, POLYS)?;
        if let Family::Seeded(seed) = self.family {
            c.expect(seed, POINT)?;
        }
        c.expect(
            self.first,
            match self.family {
                Family::Line => CARRIERS,
                Family::Pair => POLYS,
                Family::Conic => CONIC_OBJECTS,
                Family::Polynomial | Family::Seeded(_) => FUNCTION,
            },
        )
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let mut remaining = self.which;
        let mut value = None;
        self.points(c, |(x, y)| {
            if remaining == 0 {
                // Changes in the sign of zero compare equal during a move.
                value = Some(Value::Point {
                    x: if x == 0.0 { 0.0 } else { x },
                    y: if y == 0.0 { 0.0 } else { y },
                });
                return false;
            }
            remaining -= 1;
            true
        });
        value
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let ("Intersect", [first, second, seed]) = (func, args) else {
            return None;
        };
        let (a, b, seed) = (c.id(first)?, c.id(second)?, c.id(seed)?);
        let (first, second) = match (c.kind(a), c.kind(b), c.kind(seed)) {
            (Kind::Function, Kind::Polygon | Kind::PolyLine, Kind::Point) => (a, b),
            (Kind::Polygon | Kind::PolyLine, Kind::Function, Kind::Point) => (b, a),
            _ => return None,
        };
        Some(Ok(Self {
            first,
            second,
            which: 0,
            family: Family::Seeded(seed),
        }))
    }
}

pub(crate) fn parse_multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<BoundaryDef>> {
    let ("Intersect", [first, second]) = (name, args) else {
        return None;
    };
    let (mut first, mut second) = (c.id(first)?, c.id(second)?);
    let (a, b) = (c.kind(first), c.kind(second));
    let family = if POLYS.contains(&a) && POLYS.contains(&b) {
        // CmdIntersect passes two polylines in reverse order. For a mixed
        // pair the polyline is traversed first, independent of argument order.
        if b == Kind::PolyLine {
            std::mem::swap(&mut first, &mut second);
        }
        Family::Pair
    } else {
        let carrier = if POLYS.contains(&b) {
            a
        } else if POLYS.contains(&a) {
            std::mem::swap(&mut first, &mut second);
            b
        } else {
            return None;
        };
        if CARRIERS.contains(&carrier) {
            Family::Line
        } else if CONIC_OBJECTS.contains(&carrier) {
            Family::Conic
        } else if carrier == Kind::Function && c.function_polynomial(first).is_some() {
            Family::Polynomial
        } else {
            return None;
        }
    };
    let d = EdgeIntersection {
        first,
        second,
        which: 0,
        family,
    };
    let mut count = 0;
    d.points(c, |_| {
        count += 1;
        true
    });
    Some(
        (0..count.max(1))
            .map(|which| BoundaryDef(Intersection::Edges(EdgeIntersection { which, ..d })))
            .collect(),
    )
}

#[derive(Clone, Copy)]
struct Edge {
    start: Point,
    end: Point,
    line: Line,
}

impl Edge {
    fn contains(self, p: Point, eps: f64) -> bool {
        on_line(self.line, p, eps)
            && (-eps..=1.0 + eps).contains(&parameter(self.line, self.start, p))
    }

    /// `GeoSegment.distance`: once the perpendicular projection leaves the
    /// segment, the nearest point is an endpoint.
    fn distance(self, p: Point) -> f64 {
        let t = parameter(self.line, self.start, p);
        let (a, b, d) = self.line;
        if t < 0.0 {
            (p.0 - self.start.0).hypot(p.1 - self.start.1)
        } else if t > 1.0 {
            (p.0 - self.end.0).hypot(p.1 - self.end.1)
        } else {
            ((a * p.0 + b * p.1 + d) / a.hypot(b)).abs()
        }
    }

    fn crosses(self, other: Self) -> Option<Point> {
        let p = finite(cross(self.line, other.line))?;
        (self.distance(p) < STANDARD_PRECISION && other.distance(p) < STANDARD_PRECISION)
            .then_some(p)
    }

    /// The roots of `f(x) - line(x)`, sorted and made unique per edge.
    fn polynomial_roots(self, mut coefficients: [f64; 5]) -> ([f64; 4], usize) {
        let (a, b, d) = self.line;
        if b.abs() < STANDARD_PRECISION {
            return ([-d / a, 0.0, 0.0, 0.0], 1);
        }
        coefficients[0] += d / b;
        coefficients[1] += a / b;
        let (mut roots, count) = super::conics::polynomial_roots(coefficients);
        roots[..count].sort_by(f64::total_cmp);
        let mut unique = 0;
        for index in 0..count {
            let root = roots[index];
            if root.is_finite()
                && (unique == 0 || (root - roots[unique - 1]).abs() >= STANDARD_PRECISION)
            {
                roots[unique] = root;
                unique += 1;
            }
        }
        (roots, unique)
    }
}

fn edges(c: &Construction, poly: Id) -> impl Iterator<Item = Edge> + '_ {
    let mut vertices = c.vertices(poly);
    let first = vertices.next().flatten();
    let mut previous = first;
    let closing = (c.kind(poly) == Kind::Polygon).then_some(first);
    vertices.chain(closing).filter_map(move |vertex| {
        let start = std::mem::replace(&mut previous, vertex)?;
        let end = vertex?;
        Some(Edge {
            start,
            end,
            line: join(start, end),
        })
    })
}

#[derive(Clone, Copy)]
pub(super) struct Carrier {
    pub(super) line: Line,
    origin: Point,
    min: f64,
    max: f64,
}

impl Carrier {
    pub(super) fn of(c: &Construction, id: Id) -> Option<Self> {
        let line @ (a, b, d) = c.raw_carrier(id)?;
        let (origin, min, max) = match c.value(id) {
            Value::Line { .. } => (
                match c.def(id) {
                    Def::Line(start, _) => c.point(start)?,
                    Def::Parallel { through, .. } | Def::Perpendicular { through, .. } => {
                        c.point(through)?
                    }
                    _ => {
                        let q = a * a + b * b;
                        (-d * a / q, -d * b / q)
                    }
                },
                f64::NEG_INFINITY,
                f64::INFINITY,
            ),
            Value::Ray { x, y, .. } => ((x, y), 0.0, f64::INFINITY),
            Value::Segment { x1, y1, .. } => ((x1, y1), 0.0, 1.0),
            _ => return None,
        };
        Some(Self {
            line,
            origin,
            min,
            max,
        })
    }

    pub(super) fn incident(self, p: Point, eps: f64) -> bool {
        on_line(self.line, p, eps)
            && (self.min - eps..=self.max + eps).contains(&parameter(self.line, self.origin, p))
    }

    fn hits(self, edge: Edge) -> [Option<(f64, Point)>; 2] {
        let to_hit = |p| (parameter(self.line, self.origin, p), project(self.line, p));
        let coords = cross(edge.line, self.line);
        if coords.2.abs() < STANDARD_PRECISION {
            if self.incident(edge.start, STANDARD_PRECISION)
                && self.incident(edge.end, STANDARD_PRECISION)
            {
                return [Some(to_hit(edge.start)), Some(to_hit(edge.end))];
            }
            return [None; 2];
        }
        let point = finite(coords).and_then(|p| {
            let (p0, p1, value) = if (edge.start.0 - edge.end.0).abs() <= STANDARD_PRECISION {
                (edge.start.1, edge.end.1, p.1)
            } else {
                (edge.start.0, edge.end.0, p.0)
            };
            let hit = to_hit(p);
            ((p0.min(p1) - STANDARD_PRECISION..=p0.max(p1) + STANDARD_PRECISION).contains(&value)
                && (self.min..=self.max).contains(&hit.0))
            .then_some(hit)
        });
        [point, None]
    }
}

/// Line intersections are ordered by the parameter on the line, and repeated
/// hits at a vertex share a slot. Scan for the next distinct parameter so
/// point recomputation does not need to allocate a sorted map.
fn next_on_line(
    c: &Construction,
    poly: Id,
    carrier: Carrier,
    previous: Option<f64>,
) -> Option<(f64, Point)> {
    let mut next: Option<(f64, Point)> = None;
    for (t, p) in edges(c, poly).flat_map(|edge| carrier.hits(edge)).flatten() {
        if !(t.is_finite() && p.0.is_finite() && p.1.is_finite()) {
            continue;
        }
        if previous.is_some_and(|prev| t <= prev || (t - prev).abs() < STANDARD_PRECISION) {
            continue;
        }
        match next {
            Some((there, _)) if (t - there).abs() < STANDARD_PRECISION => next = Some((there, p)),
            Some((there, _)) if t >= there => {}
            _ => next = Some((t, p)),
        }
    }
    next
}

fn cross((a, b, d): Line, (u, v, w): Line) -> Line {
    (b * w - d * v, d * u - a * w, a * v - b * u)
}

fn finite((x, y, z): Line) -> Option<Point> {
    let p = (x / z, y / z);
    (p.0.is_finite() && p.1.is_finite()).then_some(p)
}

fn on_line((a, b, d): Line, (x, y): Point, eps: f64) -> bool {
    (a * x + b * y + d).abs() < eps * (a.abs() + b.abs())
}

fn project((a, b, d): Line, (x, y): Point) -> Point {
    let t = -(d + a * x + b * y) / (a * a + b * b);
    (x + t * a, y + t * b)
}

fn parameter(line @ (a, b, _): Line, start: Point, p: Point) -> f64 {
    let (x, y) = project(line, p);
    if a.abs() <= b.abs() {
        (x - start.0) / b
    } else {
        (start.1 - y) / a
    }
}

/// Try one root on each edge from the same initial x-coordinate, then keep
/// the result closest to the initial point. Equal distances keep edge order.
fn seeded(c: &Construction, function: Id, poly: Id, start: Id) -> Option<Point> {
    let seed = c.point(start)?;
    if c.kind(poly) == Kind::Polygon && !c.value(poly).is_defined() {
        return None;
    }
    let value = |x| c.function_value(function, x).unwrap_or(f64::NAN);
    let mut best = None;
    let mut best_distance = f64::INFINITY;
    for edge in edges(c, poly) {
        let (a, b, d) = edge.line;
        let x = if b.abs() < STANDARD_PRECISION {
            -d / a
        } else {
            let slope = a / b;
            let diff = |x| value(x) + slope * x + d / b;
            let derivative = |x| {
                c.function_derivative(function, x)
                    .map_or_else(|| curve::slope(diff, x), |v| v + slope)
            };
            let Some(x) = root_near(diff, derivative, seed.0) else {
                continue;
            };
            x
        };
        let p = (x, value(x));
        let distance = (p.0 - seed.0).powi(2) + (p.1 - seed.1).powi(2);
        if distance < best_distance && edge.contains(p, MIN_PRECISION) {
            best = Some(p);
            best_distance = distance;
        }
    }
    best
}

/// `AlgoRootNewton`: try brackets on the two sides of the start, then fall
/// back to Newton iteration when no sign-changing bracket was found.
fn root_near(f: impl Fn(f64) -> f64, derivative: impl Fn(f64) -> f64, seed: f64) -> Option<f64> {
    let accepted = |x: f64| x.is_finite() && f(x).abs() < MIN_PRECISION;
    let bracketed = |lo, hi| {
        if !(lo < seed && seed < hi) {
            return None;
        }
        let fs = f(seed);
        if fs.abs() < 1e-15 {
            return Some(seed);
        }
        for (a, b) in [(lo, seed), (seed, hi)] {
            let (fa, fb) = (f(a), f(b));
            if !(fa.is_finite() && fb.is_finite()) {
                continue;
            }
            if fa.abs() < 1e-15 {
                return Some(a);
            }
            if fb.abs() < 1e-15 {
                return Some(b);
            }
            if fa.is_sign_positive() != fb.is_sign_positive() {
                let sign = if fa < 0.0 { 1.0 } else { -1.0 };
                let x = curve::bisect(a, b, |x| sign * f(x));
                if accepted(x) {
                    return Some(x);
                }
            }
        }
        None
    };
    if let Some(x) = bracketed(seed - 1.0, seed + 1.0) {
        return Some(x);
    }

    // The second bracket is restricted to the defined interval near seed.
    let mut lo = seed - 0.5;
    let mut hi = seed + 0.5;
    let (left, right) = (f(lo).is_finite(), f(hi).is_finite());
    if !left && !right && !f(seed).is_finite() {
        return None;
    }
    if !left {
        lo = definition_border(&f, lo, if right { hi } else { seed });
    }
    if !right {
        hi = definition_border(&f, hi, if left { lo } else { seed });
    }
    if let Some(x) = bracketed(lo, hi) {
        return Some(x);
    }

    let mut x = if f(seed).is_finite() {
        seed
    } else {
        (0.9 * lo + 0.1 * hi).midpoint(hi)
    };
    // Each Newton step uses two of the 100 evaluations. Verify the residual
    // within that budget, including for candidates with a small last step.
    let mut converged = false;
    for _ in 0..50 {
        let y = f(x);
        if y == 0.0 || converged {
            return (y.abs() < MIN_PRECISION).then_some(x);
        }
        let next = x - y / derivative(x);
        if !next.is_finite() {
            return None;
        }
        converged = (next - x).abs() <= 1e-6;
        x = next;
    }
    None
}

fn definition_border(f: impl Fn(f64) -> f64, mut undefined: f64, mut defined: f64) -> f64 {
    for _ in 0..50 {
        let mid = undefined.midpoint(defined);
        if mid == undefined || mid == defined {
            break;
        }
        if f(mid).is_finite() {
            defined = mid;
        } else {
            undefined = mid;
        }
    }
    defined
}
