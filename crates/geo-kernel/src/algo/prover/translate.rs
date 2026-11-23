// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/util/Prover.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/ProverBotanasMethod.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreCollinear.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreParallel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoArePerpendicular.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreConcurrent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreConcyclic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoAreEqual.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/adapters/PointOnPathAdapter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/adapters/OrthoLinePointLineAdapter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/adapters/BotanaCircleThreePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSingle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectConics.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/adapters/IntersectConicsAdapter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCirclePointRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLineBisector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/SymbolicParameters.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/polynomial/PPolynomial.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Polynomial equations of a construction and of a statement.
//!
//! A point has two coordinate variables; a line or segment is represented
//! by two points on it, a circle by its centre and a point on it. Each
//! construction step adds the equations its object satisfies. The free
//! variables are the coordinates of the free points and the first
//! coordinate of a point on a path; all others are eliminated.

use std::collections::{BTreeMap, BTreeSet};

use super::groebner;
use super::poly::{Order, Poly, MAX_VARS};
use super::{Budget, Outcome};
use crate::algo::conic::ConicDef;
use crate::algo::intersect::pair::Output;
use crate::algo::lines::LineDef;
use crate::algo::path::PathDef;
use crate::algo::predicate::PredicateDef;
use crate::algo::vector::VectorDef;
use crate::expr::{Bin, Node, Operand};
use crate::kernel::{Construction, Def, Id, Kind, Value};

/// The largest number of objects a proof translates.
const MAX_OBJECTS: usize = 64;

#[derive(Clone, Copy)]
enum IntersectionConstraint {
    None,
    DistinctOutput(Id),
    ExcludedPoint(Id),
    MissingEquation,
}

/// The intersection adapter caches one translation per output. Object IDs
/// stand for symbolic points across proofs, whose variable indices are local.
#[derive(Default)]
pub(crate) struct IntersectionTranslations {
    constraints: BTreeMap<Id, IntersectionConstraint>,
    order: Vec<Id>,
}

impl IntersectionTranslations {
    pub(crate) fn len(&self) -> usize {
        self.order.len()
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        for id in self.order.drain(len..) {
            self.constraints.remove(&id);
        }
    }

    fn insert(&mut self, id: Id, constraint: IntersectionConstraint) {
        self.order.push(id);
        self.constraints.insert(id, constraint);
    }
}

pub(crate) struct System {
    /// The hypotheses, each in canonical order, without repetitions.
    pub(super) hypotheses: Vec<Poly>,
    /// The thesis polynomial of each alternative of the statement.
    thesis: Vec<Poly>,
    pub(super) var_count: usize,
    pub(super) free_vars: Vec<bool>,
    /// The free points among the predecessors, in construction order.
    pub(super) free_points: Vec<Id>,
    pub(super) point_vars: BTreeMap<Id, [usize; 2]>,
    /// The first coordinate of each point on a path (free variables too).
    pub(super) path_points: Vec<Id>,
    line_vars: BTreeMap<Id, [usize; 4]>,
    circle_vars: BTreeMap<Id, [usize; 4]>,
    radius_vars: BTreeMap<Id, usize>,
    pending_distinct_outputs: Vec<(Id, [usize; 2])>,
    /// How many coordinates of the first two free points are fixed.
    fixed_coordinates: usize,
    budget_vars: usize,
    /// The statement has no translation in the reference either.
    unknown: bool,
    /// The statement is an arithmetic formula the reference has its CAS
    /// rewrite, which also words its conditions.
    pub(super) arithmetic: bool,
}

type Built = Result<System, Outcome>;

impl System {
    pub(super) fn build(
        c: &Construction,
        statement: Id,
        budget: &Budget,
        intersections: &mut IntersectionTranslations,
    ) -> Built {
        let mut system = Self::hypotheses(c, statement, budget, intersections)?;
        let done = match c.def(statement) {
            Def::Predicate(predicate) => system.statement(c, predicate, budget),
            _ => system.expression(c, statement, budget),
        };
        if done.is_none() {
            return Err(if system.unknown {
                Outcome::Unknown
            } else {
                Outcome::Undecided
            });
        }
        Ok(system)
    }

    /// The translation of the construction the statement depends on
    /// (`AlgebraicStatement.setHypotheses`).
    pub(super) fn hypotheses(
        c: &Construction,
        statement: Id,
        budget: &Budget,
        intersections: &mut IntersectionTranslations,
    ) -> Built {
        if c.kind(statement) != Kind::Boolean {
            return Err(Outcome::Undecided);
        }
        match c.def(statement) {
            Def::Predicate(_) => {}
            // A Boolean without a parent algorithm is trusted
            // (`Prover.decideStatement`, step 2).
            Def::Boolean(_) => return Err(constant(c.value(statement))),
            Def::Expr(_) if c.reads(statement).is_empty() => {
                return Err(constant(c.value(statement)))
            }
            Def::Expr(_) => {}
            _ => return Err(Outcome::Undecided),
        }
        let mut system = System {
            hypotheses: Vec::new(),
            thesis: Vec::new(),
            var_count: 0,
            free_vars: Vec::new(),
            free_points: Vec::new(),
            point_vars: BTreeMap::new(),
            path_points: Vec::new(),
            line_vars: BTreeMap::new(),
            circle_vars: BTreeMap::new(),
            radius_vars: BTreeMap::new(),
            pending_distinct_outputs: Vec::new(),
            fixed_coordinates: 4,
            budget_vars: MAX_VARS,
            unknown: false,
            arithmetic: false,
        };
        let predecessors = predecessors(c, statement).ok_or(Outcome::Undecided)?;
        // A line of fixed slope (an axis) makes the reference give up.
        if predecessors
            .iter()
            .any(|&id| matches!(c.def(id), Def::XAxis | Def::YAxis))
        {
            return Err(Outcome::Unknown);
        }
        for &id in &predecessors {
            system
                .object(c, id, budget, intersections)
                .ok_or(Outcome::Undecided)?;
        }
        for (other, p) in std::mem::take(&mut system.pending_distinct_outputs) {
            // Without hypotheses for the other output, its unconstrained
            // coordinates can always satisfy the distinctness equation.
            if let Some(q) = system.point(other) {
                system
                    .add_distinct(p, q, budget)
                    .ok_or(Outcome::Undecided)?;
            }
        }
        Ok(system)
    }

