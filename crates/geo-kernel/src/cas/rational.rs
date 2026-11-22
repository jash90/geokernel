//! Checked rational arithmetic with cross-cancellation before multiplication.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rat {
    pub(crate) num: i128,
    pub(crate) den: i128,
}

impl Rat {
    pub(crate) const ZERO: Self = Self { num: 0, den: 1 };
    pub(crate) const ONE: Self = Self { num: 1, den: 1 };

    pub(crate) fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 || den == i128::MIN || num == i128::MIN {
            return None;
        }
        let gcd = gcd(num.unsigned_abs(), den.unsigned_abs()) as i128;
        let sign = den.signum();
        Some(Self {
            num: (num / gcd) * sign,
            den: den.abs() / gcd,
        })
    }

    pub(crate) fn decimal(coefficient: i128, exponent: i64) -> Option<Self> {
        if coefficient == 0 {
            return Some(Self::ZERO);
        }
        let power = 10i128.checked_pow(exponent.unsigned_abs().try_into().ok()?)?;
        if exponent < 0 {
            Self::new(coefficient, power)
        } else {
            Self::new(coefficient.checked_mul(power)?, 1)
        }
    }

    /// A double as the reference prints it into a CAS call: its 15
    /// significant digits (`StringTemplate.prefixedDefaultSF`, whose
    /// `ScientificFormat` is a `DecimalFormat` with the pattern
    /// `0.00000000000000E0` and `RoundingMode.HALF_UP`). `DecimalFormat`
    /// rounds the exact binary value of the double, so a tie such as
    /// `1.000030517578125` rounds away from zero to `1.00003051757813`.
    pub(crate) fn printed(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        if value == 0.0 {
            return Some(Self::ZERO);
        }
        // The exact decimal expansion: a double has at most 767 significant
        // digits.
        let text = format!("{:.800e}", value.abs());
        let (mantissa, exponent) = text.split_once('e')?;
        let digits: Vec<u8> = mantissa
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|d| d - b'0')
            .collect();
        let mut kept: i128 = digits[..15].iter().fold(0, |n, &d| n * 10 + i128::from(d));
        if digits[15] >= 5 {
            kept += 1;
        }
        let exponent = exponent.parse::<i64>().ok()? - 14;
        let sign = if value < 0.0 { -1 } else { 1 };
        Self::decimal(sign * kept, exponent)
    }

    pub(crate) fn neg(self) -> Option<Self> {
        Self::new(self.num.checked_neg()?, self.den)
    }

    pub(crate) fn add(self, rhs: Self) -> Option<Self> {
        let g = gcd(self.den as u128, rhs.den as u128) as i128;
        let right = rhs.den / g;
        let left = self.den / g;
        let num = self
            .num
            .checked_mul(right)?
            .checked_add(rhs.num.checked_mul(left)?)?;
        let reduction = gcd(num.unsigned_abs(), g as u128) as i128;
        Self::new(num / reduction, (self.den / reduction).checked_mul(right)?)
    }

    pub(crate) fn sub(self, rhs: Self) -> Option<Self> {
        self.add(rhs.neg()?)
    }

    pub(crate) fn mul(self, rhs: Self) -> Option<Self> {
        let g = gcd(self.num.unsigned_abs(), rhs.den as u128) as i128;
        let h = gcd(rhs.num.unsigned_abs(), self.den as u128) as i128;
        Self::new(
            (self.num / g).checked_mul(rhs.num / h)?,
            (self.den / h).checked_mul(rhs.den / g)?,
        )
    }

    pub(crate) fn div(self, rhs: Self) -> Option<Self> {
        self.mul(Self::new(rhs.den, rhs.num)?)
    }

    pub(crate) fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

pub(crate) fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod printed_tests {
    use super::Rat;

    #[test]
    fn printing_rounds_ties_away_from_zero_on_the_exact_binary_value() {
        let digits = |v: f64| Rat::printed(v).unwrap();
        // Exactly representable ties round up, as `RoundingMode.HALF_UP`.
        assert_eq!(
            digits(1.000030517578125),
            Rat::decimal(100_003_051_757_813, -14).unwrap()
        );
        assert_eq!(
            digits(1.000091552734375),
            Rat::decimal(100_009_155_273_438, -14).unwrap()
        );
        assert_eq!(
            digits(-1.000030517578125),
            Rat::decimal(-100_003_051_757_813, -14).unwrap()
        );
        // 0.1 is a little above one tenth, but its 15 digits are 0.1.
        assert_eq!(digits(0.1), Rat::new(1, 10).unwrap());
        assert_eq!(
            digits(1.0 / 3.0),
            Rat::decimal(333_333_333_333_333, -15).unwrap()
        );
        assert_eq!(digits(0.0), Rat::ZERO);
        assert_eq!(Rat::printed(f64::NAN), None);
    }
}
