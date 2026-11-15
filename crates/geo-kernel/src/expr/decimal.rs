//! Exact decimal numbers, for the arithmetic GeoGebra does on decimal
//! literals with Java's `BigDecimal` (see `DoubleOperation`): addition,
//! subtraction and multiplication are exact; division and whole powers are
//! rounded to 34 significant digits, half to even (`MathContext.DECIMAL128`);
//! `round` uses `setScale(digits, HALF_UP)`.
//!
//! Written from the documented behaviour of those `BigDecimal` operations.
//! Only values matter here, never the scale a `BigDecimal` would carry, so a
//! number is kept in a canonical form: a coefficient without trailing zeros.
//! A coefficient of up to 38 digits is kept inline in a `u128`, so the
//! common small cases neither allocate nor loop over digits; one of 39 to
//! 77 digits is kept inline in a 256-bit integer ([`U256`]), which holds
//! the exact decimal of most doubles (see [`Dec::from_f64`]); longer ones
//! use a digit vector.
//!
//! What allocates: a result or an operand of more than 77 digits (the
//! digit vector); the exact decimal of a double that has more than 77
//! digits (a double with a full 53-bit mantissa below about `2^-35`, or
//! one of `2^256` or more); a division whose divisor has more than 36
//! digits; a power whose base has more than 38 digits or whose exact
//! intermediate products pass 38 digits (before rounding to the working
//! precision); `setScale` and the other operations never allocate below
//! 78 digits.
//!
//! The work of one operation is bounded, not the size of the numbers
//! GeoGebra would handle: a result may have up to [`MAX_DIGITS`] digits; a
//! product may take up to [`MAX_WORK`] digit products; the exponent stays
//! in the range of Java's `int` scale. An operation beyond these limits
//! gives `None` and the caller computes with doubles from there on (where
//! GeoGebra throws, runs out of memory or carries on, see `DIFFERENCES.md`).

use super::wide::{self, U256};
use std::cmp::Ordering;
use std::sync::Arc;

/// Most digits a decimal may have.
pub(crate) const MAX_DIGITS: usize = 100_000;

/// Most digit products a multiplication or division may take.
pub(crate) const MAX_WORK: usize = 10_000_000;

/// Largest exponent magnitude: Java's `BigDecimal` scale is an `int`.
const MAX_EXP: i64 = i32::MAX as i64;

/// Significant digits of `MathContext.DECIMAL128`.
const PRECISION: usize = 34;

/// Digits that always fit a `u128` (`10^38 < 2^128`).
const SMALL_DIGITS: usize = 38;

const POW10: [u128; 39] = {
    let mut t = [1u128; 39];
    let mut i = 1;
    while i < 39 {
        t[i] = t[i - 1] * 10;
        i += 1;
    }
    t
};

/// `sign · coefficient · 10^exp`; the coefficient has no trailing zero, and
/// zero is `+0 · 10^0`. A coefficient of at most 38 digits is always
/// [`Mag::Small`], one of 39 to 77 digits [`Mag::Wide`], a longer one
/// [`Mag::Big`], so equal values compare equal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dec {
    neg: bool,
    exp: i64,
    mag: Mag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mag {
    Small(u128),
    /// 39 to 77 digits.
    Wide(U256),
    /// Digits, least significant first, no zero at either end.
    Big(Arc<[u8]>),
}

/// An intermediate result before it is normalised.
struct Raw {
    neg: bool,
    digits: Vec<u8>,
    exp: i64,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    HalfEven,
    HalfUp,
}