    fn new_var(&mut self, free: bool) -> Option<usize> {
        if self.var_count >= self.budget_vars {
            return None;
        }
        self.var_count += 1;
        self.free_vars.push(free);
        Some(self.var_count - 1)
    }

    fn add(&mut self, p: Poly) {
        let p = p.canonical();
        if !p.is_zero() && !self.hypotheses.contains(&p) {
            self.hypotheses.push(p);
        }
    }

    fn point(&self, id: Id) -> Option<[usize; 2]> {
        self.point_vars.get(&id).copied()
    }

    fn line(&self, id: Id) -> Option<[usize; 4]> {
        self.line_vars.get(&id).copied()
    }

    fn circle(&self, id: Id) -> Option<[usize; 4]> {
        self.circle_vars.get(&id).copied()
    }

    /// Adds the variables and equations of one predecessor.
    fn object(
        &mut self,
        c: &Construction,
        id: Id,
        budget: &Budget,
        intersections: &mut IntersectionTranslations,
    ) -> Option<()> {
        let def = c.def(id);
        match def {
            // Numbers without a parent algorithm or with an arithmetic
            // definition are not part of the algebraic translation.
            Def::Number(_) => return Some(()),
            // A distance of two points has no equations, but its variables
            // are the coordinates of both points, and as for a point on a
            // path the first one is declared free.
            Def::Distance(p, q) if c.kind(p) == Kind::Point && c.kind(q) == Kind::Point => {
                let [x, _] = self.point(p)?;
                self.point(q)?;
                self.free_vars[x] = true;
            }
            Def::Expr(_) if matches!(c.kind(id), Kind::Number) => return Some(()),
            Def::FreePoint { .. } => {
                let x = self.new_var(true)?;
                let y = self.new_var(true)?;
                self.point_vars.insert(id, [x, y]);
                self.free_points.push(id);
            }
            Def::Line(p, q) | Def::Segment(p, q) | Def::Vector(VectorDef::RayPoints(p, q))
                if c.kind(p) == Kind::Point && c.kind(q) == Kind::Point =>
            {
                let [a, b] = self.point(p)?;
                let [d, e] = self.point(q)?;
                self.line_vars.insert(id, [a, b, d, e]);
            }
            Def::Midpoint(p, q) => {
                let [p1, p2] = self.point(p)?;
                let [q1, q2] = self.point(q)?;
                let m1 = self.new_var(false)?;
                let m2 = self.new_var(false)?;
                self.add(linear(&[(2, m1), (-1, p1), (-1, q1)]));
                self.add(linear(&[(2, m2), (-1, p2), (-1, q2)]));
                self.point_vars.insert(id, [m1, m2]);
            }
            Def::IntersectLines(g, h) => {
                let g = self.line(g)?;
                let h = self.line(h)?;
                let x = self.new_var(false)?;
                let y = self.new_var(false)?;
                self.add(collinear([g[0], g[1]], [g[2], g[3]], [x, y], budget)?);
                self.add(collinear([h[0], h[1]], [h[2], h[3]], [x, y], budget)?);
                self.point_vars.insert(id, [x, y]);
            }
            Def::IntersectLineConic(_)
            | Def::IntersectLineCircle { .. }
            | Def::IntersectConics(_)
            | Def::IntersectCircles { .. } => {
                self.circle_intersection(c, id, budget, intersections)?;
            }
            Def::Perpendicular { through, to } => {
                // The second point of the line is the through point moved
                // by the direction of `to` turned a quarter.
                let [p1, p2] = self.point(through)?;
                let [a1, a2, b1, b2] = self.line(to)?;
                let n1 = self.new_var(false)?;
                let n2 = self.new_var(false)?;
                self.add(linear(&[(1, b1), (-1, a1), (1, p2), (-1, n2)]));
                self.add(linear(&[(1, p1), (-1, b2), (1, a2), (-1, n1)]));
                self.line_vars.insert(id, [n1, n2, p1, p2]);
            }
            Def::Parallel { through, to } => {
                let [c1, c2] = self.point(through)?;
                let [a1, a2, b1, b2] = self.line(to)?;
                let d1 = self.new_var(false)?;
                let d2 = self.new_var(false)?;
                self.add(linear(&[(1, d1), (-1, c1), (-1, b1), (1, a1)]));
                self.add(linear(&[(1, d2), (-1, c2), (-1, b2), (1, a2)]));
                self.line_vars.insert(id, [d1, d2, c1, c2]);
            }
            Def::Lines(LineDef::BisectorPoints(p, q)) => {
                let [a1, a2] = self.point(p)?;
                let [b1, b2] = self.point(q)?;
                let vars = [
                    self.new_var(false)?,
                    self.new_var(false)?,
                    self.new_var(false)?,
                    self.new_var(false)?,
                ];
                let [c1, c2, d1, d2] = vars;
                self.add(linear(&[(2, c1), (-1, a1), (-1, b1)]));
                self.add(linear(&[(2, c2), (-1, a2), (-1, b2)]));
                self.add(linear(&[(1, d2), (-1, c2), (-1, c1), (1, a1)]));
                self.add(linear(&[(1, d1), (-1, c1), (1, c2), (-1, a2)]));
                self.line_vars.insert(id, vars);
            }
            Def::CircleThrough { center, through } => {
                let [m1, m2] = self.point(center)?;
                let [p1, p2] = self.point(through)?;
                self.circle_vars.insert(id, [m1, m2, p1, p2]);
            }
            Def::CircleRadius { center, radius } if matches!(c.def(radius), Def::Number(_)) => {
                let [m1, m2] = self.point(center)?;
                let p1 = self.new_var(false)?;
                let p2 = self.new_var(false)?;
                let r = if let Some(&r) = self.radius_vars.get(&radius) {
                    r
                } else {
                    let r = self.new_var(false)?;
                    self.radius_vars.insert(radius, r);
                    r
                };
                self.add(
                    square_distance([m1, m2], [p1, p2], budget)?
                        .sub(&v(r).mul(&v(r), budget)?, budget)?,
                );
                self.circle_vars.insert(id, [m1, m2, p1, p2]);
                // A numeric radius prevents fixing the second free point.
                self.fixed_coordinates = 2;
            }
            Def::Conic(ConicDef::ThroughThree(p, q, r)) => {
                let a = self.point(p)?;
                let b = self.point(q)?;
                let d = self.point(r)?;
                let o1 = self.new_var(false)?;
                let o2 = self.new_var(false)?;
                self.add(equidistant(a, [o1, o2], b, budget)?);
                self.add(equidistant(a, [o1, o2], d, budget)?);
                self.circle_vars.insert(id, [o1, o2, a[0], a[1]]);
            }
            Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => {
                let x = self.new_var(true)?;
                let y = self.new_var(false)?;
                if let Some(l) = self.line(path) {
                    self.add(collinear([l[0], l[1]], [l[2], l[3]], [x, y], budget)?);
                    // A point on a line keeps the second free point free
                    // (`ProverBotanasMethod.setHypotheses`).
                    self.fixed_coordinates = 2;
                } else {
                    if !proper_circle(c, path) {
                        return None;
                    }
                    let k = self.circle(path)?;
                    self.add(equidistant([k[2], k[3]], [k[0], k[1]], [x, y], budget)?);
                }
                self.point_vars.insert(id, [x, y]);
                self.path_points.push(id);
            }
            _ => return None,
        }
        Some(())
    }

