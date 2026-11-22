// Licensed under the EUPL 1.2.

//! Reduced Groebner bases over the rationals, computed fraction-free.
//!
//! Buchberger's algorithm with the normal selection strategy and the pair
//! criteria of Gebauer and Möller ("On an installation of Buchberger's
//! algorithm", J. Symbolic Computation 6, 1988). Every polynomial is kept
//! primitive with a positive leading coefficient, so the result is the
//! reduced basis up to a positive integer factor per element. All work is
//! charged to a [`Budget`]; an exhausted budget gives `None`.

use std::cmp::Ordering;

use super::int::Int;
use super::poly::{Mono, Order, Poly};
use super::Budget;

/// The largest number of polynomials a computation may hold.
const MAX_POLYS: usize = 600;
/// The largest number of terms of one polynomial.
const MAX_TERMS: usize = 20_000;
/// The largest coefficient size, in 32-bit limbs.
const MAX_LIMBS: usize = 64;

struct Pair {
    i: usize,
    j: usize,
    lcm: Mono,
}

/// The reduced Groebner basis of `polys` for `order`, each element
/// primitive, sorted by `order` and with a positive leading coefficient,
/// listed by increasing leading monomial.
pub(crate) fn basis(polys: &[Poly], order: &Order, budget: &Budget) -> Option<Vec<Poly>> {
    let mut all: Vec<Poly> = Vec::new();
    let mut active: Vec<bool> = Vec::new();
    let mut pairs: Vec<Pair> = Vec::new();
    for p in polys {
        let mut p = p.clone();
        p.sort(order);
        let p = reduce(p, &all, &active, order, budget)?;
        if p.is_zero() {
            continue;
        }
        if p.is_constant() {
            return Some(vec![Poly::constant(1)]);
        }
        update(&mut all, &mut active, &mut pairs, p)?;
    }
    while !pairs.is_empty() {
        budget.charge(1)?;
        let best = (0..pairs.len())
            .min_by(|&a, &b| {
                order
                    .cmp(&pairs[a].lcm, &pairs[b].lcm)
                    .then_with(|| (pairs[a].j, pairs[a].i).cmp(&(pairs[b].j, pairs[b].i)))
            })
            .expect("pairs is not empty");
        let pair = pairs.swap_remove(best);
        let s = s_polynomial(&all[pair.i], &all[pair.j], &pair.lcm, order, budget)?;
        let h = reduce(s, &all, &active, order, budget)?;
        if h.is_zero() {
            continue;
        }
        if h.is_constant() {
            return Some(vec![Poly::constant(1)]);
        }
        update(&mut all, &mut active, &mut pairs, h)?;
    }
    // Interreduce the minimal basis.
    let mut minimal: Vec<Poly> = all
        .into_iter()
        .zip(active)
        .filter_map(|(p, a)| a.then_some(p))
        .collect();
    minimal.sort_by(|a, b| order.cmp(&a.terms[0].0, &b.terms[0].0));
    let mut reduced = Vec::with_capacity(minimal.len());
    for k in 0..minimal.len() {
        let others: Vec<Poly> = minimal
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != k)
            .map(|(_, p)| p.clone())
            .collect();
        let flags = vec![true; others.len()];
        let lead = minimal[k].terms[0].clone();
        let tail = Poly {
            terms: minimal[k].terms[1..].to_vec(),
        };
        let tail = reduce_scaled(tail, &others, &flags, order, budget)?;
        let mut terms = vec![(lead.0, lead.1.mul(&tail.1))];
        terms.extend(tail.0.terms);
        reduced.push(Poly { terms }.primitive());
    }
    Some(reduced)
}

