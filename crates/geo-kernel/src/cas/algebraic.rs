//! Closed forms of one variable solved from an equation in two variables:
//! a rational function, optionally with one square root.

use super::Rat;

/// `(numerator + sign * multiplier * sqrt(radicand)) / denominator`, every
/// part a polynomial in one parameter with ascending coefficients.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Algebraic {
    pub(crate) numerator: Vec<Rat>,
    pub(crate) denominator: Vec<Rat>,
    pub(crate) radical: Option<Radical>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Radical {
    pub(crate) negative: bool,
    pub(crate) multiplier: Vec<Rat>,
    pub(crate) radicand: Vec<Rat>,
}

fn horner(coefficients: &[Rat], t: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |value, c| value * t + c.to_f64())
}

impl Algebraic {
    /// The value at `t`; not finite at a pole and where the radicand is
    /// negative.
    pub(crate) fn value(&self, t: f64) -> f64 {
        let mut value = horner(&self.numerator, t);
        if let Some(radical) = &self.radical {
            let root = horner(&radical.multiplier, t) * horner(&radical.radicand, t).sqrt();
            value += if radical.negative { -root } else { root };
        }
        value / horner(&self.denominator, t)
    }

    /// The coefficients `[constant, slope]` when the form is a polynomial of
    /// degree at most one in the parameter: a constant denominator and, if
    /// there is a root, a constant radicand.
    pub(crate) fn linear(&self) -> Option<[f64; 2]> {
        let [denominator] = self.denominator[..] else {
            return None;
        };
        let denominator = denominator.to_f64();
        let mut coefficients = [0.0; 2];
        if self.numerator.len() > 2 {
            return None;
        }
        for (slot, c) in coefficients.iter_mut().zip(&self.numerator) {
            *slot = c.to_f64();
        }
        if let Some(radical) = &self.radical {
            let [radicand] = radical.radicand[..] else {
                return None;
            };
            if radical.multiplier.len() > 2 {
                return None;
            }
            let root = radicand.to_f64().sqrt();
            let root = if radical.negative { -root } else { root };
            for (slot, c) in coefficients.iter_mut().zip(&radical.multiplier) {
                *slot += c.to_f64() * root;
            }
        }
        Some(coefficients.map(|c| c / denominator))
    }

    /// The expression in `variable`, for display.
    pub(crate) fn display(&self, variable: &str) -> String {
        let polynomial = |coefficients: &[Rat]| print(coefficients, variable);
        let mut text = polynomial(&self.numerator);
        if let Some(radical) = &self.radical {
            let root = format!("sqrt({})", polynomial(&radical.radicand));
            let root = match radical.multiplier[..] {
                [one] if one == Rat::ONE => root,
                _ => format!("({})*{root}", polynomial(&radical.multiplier)),
            };
            let sign = if radical.negative { '-' } else { '+' };
            text = if text == "0" {
                format!("{}{root}", if radical.negative { "-" } else { "" })
            } else {
                format!("{text} {sign} {root}")
            };
        }
        match self.denominator[..] {
            [one] if one == Rat::ONE => text,
            _ => format!("({text})/({})", polynomial(&self.denominator)),
        }
    }
}

fn print(coefficients: &[Rat], variable: &str) -> String {
    let mut text = String::new();
    for (power, coefficient) in coefficients.iter().enumerate().rev() {
        if coefficient.num == 0 {
            continue;
        }
        let magnitude = Rat {
            num: coefficient.num.abs(),
            den: coefficient.den,
        };
        if text.is_empty() {
            if coefficient.num < 0 {
                text.push('-');
            }
        } else {
            text.push_str(if coefficient.num < 0 { " - " } else { " + " });
        }
        let number = if magnitude.den == 1 {
            magnitude.num.to_string()
        } else {
            format!("{}/{}", magnitude.num, magnitude.den)
        };
        match power {
            0 => text.push_str(&number),
            _ => {
                if magnitude != Rat::ONE {
                    text.push_str(&number);
                    text.push('*');
                }
                text.push_str(variable);
                if power > 1 {
                    text.push('^');
                    text.push_str(&power.to_string());
                }
            }
        }
    }
    if text.is_empty() {
        text.push('0');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rats(values: &[i128]) -> Vec<Rat> {
        values.iter().map(|&v| Rat::new(v, 1).unwrap()).collect()
    }

    #[test]
    fn a_circle_branch_is_undefined_outside_its_interval() {
        // x = -sqrt(4 - y^2)
        let branch = Algebraic {
            numerator: Vec::new(),
            denominator: rats(&[1]),
            radical: Some(Radical {
                negative: true,
                multiplier: rats(&[1]),
                radicand: rats(&[4, 0, -1]),
            }),
        };
        assert_eq!(branch.value(0.0), -2.0);
        assert_eq!(branch.value(2.0), 0.0);
        assert!(branch.value(4.0).is_nan());
        assert_eq!(branch.linear(), None);
        assert_eq!(branch.display("y"), "-sqrt(-y^2 + 4)");
    }

    #[test]
    fn a_rational_branch_has_a_pole() {
        // y = (1 - x^2) / x
        let branch = Algebraic {
            numerator: rats(&[1, 0, -1]),
            denominator: rats(&[0, 1]),
            radical: None,
        };
        assert_eq!(branch.value(2.0), -1.5);
        assert!(!branch.value(0.0).is_finite());
        assert_eq!(branch.display("x"), "(-x^2 + 1)/(x)");
    }

    #[test]
    fn an_irrational_slope_is_linear() {
        // x = (0 + y * sqrt(8)) / 2 = sqrt(2) * y
        let branch = Algebraic {
            numerator: Vec::new(),
            denominator: rats(&[2]),
            radical: Some(Radical {
                negative: false,
                multiplier: rats(&[0, 1]),
                radicand: rats(&[8]),
            }),
        };
        let [constant, slope] = branch.linear().unwrap();
        assert_eq!(constant, 0.0);
        assert!((slope - std::f64::consts::SQRT_2).abs() < 1e-15);
        assert!((branch.value(-3.0) + 3.0 * std::f64::consts::SQRT_2).abs() < 1e-15);
    }
}
