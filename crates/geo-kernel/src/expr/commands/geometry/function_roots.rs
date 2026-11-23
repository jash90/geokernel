// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoFunctionInvert.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/FunctionNVar.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/Polynomial.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::eval::{Ctx, Env};
use crate::expr::{self, math, Bin, IntersectionWork, Node, Un, MAX_EVALUATION_WORK};
use crate::kernel::{Construction, Def, Id, Value};

type Polynomial = [f64; 5];

impl Construction {
    /// Coefficients in ascending degree for a polynomial function of degree
    /// at most four. Coefficients can read other construction objects.
    pub(crate) fn function_polynomial(&self, id: Id) -> Option<Polynomial> {
        let ctx = function_context::<false>(self, id, &Env::None, None)?;
        let p = polynomial(&ctx, ctx.e.root)?;
        p.iter().all(|x| x.is_finite()).then_some(p)
    }

    /// The derivative at a seed for Newton iteration, when the expression
    /// tree consists of operations with scalar differentiation rules.
    pub(crate) fn function_derivative(&self, id: Id, x: f64) -> Option<f64> {
        IntersectionWork::new().derivative(self, id, x)
    }

    /// `GeoFunction.isEqualBooleanFunction`: test both functions at the
    /// inequality boundaries and inside each interval they delimit.
    pub(crate) fn boolean_functions_equal(&self, a: Id, b: Id) -> bool {
        let Some(a_ctx) = function_context::<false>(self, a, &Env::None, None) else {
            return false;
        };
        let Some(b_ctx) = function_context::<false>(self, b, &Env::None, None) else {
            return false;
        };
        let mut work = MAX_EVALUATION_WORK;
        let mut zeros = Vec::new();
        for ctx in [&a_ctx, &b_ctx] {
            let cost = ctx.e.cost.saturating_mul(u64::from(ctx.e.depth));
            let Some(left) = work.checked_sub(cost) else {
                return false;
            };
            work = left;
            if inequality_zeros(ctx, ctx.e.root, &mut zeros).is_none() {
                return false;
            }
        }
        if zeros.is_empty() {
            zeros.push(0.0);
        }
        zeros.sort_by(f64::total_cmp);
        zeros.dedup();
        let samples = (2 * zeros.len() + 1) as u64;
        if (a_ctx.e.cost + b_ctx.e.cost).saturating_mul(samples) > work {
            return false;
        }
        let agrees = |x| self.function_value(a, x) == self.function_value(b, x);
        let mut last = zeros[0] - 1.0;
        for zero in zeros {
            if !agrees(zero) || !agrees((last + zero) / 2.0) {
                return false;
            }
            last = zero;
        }
        agrees(last + 1.0)
    }
}

fn inequality_zeros(ctx: &Ctx<'_, false>, i: u32, zeros: &mut Vec<f64>) -> Option<()> {
    match ctx.node(i) {
        Node::Bin(Bin::Less | Bin::LessEqual | Bin::Greater | Bin::GreaterEqual, a, b) => {
            let a = polynomial(ctx, a)?;
            let b = polynomial(ctx, b)?;
            let coefficients = std::array::from_fn(|i| a[i] - b[i]);
            if coefficients.iter().any(|x| !x.is_finite()) {
                return None;
            }
            let (roots, count) = crate::algo::intersect::polynomial_roots(coefficients);
            zeros.extend(roots[..count].iter().copied().filter(|x| x.is_finite()));
            Some(())
        }
        Node::Bin(
            Bin::Equal | Bin::NotEqual | Bin::And | Bin::Or | Bin::Xor | Bin::Implies,
            a,
            b,
        ) => {
            inequality_zeros(ctx, a, zeros)?;
            inequality_zeros(ctx, b, zeros)
        }
        Node::Un(Un::Not, a) => inequality_zeros(ctx, a, zeros),
        _ => None,
    }
}

fn function_context<'a, const M: bool>(
    c: &'a Construction,
    id: Id,
    env: &'a Env<'a, M>,
    meter: Option<&'a expr::eval::Lists>,
) -> Option<Ctx<'a, M>> {
    let (Def::Expr(def), Value::Function) = (c.def(id), c.value(id)) else {
        return None;
    };
    Some(Ctx {
        meter,
        ..Ctx::new(c, expr::entry(c, def), env)
    })
}