fn update(
    all: &mut Vec<Poly>,
    active: &mut Vec<bool>,
    pairs: &mut Vec<Pair>,
    h: Poly,
) -> Option<()> {
    if all.len() >= MAX_POLYS {
        return None;
    }
    let lh = h.terms[0].0;
    let n = all.len();
    // New pairs (g, h), pruned by the chain criterion among themselves.
    let candidates: Vec<(usize, Mono, bool)> = (0..n)
        .filter(|&g| active[g])
        .map(|g| {
            let lg = all[g].terms[0].0;
            (g, lg.lcm(&lh), lg.coprime(&lh))
        })
        .collect();
    let mut kept: Vec<(usize, Mono, bool)> = Vec::new();
    for (k, &(g, lcm, coprime)) in candidates.iter().enumerate() {
        let dominated = !coprime
            && candidates.iter().enumerate().any(|(l, &(_, other, _))| {
                l != k && other.divides(&lcm) && (other != lcm || l < k)
            });
        if !dominated {
            kept.push((g, lcm, coprime));
        }
    }
    // Old pairs whose lcm is divisible by lt(h) without being the lcm of h
    // with either member are redundant.
    pairs.retain(|p| {
        !lh.divides(&p.lcm)
            || all[p.i].terms[0].0.lcm(&lh) == p.lcm
            || all[p.j].terms[0].0.lcm(&lh) == p.lcm
    });
    for (g, lcm, coprime) in kept {
        if !coprime {
            pairs.push(Pair { i: g, j: n, lcm });
        }
    }
    for g in 0..n {
        if active[g] && lh.divides(&all[g].terms[0].0) {
            active[g] = false;
        }
    }
    all.push(h);
    active.push(true);
    Some(())
}

fn s_polynomial(f: &Poly, g: &Poly, lcm: &Mono, order: &Order, budget: &Budget) -> Option<Poly> {
    let (lf, cf) = &f.terms[0];
    let (lg, cg) = &g.terms[0];
    let d = cf.gcd(cg);
    let a = cg.div_exact(&d)?;
    let b = cf.div_exact(&d)?;
    combine(
        f,
        &a,
        &lf.quotient(lcm),
        g,
        &b,
        &lg.quotient(lcm),
        order,
        budget,
    )
}

/// `a*mf*f - b*mg*g`, both sorted by `order`.
#[allow(clippy::too_many_arguments)]
fn combine(
    f: &Poly,
    a: &Int,
    mf: &Mono,
    g: &Poly,
    b: &Int,
    mg: &Mono,
    order: &Order,
    budget: &Budget,
) -> Option<Poly> {
    // Coefficient arithmetic grows with the size of the multipliers.
    let width = 1 + a.size() + b.size();
    budget.charge((f.terms.len() + g.terms.len()).saturating_mul(width))?;
    let mut terms = Vec::with_capacity(f.terms.len() + g.terms.len());
    let mut x = f.terms.iter().peekable();
    let mut y = g.terms.iter().peekable();
    loop {
        let next = match (x.peek(), y.peek()) {
            (None, None) => break,
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (Some((m1, _)), Some((m2, _))) => order.cmp(&m1.mul(mf)?, &m2.mul(mg)?),
        };
        match next {
            Ordering::Greater => {
                let (m, c) = x.next().expect("peeked");
                terms.push((m.mul(mf)?, c.mul(a)));
            }
            Ordering::Less => {
                let (m, c) = y.next().expect("peeked");
                terms.push((m.mul(mg)?, c.mul(b).neg()));
            }
            Ordering::Equal => {
                let (m, c1) = x.next().expect("peeked");
                let (_, c2) = y.next().expect("peeked");
                let c = c1.mul(a).sub(&c2.mul(b));
                if !c.is_zero() {
                    terms.push((m.mul(mf)?, c));
                }
            }
        }
    }
    if terms.len() > MAX_TERMS || terms.iter().any(|(_, c)| c.size() > MAX_LIMBS) {
        return None;
    }
    Some(Poly { terms })
}

/// The normal form of `f` by the active polynomials, made primitive.
fn reduce(f: Poly, all: &[Poly], active: &[bool], order: &Order, budget: &Budget) -> Option<Poly> {
    let (r, _) = reduce_scaled(f, all, active, order, budget)?;
    Some(r.primitive())
}

