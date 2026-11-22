// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/util/Prover.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoProve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/AlgoProveDetails.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/ProverBotanasMethod.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/HilbertDimension.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/prover/polynomial/PPolynomial.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The symbolic prover of `Prove` and `ProveDetails`.
//!
//! A statement is decided by the algebraic method: every construction step
//! becomes polynomial equations in the coordinates of its points
//! ([`translate`]), the first two free points are fixed at (0, 0) and
//! (0, 1), and the elimination ideal of the hypotheses together with the
//! negated thesis is computed over the rationals ([`groebner`]). A nonzero
//! elimination ideal proves the statement under the nondegeneracy condition
//! that its generator does not vanish ([`conditions`] reads that generator as
//! geometric conditions). A zero ideal means the statement fails on a
//! component of the configuration space; it is false unless it also holds
//! on one, in which case the dimension of the hypotheses decides between
//! "true on components" and "unknown".
//!
//! The answer depends only on the construction, not on coordinates. Every
//! computation runs under a [`Budget`]; a construction or statement outside
//! the translated subset, or one that exhausts the budget, is
//! [`Outcome::Undecided`]: the kernel does not know what the reference
//! answers.

mod conditions;
mod groebner;
mod int;
mod poly;
mod translate;

use std::cell::Cell;

use crate::expr::Slot;
use crate::kernel::{Construction, Id, Kind, Value};
use poly::{Order, Poly};

/// The work allowance of one proof, in polynomial term operations.
const PROOF_WORK: u64 = 4_000_000;

/// A work counter. Every operation charges it; an exhausted budget makes the
/// computation give up.
pub(crate) struct Budget {
    remaining: Cell<u64>,
}

impl Budget {
    pub(crate) fn new(units: u64) -> Self {
        Self {
            remaining: Cell::new(units),
        }
    }

    pub(crate) fn charge(&self, units: usize) -> Option<()> {
        let units = u64::try_from(units).ok()?;
        let left = self.remaining.get().checked_sub(units)?;
        self.remaining.set(left);
        Some(())
    }
}

/// The reference's proof results (`Prover.ProofResult`), and the kernel's
/// own "cannot tell".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// True; for `ProveDetails`, under these conditions, sorted as the
    /// reference lists them.
    True(Vec<String>),
    /// True, but a nondegeneracy condition has no geometric reading.
    TrueUnreadable,
    /// True on some components of the configuration only.
    TrueOnComponents,
    False,
    /// The reference gives no answer.
    Unknown,
    /// The kernel cannot reproduce the reference's answer.
    Undecided,
}

/// What `Prove` and `ProveDetails` are asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Question {
    Prove,
    Details,
}

/// Decides `statement`, a Boolean object of `c`.
pub(crate) fn decide(c: &Construction, statement: Id, question: Question) -> Outcome {
    let budget = Budget::new(PROOF_WORK);
    let system = match translate::System::build(c, statement, &budget) {
        Ok(system) => system,
        Err(outcome) => return outcome,
    };
    system.decide(c, question, &budget).unwrap_or(Outcome::Undecided)
}

impl translate::System {
    fn decide(&self, c: &Construction, question: Question, budget: &Budget) -> Option<Outcome> {
        let details = question == Question::Details;
        let substitutions = self.substitutions();
        let fixed_two = substitutions.len() == 4;
        // The hypotheses form a set: a thesis polynomial equal to a
        // hypothesis is the same element.
        let negated = self.negated_thesis(budget)?;
        let first = self.with(&[negated.clone()]);
        let elimination = self.eliminate(&first, &substitutions, budget)?;
        if elimination.is_empty() {
            let thesis = self.thesis_factors();
            let second = self.with(&thesis);
            if !self.eliminate(&second, &substitutions, budget)?.is_empty() {
                return Some(Outcome::False);
            }
            let hypotheses = remove_all(&second, &thesis);
            return Some(if self.dimension_exceeds(&hypotheses, &substitutions, budget)? {
                Outcome::Unknown
            } else {
                Outcome::TrueOnComponents
            });
        }
        if !details {
            return Some(Outcome::True(Vec::new()));
        }
        let mut texts = Vec::new();
        if fixed_two {
            texts.push(conditions::equal_text(c, self.free_points[0], self.free_points[1])?);
        }
        match self.best_reading(c, &elimination, &substitutions, budget)? {
            None => return Some(Outcome::TrueUnreadable),
            Some(found) => texts.extend(found),
        }
        texts.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        texts.dedup();
        Some(Outcome::True(texts))
    }

