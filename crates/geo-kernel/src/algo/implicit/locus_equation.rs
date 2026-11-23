// The construction rules follow reference commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdLocusEquation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCirclePointRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoLocusEquation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoEnvelope.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/ProverBotanasMethod.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/adapters/PointOnPathAdapter.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use std::collections::{BTreeMap, BTreeSet};

use crate::algo::{conic::ConicDef, lines::LineDef, path::PathDef, point::PointDef};
use crate::cas::{
    project_plane, rounded_numerator, PlanePolynomial as Polynomial, Rat, Work, PLANE_X, PLANE_Y,
};
use crate::expr::equation::implicit::Coefficients;
use crate::{Construction, Def, Id, Kind, LocusDef, Value};

type Point = [Polynomial; 2];
type Line = [Polynomial; 3];
const MAX_DEPTH: usize = 64;
const NEG_ONE: Rat = Rat { num: -1, den: 1 };
const SATURATION: usize = 0;
const MOVER_X: usize = 1;
const MOVER_Y: usize = 2;
const MIN_PRECISION: i64 = 10_000;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Definition {
    Traced { point: Id, mover: Id },
    Condition { boolean: Id, mover: Id },
    Envelope { path: Id, mover: Id },
}

impl Definition {
    pub(crate) fn parse(c: &Construction, args: &[&str]) -> Result<Self, String> {
        let named = |arg| {
            c.id(arg)
                .ok_or_else(|| format!("expected a named object, got `{arg}`"))
        };
        match *args {
            [locus] => {
                let locus = named(locus)?;
                match c.def(locus) {
                    Def::Locus(LocusDef::Path { point, mover }) => Self::traced(c, point, mover),
                    _ => Err("LocusEquation needs a point-path locus".into()),
                }
            }
            [source, mover] => {
                let source = named(source)?;
                let mover = named(mover)?;
                if c.kind(mover) != Kind::Point {
                    return Err("LocusEquation needs a moving point".into());
                }
                match c.kind(source) {
                    Kind::Point => Self::traced(c, source, mover),
                    Kind::Boolean => {
                        if mover_path(c, mover).is_some()
                            || matches!(c.def(mover), Def::FreePoint { .. })
                        {
                            Ok(Self::Condition {
                                boolean: source,
                                mover,
                            })
                        } else {
                            Err("LocusEquation needs a free point or a point on a path".into())
                        }
                    }
                    _ => Err("LocusEquation needs a traced point or a Boolean condition".into()),
                }
            }
            _ => Err("LocusEquation needs a locus or two arguments".into()),
        }
    }

    fn traced(c: &Construction, point: Id, mover: Id) -> Result<Self, String> {
        if mover_path(c, mover).is_none() || mover_path(c, point).is_some() {
            return Err("LocusEquation needs a tracer without a path and a mover on a path".into());
        }
        let mut geometry = Geometry::new(c, mover, coordinates(MOVER_X, MOVER_Y));
        match geometry.depends(point, 0) {
            Some(false) => Err("the traced point must depend on the mover".into()),
            _ => Ok(Self::Traced { point, mover }),
        }
    }

    pub(crate) fn parse_envelope(c: &Construction, args: &[&str]) -> Result<Self, String> {
        let path_kind = |id| {
            matches!(
                c.kind(id),
                Kind::Line
                    | Kind::Ray
                    | Kind::Segment
                    | Kind::Circle
                    | Kind::Conic
                    | Kind::Arc
                    | Kind::Sector
                    | Kind::PolyLine
                    | Kind::Polygon
                    | Kind::Function
                    | Kind::Curve
                    | Kind::ImplicitCurve
                    | Kind::Locus
                    | Kind::List
            )
        };
        if let [path, mover] = *args {
            if let (Some(path), Some(mover)) = (c.id(path).filter(|&id| path_kind(id)), c.id(mover))
            {
                if c.kind(mover) == Kind::Point {
                    return Ok(Self::Envelope { path, mover });
                }
            }
        }
        Err("Envelope needs a named path and a named moving point".into())
    }

