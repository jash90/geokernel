//! Real roots of the determinant and elimination polynomials.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/EquationSolver.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::tol::STANDARD_PRECISION;

pub(super) fn quadratic([c, b, a]: [f64; 3], eps: f64) -> ([f64; 3], usize) {
    if a.abs() < eps {
        if b.abs() < eps {
            return ([0.0; 3], 0);
        }
        return ([-c / b, 0.0, 0.0], 1);
    }
    if b.abs() < eps * a.abs() {
        let square = -c / a;
        if square.abs() < eps {
            return ([0.0; 3], 1);
        }
        if square < 0.0 {
            return ([0.0; 3], 0);
        }
        let root = square.sqrt();
        return ([root, -root, 0.0], 2);
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant.abs() < eps * b * b {
        return ([-b / (2.0 * a), 0.0, 0.0], 1);
    }
    if discriminant < 0.0 {
        return ([0.0; 3], 0);
    }
    let q = -(b + discriminant.sqrt().copysign(b)) / 2.0;
    ([q / a, c / q, 0.0], 2)
}

/// `EquationSolver.solveCubicS`. Its order is significant in the cubic
/// elimination case, while the determinant pencil sorts the real roots.
pub(super) fn cubic(coefficients: [f64; 4], eps: f64) -> ([f64; 3], usize) {
    let [c, b, a, d] = coefficients;
    if d.abs() < eps {
        return quadratic([c, b, a], eps);
    }
    if c == 0.0 {
        let (mut roots, count) = quadratic([b, a, d], eps);
        roots[count] = 0.0;
        return (roots, count + 1);
    }
    let (a, b, c) = (a / d, b / d, c / d);
    let q = a * a - 3.0 * b;
    let r = 2.0 * a * a * a - 9.0 * a * b + 27.0 * c;
    let (q9, r54) = (q / 9.0, r / 54.0);
    let r2 = 729.0 * r * r;
    let q3 = 2916.0 * q * q * q;
    let offset = a / 3.0;
    if r54 == 0.0 && q9 == 0.0 {
        ([-offset; 3], 3)
    } else if (r2 - q3).abs()
        < r2.max(q3) * 10.0 / (2.0 * a + 3.0).abs().max(1.0) * STANDARD_PRECISION
    {
        let sq = q9.sqrt();
        let roots = if r54 > 0.0 {
            [-2.0 * sq - offset, sq - offset, sq - offset]
        } else {
            [-sq - offset, -sq - offset, 2.0 * sq - offset]
        };
        (roots, 3)
    } else if r2 < q3 {
        let sq = q9.sqrt();
        let angle = (r54 / (sq * sq * sq)).clamp(-1.0, 1.0).acos();
        let roots = [0.0, std::f64::consts::TAU, -std::f64::consts::TAU]
            .map(|delta| -2.0 * sq * ((angle + delta) / 3.0).cos() - offset);
        (roots, 3)
    } else {
        let root = -(r54.abs() + (r54 * r54 - q9 * q9 * q9).sqrt())
            .powf(1.0 / 3.0)
            .copysign(r54);
        ([root + q9 / root - offset, 0.0, 0.0], 1)
    }
}

fn evaluate_polynomial(coefficients: &[f64], x: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |value, c| value * x + c)
}

const ROOT_RELATIVE_ACCURACY: f64 = 1e-14;
const ROOT_ABSOLUTE_ACCURACY: f64 = 1e-6;
const ROOT_VALUE_ACCURACY: f64 = 1e-15;
const ROOT_ITERATIONS: usize = 100;
// Binary64 value of Euler's constant, the reference solver's alternate seed.
const LAGUERRE_ALTERNATE_SEED: f64 = f64::from_bits(0x3fe2_788c_fc6f_b619);

#[derive(Clone, Copy, PartialEq)]
struct Complex(f64, f64);

