// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootInterval.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoExtremumMulti.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoComplexRootsPolynomial.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{Point, RootInput};
use crate::algo::intersect::{
    bracketed_root, complex_roots,
    function_roots::{defined_interval, interval_roots, newton, root_near},
    graph_roots, polynomial_analysis_roots,
};
use crate::expr::{brent_minimum, IntersectionWork};
use crate::kernel::locus::HEADLESS_VIEW;
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};
use crate::{Construction, Id};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Mode {
    Polynomial,
    Roots(Option<(u32, u32)>),
    Seed(u32),
    SingleInterval(u32, u32),
    Extremum(Option<(u32, u32)>),
    TurningPoint,
    Complex,
    Discontinuity,
}

pub(super) fn solve(c: &Construction, input: RootInput) -> Vec<Point> {
    let RootInput { function, mode } = input;
    let work = IntersectionWork::new();
    let f = |x| work.sample(c, function, x);
    let derivative = |x| work.derivative(c, function, x).unwrap_or(f64::NAN);
    let view = c.locus_view().unwrap_or(HEADLESS_VIEW);
    let bounds = |pair: Option<(u32, u32)>| {
        pair.map_or((view[0], view[1]), |(a, b)| (number(c, a), number(c, b)))
    };
    let real_points = |roots: Vec<f64>| roots.into_iter().map(|x| (x, 0.0)).collect();
    let scan_roots = |(a, b)| interval_roots(&f, a, b, view[4], || work.exhausted());
    let points = match mode {
        Mode::Seed(seed) => real_points(
            c.number(Id(seed as usize))
                .and_then(|seed| root_near(&f, derivative, seed, || work.inverse(c, function, 0.0)))
                .into_iter()
                .collect(),
        ),
        Mode::SingleInterval(a, b) => real_points(
            single_root(&f, derivative, number(c, a), number(c, b))
                .into_iter()
                .collect(),
        ),
        Mode::Roots(pair) => real_points(scan_roots(bounds(pair))),
        Mode::Extremum(Some(pair)) => {
            let (a, b) = bounds(Some(pair));
            extrema(&f, a, b, view[4], &work)
        }
        Mode::Discontinuity => discontinuities(c, function, &work),
        Mode::Complex => {
            let mut roots: Vec<Point> = Vec::new();
            for polynomial in work
                .analysis_polynomials(c, function, 0)
                .into_iter()
                .flatten()
            {
                for root in complex_roots(&polynomial, &|n| work.charge(n)) {
                    if !work.charge(roots.len() as u64 + 1) {
                        return Vec::new();
                    }
                    if !roots.iter().any(|prior| {
                        (prior.0 - root.0).abs() < STANDARD_PRECISION
                            && (prior.1 - root.1).abs() < STANDARD_PRECISION
                    }) {
                        roots.push(root);
                    }
                }
            }
            roots
        }
        Mode::Polynomial | Mode::Extremum(None) | Mode::TurningPoint => {
            let order = match mode {
                Mode::Extremum(_) => 1,
                Mode::TurningPoint => 2,
                _ => 0,
            };
            if let Some(polynomials) = work.analysis_polynomials(c, function, order) {
                let mut roots: Vec<_> = polynomials
                    .iter()
                    .flat_map(|poly| polynomial_analysis_roots(poly, &|n| work.charge(n)))
                    .collect();
                roots.sort_by(f64::total_cmp);
                roots.dedup_by(|a, b| (*a - *b).abs() < STANDARD_PRECISION);
                roots
                    .into_iter()
                    .filter(|&x| {
                        order == 0
                            || polynomials
                                .iter()
                                .any(|p| work.charge(2 * p.len() as u64) && changes_sign(p, x))
                    })
                    .map(|x| {
                        let y = f(x);
                        (x, if order == 0 && y.is_finite() { 0.0 } else { y })
                    })
                    .collect()
            } else if work.analysis_is_restricted(c, function) {
                Vec::new()
            } else {
                let (a, b) = bounds(None);
                match mode {
                    Mode::Extremum(_) => extrema(&f, a, b, view[4], &work),
                    Mode::TurningPoint => Vec::new(),
                    _ => real_points(scan_roots((a, b))),
                }
            }
        }
    };
    if work.exhausted() {
        Vec::new()
    } else {
        points
    }
}

fn number(c: &Construction, index: u32) -> f64 {
    match c.value(Id(index as usize)) {
        crate::Value::Number(n) => n,
        _ => f64::NAN,
    }
}

fn single_root(
    f: &impl Fn(f64) -> f64,
    derivative: impl Fn(f64) -> f64,
    a: f64,
    b: f64,
) -> Option<f64> {
    let solve = |l, r| bracketed_root(f, l, r, l + (r - l) * 0.5, 1e-6);
    let root = solve(a, b).or_else(|| {
        let (l, r) = defined_interval(f, a, b)?;
        solve(l, r)
    })?;
    let refined = newton(f, derivative, root)
        .filter(|&x| f(x).abs() < f(root).abs())
        .unwrap_or(root);
    (f(refined).abs() < MIN_PRECISION).then_some(refined)
}