fn scalar_polynomial(x: f64) -> Polynomial {
    [x, 0.0, 0.0, 0.0, 0.0]
}

fn product(a: Polynomial, b: Polynomial) -> Option<Polynomial> {
    let mut out = [0.0; 5];
    for (i, x) in a.into_iter().enumerate().filter(|&(_, x)| x != 0.0) {
        for (j, y) in b.into_iter().enumerate().filter(|&(_, y)| y != 0.0) {
            *out.get_mut(i + j)? += x * y;
        }
    }
    Some(out)
}

fn has_variable<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> Option<bool> {
    if M && !ctx.meter?.charge(1) {
        return None;
    }
    let node = ctx.node(i);
    if matches!(node, Node::Var) {
        return Some(true);
    }
    for child in expr::children(&node) {
        if has_variable(ctx, child)? {
            return Some(true);
        }
    }
    Some(false)
}

fn polynomial(ctx: &Ctx<'_, false>, i: u32) -> Option<Polynomial> {
    if !has_variable(ctx, i)? {
        return Some(scalar_polynomial(ctx.full_number(i)));
    }
    match ctx.node(i) {
        Node::Var => Some([0.0, 1.0, 0.0, 0.0, 0.0]),
        Node::Bin(op, a, b) => {
            let a = polynomial(ctx, a)?;
            let b = polynomial(ctx, b)?;
            let b_is_scalar = b[1..].iter().all(|&x| x == 0.0);
            match op {
                Bin::Add => Some(std::array::from_fn(|i| a[i] + b[i])),
                Bin::Sub => Some(std::array::from_fn(|i| a[i] - b[i])),
                Bin::Mul => product(a, b),
                Bin::Div if b_is_scalar => Some(a.map(|x| x / b[0])),
                Bin::Pow if b_is_scalar && (0.0..=4.0).contains(&b[0]) && b[0].fract() == 0.0 => (0
                    ..b[0] as usize)
                    .try_fold(scalar_polynomial(1.0), |power, _| product(power, a)),
                _ => None,
            }
        }
        Node::Apply(f, arg) => {
            let a = polynomial(ctx, arg)?;
            let p = ctx.c.function_polynomial(f)?;
            let degree = p.iter().rposition(|&x| x != 0.0).unwrap_or(0);
            let mut out = scalar_polynomial(p[degree]);
            for &constant in p[..degree].iter().rev() {
                out = product(out, a)?;
                out[0] += constant;
            }
            Some(out)
        }
        _ => None,
    }
}

fn derivative(ctx: &Ctx<'_, true>, i: u32, depth: u32) -> Option<f64> {
    let depth = depth.checked_sub(1)?;
    if let Some(rational) = ctx.e.cas_quotient(i) {
        return rational.slope(ctx.variable(), |n| !ctx.stop(n));
    }
    if !has_variable(ctx, i)? {
        return Some(0.0);
    }
    Some(match ctx.node(i) {
        Node::Var => 1.0,
        Node::Apply(f, arg) => {
            let env = Env::Value(ctx.full_number(arg));
            let applied = function_context(ctx.c, f, &env, ctx.meter)?;
            derivative(ctx, arg, depth)? * derivative(&applied, applied.e.root, depth)?
        }
        Node::Bin(op, a, b) => {
            let (da, db) = (derivative(ctx, a, depth)?, derivative(ctx, b, depth)?);
            match op {
                Bin::Add => da + db,
                Bin::Sub => da - db,
                _ => {
                    let (a, b) = (ctx.full_number(a), ctx.full_number(b));
                    match op {
                        Bin::Mul => da * b + a * db,
                        Bin::Div => (da * b - a * db) / (b * b),
                        Bin::Pow if db == 0.0 => b * a.powf(b - 1.0) * da,
                        Bin::Pow => a.powf(b) * (db * a.ln() + b * da / a),
                        _ => return None,
                    }
                }
            }
        }
        Node::Un(op, arg) => {
            let x = ctx.full_number(arg);
            let factor = match op {
                Un::Sin => math::cos(x),
                Un::Cos => -math::sin(x),
                Un::Tan => 1.0 / math::cos(x).powi(2),
                Un::Sinh => x.cosh(),
                Un::Cosh => x.sinh(),
                Un::Tanh => 1.0 / x.cosh().powi(2),
                Un::Asin => 1.0 / (1.0 - x * x).sqrt(),
                Un::Acos => -1.0 / (1.0 - x * x).sqrt(),
                Un::Atan => 1.0 / (1.0 + x * x),
                Un::Sqrt => 0.5 / x.sqrt(),
                Un::Cbrt => 1.0 / (3.0 * x.cbrt().powi(2)),
                Un::Abs => x / x.abs(),
                Un::Exp => x.exp(),
                Un::Ln => x.recip(),
                Un::Log2 => 1.0 / (x * std::f64::consts::LN_2),
                Un::Log10 => 1.0 / (x * std::f64::consts::LN_10),
                _ => return None,
            };
            factor * derivative(ctx, arg, depth)?
        }
        _ => return None,
    })
}