    pub(crate) fn parents(self, c: &Construction) -> Option<Box<[Id]>> {
        let roots = match self {
            Self::Traced { point, mover } => [point, mover],
            Self::Condition { boolean, mover } => [boolean, mover],
            Self::Envelope { path, mover } => [path, mover],
        };
        construction_parents(c, roots)
    }

    pub(crate) fn input_key(self, c: &Construction, parents: &[Id]) -> Option<Vec<[u64; 3]>> {
        let mover = match self {
            Self::Traced { mover, .. }
            | Self::Condition { mover, .. }
            | Self::Envelope { mover, .. } => mover,
        };
        let carrier = if matches!(self, Self::Traced { .. } | Self::Envelope { .. }) {
            mover_path(c, mover)
        } else {
            None
        };
        parents
            .iter()
            .copied()
            .filter(|&id| {
                id != mover
                    && (c.def(id).is_free() || mover_path(c, id).is_some() || carrier == Some(id))
            })
            .map(|id| {
                let value = c.value(id);
                let values = match value {
                    Value::Undefined => [f64::NAN; 3],
                    Value::Bool(value) => [f64::from(value), 0.0, 0.0],
                    Value::Point { x, y } | Value::Vector { x, y } => [x, y, 0.0],
                    Value::Number(value) => [value, 0.0, 0.0],
                    Value::Circle { cx, cy, r } => [cx, cy, r],
                    Value::Segment { x1, y1, x2, y2 } => [y1 - y2, x2 - x1, x1 * y2 - y1 * x2],
                    Value::Line { .. } => {
                        let (a, b, d) = value.unit_line()?;
                        [a, b, d]
                    }
                    _ => return None,
                };
                // The default algebra descriptions display two decimal places.
                Some(values.map(|v| ((v * 100.0).round_ties_even() + 0.0).to_bits()))
            })
            .collect()
    }

    pub(crate) fn coefficients(self, c: &Construction, parents: &[Id]) -> Option<Coefficients> {
        let output = coordinates(PLANE_X, PLANE_Y);
        let mut system;
        match self {
            Self::Traced { point, mover } => {
                let path = mover_path(c, mover)?;
                system = Geometry::new(c, mover, coordinates(MOVER_X, MOVER_Y));
                let driver = system.path(path, &system.position.clone(), 0)?;
                let has_driver = !driver.is_zero();
                system.equations.push(driver);
                system.trace(point, &output, 0)?;
                if has_driver {
                    system.exclude_coincidences(path, parents)?;
                }
            }
            Self::Condition { boolean, mover } => {
                system = Geometry::new(c, mover, output);
                system.implicit = true;
                let condition = system.condition(boolean)?;
                system.equations.push(condition);
                if let Some(path) = mover_path(c, mover) {
                    let driver = system.path(path, &system.position.clone(), 0)?;
                    system.equations.push(driver);
                }
            }
            Self::Envelope { path, mover } => {
                let driver_path = mover_path(c, mover)?;
                system = Geometry::new(c, mover, coordinates(MOVER_X, MOVER_Y));
                if !system.depends(path, 0)? {
                    return None;
                }
                system.envelope(path, driver_path, &output)?;
            }
        }
        coefficients(system.equations, system.precision, &mut system.work)
    }
}

/// A symbolic equation can change while every displayed output of an
/// intermediate construction step stays the same.
fn construction_parents(c: &Construction, roots: [Id; 2]) -> Option<Box<[Id]>> {
    const MAX_EDGES: usize = 4096;
    let mut pending = roots.to_vec();
    let mut seen = BTreeSet::new();
    let mut remaining = MAX_EDGES;
    while let Some(id) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        if seen.insert(id) {
            let def = c.def(id);
            let parents = if matches!(def, Def::Locus(_)) {
                def.parents().as_slice().to_vec()
            } else {
                c.read_ids(&def)
            };
            if parents.len() > remaining.saturating_sub(pending.len()) {
                return None;
            }
            pending.extend(parents);
        }
    }
    Some(seen.into_iter().collect())
}

fn mover_path(c: &Construction, mover: Id) -> Option<Id> {
    match c.def(mover) {
        Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => Some(path),
        _ => None,
    }
}

fn coordinates(x: usize, y: usize) -> Point {
    [Polynomial::variable(x), Polynomial::variable(y)]
}

