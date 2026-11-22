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

    pub(crate) fn from_value(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let text = format!("{value:e}");
        let (mantissa, exponent) = text.split_once('e')?;
        let scale = mantissa
            .split_once('.')
            .map_or(0, |(_, digits)| digits.len());
        Self::decimal(
            mantissa.replace('.', "").parse().ok()?,
            exponent.parse::<i64>().ok()? - scale as i64,
        )
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