/// The normal form `r` of `f` with the factor `s` such that `s*f - r` lies
/// in the ideal; `s` is a positive integer.
fn reduce_scaled(
    mut f: Poly,
    all: &[Poly],
    active: &[bool],
    order: &Order,
    budget: &Budget,
) -> Option<(Poly, Int)> {
    let mut done: Vec<(Mono, Int)> = Vec::new();
    let mut scale = Int::from_i64(1);
    while let Some((m, c)) = f.terms.first().cloned() {
        budget.charge(1)?;
        let divisor = (0..all.len()).find(|&k| active[k] && all[k].terms[0].0.divides(&m));
        let Some(k) = divisor else {
            done.push((m, c));
            f.terms.remove(0);
            continue;
        };
        let (lg, cg) = &all[k].terms[0];
        let d = c.gcd(cg);
        let a = cg.div_exact(&d)?;
        let b = c.div_exact(&d)?;
        if a.is_negative() {
            return None;
        }
        f = combine(
            &f,
            &a,
            &Mono::ONE,
            &all[k],
            &b,
            &lg.quotient(&m),
            order,
            budget,
        )?;
        if !a.is_unit() {
            for (_, dc) in &mut done {
                *dc = dc.mul(&a);
            }
            scale = scale.mul(&a);
        }
        // Keep the coefficients small: divide out the common content.
        let big = f.terms.iter().chain(&done).any(|(_, c)| c.size() > 2);
        if big {
            let mut g = scale.clone();
            for (_, c) in f.terms.iter().chain(&done) {
                g = g.gcd(c);
                if g.is_unit() {
                    break;
                }
            }
            if !g.is_unit() && !g.is_zero() {
                for (_, c) in f.terms.iter_mut().chain(done.iter_mut()) {
                    *c = c.div_exact(&g)?;
                }
                scale = scale.div_exact(&g)?;
            }
        }
        if done.len() + f.terms.len() > MAX_TERMS
            || scale.size() > MAX_LIMBS
            || done.iter().any(|(_, c)| c.size() > MAX_LIMBS)
        {
            return None;
        }
    }
    Some((Poly { terms: done }, scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(terms: &[(i64, &[(usize, u8)])], order: &Order) -> Poly {
        let mut poly = Poly {
            terms: terms
                .iter()
                .map(|&(c, vars)| {
                    let mut m = Mono::ONE;
                    for &(v, e) in vars {
                        m.0[v] = e;
                    }
                    (m, Int::from_i64(c))
                })
                .collect(),
        };
        poly.sort(order);
        poly
    }

    #[test]
    fn eliminates_a_parameter() {
        // x = t^2, y = t^3, eliminate t (variable 0): y^2 - x^3.
        let order = Order { split: 1, vars: 3 };
        let budget = Budget::new(1_000_000);
        let f = p(&[(1, &[(1, 1)]), (-1, &[(0, 2)])], &order);
        let g = p(&[(1, &[(2, 1)]), (-1, &[(0, 3)])], &order);
        let basis = basis(&[f, g], &order, &budget).unwrap();
        let free: Vec<_> = basis
            .iter()
            .filter(|q| !q.terms[0].0.uses_any(0..1))
            .collect();
        assert_eq!(free.len(), 1);
        let expected = p(&[(1, &[(2, 2)]), (-1, &[(1, 3)])], &order);
        let found = free[0].clone().canonical();
        let expected = expected.canonical();
        assert!(found == expected || found == expected.neg());
    }

    #[test]
    fn detects_the_unit_ideal_and_runs_out_of_budget() {
        let order = Order { split: 0, vars: 2 };
        let f = p(&[(1, &[(0, 1), (1, 1)]), (-1, &[])], &order);
        let g = p(&[(1, &[(0, 1)])], &order);
        let unit = basis(&[f.clone(), g.clone()], &order, &Budget::new(1_000)).unwrap();
        assert_eq!(unit, vec![Poly::constant(1)]);
        assert!(basis(&[f, g], &order, &Budget::new(2)).is_none());
    }
}
