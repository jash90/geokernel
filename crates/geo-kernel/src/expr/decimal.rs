//! Exact decimal numbers, for the arithmetic GeoGebra does on decimal
//! literals with Java's `BigDecimal` (see `DoubleOperation`): addition,
//! subtraction and multiplication are exact; division and whole powers are
//! rounded to 34 significant digits, half to even (`MathContext.DECIMAL128`);
//! `round` uses `setScale(digits, HALF_UP)`.
//!
//! Written from the documented behaviour of those `BigDecimal` operations.
//! Only values matter here, never the scale a `BigDecimal` would carry, so a
//! number is kept in a canonical form. Sizes are capped: an operation whose
//! result would need more than [`MAX_DIGITS`] digits gives `None`, and the
//! caller falls back to `double` arithmetic (GeoGebra would carry on).

use std::cmp::Ordering;

/// Most digits a decimal may have, and the largest shift that aligning two
/// decimals for an addition may need.
pub(crate) const MAX_DIGITS: usize = 800;

/// Significant digits of `MathContext.DECIMAL128`.
const PRECISION: usize = 34;

/// `sign · digits · 10^exp`, with the digits least significant first, no
/// zero at either end; zero has no digits, exponent 0 and a plus sign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dec {
    neg: bool,
    digits: Vec<u8>,
    exp: i64,
}

impl Dec {
    fn zero() -> Dec {
        Dec {
            neg: false,
            digits: Vec::new(),
            exp: 0,
        }
    }

    fn one() -> Dec {
        Dec {
            neg: false,
            digits: vec![1],
            exp: 0,
        }
    }

    /// Normalises and checks the size.
    fn make(neg: bool, mut digits: Vec<u8>, mut exp: i64) -> Option<Dec> {
        while digits.last() == Some(&0) {
            digits.pop();
        }
        let low = digits.iter().take_while(|&&d| d == 0).count();
        digits.drain(..low);
        exp += low as i64;
        if digits.is_empty() {
            return Some(Dec::zero());
        }
        if digits.len() > MAX_DIGITS || exp.unsigned_abs() > 1 << 40 {
            return None;
        }
        Some(Dec { neg, digits, exp })
    }

    /// A decimal literal: `12`, `-0.5`, `.5`, `5.`, `1.5E-3` (the minus signs
    /// `–` and `−` are accepted in the exponent, as the lexer does).
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
                let (eneg, digits) = match e.chars().next() {
                    Some('-') => (true, &e[1..]),
                    Some('+') => (false, &e[1..]),
                    _ => (false, e),
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
        Dec::make(neg, digits, exp)
    }

    /// The exact value of a finite double (`new BigDecimal(double)`).
    pub(crate) fn from_f64(x: f64) -> Option<Dec> {
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
        while m & 1 == 0 {
            m >>= 1;
            e += 1;
        }
        let mut digits: Vec<u8> = m.to_string().bytes().rev().map(|b| b - b'0').collect();
        if e >= 0 {
            for _ in 0..e {
                mul_small(&mut digits, 2);
            }
            Dec::make(neg, digits, 0)
        } else {
            // m · 2^e = m · 5^-e · 10^e
            for _ in 0..-e {
                mul_small(&mut digits, 5);
            }
            Dec::make(neg, digits, e)
        }
    }

    /// The nearest double (`BigDecimal.doubleValue`).
    pub(crate) fn to_f64(&self) -> f64 {
        if self.digits.is_empty() {
            return 0.0;
        }
        let mut s = String::with_capacity(self.digits.len() + 24);
        if self.neg {
            s.push('-');
        }
        s.extend(self.digits.iter().rev().map(|&d| char::from(b'0' + d)));
        s.push('e');
        s.push_str(&self.exp.to_string());
        s.parse().unwrap_or(f64::NAN)
    }

    fn is_zero(&self) -> bool {
        self.digits.is_empty()
    }

    pub(crate) fn add(&self, other: &Dec) -> Option<Dec> {
        if self.is_zero() {
            return Some(other.clone());
        }
        if other.is_zero() {
            return Some(self.clone());
        }
        let exp = self.exp.min(other.exp);
        let a = shifted(&self.digits, self.exp - exp)?;
        let b = shifted(&other.digits, other.exp - exp)?;
        if self.neg == other.neg {
            return Dec::make(self.neg, add_mag(&a, &b), exp);
        }
        match cmp_mag(&a, &b) {
            Ordering::Equal => Some(Dec::zero()),
            Ordering::Greater => Dec::make(self.neg, sub_mag(&a, &b), exp),
            Ordering::Less => Dec::make(other.neg, sub_mag(&b, &a), exp),
        }
    }

    pub(crate) fn sub(&self, other: &Dec) -> Option<Dec> {
        self.add(&other.negated())
    }

    pub(crate) fn negated(&self) -> Dec {
        let mut d = self.clone();
        d.neg = !d.neg && !d.is_zero();
        d
    }

