// Licensed under the EUPL 1.2.

//! Signed integers of any size for exact polynomial coefficients.
//!
//! The magnitude is stored in little-endian 32-bit limbs without leading
//! zero limbs; zero has no limbs and is never negative. Division follows
//! Knuth, The Art of Computer Programming, vol. 2, section 4.3.1,
//! algorithm D.

use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Int {
    negative: bool,
    limbs: Vec<u32>,
}

impl Int {
    pub(crate) fn zero() -> Self {
        Self::default()
    }

    pub(crate) fn from_i64(value: i64) -> Self {
        let mut magnitude = value.unsigned_abs();
        let mut limbs = Vec::new();
        while magnitude != 0 {
            limbs.push(magnitude as u32);
            magnitude >>= 32;
        }
        Self {
            negative: value < 0,
            limbs,
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub(crate) fn is_negative(&self) -> bool {
        self.negative
    }

    pub(crate) fn is_unit(&self) -> bool {
        self.limbs == [1]
    }

    /// The number of limbs of the magnitude, a measure of its size.
    pub(crate) fn size(&self) -> usize {
        self.limbs.len()
    }

    /// The value when it fits in an `i128`.
    pub(crate) fn to_i128(&self) -> Option<i128> {
        if self.limbs.len() > 4 {
            return None;
        }
        let magnitude = self
            .limbs
            .iter()
            .rev()
            .fold(0u128, |acc, &limb| (acc << 32) | u128::from(limb));
        let magnitude = i128::try_from(magnitude).ok()?;
        Some(if self.negative { -magnitude } else { magnitude })
    }

    pub(crate) fn neg(&self) -> Self {
        let mut result = self.clone();
        result.negative = !result.negative && !result.is_zero();
        result
    }

    pub(crate) fn abs(&self) -> Self {
        Self {
            negative: false,
            limbs: self.limbs.clone(),
        }
    }

    fn from_parts(negative: bool, mut limbs: Vec<u32>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        let negative = negative && !limbs.is_empty();
        Self { negative, limbs }
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        if self.negative == other.negative {
            return Self::from_parts(self.negative, add_magnitudes(&self.limbs, &other.limbs));
        }
        match compare_magnitudes(&self.limbs, &other.limbs) {
            Ordering::Equal => Self::zero(),
            Ordering::Greater => {
                Self::from_parts(self.negative, sub_magnitudes(&self.limbs, &other.limbs))
            }
            Ordering::Less => {
                Self::from_parts(other.negative, sub_magnitudes(&other.limbs, &self.limbs))
            }
        }
    }

    pub(crate) fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.limbs.iter().enumerate() {
                let t = u64::from(a) * u64::from(b) + u64::from(limbs[i + j]) + carry;
                limbs[i + j] = t as u32;
                carry = t >> 32;
            }
            let mut k = i + other.limbs.len();
            while carry != 0 {
                let t = u64::from(limbs[k]) + carry;
                limbs[k] = t as u32;
                carry = t >> 32;
                k += 1;
            }
        }
        Self::from_parts(self.negative != other.negative, limbs)
    }

