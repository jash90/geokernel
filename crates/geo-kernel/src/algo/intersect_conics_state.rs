//! Output slots and the near-to permutation for one conic pair.

// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectConics.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/PointPairList.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{
    compact, distinct, points_equal, solve, with_bilinear_multiplicity, ConicData, Points, Pt,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    First,
    Assign,
    Fixed,
}

/// The compact part of the branch state needed to rebuild the current slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SavedSlots {
    order: [u8; 4],
    phase: Phase,
    labelled_second: bool,
}

/// The pencil and output slots of one conic pair. State belongs to the
/// pair, shared by its full, indexed and nearest-point outputs.
#[derive(Clone, Debug)]
pub(crate) struct Intersection {
    pencil: ConicData,
    current: Points,
    previous: Points,
    age: [u32; 4],
    labelled: [bool; 4],
    alive: [bool; 4],
    order: [usize; 4],
    first: bool,
    needs_permutation: bool,
}

impl Default for Intersection {
    fn default() -> Self {
        Self {
            pencil: ConicData::new(),
            current: [None; 4],
            previous: [None; 4],
            age: [0; 4],
            labelled: [false; 4],
            alive: [false; 4],
            order: [0, 1, 2, 3],
            first: true,
            needs_permutation: true,
        }
    }
}

impl Intersection {
    pub(crate) fn fixed_order(&mut self) {
        self.first = false;
        self.needs_permutation = false;
    }

    pub(crate) fn saved_slots(&self) -> SavedSlots {
        SavedSlots {
            order: self.order.map(|i| i as u8),
            phase: if self.first {
                Phase::First
            } else if self.needs_permutation {
                Phase::Assign
            } else {
                Phase::Fixed
            },
            labelled_second: self.labelled[1],
        }
    }

    pub(crate) fn restore_slots(&mut self, saved: SavedSlots) {
        self.order = saved.order.map(usize::from);
        self.first = saved.phase != Phase::Fixed;
        self.needs_permutation = self.first;
        self.labelled[1] = saved.labelled_second;
    }

    pub(crate) fn current(&self) -> Points {
        self.current
    }

    pub(crate) fn undefined(&mut self) -> Points {
        self.current = [None; 4];
        self.current
    }

    pub(crate) fn label(&mut self, slot: usize) {
        self.labelled[slot] = true;
    }

    pub(crate) fn reset_permutation(&mut self) {
        self.age = [0; 4];
        self.alive = [false; 4];
        self.needs_permutation = true;
    }

    /// Compute one update. The non-continuous mode initializes a
    /// permutation once, then uses it on subsequent updates. The
    /// continuous mode keeps the closest available branches.
    pub(crate) fn update(
        &mut self,
        a: &ConicData,
        b: &ConicData,
        continuous: bool,
        on_paths: impl Fn((f64, f64)) -> bool,
    ) -> Points {
        if !a.defined || !b.defined {
            return self.undefined();
        }
        let mut candidates = solve(a, b, &mut self.pencil, continuous);
        if self.needs_permutation && self.current.iter().all(Option::is_some) {
            candidates = with_bilinear_multiplicity(a, b, candidates);
        }
        let on_path = candidates.map(|p| p.is_some_and(&on_paths));

        if self.first {
            self.current = compact(std::array::from_fn(|i| {
                candidates[i].filter(|_| on_path[i])
            }));
            if self.current[0].is_some() {
                self.previous = self.current;
                self.first = false;
            }
        } else {
            if continuous || self.needs_permutation {
                self.remember();
                let close = candidates.iter().enumerate().any(|(i, &p)| {
                    candidates[i + 1..]
                        .iter()
                        .any(|&q| distance_squared(p, q) <= 0.0004)
                });
                if self.needs_permutation || !close {
                    self.order = self.nearest(candidates, on_path);
                }
                self.needs_permutation = false;
            }

            let p = self.order.map(|i| candidates[i]);
            self.current = self.order.map(|i| candidates[i].filter(|_| on_path[i]));
            if continuous {
                for (i, candidate) in p.into_iter().enumerate() {
                    if self.current[i].is_none() && candidate.is_some() {
                        self.previous[i] = candidate;
                        self.age[i] = 0;
                    }
                }
            }
        }
        if !self.labelled[1] {
            self.current = distinct(self.current);
        }
        self.current
    }

    fn remember(&mut self) {
        let mut finite = self.current.into_iter().flatten();
        let singular = match (finite.next(), finite.next(), finite.next()) {
            (Some(p), Some(q), None) => points_equal(Some(p), Some(q)),
            _ => false,
        };
        for i in 0..4 {
            let current = self.current[i];
            if current.is_some() && !singular {
                self.previous[i] = current;
                self.age[i] = 0;
            } else {
                self.age[i] = self.age[i].saturating_add(1);
            }
            self.alive[i] |= current.is_some() || self.labelled[i];
        }
    }

    fn nearest(&self, candidates: Points, on_path: [bool; 4]) -> [usize; 4] {
        let mut pairs: [(usize, usize, f64); 16] = std::array::from_fn(|i| {
            let (old, new) = (i / 4, i % 4);
            (
                old,
                new,
                distance_squared(self.previous[old], candidates[new]) + f64::from(self.age[old]),
            )
        });
        let max = pairs
            .iter()
            .map(|p| p.2)
            .filter(|d| d.is_finite())
            .fold(-1.0, f64::max);
        for pair in &mut pairs {
            if !pair.2.is_finite() {
                pair.2 = max + 1.0;
            }
        }
        pairs.sort_by(|a, b| {
            self.alive[b.0]
                .cmp(&self.alive[a.0])
                .then_with(|| on_path[b.1].cmp(&on_path[a.1]))
                .then_with(|| a.2.total_cmp(&b.2))
        });
        let mut used = [false; 4];
        let mut order = [usize::MAX; 4];
        for (old, new, _) in pairs {
            if order[old] == usize::MAX && !used[new] {
                order[old] = new;
                used[new] = true;
            }
        }
        order
    }
}

fn distance_squared(a: Pt, b: Pt) -> f64 {
    match (a, b) {
        (Some((x, y)), Some((u, v))) => (x - u) * (x - u) + (y - v) * (y - v),
        _ => f64::INFINITY,
    }
}