fn changes_sign(poly: &[f64], x: f64) -> bool {
    let eval = |x| poly.iter().rev().fold(0.0, |v, a| v * x + a);
    let delta = 1e-5 * x.abs().max(1.0);
    let (a, b) = (eval(x - delta), eval(x + delta));
    (a < 0.0 && b > 0.0) || (a > 0.0 && b < 0.0)
}

fn extrema(
    f: &impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    scale: f64,
    work: &IntersectionWork,
) -> Vec<Point> {
    if !a.is_finite() || !b.is_finite() || a == b {
        return Vec::new();
    }
    let (a, b) = (a.min(b), a.max(b));
    let mut samples = ((b - a) * scale / 5.0).round().clamp(50.0, 400.0) as usize;
    loop {
        let step = (b - a) / samples as f64;
        let mut found = Vec::new();
        let mut y = f(a);
        let mut increasing = false;
        for i in 1..=samples {
            if !work.charge(1) {
                return Vec::new();
            }
            let current = f(a + i as f64 * step);
            let next = current >= y;
            if i > 1 && increasing != next {
                let lo = a + (i - 2) as f64 * step;
                let hi = lo + 2.0 * step;
                let signed = |x| if increasing { -f(x) } else { f(x) };
                if let Some(x) = brent_minimum(lo, hi, 3e-8, signed, |n| !work.charge(n)) {
                    if gradient_changes_sign(f, x, lo, hi) {
                        let decimals = x * 1e5;
                        let nearest = (decimals + 0.5).floor();
                        let rounded = nearest / 1e5;
                        let x = if (decimals - nearest).abs() < 0.1 && signed(rounded) <= signed(x)
                        {
                            rounded
                        } else {
                            x
                        };
                        found.push((x, f(x)));
                    }
                }
            }
            (y, increasing) = (current, next);
        }
        if found.len() < samples / 2 || samples * 2 >= 400 {
            return found;
        }
        samples *= 2;
    }
}

fn gradient_changes_sign(f: &impl Fn(f64) -> f64, x: f64, l: f64, r: f64) -> bool {
    let dx = (r - l) / 1e8;
    let (v, right, left) = (f(x), f(x + dx), f(x - dx));
    if right == v && left == v && f(x + 1e6 * dx) == v && f(x - 1e6 * dx) == v {
        return false;
    }
    (right >= v && left >= v) || (right <= v && left <= v) || ((right - v) / dx).abs() < 1e-4
}

fn discontinuities(c: &Construction, function: Id, work: &IntersectionWork) -> Vec<Point> {
    if let Some((numerator, denominator)) = work.fraction(c, function) {
        return graph_roots(&denominator, &|n| work.charge(n))
            .into_iter()
            .filter_map(|x| rational_limit(&numerator, &denominator, x, work).map(|y| (x, y)))
            .collect();
    }
    let polynomials = work.analysis_denominators(c, function);
    let mut holes: Vec<_> = polynomials
        .iter()
        .flat_map(|poly| graph_roots(poly, &|n| work.charge(n)))
        .filter_map(|x| numerical_limit(c, function, x, work).map(|y| (x, y)))
        .collect();
    holes.sort_by(|a, b| a.0.total_cmp(&b.0));
    holes.dedup_by(|a, b| (a.0 - b.0).abs() < STANDARD_PRECISION);
    holes
}

pub(super) fn multiplicity(poly: &[f64], x: f64, work: &IntersectionWork) -> Option<(usize, f64)> {
    let mut poly = poly.to_vec();
    let mut order = 0;
    while !poly.is_empty() && work.charge(poly.len() as u64) {
        let (v, scale) = poly.iter().rev().fold((0.0, 0.0), |(v, scale), &a| {
            (v * x + a, scale * x.abs() + a.abs())
        });
        if v.abs() > 1e-10 * scale {
            return Some((order, v));
        }
        order += 1;
        poly = poly
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, a)| i as f64 * a)
            .collect();
    }
    None
}

fn rational_limit(
    numerator: &[f64],
    denominator: &[f64],
    x: f64,
    work: &IntersectionWork,
) -> Option<f64> {
    let (p_order, p) = multiplicity(numerator, x, work).unwrap_or((usize::MAX, 0.0));
    let (q_order, q) = multiplicity(denominator, x, work)?;
    match p_order.cmp(&q_order) {
        std::cmp::Ordering::Less => None,
        std::cmp::Ordering::Equal => Some(p / q).filter(|v| v.is_finite()),
        std::cmp::Ordering::Greater => Some(0.0),
    }
}

fn numerical_limit(c: &Construction, function: Id, x: f64, work: &IntersectionWork) -> Option<f64> {
    let sample = |h: f64| work.sample(c, function, x + h);
    let h = 0.01 * x.abs().max(1.0);
    let symmetric = |d| (sample(d) + sample(-d)) * 0.5;
    let a = symmetric(h);
    let b = symmetric(h / 2.0);
    let d = symmetric(h / 4.0);
    let coarse = (4.0 * b - a) / 3.0;
    let fine = (4.0 * d - b) / 3.0;
    let value = (16.0 * fine - coarse) / 15.0;
    let scale = value.abs().max(1.0);
    let even_convergence = (value - fine).abs() <= 1e-8 * scale;
    let odd_convergence = (sample(h / 4.0) - sample(-h / 4.0)).abs() <= 1e-3 * scale;
    (value.is_finite() && even_convergence && odd_convergence).then_some(value)
}