fn coefficients(
    equations: Vec<Polynomial>,
    precision: i64,
    work: &mut Work<'_>,
) -> Option<Coefficients> {
    let polynomial = project_plane(equations, precision, work)?;
    let (nx, ny) = polynomial
        .0
        .keys()
        .fold((0, 0), |(nx, ny), &[x, y, _]| (nx.max(x), ny.max(y)));
    let mut matrix = vec![vec![0.0; usize::from(ny) + 1]; usize::from(nx) + 1];
    for ([x, y, _], coefficient) in polynomial.0 {
        matrix[usize::from(x)][usize::from(y)] = coefficient.to_f64();
    }
    Some(matrix)
}

struct Geometry<'a> {
    c: &'a Construction,
    mover: Id,
    position: Point,
    precision: i64,
    dependencies: BTreeMap<Id, bool>,
    points: BTreeMap<Id, Point>,
    radii: BTreeMap<Id, Polynomial>,
    equations: Vec<Polynomial>,
    next_variable: usize,
    implicit: bool,
    work: Work<'a>,
}

impl<'a> Geometry<'a> {
    fn new(c: &'a Construction, mover: Id, position: Point) -> Self {
        let [.., xscale, yscale] = c.view();
        Self {
            c,
            mover,
            position,
            precision: (xscale.min(yscale) as i64).max(MIN_PRECISION),
            dependencies: BTreeMap::new(),
            points: BTreeMap::new(),
            radii: BTreeMap::new(),
            equations: Vec::new(),
            next_variable: MOVER_Y + 1,
            implicit: false,
            work: Work::new(&|_| true),
        }
    }

    fn depends(&mut self, id: Id, depth: usize) -> Option<bool> {
        self.visit(depth)?;
        if id == self.mover {
            return Some(true);
        }
        if let Some(&depends) = self.dependencies.get(&id) {
            return Some(depends);
        }
        if id.0 < self.mover.0 {
            return Some(false);
        }
        let parents = self.c.read_ids(&self.c.def(id));
        self.work.spend(parents.len())?;
        let mut depends = false;
        for parent in parents {
            if self.depends(parent, depth + 1)? {
                depends = true;
                break;
            }
        }
        self.dependencies.insert(id, depends);
        Some(depends)
    }

    fn visit(&mut self, depth: usize) -> Option<()> {
        if depth >= MAX_DEPTH {
            return None;
        }
        self.work.spend(1)
    }

    fn constant(&self, value: f64) -> Option<Polynomial> {
        let (numerator, denominator) = if value.fract() == 0.0 {
            (rounded_numerator(value, 1)?, 1)
        } else {
            (rounded_numerator(value, self.precision)?, self.precision)
        };
        Some(Polynomial::constant(Rat::new(
            numerator,
            i128::from(denominator),
        )?))
    }

    fn auxiliary(&mut self) -> Option<Polynomial> {
        if self.next_variable >= PLANE_X {
            return None;
        }
        let index = self.next_variable;
        self.next_variable += 1;
        Some(Polynomial::variable(index))
    }

    fn point(&mut self, id: Id, depth: usize) -> Option<Point> {
        self.visit(depth)?;
        if id == self.mover {
            return Some(self.position.clone());
        }
        if matches!(self.c.def(id), Def::FreePoint { .. }) {
            let (x, y) = self.c.point(id)?;
            return Some([self.constant(x)?, self.constant(y)?]);
        }
        if let Some(point) = self.points.get(&id) {
            return Some(point.clone());
        }
        match self.c.def(id) {
            Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => {
                self.fixed_path_point(id, path, depth + 1)
            }
            Def::Midpoint(a, b) => self.midpoint(a, b, depth + 1),
            Def::Conic(ConicDef::MidpointSegment(segment)) => {
                let Def::Segment(a, b) = self.c.def(segment) else {
                    return None;
                };
                self.midpoint(a, b, depth + 1)
            }
            _ if self.intersection_paths(id).is_some() => {
                let point = [self.auxiliary()?, self.auxiliary()?];
                self.trace(id, &point, depth + 1)?;
                self.points.insert(id, point.clone());
                Some(point)
            }
            _ => None,
        }
    }

