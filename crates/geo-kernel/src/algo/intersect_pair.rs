//! A conic pair and its labelled, indexed or nearest output.

use super::{
    circle_points,
    conics::{on_part, Intersection, SavedSlots},
    general_conic_points, on_circle_by_construction, on_general_conic_by_construction, other_point,
    shared_point_case, Conic, Pt,
};
use crate::algo::{
    arc::ArcData,
    conic_nd::{ConicData, ConicType},
    Group, Parents, CONIC_OBJECTS, POINT,
};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::tol::STANDARD_PRECISION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairDef {
    pub(crate) first: Id,
    pub(crate) second: Id,
    pub(crate) output: Output,
    saved: Option<SavedPair>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SavedPair {
    slots: SavedSlots,
    special: bool,
    nearest: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Output {
    Slot(usize),
    Index(usize),
    Number(Id),
    Nearest(Id),
}

fn index(n: f64) -> usize {
    (n as i32).wrapping_sub(1).max(0) as usize
}

impl PairDef {
    pub(crate) fn with_state(self, state: &PairState) -> Self {
        Self {
            saved: Some(SavedPair {
                slots: state.intersection.saved_slots(),
                special: state.special,
                nearest: state.nearest as u8,
            }),
            ..self
        }
    }

    pub(crate) fn with_slot(self, slot: usize) -> Self {
        Self {
            output: Output::Slot(slot),
            ..self
        }
    }

    pub(crate) fn matches(self, other: Self) -> bool {
        (self.first, self.second) == (other.first, other.second)
            || (self.first, self.second) == (other.second, other.first)
    }

    pub(crate) fn oriented(self, c: &Construction) -> Self {
        for &id in c.children(self.first) {
            let (first, second) = match c.def(id) {
                Def::IntersectConics(d) => (d.first, d.second),
                Def::IntersectCircles { c1, c2, .. } => (c1, c2),
                _ => continue,
            };
            if (first, second) == (self.first, self.second)
                || (first, second) == (self.second, self.first)
            {
                return Self {
                    first,
                    second,
                    ..self
                };
            }
        }
        self
    }

    pub(crate) fn previous(self, c: &Construction) -> Option<Id> {
        c.children(self.first)
            .iter()
            .rev()
            .copied()
            .find(|&id| matches!(c.def(id), Def::IntersectConics(d) if self.matches(d)))
    }

    pub(crate) fn common_points(self, c: &Construction) -> impl Iterator<Item = Id> + '_ {
        let shared = |a, b| {
            let points = if c.kind(a) == Kind::Circle {
                let [p, q, r] = circle_points(c, a);
                [p, q, r, None, None]
            } else {
                general_conic_points(c, a)
            };
            points.into_iter().flatten().filter(move |&p| {
                if c.kind(b) == Kind::Circle {
                    on_circle_by_construction(c, p, b)
                } else {
                    on_general_conic_by_construction(c, p, b)
                }
            })
        };
        shared(self.first, self.second).chain(shared(self.second, self.first))
    }

    pub(crate) fn create(
        self,
        c: &Construction,
        previous: Option<&PairState>,
        continuous: bool,
    ) -> PairState {
        let fixed_circles = self.plain_circles(c) && !continuous;
        if let Some(saved) = self.saved {
            let mut state = PairState {
                special: saved.special,
                nearest: usize::from(saved.nearest),
                common: self.common_array(c),
                ..PairState::default()
            };
            state.intersection.restore_slots(saved.slots);
            if fixed_circles {
                state.intersection.fixed_order();
            }
            state.refresh(self, c, continuous);
            return state;
        }
        let mut state = previous.cloned().unwrap_or_default();
        state.nearest = 0;
        if fixed_circles {
            state.intersection.fixed_order();
        }
        state.refresh(self, c, continuous);
        if previous.is_none() {
            state.common = self.common_array(c);
            state.special = state.circle_intersections(c).is_some();
        }
        if let Output::Slot(slot) = self.output {
            state.intersection.label(slot);
        } else if !fixed_circles {
            state.intersection.reset_permutation();
            state.recompute(continuous);
        }
        state
    }

    fn common_array(self, c: &Construction) -> [Option<Id>; 10] {
        let mut common = [None; 10];
        for (slot, id) in common.iter_mut().zip(self.common_points(c)) {
            *slot = Some(id);
        }
        common
    }

    fn plain_circles(self, c: &Construction) -> bool {
        c.kind(self.first) == Kind::Circle && c.kind(self.second) == Kind::Circle
    }

    pub(crate) fn multi(c: &Construction, func: &str, args: &[&str]) -> Option<Self> {
        let ("Intersect", [first, second]) = (func, args) else {
            return None;
        };
        let pair = Self::pair(c, first, second, Output::Slot(0))?;
        // A circle with itself has at most its first incident point. Keep
        // the original definition and its creation-time incidence flag.
        (pair.first != pair.second || !pair.plain_circles(c)).then_some(pair)
    }

    fn pair(c: &Construction, first: &str, second: &str, output: Output) -> Option<Self> {
        let (first, second) = (c.id(first)?, c.id(second)?);
        (CONIC_OBJECTS.contains(&c.kind(first)) && CONIC_OBJECTS.contains(&c.kind(second)))
            .then_some(Self {
                first,
                second,
                output,
                saved: None,
            })
    }
}

impl Group for PairDef {
    fn kind(&self) -> Kind {
        Kind::Point
    }

    fn parents(&self) -> Parents {
        match self.output {
            Output::Number(id) | Output::Nearest(id) => Parents::of(&[self.first, self.second, id]),
            Output::Slot(_) | Output::Index(_) => Parents::of(&[self.first, self.second]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        c.expect(self.first, CONIC_OBJECTS)?;
        c.expect(self.second, CONIC_OBJECTS)?;
        match self.output {
            Output::Number(id) => c.expect(id, &[Kind::Number]),
            Output::Nearest(id) => c.expect(id, POINT),
            Output::Slot(slot) if slot >= 4 => Err(Error::BadIndex(slot as u8)),
            _ => Ok(()),
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        c.preview_conic_intersection(*self)
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let ("Intersect", [first, second, selector]) = (func, args) else {
            return None;
        };
        let mut pair = Self::pair(c, first, second, Output::Index(0))?;
        if pair.plain_circles(c) && matches!(*selector, "1" | "2") && pair.previous(c).is_none() {
            // Keep the original indexed circle definition used by callers.
            return None;
        }
        pair.output = match c.id(selector).map(|id| (id, c.kind(id))) {
            Some((id, Kind::Point)) => Output::Nearest(id),
            Some((id, kind)) if kind.is_number() => Output::Number(id),
            _ => match crate::expr::plain_number(selector) {
                Some(n) => Output::Index(index(n)),
                None => {
                    return Some(Err(format!(
                        "expected an intersection index or point, got `{selector}`"
                    )))
                }
            },
        };
        Some(Ok(pair))
    }
}

#[derive(Clone, Copy, Debug)]
enum Operand {
    Full(ConicData),
    Limited(ArcData),
}

impl Operand {
    fn get(c: &Construction, id: Id) -> Option<Self> {
        if let Some(part) = c.arc(id) {
            part.defined.then_some(Self::Limited(part))
        } else {
            c.conic(id).filter(|d| d.defined).map(Self::Full)
        }
    }

    fn conic(&self) -> &ConicData {
        match self {
            Self::Full(data) => data,
            Self::Limited(part) => &part.conic,
        }
    }

    fn contains(&self, point: (f64, f64)) -> bool {
        match self {
            Self::Full(_) => true,
            Self::Limited(part) => on_part(part, point),
        }
    }

    fn same(a: Option<Self>, b: Option<Self>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(Self::Full(a)), Some(Self::Full(b))) => a.same(&b),
            (Some(Self::Limited(a)), Some(Self::Limited(b))) => a.same(&b),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PairState {
    operands: [Option<Operand>; 2],
    intersection: Intersection,
    nearest: usize,
    common: [Option<Id>; 10],
    special: bool,
}

impl PairState {
    fn refresh(&mut self, def: PairDef, c: &Construction, continuous: bool) {
        let operands = [Operand::get(c, def.first), Operand::get(c, def.second)];
        if !Operand::same(operands[0], self.operands[0])
            || !Operand::same(operands[1], self.operands[1])
        {
            self.operands = operands;
            self.recompute(continuous);
        }
    }

    fn recompute(&mut self, continuous: bool) {
        if let [Some(a), Some(b)] = &self.operands {
            self.intersection
                .update(a.conic(), b.conic(), continuous, |point| {
                    a.contains(point) && b.contains(point)
                });
        } else {
            self.intersection.undefined();
        }
    }

    pub(crate) fn update(
        &mut self,
        def: PairDef,
        c: &Construction,
        continuous: bool,
    ) -> Option<Value> {
        self.refresh(def, c, continuous);
        self.value(def, c)
    }

    pub(crate) fn value(&mut self, def: PairDef, c: &Construction) -> Option<Value> {
        let points = if self.special {
            self.circle_intersections(c)
                .unwrap_or_else(|| self.intersection.current())
        } else {
            self.intersection.current()
        };
        let slot = match def.output {
            Output::Slot(slot) | Output::Index(slot) => slot,
            Output::Number(id) => index(c.number(id).unwrap_or(f64::NAN)),
            Output::Nearest(id) => {
                if let Some(seed) = c.point(id) {
                    self.nearest = closest(points, seed);
                }
                self.nearest
            }
        };
        let (x, y) = (*points.get(slot)?)?;
        Some(Value::Point { x, y })
    }

    fn circle_intersections(&self, c: &Construction) -> Option<[Pt; 4]> {
        let [Some(Operand::Full(a)), Some(Operand::Full(b))] = &self.operands else {
            return None;
        };
        if a.ty != ConicType::Circle || b.ty != ConicType::Circle {
            return None;
        }
        let first = self.common.into_iter().flatten().find_map(|id| {
            let p = c.point(id)?;
            (a.on_full_conic(p, STANDARD_PRECISION) && b.on_full_conic(p, STANDARD_PRECISION))
                .then_some(p)
        });
        let first = Some(first?);
        let circle = |d: &ConicData| Conic::new(d.midpoint[0], d.midpoint[1], d.half_axes[0]);
        let other = other_point(&shared_point_case(&circle(a), &circle(b)), first);
        Some([first, other, None, None])
    }
}

fn closest(points: [Pt; 4], (x, y): (f64, f64)) -> usize {
    let mut best = 0;
    let mut distance = f64::INFINITY;
    for (i, (u, v)) in points
        .into_iter()
        .enumerate()
        .filter_map(|(i, p)| Some((i, p?)))
    {
        let d = (u - x) * (u - x) + (v - y) * (v - y);
        if distance - d > STANDARD_PRECISION {
            distance = d;
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use crate::kernel::Value;
    use crate::script;

    #[test]
    fn continuous_slots_keep_their_branches_after_tangency() {
        let mut c = script::load(concat!(
            "A = (0, 0)\nB = (6, 0)\nr = 5\n",
            "k = Circle(A, r)\nh = Circle(B, r)\nP = Intersect(k, h)",
        ))
        .unwrap();
        let b = c.id("B").unwrap();
        let p = c.id("P_{1}").unwrap();
        let q = c.id("P_{2}").unwrap();
        let above = |value| matches!(value, Value::Point { y, .. } if y > 0.0);
        assert!(above(c.value(p)));
        assert!(!above(c.value(q)));
        c.set_conic_intersections_continuous(true);
        c.move_point(b, 10.0, 0.0).unwrap();
        assert_eq!(c.value(p), c.value(q));
        c.move_point(b, 11.0, 0.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        c.move_point(b, -6.0, 0.0).unwrap();
        assert!(above(c.value(p)));
        assert!(matches!(c.value(q), Value::Point { y, .. } if y < 0.0));
    }

    #[test]
    fn rolled_back_pair_leaves_no_state_attached_to_the_reused_ids() {
        let mut c =
            script::load("k = Conic({1, 4, -16, 0, 0, 0})\nh = Conic({4, 1, -12, 0, 0, 0})")
                .unwrap();
        let before = c.checkpoint();
        let p = script::exec(&mut c, "P = Intersect(k, h, 1)")
            .unwrap()
            .unwrap();
        c.rollback(before).unwrap();
        let q = script::exec(&mut c, "Q = Intersect(h, k, 3)")
            .unwrap()
            .unwrap();
        assert_eq!(p, q);
        let fresh = script::load(concat!(
            "k = Conic({1, 4, -16, 0, 0, 0})\nh = Conic({4, 1, -12, 0, 0, 0})\n",
            "Q = Intersect(h, k, 3)",
        ))
        .unwrap();
        assert_eq!(c.value(q), fresh.value(fresh.id("Q").unwrap()));
    }
}