#[cfg(test)]
thread_local! {
    /// Decimal operations done on this thread (see the expression tests).
    pub(crate) static OPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn count_op() {
    #[cfg(test)]
    OPS.with(|n| n.set(n.get() + 1));
}

fn digit_count(x: u128) -> usize {
    POW10.iter().position(|&p| p > x).unwrap_or(39)
}

/// How many decimal digits `n` has (`n.to_string().len()`), without
/// allocating.
fn decimal_digits(n: u32) -> usize {
    let mut digits = 1;
    let mut rest = n / 10;
    while rest > 0 {
        digits += 1;
        rest /= 10;
    }
    digits
}

fn to_digits(mut x: u128) -> Vec<u8> {
    let mut d = Vec::with_capacity(39);
    while x > 0 {
        d.push((x % 10) as u8);
        x /= 10;
    }
    d
}

impl Dec {
    fn zero() -> Dec {
        Dec {
            neg: false,
            exp: 0,
            mag: Mag::Small(0),
        }
    }

    fn is_zero(&self) -> bool {
        match &self.mag {
            Mag::Small(c) => *c == 0,
            Mag::Wide(_) => false,
            Mag::Big(d) => d.is_empty(),
        }
    }

    fn small_coef(&self) -> Option<u128> {
        match self.mag {
            Mag::Small(c) => Some(c),
            _ => None,
        }
    }

    /// The coefficient when it is inline.
    fn wide_coef(&self) -> Option<U256> {
        match self.mag {
            Mag::Small(c) => Some(U256::from_u128(c)),
            Mag::Wide(w) => Some(w),
            Mag::Big(_) => None,
        }
    }

    /// The canonical decimal `±coef · 10^exp` of an inline coefficient.
    fn wide(neg: bool, mut coef: U256, mut exp: i64) -> Option<Dec> {
        if coef.is_zero() {
            return Some(Dec::zero());
        }
        exp += coef.strip_zeros();
        if let Some(c) = coef.to_u128() {
            return Dec::small(neg, c, exp);
        }
        if coef.digit_count() > wide::DIGITS {
            let mut buf = [0; 78];
            return Dec::from_raw(Raw {
                neg,
                digits: coef.write_digits(&mut buf).to_vec(),
                exp,
            });
        }
        (exp.abs() <= MAX_EXP).then_some(Dec {
            neg,
            exp,
            mag: Mag::Wide(coef),
        })
    }

    /// The canonical decimal `±coef · 10^exp`.
    fn small(neg: bool, mut coef: u128, mut exp: i64) -> Option<Dec> {
        if coef == 0 {
            return Some(Dec::zero());
        }
        while coef.is_multiple_of(10) {
            coef /= 10;
            exp += 1;
        }
        if coef >= POW10[SMALL_DIGITS] {
            return (exp.abs() <= MAX_EXP).then_some(Dec {
                neg,
                exp,
                mag: Mag::Wide(U256::from_u128(coef)),
            });
        }
        (exp.abs() <= MAX_EXP).then_some(Dec {
            neg,
            exp,
            mag: Mag::Small(coef),
        })
    }

    /// Normalises and checks the size.
    fn from_raw(raw: Raw) -> Option<Dec> {
        let Raw {
            neg,
            mut digits,
            mut exp,
        } = raw;
        while digits.last() == Some(&0) {
            digits.pop();
        }
        let low = digits.iter().take_while(|&&d| d == 0).count();
        digits.drain(..low);
        exp += low as i64;
        if digits.is_empty() {
            return Some(Dec::zero());
        }
        if digits.len() > MAX_DIGITS || exp.abs() > MAX_EXP {
            return None;
        }
        let mag = if digits.len() <= SMALL_DIGITS {
            Mag::Small(digits.iter().rev().fold(0, |v, &d| v * 10 + u128::from(d)))
        } else if digits.len() <= wide::DIGITS {
            Mag::Wide(U256::from_digits(&digits))
        } else {
            Mag::Big(digits.into())
        };
        Some(Dec { neg, exp, mag })
    }

    /// The digits, least significant first (allocates).
    fn digits(&self) -> Vec<u8> {
        match &self.mag {
            Mag::Small(c) => to_digits(*c),
            Mag::Wide(w) => w.write_digits(&mut [0; 78]).to_vec(),
            Mag::Big(d) => d.to_vec(),
        }
    }

    fn len(&self) -> usize {
        match &self.mag {
            Mag::Small(c) => digit_count(*c),
            Mag::Wide(w) => w.digit_count(),
            Mag::Big(d) => d.len(),
        }
    }

    /// A decimal literal: `12`, `-0.5`, `.5`, `5.`, `1.5E-3`.
    pub(crate) fn parse(text: &str) -> Option<Dec> {
        let (neg, rest) = match text.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (mantissa, exponent) = match rest.find(['E', 'e']) {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        let mut exp: i64 = match exponent {
            None => 0,
            Some(e) => {
                let (eneg, digits) = match e.strip_prefix('-') {
                    Some(d) => (true, d),
                    None => (false, e.strip_prefix('+').unwrap_or(e)),
                };
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let v: i64 = digits.parse().unwrap_or(i64::MAX / 2).min(1 << 50);
                if eneg {
                    -v
                } else {
                    v
                }
            }
        };
        let mut digits = Vec::new();
        let mut seen_point = false;
        for b in mantissa.bytes() {
            match b {
                b'0'..=b'9' => {
                    digits.push(b - b'0');
                    if seen_point {
                        exp -= 1;
                    }
                }
                b'.' if !seen_point => seen_point = true,
                _ => return None,
            }
        }
        if digits.is_empty() {
            return None;
        }
        digits.reverse();
        Dec::from_raw(Raw {
            neg: neg && digits.iter().any(|&d| d != 0),
            digits,
            exp,
        })
    }

    /// The exact value of a finite double (`new BigDecimal(double)`).
    pub(crate) fn from_f64(x: f64) -> Option<Dec> {
        count_op();
        if !x.is_finite() {
            return None;
        }
        if x == 0.0 {
            return Some(Dec::zero());
        }
        let bits = x.to_bits();
        let neg = bits >> 63 == 1;
        let raw_exp = ((bits >> 52) & 0x7ff) as i64;
        let frac = bits & ((1 << 52) - 1);
        let (mut m, mut e) = if raw_exp == 0 {
            (frac, -1074)
        } else {
            (frac | (1 << 52), raw_exp - 1075)
        };
        let zeros = m.trailing_zeros();
        m >>= zeros;
        e += i64::from(zeros);
        let m = u128::from(m);
        if (0..=74).contains(&e) {
            return Dec::small(neg, m << e, 0);
        }
        if (-31..0).contains(&e) {
            // m · 2^e = m · 5^-e · 10^e, with m · 5^-e below 2^127
            return Dec::small(neg, m * 5u128.pow((-e) as u32), e);
        }
        // the same in 256 bits, while the coefficient fits
        let inline = if e > 0 {
            U256::from_u128(m).checked_shl(e as u32).map(|c| (c, 0))
        } else {
            U256::pow5((-e) as usize)
                .and_then(|p| p.checked_mul(U256::from_u128(m)))
                .map(|c| (c, e))
        };
        if let Some((c, exp)) = inline.filter(|(c, _)| c.digit_count() <= wide::DIGITS) {
            return Dec::wide(neg, c, exp);
        }
        Dec::from_f64_digits(neg, m, e)
    }

    /// [`Dec::from_f64`] of `±m · 2^e` on a digit vector.
    fn from_f64_digits(neg: bool, m: u128, e: i64) -> Option<Dec> {
        let mut digits = to_digits(m);
        if e >= 0 {
            for _ in 0..e {
                mul_small(&mut digits, 2);
            }
            Dec::from_raw(Raw {
                neg,
                digits,
                exp: 0,
            })
        } else {
            for _ in 0..-e {
                mul_small(&mut digits, 5);
            }
            Dec::from_raw(Raw {
                neg,
                digits,
                exp: e,
            })
        }
    }

    /// The nearest double (`BigDecimal.doubleValue`), without allocating for
    /// a small coefficient.
    pub(crate) fn to_f64(&self) -> f64 {
        let v = match self.mag {
            Mag::Small(c) => to_f64_parts(c, self.exp),
            Mag::Wide(w) => digits_to_f64(w.write_digits(&mut [0; 78]), self.exp),
            Mag::Big(ref d) if d.is_empty() => 0.0,
            Mag::Big(ref d) => {
                let mut s = String::with_capacity(d.len() + 24);
                s.extend(d.iter().rev().map(|&d| char::from(b'0' + d)));
                s.push('e');
                s.push_str(&self.exp.to_string());
                s.parse().unwrap_or(f64::NAN)
            }
        };
        if self.neg {
            -v
        } else {
            v
        }
    }

    pub(crate) fn negated(&self) -> Dec {
        let mut d = self.clone();
        d.neg = !d.neg && !d.is_zero();
        d
    }

    pub(crate) fn add(&self, other: &Dec) -> Option<Dec> {
        count_op();
        if self.is_zero() {
            return Some(other.clone());
        }
        if other.is_zero() {
            return Some(self.clone());
        }
        let exp = self.exp.min(other.exp);
        if let (Some(a), Some(b)) = (self.small_coef(), other.small_coef()) {
            let shift = |c: u128, by: i64| {
                usize::try_from(by)
                    .ok()
                    .and_then(|by| POW10.get(by))
                    .and_then(|&p| c.checked_mul(p))
            };
            if let (Some(a), Some(b)) = (shift(a, self.exp - exp), shift(b, other.exp - exp)) {
                let sum = if self.neg == other.neg {
                    a.checked_add(b).map(|s| (self.neg, s))
                } else if a >= b {
                    Some((self.neg, a - b))
                } else {
                    Some((other.neg, b - a))
                };
                if let Some((neg, s)) = sum {
                    return Dec::small(neg, s, exp);
                }
            }
        }
        if let (Some(a), Some(b)) = (self.wide_coef(), other.wide_coef()) {
            let shift = |c: U256, by: i64| {
                usize::try_from(by)
                    .ok()
                    .and_then(U256::pow10)
                    .and_then(|p| c.checked_mul(p))
            };
            if let (Some(a), Some(b)) = (shift(a, self.exp - exp), shift(b, other.exp - exp)) {
                let sum = if self.neg == other.neg {
                    a.checked_add(b).map(|s| (self.neg, s))
                } else if a >= b {
                    Some((self.neg, a.sub(b)))
                } else {
                    Some((other.neg, b.sub(a)))
                };
                if let Some((neg, s)) = sum {
                    return Dec::wide(neg, s, exp);
                }
            }
        }
        // aligning must not need more digits than a result may have
        let (la, lb) = (self.len() as i64, other.len() as i64);
        if (la + self.exp - exp).max(lb + other.exp - exp) > MAX_DIGITS as i64 + 1 {
            return None;
        }
        let a = shifted(&self.digits(), (self.exp - exp) as usize);
        let b = shifted(&other.digits(), (other.exp - exp) as usize);
        let (neg, digits) = if self.neg == other.neg {
            (self.neg, add_mag(&a, &b))
        } else {
            match cmp_mag(&a, &b) {
                Ordering::Equal => return Some(Dec::zero()),
                Ordering::Greater => (self.neg, sub_mag(&a, &b)),
                Ordering::Less => (other.neg, sub_mag(&b, &a)),
            }
        };
        Dec::from_raw(Raw { neg, digits, exp })
    }

    pub(crate) fn sub(&self, other: &Dec) -> Option<Dec> {
        if other.is_zero() {
            count_op();
            return Some(self.clone());
        }
        // negating does not allocate: a digit vector is shared
        self.add(&other.negated())
    }

    pub(crate) fn mul(&self, other: &Dec) -> Option<Dec> {
        count_op();
        if self.is_zero() || other.is_zero() {
            return Some(Dec::zero());
        }
        let neg = self.neg != other.neg;
        let exp = self.exp + other.exp;
        if let (Some(a), Some(b)) = (self.small_coef(), other.small_coef()) {
            if let Some(p) = a.checked_mul(b) {
                return Dec::small(neg, p, exp);
            }
        }
        if let (Some(a), Some(b)) = (self.wide_coef(), other.wide_coef()) {
            if let Some(p) = a.checked_mul(b) {
                return Dec::wide(neg, p, exp);
            }
        }
        Dec::from_raw(self.mul_raw(other)?)
    }

    /// The exact product, before normalising; `None` beyond [`MAX_WORK`].
    fn mul_raw(&self, other: &Dec) -> Option<Raw> {
        let (a, b) = (self.digits(), other.digits());
        if a.len() * b.len() > MAX_WORK {
            return None;
        }
        Some(Raw {
            neg: self.neg != other.neg,
            digits: mul_mag(&a, &b),
            exp: self.exp + other.exp,
        })
    }

    /// `divide(other, DECIMAL128)`; `None` for a zero divisor.
    pub(crate) fn div(&self, other: &Dec) -> Option<Dec> {
        count_op();
        if other.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some(Dec::zero());
        }
        let neg = self.neg != other.neg;
        if let (Some(x), Some(y)) = (self.small_coef(), other.small_coef()) {
            if y < POW10[37] {
                return div_small(neg, x, y, self.exp - other.exp);
            }
        }
        if let (Some(x), Some(y)) = (self.wide_coef(), other.small_coef()) {
            if y < POW10[37] {
                return div_wide(neg, x, y, self.exp - other.exp);
            }
        }
        // the quotient needs PRECISION + 2 digits: more of the dividend
        // only decide whether the remainder is zero
        let (a, b) = (self.digits(), other.digits());
        let want = PRECISION + 2 + b.len();
        if b.len() * want > MAX_WORK {
            return None;
        }
        let (num, exp, sticky) = if a.len() >= want {
            let drop = a.len() - want;
            let sticky = a[..drop].iter().any(|&d| d != 0);
            (a[drop..].to_vec(), self.exp + drop as i64, sticky)
        } else {
            let pad = want - a.len();
            (shifted(&a, pad), self.exp - pad as i64, false)
        };
        let (q, r) = div_mag(&num, &b);
        round_raw(
            Raw {
                neg,
                digits: q,
                exp: exp - other.exp,
            },
            PRECISION,
            sticky || !r.is_empty(),
            Mode::HalfEven,
        )
    }

    /// `pow(n, DECIMAL128)` for `0 <= n < 1e6`: repeated squaring into one
    /// accumulator, each product rounded to `34 + digits(n) + 1` digits, and
    /// the result rounded to 34 digits (the algorithm `BigDecimal.pow`
    /// documents, from ANSI X3.274-1996).
    pub(crate) fn pow(&self, n: u32) -> Option<Dec> {
        count_op();
        if n == 0 {
            return Dec::small(false, 1, 0);
        }
        let work = PRECISION + decimal_digits(n) + 1;
        // nothing is rounded while every product stays within the working
        // precision, so exact products of small coefficients are the result
        if let Some(acc) = self.pow_small(n, POW10[work.min(SMALL_DIGITS)]) {
            return round_dec(acc, PRECISION);
        }
        let mut acc = Raw {
            neg: false,
            digits: vec![1],
            exp: 0,
        };
        let mut seen = false;
        for bit in (0..32).rev() {
            if seen {
                let a = Dec::from_raw_unchecked(&acc);
                acc = round_raw_to(a.mul_raw(&a)?, work)?;
            }
            if (n >> bit) & 1 == 1 {
                acc = round_raw_to(Dec::from_raw_unchecked(&acc).mul_raw(self)?, work)?;
                seen = true;
            }
        }
        round_raw(acc, PRECISION, false, Mode::HalfEven)
    }

    fn pow_small(&self, n: u32, limit: u128) -> Option<Dec> {
        let base = self.small_coef()?;
        let mut acc: u128 = 1;
        let mut seen = false;
        for bit in (0..32).rev() {
            if seen {
                acc = acc.checked_mul(acc).filter(|&a| a < limit)?;
            }
            if (n >> bit) & 1 == 1 {
                acc = acc.checked_mul(base).filter(|&a| a < limit)?;
                seen = true;
            }
        }
        let exp = self.exp.checked_mul(i64::from(n))?;
        Dec::small(self.neg && n % 2 == 1, acc, exp)
    }

    /// A [`Dec`] with the digits of an intermediate (which may be longer
    /// than [`MAX_DIGITS`] for one step) for multiplying it.
    fn from_raw_unchecked(raw: &Raw) -> Dec {
        Dec {
            neg: raw.neg,
            exp: raw.exp,
            mag: Mag::Big(raw.digits.clone().into()),
        }
    }

    /// `setScale(scale, HALF_UP)`: rounded to `scale` decimal places.
    pub(crate) fn set_scale_half_up(&self, scale: i64) -> Option<Dec> {
        count_op();
        let drop = -scale - self.exp;
        if drop <= 0 {
            return Some(self.clone());
        }
        if let Some(c) = self.small_coef() {
            let Some(&p) = usize::try_from(drop).ok().and_then(|d| POW10.get(d)) else {
                // every digit goes and the first dropped one is a zero
                return Some(Dec::zero());
            };
            let kept = c / p + u128::from(c % p >= p / 2);
            return Dec::small(self.neg, kept, self.exp + drop);
        }
        if let Mag::Wide(c) = self.mag {
            let Some(p) = usize::try_from(drop).ok().and_then(U256::pow10) else {
                return Some(Dec::zero());
            };
            let (q, r) = c.divrem(p);
            let half = p.divrem_u64(2).0;
            let kept = if r >= half {
                q.checked_add(U256::from_u128(1))?
            } else {
                q
            };
            return Dec::wide(self.neg, kept, self.exp + drop);
        }
        let len = self.len() as i64;
        if drop > len {
            return Some(Dec::zero());
        }
        round_raw(
            Raw {
                neg: self.neg,
                digits: self.digits(),
                exp: self.exp,
            },
            (len - drop) as usize,
            false,
            Mode::HalfUp,
        )
    }

    /// The same value with its coefficient as a digit vector, which sends
    /// every operation down the general path.
    #[cfg(test)]
    fn forced_big(&self) -> Dec {
        Dec {
            neg: self.neg,
            exp: self.exp,
            mag: Mag::Big(self.digits().into()),
        }
    }
}

/// `x / y · 10^exp` rounded to 34 digits, half to even, for `y < 10^37`.
fn div_small(neg: bool, x: u128, y: u128, mut exp: i64) -> Option<Dec> {
    let mut q = x / y;
    let mut r = x % y;
    // `q < 10^35` keeps `q * 10 + 9` and `r * 10 < 10 y` inside a `u128`
    while r != 0 && q < POW10[35] {
        r *= 10;
        q = q * 10 + r / y;
        r %= y;
        exp -= 1;
    }
    let (q, exp) = round_u128(q, exp, PRECISION, r != 0, Mode::HalfEven);
    Dec::small(neg, q, exp)
}

/// `x / y · 10^exp` rounded to 34 digits, half to even, for an inline `x`
/// and `y < 10^37`.
fn div_wide(neg: bool, x: U256, y: u128, exp: i64) -> Option<Dec> {
    let (q, r) = x.divrem(U256::from_u128(y));
    let r = r.to_u128()?;
    match q.to_u128() {
        Some(q) if q < POW10[35] => {
            // the loop of `div_small`, from the first quotient on
            let (mut q, mut r, mut exp) = (q, r, exp);
            while r != 0 && q < POW10[35] {
                r *= 10;
                q = q * 10 + r / y;
                r %= y;
                exp -= 1;
            }
            let (q, exp) = round_u128(q, exp, PRECISION, r != 0, Mode::HalfEven);
            Dec::small(neg, q, exp)
        }
        // 36 digits or more: round them
        _ => {
            let (q, exp) = round_u256(q, exp, PRECISION, r != 0)?;
            Dec::wide(neg, q, exp)
        }
    }
}

/// [`round_u128`] half to even of a 256-bit coefficient.
fn round_u256(q: U256, exp: i64, keep: usize, sticky: bool) -> Option<(U256, i64)> {
    let len = q.digit_count();
    if len <= keep {
        return Some((q, exp));
    }
    let drop = len - keep;
    let p = U256::pow10(drop)?;
    let (kept, rest) = q.divrem(p);
    let half = p.divrem_u64(2).0;
    let up = match rest.cmp(&half) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => sticky || kept.divrem_u64(2).1 == 1,
    };
    let kept = if up {
        kept.checked_add(U256::from_u128(1))?
    } else {
        kept
    };
    Some((kept, exp + drop as i64))
}