    fn fixed_path_point(&mut self, id: Id, path: Id, depth: usize) -> Option<Point> {
        let (x, y) = self.c.point(id)?;
        let fixed_axis = if let Some((_, b, _)) = self.c.raw_carrier(path) {
            Some(usize::from(b == 0.0))
        } else if self.implicit {
            self.c
                .circle(path)
                .map(|(cx, _, _)| usize::from((x - cx).abs() < crate::tol::STANDARD_PRECISION))
        } else {
            None
        };
        let Some(axis) = fixed_axis else {
            return Some([self.constant(x)?, self.constant(y)?]);
        };
        // Round the coordinate along the path. The carrier equation determines
        // the other coordinate and keeps the point incident after rounding.
        let fixed = self.constant([x, y][axis])?;
        let free = self.auxiliary()?;
        let point = if axis == 0 {
            [fixed, free]
        } else {
            [free, fixed]
        };
        let incidence = self.path(path, &point, depth)?;
        self.equations.push(incidence);
        self.points.insert(id, point.clone());
        Some(point)
    }

    fn midpoint(&mut self, a: Id, b: Id, depth: usize) -> Option<Point> {
        let [ax, ay] = self.point(a, depth)?;
        let [bx, by] = self.point(b, depth)?;
        let half = Polynomial::constant(Rat { num: 1, den: 2 });
        Some([
            ax.add_scaled(&bx, Rat::ONE, &mut self.work)?
                .product(&half, &mut self.work)?,
            ay.add_scaled(&by, Rat::ONE, &mut self.work)?
                .product(&half, &mut self.work)?,
        ])
    }

    fn subtract(&mut self, left: Polynomial, right: &Polynomial) -> Option<Polynomial> {
        left.add_scaled(right, NEG_ONE, &mut self.work)
    }

    fn squared_distance(&mut self, [ax, ay]: Point, [bx, by]: &Point) -> Option<Polynomial> {
        let x = self.subtract(ax, bx)?;
        let y = self.subtract(ay, by)?;
        let xx = x.product(&x, &mut self.work)?;
        let yy = y.product(&y, &mut self.work)?;
        xx.add_scaled(&yy, Rat::ONE, &mut self.work)
    }

    fn line(&mut self, id: Id, depth: usize) -> Option<Line> {
        self.visit(depth)?;
        match self.c.def(id) {
            Def::Line(a, b) | Def::Segment(a, b) => {
                let [ax, ay] = self.point(a, depth + 1)?;
                let [bx, by] = self.point(b, depth + 1)?;
                let a = self.subtract(ay.clone(), &by)?;
                let b = self.subtract(bx.clone(), &ax)?;
                let c = ax.product(&by, &mut self.work)?;
                let d = ay.product(&bx, &mut self.work)?;
                Some([a, b, self.subtract(c, &d)?])
            }
            Def::Perpendicular { through, to } | Def::Parallel { through, to } => {
                let [mut a, mut b, _] = self.line(to, depth + 1)?;
                if matches!(self.c.def(id), Def::Perpendicular { .. }) {
                    (a, b) = (
                        b,
                        Polynomial::constant(NEG_ONE).product(&a, &mut self.work)?,
                    );
                }
                let [x, y] = self.point(through, depth + 1)?;
                let ax = a.product(&x, &mut self.work)?;
                let by = b.product(&y, &mut self.work)?;
                let constant = self.subtract(Polynomial::constant(Rat::ZERO), &ax)?;
                Some([a, b, self.subtract(constant, &by)?])
            }
            Def::Lines(LineDef::BisectorPoints(a, b)) => self.bisector(a, b, depth + 1),
            Def::Lines(LineDef::BisectorSegment(segment)) => {
                let Def::Segment(a, b) = self.c.def(segment) else {
                    return None;
                };
                self.bisector(a, b, depth + 1)
            }
            _ if self.c.read_ids(&self.c.def(id)).is_empty() => {
                let (a, b, c) = self.c.raw_carrier(id)?;
                Some([self.constant(a)?, self.constant(b)?, self.constant(c)?])
            }
            _ => None,
        }
    }