impl Complex {
    const ZERO: Self = Self(0.0, 0.0);

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0, self.1 + rhs.1)
    }

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0, self.1 - rhs.1)
    }

    fn scale(self, x: f64) -> Self {
        Self(self.0 * x, self.1 * x)
    }

    fn mul(self, rhs: Self) -> Self {
        Self(
            self.0 * rhs.0 - self.1 * rhs.1,
            self.0 * rhs.1 + self.1 * rhs.0,
        )
    }

    fn div(self, rhs: Self) -> Self {
        // Rescale by the dominant component to avoid a squared denominator.
        if rhs.0.abs() < rhs.1.abs() {
            let r = rhs.0 / rhs.1;
            let d = rhs.0 * r + rhs.1;
            Self((self.0 * r + self.1) / d, (self.1 * r - self.0) / d)
        } else {
            let r = rhs.1 / rhs.0;
            let d = rhs.1 * r + rhs.0;
            Self((self.1 * r + self.0) / d, (self.1 - self.0 * r) / d)
        }
    }

    fn norm(self) -> f64 {
        let (x, y) = (self.0.abs(), self.1.abs());
        let (large, small) = if x < y { (y, x) } else { (x, y) };
        if large == 0.0 || large.is_infinite() {
            return large;
        }
        let ratio = small / large;
        large * (1.0 + ratio * ratio).sqrt()
    }

    fn sqrt(self) -> Self {
        if self == Self::ZERO {
            return Self::ZERO;
        }
        let t = ((self.0.abs() + self.norm()) / 2.0).sqrt();
        if self.0 >= 0.0 {
            Self(t, self.1 / (2.0 * t))
        } else {
            Self(self.1.abs() / (2.0 * t), t.copysign(self.1))
        }
    }
}

/// One Laguerre root. Use Horner's method to evaluate the polynomial and
/// its two derivatives together, then take the larger Laguerre denominator.
fn laguerre(
    coefficients: &[Complex],
    remaining: &mut usize,
    start: f64,
    charge: &impl Fn(u64) -> bool,
) -> Option<Complex> {
    let degree = (coefficients.len() - 1) as f64;
    let (&leading, tail) = coefficients.split_last()?;
    let mut x = Complex(start, 0.0);
    let mut previous = Complex(f64::INFINITY, f64::INFINITY);
    while *remaining != 0 {
        if !charge(coefficients.len() as u64) {
            return None;
        }
        *remaining -= 1;
        let mut value = leading;
        let mut first = Complex::ZERO;
        let mut second = Complex::ZERO;
        for &coefficient in tail.iter().rev() {
            second = first.add(x.mul(second));
            first = value.add(x.mul(first));
            value = coefficient.add(x.mul(value));
        }
        let accuracy = (ROOT_RELATIVE_ACCURACY * x.norm()).max(ROOT_ABSOLUTE_ACCURACY);
        if x.sub(previous).norm() <= accuracy || value.norm() <= ROOT_VALUE_ACCURACY {
            return Some(x);
        }
        let g = first.div(value);
        let g_squared = g.mul(g);
        let h = g_squared.sub(second.scale(2.0).div(value));
        let radical = h.scale(degree).sub(g_squared).scale(degree - 1.0).sqrt();
        let plus = g.add(radical);
        let minus = g.sub(radical);
        let denominator = if plus.norm() > minus.norm() {
            plus
        } else {
            minus
        };
        if denominator == Complex::ZERO {
            x = x.add(Complex(ROOT_ABSOLUTE_ACCURACY, ROOT_ABSOLUTE_ACCURACY));
            previous = Complex(f64::INFINITY, f64::INFINITY);
        } else {
            previous = x;
            x = x.sub(Complex(degree, 0.0).div(denominator));
        }
        if !x.0.is_finite() || !x.1.is_finite() {
            return None;
        }
    }
    None
}

/// Find the roots, including multiplicities, by synthetic deflation.
fn quartic_candidates(coefficients: [f64; 5]) -> Option<[f64; 4]> {
    let mut coefficients = coefficients.map(|x| Complex(x, 0.0));
    let mut roots = [0.0; 4];
    let mut remaining = ROOT_ITERATIONS;
    for (i, slot) in roots.iter_mut().enumerate() {
        let degree = 4 - i;
        let root = laguerre(&coefficients[..=degree], &mut remaining, -1.0, &|_| true)?;
        *slot = root.0;
        let mut carry = coefficients[degree];
        for coefficient in coefficients[..degree].iter_mut().rev() {
            let next = coefficient.add(carry.mul(root));
            *coefficient = carry;
            carry = next;
        }
    }
    roots.sort_by(f64::total_cmp);
    Some(roots)
}