    fn circle_intersection(
        &mut self,
        c: &Construction,
        id: Id,
        budget: &Budget,
        intersections: &mut IntersectionTranslations,
    ) -> Option<()> {
        let ([first, second], indexed) = symbolic_intersection(c, id)?;
        let two_circles = c.kind(first) == Kind::Circle;
        let cached = intersections.constraints.get(&id).copied();
        if cached.is_none()
            && (!proper_circle(c, second) || (two_circles && !proper_circle(c, first)))
        {
            return None;
        }
        let p = [self.new_var(false)?, self.new_var(false)?];
        if two_circles {
            let [a, b, x, y] = self.circle(first)?;
            self.add(equidistant([x, y], [a, b], p, budget)?);
        } else {
            let [a, b, x, y] = self.line(first)?;
            self.add(collinear([a, b], [x, y], p, budget)?);
        }
        let [a, b, x, y] = self.circle(second)?;
        self.add(equidistant([x, y], [a, b], p, budget)?);

        let constraint = match cached {
            Some(constraint) => constraint,
            None => {
                let constraint = intersection_constraint(
                    c,
                    id,
                    [first, second],
                    indexed,
                    budget,
                    intersections,
                )?;
                intersections.insert(id, constraint);
                constraint
            }
        };
        match constraint {
            IntersectionConstraint::None => {}
            IntersectionConstraint::DistinctOutput(other) => {
                if let Some(q) = self.point(other) {
                    self.add_distinct(p, q, budget)?;
                } else {
                    self.pending_distinct_outputs.push((other, p));
                }
            }
            IntersectionConstraint::ExcludedPoint(other) => {
                self.add_distinct(p, self.point(other)?, budget)?;
            }
            IntersectionConstraint::MissingEquation => return None,
        }
        self.point_vars.insert(id, p);
        Some(())
    }

    fn add_distinct(&mut self, p: [usize; 2], q: [usize; 2], budget: &Budget) -> Option<()> {
        let t = self.new_var(false)?;
        self.add(distinct(p, q, t, budget)?);
        Some(())
    }