/// `q · 10^exp` (plus a nonzero tail when `sticky`) rounded to `keep`
/// significant digits.
fn round_u128(q: u128, exp: i64, keep: usize, sticky: bool, mode: Mode) -> (u128, i64) {
    let len = digit_count(q);
    if len <= keep {
        return (q, exp);
    }
    let drop = len - keep;
    let p = POW10[drop];
    let (kept, rest) = (q / p, q % p);
    let half = p / 2;
    let up = match rest.cmp(&half) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            sticky
                || match mode {
                    Mode::HalfUp => true,
                    Mode::HalfEven => kept % 2 == 1,
                }
        }
    };
    (kept + u128::from(up), exp + drop as i64)
}

/// A decimal rounded to `keep` digits, half to even.
fn round_dec(d: Dec, keep: usize) -> Option<Dec> {
    match d.mag {
        Mag::Small(c) => {
            let (c, exp) = round_u128(c, d.exp, keep, false, Mode::HalfEven);
            Dec::small(d.neg, c, exp)
        }
        Mag::Wide(c) => {
            let (c, exp) = round_u256(c, d.exp, keep, false)?;
            Dec::wide(d.neg, c, exp)
        }
        Mag::Big(_) => {
            let digits = d.digits();
            round_raw(
                Raw {
                    neg: d.neg,
                    digits,
                    exp: d.exp,
                },
                keep,
                false,
                Mode::HalfEven,
            )
        }
    }
}

