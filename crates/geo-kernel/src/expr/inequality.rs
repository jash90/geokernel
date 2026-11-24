//! Boolean combinations of borders in two coordinates.

// Border classification and dispatch follow Inequality.java and AlgoVertexIneq.java
// at upstream revision 112919f2d450ec7430a5cd3b65dba226fdc43b74.
// Licensed under the EUPL 1.2.

use super::equation::Builder;
use super::{entry, Bin, Entry, IntersectionWork, Node, Ty, Un};
use crate::algo::conic_nd::ConicData;
use crate::algo::intersect::boundary::line_crossing;
use crate::algo::intersect::helper_line_conic_points;
use crate::algo::plane_polynomial::{self, PlanePolynomial};
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Def, Id};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const MAX_BORDERS: usize = 32;
pub(crate) const MAX_VERTICES: usize = 512;
const MAX_EXPANSION_WORK: u64 = 100_000;
// Bound dense coefficient storage even when expansion returns a single high power.
const MAX_BORDER_DEGREE: u32 = 300;

pub(crate) fn is_inequality(c: &Construction, id: Id) -> bool {
    let Def::Expr(def) = c.def(id) else {
        return false;
    };
    let e = entry(c, def);
    matches!(e.nodes[e.root as usize], Node::SystemFunction(2, root)
        if e.tys[root as usize] == Ty::Bool)
}

/// Helper classes belong to a construction, separate from the per-update
/// cache of output coordinates.
#[derive(Default)]
pub(crate) struct VertexHelpers {
    records: Mutex<HelperRecords>,
    outputs: Vec<Id>,
    changed: AtomicBool,
    pub(crate) pending: BTreeSet<Id>,
    pub(crate) refreshing: bool,
}

#[derive(Default)]
struct HelperRecords {
    attached: BTreeMap<Id, Helpers>,
    /// A command can preview its outputs before the first one is added.
    preview: Option<(Id, Preview)>,
}

struct Preview {
    helpers: Helpers,
    points: Arc<[(f64, f64)]>,
    fails: bool,
}

impl HelperRecords {
    fn preview(&mut self, c: &Construction, source: Id) -> &Preview {
        if self.preview.as_ref().is_none_or(|(id, _)| *id != source) {
            let mut helpers = Helpers::default();
            let outcome = compute(c, source, &mut helpers);
            let fails = outcome.as_ref().is_some_and(|outcome| outcome.fails);
            self.preview = Some((
                source,
                Preview {
                    helpers,
                    points: points(outcome),
                    fails,
                },
            ));
        }
        &self.preview.as_ref().expect("vertex preview").1
    }
}

impl VertexHelpers {
    pub(crate) fn attach(&mut self, source: Id, owner: Id, output: Id) {
        self.outputs.push(output);
        if output != owner {
            if self.pending.contains(&owner) {
                self.pending.insert(output);
            }
            return;
        }
        let records = self.records.get_mut().expect("inequality helpers");
        let helpers = match records.preview.take() {
            Some((id, preview)) if source == id => preview.helpers,
            _ => Helpers::default(),
        };
        records.attached.insert(owner, helpers);
    }

    pub(crate) fn outputs(&self) -> &[Id] {
        &self.outputs
    }

    pub(crate) fn clear_history(&mut self) {
        let records = self.records.get_mut().expect("inequality helpers");
        records.attached.clear();
        records.preview = None;
        self.pending.clear();
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.outputs
            .truncate(self.outputs.partition_point(|id| id.index() < len));
        self.pending.split_off(&Id(len));
        let records = self.records.get_mut().expect("inequality helpers");
        records.attached.split_off(&Id(len));
        records.preview = None;
    }

    pub(crate) fn history(&self, owner: Id) -> Option<Helpers> {
        self.records
            .lock()
            .expect("inequality helpers")
            .attached
            .get(&owner)
            .cloned()
    }

    pub(crate) fn restore_history(&mut self, owner: Id, history: Option<Helpers>) {
        let attached = &mut self.records.get_mut().expect("inequality helpers").attached;
        if let Some(history) = history {
            attached.insert(owner, history);
        } else {
            attached.remove(&owner);
        }
    }

    pub(crate) fn begin_update(&mut self) {
        self.records.get_mut().expect("inequality helpers").preview = None;
        *self.changed.get_mut() = false;
    }