    fn statement(&mut self, c: &Construction, p: PredicateDef, budget: &Budget) -> Option<()> {
        let alternatives: Vec<Vec<Poly>> = match p {
            PredicateDef::Collinear(a, b, d) => {
                let (a, b, d) = (self.point(a)?, self.point(b)?, self.point(d)?);
                vec![vec![collinear(a, b, d, budget)?]]
            }
            PredicateDef::Parallel(g, h) => {
                let (g, h) = (self.line(g)?, self.line(h)?);
                vec![vec![parallel(g, h, budget)?]]
            }
            PredicateDef::Perpendicular(g, h) => {
                let (g, h) = (self.line(g)?, self.line(h)?);
                vec![vec![perpendicular(g, h, budget)?]]
            }
            PredicateDef::Concurrent(f, g, h) => {
                let lines = [self.line(f)?, self.line(g)?, self.line(h)?];
                let x = self.new_var(false)?;
                let y = self.new_var(false)?;
                let polys = lines
                    .iter()
                    .map(|l| collinear([l[0], l[1]], [l[2], l[3]], [x, y], budget))
                    .collect::<Option<Vec<_>>>()?;
                vec![polys]
            }
            PredicateDef::Concyclic(a, b, d, e) => {
                let points = [
                    self.point(a)?,
                    self.point(b)?,
                    self.point(d)?,
                    self.point(e)?,
                ];
                vec![vec![concyclic(points, budget)?]]
            }
            PredicateDef::Equal(a, b) => self.equal(c, a, b, budget)?,
            _ => return None,
        };
        for alternative in alternatives {
            let (thesis, rest) = alternative.split_last()?;
            for p in rest {
                self.add(p.clone());
            }
            self.thesis.push(thesis.clone());
        }
        Some(())
    }

    /// A statement written as a Boolean expression
    /// (`DependentBooleanAdapter.getBotanaPolynomials`). Two objects
    /// compared with `==` are congruent (`AlgoAreCongruent`); a formula over
    /// segments is rewritten by eliminating the segment variables
    /// ([`Self::formula`]). The other operations have no translation.
    fn expression(&mut self, c: &Construction, statement: Id, budget: &Budget) -> Option<()> {
        let (op, [left, right]) = crate::expr::comparison(c, statement)?;
        if op != Bin::Equal {
            self.unknown = true;
            return None;
        }
        let expr = crate::expr::boolean_entry(c, statement)?;
        let objects = match expr.nodes[expr.root as usize] {
            Node::Bin(_, l, r) => match (expr.nodes[l as usize], expr.nodes[r as usize]) {
                (Node::Obj(a), Node::Obj(b)) => Some((a, b)),
                _ => None,
            },
            _ => None,
        };
        let alternatives = match (left, right) {
            // Two objects that are not numbers (`AlgoAreCongruent`).
            // Two segments compare by length below, from their endpoints: a
            // replaced operand need not be among the predecessors.
            (Operand::Segment(_), Operand::Segment(_)) => {
                let (k, l) = (self.segment_vars(c, left)?, self.segment_vars(c, right)?);
                let length = |v: [usize; 4]| square_distance([v[0], v[1]], [v[2], v[3]], budget);
                vec![vec![length(k)?.sub(&length(l)?, budget)?]]
            }
            _ if objects
                .is_some_and(|(a, b)| c.kind(a) != Kind::Number && c.kind(b) != Kind::Number) =>
            {
                let (a, b) = objects?;
                self.congruent(c, a, b, budget)?
            }
            (
                Operand::Segment(_) | Operand::Object(_),
                Operand::Segment(_) | Operand::Object(_),
            ) if self.segment_vars(c, left).is_some() && self.segment_vars(c, right).is_some() => {
                let (k, l) = (self.segment_vars(c, left)?, self.segment_vars(c, right)?);
                let length = |v: [usize; 4]| square_distance([v[0], v[1]], [v[2], v[3]], budget);
                vec![vec![length(k)?.sub(&length(l)?, budget)?]]
            }
            _ => vec![self.formula(c, statement, budget)?],
        };
        self.push_alternatives(alternatives);
        Some(())
    }

    /// The endpoint variables of a segment operand.
    fn segment_vars(&self, c: &Construction, operand: Operand) -> Option<[usize; 4]> {
        match operand {
            Operand::Segment([a, b]) => {
                let ([a1, a2], [b1, b2]) = (self.point(a)?, self.point(b)?);
                Some([a1, a2, b1, b2])
            }
            Operand::Object(id) if c.kind(id) == Kind::Segment => self.line(id),
            _ => None,
        }
    }

    /// `AlgoAreCongruent` of two objects that are not numbers.
    fn congruent(
        &mut self,
        c: &Construction,
        a: Id,
        b: Id,
        budget: &Budget,
    ) -> Option<Vec<Vec<Poly>>> {
        let line = |k: Kind| matches!(k, Kind::Line | Kind::Segment | Kind::Ray);
        match (c.kind(a), c.kind(b)) {
            (Kind::Point, Kind::Point) => self.equal(c, a, b, budget),
            (Kind::Segment, Kind::Segment) => {
                let (k, l) = (self.line(a)?, self.line(b)?);
                let length = |v: [usize; 4]| square_distance([v[0], v[1]], [v[2], v[3]], budget);
                Some(vec![vec![length(k)?.sub(&length(l)?, budget)?]])
            }
            (k, l) if line(k) && line(l) => {
                let (g, h) = (self.line(a)?, self.line(b)?);
                Some(vec![
                    vec![collinear([g[0], g[1]], [g[2], g[3]], [h[0], h[1]], budget)?],
                    vec![collinear([g[0], g[1]], [g[2], g[3]], [h[2], h[3]], budget)?],
                ])
            }
            (Kind::Circle, Kind::Circle) => {
                if !proper_circle(c, a) || !proper_circle(c, b) {
                    return None;
                }
                let (k, l) = (self.circle(a)?, self.circle(b)?);
                let radius = |v: [usize; 4]| square_distance([v[0], v[1]], [v[2], v[3]], budget);
                Some(vec![vec![radius(k)?.sub(&radius(l)?, budget)?]])
            }
            _ => None,
        }
    }