fn newton(
    coefficients: &[f64],
    derivative: &[f64],
    mut x: f64,
    charge: &impl Fn(u64) -> bool,
) -> Option<f64> {
    for _ in 0..ROOT_ITERATIONS {
        if !charge((coefficients.len() + derivative.len()) as u64) {
            return None;
        }
        let next = x - evaluate_polynomial(coefficients, x) / evaluate_polynomial(derivative, x);
        if !next.is_finite() {
            return None;
        }
        if (next - x).abs() <= ROOT_ABSOLUTE_ACCURACY {
            return Some(next);
        }
        x = next;
    }
    None
}

/// Brent's bracketed root search, with bisection when inverse
/// interpolation would not shrink the bracket fast enough.
fn brent(coefficients: &[f64], left: f64, right: f64) -> Option<f64> {
    bracketed_root(
        &|x| evaluate_polynomial(coefficients, x),
        left,
        right,
        left + 0.5 * (right - left),
        ROOT_ABSOLUTE_ACCURACY,
    )
}

pub(in crate::algo) fn bracketed_root(
    f: &impl Fn(f64) -> f64,
    left: f64,
    right: f64,
    middle: f64,
    absolute_accuracy: f64,
) -> Option<f64> {
    if !(left < middle && middle < right) {
        return None;
    }
    let fm = f(middle);
    if fm.abs() <= ROOT_VALUE_ACCURACY {
        return Some(middle);
    }
    let fl = f(left);
    if fl.abs() <= ROOT_VALUE_ACCURACY {
        return Some(left);
    }
    let (mut a, mut fa, mut b, mut fb) = if fl.is_finite() && fm.is_finite() && fl * fm < 0.0 {
        (left, fl, middle, fm)
    } else {
        let fr = f(right);
        if fr.abs() <= ROOT_VALUE_ACCURACY {
            return Some(right);
        }
        if fm * fr >= 0.0 || !fm.is_finite() || !fr.is_finite() {
            return None;
        }
        (middle, fm, right, fr)
    };
    let (mut c, mut fc) = (a, fa);
    let mut step = b - a;
    let mut previous_step = step;
    for _ in 0..ROOT_ITERATIONS {
        if fc.abs() < fb.abs() {
            (a, b, c) = (b, c, b);
            (fa, fb, fc) = (fb, fc, fb);
        }
        let accuracy = 2.0 * ROOT_RELATIVE_ACCURACY * b.abs() + absolute_accuracy;
        let half = 0.5 * (c - b);
        if half.abs() <= accuracy || fb.abs() <= f64::from_bits(1) {
            return Some(b);
        }
        if previous_step.abs() < accuracy || fa.abs() <= fb.abs() {
            step = half;
            previous_step = step;
        } else {
            let ratio = fb / fa;
            let (mut p, mut q) = if a == c {
                (2.0 * half * ratio, 1.0 - ratio)
            } else {
                let q = fa / fc;
                let r = fb / fc;
                (
                    ratio * (2.0 * half * q * (q - r) - (b - a) * (r - 1.0)),
                    (q - 1.0) * (r - 1.0) * (ratio - 1.0),
                )
            };
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            let old_step = previous_step;
            previous_step = step;
            if p >= 1.5 * half * q - (accuracy * q).abs() / 2.0 || p >= (0.5 * old_step * q).abs() {
                step = half;
                previous_step = step;
            } else {
                step = p / q;
            }
        }
        (a, fa) = (b, fb);
        b += if step.abs() > accuracy {
            step
        } else {
            accuracy.copysign(half)
        };
        fb = f(b);
        if (fb > 0.0) == (fc > 0.0) {
            (c, fc) = (a, fa);
            step = b - a;
            previous_step = step;
        }
    }
    None
}