    fn bisector(&mut self, a: Id, b: Id, depth: usize) -> Option<Line> {
        let [ax, ay] = self.point(a, depth)?;
        let [bx, by] = self.point(b, depth)?;
        let two = Polynomial::constant(Rat { num: 2, den: 1 });
        let a = self
            .subtract(bx.clone(), &ax)?
            .product(&two, &mut self.work)?;
        let b = self
            .subtract(by.clone(), &ay)?
            .product(&two, &mut self.work)?;
        let origin = [
            Polynomial::constant(Rat::ZERO),
            Polynomial::constant(Rat::ZERO),
        ];
        let lhs = self.squared_distance([ax, ay], &origin)?;
        let rhs = self.squared_distance([bx, by], &origin)?;
        Some([a, b, self.subtract(lhs, &rhs)?])
    }

    fn path(&mut self, id: Id, at: &Point, depth: usize) -> Option<Polynomial> {
        self.visit(depth)?;
        if matches!(self.c.kind(id), Kind::Line | Kind::Ray | Kind::Segment) {
            let [a, b, c] = self.line(id, depth + 1)?;
            let ax = a.product(&at[0], &mut self.work)?;
            let by = b.product(&at[1], &mut self.work)?;
            return c.add_scaled(&ax, Rat::ONE, &mut self.work)?.add_scaled(
                &by,
                Rat::ONE,
                &mut self.work,
            );
        }
        let (center, radius_squared) = self.circle(id, depth + 1)?;
        let distance = self.squared_distance(center, at)?;
        self.subtract(distance, &radius_squared)
    }

    fn circle(&mut self, id: Id, depth: usize) -> Option<(Point, Polynomial)> {
        self.visit(depth)?;
        let result = match self.c.def(id) {
            Def::CircleThrough { center, through } => {
                let a = self.point(center, depth + 1)?;
                let b = self.point(through, depth + 1)?;
                (a.clone(), self.squared_distance(a, &b)?)
            }
            Def::Conic(ConicDef::SegmentRadius { center, segment }) => {
                let Def::Segment(a, b) = self.c.def(segment) else {
                    return None;
                };
                let a = self.point(a, depth + 1)?;
                let b = self.point(b, depth + 1)?;
                (
                    self.point(center, depth + 1)?,
                    self.squared_distance(a, &b)?,
                )
            }
            Def::CircleRadius { center, radius } => {
                let squared = match self.c.def(radius) {
                    Def::Number(_) => {
                        // Free numeric radii are eliminated as independent algebraic variables.
                        if let Some(squared) = self.radii.get(&radius) {
                            squared.clone()
                        } else {
                            let squared = self.auxiliary()?;
                            self.radii.insert(radius, squared.clone());
                            squared
                        }
                    }
                    Def::Distance(first, _) => {
                        // A point-to-point distance exposes its endpoint coordinates
                        // to the prover. The circle uses the first coordinate as radius.
                        let [x, _] = self.point(first, depth + 1)?;
                        x.product(&x, &mut self.work)?
                    }
                    _ => return None,
                };
                (self.point(center, depth + 1)?, squared)
            }
            _ => return None,
        };
        Some(result)
    }

    fn intersection_paths(&self, id: Id) -> Option<[Id; 2]> {
        match self.c.def(id) {
            Def::IntersectLines(first, second)
            | Def::Point(PointDef::LineIntersection { first, second }) => Some([first, second]),
            Def::IntersectLineCircle { line, circle, .. } => Some([line, circle]),
            Def::IntersectLineConic(d)
                if self.c.kind(d.line) == Kind::Line && self.c.kind(d.conic) == Kind::Circle =>
            {
                Some([d.line, d.conic])
            }

            Def::IntersectCircles { c1, c2, .. } => Some([c1, c2]),
            _ => None,
        }
    }

    fn trace(&mut self, id: Id, output: &Point, depth: usize) -> Option<()> {
        let constraints = if let Some([first, second]) = self.intersection_paths(id) {
            [
                self.path(first, output, depth)?,
                self.path(second, output, depth)?,
            ]
        } else {
            let [x, y] = self.point(id, depth)?;
            [self.subtract(x, &output[0])?, self.subtract(y, &output[1])?]
        };
        self.equations.extend(constraints);
        Some(())
    }

