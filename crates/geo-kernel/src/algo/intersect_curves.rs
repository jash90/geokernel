// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectCoordSysCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectCurveCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectPathPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/IntersectLineSpline.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::boundary::{BoundaryDef, Carrier};
use super::function_roots::fixed_steps_roots;
use crate::algo::curve::Cartesian;
use crate::algo::{Group, Parents, CARRIERS, POINT};
use crate::expr::{IntersectionWork, Raw};
use crate::tol::{MIN_PRECISION, STANDARD_PRECISION};
use crate::{Construction, Def, Error, Id, Kind, Value};

type Point = (f64, f64);
pub(super) type CurveRoots = Vec<Option<Point>>;
const CURVE: &[Kind] = &[Kind::Curve];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CurveIntersection {
    curve: Id,
    other: Id,
    method: Method,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Line(usize),
    Seeds([u32; 2]),
    Point,
}

impl CurveIntersection {
    // A polynomial has degree at most 300. For splines the cubic solve
    // budget admits fewer than 432 coefficients across all pieces.
    pub(crate) const MAX_SAVED_SLOTS: usize = 512;

    pub(crate) fn carrier_pair(c: &Construction, name: &str, args: &[&str]) -> Option<Self> {
        let ("Intersect", [first, second]) = (name, args) else {
            return None;
        };
        let (first, second) = (c.id(first)?, c.id(second)?);
        let (curve, other) = match (c.kind(first), c.kind(second)) {
            (Kind::Curve, kind) if CARRIERS.contains(&kind) => (first, second),
            (kind, Kind::Curve) if CARRIERS.contains(&kind) => (second, first),
            _ => return None,
        };
        Some(Self {
            curve,
            other,
            method: Method::Line(0),
        })
    }

    pub(super) fn multi(c: &Construction, name: &str, args: &[&str]) -> Option<Vec<BoundaryDef>> {
        let first = Self::carrier_pair(c, name, args)?;
        Some(first.line_slots(first.root_count(c).max(1)).collect())
    }

    /// Slots in a saved construction survive an edit that removed crossings.
    pub(crate) fn saved_slots(self, count: usize) -> Vec<Def> {
        self.line_slots(count).map(Def::IntersectBoundary).collect()
    }

    fn line_slots(self, count: usize) -> impl Iterator<Item = BoundaryDef> {
        (0..count).map(move |slot| BoundaryDef::cartesian(self.with_slot(slot)))
    }

    fn with_slot(self, slot: usize) -> Self {
        Self {
            method: Method::Line(slot),
            ..self
        }
    }

    pub(super) fn can_grow(self) -> bool {
        self.method == Method::Line(0)
    }

    pub(super) fn grown_slot(self, slot: usize) -> Option<Self> {
        self.can_grow().then(|| self.with_slot(slot))
    }

    pub(super) fn root_count(self, c: &Construction) -> usize {
        self.with_roots(c, <[Option<Point>]>::len)
    }

    pub(super) fn labels_indexed(self) -> bool {
        self.method != Method::Point
    }

    pub(super) fn is_line(self) -> bool {
        matches!(self.method, Method::Line(_))
    }

    pub(crate) fn seed_pair(c: &Construction, args: &[&str]) -> Option<[Id; 2]> {
        let [first, second, _, _] = args else {
            return None;
        };
        let ids = [c.id(first)?, c.id(second)?];
        ids.iter()
            .all(|&id| c.kind(id) == Kind::Curve)
            .then_some(ids)
    }

    pub(crate) fn selected(c: &mut Construction, args: &[&str]) -> Option<Result<Def, String>> {
        let [curve, other] = Self::seed_pair(c, args)?;
        Some((|| {
            let mut seeds = [0; 2];
            for (arg, seed) in args[2..].iter().zip(&mut seeds) {
                let (id, kind) = c.store_intersection_argument(arg)?;
                if !kind.is_number() {
                    return Err(format!("intersection parameter `{arg}` is not a number"));
                }
                *seed = id;
            }
            Ok(Def::IntersectBoundary(BoundaryDef::cartesian(Self {
                curve,
                other,
                method: Method::Seeds(seeds),
            })))
        })())
    }

    pub(super) fn hidden_parents(self, c: &Construction) -> Vec<Id> {
        let Method::Seeds(seeds) = self.method else {
            return Vec::new();
        };
        seeds
            .into_iter()
            .filter_map(|seed| c.intersection_argument_parents(seed))
            .flatten()
            .copied()
            .collect()
    }