/// `EquationSolver.laguerreAll` tests the sorted real parts of all four
/// candidates after polishing. A repeated root can retain two nearby
/// real approximations, and a complex root's real part may be accepted
/// at the polynomial residual tolerance.
fn quartic(coefficients: [f64; 5]) -> ([f64; 4], usize) {
    let Some(candidates) = quartic_candidates(coefficients) else {
        return ([0.0; 4], 0);
    };
    let [_, b, a, d, e] = coefficients;
    let derivative = [b, 2.0 * a, 3.0 * d, 4.0 * e];
    let value = |x| evaluate_polynomial(&coefficients, x);
    let better = |x, fallback| {
        if value(x).abs() < value(fallback).abs() {
            x
        } else {
            fallback
        }
    };
    let mut roots = [0.0; 4];
    let mut count = 0;
    for (i, x) in candidates.into_iter().enumerate() {
        let left = if i == 0 {
            x - 1.0
        } else {
            (x + candidates[i - 1]) / 2.0
        };
        let right = if i == 3 {
            x + 1.0
        } else {
            (x + candidates[i + 1]) / 2.0
        };
        let polished = if value(left) * value(right) < 0.0 {
            brent(&coefficients, left, right)
        } else {
            newton(&coefficients, &derivative, x, &|_| true)
        };
        let root = if let Some(polished) = polished {
            let root = better(polished, x);
            let fraction = root * 1e5;
            let rounded = super::super::java_round(fraction);
            if (fraction - rounded).abs() < 0.1 {
                better(rounded / 1e5, root)
            } else {
                root
            }
        } else {
            brent(&derivative, left, right).map_or(x, |critical| better(critical, x))
        };

        // Evaluate the derivative with the same simultaneous Horner recurrence.
        let mut fx = e;
        let mut dx = 0.0;
        for &coefficient in coefficients[..4].iter().rev() {
            dx = dx * root + fx;
            fx = fx * root + coefficient;
        }
        if fx.abs() < 1e-5 * dx.abs().max(1.0) {
            roots[count] = root;
            count += 1;
        }
    }
    (roots, count)
}

pub(crate) fn polynomial_roots(coefficients: [f64; 5]) -> ([f64; 4], usize) {
    let mut roots = [0.0; 4];
    let Some(degree) = coefficients.iter().rposition(|&v| v != 0.0) else {
        return (roots, 0);
    };
    let shift = coefficients.iter().position(|&v| v != 0.0).unwrap_or(0);
    let input = &coefficients[shift..=degree];
    let count = match input {
        [_] => 0,
        &[c, b] => {
            roots[0] = -c / b;
            1
        }
        &[c, b, a] => {
            let (values, count) = quadratic([c, b, a], STANDARD_PRECISION);
            roots[..3].copy_from_slice(&values);
            count
        }
        &[c, b, a, d] => {
            let (values, count) = cubic([c, b, a, d], STANDARD_PRECISION);
            roots[..3].copy_from_slice(&values);
            count
        }
        _ => return quartic(coefficients),
    };
    roots[count] = 0.0;
    (roots, count + usize::from(shift != 0))
}

/// Real roots of the polynomial obtained by substituting a graph into
/// another equation. Above degree four, use the same Laguerre iteration and
/// deflation as the quartic solver, then polish against the original input.
pub(in crate::algo) fn graph_roots(
    coefficients: &[f64],
    charge: &impl Fn(u64) -> bool,
) -> Vec<f64> {
    if !charge(coefficients.len() as u64) || coefficients.iter().any(|a| !a.is_finite()) {
        return Vec::new();
    }
    let Some(degree) = coefficients.iter().rposition(|&a| a != 0.0) else {
        return Vec::new();
    };
    let coefficients = &coefficients[..=degree];
    let mut roots = if degree <= 4 {
        // Fixed-size solver: charge for the small polynomial path as a unit.
        if !charge((degree + 1) as u64) {
            return Vec::new();
        }
        let mut input = [0.0; 5];
        input[..=degree].copy_from_slice(coefficients);
        let (roots, count) = polynomial_roots(input);
        roots[..count].to_vec()
    } else {
        let shift = coefficients.iter().position(|&a| a != 0.0).unwrap_or(0);
        let mut working: Vec<_> = coefficients[shift..]
            .iter()
            .map(|&a| Complex(a, 0.0))
            .collect();
        let mut remaining = 1000;
        let mut results = Vec::with_capacity(degree);
        if shift > 0 {
            results.push(0.0);
        }
        for degree in (1..working.len()).rev() {
            if !charge(degree as u64) {
                return Vec::new();
            }
            let Some(root) = laguerre(&working[..=degree], &mut remaining, -1.0, charge) else {
                break;
            };
            results.push(root.0);
            let mut carry = working[degree];
            for a in working[..degree].iter_mut().rev() {
                let next = a.add(carry.mul(root));
                *a = carry;
                carry = next;
            }
        }
        results.sort_by(f64::total_cmp);
        let derivative: Vec<_> = coefficients
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, a)| i as f64 * a)
            .collect();
        let value = |x| evaluate_polynomial(coefficients, x).abs();
        for x in &mut results {
            if !charge((4 * coefficients.len()) as u64) {
                return Vec::new();
            }
            if let Some(refined) = newton(coefficients, &derivative, *x, charge) {
                if value(refined) < value(*x) {
                    *x = refined;
                }
            }
        }
        results.retain(|&x| value(x) < 1e-5 * evaluate_polynomial(&derivative, x).abs().max(1.0));
        results
    };
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() < STANDARD_PRECISION);
    roots
}

