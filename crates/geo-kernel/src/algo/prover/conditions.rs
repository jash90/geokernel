// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/NDGDetector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoProveDetails.java
//   source/shared/common/src/main/java/org/geogebra/common/util/Prover.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Geometric readings of a nondegeneracy polynomial.
//!
//! The reference factors the generator of the elimination ideal into
//! irreducible factors and looks each factor up among polynomials of known
//! meaning, in this order: three free points collinear, two free points
//! equal, two equal first or second coordinates, then perpendicular,
//! parallel and congruent pairs of free points. A factor matches a
//! polynomial equal to it or to its negative. Instead of factoring, the
//! kernel divides the generator by every irreducible primitive candidate in
//! that order: what is left after all divisions is a product of factors
//! without a reading.

use super::int::Int;
use super::poly::Poly;
use super::translate::{collinear, linear, parallel, perpendicular, square_distance, System};
use super::Budget;
use crate::kernel::{Construction, Id};

/// What a factor of a generator means.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Meaning {
    /// A condition text and its readability in quarters (collinear 4,
    /// equal 2).
    Text { text: String, score: u32 },
    /// Two equal first (or `second`) coordinates of the variables `pair`:
    /// matched, with an infinite readability.
    Hidden { second: bool, pair: (usize, usize) },
    /// Line conditions: the reference names lines it looks up or creates.
    Lines,
}

/// Reads `generator` (canonical, in the original variables): the meanings
/// of its distinct nonconstant factors, `Some(None)` when a factor has none,
/// `None` when the reading needs what the kernel cannot reproduce.
pub(super) fn read(
    c: &Construction,
    system: &System,
    generator: &Poly,
    substitutions: &[(usize, i64)],
    budget: &Budget,
) -> Option<Option<Vec<Meaning>>> {
    let mut rest = generator.clone().primitive().canonical();
    if rest.is_constant() {
        return Some(Some(Vec::new()));
    }
    let mut meanings = Vec::new();
    for (candidate, meaning) in candidates(c, system, budget)? {
        let candidate = candidate.substitute(substitutions, budget)?;
        if candidate.is_constant() || !candidate.content().is_unit() || !irreducible(&candidate)? {
            continue;
        }
        let mut matched = false;
        while let Some(quotient) = rest.divide(&candidate, budget) {
            rest = quotient;
            matched = true;
        }
        if matched {
            meanings.push(meaning?);
        }
        if rest.is_constant() {
            break;
        }
    }
    if !rest.is_constant() {
        return Some(None);
    }
    Some(Some(meanings))
}

/// The candidate polynomials with their meanings, in the reference's order
/// of lookup. A meaning is `None` when its text cannot be written (an
/// unlabelled point).
fn candidates(
    c: &Construction,
    system: &System,
    budget: &Budget,
) -> Option<Vec<(Poly, Option<Meaning>)>> {
    let points = &system.free_points;
    let vars = |id: &Id| system.point_vars[id];
    let mut list = Vec::new();
    for (i, a) in points.iter().enumerate() {
        for (j, b) in points.iter().enumerate().skip(i + 1) {
            for d in points.iter().skip(j + 1) {
                let p = collinear(vars(a), vars(b), vars(d), budget)?;
                let text = text(c, "AreCollinear", &[*a, *b, *d]);
                list.push((p, text.map(|text| Meaning::Text { text, score: 4 })));
            }
        }
    }
    let pairs: Vec<(Id, Id)> = points
        .iter()
        .enumerate()
        .flat_map(|(i, &a)| points.iter().skip(i + 1).map(move |&b| (a, b)))
        .collect();
    for &(a, b) in &pairs {
        let p = square_distance(vars(&a), vars(&b), budget)?;
        let text = equal_text(c, a, b);
        list.push((p, text.map(|text| Meaning::Text { text, score: 2 })));
    }
    // Coordinates that are free variables: both of a free point, the first
    // of a point on a path.
    let mut xs: Vec<usize> = points.iter().map(|id| vars(id)[0]).collect();
    xs.extend(system.path_points.iter().map(|id| system.point_vars[id][0]));
    let ys: Vec<usize> = points.iter().map(|id| vars(id)[1]).collect();
    for (second, coordinates) in [(false, &xs), (true, &ys)] {
        for (i, &u) in coordinates.iter().enumerate() {
            for &w in coordinates.iter().skip(i + 1) {
                let meaning = Meaning::Hidden {
                    second,
                    pair: point_pair(u, w, second),
                };
                list.push((linear(&[(1, u), (-1, w)]), Some(meaning)));
            }
        }
    }
    for &(a, b) in &pairs {
        for &(d, e) in &pairs {
            let g = [vars(&a)[0], vars(&a)[1], vars(&b)[0], vars(&b)[1]];
            let h = [vars(&d)[0], vars(&d)[1], vars(&e)[0], vars(&e)[1]];
            list.push((perpendicular(g, h, budget)?, Some(Meaning::Lines)));
            list.push((parallel(g, h, budget)?, Some(Meaning::Lines)));
            let congruent = square_distance(vars(&a), vars(&b), budget)?
                .sub(&square_distance(vars(&d), vars(&e), budget)?, budget)?;
            list.push((congruent, Some(Meaning::Lines)));
        }
    }
    Some(list)
}

/// The points whose coordinates are `u` and `w`, as the pair of their first
/// variables in order, so that equal first and second coordinates of the
/// same points give the same pair.
fn point_pair(u: usize, w: usize, second: bool) -> (usize, usize) {
    let first = |var: usize| if second { var - 1 } else { var };
    let (a, b) = (first(u), first(w));
    (a.min(b), a.max(b))
}

