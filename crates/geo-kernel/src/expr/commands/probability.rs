use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Ctx, Node, Parser, Ty, Val};
use crate::expr::NONE;

pub(super) const NAMES: &[&str] = &["Normal", "InverseNormal"];
const QUANTILE_STEPS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Probability {
    Normal { function: bool },
    InverseNormal,
}

impl Probability {
    pub(in crate::expr) fn cost(self) -> u64 {
        match self {
            Self::Normal { .. } => 1,
            Self::InverseNormal => QUANTILE_STEPS + 1,
        }
    }
}

pub(super) fn parse(p: &mut Parser, name: &str) -> Result<Option<u32>, String> {
    if !NAMES.contains(&name) {
        return Ok(None);
    }
    let args = p.arguments()?;
    let op = match (name, args.as_slice()) {
        ("Normal", [_, _, x] | [_, _, x, _]) => Probability::Normal {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseNormal", [_, _, _]) => Probability::InverseNormal,
        _ => return Err(format!("wrong number of arguments to `{name}`")),
    };
    let mut slots = [NONE; 4];
    slots[..args.len()].copy_from_slice(&args);
    Ok(Some(p.push(Node::Probability(op, slots))))
}

pub(in crate::expr) fn value_type(
    op: Probability,
    args: [u32; 4],
    types: &[Ty],
) -> Result<Ty, String> {
    let [mean, scale, x, flag] = args.map(|arg| types.get(arg as usize).copied());
    let valid = matches!(
        (mean, scale, x),
        (Some(Ty::Num(_)), Some(Ty::Num(_)), Some(Ty::Num(_)))
    ) && match flag {
        None => true,
        Some(Ty::Bool) => matches!(op, Probability::Normal { .. }),
        Some(Ty::Num(_)) => matches!(op, Probability::Normal { function: false }),
        _ => false,
    };
    if valid {
        Ok(Ty::Num(0))
    } else {
        Err("a normal distribution needs numeric parameters and an optional cumulative flag".into())
    }
}

pub(in crate::expr) fn value<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: Probability,
    [mean, deviation, x, flag]: [u32; 4],
) -> f64 {
    let mean = ctx.fast(mean);
    let deviation = ctx.fast(deviation);
    let x = ctx.fast(x);
    let fourth = if flag != NONE {
        ctx.full(flag).val
    } else {
        Val::B(true, true)
    };
    if matches!(op, Probability::Normal { function: true }) {
        let delta = x - mean;
        return if matches!(fourth, Val::B(true, _)) {
            (1.0 + libm::erf(delta / (SQRT_2 * deviation.abs()))) * 0.5
        } else {
            libm::exp(-(delta * delta) / (2.0 * (deviation * deviation)))
                / ((2.0 * PI).sqrt() * deviation.abs())
        };
    }
    if mean.is_nan() || x.is_nan() || deviation.is_nan() || deviation <= 0.0 {
        return f64::NAN;
    }
    if op == Probability::InverseNormal {
        if !(0.0..=1.0).contains(&x) || ctx.stop(QUANTILE_STEPS) {
            return f64::NAN;
        }
        if ctx.meter.is_none() {
            crate::expr::charge(QUANTILE_STEPS);
        }
        return inverse(mean, deviation, x);
    }
    match fourth {
        Val::B(true, _) => cdf(mean, deviation, x),
        Val::B(false, _) => {
            let z = (x - mean) / deviation;
            let log_normalizer = libm::log(deviation) + 0.5 * libm::log(2.0 * PI);
            libm::exp(-0.5 * z * z - log_normalizer)
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
        return mean + deviation * f64::INFINITY.copysign(centered);
    }
    // Near the median, compare erf directly to avoid rounding away the
    // deviation from 0.5. Use the complementary tail farther out.
    let magnitude = centered.abs();
    let central = magnitude <= 0.5;
    let (mut lo, mut hi) = (0.0, if central { 2.0 * magnitude } else { 40.0 });
    for _ in 0..QUANTILE_STEPS {
        let mid = (lo + hi) * 0.5;
        let below = if central {
            libm::erf(mid * FRAC_1_SQRT_2) < magnitude
        } else {
            cdf(0.0, 1.0, -mid) > tail
        };
        if below {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    mean + deviation * ((lo + hi) * 0.5).copysign(centered)
}
