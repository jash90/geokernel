// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoFunctionInvert.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::eval::{function_entry, scalar, Ctx, Env, Val};
use super::{children, math, Bin, Entry, Node, Un, NONE};
use crate::algo::transform::zero_dilation;
use crate::kernel::Construction;

pub(super) fn has_inverse(c: &Construction, e: &Entry, source: u32) -> bool {
    sample(&Ctx::<false>::new(c, e, &Env::None), source, 1.0).is_some()
}

/// Peel off operations along the path to x. An unsupported operation gives
/// no inverse, while a sample outside the inverse's domain gives NaN.
pub(super) fn sample<const M: bool>(ctx: &Ctx<'_, M>, node: u32, y: f64) -> Option<f64> {
    if ctx.stop(1) {
        return None;
    }
    match ctx.node(node) {
        Node::Var => Some(y),
        Node::Un(op, a) => sample(ctx, a, undo_unary(op, y)?),
        Node::Bin(op, l, r) => match (uses_x(ctx, l), uses_x(ctx, r)) {
            (true, false) => sample(ctx, l, undo_right(ctx, op, r, y)?),
            (false, true) => sample(ctx, r, undo_left(op, ctx.full_number(l), y)?),
            _ => None,
        },
        Node::If(_, condition, branch, NONE) => {
            let x = sample(ctx, branch, y)?;
            let at_x = Ctx {
                env: &Env::Value(x),
                ..*ctx
            };
            Some(if matches!(at_x.full(condition).val, Val::B(true, _)) {
                x
            } else {
                f64::NAN
            })
        }
        Node::Apply(f, a) => {
            let e = function_entry(ctx.c, f)?;
            let inner = Ctx { e, ..*ctx };
            sample(ctx, a, sample(&inner, e.root, y)?)
        }
        Node::Dilate(src, factor, center) => {
            let (r, sx, sy) = ctx.dilation(factor, center);
            if zero_dilation(r) {
                return None;
            }
            let x = sample(ctx, src, (y - (-sy * r + sy)) / r)?;
            Some(x * r - (sx * r - sx))
        }
        Node::Inverse(a) => {
            sample(ctx, a, y)?;
            Some(
                Ctx {
                    env: &Env::Value(y),
                    ..*ctx
                }
                .full_number(a),
            )
        }
        // The reference uses its CAS when these rules do not give an inverse.
        _ => None,
    }
}

fn uses_x<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> bool {
    !ctx.stop(1)
        && (matches!(ctx.node(i), Node::Var | Node::Inverse(_))
            || children(&ctx.node(i)).any(|child| uses_x(ctx, child)))
}

fn undo_unary(op: Un, y: f64) -> Option<f64> {
    let (inverse, arg) = match op {
        Un::Sin => (Un::Asin, y),
        Un::Cos => (Un::Acos, y),
        Un::Tan => (Un::Atan, y),
        Un::Asin | Un::AsinD => (Un::Sin, y),
        Un::Acos => (Un::Cos, y),
        Un::Atan => (Un::Tan, y),
        Un::Sinh => (Un::Asinh, y),
        Un::Cosh => (Un::Acosh, y),
        Un::Tanh => (Un::Atanh, y),
        Un::Asinh => (Un::Sinh, y),
        Un::Acosh => (Un::Cosh, y),
        Un::Atanh => (Un::Tanh, y),
        Un::Exp => (Un::Ln, y),
        Un::Ln => (Un::Exp, y),
        Un::Csc => (Un::Asin, 1.0 / y),
        Un::Sec => (Un::Acos, 1.0 / y),
        Un::Cot => (Un::Atan, 1.0 / y),
        Un::Csch => (Un::Asinh, 1.0 / y),
        Un::Sech => (Un::Acosh, 1.0 / y),
        Un::Coth => (Un::Atanh, 1.0 / y),
        Un::Sqrt => return Some(y * y),
        Un::Cbrt => return Some(y * y * y),
        Un::Log2 => return Some(math::my_pow(2.0, y)),
        Un::Log10 => return Some(math::my_pow(10.0, y)),
        _ => return None,
    };
    Some(scalar(inverse, arg, false))
}

fn undo_left(op: Bin, a: f64, y: f64) -> Option<f64> {
    Some(match op {
        Bin::Add => y - a,
        Bin::Sub => a - y,
        Bin::Mul => y / a,
        Bin::Div if a != 0.0 => a / y,
        Bin::Pow => y.ln() / a.ln(),
        Bin::LogB => math::my_pow(a, y),
        _ => return None,
    })
}

fn undo_right<const M: bool>(ctx: &Ctx<'_, M>, op: Bin, r: u32, y: f64) -> Option<f64> {
    let b = ctx.full_number(r);
    Some(match op {
        Bin::Add => y - b,
        Bin::Sub => y + b,
        Bin::Mul => y / b,
        Bin::Div if b != 0.0 => b * y,
        Bin::Pow if b != 0.0 && b.is_finite() => inverse_power(ctx, r, b, y),
        Bin::LogB => math::my_pow(b, 1.0 / y),
        _ => return None,
    })
}

fn inverse_power<const M: bool>(ctx: &Ctx<'_, M>, exponent: u32, b: f64, y: f64) -> f64 {
    if math::is_equal(b, 3.0) {
        return math::cbrt(y);
    }
    if math::is_equal(b, 2.0) {
        return y.sqrt();
    }
    if math::is_equal(b, -1.0) {
        return y.recip();
    }
    let fraction = match ctx.node(exponent) {
        Node::Bin(Bin::Div, n, d) => Some((ctx.full_number(d), ctx.full_number(n))),
        _ => {
            let (n, d) = super::derivative::decimal_to_fraction(b, crate::tol::STANDARD_PRECISION);
            if n == 0.0 || d == 0.0 {
                return f64::NAN;
            }
            (n.abs() < 100.0 && d.abs() < 100.0).then_some((d, n))
        }
    };
    if let Some((n, d)) = fraction {
        if y < 0.0 {
            return math::neg_power_integers(y, n, d).unwrap_or_else(|| math::my_pow(y, n / d));
        }
        return math::my_pow(y, n / d);
    }
    math::my_pow(y, 1.0 / b)
}