    /// Truncating division: the quotient rounds toward zero and the
    /// remainder has the sign of the dividend. `None` for a zero divisor.
    pub(crate) fn div_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        if divisor.is_zero() {
            return None;
        }
        if compare_magnitudes(&self.limbs, &divisor.limbs) == Ordering::Less {
            return Some((Self::zero(), self.clone()));
        }
        let (quotient, remainder) = div_rem_magnitudes(&self.limbs, &divisor.limbs);
        Some((
            Self::from_parts(self.negative != divisor.negative, quotient),
            Self::from_parts(self.negative, remainder),
        ))
    }

    /// The quotient of a division known to be exact.
    pub(crate) fn div_exact(&self, divisor: &Self) -> Option<Self> {
        let (quotient, remainder) = self.div_rem(divisor)?;
        remainder.is_zero().then_some(quotient)
    }

    /// The non-negative greatest common divisor.
    pub(crate) fn gcd(&self, other: &Self) -> Self {
        let mut a = self.abs();
        let mut b = other.abs();
        while !b.is_zero() {
            let (_, r) = a.div_rem(&b).expect("nonzero divisor");
            a = b;
            b = r;
        }
        a
    }

    pub(crate) fn cmp_abs(&self, other: &Self) -> Ordering {
        compare_magnitudes(&self.limbs, &other.limbs)
    }

    /// The square root of a perfect square, `None` otherwise or for a
    /// negative number.
    pub(crate) fn exact_sqrt(&self) -> Option<Self> {
        if self.negative {
            return None;
        }
        if self.is_zero() {
            return Some(Self::zero());
        }
        // Newton's iteration from a power of two above the root.
        let bits = self.limbs.len() * 32;
        let mut x = Self::from_parts(false, {
            let mut limbs = vec![0u32; bits / 64 + 2];
            let top = bits.div_ceil(2) + 1;
            limbs[top / 32] |= 1 << (top % 32);
            limbs
        });
        let two = Self::from_i64(2);
        loop {
            let (q, _) = self.div_rem(&x)?;
            let (next, _) = x.add(&q).div_rem(&two)?;
            if next.cmp_abs(&x) != Ordering::Less {
                break;
            }
            x = next;
        }
        (x.mul(&x) == *self).then_some(x)
    }
}

impl std::fmt::Display for Int {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }
        let mut digits = Vec::new();
        let mut rest = self.abs();
        let base = Self::from_i64(1_000_000_000);
        while !rest.is_zero() {
            let (q, r) = rest.div_rem(&base).expect("nonzero divisor");
            digits.push(r.to_i128().unwrap_or(0));
            rest = q;
        }
        if self.negative {
            f.write_str("-")?;
        }
        write!(f, "{}", digits.last().copied().unwrap_or(0))?;
        for d in digits.iter().rev().skip(1) {
            write!(f, "{d:09}")?;
        }
        Ok(())
    }
}

