use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Ctx, Node, Parser, Ty, Val};
use crate::expr::NONE;

pub(super) const NAMES: &[&str] = &[
    "Normal",
    "InverseNormal",
    "Exponential",
    "InverseExponential",
];
const QUANTILE_STEPS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Probability {
    Normal { function: bool },
    InverseNormal,
    Exponential { function: bool },
    InverseExponential,
}

impl Probability {
    pub(in crate::expr) fn is_function(self) -> bool {
        matches!(
            self,
            Self::Normal { function: true } | Self::Exponential { function: true }
        )
    }

    pub(in crate::expr) fn cost(self) -> u64 {
        match self {
            Self::Normal { .. } | Self::Exponential { .. } | Self::InverseExponential => 1,
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
        ("Exponential", [_, x] | [_, x, _]) => Probability::Exponential {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseExponential", [_, _]) => Probability::InverseExponential,
        _ => return Err(format!("wrong number of arguments to `{name}`")),
    };
    let mut slots = [NONE; 4];
    // Both distribution families share the parameter, sample and flag slots.
    let offset = usize::from(matches!(
        op,
        Probability::Exponential { .. } | Probability::InverseExponential
    ));
    slots[offset..offset + args.len()].copy_from_slice(&args);
    Ok(Some(p.push(Node::Probability(op, slots))))
}

pub(in crate::expr) fn value_type(
    op: Probability,
    [mean, scale, x, flag]: [Option<Ty>; 4],
) -> Result<Ty, String> {
    let has_mean = matches!(op, Probability::Normal { .. } | Probability::InverseNormal);
    let valid = (!has_mean || matches!(mean, Some(Ty::Num(_))))
        && matches!((scale, x), (Some(Ty::Num(_)), Some(Ty::Num(_))))
        && match flag {
            None => true,
            Some(Ty::Bool) => matches!(
                op,
                Probability::Normal { .. } | Probability::Exponential { .. }
            ),
            Some(Ty::Num(_)) => matches!(op, Probability::Normal { function: false }),
            _ => false,
        };
    if valid {
        Ok(Ty::Num(0))
    } else {
        Err("a distribution needs numeric parameters and an optional cumulative flag".into())
    }
}

pub(in crate::expr) fn value<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: Probability,
    [mean, deviation, x, flag]: [u32; 4],
) -> f64 {
    let deviation = ctx.full_number(deviation);
    let x = ctx.full_number(x);
    let fourth = if flag != NONE {
        ctx.full(flag).val
    } else {
        Val::B(true, true)
    };
    match op {
        Probability::Exponential { function } => {
            return exponential(deviation, x, matches!(fourth, Val::B(true, _)), function);
        }
        Probability::InverseExponential => return inverse_exponential(deviation, x),
        _ => {}
    }
    let mean = ctx.full_number(mean);
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

fn exponential(rate: f64, x: f64, cumulative: bool, function: bool) -> f64 {
    if function {
        if x < 0.0 {
            return 0.0;
        }
        let survival = libm::exp(-(x * rate));
        return if cumulative {
            1.0 - survival
        } else {
            survival * rate
        };
    }
    // The scalar command passes reciprocal rate as a distribution scale.
    let scale = 1.0 / rate;
    if scale <= 0.0 || scale.is_nan() || x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        return 0.0;
    }
    let survival = libm::exp(-x / scale);
    if cumulative {
        1.0 - survival
    } else {
        survival / scale
    }
}

fn inverse_exponential(rate: f64, probability: f64) -> f64 {
    let scale = 1.0 / rate;
    if scale <= 0.0 || scale.is_nan() || !(0.0..=1.0).contains(&probability) {
        return f64::NAN;
    }
    if probability == 1.0 {
        return f64::INFINITY;
    }
    // The reference takes the logarithm after rounding the complement.
    -scale * libm::log(1.0 - probability)
}

fn cdf(mean: f64, deviation: f64, x: f64) -> f64 {
    libm::erfc(-(x - mean) / (deviation * SQRT_2)) * 0.5
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

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_distribution_function_slot_accepts_a_free_variable() {
        for source in [
            "f = Normal(0,1,x+1)",
            "f = Normal(x,1,x)",
            "f = Normal(0,1,1, x > 0)",
            "f = InverseNormal(0,1,x)",
            "f = Exponential(2,x+1)",
            "f = Exponential(x,x)",
            "f = Exponential(2,1,x>0)",
            "f = Exponential(2,1,3)",
            "f = InverseExponential(2,x)",
        ] {
            assert!(crate::script::load(source).is_err(), "{source}");
        }
    }
}