    pub(crate) fn restore_changed(&mut self, changed: bool) {
        *self.changed.get_mut() = changed;
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Helpers {
    count: usize,
    slots: Vec<Option<Helper>>,
    changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Helper {
    Polynomial,
    Newton,
}

impl Helpers {
    fn select(
        &mut self,
        count: usize,
        i: usize,
        j: usize,
        class: Option<Helper>,
    ) -> Option<Helper> {
        if self.count != count {
            self.slots = vec![None; count * count];
            self.count = count;
            self.changed = true;
        }
        let slot = &mut self.slots[i * count + j];
        if slot.is_none() && class.is_some() {
            *slot = class;
            self.changed = true;
        }
        *slot
    }
}

fn points(outcome: Option<Outcome>) -> Arc<[(f64, f64)]> {
    outcome
        .filter(|outcome| !outcome.fails)
        .map(|outcome| outcome.vertices)
        .unwrap_or_default()
        .into()
}

pub(crate) fn vertices(c: &Construction, source: Id, owner: Id) -> Arc<[(f64, f64)]> {
    if owner.index() >= c.len() {
        return preview_vertices(c, source);
    }
    c.function_roots.inequality(owner, || {
        let mut records = c
            .inequality_helpers
            .records
            .lock()
            .expect("inequality helpers");
        let helpers = records.attached.entry(owner).or_default();
        if c.inequality_helpers.refreshing {
            *helpers = Helpers::default();
        }
        helpers.changed = false;
        let result = points(compute(c, source, helpers));
        if helpers.changed {
            c.inequality_helpers.changed.store(true, Ordering::Relaxed);
        }
        result
    })
}

pub(crate) fn preview_vertices(c: &Construction, source: Id) -> Arc<[(f64, f64)]> {
    c.inequality_helpers
        .records
        .lock()
        .expect("inequality helpers")
        .preview(c, source)
        .points
        .clone()
}

/// A border that is neither polynomial nor a function of one coordinate
/// stops building the inequality tree. Testing a candidate can then fail.
pub(crate) fn fails(c: &Construction, source: Id) -> bool {
    c.inequality_helpers
        .records
        .lock()
        .expect("inequality helpers")
        .preview(c, source)
        .fails
}

struct Outcome {
    vertices: Vec<(f64, f64)>,
    fails: bool,
}

fn compute(c: &Construction, source: Id, helpers: &mut Helpers) -> Option<Outcome> {
    let Def::Expr(def) = c.def(source) else {
        return None;
    };
    let e = entry(c, def);
    let Node::SystemFunction(2, root) = e.nodes[e.root as usize] else {
        return None;
    };
    let work = IntersectionWork::new();
    let mut builder = Builder::function(e);
    builder.limit_expansion(MAX_EXPANSION_WORK);
    for node in &mut builder.nodes {
        *node = match *node {
            Node::Local(0, _) => Node::Var,
            Node::Local(1, _) => Node::VarY,
            other => other,
        };
    }
    let mut borders = Vec::new();
    let mut stopped = false;
    let tree = Tree::build(
        c,
        &mut builder,
        root,
        false,
        &mut borders,
        &mut stopped,
        &work,
    )?;
    let at = Plane { c, e, work: &work };
    let mut vertices = Vec::new();
    for (i, a) in borders.iter().enumerate() {
        for (j, b) in borders.iter().enumerate().skip(i + 1) {
            if !work.charge(1) {
                return None;
            }
            let (first, second, first_index, second_index) = if a.helper_kind() < b.helper_kind() {
                (a, b, i, j)
            } else {
                (b, a, j, i)
            };
            let helper = first.select_helper(second, |class| {
                helpers.select(borders.len(), first_index, second_index, class)
            });
            let candidates = match helper {
                Some(Helper::Newton) => Vec::new(),
                Some(Helper::Polynomial) if first.expression.is_some() => {
                    first.live_line_intersection(second, &at)
                }
                _ => a.intersections(b, &at)?,
            };
            if stopped && Border::validates(a, b, &candidates, &at)? {
                return Some(Outcome {
                    vertices: Vec::new(),
                    fails: true,
                });
            }
            for (x, y) in candidates {
                if tree.truth(&borders, x, y, &at)? == 3 {
                    if vertices.len() >= MAX_VERTICES {
                        return None;
                    }
                    vertices.push((x, y));
                }
            }
        }
    }
    Some(Outcome {
        vertices,
        fails: false,
    })
}

/// The function and work meter for evaluating nonpolynomial borders.
struct Plane<'a> {
    c: &'a Construction,
    e: &'a Entry,
    work: &'a IntersectionWork,
}

impl Plane<'_> {
    fn charge(&self, units: u64) -> bool {
        self.work.charge(units)
    }

    /// `Inequality.normal` (`lhs - rhs`, oriented so that the region is
    /// positive) at `(x, y)`.
    fn normal(&self, expression: &Expression, x: f64, y: f64) -> f64 {
        let at = |i: u32| {
            if i as usize >= self.e.nodes.len() {
                return f64::NAN;
            }
            self.work.sample_plane(self.c, self.e, i, x, y)
        };
        at(expression.positive) - at(expression.negative)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BorderType {
    ParametricX,
    ParametricY,
    Line,
    Conic,
    OneVariableX,
    OneVariableY,
    Implicit,
    /// `INEQUALITY_INVALID`: a border without intersections that only
    /// takes part in the Boolean test.
    Invalid,
}

/// A border that does not expand to a polynomial, kept as the two sides of
/// its comparison in the input tree.
#[derive(Clone, Copy)]
struct Expression {
    positive: u32,
    negative: u32,
    /// For a parametric border, `-coefficient` of the linear coordinate
    /// (`Inequality.coef`), which divides the border function.
    divisor: f64,
}

struct Border {
    kind: BorderType,
    /// A structurally parametric border can simplify to a line at a
    /// particular value (for example y > x^n at n = 0).
    parametric_kind: Option<BorderType>,
    polynomial: PlanePolynomial,
    evaluation_cost: u64,
    expression: Option<Expression>,
    /// The two sides of a parametric border, for its border function.
    sides: Option<Expression>,
    /// For a one-variable border that is not linear, the coefficients of
    /// the numerator of its normal form (`AlgoRootsPolynomial.calcRoots`
    /// solves the numerator of `Fractions.getFraction`).
    numerator: Option<Vec<f64>>,
    strict: bool,
}

impl Border {
    /// `Inequality.update` for a border whose normal form is not a
    /// polynomial: only `y = f(x)` and `x = f(y)` borders keep a function.
    fn nonpolynomial(
        c: &Construction,
        builder: &Builder,
        positive: u32,
        negative: u32,
        strict: bool,
    ) -> Self {
        let normal = |var| {
            let a = coefficient(c, builder, positive, var)?;
            let b = coefficient(c, builder, negative, var)?;
            Some(a - b)
        };
        let usable = |c: Option<f64>| c.filter(|c| !is_zero(*c) && !c.is_nan());
        let (coef_x, coef_y) = (normal(Node::Var), normal(Node::VarY));
        let (kind, coefficient) = match (usable(coef_x), usable(coef_y)) {
            (None, Some(y)) if coef_x.is_none() => (BorderType::ParametricY, y),
            (Some(x), None) if coef_y.is_none() => (BorderType::ParametricX, x),
            _ => (BorderType::Invalid, f64::NAN),
        };
        let expression = Expression {
            positive,
            negative,
            divisor: -coefficient,
        };
        Self {
            kind,
            parametric_kind: None,
            polynomial: Vec::new(),
            evaluation_cost: 1,
            expression: Some(expression),
            sides: Some(expression),
            numerator: None,
            strict,
        }
    }

    /// The border function of a nonpolynomial parametric border at `t`
    /// (`Inequality.funBorder`: the normal form with the other coordinate
    /// replaced by 0, divided by `-coef`).
    fn border_value(&self, at: &Plane<'_>, t: f64) -> f64 {
        let Some(expression) = &self.sides else {
            return f64::NAN;
        };
        let value = if self.kind == BorderType::ParametricX {
            at.normal(expression, 0.0, t)
        } else {
            at.normal(expression, t, 0.0)
        };
        value / expression.divisor
    }

    /// `Inequality.rootMultiple` of a one-variable polynomial border: its
    /// real roots, a root twice where the border touches without a sign
    /// change and the closed side is the inside.
    fn zeros(&self, work: &IntersectionWork) -> Option<Vec<f64>> {
        let coefficients: Vec<f64> = if let Some(numerator) = &self.numerator {
            numerator.clone()
        } else if self.kind == BorderType::OneVariableX {
            self.polynomial.first().cloned().unwrap_or_default()
        } else {
            self.polynomial
                .iter()
                .map(|row| row.first().copied().unwrap_or_default())
                .collect()
        };
        let mut roots = plane_polynomial::unscaled_roots(&coefficients, work);
        roots.sort_by(f64::total_cmp);
        if !work.charge((roots.len() * (coefficients.len() + 1)) as u64) {
            return None;
        }
        let value = |t: f64| coefficients.iter().rev().fold(0.0, |v, c| v * t + c);
        let mut zeros = Vec::with_capacity(roots.len());
        for (i, &root) in roots.iter().enumerate() {
            let before = if i == 0 {
                root - 1.0
            } else {
                (root + roots[i - 1]) / 2.0
            };
            let after = roots
                .get(i + 1)
                .map_or(root + 1.0, |next| (root + next) / 2.0);
            let (sign_before, sign_after) = (value(before).signum(), value(after).signum());
            if sign_before != sign_after {
                zeros.push(root);
            } else if (sign_before > 0.0) ^ !self.strict {
                zeros.extend([root, root]);
            }
        }
        Some(zeros)
    }

    fn new(terms: Vec<(usize, usize, f64)>, strict: bool) -> Self {
        use BorderType::*;
        let mut degree = 0;
        let mut degrees = [0, 0];
        let mut mixed = false;
        for &(x, y, _) in &terms {
            degree = degree.max(x + y);
            degrees[0] = degrees[0].max(x);
            degrees[1] = degrees[1].max(y);
            mixed |= x > 0 && y > 0;
        }
        let kind = match degrees {
            [_, 0] => OneVariableX,
            [0, _] => OneVariableY,
            _ if degree == 1 => Line,
            [1, _] if !mixed => ParametricX,
            [_, 1] if !mixed => ParametricY,
            _ if degree == 2 => Conic,
            _ => Implicit,
        };
        let polynomial = plane_polynomial::from_terms(terms);
        let evaluation_cost = plane_polynomial::evaluation_cost(&polynomial);
        Self {
            kind,
            parametric_kind: None,
            polynomial,
            evaluation_cost,
            expression: None,
            sides: None,
            numerator: None,
            strict,
        }
    }

    fn coefficient(&self, x: usize, y: usize) -> f64 {
        self.polynomial
            .get(y)
            .and_then(|row| row.get(x))
            .copied()
            .unwrap_or_default()
    }

    fn helper_kind(&self) -> BorderType {
        self.parametric_kind
            .unwrap_or_else(|| self.reference_kind())
    }

    fn select_helper(
        &self,
        other: &Self,
        select: impl FnOnce(Option<Helper>) -> Option<Helper>,
    ) -> Option<Helper> {
        use BorderType::*;
        let polynomial = self.expression.is_none();
        let helper = match (self.helper_kind(), other.helper_kind()) {
            (ParametricX, ParametricX | Line | OneVariableX)
            | (ParametricY, ParametricY | Line | OneVariableY) => polynomial,
            (ParametricX, ParametricY) => polynomial && other.expression.is_none(),
            (ParametricX | ParametricY, Conic) if !polynomial => return select(None),
            (ParametricX | ParametricY, Conic) => true,
            _ => return None,
        };
        select(Some(if helper {
            Helper::Polynomial
        } else {
            Helper::Newton
        }))
    }

    /// A polynomial function-line helper can still read a live function
    /// at the argument fixed by a vertical line in its coordinate system.
    fn live_line_intersection(&self, other: &Self, at: &Plane<'_>) -> Vec<(f64, f64)> {
        if other.helper_kind() != BorderType::Line {
            return Vec::new();
        }
        let transpose = self.kind == BorderType::ParametricX;
        let (argument, value) = if transpose {
            ((0, 1), (1, 0))
        } else {
            ((1, 0), (0, 1))
        };
        let slope = other.coefficient(argument.0, argument.1);
        if is_zero(slope) || !is_zero(other.coefficient(value.0, value.1)) {
            return Vec::new();
        }
        let t = -other.coefficient(0, 0) / slope;
        let f = self.border_value(at, t);
        if transpose {
            vec![(f, t)]
        } else {
            vec![(t, f)]
        }
    }

    fn intersections(&self, other: &Self, at: &Plane<'_>) -> Option<Vec<(f64, f64)>> {
        use BorderType::*;
        let work = at.work;
        let (a, b) = if self.kind <= other.kind {
            (self, other)
        } else {
            (other, self)
        };
        if let Some(points) = Self::zero_intersections(a, b, at) {
            return points;
        }
        if a.expression.is_some() || b.expression.is_some() {
            return Self::nonpolynomial_intersections(a, b, at);
        }
        match (a.kind, b.kind) {
            (_, Implicit) | (OneVariableY, OneVariableY) => Some(Vec::new()),
            (Line, Line) => {
                // The line-border helper works in homogeneous coordinates and
                // divides by any nonzero weight before testing the Boolean tree.
                let line = |p: &Self| {
                    (
                        p.coefficient(1, 0),
                        p.coefficient(0, 1),
                        p.coefficient(0, 0),
                    )
                };
                Some(line_crossing(line(a), line(b)).into_iter().collect())
            }
            (Line, Conic) => {
                let mut conic = ConicData::new();
                conic.set_matrix([
                    b.coefficient(2, 0),
                    b.coefficient(0, 2),
                    b.coefficient(0, 0),
                    b.coefficient(1, 1) * 0.5,
                    b.coefficient(1, 0) * 0.5,
                    b.coefficient(0, 1) * 0.5,
                ]);
                let line = [
                    a.coefficient(1, 0),
                    a.coefficient(0, 1),
                    a.coefficient(0, 0),
                ];
                Some(
                    helper_line_conic_points(line, &conic)
                        .into_iter()
                        .flatten()
                        .collect(),
                )
            }
            (ParametricX, ParametricX | Line | Conic | OneVariableX | OneVariableY) => {
                // Use y as the independent coordinate of x = f(y).
                if !work.charge(a.evaluation_cost + b.evaluation_cost) {
                    return None;
                }
                let a = plane_polynomial::transpose(&a.polynomial);
                let b = plane_polynomial::transpose(&b.polynomial);
                Some(
                    plane_polynomial::intersections(&a, &b, work)?
                        .into_iter()
                        .map(|(y, x)| (x, y))
                        .collect(),
                )
            }
            _ => plane_polynomial::intersections(&a.polynomial, &b.polynomial, work),
        }
    }

    /// Whether `AlgoVertexIneq.typeSwitch` tests at least one candidate of
    /// this pair (`validateLast`), including the undefined outputs of its
    /// intersection helpers. `candidates` are the pair's own intersections.
    fn validates(a: &Self, b: &Self, candidates: &[(f64, f64)], at: &Plane<'_>) -> Option<bool> {
        use BorderType::*;
        let kind = |border: &Self| match border.kind {
            OneVariableX | OneVariableY if !border.is_one_variable_nonlinear() => Line,
            kind => kind,
        };
        let (ka, kb) = if kind(a) <= kind(b) {
            ((kind(a), a), (kind(b), b))
        } else {
            ((kind(b), b), (kind(a), a))
        };
        let ((ka, a), (kb, b)) = (ka, kb);
        let zeros = |border: &Self| border.zeros(at.work).map(|zeros| !zeros.is_empty());
        Some(match (ka, kb) {
            (Implicit | Invalid, _) | (_, Implicit | Invalid) => false,
            (ParametricX | ParametricY, Conic) => a.expression.is_none(),
            (ParametricX | ParametricY, OneVariableX | OneVariableY) => zeros(b)?,
            (ParametricX | ParametricY, _) | (Line, Line | Conic) | (Conic, Conic) => true,
            (Line, OneVariableX | OneVariableY) => {
                // `intLinearX` and `intLinearY` skip a line parallel to the
                // one-variable border's zeros.
                let line = if kb == OneVariableX {
                    a.coefficient(0, 1)
                } else {
                    a.coefficient(1, 0)
                };
                !is_zero(line) && zeros(b)?
            }
            (Conic, OneVariableX | OneVariableY) => !candidates.is_empty(),
            (OneVariableX, OneVariableY) => zeros(a)? && zeros(b)?,
            _ => false,
        })
    }

    /// The kind `Inequality.update` gives: a border linear in its one
    /// coordinate is `INEQUALITY_LINEAR`.
    fn reference_kind(&self) -> BorderType {
        match self.kind {
            BorderType::OneVariableX | BorderType::OneVariableY
                if !self.is_one_variable_nonlinear() =>
            {
                BorderType::Line
            }
            kind => kind,
        }
    }

    /// The pairs of `AlgoVertexIneq.typeSwitch` that read the zeros of a
    /// one-variable border: `intParamYX`, `intParamXY`, `intLinearX`,
    /// `intLinearY` and `intXY`, in the reference's order. `None` for the
    /// other pairs.
    #[allow(clippy::option_option)]
    fn zero_intersections(a: &Self, b: &Self, at: &Plane<'_>) -> Option<Option<Vec<(f64, f64)>>> {
        use BorderType::*;
        let (a, b) = if a.reference_kind() <= b.reference_kind() {
            (a, b)
        } else {
            (b, a)
        };
        let zeros = |border: &Self| border.zeros(at.work);
        let points: Option<Vec<(f64, f64)>> = match (a.reference_kind(), b.reference_kind()) {
            (ParametricY, OneVariableX) | (ParametricX, OneVariableY) => {
                a.sides?;
                zeros(b).map(|zeros| {
                    zeros
                        .into_iter()
                        .map(|zero| {
                            let value = a.border_value(at, zero);
                            if a.kind == ParametricY {
                                (zero, value)
                            } else {
                                (value, zero)
                            }
                        })
                        .collect()
                })
            }
            (Line, kind @ (OneVariableX | OneVariableY)) => {
                let (lx, ly, lc) = (
                    a.coefficient(1, 0),
                    a.coefficient(0, 1),
                    a.coefficient(0, 0),
                );
                let along_x = kind == OneVariableX;
                if is_zero(if along_x { ly } else { lx }) {
                    Some(Vec::new())
                } else {
                    zeros(b).map(|zeros| {
                        zeros
                            .into_iter()
                            .map(|zero| {
                                if along_x {
                                    (zero, (-lx * zero - lc) / ly)
                                } else {
                                    ((-ly * zero - lc) / lx, zero)
                                }
                            })
                            .collect()
                    })
                }
            }
            (OneVariableX, OneVariableY) => zeros(a).zip(zeros(b)).and_then(|(xs, ys)| {
                at.charge((xs.len() * ys.len()) as u64).then(|| {
                    xs.iter()
                        .flat_map(|&x| ys.iter().map(move |&y| (x, y)))
                        .collect()
                })
            }),
            _ => return None,
        };
        if points.as_ref().is_some_and(|p| !at.charge(p.len() as u64)) {
            return Some(None);
        }
        Some(points)
    }

    fn is_one_variable_nonlinear(&self) -> bool {
        match self.kind {
            BorderType::OneVariableX => self.polynomial.first().is_some_and(|row| row.len() > 2),
            BorderType::OneVariableY => self.polynomial.len() > 2,
            _ => false,
        }
    }

    /// The pairs of `AlgoVertexIneq` with a nonpolynomial border, ordered
    /// so that `a.kind <= b.kind`. Only a one-variable border meets a
    /// nonpolynomial function border (`intParamYX`, `intParamXY`); the
    /// other pairs use Newton helpers that start from an undefined point
    /// and so never yield a vertex.
    fn nonpolynomial_intersections(a: &Self, b: &Self, at: &Plane<'_>) -> Option<Vec<(f64, f64)>> {
        use BorderType::*;
        match (a.kind, b.kind) {
            // A border linear in its coordinate is `INEQUALITY_LINEAR`.
            (ParametricY, OneVariableX) | (ParametricX, OneVariableY)
                if a.expression.is_some() && b.is_one_variable_nonlinear() =>
            {
                let zeros = b.zeros(at.work)?;
                if !at.charge(zeros.len() as u64) {
                    return None;
                }
                Some(
                    zeros
                        .into_iter()
                        .map(|zero| {
                            let value = a.border_value(at, zero);
                            if a.kind == ParametricY {
                                (zero, value)
                            } else {
                                (value, zero)
                            }
                        })
                        .collect(),
                )
            }
            // `intParamParam` with a polynomial border intersects the two
            // functions as polynomials; the nonpolynomial difference has
            // no polynomial factors, so it has no roots.
            _ => Some(Vec::new()),
        }
    }
}

/// Whether `a && b` is a comparison chain such as `-1 < y < 2`, which the
/// parser joins through the shared middle operand (`AND_INTERVAL`).
fn is_chain(builder: &Builder, a: u32, b: u32) -> bool {
    fn end(builder: &Builder, i: u32) -> Option<u32> {
        match builder.node(i) {
            Node::Bin(op, _, right) if is_comparison(op) => Some(right),
            Node::Bin(Bin::And, a, b) if is_chain(builder, a, b) => end(builder, b),
            _ => None,
        }
    }
    matches!(builder.node(b), Node::Bin(op, left, _)
        if is_comparison(op) && end(builder, a) == Some(left))
}

fn is_comparison(op: Bin) -> bool {
    matches!(
        op,
        Bin::Greater | Bin::GreaterEqual | Bin::Less | Bin::LessEqual | Bin::Equal | Bin::NotEqual
    )
}

/// `ExpressionNode.getCoefficient`: the coefficient of `var` in the
/// subtree `i` when the subtree is linear in it, `None` otherwise.
fn coefficient(c: &Construction, builder: &Builder, i: u32, var: Node) -> Option<f64> {
    let node = builder.node(i);
    let reads = |j: u32| {
        let mut pending = vec![j];
        let mut budget = 10_000;
        while let Some(k) = pending.pop() {
            budget -= 1;
            if budget == 0 {
                return true;
            }
            let n = builder.node(k);
            if n == var {
                return true;
            }
            pending.extend(super::children(&n));
        }
        false
    };
    match node {
        Node::Bin(op @ (Bin::Add | Bin::Sub | Bin::Mul | Bin::Div), a, b) => {
            let left = coefficient(c, builder, a, var)?;
            let right = coefficient(c, builder, b, var)?;
            let constant = |j| builder.number_value(c, j);
            match op {
                Bin::Add => Some(left + right),
                Bin::Sub => Some(left - right),
                Bin::Mul if !builder.contains_var(b) => Some(left * constant(b)?),
                Bin::Mul if !builder.contains_var(a) => Some(right * constant(a)?),
                Bin::Div if !builder.contains_var(b) => Some(left / constant(b)?),
                _ => (!reads(a) && !reads(b)).then_some(0.0),
            }
        }
        node if super::children(&node).next().is_none() => {
            Some(if node == var { 1.0 } else { 0.0 })
        }
        _ => (!reads(i)).then_some(0.0),
    }
}

/// The coefficients of the numerator of a one-variable normal form in its
/// coordinate; empty when the numerator does not expand.
fn numerator(c: &Construction, builder: &mut Builder, root: u32, kind: BorderType) -> Vec<f64> {
    let (numerator, _) = super::fraction::get_fraction(builder, root, true);
    let Ok(polynomial) = builder.expand(c, numerator.unwrap_or(root)) else {
        return Vec::new();
    };
    let terms = polynomial.terms().to_vec();
    let values = builder.number_values(c, terms.iter().map(|term| term.coeff));
    let mut coefficients = Vec::new();
    for (term, value) in terms.iter().zip(values) {
        let (power, other) = if kind == BorderType::OneVariableX {
            (term.x, term.y)
        } else {
            (term.y, term.x)
        };
        let Some(value) = value.filter(|v| v.is_finite()) else {
            return Vec::new();
        };
        if other != 0 || power as usize > MAX_BORDER_DEGREE as usize {
            return Vec::new();
        }
        if coefficients.len() <= power as usize {
            coefficients.resize(power as usize + 1, 0.0);
        }
        coefficients[power as usize] += value;
    }
    coefficients
}

/// A plane polynomial at `(x, y)` that leaves out a coordinate it does
/// not read, so that a NaN there does not reach the value through `0 * y`.
fn reading_value(polynomial: &PlanePolynomial, x: f64, y: f64) -> f64 {
    fn horner(coefficients: impl DoubleEndedIterator<Item = f64>, t: f64) -> f64 {
        let mut coefficients = coefficients.rev();
        let first = coefficients.next().unwrap_or(0.0);
        coefficients.fold(first, |sum, c| sum * t + c)
    }
    horner(
        polynomial.iter().map(|row| horner(row.iter().copied(), x)),
        y,
    )
}

fn is_zero(v: f64) -> bool {
    v.abs() < crate::tol::STANDARD_PRECISION
}

fn rewrite_abs(builder: &mut Builder, a: u32, b: u32) -> (u32, u32) {
    let square = |builder: &mut Builder, x| builder.push(Node::Bin(Bin::Mul, x, x));
    let signed_square = |builder: &mut Builder, x| {
        let abs = builder.push(Node::Un(Un::Abs, x));
        builder.push(Node::Bin(Bin::Mul, x, abs))
    };
    match (builder.node(a), builder.node(b)) {
        (Node::Un(Un::Abs, x), Node::Un(Un::Abs, y)) => (square(builder, x), square(builder, y)),
        (Node::Un(Un::Abs, x), _) if !builder.contains_var(b) => {
            (square(builder, x), signed_square(builder, b))
        }
        (_, Node::Un(Un::Abs, y)) if !builder.contains_var(a) => {
            (signed_square(builder, a), square(builder, y))
        }
        _ => (a, b),
    }
}

enum Tree {
    /// A part of the tree that `initIneqs` never reached.
    Empty,
    Border(usize),
    Constant(bool),
    /// `NO_OPERATION`, which `IneqTree.valueAround` has no case for: it
    /// reads its left side and is unknown.
    Unknown(Box<Tree>),
    Pair(Bin, Box<Tree>, Box<Tree>),
}

impl Tree {
    fn build(
        c: &Construction,
        builder: &mut Builder,
        root: u32,
        negate: bool,
        borders: &mut Vec<Border>,
        stopped: &mut bool,
        work: &IntersectionWork,
    ) -> Option<Self> {
        if *stopped {
            return Some(Self::Empty);
        }
        if !work.charge(1) {
            return None;
        }
        // `FunctionNVar.initIneqs` pushes a negation into the operations
        // (`adjustOp`), down to the comparisons, where a NaN residual still
        // reads as false.
        let mut child = |builder: &mut Builder, i, negate| {
            Self::build(c, builder, i, negate, borders, stopped, work).map(Box::new)
        };
        let tree = match builder.node(root) {
            Node::Bin(Bin::Implies, a, b) => {
                let op = if negate { Bin::And } else { Bin::Or };
                let left = child(builder, a, !negate)?;
                Self::Pair(op, left, child(builder, b, negate)?)
            }
            // `IneqTree.valueAround` has no case for `XOR`, nor for the
            // `NO_OPERATION` that negation makes of it or of a comparison
            // chain (`AND_INTERVAL`): they read as unknown.
            Node::Bin(Bin::Xor, a, b) => {
                let left = child(builder, a, negate)?;
                child(builder, b, negate)?;
                Self::Unknown(left)
            }
            Node::Bin(Bin::And, a, b) if negate && is_chain(builder, a, b) => {
                let left = child(builder, a, negate)?;
                child(builder, b, negate)?;
                Self::Unknown(left)
            }
            Node::Bin(op, a, b) if op.is_logic() => {
                let op = match (op, negate) {
                    (Bin::And, true) => Bin::Or,
                    (Bin::Or, true) => Bin::And,
                    (op, _) => op,
                };
                let left = child(builder, a, negate)?;
                Self::Pair(op, left, child(builder, b, negate)?)
            }
            Node::Un(Un::Not, a) => *child(builder, a, !negate)?,
            Node::Bin(
                op @ (Bin::Greater | Bin::GreaterEqual | Bin::Less | Bin::LessEqual),
                a,
                b,
            ) => {
                let op = match (op, negate) {
                    (Bin::Greater, true) => Bin::LessEqual,
                    (Bin::GreaterEqual, true) => Bin::Less,
                    (Bin::Less, true) => Bin::GreaterEqual,
                    (Bin::LessEqual, true) => Bin::Greater,
                    (op, _) => op,
                };
                let (a, b) = if matches!(op, Bin::Less | Bin::LessEqual) {
                    (b, a)
                } else {
                    (a, b)
                };
                let strict = matches!(op, Bin::Greater | Bin::Less);
                let (positive, negative) = (a, b);
                let (a, b) = rewrite_abs(builder, a, b);
                let root = builder.push(Node::Bin(Bin::Sub, a, b));
                let polynomial = match builder.expand(c, root) {
                    Ok(polynomial) => polynomial,
                    Err(
                        super::equation::NotPolynomial::ImplicitCurve
                        | super::equation::NotPolynomial::VariableDegree,
                    ) if !builder.expansion_spent() => {
                        let border = Border::nonpolynomial(c, builder, positive, negative, strict);
                        if border.kind == BorderType::Invalid {
                            *stopped = true;
                            return Some(Self::Empty);
                        }
                        if borders.len() == MAX_BORDERS {
                            return None;
                        }
                        borders.push(border);
                        return Some(Self::Border(borders.len() - 1));
                    }
                    Err(_) => return None,
                };
                if polynomial.degree() > MAX_BORDER_DEGREE {
                    return None;
                }
                let terms = polynomial.terms();
                let coefficients = builder.number_values(c, terms.iter().map(|term| term.coeff));
                let terms = terms
                    .iter()
                    .zip(coefficients)
                    .map(|(term, value)| {
                        Some((
                            term.x as usize,
                            term.y as usize,
                            value.filter(|c| c.is_finite())?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                if terms.iter().all(|&(x, y, v)| (x, y) == (0, 0) || v == 0.0) {
                    let constant = terms
                        .iter()
                        .find(|&&(x, y, _)| (x, y) == (0, 0))
                        .map_or(0.0, |&(_, _, v)| v);
                    Self::Constant(match op {
                        Bin::GreaterEqual | Bin::LessEqual => constant >= 0.0,
                        _ => constant > 0.0,
                    })
                } else {
                    if borders.len() == MAX_BORDERS || !work.charge(terms.len() as u64) {
                        return None;
                    }
                    let index = borders.len();
                    let mut border = Border::new(
                        terms.into_iter().filter(|&(_, _, v)| v != 0.0).collect(),
                        strict,
                    );
                    if border.reference_kind() == BorderType::Line {
                        let shape = Border::nonpolynomial(c, builder, positive, negative, strict);
                        if matches!(
                            shape.kind,
                            BorderType::ParametricX | BorderType::ParametricY
                        ) {
                            border.parametric_kind = Some(shape.kind);
                        }
                    }
                    border.sides = match border.kind {
                        BorderType::ParametricY => Some(-border.coefficient(0, 1)),
                        BorderType::ParametricX => Some(-border.coefficient(1, 0)),
                        _ => None,
                    }
                    .map(|divisor| Expression {
                        positive,
                        negative,
                        divisor,
                    });
                    if border.is_one_variable_nonlinear() {
                        border.numerator = Some(numerator(c, builder, root, border.kind));
                    }
                    borders.push(border);
                    Self::Border(index)
                }
            }
            // `initIneqs` builds nothing for a Boolean constant or object and
            // stops there.
            Node::Obj(id) if c.kind(id) == crate::Kind::Boolean => {
                *stopped = true;
                Self::Empty
            }
            Node::Bool(_) => {
                *stopped = true;
                Self::Empty
            }
            _ => return None,
        };
        Some(tree)
    }

    /// Bit 0 allows false, bit 1 allows true. At a border both are possible.
    fn truth(&self, borders: &[Border], x: f64, y: f64, at: &Plane<'_>) -> Option<u8> {
        let work = at.work;
        if !work.charge(1) {
            return None;
        }
        let mask = match self {
            Self::Empty => return None,
            Self::Border(index) => {
                let border = &borders[*index];
                if !work.charge(border.evaluation_cost) {
                    return None;
                }
                if let Some(expression) = &border.expression {
                    // `Inequality.valueAround`: NaN is neither zero nor
                    // positive, so it reads as false.
                    let v = at.normal(expression, x, y);
                    if work.exhausted() {
                        return None;
                    }
                    return Some(if is_zero(v) {
                        3
                    } else {
                        1 << u8::from(v > 0.0)
                    });
                }
                // `Inequality.valueAround`: the normal form reads only the
                // coordinates it has, and a NaN value reads as false.
                let v = reading_value(&border.polynomial, x, y);
                if v.is_nan() {
                    1
                } else if v.abs() < STANDARD_PRECISION {
                    3
                } else {
                    1 << u8::from(v > 0.0)
                }
            }
            Self::Constant(v) => 1 << u8::from(*v),
            Self::Unknown(a) => {
                a.truth(borders, x, y, at)?;
                3
            }
            Self::Pair(op, a, b) => {
                let a = a.truth(borders, x, y, at)?;
                let b = b.truth(borders, x, y, at)?;
                let mut result = 0;
                for u in [false, true] {
                    for v in [false, true] {
                        if a & (1 << u8::from(u)) == 0 || b & (1 << u8::from(v)) == 0 {
                            continue;
                        }
                        let truth = match op {
                            Bin::And => u && v,
                            Bin::Or => u || v,
                            Bin::Xor => u ^ v,
                            Bin::Implies => !u || v,
                            _ => unreachable!("logical pair"),
                        };
                        result |= 1 << u8::from(truth);
                    }
                }
                result
            }
        };
        Some(mask)
    }
}

#[cfg(test)]
mod tests {
    use crate::{script, Value};

    #[test]
    fn a_polynomial_helper_reads_the_new_exponent_on_a_vertical_border() {
        let mut c = script::load("n = 2\nq(x,y) = y > x^n && x > 4 && y > 0").unwrap();
        let source = c.id("q").unwrap();
        let points = super::preview_vertices(&c, source);
        let after_first = crate::expr::work_done();
        assert_eq!(super::preview_vertices(&c, source), points);
        assert_eq!(crate::expr::work_done(), after_first);

        let p = script::exec(&mut c, "P = Vertex(q)").unwrap().unwrap();
        let n = c.id("n").unwrap();
        assert_eq!(c.value(p), Value::Point { x: 4.0, y: 16.0 });
        c.set_number(n, 0.5).unwrap();
        assert_eq!(c.value(p), Value::Point { x: 4.0, y: 2.0 });
    }

    #[test]
    fn rolling_back_the_first_vertex_drops_its_helper_history() {
        let mut c = script::load("n = 2\nq(x,y) = y > x^n && x > 4 && y > 0").unwrap();
        let n = c.id("n").unwrap();
        let at = c.checkpoint();
        script::exec(&mut c, "T = Vertex(q)").unwrap();
        c.rollback(at).unwrap();
        c.set_number(n, 0.5).unwrap();
        let p = script::exec(&mut c, "P = Vertex(q)").unwrap().unwrap();
        c.set_number(n, 2.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
    }

    #[test]
    fn redefining_an_unrelated_number_resets_helpers_only_on_a_rebuild() {
        for (dependent, expected) in [
            ("b = a + 1", Value::Undefined),
            ("", Value::Point { x: 4.0, y: 2.0 }),
        ] {
            let mut c = script::load(&format!(
                "n = 2\nq(x,y) = y > x^n && x > 4 && y > 0\nP = Vertex(q)\na = 5\n{dependent}"
            ))
            .unwrap();
            let p = c.id("P_{1}").unwrap();
            c.set_number(c.id("n").unwrap(), 0.5).unwrap();
            assert_eq!(c.value(p), Value::Point { x: 4.0, y: 2.0 });

            script::exec(&mut c, "a = 3*n").unwrap();
            assert_eq!(c.value(p), expected);
        }
    }

    #[test]
    fn a_differently_written_coefficient_rebuilds_inequality_helpers() {
        let mut c = script::load(concat!(
            "n = 2\nq(x,y) = y > x^n && x > 4 && y > 0\nP = Vertex(q)\n",
            "a = 3*n\nb = a + 1",
        ))
        .unwrap();
        let p = c.id("P_{1}").unwrap();
        c.set_number(c.id("n").unwrap(), 0.5).unwrap();
        assert_eq!(c.value(p), Value::Point { x: 4.0, y: 2.0 });
        script::exec(&mut c, "a = 3.0 * n").unwrap();
        assert_eq!(c.value(p), Value::Undefined);
    }

    #[test]
    fn an_identical_point_parameter_keeps_inequality_helpers() {
        let mut c = script::load(concat!(
            "n = 2\nq(x,y) = y > x^n && x > 4 && y > 0\nP = Vertex(q)\n",
            "A = (0, 0)\nB = (2, 0)\ns = Segment(A, B)\nm = 0.25\n",
            "T = Point(s, 2*m)\nu = x(T)",
        ))
        .unwrap();
        let p = c.id("P_{1}").unwrap();
        c.set_number(c.id("n").unwrap(), 0.5).unwrap();
        let vertex = Value::Point { x: 4.0, y: 2.0 };
        assert_eq!(c.value(p), vertex);
        script::exec(&mut c, "T = Point(s, 2*m)").unwrap();
        assert_eq!(c.value(p), vertex);
        c.set_number(c.id("m").unwrap(), 0.375).unwrap();
        assert_eq!(c.value(c.id("u").unwrap()), Value::Number(1.5));
        script::exec(&mut c, "T = Point(s, 3*m)").unwrap();
        assert_eq!(c.value(p), Value::Undefined);
    }

    #[test]
    fn redefining_the_inequality_rebuilds_its_polynomial_helper() {
        let mut c =
            script::load("n = 2\nq(x,y) = y > x^n && x > 4 && y > 0\nP = Vertex(q)").unwrap();
        let p = c.id("P_{1}").unwrap();
        script::exec(&mut c, "q(x,y) = y > sqrt(x) && x > 4 && y > 0").unwrap();
        assert_eq!(c.value(p), Value::Undefined);
    }

    #[test]
    fn inequality_vertices_recover_after_an_exponent_exceeds_the_storage_budget() {
        let mut c = script::load("n = 2\nq(x,y) = y > x^n && x < 1\nP = Vertex(q)").unwrap();
        let p = c.id("P_{1}").or_else(|| c.id("P")).unwrap();
        let n = c.id("n").unwrap();
        let vertex = Value::Point { x: 1.0, y: 1.0 };
        assert_eq!(c.value(p), vertex);
        c.set_number(n, 1_000_000_000.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        c.set_number(n, 2.0).unwrap();
        assert_eq!(c.value(p), vertex);
    }
}