    fn with_roots<T>(self, c: &Construction, read: impl FnOnce(&[Option<Point>]) -> T) -> T {
        c.function_roots.cartesian_line(
            [self.curve, self.other],
            || line_roots(c, self.curve, self.other),
            read,
        )
    }
}

impl Group for CurveIntersection {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        Parents::of(&[self.curve, self.other])
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.curve, CURVE)?;
        c.expect(
            self.other,
            match self.method {
                Method::Line(_) => CARRIERS,
                Method::Seeds(_) => CURVE,
                Method::Point => POINT,
            },
        )
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let (x, y) = match self.method {
            Method::Line(slot) => self.with_roots(c, |roots| roots.get(slot).copied().flatten())?,
            Method::Seeds(seeds) => seeded(c, self.curve, self.other, seeds)?,
            Method::Point => {
                let p = c.point(self.other)?;
                let Value::Point { x, y } = crate::algo::path::project(c, self.curve, p)? else {
                    return None;
                };
                if (x - p.0).abs() > STANDARD_PRECISION || (y - p.1).abs() > STANDARD_PRECISION {
                    return None;
                }
                (x, y)
            }
        };
        Some(Value::Point { x, y })
    }

    fn parse(c: &Construction, name: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let ("Intersect", [first, second]) = (name, args) else {
            return None;
        };
        let (first, second) = (c.id(first)?, c.id(second)?);
        let (curve, other) = match (c.kind(first), c.kind(second)) {
            (Kind::Curve, Kind::Point) => (first, second),
            (Kind::Point, Kind::Curve) => (second, first),
            _ => return None,
        };
        Some(Ok(Self {
            curve,
            other,
            method: Method::Point,
        }))
    }
}

fn line_roots(c: &Construction, curve: Id, line: Id) -> CurveRoots {
    let Some((cartesian, carrier)) = Cartesian::new(c, curve).zip(Carrier::of(c, line)) else {
        return Vec::new();
    };
    let (start, end) = cartesian.parameter_bounds();
    let (mut a, mut b, mut constant) = carrier.line;
    if constant.abs() < STANDARD_PRECISION {
        constant = 0.0;
    } else {
        a /= constant;
        b /= constant;
        constant = 1.0;
    }
    let work = IntersectionWork::new();
    let residual = |t| {
        let (x, y) = cartesian.value(t);
        if work.charge(1) {
            a * x + b * y + constant
        } else {
            f64::NAN
        }
    };
    let mut parameters = cartesian
        .polynomial_line_roots(c, &work, (a, b, constant))
        .unwrap_or_else(|| {
            fixed_steps_roots(&residual, start, end, 100, || {
                work.exhausted() || !cartesian.has_budget()
            })
        });
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|a, b| (*a - *b).abs() < STANDARD_PRECISION);
    let points = parameters
        .into_iter()
        .map(|t| {
            if !(start..=end).contains(&t) {
                return None;
            }
            let p = cartesian.value(t);
            carrier.incident(p, MIN_PRECISION).then_some(p)
        })
        .collect();
    if work.exhausted() || !cartesian.has_budget() {
        Vec::new()
    } else {
        points
    }
}

fn seeded(c: &Construction, first: Id, second: Id, seeds: [u32; 2]) -> Option<Point> {
    let left = Cartesian::new(c, first)?;
    let right = Cartesian::new(c, second)?;
    let [Raw::Num(mut u), Raw::Num(mut v)] = [
        c.intersection_argument_value(seeds[0])?,
        c.intersection_argument_value(seeds[1])?,
    ] else {
        return None;
    };
    let mut previous = (u + 1.0, v + 1.0);
    for _ in 0..100 {
        if !(u.is_finite() && v.is_finite()) {
            return None;
        }
        if (u - previous.0).abs() <= 1e-15 && (v - previous.1).abs() <= 1e-15 {
            let (start_u, end_u) = left.parameter_bounds();
            let (start_v, end_v) = right.parameter_bounds();
            if !(start_u - STANDARD_PRECISION..=end_u + STANDARD_PRECISION).contains(&u)
                || !(start_v - STANDARD_PRECISION..=end_v + STANDARD_PRECISION).contains(&v)
            {
                return None;
            }
            // The reference's onCurve2 compares the z coordinates, both zero in 2D.
            return Some(left.value(u));
        }
        previous = (u, v);
        let (ax, ay) = left.first(u);
        let (bx, by) = right.first(v);
        let (x, y) = left.value(u);
        let (q, r) = right.value(v);
        let (dx, dy) = (x - q, y - r);
        let determinant = ax * -by - ay * -bx;
        u -= (-by * dx + bx * dy) / determinant;
        v -= (ax * dy - ay * dx) / determinant;
    }
    None
}