/// Complex Laguerre roots without the real-axis residual filter.
pub(in crate::algo) fn complex_roots(
    coefficients: &[f64],
    charge: &impl Fn(u64) -> bool,
) -> Vec<(f64, f64)> {
    if !charge(coefficients.len() as u64) || coefficients.iter().any(|a| !a.is_finite()) {
        return Vec::new();
    }
    let Some(first) = coefficients.iter().position(|&a| a != 0.0) else {
        return Vec::new();
    };
    let last = coefficients
        .iter()
        .rposition(|&a| a != 0.0)
        .unwrap_or(first);
    let mut roots = if first == 0 {
        Vec::new()
    } else {
        vec![(0.0, 0.0)]
    };
    let reduced = &coefficients[first..=last];
    match *reduced {
        [_, _] => roots.push((-reduced[0] / reduced[1], 0.0)),
        [c, b, a] => roots.extend(complex_quadratic(c, b, a)),
        [_] => {}
        _ => {
            if let Some(mut candidates) = [-1.0, LAGUERRE_ALTERNATE_SEED]
                .into_iter()
                .find_map(|start| complex_with_seed(reduced, start, charge))
            {
                candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut offset = 0;
                while let Some(&(x, _)) = candidates.get(offset) {
                    let len =
                        candidates[offset..].partition_point(|p| p.0 - x < STANDARD_PRECISION);
                    if len == 0 || !charge(len as u64) {
                        return Vec::new();
                    }
                    candidates[offset..offset + len].sort_by(|a, b| a.1.total_cmp(&b.1));
                    offset += len;
                }
                roots.extend(candidates);
            }
        }
    }
    roots
}

fn complex_quadratic(c: f64, b: f64, a: f64) -> Vec<(f64, f64)> {
    if a.abs() < STANDARD_PRECISION {
        return (b.abs() >= STANDARD_PRECISION)
            .then_some((-c / b, 0.0))
            .into_iter()
            .collect();
    }
    let d = b * b - 4.0 * a * c;
    let real = -b / (2.0 * a);
    if d.abs() < STANDARD_PRECISION {
        vec![(real, 0.0)]
    } else if d < 0.0 {
        let imag = (-d).sqrt() / (2.0 * a);
        vec![(real, imag), (real, -imag)]
    } else {
        let q = -0.5 * (b + if b < 0.0 { -d.sqrt() } else { d.sqrt() });
        vec![(q / a, 0.0), (c / q, 0.0)]
    }
}

fn complex_with_seed(
    coefficients: &[f64],
    start: f64,
    charge: &impl Fn(u64) -> bool,
) -> Option<Vec<(f64, f64)>> {
    let mut work: Vec<_> = coefficients.iter().map(|&a| Complex(a, 0.0)).collect();
    let mut roots = Vec::with_capacity(work.len());
    let mut remaining = 1000;
    for degree in (1..work.len()).rev() {
        if !charge(degree as u64) {
            return None;
        }
        let root = if degree == 1 {
            work[0].scale(-1.0).div(work[1])
        } else {
            laguerre(&work[..=degree], &mut remaining, start, charge)?
        };
        roots.push((root.0, root.1));
        let mut carry = work[degree];
        for a in work[..degree].iter_mut().rev() {
            let next = a.add(carry.mul(root));
            *a = carry;
            carry = next;
        }
    }
    Some(roots)
}