/// An intermediate rounded to `keep` digits, half to even, still raw.
fn round_raw_to(raw: Raw, keep: usize) -> Option<Raw> {
    let d = round_raw(raw, keep, false, Mode::HalfEven)?;
    Some(Raw {
        neg: d.neg,
        digits: d.digits(),
        exp: d.exp,
    })
}

/// `sign · digits · 10^exp` (plus a nonzero tail below the last digit when
/// `sticky`) rounded to `keep` significant digits.
fn round_raw(raw: Raw, keep: usize, sticky: bool, mode: Mode) -> Option<Dec> {
    let Raw {
        neg,
        mut digits,
        exp,
    } = raw;
    while digits.last() == Some(&0) {
        digits.pop();
    }
    let len = digits.len();
    if len <= keep {
        // a tail below the digits only counts when there is room for it,
        // which never happens: callers give at least `keep + 2` digits
        return Dec::from_raw(Raw { neg, digits, exp });
    }
    let drop = len - keep;
    let first = digits[drop - 1];
    let rest_nonzero = sticky || digits[..drop - 1].iter().any(|&d| d != 0);
    let mut kept = digits[drop..].to_vec();
    let up = match first.cmp(&5) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            rest_nonzero
                || match mode {
                    Mode::HalfUp => true,
                    Mode::HalfEven => kept.first().is_some_and(|d| d % 2 == 1),
                }
        }
    };
    if up {
        if kept.is_empty() {
            kept.push(0);
        }
        add_small(&mut kept, 1);
    }
    Dec::from_raw(Raw {
        neg,
        digits: kept,
        exp: exp + drop as i64,
    })
}

