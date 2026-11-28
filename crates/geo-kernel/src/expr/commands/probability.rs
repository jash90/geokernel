use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Ctx, Node, Parser, Ty, Val};
use crate::expr::CallOp;

pub(super) const NAMES: &[&str] = &["Normal", "InverseNormal"];
const QUANTILE_STEPS: u64 = 60;

pub(super) fn parse(p: &mut Parser, name: &str) -> Result<Option<u32>, String> {
    if !NAMES.contains(&name) {
        return Ok(None);
    }
    let args = p.arguments()?;
    let op = match (name, args.as_slice()) {
        ("Normal", [_, _, x] | [_, _, x, _]) => CallOp::Normal {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseNormal", [_, _, _]) => CallOp::InverseNormal,
        _ => return Err(format!("wrong number of arguments to `{name}`")),
    };
    let first = p.call_run(&args);
    Ok(Some(p.push(Node::Call(op, first, args.len() as u32))))
}

pub(super) fn call_type(op: CallOp, args: &[Ty]) -> Result<Ty, String> {
    let valid = match args {
        [Ty::Num(_), Ty::Num(_), Ty::Num(_)] => true,
        [Ty::Num(_), Ty::Num(_), Ty::Num(_), Ty::Bool] => matches!(op, CallOp::Normal { .. }),
        [Ty::Num(_), Ty::Num(_), Ty::Num(_), Ty::Num(_)] => {
            matches!(op, CallOp::Normal { function: false })
        }
        _ => false,
    };
    if valid {
        Ok(Ty::Num(0))
    } else {
        Err("a normal distribution needs numeric parameters and an optional cumulative flag".into())
    }
}

pub(super) fn call_value<const M: bool>(ctx: &Ctx<'_, M>, op: CallOp, first: u32, len: u32) -> f64 {
    let mean = ctx.fast(first);
    let deviation = ctx.fast(first + 1);
    let x = ctx.fast(first + 2);
    let fourth = if len == 4 {
        ctx.full(first + 3).val
    } else {
        Val::B(true, true)
    };
    if matches!(op, CallOp::Normal { function: true }) {
        let delta = x - mean;
        return if matches!(fourth, Val::B(true, _)) {
            (1.0 + libm::erf(delta / (SQRT_2 * deviation.abs()))) * 0.5
        } else {
            libm::exp(-delta * delta / (2.0 * deviation * deviation))
                / ((2.0 * PI).sqrt() * deviation.abs())
        };
    }
    if mean.is_nan() || x.is_nan() || deviation.is_nan() || deviation <= 0.0 {
        return f64::NAN;
    }
    if op == CallOp::InverseNormal {
        if !(0.0..=1.0).contains(&x) || !ctx.lists.is_some_and(|lists| lists.charge(QUANTILE_STEPS))
        {
            return f64::NAN;
        }
        return inverse(mean, deviation, x);
    }
    match fourth {
        Val::B(true, _) => cdf(mean, deviation, x),
        Val::B(false, _) => {
            let z = (x - mean) / deviation;
            libm::exp(-0.5 * z * z) / (deviation * (2.0 * PI).sqrt())
        }
        Val::N(upper) if upper >= x => cdf(mean, deviation, upper) - cdf(mean, deviation, x),
        _ => f64::NAN,
    }
}

fn cdf(mean: f64, deviation: f64, x: f64) -> f64 {
    libm::erfc(-(x - mean) / deviation * FRAC_1_SQRT_2) * 0.5
}

fn inverse(mean: f64, deviation: f64, probability: f64) -> f64 {
    // The reference evaluates the quantile of the centered probability.
    // Round it before reflection, including its saturation at either end.
    let centered = 2.0 * probability - 1.0;
    if centered == 0.0 {
        return mean + deviation * 0.0;
    }
    let tail = (1.0 - centered.abs()) * 0.5;
    if tail == 0.0 {
        return f64::INFINITY.copysign(centered);
    }
    let (mut lo, mut hi) = (0.0, 40.0);
    for _ in 0..QUANTILE_STEPS {
        let mid = (lo + hi) * 0.5;
        if cdf(0.0, 1.0, -mid) > tail {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    mean + deviation * ((lo + hi) * 0.5).copysign(centered)
}