impl IntersectionWork {
    pub(crate) fn curve_polynomial(&self, c: &Construction, id: Id) -> Option<Vec<f64>> {
        let ctx = self.context(c, id, &Env::None)?;
        self.polynomial_at(&ctx, ctx.e.root)
    }

    fn context<'a>(
        &'a self,
        c: &'a Construction,
        id: Id,
        env: &'a Env<'a, true>,
    ) -> Option<Ctx<'a, true>> {
        function_context(c, id, env, Some(&self.0))
    }

    pub(crate) fn sample(&self, c: &Construction, id: Id, x: f64) -> f64 {
        if !self.charge(1) {
            return f64::NAN;
        }
        // A chart or freehand function has no tree; it answers by sampling.
        if crate::expr::is_sampled_function(c, id) {
            return if self.charge(32) {
                c.function_value(id, x).unwrap_or(f64::NAN)
            } else {
                f64::NAN
            };
        }
        let env = Env::Value(x);
        self.context(c, id, &env)
            .map_or(f64::NAN, |ctx| ctx.fast(ctx.e.root))
    }

    pub(crate) fn derivative(&self, c: &Construction, id: Id, x: f64) -> Option<f64> {
        let env = Env::Value(x);
        let ctx = self.context(c, id, &env)?;
        derivative(&ctx, ctx.e.root, expr::parse::MAX_DEPTH).filter(|d| d.is_finite())
    }

    /// Coefficients of the numerator and denominator of a rational graph.
    /// The caller checks roots against the graph to exclude holes.
    pub(crate) fn fraction(&self, c: &Construction, id: Id) -> Option<(Vec<f64>, Vec<f64>)> {
        let ctx = self.context(c, id, &Env::None)?;
        let result = fraction(
            &ctx,
            ctx.e.root,
            Expansion::graph(self),
            expr::parse::MAX_DEPTH,
        )?;
        (!self.exhausted()).then(|| result.into_parts())
    }

    pub(in crate::expr::commands) fn polynomial_at(
        &self,
        ctx: &Ctx<'_, true>,
        node: u32,
    ) -> Option<Vec<f64>> {
        let f = fraction(ctx, node, Expansion::graph(self), expr::parse::MAX_DEPTH)?;
        let &[scale] = f.denominator.as_slice() else {
            return None;
        };
        if scale == 0.0 || !scale.is_finite() || self.exhausted() {
            return None;
        }
        let mut coefficients = f.numerator;
        (self.charge(coefficients.len() as u64)
            && coefficients.iter_mut().all(|coefficient| {
                *coefficient /= scale;
                coefficient.is_finite()
            }))
        .then_some(coefficients)
    }

    /// Extract polynomial factors from the branch of a restricted function.
    /// Keeping powers factored avoids ill-conditioned expanded coefficients at
    /// repeated roots. Higher derivatives of ordinary polynomials use coefficients.
    pub(crate) fn analysis_polynomials(
        &self,
        c: &Construction,
        id: Id,
        order: usize,
    ) -> Option<Vec<Vec<f64>>> {
        let ctx = self.context(c, id, &Env::None)?;
        let root = match ctx.node(ctx.e.root) {
            Node::If(_, _, branch, expr::NONE) => branch,
            _ => {
                if order == 1 {
                    let expanded = fraction(
                        &ctx,
                        ctx.e.root,
                        Expansion::graph(self),
                        expr::parse::MAX_DEPTH,
                    )?;
                    if expanded.denominator.len() != 1 || expanded.numerator.len() > 26 {
                        return None;
                    }
                }
                ctx.e.root
            }
        };
        analysis_polynomials(&ctx, root, order, self, expr::parse::MAX_DEPTH)
    }

    pub(crate) fn analysis_denominators(&self, c: &Construction, id: Id) -> Vec<Vec<f64>> {
        let Some(ctx) = self.context(c, id, &Env::None) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        let mut pending = vec![ctx.e.root];
        while let Some(i) = pending.pop() {
            if !self.charge(1) {
                return Vec::new();
            }
            let node = ctx.node(i);
            if let Node::Bin(Bin::Div, _, b) = node {
                if let Some(poly) = self.polynomial_at(&ctx, b).filter(|p| p.len() > 1) {
                    results.push(poly);
                }
            }
            pending.extend(expr::children(&node));
        }
        results
    }

    pub(crate) fn analysis_transcendental(
        &self,
        c: &Construction,
        id: Id,
    ) -> Option<(u8, [f64; 2])> {
        let ctx = self.context(c, id, &Env::None)?;
        let Node::Un(op, a) = ctx.node(ctx.e.root) else {
            return None;
        };
        let operation = match op {
            Un::Exp => 0,
            Un::Atan => 1,
            Un::Ln | Un::Log10 | Un::Log2 => 2,
            _ => return None,
        };
        let poly: [f64; 2] = self.polynomial_at(&ctx, a)?.try_into().ok()?;
        (poly[1] != 0.0).then_some((operation, poly))
    }

    pub(crate) fn analysis_is_restricted(&self, c: &Construction, id: Id) -> bool {
        self.context(c, id, &Env::None)
            .is_some_and(|ctx| matches!(ctx.node(ctx.e.root), Node::If(_, _, _, expr::NONE)))
    }

    pub(crate) fn constant(&self, c: &Construction, id: Id) -> Option<f64> {
        let ctx = self.context(c, id, &Env::None)?;
        let root = ctx.e.root;
        (!has_variable(&ctx, root)?)
            .then(|| ctx.full_number(root))
            .filter(|value| value.is_finite())
    }

    pub(crate) fn inverse(&self, c: &Construction, id: Id, value: f64) -> Option<f64> {
        let ctx = self.context(c, id, &Env::None)?;
        inverse(&ctx, ctx.e.root, value, expr::parse::MAX_DEPTH)
    }
}