/// The powers of ten that are exact doubles, `10^0` to `10^22`.
const EXACT_POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// The nearest double to `c · 10^exp`, without allocating.
pub(crate) fn to_f64_parts(c: u128, exp: i64) -> f64 {
    // both operands exact and one rounding: correctly rounded
    if c < 1 << 53 && (-22..=22).contains(&exp) {
        let p = EXACT_POW10[exp.unsigned_abs() as usize];
        if exp >= 0 {
            c as f64 * p
        } else {
            c as f64 / p
        }
    } else if c == 0 {
        0.0
    } else {
        small_to_f64(c, exp)
    }
}

/// The nearest double to `c · 10^exp`, through a decimal string on the
/// stack.
fn small_to_f64(mut c: u128, exp: i64) -> f64 {
    let mut digits = [0u8; 39];
    let mut n = 0;
    while c > 0 {
        digits[n] = (c % 10) as u8;
        c /= 10;
        n += 1;
    }
    digits_to_f64(&digits[..n], exp)
}

/// The nearest double to `digits · 10^exp` for at most 78 digits, least
/// significant first, through a decimal string on the stack.
fn digits_to_f64(digits: &[u8], exp: i64) -> f64 {
    let mut buf = [0u8; 104];
    let mut len = 0;
    for &d in digits.iter().rev() {
        buf[len] = b'0' + d;
        len += 1;
    }
    buf[len] = b'e';
    len += 1;
    if exp < 0 {
        buf[len] = b'-';
        len += 1;
    }
    let mut e = exp.unsigned_abs();
    let mut edigits = [0u8; 20];
    let mut m = 0;
    loop {
        edigits[m] = b'0' + (e % 10) as u8;
        e /= 10;
        m += 1;
        if e == 0 {
            break;
        }
    }
    for &d in edigits[..m].iter().rev() {
        buf[len] = d;
        len += 1;
    }
    std::str::from_utf8(&buf[..len])
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::NAN)
}