    fn exclude_coincidences(&mut self, driver: Id, parents: &[Id]) -> Option<()> {
        let (endpoints, along) = match self.c.def(driver) {
            Def::Line(a, b) | Def::Segment(a, b) => {
                let free = |id| matches!(self.c.def(id), Def::FreePoint { .. });
                let along = if free(a) && free(b) {
                    let [ax, _] = self.point(a, 0)?;
                    let [bx, _] = self.point(b, 0)?;
                    // On the carrier, equality in a varying coordinate
                    // is equivalent to coincidence with the endpoint.
                    Some(usize::from(self.subtract(ax, &bx)?.is_zero()))
                } else {
                    None
                };
                ([Some(a), Some(b)], along)
            }
            Def::CircleThrough { through, .. } => {
                let mover = self.mover;
                self.work.spend(parents.len())?;
                let incident = parents.iter().any(|&id| {
                    matches!(self.c.def(id), Def::Line(a, b)
                        if [a, b] == [through, mover] || [b, a] == [through, mover])
                });
                let distinct = self.c.point(through) != self.c.point(mover);
                ([(incident && distinct).then_some(through), None], None)
            }
            _ => return Some(()),
        };
        let mut nonzero = Polynomial::variable(SATURATION);
        let mut changed = false;
        for endpoint in endpoints.into_iter().flatten() {
            if self.numeric_point(endpoint) {
                let fixed = self.point(endpoint, 0)?;
                let distance = if let Some(axis) = along {
                    self.subtract(self.position[axis].clone(), &fixed[axis])?
                } else {
                    self.squared_distance(fixed, &self.position.clone())?
                };
                nonzero = nonzero.product(&distance, &mut self.work)?;
                changed = true;
            }
        }
        if changed {
            let condition = self.subtract(nonzero, &Polynomial::constant(Rat::ONE))?;
            self.equations.push(condition);
        }
        Some(())
    }

    fn numeric_point(&self, id: Id) -> bool {
        matches!(self.c.def(id), Def::FreePoint { .. }) || mover_path(self.c, id).is_some()
    }

    fn envelope(&mut self, path: Id, driver_path: Id, output: &Point) -> Option<()> {
        let driver = self.path(driver_path, &self.position.clone(), 0)?;
        let family = self.path(path, output, 0)?;
        // The virtual tracer only receives circle incidence constraints when
        // its current path is a circle. A single-point conic has no such rule.
        if self
            .c
            .circle(path)
            .is_some_and(|(_, _, radius)| radius == 0.0)
        {
            self.equations.push(driver);
            return Some(());
        }
        if !self.points.is_empty() {
            return None;
        }
        // With a free radius there are more variables than incidence
        // equations, so the Jacobian supplies no contact constraint.
        if !driver.is_zero() && !family.is_zero() && self.radii.is_empty() {
            // The gradients in the mover coordinates are dependent at a contact.
            let du = driver.derivative(MOVER_X, &mut self.work)?;
            let dv = driver.derivative(MOVER_Y, &mut self.work)?;
            let fu = family.derivative(MOVER_X, &mut self.work)?;
            let fv = family.derivative(MOVER_Y, &mut self.work)?;
            let lhs = du.product(&fv, &mut self.work)?;
            let rhs = dv.product(&fu, &mut self.work)?;
            let contact = self.subtract(lhs, &rhs)?;
            self.equations.push(contact);
        }
        self.equations.extend([driver, family]);
        Some(())
    }

    fn condition(&mut self, boolean: Id) -> Option<Polynomial> {
        // Two segments compared with `==` are congruent: their squared
        // lengths agree (`AlgoAreCongruent`).
        use crate::expr::{Bin, Operand};
        let (Bin::Equal, [Operand::Segment([a, b]), Operand::Segment([d, e])]) =
            crate::expr::comparison(self.c, boolean)?
        else {
            return None;
        };
        let (a, b) = (self.point(a, 0)?, self.point(b, 0)?);
        let (d, e) = (self.point(d, 0)?, self.point(e, 0)?);
        let lhs = self.squared_distance(a, &b)?;
        let rhs = self.squared_distance(d, &e)?;
        self.subtract(lhs, &rhs)
    }
}