fn analysis_polynomials(
    ctx: &Ctx<'_, true>,
    root: u32,
    order: usize,
    work: &IntersectionWork,
    depth: u32,
) -> Option<Vec<Vec<f64>>> {
    use crate::expr::NONE;
    let depth = depth.checked_sub(1)?;
    if !work.charge(1) {
        return None;
    }
    match ctx.node(root) {
        Node::Bin(Bin::Mul, a, b) if order == 0 => {
            let mut first = analysis_polynomials(ctx, a, order, work, depth)?;
            first.extend(analysis_polynomials(ctx, b, order, work, depth)?);
            return Some(first);
        }
        Node::Bin(Bin::Pow, base, exponent) if order < 2 && !has_variable(ctx, exponent)? => {
            let exponent = ctx.full_number(exponent);
            if exponent > 0.0 && exponent.fract() == 0.0 {
                let mut factors = analysis_polynomials(ctx, base, order, work, depth)?;
                if order == 1 && exponent % 2.0 == 0.0 {
                    factors.extend(analysis_polynomials(ctx, base, 0, work, depth)?);
                }
                return Some(factors);
            }
        }
        Node::Un(Un::Sqrt | Un::Cbrt | Un::Abs, a) if order == 0 => {
            return analysis_polynomials(ctx, a, order, work, depth);
        }
        Node::If(_, _, branch, NONE) => {
            return analysis_polynomials(ctx, branch, order, work, depth);
        }
        _ => {}
    }
    let f = fraction(ctx, root, Expansion::analysis(work), depth)?;
    if f.denominator.len() != 1 && order > 0 {
        return None;
    }
    let mut poly = f.numerator;
    for _ in 0..order {
        if !work.charge(poly.len() as u64) {
            return None;
        }
        poly = poly
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, a)| i as f64 * a)
            .collect();
    }
    Some(vec![poly])
}