fn compare_magnitudes(a: &[u32], b: &[u32]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_magnitudes(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut result = Vec::with_capacity(long.len() + 1);
    let mut carry = 0u64;
    for (i, &x) in long.iter().enumerate() {
        let t = u64::from(x) + u64::from(short.get(i).copied().unwrap_or(0)) + carry;
        result.push(t as u32);
        carry = t >> 32;
    }
    if carry != 0 {
        result.push(carry as u32);
    }
    result
}

/// `a - b` for `a >= b`.
fn sub_magnitudes(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut result = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for (i, &x) in a.iter().enumerate() {
        let mut t = i64::from(x) - i64::from(b.get(i).copied().unwrap_or(0)) - borrow;
        borrow = 0;
        if t < 0 {
            t += 1 << 32;
            borrow = 1;
        }
        result.push(t as u32);
    }
    result
}

fn div_rem_magnitudes(u: &[u32], v: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if v.len() == 1 {
        let d = u64::from(v[0]);
        let mut quotient = vec![0u32; u.len()];
        let mut r = 0u64;
        for i in (0..u.len()).rev() {
            let t = (r << 32) | u64::from(u[i]);
            quotient[i] = (t / d) as u32;
            r = t % d;
        }
        return (quotient, vec![r as u32]);
    }
    // Normalize so that the top limb of the divisor has its high bit set.
    let shift = v[v.len() - 1].leading_zeros();
    let v = shift_left(v, shift);
    let mut u = shift_left(u, shift);
    u.push(0);
    let n = v.len();
    let m = u.len() - n - 1;
    let mut quotient = vec![0u32; m + 1];
    let b = 1u64 << 32;
    for j in (0..=m).rev() {
        let numerator = (u64::from(u[j + n]) << 32) | u64::from(u[j + n - 1]);
        let mut qhat = numerator / u64::from(v[n - 1]);
        let mut rhat = numerator % u64::from(v[n - 1]);
        while qhat >= b || qhat * u64::from(v[n - 2]) > ((rhat << 32) | u64::from(u[j + n - 2])) {
            qhat -= 1;
            rhat += u64::from(v[n - 1]);
            if rhat >= b {
                break;
            }
        }
        // Multiply and subtract.
        let mut borrow = 0i64;
        let mut carry = 0u64;
        for i in 0..n {
            let p = qhat * u64::from(v[i]) + carry;
            carry = p >> 32;
            let t = i64::from(u[i + j]) - borrow - (p & 0xffff_ffff) as i64;
            u[i + j] = t as u32;
            borrow = i64::from(t < 0);
        }
        let t = i64::from(u[j + n]) - borrow - carry as i64;
        u[j + n] = t as u32;
        if t < 0 {
            // Add back.
            qhat -= 1;
            let mut carry = 0u64;
            for i in 0..n {
                let t = u64::from(u[i + j]) + u64::from(v[i]) + carry;
                u[i + j] = t as u32;
                carry = t >> 32;
            }
            u[j + n] = u[j + n].wrapping_add(carry as u32);
        }
        quotient[j] = qhat as u32;
    }
    u.truncate(n);
    let remainder = shift_right(&u, shift);
    (quotient, remainder)
}

fn shift_left(a: &[u32], shift: u32) -> Vec<u32> {
    if shift == 0 {
        return a.to_vec();
    }
    let mut result = Vec::with_capacity(a.len() + 1);
    let mut carry = 0u32;
    for &x in a {
        result.push((x << shift) | carry);
        carry = x >> (32 - shift);
    }
    if carry != 0 {
        result.push(carry);
    }
    result
}

fn shift_right(a: &[u32], shift: u32) -> Vec<u32> {
    if shift == 0 {
        return a.to_vec();
    }
    let mut result = vec![0u32; a.len()];
    for i in 0..a.len() {
        let high = a.get(i + 1).map_or(0, |&x| x << (32 - shift));
        result[i] = (a[i] >> shift) | high;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::Int;

    fn big(digits: &str) -> Int {
        let ten = Int::from_i64(10);
        let negative = digits.starts_with('-');
        let mut value = Int::zero();
        for d in digits.trim_start_matches('-').bytes() {
            value = value.mul(&ten).add(&Int::from_i64(i64::from(d - b'0')));
        }
        if negative {
            value.neg()
        } else {
            value
        }
    }

    #[test]
    fn arithmetic_round_trips() {
        let a = big("123456789012345678901234567890123");
        let b = big("-98765432109876543210987");
        let product = a.mul(&b);
        assert_eq!(
            product.to_string(),
            "-12193263113702179522618422493004797134336296860222381401"
        );
        assert_eq!(product.div_exact(&b), Some(a.clone()));
        let (q, r) = a.div_rem(&b).unwrap();
        assert_eq!(q.mul(&b).add(&r), a);
        assert!(r.cmp_abs(&b).is_lt());
        assert_eq!(a.sub(&a), Int::zero());
        assert_eq!(
            big("-7").div_rem(&Int::from_i64(2)).unwrap().1,
            Int::from_i64(-1)
        );
    }

    #[test]
    fn gcd_and_square_roots() {
        let a = big("1071").mul(&big("1000000000000000000000"));
        let b = big("462").mul(&big("1000000000000000000000"));
        assert_eq!(a.gcd(&b).to_string(), "21000000000000000000000");
        let root = big("31415926535897932384626433");
        assert_eq!(root.mul(&root).exact_sqrt(), Some(root.clone()));
        assert_eq!(root.mul(&root).add(&Int::from_i64(1)).exact_sqrt(), None);
        assert_eq!(Int::from_i64(-4).exact_sqrt(), None);
        assert_eq!(Int::from_i64(49).exact_sqrt(), Some(Int::from_i64(7)));
    }

    #[test]
    fn division_needing_the_add_back_step() {
        // Divisors whose top limbs make the trial quotient one too large.
        let u = Int::from_parts(false, vec![0, 0, 0x8000_0000, 0x7fff_ffff]);
        let v = Int::from_parts(false, vec![1, 0, 0x8000_0000]);
        let (q, r) = u.div_rem(&v).unwrap();
        assert_eq!(q.mul(&v).add(&r), u);
        assert!(r.cmp_abs(&v).is_lt());
    }
}