/// `digits · 10^by`.
fn shifted(digits: &[u8], by: usize) -> Vec<u8> {
    let mut out = vec![0; by];
    out.extend_from_slice(digits);
    out
}

fn mul_small(digits: &mut Vec<u8>, k: u8) {
    let mut carry = 0u16;
    for d in digits.iter_mut() {
        let v = u16::from(*d) * u16::from(k) + carry;
        *d = (v % 10) as u8;
        carry = v / 10;
    }
    while carry > 0 {
        digits.push((carry % 10) as u8);
        carry /= 10;
    }
}

fn add_small(digits: &mut Vec<u8>, k: u8) {
    let mut carry = k;
    for d in digits.iter_mut() {
        if carry == 0 {
            return;
        }
        let v = *d + carry;
        *d = v % 10;
        carry = v / 10;
    }
    if carry > 0 {
        digits.push(carry);
    }
}

fn cmp_mag(a: &[u8], b: &[u8]) -> Ordering {
    let (la, lb) = (significant(a), significant(b));
    la.cmp(&lb)
        .then_with(|| a[..la].iter().rev().cmp(b[..lb].iter().rev()))
}

fn significant(a: &[u8]) -> usize {
    a.len() - a.iter().rev().take_while(|&&d| d == 0).count()
}

fn add_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0;
    for i in 0..a.len().max(b.len()) {
        let v = a.get(i).copied().unwrap_or(0) + b.get(i).copied().unwrap_or(0) + carry;
        out.push(v % 10);
        carry = v / 10;
    }
    if carry > 0 {
        out.push(carry);
    }
    out
}

/// `a - b` for `a >= b`.
fn sub_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i8;
    for (i, &d) in a.iter().enumerate() {
        let mut v = d as i8 - b.get(i).copied().unwrap_or(0) as i8 - borrow;
        borrow = 0;
        if v < 0 {
            v += 10;
            borrow = 1;
        }
        out.push(v as u8);
    }
    out
}

fn mul_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut acc = vec![0u32; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        if x == 0 {
            continue;
        }
        for (j, &y) in b.iter().enumerate() {
            acc[i + j] += u32::from(x) * u32::from(y);
        }
        // keep the cells small
        if i % 64 == 63 {
            carry_all(&mut acc);
        }
    }
    carry_all(&mut acc);
    acc.into_iter().map(|v| v as u8).collect()
}

fn carry_all(acc: &mut Vec<u32>) {
    let mut carry = 0u32;
    for v in acc.iter_mut() {
        let t = *v + carry;
        *v = t % 10;
        carry = t / 10;
    }
    while carry > 0 {
        acc.push(carry % 10);
        carry /= 10;
    }
}