    pub(crate) fn mul(&self, other: &Dec) -> Option<Dec> {
        if self.is_zero() || other.is_zero() {
            return Some(Dec::zero());
        }
        if self.digits.len() + other.digits.len() > MAX_DIGITS + 1 {
            return None;
        }
        Dec::make(
            self.neg != other.neg,
            mul_mag(&self.digits, &other.digits),
            self.exp + other.exp,
        )
    }

    /// `divide(other, DECIMAL128)`; `None` for a zero divisor.
    pub(crate) fn div(&self, other: &Dec) -> Option<Dec> {
        if other.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some(Dec::zero());
        }
        // enough digits for the rounding: at least PRECISION + 2
        let shift = (PRECISION + 2 + other.digits.len()).saturating_sub(self.digits.len());
        let num = shifted(&self.digits, shift as i64)?;
        let (q, r) = div_mag(&num, &other.digits);
        let neg = self.neg != other.neg;
        let exp = self.exp - other.exp - shift as i64;
        round_digits(neg, q, exp, PRECISION, !r.is_empty(), Mode::HalfEven)
    }

    /// `pow(n, DECIMAL128)` for `0 <= n < 1e6`: repeated squaring into one
    /// accumulator, each product rounded to `34 + digits(n) + 1` digits, and
    /// the result rounded to 34 digits (the algorithm `BigDecimal.pow`
    /// documents, from ANSI X3.274-1996).
    pub(crate) fn pow(&self, n: u32) -> Option<Dec> {
        if n == 0 {
            return Some(Dec::one());
        }
        let work = PRECISION + n.to_string().len() + 1;
        let mut acc = Dec::one();
        let mut seen = false;
        for bit in (0..32).rev() {
            if seen {
                acc = acc.mul(&acc)?.rounded(work, Mode::HalfEven)?;
            }
            if (n >> bit) & 1 == 1 {
                acc = acc.mul(self)?.rounded(work, Mode::HalfEven)?;
                seen = true;
            }
        }
        acc.rounded(PRECISION, Mode::HalfEven)
    }

    /// `setScale(scale, HALF_UP)`: rounded to `scale` decimal places.
    pub(crate) fn set_scale_half_up(&self, scale: i64) -> Option<Dec> {
        let drop = -scale - self.exp;
        if drop <= 0 {
            return Some(self.clone());
        }
        let len = self.digits.len() as i64;
        if drop > len {
            // every digit goes and the first dropped one is a zero
            return Some(Dec::zero());
        }
        let keep = (len - drop) as usize;
        round_digits(
            self.neg,
            self.digits.clone(),
            self.exp,
            keep,
            false,
            Mode::HalfUp,
        )
    }

    fn rounded(&self, precision: usize, mode: Mode) -> Option<Dec> {
        round_digits(
            self.neg,
            self.digits.clone(),
            self.exp,
            precision,
            false,
            mode,
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    HalfEven,
    HalfUp,
}

/// `sign · digits · 10^exp` (plus a nonzero tail below the last digit when
/// `sticky`) rounded to `keep` significant digits.
fn round_digits(
    neg: bool,
    digits: Vec<u8>,
    exp: i64,
    keep: usize,
    sticky: bool,
    mode: Mode,
) -> Option<Dec> {
    let len = digits.len();
    if len <= keep {
        // a tail below the digits only counts when there is room for it,
        // which never happens: callers give at least `keep + 2` digits
        return Dec::make(neg, digits, exp);
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
    Dec::make(neg, kept, exp + drop as i64)
}

/// `digits · 10^by`; `None` when that is too long.
fn shifted(digits: &[u8], by: i64) -> Option<Vec<u8>> {
    if by < 0 || digits.len() as i64 + by > (2 * MAX_DIGITS) as i64 {
        return None;
    }
    let mut out = vec![0; by as usize];
    out.extend_from_slice(digits);
    Some(out)
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
        assert_eq!(Dec::from_f64(1e300).unwrap().to_f64(), 1e300);
        assert_eq!(Dec::from_f64(5e-324).unwrap().to_f64(), 5e-324);
        assert_eq!(d("1E-400").to_f64(), 0.0);
        assert_eq!(d("1E400").to_f64(), f64::INFINITY);
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
    }

    #[test]
    fn powers_and_scales() {
        assert_eq!(d("0.1").pow(3).unwrap(), d("0.001"));
        assert_eq!(d("1.1").pow(2).unwrap(), d("1.21"));
        assert_eq!(d("7").pow(0).unwrap(), d("1"));
        assert_eq!(d("2").pow(200).unwrap().to_f64(), 2f64.powi(200));
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
    fn sizes_are_capped() {
        assert!(d("1E-1000000").add(&d("1")).is_none());
        let big = d(&"7".repeat(MAX_DIGITS));
        assert!(big.mul(&big).is_none());
    }
}