    /// A formula over segments and integers (`getStrForGiac`): with a
    /// variable `s` for each segment and `v` for its length, the CAS
    /// eliminates the `s` from the formula and `s^2 = v^2`; the result in
    /// the `v` is the thesis, and `v^2 = |PQ|^2` for each segment `PQ` are
    /// hypotheses. Only the first point is fixed. The kernel computes the
    /// elimination with its own basis; a result with more than one element
    /// (the reference takes the first of the CAS's list) is undecided.
    fn formula(&mut self, c: &Construction, statement: Id, budget: &Budget) -> Option<Vec<Poly>> {
        let expr = crate::expr::boolean_entry(c, statement)?;
        let Node::Bin(Bin::Equal, lhs, rhs) = expr.nodes[expr.root as usize] else {
            return None;
        };
        // A distance operand of the root becomes a new segment, which the
        // formula translation labels and then cannot read back: the
        // reference answers unknown (`prove-expression-017`), whether or not
        // an earlier proof already replaced it.
        let replaced_distance = |side: u32| {
            let segment_object = matches!(expr.nodes[side as usize],
                Node::Obj(id) if matches!(c.def(id), Def::Segment(..)));
            !segment_object && matches!(crate::expr::operand(c, expr, side), Operand::Segment(_))
        };
        if replaced_distance(lhs) || replaced_distance(rhs) {
            self.unknown = true;
            return None;
        }
        let mut segments = Vec::new();
        let left = formula_poly(c, expr, lhs, &mut segments, 0, budget)?;
        let right = formula_poly(c, expr, rhs, &mut segments, 0, budget)?;
        let difference = left.sub(&right, budget)?;
        let n = segments.len();
        if n == 0 || 2 * n > MAX_VARS {
            return None;
        }
        let mut ideal = vec![difference];
        for i in 0..n {
            let s = Poly::var(i).mul(&Poly::var(i), budget)?;
            let v = Poly::var(n + i).mul(&Poly::var(n + i), budget)?;
            ideal.push(s.sub(&v, budget)?);
        }
        let order = Order {
            split: n,
            vars: 2 * n,
        };
        let eliminated: Vec<Poly> = groebner::basis(&ideal, &order, budget)?
            .into_iter()
            .filter(|p| !p.terms[0].0.uses_any(0..n))
            .collect();
        let [generator] = eliminated.as_slice() else {
            return None;
        };
        let mut map = vec![0; MAX_VARS];
        let mut extra = Vec::new();
        for (i, &FormulaSegment { ends: [a, b], .. }) in segments.iter().enumerate() {
            let v = self.new_var(false)?;
            map[n + i] = v;
            let length = square_distance(self.point(a)?, self.point(b)?, budget)?;
            extra.push(
                Poly::var(v)
                    .mul(&Poly::var(v), budget)?
                    .sub(&length, budget)?,
            );
        }
        self.fixed_coordinates = self.fixed_coordinates.min(2);
        self.arithmetic = true;
        extra.push(generator.rename(&map));
        Some(extra)
    }

    fn push_alternatives(&mut self, alternatives: Vec<Vec<Poly>>) {
        for alternative in alternatives {
            if let Some((thesis, rest)) = alternative.split_last() {
                for p in rest {
                    self.add(p.clone());
                }
                self.thesis.push(thesis.clone());
            }
        }
    }

    fn equal(&mut self, c: &Construction, a: Id, b: Id, budget: &Budget) -> Option<Vec<Vec<Poly>>> {
        match (c.kind(a), c.kind(b)) {
            (Kind::Point, Kind::Point) => {
                let (p, q) = (self.point(a)?, self.point(b)?);
                Some(vec![
                    vec![linear(&[(1, p[0]), (-1, q[0])])],
                    vec![linear(&[(1, p[1]), (-1, q[1])])],
                ])
            }
            (Kind::Line, Kind::Line) => {
                let (g, h) = (self.line(a)?, self.line(b)?);
                Some(vec![
                    vec![collinear([g[0], g[1]], [g[2], g[3]], [h[0], h[1]], budget)?],
                    vec![collinear([g[0], g[1]], [g[2], g[3]], [h[2], h[3]], budget)?],
                ])
            }
            (Kind::Circle, Kind::Circle) => {
                // A circle of radius zero is a single-point conic for the
                // reference; its equality test then has no translation.
                if !proper_circle(c, a) || !proper_circle(c, b) {
                    self.unknown = true;
                    return None;
                }
                let (k, l) = (self.circle(a)?, self.circle(b)?);
                let radius = |v: [usize; 4]| square_distance([v[0], v[1]], [v[2], v[3]], budget);
                Some(vec![
                    vec![square_distance([k[0], k[1]], [l[0], l[1]], budget)?],
                    vec![radius(k)?.sub(&radius(l)?, budget)?],
                ])
            }
            _ => None,
        }
    }

