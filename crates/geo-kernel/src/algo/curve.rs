// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Derivative.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/LengthFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoLengthCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoCurveCartesianND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoIntegralDefinite.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoCurveCartesian.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::derivative::FunctionDerivative;
use crate::expr::IntersectionWork;
use crate::tol::STANDARD_PRECISION;
use crate::{Construction, Id, Value};

use super::{transform::TransformDef, vector::matrix::Matrix};

/// Bounds of the reference function-to-curve conversion. Only legacy saved
/// interval functions carry a domain in addition to their expression.
pub(super) fn function_graph_range(c: &Construction, id: Id) -> Option<Value> {
    let interval = crate::expr::function_interval(c, id);
    if interval.is_none() && c.value(id) != Value::Function {
        return None;
    }
    let [start, end] = interval.flatten().unwrap_or_else(|| {
        let [xmin, xmax, ymin, ymax, ..] = c
            .locus_view()
            .unwrap_or(crate::kernel::locus::HEADLESS_VIEW);
        if xmin == xmax {
            [-10.0, 10.0]
        } else {
            [
                (2.0 * xmin) - xmax + ymin - ymax,
                (2.0 * xmax) - xmin + ymax - ymin,
            ]
        }
    });
    (start <= end).then_some(Value::Curve { start, end })
}

/// A bracket with a nonpositive left value and a nonnegative right value.
pub(crate) fn bisect(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    for _ in 0..60 {
        let mid = lo.midpoint(hi);
        if mid == lo || mid == hi {
            break;
        }
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo.midpoint(hi)
}

/// Differentiate through the public function evaluator. The symmetric
/// five-point rule cancels the quadratic error of a centered difference.
pub(crate) fn slope(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    let h = 0.0005 * x.abs().max(1.0);
    let coarse = f(x + 2.0 * h) - f(x - 2.0 * h);
    let fine = f(x + h) - f(x - h);
    (8.0 * fine - coarse) / (12.0 * h)
}

/// `LengthFunction.integral`: the arc length of the graph of `id` between
/// `a` and `b`. `T = hypot(1, f'(x))` is integrated by GeoGebra's adaptive
/// Gauss quadrature; an end where `T` is infinite but the function has a
/// value is replaced by a short chord.
pub(crate) fn length(c: &Construction, id: Id, a: f64, b: f64) -> Option<f64> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let function = FunctionDerivative::new(c, id)?;
    let f = |x| function.value(x);
    let t = |x| 1.0f64.hypot(function.slope(x));
    let mut min = a;
    let mut max = b;
    let mut edges = 0.0;
    let dx = (b - a) * crate::tol::MAX_PRECISION;
    if t(min).is_infinite() {
        edges += dx.hypot(f(min + dx) - f(min));
        min += dx;
    }
    if t(max).is_infinite() {
        edges += dx.hypot(f(max - dx) - f(max));
        max -= dx;
    }
    Some(super::elliptic::numeric_integration(&t, min, max).abs() + edges)
}

