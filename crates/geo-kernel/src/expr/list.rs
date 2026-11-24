//! The elements of list objects (`{1, 2, 3}`, `Sequence(n)`).
//!
//! A list keeps its elements in its expression's entry, flattened: the
//! top-level elements come first, and an element that is itself a list
//! names the run of slots, further on in the same buffer, that holds its
//! elements. The object's [`Value::List`] only carries the length. The
//! slots can own text, and [`List`] reads the elements from the buffer.

use crate::kernel::{Construction, Id, Kind, Value};
use std::sync::Arc;

// The largest two ids belong to the axes. Ordinary ids index vectors.
const NO_OBJECT: Id = Id(usize::MAX - 2);

/// One element of a list: its kind (also when it is undefined), its value,
/// and for a nested list where its elements start.
#[derive(Clone, Debug)]
pub(crate) struct Slot {
    pub(crate) kind: Kind,
    pub(crate) value: Value,
    pub(crate) start: u32,
    /// A named geometric object whose equality uses its construction data.
    object: Id,
    equation_revision: u64,
    extra: Option<Arc<SlotExtra>>,
}

#[derive(Debug, PartialEq)]
enum SlotExtra {
    Text(String),
    ListShape(Arc<[Kind]>),
    InfiniteCircle,
    Polynomial(crate::cas::Poly),
    Symbolic(String),
    /// A function the Giac backend read from its output (the `giac` feature).
    #[cfg(feature = "giac")]
    Giac(super::cas::GiacFunction),
}

impl Slot {
    /// The numeric field, including NaN and infinity.
    pub(super) fn numeric_cell(&self) -> Option<f64> {
        match (self.kind, self.value, self.start) {
            (_, Value::Number(v), _) => Some(v),
            (Kind::Number, Value::Undefined, 1) => Some(f64::INFINITY),
            (Kind::Number, Value::Undefined, 2) => Some(f64::NEG_INFINITY),
            (Kind::Number | Kind::Angle, Value::Undefined, _) => Some(f64::NAN),
            _ => None,
        }
    }

    pub(crate) fn undefined(kind: Kind) -> Slot {
        Slot {
            kind,
            value: Value::Undefined,
            start: 0,
            object: NO_OBJECT,
            equation_revision: 0,
            extra: None,
        }
    }

    pub(super) fn undefined_list(shape: Arc<[Kind]>) -> Slot {
        Slot {
            extra: Some(Arc::new(SlotExtra::ListShape(shape))),
            ..Self::undefined(Kind::List)
        }
    }

    pub(super) fn list_shape(&self) -> Option<&[Kind]> {
        match self.extra.as_deref()? {
            SlotExtra::ListShape(shape) => Some(shape),
            _ => None,
        }
    }

    pub(crate) fn with_value(kind: Kind, value: Value) -> Slot {
        Self {
            value,
            ..Self::undefined(kind)
        }
    }

    /// A number element. An infinite one keeps its value, like a number
    /// object (`GeoNumeric.isDefined` only refuses NaN).
    pub(crate) fn number(v: f64) -> Slot {
        if v.is_nan() {
            Slot::undefined(Kind::Number)
        } else {
            Slot::with_value(Kind::Number, Value::Number(v))
        }
    }

    /// A point or vector element (`kind`).
    pub(crate) fn coords(kind: Kind, x: f64, y: f64) -> Slot {
        if x.is_nan() || y.is_nan() {
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
            object: NO_OBJECT,
            equation_revision: 0,
            extra: None,
        }
    }

    pub(crate) fn line(value: Value) -> Slot {
        debug_assert!(matches!(value, Value::Line { .. }));
        Slot {
            value,
            ..Slot::undefined(Kind::Line)
        }
    }

    pub(crate) fn point(value: Value) -> Slot {
        debug_assert!(matches!(
            value,
            Value::Point { .. } | Value::HomogeneousPoint { .. } | Value::Undefined
        ));
        Slot {
            kind: Kind::Point,
            value,
            start: 0,
            object: NO_OBJECT,
            equation_revision: 0,
            extra: None,
        }
    }

