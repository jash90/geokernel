//! A 256-bit unsigned integer, the coefficient of an exact decimal of 39 to
//! 77 digits (see [`super::decimal`]). Only what the decimals need: checked
//! addition and multiplication, subtraction, division, powers of ten and
//! five, and the decimal digits. Nothing allocates.

use std::cmp::Ordering;

/// Little-endian 64-bit limbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct U256([u64; 4]);

/// Digits that always fit (`10^77 < 2^256`).
pub(super) const DIGITS: usize = 77;

const fn mul_small(x: [u64; 4], k: u64) -> [u64; 4] {
    let mut out = [0u64; 4];
    let mut carry = 0u128;
    let mut i = 0;
    while i < 4 {
        let v = x[i] as u128 * k as u128 + carry;
        out[i] = v as u64;
        carry = v >> 64;
        i += 1;
    }
    out
}

/// `10^k` for `k <= 77`.
const POW10: [U256; 78] = {
    let mut t = [U256([1, 0, 0, 0]); 78];
    let mut i = 1;
    while i < 78 {
        t[i] = U256(mul_small(t[i - 1].0, 10));
        i += 1;
    }
    t
};

/// `5^k` for `k <= 110` (`5^110 < 2^256`).
const POW5: [U256; 111] = {
    let mut t = [U256([1, 0, 0, 0]); 111];
    let mut i = 1;
    while i < 111 {
        t[i] = U256(mul_small(t[i - 1].0, 5));
        i += 1;
    }
    t
};

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl U256 {
    pub(super) const ZERO: U256 = U256([0; 4]);

    pub(super) fn from_u128(x: u128) -> U256 {
        U256([x as u64, (x >> 64) as u64, 0, 0])
    }

    pub(super) fn to_u128(self) -> Option<u128> {
        (self.0[2] == 0 && self.0[3] == 0)
            .then(|| u128::from(self.0[0]) | u128::from(self.0[1]) << 64)
    }

    pub(super) fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }

    /// `10^k`, `None` above 77.
    pub(super) fn pow10(k: usize) -> Option<U256> {
        POW10.get(k).copied()
    }

    /// `5^k`, `None` above 110.
    pub(super) fn pow5(k: usize) -> Option<U256> {
        POW5.get(k).copied()
    }

    /// `self · 2^bits`, `None` when it does not fit.
    pub(super) fn checked_shl(self, bits: u32) -> Option<U256> {
        if self.is_zero() {
            return Some(self);
        }
        if bits >= 256 || self.bits() + bits > 256 {
            return None;
        }
        let (limbs, rest) = ((bits / 64) as usize, bits % 64);
        let mut out = [0u64; 4];
        for i in (limbs..4).rev() {
            let low = self.0[i - limbs];
            out[i] = if rest == 0 {
                low
            } else {
                let below = if i > limbs {
                    self.0[i - limbs - 1] >> (64 - rest)
                } else {
                    0
                };
                low << rest | below
            };
        }
        Some(U256(out))
    }

    /// Number of significant bits.
    fn bits(self) -> u32 {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return 64 * i as u32 + (64 - self.0[i].leading_zeros());
            }
        }
        0
    }

    pub(super) fn checked_add(self, o: U256) -> Option<U256> {
        let mut out = [0u64; 4];
        let mut carry = false;
        for (i, slot) in out.iter_mut().enumerate() {
            let (s, c1) = self.0[i].overflowing_add(o.0[i]);
            let (s, c2) = s.overflowing_add(u64::from(carry));
            *slot = s;
            carry = c1 || c2;
        }
        (!carry).then_some(U256(out))
    }

    /// `self - o` for `self >= o`.
    pub(super) fn sub(self, o: U256) -> U256 {
        let mut out = [0u64; 4];
        let mut borrow = false;
        for (i, slot) in out.iter_mut().enumerate() {
            let (d, b1) = self.0[i].overflowing_sub(o.0[i]);
            let (d, b2) = d.overflowing_sub(u64::from(borrow));
            *slot = d;
            borrow = b1 || b2;
        }
        debug_assert!(!borrow, "a difference below zero");
        U256(out)
    }

    pub(super) fn checked_mul(self, o: U256) -> Option<U256> {
        let mut out = [0u64; 4];
        for i in 0..4 {
            if self.0[i] == 0 {
                continue;
            }
            let mut carry = 0u128;
            for j in 0..4 {
                let p = u128::from(self.0[i]) * u128::from(o.0[j]);
                if i + j >= 4 {
                    if p != 0 {
                        return None;
                    }
                    continue;
                }
                let v = u128::from(out[i + j]) + p + carry;
                out[i + j] = v as u64;
                carry = v >> 64;
            }
            if carry != 0 {
                return None;
            }
        }
        Some(U256(out))
    }

    /// Quotient and remainder by a nonzero `d` below `2^64`.
    pub(super) fn divrem_u64(self, d: u64) -> (U256, u64) {
        let mut out = [0u64; 4];
        let mut rem = 0u128;
        for i in (0..4).rev() {
            let cur = rem << 64 | u128::from(self.0[i]);
            out[i] = (cur / u128::from(d)) as u64;
            rem = cur % u128::from(d);
        }
        (U256(out), rem as u64)
    }

    /// Quotient and remainder by a nonzero `d`.
    pub(super) fn divrem(self, d: U256) -> (U256, U256) {
        if d.0[1] == 0 && d.0[2] == 0 && d.0[3] == 0 {
            let (q, r) = self.divrem_u64(d.0[0]);
            return (q, U256([r, 0, 0, 0]));
        }
        if self < d {
            return (U256::ZERO, self);
        }
        // shift and subtract, one bit at a time from the top of the quotient
        let shift = self.bits() - d.bits();
        let mut q = U256::ZERO;
        let mut r = self;
        let mut m = d.checked_shl(shift).unwrap_or(d);
        for bit in (0..=shift).rev() {
            if r >= m {
                r = r.sub(m);
                q.0[(bit / 64) as usize] |= 1 << (bit % 64);
            }
            m = m.shr1();
        }
        (q, r)
    }

    fn shr1(self) -> U256 {
        let mut out = [0u64; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            let high = if i < 3 { self.0[i + 1] << 63 } else { 0 };
            *slot = self.0[i] >> 1 | high;
        }
        U256(out)
    }

    /// How many decimal digits; 0 has none.
    pub(super) fn digit_count(self) -> usize {
        POW10.iter().position(|&p| p > self).unwrap_or(78)
    }

    /// Strips trailing decimal zeros of a nonzero value; how many went.
    pub(super) fn strip_zeros(&mut self) -> i64 {
        let mut n = 0;
        loop {
            let (q, r) = self.divrem_u64(10);
            if r != 0 || self.is_zero() {
                return n;
            }
            *self = q;
            n += 1;
        }
    }

    /// The decimal digits, least significant first, into `buf`.
    pub(super) fn write_digits(self, buf: &mut [u8; 78]) -> &[u8] {
        let mut x = self;
        let mut n = 0;
        while !x.is_zero() {
            let (q, r) = x.divrem_u64(10);
            buf[n] = r as u8;
            n += 1;
            x = q;
        }
        &buf[..n]
    }

    /// The value of at most 77 digits, least significant first.
    pub(super) fn from_digits(digits: &[u8]) -> U256 {
        digits.iter().rev().fold(U256::ZERO, |v, &d| {
            let mut t = U256(mul_small(v.0, 10));
            t.0[0] = t.0[0].wrapping_add(u64::from(d));
            if t.0[0] < u64::from(d) {
                // carry into the higher limbs
                for limb in &mut t.0[1..] {
                    *limb = limb.wrapping_add(1);
                    if *limb != 0 {
                        break;
                    }
                }
            }
            t
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_matches_u128_and_the_digits() {
        let a = U256::from_u128(u128::MAX);
        let b = U256::from_u128(12345678901234567890);
        let p = a.checked_mul(b).unwrap();
        let (q, r) = p.divrem(b);
        assert_eq!((q, r), (a, U256::ZERO));
        let (q, r) = p.checked_add(U256::from_u128(7)).unwrap().divrem(a);
        assert_eq!((q, r), (b, U256::from_u128(7)));
        assert!(POW10[77].checked_mul(U256::from_u128(12)).is_none());
        assert_eq!(POW10[77].digit_count(), 78);
        assert_eq!(POW10[77].sub(U256::from_u128(1)).digit_count(), 77);
        let mut buf = [0; 78];
        let digits = POW10[50].checked_add(U256::from_u128(3)).unwrap();
        let written = digits.write_digits(&mut buf).to_vec();
        assert_eq!(written.len(), 51);
        assert_eq!(U256::from_digits(&written), digits);
        let mut z = POW10[60].checked_mul(U256::from_u128(7)).unwrap();
        assert_eq!(z.strip_zeros(), 60);
        assert_eq!(z, U256::from_u128(7));
        assert_eq!(U256::from_u128(1).checked_shl(255).unwrap().bits(), 256);
        assert!(U256::from_u128(2).checked_shl(255).is_none());
        assert_eq!(
            U256::from_u128(3)
                .checked_shl(130)
                .unwrap()
                .divrem(U256::from_u128(3).checked_shl(66).unwrap()),
            (U256::from_u128(1 << 64), U256::ZERO)
        );
    }
}