    /// The conditions of the generator with the most readable reading
    /// (`ProverBotanasMethod.prove`): `Some(None)` when no generator has a
    /// readable one, `None` when the reference's choice is not derivable.
    fn best_reading(
        &self,
        c: &Construction,
        elimination: &[Poly],
        substitutions: &[(usize, i64)],
        budget: &Budget,
    ) -> Option<Option<Vec<String>>> {
        if elimination.len() > 1 {
            // A reduced basis whose elements use disjoint variables is the
            // reduced basis for every term order; otherwise which basis the
            // reference's elimination returns is not derivable.
            for (i, p) in elimination.iter().enumerate() {
                for q in &elimination[i + 1..] {
                    if (0..self.total_vars()).any(|v| p.uses_var(v) && q.uses_var(v)) {
                        return None;
                    }
                }
            }
        }
        let mut readings = Vec::new();
        for generator in elimination {
            readings.push(conditions::read(c, self, generator, substitutions, budget)?);
        }
        // With two generators the reference rewrites a pair of equal first
        // and second coordinates of the same points into point equality.
        if readings.len() == 2 {
            let hidden: Vec<(bool, (usize, usize))> = readings
                .iter()
                .flat_map(|r| r.iter().flatten())
                .filter_map(|m| match m {
                    conditions::Meaning::Hidden { second, pair } => Some((*second, *pair)),
                    _ => None,
                })
                .collect();
            if hidden
                .iter()
                .any(|&(second, pair)| hidden.contains(&(!second, pair)))
            {
                return None;
            }
        }
        // Readabilities in quarters: collinear 4, equal 2, lines 3; a
        // coordinate equality is never chosen.
        let scored: Vec<(u32, &Vec<conditions::Meaning>)> = readings
            .iter()
            .flatten()
            .filter_map(|meanings| {
                let mut score = 0;
                for m in meanings {
                    score += match m {
                        conditions::Meaning::Text { score, .. } => *score,
                        conditions::Meaning::Lines => 3,
                        conditions::Meaning::Hidden { .. } => return None,
                    };
                }
                Some((score, meanings))
            })
            .collect();
        let Some(best) = scored.iter().map(|(score, _)| *score).min() else {
            return Some(None);
        };
        let chosen: Vec<&Vec<conditions::Meaning>> = scored
            .iter()
            .filter(|(score, _)| *score == best)
            .map(|(_, meanings)| *meanings)
            .collect();
        let texts = |meanings: &Vec<conditions::Meaning>| -> Option<Vec<String>> {
            meanings
                .iter()
                .map(|m| match m {
                    conditions::Meaning::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        let first = texts(chosen[0])?;
        // Among equally readable generators the reference keeps the first
        // in an unordered iteration.
        for other in &chosen[1..] {
            let mut a = texts(other)?;
            let mut b = first.clone();
            a.sort();
            b.sort();
            if a != b {
                return None;
            }
        }
        Some(Some(first))
    }

    /// The hypotheses with `extra` added as set elements.
    fn with(&self, extra: &[Poly]) -> Vec<Poly> {
        let mut polys = self.hypotheses.clone();
        for p in extra {
            let p = p.clone().canonical();
            if !polys.contains(&p) {
                polys.push(p);
            }
        }
        polys
    }

    /// The generators of the elimination ideal onto the free variables, each
    /// in canonical order; empty for the zero ideal.
    fn eliminate(
        &self,
        polys: &[Poly],
        substitutions: &[(usize, i64)],
        budget: &Budget,
    ) -> Option<Vec<Poly>> {
        let polys = polys
            .iter()
            .map(|p| p.substitute(substitutions, budget))
            .collect::<Option<Vec<_>>>()?;
        let used = self.used_vars(&polys);
        let (renamed, order) = self.renumber(&polys, &used, true);
        let basis = groebner::basis(&renamed, &order, budget)?;
        let inverse = self.inverse_numbering(&used, true);
        Some(
            basis
                .into_iter()
                .filter(|p| !p.terms[0].0.uses_any(0..order.split))
                .map(|p| p.rename(&inverse))
                .collect(),
        )
    }

    /// Variables in use renumbered: those to eliminate first (when
    /// `eliminate`), then the free ones.
    fn used_vars(&self, polys: &[Poly]) -> Vec<usize> {
        (0..self.total_vars())
            .filter(|&v| polys.iter().any(|p| p.uses_var(v)))
            .collect()
    }

    fn numbering(&self, used: &[usize], eliminate: bool) -> (Vec<usize>, usize) {
        let (dependent, free): (Vec<usize>, Vec<usize>) = used
            .iter()
            .copied()
            .partition(|&v| eliminate && !self.is_free(v));
        let split = dependent.len();
        (dependent.into_iter().chain(free).collect(), split)
    }

    fn renumber(&self, polys: &[Poly], used: &[usize], eliminate: bool) -> (Vec<Poly>, Order) {
        let (list, split) = self.numbering(used, eliminate);
        let mut map = vec![0; poly::MAX_VARS];
        for (new, &old) in list.iter().enumerate() {
            map[old] = new;
        }
        let order = Order {
            split,
            vars: list.len(),
        };
        let renamed = polys.iter().map(|p| p.rename(&map)).collect();
        (renamed, order)
    }

    fn inverse_numbering(&self, used: &[usize], eliminate: bool) -> Vec<usize> {
        let (list, _) = self.numbering(used, eliminate);
        let mut map = vec![0; poly::MAX_VARS];
        for (new, &old) in list.iter().enumerate() {
            map[new] = old;
        }
        map
    }

    /// Whether the hypotheses have Hilbert dimension above the naive one
    /// (free variables minus fixed coordinates), counted in the variables
    /// they use (`HilbertDimension.isDimGreaterThan2`).
    fn dimension_exceeds(
        &self,
        hypotheses: &[Poly],
        substitutions: &[(usize, i64)],
        budget: &Budget,
    ) -> Option<bool> {
        // The ring has every variable of the hypotheses before the
        // substitution, except the substituted ones.
        let used: Vec<usize> = self
            .used_vars(hypotheses)
            .into_iter()
            .filter(|v| substitutions.iter().all(|(s, _)| s != v))
            .collect();
        let polys = hypotheses
            .iter()
            .map(|p| p.substitute(substitutions, budget))
            .collect::<Option<Vec<_>>>()?;
        let naive = self.free_vars.iter().filter(|&&f| f).count() - substitutions.len();
        let (renamed, order) = self.renumber(&polys, &used, false);
        let basis = groebner::basis(&renamed, &order, budget)?;
        let leads: Vec<u64> = basis
            .iter()
            .map(|p| {
                (0..order.vars)
                    .filter(|&v| p.terms[0].0 .0[v] != 0)
                    .fold(0u64, |set, v| set | (1 << v))
            })
            .collect();
        let size = naive + 1;
        if size > order.vars {
            return Some(false);
        }
        // Search the subsets of the given size for one that contains the
        // variables of no leading monomial.
        let mut subset: Vec<usize> = (0..size).collect();
        loop {
            budget.charge(leads.len() + 1)?;
            let mask = subset.iter().fold(0u64, |set, &v| set | (1 << v));
            if leads.iter().all(|&lead| lead & !mask != 0) {
                return Some(true);
            }
            // Next combination in lexicographic order.
            let mut i = size;
            loop {
                if i == 0 {
                    return Some(false);
                }
                i -= 1;
                if subset[i] < order.vars - size + i {
                    break;
                }
            }
            subset[i] += 1;
            for k in i + 1..size {
                subset[k] = subset[k - 1] + 1;
            }
        }
    }
}

fn remove_all(polys: &[Poly], removed: &[Poly]) -> Vec<Poly> {
    let removed: Vec<Poly> = removed.iter().map(|p| p.clone().canonical()).collect();
    polys
        .iter()
        .filter(|p| !removed.contains(p))
        .cloned()
        .collect()
}

/// `Prove(statement)`, `ProveDetails(statement)` and
/// `ProveDetails(statement, relation)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProveDef {
    pub statement: Id,
    /// `ProveDetails`, a list; `Prove` otherwise, a Boolean.
    pub details: bool,
    /// The relation-tool form of `ProveDetails`, which words its conditions
    /// as sentences.
    pub relation: bool,
}

/// The result of a proof, decided once when the object is made: the
/// reference decides again only when the text of the statement changes,
/// which moving points never does.
#[derive(Clone, Debug)]
pub struct ProofData {
    pub(crate) value: Value,
    pub(crate) slots: Vec<Slot>,
    outcome: Outcome,
}

impl ProofData {
    /// The prover's answer.
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
}

impl ProveDef {
    pub(crate) fn kind(&self) -> Kind {
        if self.details {
            Kind::List
        } else {
            Kind::Boolean
        }
    }

    pub(crate) fn check(&self, c: &Construction) -> Result<(), crate::kernel::Error> {
        c.expect(self.statement, &[Kind::Boolean])
    }

    pub(crate) fn record(&self, c: &Construction) -> ProofData {
        let question = if self.details {
            Question::Details
        } else {
            Question::Prove
        };
        let outcome = decide(c, self.statement, question);
        let (value, slots) = if self.details {
            self.details_value(&outcome)
        } else {
            let value = match outcome {
                Outcome::True(_) | Outcome::TrueUnreadable | Outcome::TrueOnComponents => {
                    Value::Bool(true)
                }
                Outcome::False => Value::Bool(false),
                Outcome::Unknown | Outcome::Undecided => Value::Undefined,
            };
            (value, Vec::new())
        };
        ProofData {
            value,
            slots,
            outcome,
        }
    }

    /// The list `AlgoProveDetails` makes: empty for an unknown answer,
    /// `{false}`, or `true` followed by the conditions (when there are any)
    /// and the text "c" for a statement true on components only.
    fn details_value(&self, outcome: &Outcome) -> (Value, Vec<Slot>) {
        const ELLIPSIS: &str = "\u{2026}";
        let (head, conditions, components): (Vec<Slot>, Vec<String>, bool) = match outcome {
            Outcome::Undecided => return (Value::Undefined, Vec::new()),
            Outcome::Unknown => (Vec::new(), Vec::new(), false),
            Outcome::False => (vec![Slot::boolean(false)], Vec::new(), false),
            Outcome::True(texts) => {
                if self.relation && !texts.is_empty() {
                    // The relation tool words conditions with localized
                    // sentences, which the kernel does not carry.
                    return (Value::Undefined, Vec::new());
                }
                (vec![Slot::boolean(true)], texts.clone(), false)
            }
            Outcome::TrueUnreadable => (vec![Slot::boolean(true)], vec![ELLIPSIS.into()], false),
            Outcome::TrueOnComponents => {
                (vec![Slot::boolean(true)], vec![ELLIPSIS.into()], true)
            }
        };
        let mut top = head;
        let has_list = !conditions.is_empty();
        let top_len = top.len() + usize::from(has_list) + usize::from(components);
        let mut nested = Vec::new();
        if has_list {
            top.push(Slot::list(top_len as u32, conditions.len() as u32));
            nested.extend(conditions.into_iter().map(|t| Slot::text(t, None)));
        }
        if components {
            top.push(Slot::text("c", None));
        }
        let len = top.len() as u32;
        top.extend(nested);
        (Value::List { len }, top)
    }
}

/// `label = Prove(b)`, `ProveDetails(b)` or `ProveDetails(b, flag)`.
pub(crate) fn define(c: &mut Construction, label: &str, input: &str) -> Option<Result<Id, String>> {
    if !input.starts_with("Prove") {
        return None;
    }
    let (name @ ("Prove" | "ProveDetails"), args) = crate::script::split_call(input)? else {
        return None;
    };
    let details = name == "ProveDetails";
    let start = c.checkpoint();
    let result = (|| {
        let (statement, relation) = match (details, args.as_slice()) {
            (_, [statement]) => (*statement, None),
            (true, [statement, relation]) => (*statement, Some(*relation)),
            _ => return Err(format!("`{name}` does not take {} argument(s)", args.len())),
        };
        let statement = boolean_argument(c, statement)?;
        let relation = match relation {
            None => false,
            Some(flag) => {
                let flag = boolean_argument(c, flag)?;
                c.value(flag) == Value::Bool(true)
            }
        };
        c.add(
            label,
            crate::kernel::Def::Prove(ProveDef {
                statement,
                details,
                relation,
            }),
        )
        .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}

fn boolean_argument(c: &mut Construction, text: &str) -> Result<Id, String> {
    if let Some(id) = c.id(text).filter(|&id| c.kind(id) == Kind::Boolean) {
        return Ok(id);
    }
    let hidden = format!("\0arg{}", c.len());
    if crate::expr::argument_object(c, text, &hidden, Kind::Boolean) {
        return Ok(c.id(&hidden).expect("argument object was just added"));
    }
    Err(format!("expected a Boolean, got `{text}`"))
}