/// Search for stationary points of squared distance in the interval that
/// can improve on the point at the source's x coordinate.
pub(crate) fn closest(c: &Construction, id: Id, (px, py): (f64, f64)) -> Option<(f64, f64)> {
    let f = |x| c.function_value(id, x).unwrap_or(f64::NAN);
    let squared = |x: f64| (x - px).powi(2) + (f(x) - py).powi(2);
    let mut best = px;
    let mut distance = squared(px);
    if !distance.is_finite() {
        let boundary = |sign: f64| {
            let mut offset = sign * 0.1;
            while offset.abs() < 1e15 {
                let mut to = px + offset;
                if !f(to).is_nan() {
                    let mut from = px + offset / 2.0;
                    for _ in 0..15 {
                        let mid = from.midpoint(to);
                        if squared(mid).is_nan() || squared(to) < squared(mid) {
                            from = mid;
                        } else {
                            to = mid;
                        }
                    }
                    return to;
                }
                offset *= 2.0;
            }
            f64::NAN
        };
        let (left, right) = (boundary(-1.0), boundary(1.0));
        best = if squared(left) < squared(right) {
            left
        } else {
            right
        };
    } else if distance > 0.0 && !crate::expr::is_sampled_function(c, id) {
        // The fast derivative of a sampled function is undefined, so the
        // reference has no stationary roots to replace the point at px.
        let radius = distance.sqrt();
        let steps = (20.0 * radius).clamp(200.0, 10_000.0) as usize;
        let left = px - radius;
        let right = px + radius;
        let work = IntersectionWork::new();
        let polynomial = work
            .fraction(c, id)
            .is_some_and(|(_, denominator)| denominator.len() == 1);
        let start = crate::expr::work_done();
        let exhausted = || crate::expr::work_done() - start > crate::expr::MAX_EVALUATION_WORK;
        if polynomial {
            let derivative = |x| x - px + (f(x) - py) * slope(f, x);
            let mut lo = left;
            let mut before = derivative(lo);
            for i in 1..=steps {
                if exhausted() {
                    return None;
                }
                let hi = px + radius * (2.0 * i as f64 / steps as f64 - 1.0);
                let after = derivative(hi);
                if before <= 0.0 && after >= 0.0 {
                    let root = bisect(lo, hi, derivative);
                    let value = squared(root);
                    if value < distance {
                        best = root;
                        distance = value;
                    }
                }
                (lo, before) = (hi, after);
            }
        } else {
            // AlgoDistancePointObject builds the derivative of squared distance
            // and passes its roots through AlgoRoots' residual and hole checks.
            let analytic = FunctionDerivative::new(c, id);
            let derivative = |x| {
                let dy = analytic
                    .as_ref()
                    .map(|function| function.slope(x))
                    .filter(|slope| slope.is_finite())
                    .unwrap_or_else(|| slope(f, x));
                2.0 * (x - px) + 2.0 * ((f(x) - py) * dy)
            };
            for root in super::intersect::function_roots::fixed_steps_roots(
                &derivative,
                left,
                right,
                steps,
                exhausted,
            ) {
                let value = squared(root);
                if distance > value + STANDARD_PRECISION {
                    best = root;
                    distance = value;
                }
            }
        }
        if exhausted() {
            return None;
        }
    }
    let y = f(best);
    (best.is_finite() && y.is_finite()).then_some((best, y))
}

/// A Cartesian curve evaluated with a shared work limit across coordinates.
pub(super) struct Cartesian<'a> {
    source: CurveSource<'a>,
    start: f64,
    end: f64,
    work_at_start: u64,
}