/// Isolate the variable through the operations used by the Newton fallback.
fn inverse(ctx: &Ctx<'_, true>, i: u32, value: f64, depth: u32) -> Option<f64> {
    let depth = depth.checked_sub(1)?;
    if !value.is_finite() || !ctx.meter?.charge(1) {
        return None;
    }
    let (arg, value) = match ctx.node(i) {
        Node::Var => return Some(value),
        Node::Un(Un::Exp, a) => (a, value.ln()),
        Node::Un(Un::Ln, a) => (a, value.exp()),
        Node::Apply(f, a) => {
            let applied = function_context(ctx.c, f, &Env::None, ctx.meter)?;
            (a, inverse(&applied, applied.e.root, value, depth)?)
        }
        Node::Bin(op @ (Bin::Add | Bin::Sub), a, b) => {
            match (has_variable(ctx, a)?, has_variable(ctx, b)?) {
                (true, false) => {
                    let constant = ctx.full_number(b);
                    (
                        a,
                        if op == Bin::Add {
                            value - constant
                        } else {
                            value + constant
                        },
                    )
                }
                (false, true) => {
                    let constant = ctx.full_number(a);
                    (
                        b,
                        if op == Bin::Add {
                            value - constant
                        } else {
                            constant - value
                        },
                    )
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    inverse(ctx, arg, value, depth)
}

/// Ascending coefficients, with trailing zeros removed after each operation.
struct Fraction {
    numerator: Vec<f64>,
    denominator: Vec<f64>,
}

impl Fraction {
    fn polynomial(numerator: Vec<f64>) -> Self {
        Self {
            numerator,
            denominator: vec![1.0],
        }
    }

    fn into_parts(self) -> (Vec<f64>, Vec<f64>) {
        (self.numerator, self.denominator)
    }

    fn add(self, rhs: Self, sign: f64, expansion: Expansion<'_>) -> Option<Self> {
        let mut numerator = multiply(&self.numerator, &rhs.denominator, expansion)?;
        let other = multiply(&rhs.numerator, &self.denominator, expansion)?;
        if !expansion
            .work
            .charge(numerator.len().max(other.len()) as u64)
        {
            return None;
        }
        numerator.resize(numerator.len().max(other.len()), 0.0);
        for (a, b) in numerator.iter_mut().zip(other) {
            *a += sign * b;
        }
        trim(&mut numerator);
        Some(Self {
            numerator,
            denominator: multiply(&self.denominator, &rhs.denominator, expansion)?,
        })
    }

    fn mul(self, rhs: &Self, expansion: Expansion<'_>) -> Option<Self> {
        Some(Self {
            numerator: multiply(&self.numerator, &rhs.numerator, expansion)?,
            denominator: multiply(&self.denominator, &rhs.denominator, expansion)?,
        })
    }

    fn compose(coefficients: &[f64], arg: &Self, expansion: Expansion<'_>) -> Option<Self> {
        coefficients
            .iter()
            .rev()
            .try_fold(Self::polynomial(vec![0.0]), |result, &coefficient| {
                result
                    .mul(arg, expansion)?
                    .add(Self::polynomial(vec![coefficient]), 1.0, expansion)
            })
    }

    fn divide(self, rhs: Self, expansion: Expansion<'_>) -> Option<Self> {
        self.mul(
            &Self {
                numerator: rhs.denominator,
                denominator: rhs.numerator,
            },
            expansion,
        )
    }
}

const MAX_GRAPH_DEGREE: usize = 300;

#[derive(Clone, Copy)]
struct Expansion<'a> {
    work: &'a IntersectionWork,
    max_degree: usize,
}

impl<'a> Expansion<'a> {
    fn graph(work: &'a IntersectionWork) -> Self {
        Self {
            work,
            max_degree: MAX_GRAPH_DEGREE,
        }
    }

    fn analysis(work: &'a IntersectionWork) -> Self {
        Self {
            work,
            max_degree: usize::MAX - 1,
        }
    }
}

fn trim(coefficients: &mut Vec<f64>) {
    while coefficients.len() > 1 && coefficients.last() == Some(&0.0) {
        coefficients.pop();
    }
}

fn multiply(a: &[f64], b: &[f64], expansion: Expansion<'_>) -> Option<Vec<f64>> {
    let degree = (a.len() - 1).checked_add(b.len() - 1)?;
    let terms = (a.len() as u64).checked_mul(b.len() as u64)?;
    if degree > expansion.max_degree || !expansion.work.charge(terms) {
        return None;
    }
    let mut product = vec![0.0; degree + 1];
    for (i, a) in a.iter().enumerate().filter(|(_, a)| **a != 0.0) {
        for (j, b) in b.iter().enumerate() {
            product[i + j] += a * b;
        }
    }
    trim(&mut product);
    Some(product)
}

fn fraction(ctx: &Ctx<'_, true>, i: u32, expansion: Expansion<'_>, depth: u32) -> Option<Fraction> {
    let depth = depth.checked_sub(1)?;
    if let Some(rational) = ctx.e.cas_quotient(i) {
        let coefficients = |poly: &crate::cas::Poly| {
            let coeffs = poly.coefficients()?;
            if !expansion.work.charge(coeffs.len() as u64) {
                return None;
            }
            let mut coeffs: Vec<_> = coeffs.into_iter().map(crate::cas::Rat::to_f64).collect();
            if coeffs.is_empty() {
                coeffs.push(0.0);
            }
            Some(coeffs)
        };
        return Some(Fraction {
            numerator: coefficients(&rational.numerator)?,
            denominator: coefficients(&rational.denominator)?,
        });
    }
    if !has_variable(ctx, i)? {
        return Some(Fraction::polynomial(vec![ctx.full_number(i)]));
    }
    match ctx.node(i) {
        Node::Var => Some(Fraction::polynomial(vec![0.0, 1.0])),
        Node::Apply(f, arg) => {
            let arg = fraction(ctx, arg, expansion, depth)?;
            let applied = expansion.work.context(ctx.c, f, &Env::None)?;
            let callee = if let Some(coefficients) = applied.e.polynomial() {
                if !expansion.work.charge(coefficients.len() as u64) {
                    return None;
                }
                Fraction::polynomial(if coefficients.is_empty() {
                    vec![0.0]
                } else {
                    coefficients.to_vec()
                })
            } else {
                fraction(&applied, applied.e.root, expansion, depth)?
            };
            Fraction::compose(&callee.numerator, &arg, expansion)?.divide(
                Fraction::compose(&callee.denominator, &arg, expansion)?,
                expansion,
            )
        }
        Node::Bin(Bin::Pow, a, b) if !has_variable(ctx, b)? => {
            let exponent = ctx.full_number(b);
            // The reference polynomial parser also bounds a single expanded power.
            if exponent.fract() != 0.0 || exponent.abs() > MAX_GRAPH_DEGREE as f64 {
                return None;
            }
            let mut base = fraction(ctx, a, expansion, depth)?;
            if exponent < 0.0 {
                std::mem::swap(&mut base.numerator, &mut base.denominator);
            }
            (0..exponent.abs() as usize).try_fold(Fraction::polynomial(vec![1.0]), |power, _| {
                power.mul(&base, expansion)
            })
        }
        Node::Bin(op @ (Bin::Add | Bin::Sub | Bin::Mul | Bin::Div), a, b) => {
            let a = fraction(ctx, a, expansion, depth)?;
            let b = fraction(ctx, b, expansion, depth)?;
            match op {
                Bin::Add => a.add(b, 1.0, expansion),
                Bin::Sub => a.add(b, -1.0, expansion),
                Bin::Div => a.divide(b, expansion),
                _ => a.mul(&b, expansion),
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod fraction_tests {
    use super::*;
    use crate::expr::eval::with_work_limit;

    #[test]
    fn repeated_function_expansion_respects_the_work_limit() {
        let mut c = crate::script::load("f1(x) = x").unwrap();
        for i in 2..=12 {
            let j = i - 1;
            crate::script::exec(&mut c, &format!("f{i}(x) = f{j}(x) + f{j}(x)")).unwrap();
        }
        let f = c.id("f9").unwrap();
        with_work_limit(32, || {
            assert!(IntersectionWork::new().fraction(&c, f).is_none())
        });
        assert_eq!(
            IntersectionWork::new().fraction(&c, f).unwrap().0,
            [0.0, 256.0]
        );
        with_work_limit(1_000, || {
            let before = expr::work_done();
            let id = crate::script::exec(&mut c, "R = Intersect(f12, xAxis)")
                .unwrap()
                .unwrap();
            assert_eq!(c.value(id), Value::Undefined);
            let spent = expr::work_done() - before;
            assert!((1_000..=2_010).contains(&spent), "{spent}");
        });
    }
}