/// `AreEqual[A,B]` with the labels in order.
pub(super) fn equal_text(c: &Construction, a: Id, b: Id) -> Option<String> {
    text(c, "AreEqual", &[a, b])
}

/// `Name[P,Q,...]` with the labels sorted as Java strings. Only plain ASCII
/// labels: the reference sorts the texts after removing accents.
fn text(c: &Construction, name: &str, points: &[Id]) -> Option<String> {
    let mut labels = points
        .iter()
        .map(|&id| {
            let label = c.label(id);
            (!label.is_empty() && label.bytes().all(|b| b.is_ascii_graphic())).then_some(label)
        })
        .collect::<Option<Vec<_>>>()?;
    labels.sort_unstable();
    Some(format!("{name}[{}]", labels.join(",")))
}

/// Whether a primitive polynomial of degree one or two is irreducible over
/// the rationals; `None` for higher degrees.
fn irreducible(p: &Poly) -> Option<bool> {
    match p.degree() {
        0 => Some(false),
        1 => Some(true),
        2 => Some(quadratic_irreducible(p)),
        _ => None,
    }
}

/// A quadratic polynomial factors into rational linear factors exactly when
/// its homogenized form `x^T S x / 2` has rank at most two and, at rank two,
/// minus the determinant of a nonsingular principal 2x2 minor of `S` is a
/// square (the discriminant of the binary form it reduces to).
fn quadratic_irreducible(p: &Poly) -> bool {
    let mut vars: Vec<usize> = Vec::new();
    for (m, _) in &p.terms {
        for (v, &e) in m.0.iter().enumerate() {
            if e != 0 && !vars.contains(&v) {
                vars.push(v);
            }
        }
    }
    let n = vars.len() + 1;
    let index = |v: usize| vars.iter().position(|&w| w == v).map_or(0, |k| k + 1);
    let mut s = vec![vec![Int::zero(); n]; n];
    for (m, c) in &p.terms {
        let mut used: Vec<usize> = Vec::new();
        for (v, &e) in m.0.iter().enumerate() {
            for _ in 0..e {
                used.push(index(v));
            }
        }
        while used.len() < 2 {
            used.push(0);
        }
        let (i, j) = (used[0], used[1]);
        if i == j {
            s[i][i] = s[i][i].add(&c.add(c));
        } else {
            s[i][j] = s[i][j].add(c);
            s[j][i] = s[j][i].add(c);
        }
    }
    let rank = rank(s.clone());
    match rank {
        0..=1 => false,
        2 => {
            for i in 0..n {
                for j in i + 1..n {
                    let det = s[i][i].mul(&s[j][j]).sub(&s[i][j].mul(&s[j][i]));
                    if !det.is_zero() {
                        return det.neg().exact_sqrt().is_none();
                    }
                }
            }
            // A symmetric matrix of rank two has a nonsingular principal
            // 2x2 minor.
            unreachable!("rank two without a nonsingular principal minor")
        }
        _ => true,
    }
}

/// Rank by fraction-free Gaussian elimination.
fn rank(mut m: Vec<Vec<Int>>) -> usize {
    let rows = m.len();
    let cols = m.first().map_or(0, Vec::len);
    let mut rank = 0;
    for col in 0..cols {
        let Some(pivot) = (rank..rows).find(|&r| !m[r][col].is_zero()) else {
            continue;
        };
        m.swap(rank, pivot);
        for r in rank + 1..rows {
            if m[r][col].is_zero() {
                continue;
            }
            let a = m[rank][col].clone();
            let b = m[r][col].clone();
            for k in col..cols {
                m[r][k] = m[r][k].mul(&a).sub(&m[rank][k].mul(&b));
            }
            let g = m[r].iter().fold(Int::zero(), |g, x| g.gcd(x));
            if !g.is_zero() && !g.is_unit() {
                for x in &mut m[r] {
                    *x = x.div_exact(&g).expect("gcd divides");
                }
            }
        }
        rank += 1;
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratics() {
        let b = Budget::new(1_000_000);
        let x = Poly::var(0);
        let y = Poly::var(1);
        let one = Poly::constant(1);
        // x^2 + y^2 is irreducible over the rationals.
        let circle = x
            .mul(&x, &b)
            .unwrap()
            .add(&y.mul(&y, &b).unwrap(), &b)
            .unwrap();
        assert!(quadratic_irreducible(&circle));
        // x^2 - y^2 = (x - y)(x + y).
        let split = x
            .mul(&x, &b)
            .unwrap()
            .sub(&y.mul(&y, &b).unwrap(), &b)
            .unwrap();
        assert!(!quadratic_irreducible(&split));
        // x*y - x = x*(y - 1).
        let product = x.mul(&y, &b).unwrap().sub(&x, &b).unwrap();
        assert!(!quadratic_irreducible(&product));
        // x^2 - 2 has no rational factor.
        let surd = x
            .mul(&x, &b)
            .unwrap()
            .sub(&one.add(&one, &b).unwrap(), &b)
            .unwrap();
        assert!(quadratic_irreducible(&surd));
        // x*y - 1.
        let hyperbola = x.mul(&y, &b).unwrap().sub(&one, &b).unwrap();
        assert!(quadratic_irreducible(&hyperbola));
        // (x + 1)^2.
        let square = x.add(&one, &b).unwrap();
        assert!(!quadratic_irreducible(&square.mul(&square, &b).unwrap()));
    }
}
