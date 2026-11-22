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

use super::poly::{Poly, MAX_VARS};
use super::{Budget, Outcome};
use crate::algo::conic::ConicDef;
use crate::algo::lines::LineDef;
use crate::algo::path::PathDef;
use crate::algo::predicate::PredicateDef;
use crate::kernel::{Construction, Def, Id, Kind, Value};

/// The largest number of objects a proof translates.
const MAX_OBJECTS: usize = 64;

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
    /// How many coordinates of the first two free points are fixed.
    fixed_coordinates: usize,
    budget_vars: usize,
}

type Built = Result<System, Outcome>;

impl System {
    pub(super) fn build(c: &Construction, statement: Id, budget: &Budget) -> Built {
        if c.kind(statement) != Kind::Boolean {
            return Err(Outcome::Undecided);
        }
        let predicate = match c.def(statement) {
            Def::Predicate(p) => p,
            // A Boolean without a parent algorithm is trusted
            // (`Prover.decideStatement`, step 2).
            Def::Boolean(_) => return Err(constant(c.value(statement))),
            Def::Expr(_) if c.reads(statement).is_empty() => {
                return Err(constant(c.value(statement)))
            }
            _ => return Err(Outcome::Undecided),
        };
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
            fixed_coordinates: 4,
            budget_vars: MAX_VARS,
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
            system.object(c, id, budget).ok_or(Outcome::Undecided)?;
        }
        system
            .statement(c, predicate, budget)
            .ok_or(Outcome::Undecided)?;
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
    fn object(&mut self, c: &Construction, id: Id, budget: &Budget) -> Option<()> {
        let def = c.def(id);
        match def {
            // Numbers without a parent algorithm or with an arithmetic
            // definition are not part of the algebraic translation.
            Def::Number(_) => return Some(()),
            Def::Expr(_) if matches!(c.kind(id), Kind::Number) => return Some(()),
            Def::FreePoint { .. } => {
                let x = self.new_var(true)?;
                let y = self.new_var(true)?;
                self.point_vars.insert(id, [x, y]);
                self.free_points.push(id);
            }
            Def::Line(p, q) | Def::Segment(p, q) => {
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
            Def::IntersectLineCircle { line, circle, .. } => {
                // One output of `Intersect(line, circle, n)`. The unindexed
                // command makes the same definition with labels `P_{1}`,
                // `P_{2}`; its algorithm is in the construction list and does
                // not exclude an existing intersection.
                if c.label(id).contains("_{") {
                    return None;
                }
                let g = self.line(line)?;
                let k = self.circle(circle)?;
                let x = self.new_var(false)?;
                let y = self.new_var(false)?;
                self.add(collinear([g[0], g[1]], [g[2], g[3]], [x, y], budget)?);
                self.add(equidistant([k[2], k[3]], [k[0], k[1]], [x, y], budget)?);
                let existing = crate::algo::intersect::existing_line_circle_point(
                    c,
                    line,
                    c.def(line),
                    circle,
                );
                self.distinct_from_earlier(c, id, line, circle, [x, y], budget)?;
                if let Some(e) = existing {
                    if e >= id {
                        return None;
                    }
                    let e = self.point(e)?;
                    let t = self.new_var(false)?;
                    self.add(distinct([x, y], e, t, budget)?);
                }
                self.point_vars.insert(id, [x, y]);
            }
            Def::IntersectCircles { c1, c2, .. } => {
                let a = self.circle(c1)?;
                let b = self.circle(c2)?;
                // Points already on both circles: the reference takes them
                // from the points each circle was drawn through and from
                // earlier intersections; only the first source is
                // reproduced.
                if earlier_intersection_on(c, id, c1, c2) {
                    return None;
                }
                let on = |k| crate::algo::intersect::circle_points(c, k);
                let shared: Vec<Id> = on(c1)
                    .into_iter()
                    .flatten()
                    .filter(|t| on(c2).contains(&Some(*t)))
                    .collect();
                if on(c1)
                    .into_iter()
                    .chain(on(c2))
                    .flatten()
                    .any(|t| !matches!(c.def(t), Def::FreePoint { .. } | Def::Midpoint(..)))
                {
                    return None;
                }
                let x = self.new_var(false)?;
                let y = self.new_var(false)?;
                self.add(equidistant([a[2], a[3]], [a[0], a[1]], [x, y], budget)?);
                self.add(equidistant([b[2], b[3]], [b[0], b[1]], [x, y], budget)?);
                self.distinct_from_earlier(c, id, c1, c2, [x, y], budget)?;
                if let [e] = shared.as_slice() {
                    let e = self.point(*e)?;
                    let t = self.new_var(false)?;
                    self.add(distinct([x, y], e, t, budget)?);
                }
                self.point_vars.insert(id, [x, y]);
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

    /// The second output of an intersection algorithm differs from the
    /// first one whose equations were made before (`botanaVars` of the
    /// adapter): `|PQ|^2 * t = 1`.
    fn distinct_from_earlier(
        &mut self,
        c: &Construction,
        id: Id,
        first: Id,
        second: Id,
        p: [usize; 2],
        budget: &Budget,
    ) -> Option<()> {
        let same_pair = |other: Id| match c.def(other) {
            Def::IntersectLineCircle { line, circle, .. } => (line, circle) == (first, second),
            Def::IntersectCircles { c1, c2, .. } => {
                (c1, c2) == (first, second) || (c1, c2) == (second, first)
            }
            _ => false,
        };
        let others: Vec<Id> = c
            .ids()
            .filter(|&other| other != id && same_pair(other))
            .collect();
        let translated: Vec<Id> = others
            .iter()
            .copied()
            .filter(|other| self.point_vars.contains_key(other))
            .collect();
        // An output that this proof does not use may have been translated
        // by an earlier proof of the reference; that is not reproduced.
        if translated.len() != others.len() && others.iter().any(|&o| o < id) {
            return None;
        }
        match translated.as_slice() {
            [] => Some(()),
            [other] => {
                let q = self.point(*other)?;
                let t = self.new_var(false)?;
                self.add(distinct(p, q, t, budget)?);
                Some(())
            }
            _ => None,
        }
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
                let points = [self.point(a)?, self.point(b)?, self.point(d)?, self.point(e)?];
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
pub(super) fn collinear(a: [usize; 2], b: [usize; 2], d: [usize; 2], budget: &Budget) -> Option<Poly> {
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

/// `|PQ|^2 * t - 1`: P and Q differ.
fn distinct(p: [usize; 2], q: [usize; 2], t: usize, budget: &Budget) -> Option<Poly> {
    square_distance(p, q, budget)?
        .mul(&v(t), budget)?
        .sub(&Poly::constant(1), budget)
}

/// Whether an intersection with `c1` or `c2` other than theirs was made
/// before `id`.
fn earlier_intersection_on(c: &Construction, id: Id, c1: Id, c2: Id) -> bool {
    c.ids().take_while(|&other| other < id).any(|other| match c.def(other) {
        Def::IntersectLineCircle { circle, .. } => circle == c1 || circle == c2,
        Def::IntersectCircles { c1: a, c2: b, .. } => {
            (a == c1 || a == c2 || b == c1 || b == c2) && !((a, b) == (c1, c2) || (a, b) == (c2, c1))
        }
        Def::IntersectConics(_) => true,
        _ => false,
    })
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
                    .filter_map(|(k, p)| (k != skip).then(|| p.clone()))
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
                    .filter_map(|(j, p)| (j != k).then(|| p.clone()))
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
