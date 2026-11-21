//! The elements of list objects (`{1, 2, 3}`, `Sequence(n)`).
//!
//! A list keeps its elements in its expression's entry, flattened: the
//! top-level elements come first, and an element that is itself a list
//! names the run of slots, further on in the same buffer, that holds its
//! elements. The object's [`Value::List`] only carries the length, so values
//! stay `Copy` and of fixed size; [`List`] reads the elements.

use crate::kernel::{Kind, Value};

/// One element of a list: its kind (also when it is undefined), its value,
/// and for a nested list where its elements start.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Slot {
    pub(crate) kind: Kind,
    pub(crate) value: Value,
    pub(crate) start: u32,
}

impl Slot {
    pub(crate) fn undefined(kind: Kind) -> Slot {
        Slot {
            kind,
            value: Value::Undefined,
            start: 0,
        }
    }

    /// A number element; one that is not finite shows as undefined, like a
    /// number object.
    pub(crate) fn number(v: f64) -> Slot {
        if v.is_finite() {
            Slot {
                kind: Kind::Number,
                value: Value::Number(v),
                start: 0,
            }
        } else {
            Slot::undefined(Kind::Number)
        }
    }

    /// A point or vector element (`kind`).
    pub(crate) fn coords(kind: Kind, x: f64, y: f64) -> Slot {
        if !(x.is_finite() && y.is_finite()) {
            return Slot::undefined(kind);
        }
        let value = if kind == Kind::Vector {
            Value::Vector { x, y }
        } else {
            Value::Point { x, y }
        };
        Slot {
            kind,
            value,
            start: 0,
        }
    }

    pub(crate) fn boolean(b: bool) -> Slot {
        Slot {
            kind: Kind::Boolean,
            value: Value::Bool(b),
            start: 0,
        }
    }

    /// A nested list of `len` elements from `start` on.
    pub(crate) fn list(start: u32, len: u32) -> Slot {
        Slot {
            kind: Kind::List,
            value: Value::List { len },
            start,
        }
    }

    /// The run of a nested list, if this is one.
    pub(crate) fn run(&self) -> Option<(u32, u32)> {
        match self.value {
            Value::List { len } => Some((self.start, len)),
            _ => None,
        }
    }

    /// Bit equality, so that a change from `0` to `-0` is seen.
    fn same(&self, other: &Slot) -> bool {
        let bits = |v: &Value| -> [u64; 2] {
            match *v {
                Value::Number(x) => [x.to_bits(), 0],
                Value::Point { x, y } | Value::Vector { x, y } => [x.to_bits(), y.to_bits()],
                Value::Bool(b) => [u64::from(b), 0],
                Value::List { len } => [u64::from(len), 0],
                _ => [u64::MAX, u64::MAX],
            }
        };
        self.kind == other.kind
            && self.start == other.start
            && std::mem::discriminant(&self.value) == std::mem::discriminant(&other.value)
            && bits(&self.value) == bits(&other.value)
    }
}

/// Whether two flattened lists are the same, bit for bit.
pub(crate) fn same_slots(a: &[Slot], b: &[Slot]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y))
}

/// Copies the list of `len` elements at `start` in `from`, with the lists
/// nested in it, to the end of `to`, where it starts; nothing allocates once
/// `to` has the capacity. The runs are copied breadth first.
pub(crate) fn copy_tree(from: &[Slot], start: u32, len: u32, to: &mut Vec<Slot>) -> u32 {
    let base = to.len();
    let range = start as usize..(start + len) as usize;
    to.extend_from_slice(&from[range]);
    let mut i = base;
    while i < to.len() {
        if let Some((s, n)) = to[i].run() {
            to[i].start = to.len() as u32;
            to.extend_from_slice(&from[s as usize..(s + n) as usize]);
        }
        i += 1;
    }
    base as u32
}

/// The elements of a list object, read with
/// [`Construction::elements`](crate::Construction::elements).
#[derive(Clone, Copy, Debug)]
pub struct List<'a> {
    slots: &'a [Slot],
    start: u32,
    len: u32,
}

/// One element of a [`List`].
#[derive(Clone, Copy, Debug)]
pub enum Element<'a> {
    /// A number, point, vector or boolean, with its kind; an undefined
    /// element (of any kind, a list too) is [`Value::Undefined`].
    Value(Kind, Value),
    /// A defined list.
    List(List<'a>),
}

/// The shape of a coordinate list. Compute this once per list value so
/// each output of `Point(list)` can read its row in constant time.
#[derive(Clone, Copy)]
pub(super) enum CoordinateLayout {
    Flat,
    Nested,
}

impl CoordinateLayout {
    pub(super) fn of(list: List<'_>) -> Option<Self> {
        let numeric = |row: List<'_>| {
            row.iter()
                .all(|item| matches!(item, Element::Value(Kind::Number | Kind::Angle, _)))
        };
        match list.get(0)? {
            Element::List(_)
                if list
                    .iter()
                    .all(|item| matches!(item, Element::List(row) if numeric(row))) =>
            {
                Some(Self::Nested)
            }
            Element::Value(Kind::Number | Kind::Angle, _) if numeric(list) => Some(Self::Flat),
            _ => None,
        }
    }
}

pub(crate) struct CoordinateRows<'a> {
    list: List<'a>,
    layout: CoordinateLayout,
}

impl<'a> CoordinateRows<'a> {
    pub(super) fn new(list: List<'a>, layout: CoordinateLayout) -> Self {
        Self { list, layout }
    }

    pub(crate) fn len(&self) -> usize {
        match self.layout {
            CoordinateLayout::Flat => 1,
            CoordinateLayout::Nested => self.list.len(),
        }
    }

    pub(crate) fn row(&self, slot: usize) -> Option<List<'a>> {
        match self.layout {
            CoordinateLayout::Flat => (slot == 0).then_some(self.list),
            CoordinateLayout::Nested => match self.list.get(slot)? {
                Element::List(row) => Some(row),
                _ => None,
            },
        }
    }
}

impl<'a> List<'a> {
    pub(crate) fn new(slots: &'a [Slot], len: u32) -> List<'a> {
        Self::from_run(slots, 0, len)
    }

    pub(super) fn from_run(slots: &'a [Slot], start: u32, len: u32) -> List<'a> {
        List { slots, start, len }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The element at `k`, from 0.
    pub fn get(&self, k: usize) -> Option<Element<'a>> {
        if k >= self.len() {
            return None;
        }
        let slot = self.slots[self.start as usize + k];
        Some(match slot.run() {
            Some((start, len)) => Element::List(List {
                slots: self.slots,
                start,
                len,
            }),
            None => Element::Value(slot.kind, slot.value),
        })
    }

    /// The elements in order.
    pub fn iter(&self) -> impl Iterator<Item = Element<'a>> + 'a {
        let list = *self;
        (0..list.len()).filter_map(move |k| list.get(k))
    }
}