enum CurveSource<'a> {
    Coordinates([Id; 2], Box<[FunctionDerivative<'a>; 2]>),
    Spline(std::sync::Arc<super::analysis::Spline>),
    /// A curve derivative: both coordinates sample the calculus record's
    /// two trees (`Derivative(curve)`).
    Calculus(&'a Construction, Id),
    ParametricDerivative(Box<Cartesian<'a>>),
    Dilated {
        original: Box<Cartesian<'a>>,
        ratio: f64,
        center: (f64, f64),
    },
    MatrixCurve(Box<Cartesian<'a>>, Matrix),
    MatrixGraph {
        construction: &'a Construction,
        function: Id,
        derivative: Option<Box<FunctionDerivative<'a>>>,
        matrix: Matrix,
    },
}

enum LinePolynomial<'a> {
    Coefficients(Vec<f64>),
    Spline(&'a super::analysis::Spline, (f64, f64, f64)),
}

fn line_coefficients(
    x: &[f64],
    y: &[f64],
    (a, b, constant): (f64, f64, f64),
    work: &IntersectionWork,
) -> Option<Vec<f64>> {
    let len = x.len().max(y.len());
    if !work.charge(len as u64) {
        return None;
    }
    let mut coefficients = vec![0.0; len];
    for (coefficient, v) in coefficients.iter_mut().zip(x) {
        *coefficient = a * v;
    }
    for (coefficient, v) in coefficients.iter_mut().zip(y) {
        *coefficient += b * v;
    }
    coefficients[0] += constant;
    Some(coefficients)
}

fn matrix_value(matrix: Matrix, value: (f64, f64)) -> (f64, f64) {
    let (x, y, w) = matrix.curve_coordinates(value, 1.0);
    (x / w, y / w)
}

fn matrix_first(matrix: Matrix, value: (f64, f64), first: (f64, f64)) -> (f64, f64) {
    let (x, y, w) = matrix.curve_coordinates(value, 1.0);
    let (dx, dy, dw) = matrix.curve_coordinates(first, 0.0);
    let square = w * w;
    ((dx * w - x * dw) / square, (dy * w - y * dw) / square)
}

impl<'a> Cartesian<'a> {
    pub(super) fn new(c: &'a Construction, id: Id) -> Option<Self> {
        Self::with_depth(c, id, 12)
    }

    pub(super) fn retained_spline(
        c: &Construction,
        id: Id,
        input: super::analysis::SplineInput,
    ) -> Option<Self> {
        Some(Self {
            source: CurveSource::Spline(c.analysis.retained_spline(id, input)?),
            start: 0.0,
            end: 1.0,
            work_at_start: crate::expr::work_done(),
        })
    }

    fn with_depth(c: &'a Construction, id: Id, depth: u8) -> Option<Self> {
        use super::{analysis::AnalysisDef, measure::MeasureDef, vector::VectorDef};
        use crate::{Def, Kind};
        let id = c.conditional_source(id)?;
        let Value::Curve { start, end } = c.value(id) else {
            return None;
        };
        let source = match c.def(id) {
            Def::Measure(MeasureDef::CartesianCurve {
                coordinates: [x, y],
                ..
            }) => {
                let ids = [Id(x as usize), Id(y as usize)];
                CurveSource::Coordinates(
                    ids,
                    Box::new([
                        FunctionDerivative::new(c, ids[0])?,
                        FunctionDerivative::new(c, ids[1])?,
                    ]),
                )
            }
            Def::Analysis(AnalysisDef::Spline(input)) => {
                CurveSource::Spline(c.analysis.spline(c, input)?)
            }
            Def::Calculus(super::calculus::CalculusDef::CurveDerivative { .. }) => {
                CurveSource::Calculus(c, id)
            }
            Def::Analysis(AnalysisDef::ParametricDerivative(id)) => {
                CurveSource::ParametricDerivative(Box::new(Self::with_depth(
                    c,
                    id,
                    depth.checked_sub(1)?,
                )?))
            }
            Def::Transform(TransformDef::Dilate {
                src,
                factor,
                center,
                ..
            }) => CurveSource::Dilated {
                original: Box::new(Self::with_depth(c, src, depth.checked_sub(1)?)?),
                ratio: c.number(factor).unwrap_or(f64::NAN),
                center: center.map_or((0.0, 0.0), |id| c.point(id).unwrap_or((f64::NAN, f64::NAN))),
            },
            Def::Vector(VectorDef::ApplyMatrix {
                matrix,
                src,
                kind: Kind::Curve,
            }) => CurveSource::MatrixCurve(
                Box::new(Self::with_depth(c, src, depth.checked_sub(1)?)?),
                Matrix::read(c.elements(matrix)?)?,
            ),
            Def::Vector(VectorDef::ApplyMatrix {
                matrix,
                src,
                kind: Kind::Function,
            }) => CurveSource::MatrixGraph {
                construction: c,
                function: src,
                derivative: FunctionDerivative::for_curve_graph(c, src).map(Box::new),
                matrix: Matrix::read(c.elements(matrix)?)?,
            },
            _ => return None,
        };
        Some(Self {
            source,
            start,
            end,
            work_at_start: crate::expr::work_done(),
        })
    }

    pub(super) fn polynomial_line_roots(
        &self,
        c: &Construction,
        work: &IntersectionWork,
        line: (f64, f64, f64),
    ) -> Option<Vec<f64>> {
        use super::intersect::graph_roots;
        match self.line_polynomial(c, work, line)? {
            LinePolynomial::Coefficients(coefficients) => {
                let roots = graph_roots(&coefficients, &|units| work.charge(units));
                (!roots.is_empty()).then_some(roots)
            }
            LinePolynomial::Spline(spline, (a, b, constant)) => {
                let mut roots = Vec::new();
                for (left, right, row) in spline.pieces() {
                    let mut coefficients: Vec<f64> =
                        row.iter().rev().map(|&[x, y]| a * x + b * y).collect();
                    coefficients[0] += constant;
                    for parameter in graph_roots(&coefficients, &|units| work.charge(units)) {
                        // Adjacent pieces meet at a knot. Rounding can put either
                        // polynomial's root just outside its own interval.
                        if (left - STANDARD_PRECISION..=right + STANDARD_PRECISION)
                            .contains(&parameter)
                        {
                            roots.push(parameter.clamp(left, right));
                        }
                    }
                    if work.exhausted() {
                        return Some(Vec::new());
                    }
                }
                Some(roots)
            }
        }
    }

    pub(super) fn polynomial_coordinates(
        &self,
        c: &Construction,
        work: &IntersectionWork,
    ) -> Option<[Vec<f64>; 2]> {
        let coordinate = |line| match self.line_polynomial(c, work, line)? {
            LinePolynomial::Coefficients(values) => Some(values),
            LinePolynomial::Spline(..) => None,
        };
        Some([coordinate((1.0, 0.0, 0.0))?, coordinate((0.0, 1.0, 0.0))?])
    }

    fn line_polynomial(
        &self,
        c: &Construction,
        work: &IntersectionWork,
        (a, b, constant): (f64, f64, f64),
    ) -> Option<LinePolynomial<'_>> {
        let coefficients = match &self.source {
            CurveSource::Coordinates([x, y], _) => {
                let x = work.curve_polynomial(c, *x)?;
                let y = work.curve_polynomial(c, *y)?;
                line_coefficients(&x, &y, (a, b, constant), work)?
            }
            // a derivative curve keeps its own trees; a line intersection
            // falls back to the numeric path
            CurveSource::Calculus(..) => return None,
            CurveSource::Spline(spline) => {
                return Some(LinePolynomial::Spline(spline, (a, b, constant)));
            }
            CurveSource::Dilated {
                original,
                ratio,
                center: (x, y),
            } => {
                return original.line_polynomial(
                    c,
                    work,
                    (
                        a * ratio,
                        b * ratio,
                        constant + (1.0 - ratio) * (a * x + b * y),
                    ),
                );
            }
            CurveSource::MatrixCurve(original, matrix) => {
                let line = matrix.pull_back_affine_line((a, b, constant))?;
                return original.line_polynomial(c, work, line);
            }
            CurveSource::MatrixGraph {
                function, matrix, ..
            } => {
                let line = matrix.pull_back_affine_line((a, b, constant))?;
                let y = work.curve_polynomial(c, *function)?;
                line_coefficients(&[0.0, 1.0], &y, line, work)?
            }
            CurveSource::ParametricDerivative(original) => {
                let LinePolynomial::Coefficients(x) =
                    original.line_polynomial(c, work, (1.0, 0.0, 0.0))?
                else {
                    return None;
                };
                let dx = *x.get(1)?;
                if dx == 0.0 || x.iter().skip(2).any(|&coefficient| coefficient != 0.0) {
                    return None;
                }
                let LinePolynomial::Coefficients(y) =
                    original.line_polynomial(c, work, (0.0, 1.0, 0.0))?
                else {
                    return None;
                };
                let dy: Vec<_> = y
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(power, coefficient)| power as f64 * coefficient)
                    .collect();
                // With a nonzero constant x', y'/x' is also polynomial.
                line_coefficients(&x, &dy, (a, b / dx, constant), work)?
            }
        };
        Some(LinePolynomial::Coefficients(coefficients))
    }

    pub(super) fn value(&self, t: f64) -> (f64, f64) {
        if !self.has_budget() {
            return (f64::NAN, f64::NAN);
        }
        match &self.source {
            CurveSource::Coordinates(_, f) => (f[0].value(t), f[1].value(t)),
            CurveSource::Spline(spline) => spline.sample(t, 0),
            CurveSource::Calculus(c, id) => calculus_sample(c, *id, t),
            CurveSource::ParametricDerivative(original) => {
                let (x, _) = original.value(t);
                let (dx, dy) = original.first(t);
                (x, dy / dx)
            }
            CurveSource::Dilated {
                original,
                ratio,
                center: (cx, cy),
            } => {
                let (x, y) = original.value(t);
                (ratio * (x - cx) + cx, ratio * (y - cy) + cy)
            }
            CurveSource::MatrixCurve(original, matrix) => matrix_value(*matrix, original.value(t)),
            CurveSource::MatrixGraph {
                construction,
                function,
                derivative,
                matrix,
            } => {
                let y = derivative.as_ref().map_or_else(
                    || {
                        construction
                            .function_value(*function, t)
                            .unwrap_or(f64::NAN)
                    },
                    |f| f.value(t),
                );
                matrix_value(*matrix, (t, y))
            }
        }
    }

    pub(super) fn first(&self, t: f64) -> (f64, f64) {
        if !self.has_budget() {
            return (f64::NAN, f64::NAN);
        }
        match &self.source {
            CurveSource::Coordinates(_, f) => (f[0].slope(t), f[1].slope(t)),
            CurveSource::Spline(spline) => spline.sample(t, 1),
            CurveSource::Calculus(c, id) => {
                let x = |t| calculus_sample(c, *id, t).0;
                let y = |t| calculus_sample(c, *id, t).1;
                (slope(x, t), slope(y, t))
            }
            CurveSource::ParametricDerivative(original) => {
                let (dx, dy) = original.first(t);
                let (ddx, ddy) = original.second(t);
                (dx, (ddy * dx - ddx * dy) / (dx * dx))
            }
            CurveSource::Dilated {
                original, ratio, ..
            } => {
                let (dx, dy) = original.first(t);
                (ratio * dx, ratio * dy)
            }
            CurveSource::MatrixCurve(original, matrix) => {
                matrix_first(*matrix, original.value(t), original.first(t))
            }
            CurveSource::MatrixGraph {
                construction,
                function,
                derivative,
                matrix,
            } => {
                let evaluate = |x| {
                    construction
                        .function_value(*function, x)
                        .unwrap_or(f64::NAN)
                };
                let (y, dy) = derivative.as_ref().map_or_else(
                    || (evaluate(t), slope(evaluate, t)),
                    |f| (f.value(t), f.slope(t)),
                );
                matrix_first(*matrix, (t, y), (1.0, dy))
            }
        }
    }

    pub(super) fn second(&self, t: f64) -> (f64, f64) {
        if let CurveSource::Spline(spline) = &self.source {
            return spline.sample(t, 2);
        }
        (slope(|t| self.first(t).0, t), slope(|t| self.first(t).1, t))
    }

    pub(super) fn parameter_bounds(&self) -> (f64, f64) {
        (self.start, self.end)
    }

    pub(super) fn has_budget(&self) -> bool {
        crate::expr::work_done().wrapping_sub(self.work_at_start) < crate::expr::MAX_EVALUATION_WORK
    }

    pub(super) fn length(&self, start: f64, end: f64) -> f64 {
        let integrand = |t| {
            if !self.has_budget() {
                return f64::NAN;
            }
            let (x, y) = self.first(t);
            (x * x + y * y).sqrt()
        };
        super::elliptic::numeric_integration(&integrand, start, end).abs()
    }

    pub(super) fn closest(&self, (px, py): (f64, f64)) -> f64 {
        self.closest_from((px, py), self.start)
    }

    pub(super) fn closest_from(&self, (px, py): (f64, f64), seed: f64) -> f64 {
        let squared = |t| {
            if !self.has_budget() {
                return f64::NAN;
            }
            let (x, y) = self.value(t);
            let (dx, dy) = (x - px, y - py);
            dx * dx + dy * dy
        };
        // GeoCurveCartesianND samples before refining one bracket.
        let step = (self.end - self.start) / 100.0;
        let mut t = self.start;
        let mut best = t;
        let mut distance = squared(t);
        for _ in 0..100 {
            t += step;
            let d = squared(t);
            if d < distance || distance.is_nan() {
                best = t;
                distance = d;
            }
        }
        let left = self.start.max(best - step);
        let right = self.end.min(best + step);
        let adjust = |t| {
            if t < self.start {
                t + (self.end - self.start)
            } else if t > self.end {
                t - (self.end - self.start)
            } else {
                t
            }
        };
        let result = adjust(minimum(left, right, squared));
        if seed < left || right < seed {
            let alternate = adjust(minimum(seed - step, seed + step, squared));
            if squared(alternate) < squared(result) + crate::tol::MIN_PRECISION / 2.0 {
                return alternate;
            }
        }
        result
    }
}

/// Samples a Cartesian curve with one shared expression allowance and clips
/// each continuous run to the frame.
pub fn plot(c: &Construction, id: Id, frame: &crate::plot::Frame) -> Vec<crate::plot::Path> {
    let deadline = crate::expr::work_done().saturating_add(crate::plot::DRAW_DEADLINE);
    crate::expr::with_work_deadline(deadline, || {
        const STEPS: u32 = 256;
        let Some(curve) = Cartesian::new(c, id) else {
            return Vec::new();
        };
        if !frame.resolves() {
            return Vec::new();
        }
        let sample = |i: u32| {
            let f = f64::from(i) / f64::from(2 * STEPS);
            curve.value(curve.start * (1.0 - f) + curve.end * f)
        };
        let mut cutter = crate::plot::Cutter::new(frame);
        cutter.move_to(sample(0));
        for i in 1..=STEPS {
            if crate::expr::work_done() >= deadline || !curve.has_budget() {
                break;
            }
            cutter.line_to(sample(2 * i - 1));
            cutter.line_to(sample(2 * i));
        }
        cutter.finish()
    })
}

impl Construction {
    /// Evaluates a Cartesian curve at its parameter and keeps finite points.
    pub fn curve_point(&self, id: Id, parameter: f64) -> Option<(f64, f64)> {
        self.curve_sample(id, parameter)
            .filter(|&(x, y)| x.is_finite() && y.is_finite())
    }

    /// Evaluates a Cartesian curve at its parameter, including nonfinite coordinates.
    pub fn curve_sample(&self, id: Id, parameter: f64) -> Option<(f64, f64)> {
        Some(Cartesian::new(self, id)?.value(parameter))
    }
}

/// Both coordinates of the calculus-backed curve `id` at `t`: the two
/// derivative trees of its record.
fn calculus_sample(c: &Construction, id: Id, t: f64) -> (f64, f64) {
    match c.calculus_record(id) {
        Some(record) => {
            let x = record
                .tree
                .as_ref()
                .map_or(f64::NAN, |e| crate::expr::sample_tree(c, e, t));
            let y = record
                .tree2
                .as_ref()
                .map_or(f64::NAN, |e| crate::expr::sample_tree(c, e, t));
            (x, y)
        }
        None => (f64::NAN, f64::NAN),
    }
}

/// Brent minimization with a bounded golden-section fallback.
fn minimum(mut left: f64, mut right: f64, f: impl Fn(f64) -> f64) -> f64 {
    const GOLDEN: f64 = 0.381_966_011_250_105_1;
    let mut x = left + GOLDEN * (right - left);
    let (mut w, mut v) = (x, x);
    let mut fx = f(x);
    let (mut fw, mut fv) = (fx, fx);
    let (mut step, mut span): (f64, f64) = (0.0, 0.0);
    for _ in 0..128 {
        let mid = (left + right) / 2.0;
        let tolerance = f64::EPSILON.sqrt() * x.abs() + crate::tol::MIN_PRECISION / 3.0;
        if (x - mid).abs() <= 2.0 * tolerance - (right - left) / 2.0 {
            break;
        }
        let mut parabolic = false;
        if span.abs() > tolerance {
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            let mut q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            } else {
                q = -q;
            }
            let old_span = span;
            span = step;
            if p.abs() < (0.5 * q * old_span).abs() && p > q * (left - x) && p < q * (right - x) {
                step = p / q;
                let u = x + step;
                if u - left < 2.0 * tolerance || right - u < 2.0 * tolerance {
                    step = tolerance.copysign(mid - x);
                }
                parabolic = true;
            }
        }
        if !parabolic {
            span = if x >= mid { left - x } else { right - x };
            step = GOLDEN * span;
        }
        let u = x + if step.abs() >= tolerance {
            step
        } else {
            tolerance.copysign(step)
        };
        let fu = f(u);
        if fu <= fx {
            if u < x {
                right = x;
            } else {
                left = x;
            }
            (v, fv) = (w, fw);
            (w, fw) = (x, fx);
            (x, fx) = (u, fu);
        } else {
            if u < x {
                left = u;
            } else {
                right = u;
            }
            if fu <= fw || w == x {
                (v, fv) = (w, fw);
                (w, fw) = (u, fu);
            } else if fu <= fv || v == x || v == w {
                (v, fv) = (u, fu);
            }
        }
    }
    x
}
