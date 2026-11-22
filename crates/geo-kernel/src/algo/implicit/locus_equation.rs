// Licensed under the EUPL 1.2.

use std::collections::{BTreeMap, BTreeSet};

use crate::algo::{conic::ConicDef, lines::LineDef, path::PathDef, point::PointDef};
use crate::cas::{project_plane, PlanePolynomial as Polynomial, Rat, Work};
use crate::expr::equation::implicit::Coefficients;
use crate::{Construction, Def, Id, Kind, LocusDef};

type Point = [Polynomial; 2];
type Line = [Polynomial; 3];
const MAX_DEPTH: usize = 64;
const NEG_ONE: Rat = Rat { num: -1, den: 1 };

#[derive(Clone, Copy, Debug)]
pub(crate) enum Definition {
    Traced { point: Id, mover: Id },
    Condition { boolean: Id, mover: Id },
    UnsupportedLocus(Id),
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
                    Def::Locus(LocusDef::Path { point, mover }) => {
                        Ok(Self::Traced { point, mover })
                    }
                    _ if c.kind(locus) == Kind::Locus => Ok(Self::UnsupportedLocus(locus)),
                    _ => Err("LocusEquation needs a locus".into()),
                }
            }
            [source, mover] => {
                let source = named(source)?;
                let mover = named(mover)?;
                if c.kind(mover) != Kind::Point {
                    return Err("LocusEquation needs a moving point".into());
                }
                match c.kind(source) {
                    Kind::Point => Ok(Self::Traced {
                        point: source,
                        mover,
                    }),
                    Kind::Boolean => Ok(Self::Condition {
                        boolean: source,
                        mover,
                    }),
                    _ => Err("LocusEquation needs a traced point or a Boolean condition".into()),
                }
            }
            _ => Err("LocusEquation needs a locus or two arguments".into()),
        }
    }

    pub(crate) fn parents(self, c: &Construction) -> Option<Box<[Id]>> {
        let roots = match self {
            Self::Traced { point, mover } => [point, mover],
            Self::Condition { boolean, mover } => [boolean, mover],
            Self::UnsupportedLocus(locus) => return Some(Box::new([locus])),
        };
        construction_parents(c, roots)
    }

    pub(crate) fn coefficients(self, c: &Construction) -> Option<Coefficients> {
        let output = [Polynomial::variable(2), Polynomial::variable(3)];
        let mut system;
        let equations = match self {
            Self::Traced { point, mover } => {
                let path = mover_path(c, mover)?;
                if matches!(c.def(point), Def::Path(_)) {
                    return None;
                }
                system =
                    Geometry::new(c, mover, [Polynomial::variable(0), Polynomial::variable(1)]);
                if !system.depends(point, 0)? {
                    return None;
                }
                let driver = system.path(path, &system.position.clone())?;
                let mut equations = system.trace(point, &output)?;
                equations.push(driver);
                equations
            }
            Self::Condition { boolean, mover } => {
                system = Geometry::new(c, mover, output);
                if !system.depends(boolean, 0)? {
                    return None;
                }
                let condition = system.condition(boolean)?;
                if matches!(c.def(mover), Def::FreePoint { .. }) {
                    vec![condition]
                } else {
                    vec![
                        condition,
                        system.path(mover_path(c, mover)?, &system.position.clone())?,
                    ]
                }
            }
            Self::UnsupportedLocus(_) => return None,
        };
        coefficients(equations, &mut system.work)
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

fn coefficients(equations: Vec<Polynomial>, work: &mut Work<'_>) -> Option<Coefficients> {
    let polynomial = project_plane(equations, work)?;
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
    dependencies: BTreeMap<Id, bool>,
    work: Work<'a>,
}

impl<'a> Geometry<'a> {
    fn new(c: &'a Construction, mover: Id, position: Point) -> Self {
        Self {
            c,
            mover,
            position,
            dependencies: BTreeMap::new(),
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

    fn constant(value: f64) -> Option<Polynomial> {
        Some(Polynomial::constant(Rat::from_value(value)?))
    }

    fn point(&mut self, id: Id, depth: usize) -> Option<Point> {
        self.visit(depth)?;
        if id == self.mover {
            return Some(self.position.clone());
        }
        if !self.depends(id, depth)? {
            let (x, y) = self.c.point(id)?;
            return Some([Self::constant(x)?, Self::constant(y)?]);
        }
        match self.c.def(id) {
            Def::Midpoint(a, b) => self.midpoint(a, b, depth + 1),
            Def::Conic(ConicDef::MidpointSegment(segment)) => {
                let Def::Segment(a, b) = self.c.def(segment) else {
                    return None;
                };
                self.midpoint(a, b, depth + 1)
            }
            _ => None,
        }
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
            _ if !self.depends(id, depth)? => {
                let (a, b, c) = self.c.raw_carrier(id)?;
                Some([Self::constant(a)?, Self::constant(b)?, Self::constant(c)?])
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

    fn path(&mut self, id: Id, at: &Point) -> Option<Polynomial> {
        if matches!(self.c.kind(id), Kind::Line | Kind::Ray | Kind::Segment) {
            let [a, b, c] = self.line(id, 0)?;
            let ax = a.product(&at[0], &mut self.work)?;
            let by = b.product(&at[1], &mut self.work)?;
            return c.add_scaled(&ax, Rat::ONE, &mut self.work)?.add_scaled(
                &by,
                Rat::ONE,
                &mut self.work,
            );
        }
        let (center, radius_squared) = match self.c.def(id) {
            Def::CircleThrough { center, through } => {
                let a = self.point(center, 0)?;
                let b = self.point(through, 0)?;
                (a.clone(), self.squared_distance(a, &b)?)
            }
            Def::Conic(ConicDef::SegmentRadius { center, segment }) => {
                let Def::Segment(a, b) = self.c.def(segment) else {
                    return None;
                };
                let a = self.point(a, 0)?;
                let b = self.point(b, 0)?;
                (self.point(center, 0)?, self.squared_distance(a, &b)?)
            }
            _ => return None,
        };
        let distance = self.squared_distance(center, at)?;
        self.subtract(distance, &radius_squared)
    }

    fn trace(&mut self, id: Id, output: &Point) -> Option<Vec<Polynomial>> {
        let paths = match self.c.def(id) {
            Def::IntersectLines(first, second)
            | Def::Point(PointDef::LineIntersection { first, second }) => Some([first, second]),
            Def::IntersectLineCircle { line, circle, .. } => Some([line, circle]),
            Def::IntersectCircles { c1, c2, .. } => Some([c1, c2]),
            _ => None,
        };
        if let Some([first, second]) = paths {
            return Some(vec![self.path(first, output)?, self.path(second, output)?]);
        }
        let [x, y] = self.point(id, 0)?;
        Some(vec![
            self.subtract(x, &output[0])?,
            self.subtract(y, &output[1])?,
        ])
    }

    fn condition(&mut self, boolean: Id) -> Option<Polynomial> {
        let [[a, b], [d, e]] = crate::expr::distance_equality(self.c, boolean)?;
        let (a, b) = (self.point(a, 0)?, self.point(b, 0)?);
        let (d, e) = (self.point(d, 0)?, self.point(e, 0)?);
        let lhs = self.squared_distance(a, &b)?;
        let rhs = self.squared_distance(d, &e)?;
        self.subtract(lhs, &rhs)
    }
}
