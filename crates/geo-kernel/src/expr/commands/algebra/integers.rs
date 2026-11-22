// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDivision.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDivisorsList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDivisorsSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoPrimeFactors.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIsPrime.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListGCD.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListLCM.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{gcd, LARGEST_INTEGER};
use crate::expr::eval::{Ctx, Val};
use crate::expr::list::Slot;
use crate::expr::math::{check_integer, is_integer, java_round};
use crate::expr::AlgebraOp;
use crate::kernel::Value;

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn algebra_list(&self, op: AlgebraOp, a: u32, b: u32) -> Option<(u32, u32)> {
        let x = self.full_number(a);
        let lists = self.lists?;
        let output = match op {
            AlgebraOp::Division => {
                let y = self.full_number(b);
                if x.is_nan() || y.is_nan() || x.abs().max(y.abs()) > LARGEST_INTEGER {
                    return None;
                }
                let abs = y.abs();
                let residue = x % abs;
                let residue = if residue < 0.0 {
                    residue + abs
                } else {
                    residue
                };
                vec![java_round((x - residue) / y) as f64, residue]
            }
            AlgebraOp::PrimeFactors => {
                let factors = factor(positive_integer(x)?, |work| self.stop(work))?;
                let len: usize = factors.iter().map(|&(_, count)| count as usize).sum();
                if !lists.charge(len as u64) {
                    return None;
                }
                factors
                    .into_iter()
                    .flat_map(|(prime, count)| std::iter::repeat_n(prime as f64, count as usize))
                    .collect()
            }
            AlgebraOp::DivisorsList if is_integer(x) => {
                let factors = factor(positive_integer(x)?, |work| self.stop(work))?;
                let mut divisors = vec![1u64];
                for (prime, count) in factors {
                    let len = divisors.len();
                    if !lists.charge(len as u64 * u64::from(count)) {
                        return None;
                    }
                    let mut power = 1;
                    for _ in 0..count {
                        power *= prime;
                        for i in 0..len {
                            divisors.push(divisors[i] * power);
                        }
                    }
                }
                let len = divisors.len() as u64;
                if !lists.charge(len * u64::from(len.ilog2() + 2)) {
                    return None;
                }
                divisors.sort_unstable();
                divisors.into_iter().map(|d| d as f64).collect()
            }
            _ => return None,
        };
        let len = output.len() as u32;
        let start = lists.reserve(len)?;
        for (i, value) in output.into_iter().enumerate() {
            lists.set(start + i as u32, Slot::number(value));
        }
        Some((start, len))
    }

    pub(crate) fn algebra_number(&self, op: AlgebraOp, a: u32) -> Val {
        let x = self.full_number(a);
        if op == AlgebraOp::IsInteger {
            return Val::B(is_integer(x), true);
        }
        if op == AlgebraOp::IsPrime {
            return match positive_integer(x).and_then(|n| prime(n, |work| self.stop(work))) {
                Some(b) => Val::B(b, true),
                None => Val::B(false, false),
            };
        }
        let result = || {
            if !is_integer(x) {
                return None;
            }
            let factors = factor(positive_integer(x)?, |work| self.stop(work))?;
            let mut result = 1i64;
            for (p, exp) in factors {
                let term = if op == AlgebraOp::Divisors {
                    i64::from(exp + 1)
                } else {
                    let p = p as f64;
                    java_round((p.powi(exp as i32 + 1) - 1.0) / (p - 1.0))
                };
                result = result.wrapping_mul(term);
            }
            Some(result as f64)
        };
        Val::N(result().unwrap_or(f64::NAN))
    }

    pub(super) fn list_gcd_lcm(&self, is_gcd: bool, l: u32) -> f64 {
        let compute = || {
            let lists = self.lists?;
            let (start, len) = self.list(l)?;
            if len == 0 || !lists.charge(u64::from(len)) {
                return None;
            }
            let mut result = if is_gcd { 0 } else { 1 };
            const LIMIT: u128 = 1_000_000_000_000_000;
            for i in 0..len {
                let Value::Number(v) = lists.read(self.c, start + i).value else {
                    return None;
                };
                if !is_integer(v) {
                    return None;
                }
                // AlgoListLCM truncates every entry after the first.
                let rounded = if is_gcd || i == 0 {
                    check_integer(v)
                } else {
                    v
                };
                let n = u128::from((rounded as i64).unsigned_abs());
                if is_gcd {
                    result = gcd(result, n);
                } else if n == 0 {
                    result = 0;
                } else if result <= LIMIT {
                    // A later zero can still make an oversized LCM zero.
                    result = result / gcd(result, n) * n;
                }
            }
            (result <= LIMIT).then_some(result as f64)
        };
        compute().unwrap_or(f64::NAN)
    }
}

/// Both prime commands apply Java's round before checking the range.
fn positive_integer(x: f64) -> Option<u64> {
    let n = java_round(x);
    (n >= 1 && n <= LARGEST_INTEGER as i64).then_some(n as u64)
}

fn factor(mut n: u64, stop: impl Fn(u64) -> bool) -> Option<Vec<(u64, u32)>> {
    let mut factors = Vec::new();
    let mut d = 2;
    while d <= n / d {
        if stop(1) {
            return None;
        }
        let mut exponent = 0;
        while n.is_multiple_of(d) {
            if stop(1) {
                return None;
            }
            n /= d;
            exponent += 1;
        }
        if exponent != 0 {
            factors.push((d, exponent));
        }
        if (d == 2 || exponent != 0) && prime(n, &stop)? {
            break;
        }
        d += if d == 2 { 1 } else { 2 };
    }
    if n > 1 {
        factors.push((n, 1));
    }
    Some(factors)
}

/// These Miller–Rabin witnesses determine primality throughout the supported
/// range (and for all unsigned 64-bit integers).
fn prime(n: u64, stop: impl Fn(u64) -> bool) -> Option<bool> {
    if n < 4 || n.is_multiple_of(2) {
        return Some(n == 2 || n == 3);
    }
    let rounds = (n - 1).trailing_zeros();
    let odd = (n - 1) >> rounds;
    let mul = |a: u64, b: u64| (u128::from(a) * u128::from(b) % u128::from(n)) as u64;
    'witnesses: for mut a in [2, 325, 9375, 28178, 450775, 9780504, 1795265022] {
        a %= n;
        if a == 0 {
            continue;
        }
        let (mut result, mut exp) = (1, odd);
        while exp != 0 {
            if stop(1) {
                return None;
            }
            if exp & 1 == 1 {
                result = mul(result, a);
            }
            a = mul(a, a);
            exp >>= 1;
        }
        if result == 1 || result == n - 1 {
            continue;
        }
        for _ in 1..rounds {
            if stop(1) {
                return None;
            }
            result = mul(result, result);
            if result == n - 1 {
                continue 'witnesses;
            }
        }
        return Some(false);
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use crate::expr::{eval::with_work_limit, work_done};
    use crate::kernel::{Kind, Value};

    #[test]
    fn factorization_stops_within_the_expression_work_limit() {
        let before = work_done();
        with_work_limit(80, || {
            let c = crate::script::load("l = PrimeFactors(1000036000099)").unwrap();
            let id = c.id("l").unwrap();
            assert_eq!(c.kind(id), Kind::List);
            assert_eq!(c.value(id), Value::Undefined);
        });
        let spent = work_done() - before;
        assert!((80..=85).contains(&spent), "{spent} units of work");
    }
}