    /// The fixed coordinates: the first free point at (0, 0), the second at
    /// (0, 1), as many coordinates as allowed (`fixValues`).
    pub(super) fn substitutions(&self) -> Vec<(usize, i64)> {
        let values = [0, 0, 0, 1];
        self.free_points
            .iter()
            .take(2)
            .flat_map(|id| self.point_vars[id])
            .zip(values)
            .take(self.fixed_coordinates)
            .collect()
    }

    /// The product of `t*z - 1` over the thesis polynomials `t`, with a new
    /// variable `z`: it vanishes where some thesis polynomial does not.
    pub(super) fn negated_thesis(&self, budget: &Budget) -> Option<Poly> {
        if self.var_count >= self.budget_vars {
            return None;
        }
        let z = Poly::var(self.var_count);
        let mut product = Poly::constant(1);
        for t in &self.thesis {
            let factor = t.mul(&z, budget)?.sub(&Poly::constant(1), budget)?;
            product = product.mul(&factor, budget)?;
        }
        Some(product)
    }

    pub(super) fn thesis_factors(&self) -> Vec<Poly> {
        self.thesis.clone()
    }

    pub(super) fn is_free(&self, var: usize) -> bool {
        self.free_vars.get(var).copied().unwrap_or(false)
    }

    /// The variable count including the variable of the negated thesis.
    pub(super) fn total_vars(&self) -> usize {
        self.var_count + 1
    }
}

fn constant(value: Value) -> Outcome {
    match value {
        Value::Bool(true) => Outcome::True(Vec::new()),
        Value::Bool(false) => Outcome::False,
        _ => Outcome::Unknown,
    }
}

/// The objects `statement` depends on, in construction order.
fn predecessors(c: &Construction, statement: Id) -> Option<Vec<Id>> {
    let mut seen = BTreeSet::new();
    let mut stack = c.reads(statement);
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            if seen.len() > MAX_OBJECTS {
                return None;
            }
            stack.extend(c.reads(id));
        }
    }
    Some(seen.into_iter().collect())
}

/// `sum c_i * v_i`.
pub(super) fn linear(terms: &[(i64, usize)]) -> Poly {
    let budget = Budget::new(u64::MAX);
    terms.iter().fold(Poly::zero(), |sum, &(k, v)| {
        sum.add(
            &Poly::var(v)
                .mul(&Poly::constant(k), &budget)
                .expect("unlimited budget"),
            &budget,
        )
        .expect("unlimited budget")
    })
}

fn v(index: usize) -> Poly {
    Poly::var(index)
}

/// `a1*b2 - a2*b1 + c1*(a2 - b2) - c2*(a1 - b1)`: zero when the three points
/// are collinear (`PPolynomial.collinear`).
pub(super) fn collinear(
    a: [usize; 2],
    b: [usize; 2],
    d: [usize; 2],
    budget: &Budget,
) -> Option<Poly> {
    let (a1, a2, b1, b2, c1, c2) = (v(a[0]), v(a[1]), v(b[0]), v(b[1]), v(d[0]), v(d[1]));
    a1.mul(&b2, budget)?
        .sub(&a2.mul(&b1, budget)?, budget)?
        .add(&c1.mul(&a2.sub(&b2, budget)?, budget)?, budget)?
        .sub(&c2.mul(&a1.sub(&b1, budget)?, budget)?, budget)
}

/// `(a1 - b1)*(c1 - d1) + (a2 - b2)*(c2 - d2)` for the lines AB and CD.
pub(super) fn perpendicular(g: [usize; 4], h: [usize; 4], budget: &Budget) -> Option<Poly> {
    let dx = v(g[0]).sub(&v(g[2]), budget)?;
    let dy = v(g[1]).sub(&v(g[3]), budget)?;
    let ex = v(h[0]).sub(&v(h[2]), budget)?;
    let ey = v(h[1]).sub(&v(h[3]), budget)?;
    dx.mul(&ex, budget)?.add(&dy.mul(&ey, budget)?, budget)
}

/// `(a1 - b1)*(c2 - d2) - (a2 - b2)*(c1 - d1)` for the lines AB and CD.
pub(super) fn parallel(g: [usize; 4], h: [usize; 4], budget: &Budget) -> Option<Poly> {
    let dx = v(g[0]).sub(&v(g[2]), budget)?;
    let dy = v(g[1]).sub(&v(g[3]), budget)?;
    let ex = v(h[0]).sub(&v(h[2]), budget)?;
    let ey = v(h[1]).sub(&v(h[3]), budget)?;
    dx.mul(&ey, budget)?.sub(&dy.mul(&ex, budget)?, budget)
}

pub(super) fn square_distance(a: [usize; 2], b: [usize; 2], budget: &Budget) -> Option<Poly> {
    let dx = v(a[0]).sub(&v(b[0]), budget)?;
    let dy = v(a[1]).sub(&v(b[1]), budget)?;
    dx.mul(&dx, budget)?.add(&dy.mul(&dy, budget)?, budget)
}