/// Quotient and remainder of `a / b`, digit by digit; `b` is not zero.
fn div_mag(a: &[u8], b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let b = &b[..significant(b)];
    let mut q = vec![0u8; a.len()];
    let mut r: Vec<u8> = Vec::new();
    for i in (0..a.len()).rev() {
        r.insert(0, a[i]);
        let mut n = 0;
        while cmp_mag(&r, b) != Ordering::Less {
            r = sub_mag(&r, b);
            r.truncate(significant(&r));
            n += 1;
        }
        q[i] = n;
        r.truncate(significant(&r));
    }
    q.truncate(significant(&q));
    (q, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::parse(s).unwrap()
    }

    #[test]
    fn literals_and_doubles() {
        assert_eq!(d("0.10"), d("1E-1"));
        assert_eq!(d(".5"), d("5.").div(&d("10")).unwrap());
        assert_eq!(d("-0"), Dec::zero());
        assert_eq!(d("1.5E-3").to_f64(), 0.0015);
        assert!(Dec::parse("1.5E\u{2212}3").is_none());
        assert!(Dec::parse("1.2.3").is_none() && Dec::parse("E3").is_none());
        assert_eq!(
            Dec::from_f64(0.1).unwrap(),
            d("0.1000000000000000055511151231257827021181583404541015625")
        );
        assert_eq!(Dec::from_f64(-3.0).unwrap(), d("-3"));
        assert_eq!(Dec::from_f64(0.375).unwrap(), d("0.375"));
        assert_eq!(
            Dec::from_f64(2f64.powi(70)).unwrap(),
            d("1180591620717411303424")
        );
        for x in [
            1e300,
            5e-324,
            2.2250738585072014e-308,
            0.1,
            123.456,
            2f64.powi(100),
        ] {
            assert_eq!(Dec::from_f64(x).unwrap().to_f64(), x);
            assert_eq!(Dec::from_f64(-x).unwrap().to_f64(), -x);
        }
        assert_eq!(d("1E-400").to_f64(), 0.0);
        assert_eq!(d("1E400").to_f64(), f64::INFINITY);
        // halfway between two doubles: ties to even
        assert_eq!(d("9007199254740993").to_f64(), 9007199254740992.0);
        assert_eq!(
            d("1.00000000000000011102230246251565404236316680908203125").to_f64(),
            1.0
        );
    }

    #[test]
    fn exact_operations() {
        assert_eq!(d("0.1").add(&d("0.2")).unwrap(), d("0.3"));
        assert_eq!(
            d("0.3").sub(&d("0.1")).unwrap().sub(&d("0.2")).unwrap(),
            Dec::zero()
        );
        assert_eq!(d("1.1").mul(&d("1.1")).unwrap(), d("1.21"));
        assert_eq!(d("-2.5").mul(&d("4")).unwrap(), d("-10"));
        assert_eq!(d("1").sub(&d("3")).unwrap(), d("-2"));
        // past the inline size and back
        let big = d("12345678901234567890123456789012345678901234567890");
        assert_eq!(big.sub(&big).unwrap(), Dec::zero());
        assert_eq!(big.add(&d("1")).unwrap().sub(&big).unwrap(), d("1"));
    }

    #[test]
    fn division_has_34_digits_half_even() {
        assert_eq!(
            d("1").div(&d("3")).unwrap(),
            d("0.3333333333333333333333333333333333")
        );
        assert_eq!(
            d("2").div(&d("3")).unwrap(),
            d("0.6666666666666666666666666666666667")
        );
        assert_eq!(d("1").div(&d("8")).unwrap(), d("0.125"));
        assert_eq!(
            d("-1").div(&d("7")).unwrap(),
            d("-0.1428571428571428571428571428571429")
        );
        // a tie at the 35th digit goes to the even neighbour
        let tie = d("1000000000000000000000000000000000.5");
        assert_eq!(
            tie.div(&d("1")).unwrap(),
            d("1000000000000000000000000000000000")
        );
        let tie = d("1000000000000000000000000000000001.5");
        assert_eq!(
            tie.div(&d("1")).unwrap(),
            d("1000000000000000000000000000000002")
        );
        assert!(d("1").div(&Dec::zero()).is_none());
        // a long dividend: only its leading digits and whether the rest is zero matter
        let long = d(&format!("1{}1", "0".repeat(300)));
        assert_eq!(long.div(&d("3")).unwrap().to_f64(), 1e301 / 3.0);
    }

    #[test]
    fn decimal_digits_count_like_to_string() {
        for n in [0, 1, 9, 10, 99, 100, 999_999, 1_000_000, u32::MAX] {
            assert_eq!(decimal_digits(n), n.to_string().len(), "{n}");
        }
    }

    #[test]
    fn powers_and_scales() {
        assert_eq!(d("0.1").pow(3).unwrap(), d("0.001"));
        assert_eq!(d("1.1").pow(2).unwrap(), d("1.21"));
        assert_eq!(d("7").pow(0).unwrap(), d("1"));
        assert_eq!(d("-2").pow(3).unwrap(), d("-8"));
        assert_eq!(d("2").pow(200).unwrap().to_f64(), 2f64.powi(200));
        // a base longer than the working precision still gives 34 digits
        let long = d(&format!("1.{}1", "0".repeat(900)));
        assert_eq!(long.pow(3).unwrap(), d("1"));
        assert_eq!(d("1.005").set_scale_half_up(2).unwrap(), d("1.01"));
        assert_eq!(d("-2.5").set_scale_half_up(0).unwrap(), d("-3"));
        assert_eq!(d("2.4").set_scale_half_up(0).unwrap(), d("2"));
        assert_eq!(d("0.04").set_scale_half_up(0).unwrap(), Dec::zero());
        assert_eq!(d("0.5").set_scale_half_up(0).unwrap(), d("1"));
        assert_eq!(d("123").set_scale_half_up(-1).unwrap(), d("120"));
        assert_eq!(d("2.567").set_scale_half_up(400).unwrap(), d("2.567"));
        assert_eq!(d("2.567").set_scale_half_up(-400).unwrap(), Dec::zero());
    }

    #[test]
    fn work_is_bounded() {
        // aligning `1E-999999999` with 1 needs a billion digits
        assert!(d("1E-999999999").add(&d("1")).is_none());
        // the exponent stays in the range of Java's `int` scale
        assert!(d("1E-999999999").pow(3).is_none());
        let big = d(&"7".repeat(5000));
        assert!(big.mul(&big).is_none());
        // a chain through 5000 digits comes back to a small value
        let tiny = d("1E-5001");
        let back = tiny.add(&d("1")).unwrap().sub(&d("1")).unwrap();
        assert_eq!(back, tiny);
    }

    /// A random decimal of up to 77 digits from a linear congruential
    /// generator: inline in a `u128` or in 256 bits.
    fn random(state: &mut u64) -> Dec {
        let mut next = || {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state >> 11
        };
        let digits = (next() % 78) as usize;
        let mut coef = U256::ZERO;
        for _ in 0..4 {
            coef = coef
                .checked_mul(U256::from_u128(1 << 64))
                .unwrap_or(coef)
                .checked_add(U256::from_u128(
                    u128::from(next()) << 11 | u128::from(next() % 2048),
                ))
                .unwrap_or(coef);
        }
        let coef = coef.divrem(U256::pow10(digits).unwrap()).1;
        let exp = (next() % 81) as i64 - 40;
        Dec::wide(next() % 2 == 0, coef, exp).unwrap()
    }

    /// A random finite double from all of its bit patterns.
    fn random_double(state: &mut u64) -> f64 {
        loop {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let x = f64::from_bits(*state ^ (*state >> 29));
            if x.is_finite() {
                return x;
            }
        }
    }

    /// The canonical form of a result (an operation that returns an
    /// operand unchanged keeps the forced digit form).
    fn canon(d: Option<Dec>) -> Option<Dec> {
        d.map(|d| {
            Dec::from_raw(Raw {
                neg: d.neg,
                digits: d.digits(),
                exp: d.exp,
            })
            .unwrap()
        })
    }

    #[test]
    fn the_inline_path_agrees_with_the_digit_path() {
        let mut state = 7;
        for _ in 0..5_000 {
            let (a, b) = (random(&mut state), random(&mut state));
            let (ab, bb) = (a.forced_big(), b.forced_big());
            let n = (state % 40) as u32;
            let scale = (state % 60) as i64 - 30;
            let pairs = [
                (a.add(&b), ab.add(&bb), "+"),
                (a.sub(&b), ab.sub(&bb), "-"),
                (a.mul(&b), ab.mul(&bb), "*"),
                (a.div(&b), ab.div(&bb), "/"),
                (a.pow(n), ab.pow(n), "^"),
                (
                    a.set_scale_half_up(scale),
                    ab.set_scale_half_up(scale),
                    "scale",
                ),
            ];
            for (small, big, op) in pairs {
                assert_eq!(small, canon(big), "{a:?} {op} {b:?} (n {n}, scale {scale})");
            }
            assert_eq!(a.to_f64(), ab.to_f64(), "{a:?}");
        }
    }

    #[test]
    fn the_exact_decimal_of_a_double_is_inline_up_to_77_digits() {
        let mut state = 11;
        for i in 0..20_000 {
            let mut x = random_double(&mut state);
            if i % 2 == 0 {
                // doubles of the usual sizes, not only the extreme exponents
                x = x.abs().log2().fract() * 1e3 * x.signum();
            }
            let d = Dec::from_f64(x).unwrap();
            let bits = x.to_bits();
            let raw = ((bits >> 52) & 0x7ff) as i64;
            let frac = bits & ((1 << 52) - 1);
            let (m, e) = if raw == 0 {
                (frac, -1074)
            } else {
                (frac | 1 << 52, raw - 1075)
            };
            if m == 0 {
                assert_eq!(d, Dec::zero());
                continue;
            }
            let z = m.trailing_zeros();
            let slow = Dec::from_f64_digits(bits >> 63 == 1, u128::from(m >> z), e + i64::from(z));
            assert_eq!(Some(d.clone()), canon(slow), "{x:e}");
            assert_eq!(matches!(d.mag, Mag::Big(_)), d.len() > 77, "{x:e}");
            assert_eq!(d.to_f64().to_bits(), x.to_bits(), "{x:e}");
        }
        // the window: a full mantissa down to 2^-35 and up to 2^256
        let full = 1.0 + f64::EPSILON;
        for (x, inline) in [
            (full * 2f64.powi(-35), true),
            (full * 2f64.powi(-36), false),
            (0.65, true),
            (1.3e-10, true),
            (2f64.powi(255) * full, true),
            (2f64.powi(256), false),
            (123.457 / 7.0, true),
        ] {
            let d = Dec::from_f64(x).unwrap();
            assert_eq!(
                !matches!(d.mag, Mag::Big(_)),
                inline,
                "{x:e}: {} digits",
                d.len()
            );
        }
    }
}