    pub(crate) fn text(value: impl Into<String>, object: Option<Id>) -> Slot {
        Slot {
            kind: Kind::Text,
            value: Value::Text,
            start: 0,
            object: object.unwrap_or(NO_OBJECT),
            equation_revision: 0,
            extra: Some(Arc::new(SlotExtra::Text(value.into()))),
        }
    }

    pub(crate) fn text_value(&self) -> Option<&str> {
        match self.extra.as_deref()? {
            SlotExtra::Text(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn polynomial(value: crate::cas::Poly) -> Slot {
        Slot {
            value: Value::Function,
            extra: Some(Arc::new(SlotExtra::Polynomial(value))),
            ..Self::undefined(Kind::Function)
        }
    }

    #[cfg(feature = "giac")]
    pub(super) fn giac_function(value: super::cas::GiacFunction) -> Slot {
        Slot {
            value: Value::Function,
            extra: Some(Arc::new(SlotExtra::Giac(value))),
            ..Self::undefined(Kind::Function)
        }
    }

    pub(super) fn with_symbolic(mut self, text: String) -> Self {
        self.extra = Some(Arc::new(SlotExtra::Symbolic(text)));
        self
    }

    pub(crate) fn object_id(&self) -> Option<Id> {
        (self.object != NO_OBJECT).then_some(self.object)
    }

    pub(crate) fn with_object(mut self, object: Id) -> Slot {
        self.object = object;
        self
    }

    pub(super) fn detach_graph_object(mut self) -> Self {
        if matches!(self.kind, Kind::Point | Kind::Segment) {
            self.object = NO_OBJECT;
        }
        self
    }

    pub(crate) fn shown(&self) -> Value {
        self.value.cartesian()
    }

    pub(crate) fn boolean(b: bool) -> Slot {
        Slot {
            kind: Kind::Boolean,
            value: Value::Bool(b),
            start: 0,
            object: NO_OBJECT,
            equation_revision: 0,
            extra: None,
        }
    }

    /// A nested list of `len` elements from `start` on.
    pub(crate) fn list(start: u32, len: u32) -> Slot {
        Slot {
            kind: Kind::List,
            value: Value::List { len },
            start,
            object: NO_OBJECT,
            equation_revision: 0,
            extra: None,
        }
    }

    pub(super) fn object(c: &Construction, id: Id) -> Self {
        let kind = c.kind(id);
        let equation_revision = match kind {
            Kind::Line | Kind::Ray | Kind::Circle | Kind::Conic => c
                .equation_display_revision(id)
                .map_or(0, |rev| rev.wrapping_add(1)),
            _ => 0,
        };
        Self {
            kind,
            value: c.value(id),
            start: 0,
            object: id,
            equation_revision,
            extra: (kind == Kind::Circle && c.has_infinite_circle_center(id))
                .then(|| Arc::new(SlotExtra::InfiniteCircle)),
        }
    }

    pub(super) fn is_defined(&self) -> bool {
        self.value.is_defined() || matches!(self.extra.as_deref(), Some(SlotExtra::InfiniteCircle))
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
        if self.extra != other.extra
            || self.object != other.object
            || self.equation_revision != other.equation_revision
        {
            return false;
        }
        if self.object != NO_OBJECT
            && (self.value != other.value
                || self.kind == Kind::Line && !self.value.is_defined()
                || matches!(
                    self.kind,
                    Kind::Conic | Kind::Curve | Kind::Function | Kind::FunctionNVar
                ))
        {
            // An undefined line retains coefficients used by equality. Functions,
            // curves, and conics can change data held outside their value.
            return false;
        }
        if matches!(self.value, Value::Segment { .. })
            || matches!(other.value, Value::Segment { .. })
        {
            return self.value == other.value;
        }
        let bits = |v: &Value| -> [u64; 3] {
            match *v {
                Value::Number(x) => [x.to_bits(), 0, 0],
                Value::Line { a, b, c } => [a.to_bits(), b.to_bits(), c.to_bits()],
                Value::Point { x, y } | Value::Vector { x, y } => [x.to_bits(), y.to_bits(), 0],
                Value::HomogeneousPoint { x, y, z } => [x.to_bits(), y.to_bits(), z.to_bits()],
                Value::Bool(b) => [u64::from(b), 0, 0],
                Value::List { len } => [u64::from(len), 0, 0],
                _ => [u64::MAX; 3],
            }
        };
        if let (
            Value::Segment { x1, y1, x2, y2 },
            Value::Segment {
                x1: a,
                y1: b,
                x2: c,
                y2: d,
            },
        ) = (self.value, other.value)
        {
            return [x1, y1, x2, y2].map(f64::to_bits) == [a, b, c, d].map(f64::to_bits);
        }
        self.kind == other.kind
            && self.start == other.start
            && std::mem::discriminant(&self.value) == std::mem::discriminant(&other.value)
            && bits(&self.value) == bits(&other.value)
    }
}

/// Named geometric objects supported as entries in a literal list.
pub(super) fn object_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Arc
            | Kind::Circle
            | Kind::Conic
            | Kind::Curve
            | Kind::Line
            | Kind::Locus
            | Kind::Polygon
            | Kind::PolyLine
            | Kind::Ray
            | Kind::Sector
            | Kind::Segment
    )
}

/// Parents whose changes can alter an undefined named line's equality.
pub(super) fn object_parents(c: &Construction, id: Id) -> Vec<Id> {
    let mut parents = Vec::new();
    if c.kind(id) == Kind::Line {
        crate::algo::vector::line_hidden(c, id, &mut parents);
    }
    parents
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
    /// A non-list element and its kind; an undefined element is
    /// [`Value::Undefined`].
    Value(Kind, Value),
    /// The exact contents of a defined text.
    Text(&'a str),
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
        let slot = &self.slots[self.start as usize + k];
        Some(match slot.run() {
            Some((start, len)) => Element::List(List {
                slots: self.slots,
                start,
                len,
            }),
            None => slot
                .text_value()
                .map_or_else(|| Element::Value(slot.kind, slot.shown()), Element::Text),
        })
    }

    /// The original object when the element is a named geometry object.
    pub(crate) fn object(&self, k: usize) -> Option<Id> {
        (k < self.len())
            .then(|| self.slots[self.start as usize + k].object_id())
            .flatten()
    }

    /// The stored point value, including homogeneous coordinates.
    pub(crate) fn raw_point(&self, k: usize) -> Option<Value> {
        if k >= self.len() {
            return None;
        }
        let slot = &self.slots[self.start as usize + k];
        (slot.kind == Kind::Point).then_some(slot.value)
    }

    /// The construction object supplying the function at this list position.
    pub fn function_at(&self, k: usize) -> Option<Id> {
        if k >= self.len() {
            return None;
        }
        let slot = &self.slots[self.start as usize + k];
        (slot.kind == Kind::Function)
            .then(|| slot.object_id())
            .flatten()
    }

    /// Samples a named function or an inline polynomial held by the list.
    pub fn function_sample_at(&self, c: &Construction, k: usize, x: f64) -> Option<f64> {
        if k >= self.len() {
            return None;
        }
        let slot = &self.slots[self.start as usize + k];
        match slot.extra.as_deref() {
            Some(SlotExtra::Polynomial(p)) => Some(p.value([x, f64::NAN, f64::NAN])),
            #[cfg(feature = "giac")]
            Some(SlotExtra::Giac(f)) => Some(f.value(x)),
            _ => c.function_value(self.function_at(k)?, x),
        }
    }

    /// Exact expression displayed for an inline polynomial or solution.
    pub fn symbolic_at(&self, k: usize) -> Option<String> {
        if k >= self.len() {
            return None;
        }
        match self.slots[self.start as usize + k].extra.as_deref()? {
            SlotExtra::Polynomial(value) => Some(super::cas::print_poly(value)),
            SlotExtra::Symbolic(text) => Some(text.clone()),
            #[cfg(feature = "giac")]
            SlotExtra::Giac(f) => Some(f.text().to_string()),
            _ => None,
        }
    }

    /// The raw numeric field read by GgbMat, including NaN and infinity.
    pub(crate) fn numeric_cell(&self, k: usize) -> Option<f64> {
        if k >= self.len() {
            return None;
        }
        self.slots[self.start as usize + k].numeric_cell()
    }

    /// The elements in order.
    pub fn iter(&self) -> impl Iterator<Item = Element<'a>> + 'a {
        let list = *self;
        (0..list.len()).filter_map(move |k| list.get(k))
    }
}