/// Whether the circle is a circle of positive radius now, which the
/// reference's `isCircle` tests.
fn proper_circle(c: &Construction, id: Id) -> bool {
    matches!(c.value(id), Value::Circle { r, .. } if r > 0.0 && r.is_finite())
}

/// `|PQ|^2 * t - 1`: P and Q differ.
fn distinct(p: [usize; 2], q: [usize; 2], t: usize, budget: &Budget) -> Option<Poly> {
    square_distance(p, q, budget)?
        .mul(&v(t), budget)?
        .sub(&Poly::constant(1), budget)
}

/// The points an object was drawn through, which lie on it by construction.
fn drawn_through(c: &Construction, object: Id) -> Vec<Id> {
    match c.def(object) {
        Def::Line(p, q) | Def::Segment(p, q) | Def::Vector(VectorDef::RayPoints(p, q)) => {
            vec![p, q]
        }
        Def::Perpendicular { through, .. } | Def::Parallel { through, .. } => vec![through],
        Def::CircleThrough { through, .. } => vec![through],
        Def::Conic(ConicDef::ThroughThree(p, q, r)) => vec![p, q, r],
        _ => Vec::new(),
    }
}

/// Whether `point` may lie on `object` by construction: a point it was
/// drawn through, a point on it as a path, or an intersection with it.
fn on_by_construction(c: &Construction, point: Id, object: Id) -> bool {
    if drawn_through(c, object).contains(&point) {
        return true;
    }
    match c.def(point) {
        Def::Path(PathDef::On { path } | PathDef::OnParam { path, .. }) => path == object,
        Def::IntersectLines(..)
        | Def::IntersectLineCircle { .. }
        | Def::IntersectCircles { .. }
        | Def::IntersectConics(_)
        | Def::IntersectLineConic(_)
        | Def::IntersectBoundary(_)
        | Def::ConicPart(_) => c.reads(point).contains(&object),
        _ => false,
    }
}

fn symbolic_intersection(c: &Construction, id: Id) -> Option<([Id; 2], bool)> {
    let (pair, output) = match c.def(id) {
        Def::IntersectLineCircle { line, circle, .. } => return Some(([line, circle], true)),
        Def::IntersectCircles { c1, c2, .. } => return Some(([c1, c2], true)),
        Def::IntersectLineConic(d)
            if matches!(c.kind(d.line), Kind::Line | Kind::Segment | Kind::Ray)
                && c.kind(d.conic) == Kind::Circle =>
        {
            ([d.line, d.conic], d.output)
        }
        Def::IntersectConics(d)
            if c.kind(d.first) == Kind::Circle && c.kind(d.second) == Kind::Circle =>
        {
            ([d.first, d.second], d.output)
        }
        _ => return None,
    };
    match output {
        Output::Slot(0 | 1) => Some((pair, false)),
        Output::Index(0 | 1) => Some((pair, true)),
        _ => None,
    }
}

/// The shared adapter keeps one distinctness equation if any output was
/// translated before. A hidden helper's common-point exclusion overwrites
/// that equation and leaves a null entry in its place.
fn intersection_constraint(
    c: &Construction,
    id: Id,
    [first, second]: [Id; 2],
    indexed: bool,
    budget: &Budget,
    intersections: &IntersectionTranslations,
) -> Option<IntersectionConstraint> {
    budget.charge(c.child_count(first) + 1)?;
    let mut labelled = false;
    let mut earlier = None;
    for other in c.children(first).filter(|&other| other != id) {
        if let Some((pair, other_indexed)) = symbolic_intersection(c, other) {
            if pair == [first, second] || pair == [second, first] {
                labelled |= !other_indexed;
                if intersections.constraints.contains_key(&other) {
                    earlier = Some(other);
                }
            }
        }
    }
    let shared = if indexed && !labelled {
        shared_point(c, id, first, second, c.kind(first) == Kind::Circle, budget)?
    } else {
        None
    };
    Some(match (earlier, shared) {
        (Some(_), Some(_)) => IntersectionConstraint::MissingEquation,
        (Some(other), None) => IntersectionConstraint::DistinctOutput(other),
        (None, Some(shared)) => IntersectionConstraint::ExcludedPoint(shared),
        (None, None) => IntersectionConstraint::None,
    })
}

/// The common point excluded by a hidden intersection algorithm. The
/// line/circle adapter takes the first incident point. The circle pair
/// adapter applies the exclusion only when exactly one point is shared.
fn shared_point(
    c: &Construction,
    id: Id,
    a: Id,
    b: Id,
    two_circles: bool,
    budget: &Budget,
) -> Option<Option<Id>> {
    let on_a = drawn_through(c, a);
    let on_b = drawn_through(c, b);
    budget.charge((on_a.len() + on_b.len() + c.child_count(a) + c.child_count(b)) * 4 + 1)?;
    let candidates = on_b
        .into_iter()
        .chain(c.children(b))
        .chain(on_a)
        .chain(c.children(a));
    let before = c.evaluation_rank(id);
    let mut shared = candidates.filter(|&p| {
        c.evaluation_rank(p) < before
            && c.kind(p) == Kind::Point
            && !symbolic_intersection(c, p)
                .is_some_and(|(pair, _)| pair == [a, b] || pair == [b, a])
            && on_by_construction(c, p, a)
            && on_by_construction(c, p, b)
    });
    let first = shared.next();
    if two_circles && shared.any(|p| Some(p) != first) {
        return Some(None);
    }
    Some(first)
}

