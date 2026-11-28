use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Ctx, Node, Parser, Ty, Val};
use crate::expr::NONE;

pub(super) const NAMES: &[&str] = &[
    "Normal",
    "InverseNormal",
    "Exponential",
    "InverseExponential",
    "Cauchy",
    "InverseCauchy",
    "Logistic",
    "InverseLogistic",
    "LogNormal",
    "InverseLogNormal",
];
const QUANTILE_STEPS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Probability {
    Normal { function: bool },
    InverseNormal,
    Exponential { function: bool },
    InverseExponential,
    Cauchy { function: bool },
    InverseCauchy,
    Logistic { function: bool },
    InverseLogistic,
    LogNormal { function: bool },
    InverseLogNormal,
}

impl Probability {
    pub(in crate::expr) fn is_function(self) -> bool {
        matches!(
            self,
            Self::Normal { function: true }
                | Self::Exponential { function: true }
                | Self::Cauchy { function: true }
                | Self::Logistic { function: true }
                | Self::LogNormal { function: true }
        )
    }

    pub(in crate::expr) fn cost(self) -> u64 {
        match self {
            Self::InverseNormal => QUANTILE_STEPS + 1,
            _ => 1,
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
        ("Cauchy", [_, _, x] | [_, _, x, _]) => Probability::Cauchy {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseCauchy", [_, _, _]) => Probability::InverseCauchy,
        ("Logistic", [_, _, x] | [_, _, x, _]) => Probability::Logistic {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseLogistic", [_, _, _]) => Probability::InverseLogistic,
        ("LogNormal", [_, _, x] | [_, _, x, _]) => Probability::LogNormal {
            function: matches!(p.node(*x), Node::Var),
        },
        ("InverseLogNormal", [_, _, _]) => Probability::InverseLogNormal,
        _ => return Err(format!("wrong number of arguments to `{name}`")),
    };
    let mut slots = [NONE; 4];
    // Distribution families share the parameter, sample and flag slots.
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
    let has_mean = !matches!(
        op,
        Probability::Exponential { .. } | Probability::InverseExponential
    );
    let valid = (!has_mean || matches!(mean, Some(Ty::Num(_))))
        && matches!((scale, x), (Some(Ty::Num(_)), Some(Ty::Num(_))))
        && match flag {
            None => true,
            Some(Ty::Bool) => matches!(
                op,
                Probability::Normal { .. }
                    | Probability::Exponential { .. }
                    | Probability::Cauchy { .. }
                    | Probability::Logistic { .. }
                    | Probability::LogNormal { .. }
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
    match op {
        Probability::Cauchy { function } => {
            return cauchy(
                mean,
                deviation,
                x,
                matches!(fourth, Val::B(true, _)),
                function,
            );
        }
        Probability::InverseCauchy => return inverse_cauchy(mean, deviation, x),
        Probability::Logistic { function } => {
            return logistic(
                mean,
                deviation,
                x,
                matches!(fourth, Val::B(true, _)),
                function,
            );
        }
        Probability::InverseLogistic => return inverse_logistic(mean, deviation, x),
        Probability::LogNormal { function } => {
            return log_normal(
                mean,
                deviation,
                x,
                matches!(fourth, Val::B(true, _)),
                function,
            );
        }
        Probability::InverseLogNormal => return inverse_log_normal(mean, deviation, x),
        _ => {}
    }
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

fn cauchy(location: f64, scale: f64, x: f64, cumulative: bool, function: bool) -> f64 {
    if !function && scale <= 0.0 {
        return f64::NAN;
    }
    let offset = x - location;
    if cumulative {
        return libm::atan(offset / scale.abs()) / PI + 0.5;
    }
    let squares = scale * scale + offset * offset;
    // The function's denominator includes pi before division. The scalar
    // density divides the scale first, which matters when squares is large.
    if function {
        scale.abs() / (squares * PI)
    } else {
        (scale / PI) / squares
    }
}

fn inverse_cauchy(location: f64, scale: f64, probability: f64) -> f64 {
    if location.is_nan() || scale.is_nan() || scale <= 0.0 || !(0.0..=1.0).contains(&probability) {
        return f64::NAN;
    }
    match probability {
        0.0 => f64::NEG_INFINITY,
        1.0 => f64::INFINITY,
        _ => location + scale * libm::tan(PI * (probability - 0.5)),
    }
}

fn logistic(mean: f64, scale: f64, x: f64, cumulative: bool, function: bool) -> f64 {
    let scale = scale.abs();
    let tail = libm::exp(-(x - mean) / scale);
    let plus_one = tail + 1.0;
    if cumulative {
        1.0 / plus_one
    } else if function {
        // The function multiplies the scale into the denominator. The
        // scalar density divides by it first.
        tail / (scale * (plus_one * plus_one))
    } else {
        tail / scale / (plus_one * plus_one)
    }
}

fn inverse_logistic(mean: f64, scale: f64, probability: f64) -> f64 {
    if mean.is_nan() || scale.is_nan() || probability.is_nan() {
        return f64::NAN;
    }
    // The reference returns 0, not undefined, outside its domain.
    if scale <= 0.0 || probability <= 0.0 || probability > 1.0 {
        return 0.0;
    }
    mean - libm::log(1.0 / probability - 1.0) * scale
}

fn log_normal(mean: f64, deviation: f64, x: f64, cumulative: bool, function: bool) -> f64 {
    // The scalar form is undefined for undefined inputs or a non-positive
    // deviation. The function form uses the absolute deviation unguarded.
    if !function && (mean.is_nan() || deviation.is_nan() || x.is_nan() || deviation <= 0.0) {
        return f64::NAN;
    }
    // The support starts after 0, including 0 itself.
    if x <= 0.0 {
        return 0.0;
    }
    let offset = libm::log(x) - mean;
    if cumulative {
        return libm::erf(offset / (SQRT_2 * deviation.abs())) * 0.5 + 0.5;
    }
    let product = x * (2.0 * PI).sqrt() * deviation.abs();
    libm::exp(-(offset * offset) / (deviation * deviation * 2.0)) / product
}

fn inverse_log_normal(mean: f64, deviation: f64, probability: f64) -> f64 {
    if mean.is_nan() || deviation.is_nan() || probability.is_nan() {
        return f64::NAN;
    }
    // The reference returns 0, not undefined, outside its domain.
    if deviation <= 0.0 || probability <= 0.0 || probability > 1.0 {
        return 0.0;
    }
    libm::exp(erf_inv(2.0 * (probability - 0.5)) * SQRT_2 * deviation + mean)
}

/// The inverse error function of the reference's math library, a piecewise
/// polynomial in `-ln((1 - x)(1 + x))` (Giles 2010).
/// Translated from Apache Commons Math `Erf.erfInv` (Apache License 2.0,
/// see NOTICE), which the reference uses for the log-normal quantile.
fn erf_inv(x: f64) -> f64 {
    const CENTRAL: [f64; 23] = [
        -3.64441206401782E-21,
        -1.6850591381820166E-19,
        1.28584807152564E-18,
        1.1157877678025181E-17,
        -1.333171662854621E-16,
        2.0972767875968562E-17,
        6.637638134358324E-15,
        -4.054566272975207E-14,
        -8.151934197605472E-14,
        2.6335093153082323E-12,
        -1.2975133253453532E-11,
        -5.415412054294628E-11,
        1.0512122733215323E-9,
        -4.112633980346984E-9,
        -2.9070369957882005E-8,
        4.2347877827932404E-7,
        -1.3654692000834679E-6,
        -1.3882523362786469E-5,
        1.8673420803405714E-4,
        -7.40702534166267E-4,
        -0.006033670871430149,
        0.24015818242558962,
        1.6536545626831027,
    ];
    const MIDDLE: [f64; 19] = [
        2.2137376921775787E-9,
        9.075656193888539E-8,
        -2.7517406297064545E-7,
        1.8239629214389228E-8,
        1.5027403968909828E-6,
        -4.013867526981546E-6,
        2.9234449089955446E-6,
        1.2475304481671779E-5,
        -4.7318229009055734E-5,
        6.828485145957318E-5,
        2.4031110387097894E-5,
        -3.550375203628475E-4,
        9.532893797373805E-4,
        -0.0016882755560235047,
        0.002491442096107851,
        -0.003751208507569241,
        0.005370914553590064,
        1.0052589676941592,
        3.0838856104922208,
    ];
    const TAIL: [f64; 17] = [
        -2.7109920616438573E-11,
        -2.555641816996525E-10,
        1.5076572693500548E-9,
        -3.789465440126737E-9,
        7.61570120807834E-9,
        -1.496002662714924E-8,
        2.914795345090108E-8,
        -6.771199775845234E-8,
        2.2900482228026655E-7,
        -9.9298272942317E-7,
        4.526062597223154E-6,
        -1.968177810553167E-5,
        7.599527703001776E-5,
        -2.1503011930044477E-4,
        -1.3871931833623122E-4,
        1.0103004648645344,
        4.849906401408584,
    ];
    let horner = |coefficients: &[f64], w: f64| {
        coefficients[1..]
            .iter()
            .fold(coefficients[0], |p, &c| c + p * w)
    };
    // Keep the factored form; 1 - x² rounds badly near either end.
    let w = -libm::log((1.0 - x) * (1.0 + x));
    let p = if w < 6.25 {
        horner(&CENTRAL, w - 3.125)
    } else if w < 16.0 {
        horner(&MIDDLE, w.sqrt() - 3.25)
    } else if !w.is_infinite() {
        horner(&TAIL, w.sqrt() - 5.0)
    } else {
        f64::INFINITY
    };
    p * x
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
    fn log_normal_text_brackets_a_compound_deviation_before_squaring() {
        let c = crate::script::load(
            "m = 1\ns = 2\nq = LogNormal(m,s+1,x,false)\nt = FormulaText(q,false)\nu = Text(q,false)",
        )
        .unwrap();
        let text = |label| c.text(c.id(label).unwrap()).unwrap().to_owned();
        assert!(
            text("t").contains(r"\left(s + 1 \right)^{2}"),
            "{}",
            text("t")
        );
        assert!(text("u").contains("(s + 1)²"), "{}", text("u"));
        let c = crate::script::load(
            "q = LogNormal(1,-3,x,false)\nt = FormulaText(q,false)\nu = Text(q,false)",
        )
        .unwrap();
        let text = |label| c.text(c.id(label).unwrap()).unwrap().to_owned();
        assert!(text("t").contains(r"\left(-3 \right)^{2}"), "{}", text("t"));
        assert!(text("u").contains("(-3)²"), "{}", text("u"));
    }

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
            "f = Cauchy(0,1,x+1)",
            "f = Cauchy(0,x,x)",
            "f = Cauchy(0,1,2,3)",
            "f = InverseCauchy(0,1,x)",
            "f = Logistic(0,1,x+1)",
            "f = Logistic(0,x,x)",
            "f = Logistic(0,1,2,3)",
            "f = Logistic(0,1,x,2)",
            "f = InverseLogistic(0,1,x)",
            "f = LogNormal(0,1,x+1)",
            "f = LogNormal(0,x,x)",
            "f = LogNormal(x,1,x)",
            "f = LogNormal(0,1,2,3)",
            "f = LogNormal(0,1,x,2)",
            "f = InverseLogNormal(0,1,x)",
        ] {
            assert!(crate::script::load(source).is_err(), "{source}");
        }
    }
}