/// `|AO|^2 - |OB|^2`.
fn equidistant(a: [usize; 2], o: [usize; 2], b: [usize; 2], budget: &Budget) -> Option<Poly> {
    square_distance(a, o, budget)?.sub(&square_distance(o, b, budget)?, budget)
}

/// The determinant with rows `(x, y, x^2 + y^2, 1)` of the four points.
fn concyclic(points: [[usize; 2]; 4], budget: &Budget) -> Option<Poly> {
    let rows = points
        .iter()
        .map(|p| {
            let (x, y) = (v(p[0]), v(p[1]));
            let square = x.mul(&x, budget)?.add(&y.mul(&y, budget)?, budget)?;
            Some([x, y, square, Poly::constant(1)])
        })
        .collect::<Option<Vec<_>>>()?;
    determinant(&rows, budget)
}

/// Laplace expansion along the first row.
fn determinant(rows: &[[Poly; 4]], budget: &Budget) -> Option<Poly> {
    let minor = |skip: usize, rows: &[[Poly; 4]]| -> Vec<Vec<Poly>> {
        rows[1..]
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|&(k, _)| k != skip)
                    .map(|(_, p)| p.clone())
                    .collect()
            })
            .collect()
    };
    let mut total = Poly::zero();
    for k in 0..4 {
        let m = det_n(&minor(k, rows), budget)?;
        let term = rows[0][k].mul(&m, budget)?;
        total = if k % 2 == 0 {
            total.add(&term, budget)?
        } else {
            total.sub(&term, budget)?
        };
    }
    Some(total)
}

fn det_n(m: &[Vec<Poly>], budget: &Budget) -> Option<Poly> {
    if m.len() == 1 {
        return Some(m[0][0].clone());
    }
    let mut total = Poly::zero();
    for k in 0..m.len() {
        let minor: Vec<Vec<Poly>> = m[1..]
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|&(j, _)| j != k)
                    .map(|(_, p)| p.clone())
                    .collect()
            })
            .collect();
        let term = m[0][k].mul(&det_n(&minor, budget)?, budget)?;
        total = if k % 2 == 0 {
            total.add(&term, budget)?
        } else {
            total.sub(&term, budget)?
        };
    }
    Some(total)
}

/// A segment of a formula: the object, if it is one, and its endpoints.
struct FormulaSegment {
    object: Option<Id>,
    ends: [Id; 2],
}

/// The polynomial of a formula over segments, integers, `+`, `-`, `*` and
/// integer powers, with segment `i` as variable `i`. Two operands are the
/// same variable only if they are the same segment object, as the reference
/// collects segment objects; a replaced distance is a new segment each time.
fn formula_poly(
    c: &Construction,
    expr: &crate::expr::Entry,
    index: u32,
    segments: &mut Vec<FormulaSegment>,
    depth: usize,
    budget: &Budget,
) -> Option<Poly> {
    const MAX_DEPTH: usize = 32;
    const MAX_POWER: f64 = 8.0;
    if depth > MAX_DEPTH {
        return None;
    }
    budget.charge(1)?;
    match expr.nodes[index as usize] {
        Node::Num(v) if v.fract() == 0.0 && v.abs() < 1e15 => Some(Poly::constant(v as i64)),
        Node::MinusOne => Some(Poly::constant(-1)),
        Node::Bin(op @ (Bin::Add | Bin::Sub | Bin::Mul), a, b) => {
            let a = formula_poly(c, expr, a, segments, depth + 1, budget)?;
            let b = formula_poly(c, expr, b, segments, depth + 1, budget)?;
            match op {
                Bin::Add => a.add(&b, budget),
                Bin::Sub => a.sub(&b, budget),
                _ => a.mul(&b, budget),
            }
        }
        Node::Bin(Bin::Pow, a, b) => {
            let Node::Num(e) = expr.nodes[b as usize] else {
                return None;
            };
            if e.fract() != 0.0 || !(1.0..=MAX_POWER).contains(&e) {
                return None;
            }
            let base = formula_poly(c, expr, a, segments, depth + 1, budget)?;
            let mut power = base.clone();
            for _ in 1..e as u32 {
                power = power.mul(&base, budget)?;
            }
            Some(power)
        }
        _ => {
            let Operand::Segment(ends) = crate::expr::operand(c, expr, index) else {
                return None;
            };
            let object = match expr.nodes[index as usize] {
                Node::Obj(id) if matches!(c.def(id), Def::Segment(..)) => Some(id),
                // A distance object inside a formula makes the reference
                // fail: its formula substitution expects an arithmetic
                // number. A root operand is handled by `formula`.
                _ => return None,
            };
            match object.and_then(|id| segments.iter().position(|s| s.object == Some(id))) {
                Some(i) => Some(Poly::var(i)),
                None => new_segment(segments, object, ends),
            }
        }
    }
}

/// The variable of a new segment of a formula; none past the variable
/// limit (each segment needs a second variable for its length).
fn new_segment(
    segments: &mut Vec<FormulaSegment>,
    object: Option<Id>,
    ends: [Id; 2],
) -> Option<Poly> {
    if 2 * (segments.len() + 1) > MAX_VARS {
        return None;
    }
    segments.push(FormulaSegment { object, ends });
    Some(Poly::var(segments.len() - 1))
}
