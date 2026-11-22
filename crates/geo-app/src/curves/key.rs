//! What a curve's drawing is made from, apart from the view: the key under
//! which `Curves` keeps it.
//!
//! - a conic: its matrix and type, and the order of its two lines when it
//!   is a pair of lines (that order follows the conic's history and decides
//!   where the label goes);
//! - a function: its definition and those of the functions it reads (a
//!   fingerprint that names other objects by label, so it survives a
//!   rebuild that renumbers them), then the values of every object it
//!   depends on, directly or not (`Construction::reads`), the elements of
//!   the lists, the matrices of the conics and the parameters of the arcs
//!   and sectors among them included.
//!
//! Comparing a key with what the construction holds now allocates nothing;
//! a new key is made only when they differ.

use geo_kernel::expr::Element;
use geo_kernel::kernel::{Construction, Id, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, PartialEq)]
pub(super) enum Data {
    /// The bits of a conic's matrix, its type and its two lines.
    Conic([u64; 6], u8, [u64; 6]),
    Part([u64; 11]),
    Implicit(u64),
    /// The fingerprints of a function's definitions ([`Closure::prints`]),
    /// then the bits of the values it depends on.
    Function(Vec<u64>),
}

/// What a function depends on, found once per rebuild.
pub(super) struct Closure {
    /// Every object it depends on, directly or not, each once, in a fixed
    /// order.
    ids: Vec<Id>,
    /// The fingerprint of its own definition, then of each function among
    /// `ids`.
    prints: Vec<u64>,
}

impl Closure {
    pub(super) fn of(c: &Construction, id: Id) -> Closure {
        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut stack = c.reads(id);
        while let Some(a) = stack.pop() {
            if a != id && seen.insert(a) {
                ids.push(a);
                stack.extend(c.reads(a));
            }
        }
        let prints = std::iter::once(id)
            .chain(
                ids.iter()
                    .copied()
                    .filter(|&a| matches!(c.value(a), Value::Function)),
            )
            .map(|a| fingerprint(c, a, &ids, id))
            .collect();
        Closure { ids, prints }
    }

    /// Whether the construction gives `bits` now.
    pub(super) fn same(&self, c: &Construction, bits: &[u64]) -> bool {
        let n = self.prints.len();
        if bits.len() < n || bits[..n] != self.prints[..] {
            return false;
        }
        let mut i = n;
        let mut same = true;
        for &a in &self.ids {
            feed(c, a, &mut |b| {
                same &= bits.get(i) == Some(&b);
                i += 1;
            });
            if !same {
                return false;
            }
        }
        i == bits.len()
    }

    /// The key data of the function now.
    pub(super) fn data(&self, c: &Construction) -> Data {
        let mut bits = self.prints.clone();
        for &a in &self.ids {
            feed(c, a, &mut |b| bits.push(b));
        }
        Data::Function(bits)
    }
}

/// The key data of a conic.
pub(super) fn conic_data(d: &geo_kernel::ConicData) -> Data {
    let [l0, l1] = d.lines;
    let lines = [l0[0], l0[1], l0[2], l1[0], l1[1], l1[2]];
    Data::Conic(
        d.matrix.map(f64::to_bits),
        d.ty as u8,
        lines.map(f64::to_bits),
    )
}

/// A hash of the definition of the function `a`: its expression tree
/// (or its definition), with every object it names written as that
/// object's label instead of its index. Two definitions with the same
/// fingerprint read the same objects (by label) the same way; what those
/// objects hold is compared by value. A definition the fingerprint cannot
/// tell from another costs no more than a plot made again, because the
/// values of everything it reads are part of the key too.
fn fingerprint(c: &Construction, a: Id, ids: &[Id], own: Id) -> u64 {
    let text = match c.expression(a) {
        Some(e) => format!("{e:?}"),
        None => format!("{:?}", c.def(a)),
    };
    let label = |n: usize| {
        std::iter::once(own)
            .chain(ids.iter().copied())
            .find(|x| x.index() == n)
            .map(|x| c.label(x))
    };
    let mut h = DefaultHasher::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("Id(") {
        h.write(&rest.as_bytes()[..at]);
        let after = &rest[at + 3..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        match (
            after[..digits].parse::<usize>().ok().and_then(label),
            after[digits..].starts_with(')'),
        ) {
            (Some(l), true) => {
                h.write(b"\x01");
                h.write(l.as_bytes());
                h.write(b"\x02");
                rest = &after[digits + 1..];
            }
            _ => {
                h.write(b"Id(");
                rest = after;
            }
        }
    }
    h.write(rest.as_bytes());
    h.finish()
}

/// A value that cannot be compared here is never the same as before.
/// Geometry of a part, including the interval (its measure alone can stay
/// the same when the arc moves) and the endpoints of a straight part.
pub(super) fn part_data(part: &geo_kernel::algo::arc::ArcData) -> Data {
    let d = &part.conic;
    let [a, b] = d.half_axes;
    let [x, y] = d.midpoint;
    let [[ux, uy], [vx, vy]] = d.eigenvec;
    let flags = u64::from(d.ty as u8)
        | (u64::from(part.defined) << 8)
        | (u64::from(part.positive) << 9)
        | (u64::from(part.sector) << 10);
    let coordinates = if let Some([(ax, ay), (bx, by)]) = part.segment() {
        [ax, ay, bx, by, 0.0, 0.0, 0.0, 0.0]
    } else {
        [x, y, a, b, ux, uy, vx, vy]
    };
    let mut bits = [0; 11];
    bits[..8].copy_from_slice(&coordinates.map(f64::to_bits));
    bits[8] = flags;
    bits[9] = part.start.to_bits();
    bits[10] = part.extent.to_bits();
    Data::Part(bits)
}

static FRESH: AtomicU64 = AtomicU64::new(0);

/// Hands the bits of the value of `id` to `out`, a tag first.
fn feed(c: &Construction, id: Id, out: &mut dyn FnMut(u64)) {
    match c.value(id) {
        Value::Conic { ty } => match c.conic(id) {
            Some(d) => {
                out(11 + ((ty as u64) << 8));
                d.matrix.iter().for_each(|v| out(v.to_bits()));
            }
            None => out(11),
        },
        Value::Arc { .. } | Value::Sector { .. } => match c.arc(id) {
            Some(a) => {
                out(18 + ((a.sector as u64) << 8) + ((a.positive as u64) << 9));
                a.conic.matrix.iter().for_each(|v| out(v.to_bits()));
                [a.start, a.end, a.extent, a.value]
                    .iter()
                    .for_each(|v| out(v.to_bits()));
            }
            None => feed_value(&c.value(id), out),
        },
        Value::List { len } => match c.elements(id) {
            Some(list) => {
                out(15 + ((len as u64) << 8));
                feed_list(list, out);
            }
            None => out(15),
        },
        Value::Text => feed_text(c.text(id).unwrap_or_default(), out),
        v => feed_value(&v, out),
    }
}

/// The elements of a list, nested lists in brackets of tags.
fn feed_list(list: geo_kernel::expr::List<'_>, out: &mut dyn FnMut(u64)) {
    for e in list.iter() {
        match e {
            Element::Value(_, v) => feed_value(&v, out),
            Element::Text(text) => feed_text(text, out),
            Element::List(inner) => {
                out(16 + ((inner.len() as u64) << 8));
                feed_list(inner, out);
                out(17);
            }
        }
    }
}

fn feed_text(text: &str, out: &mut dyn FnMut(u64)) {
    out(21 + ((text.len() as u64) << 8));
    for chunk in text.as_bytes().chunks(8) {
        let mut bytes = [0; 8];
        bytes[..chunk.len()].copy_from_slice(chunk);
        out(u64::from_le_bytes(bytes));
    }
}

/// The bits of a value that carries its numbers.
fn feed_value(v: &Value, out: &mut dyn FnMut(u64)) {
    let mut all = |tag: u64, values: &[f64]| {
        out(tag);
        for v in values {
            out(v.to_bits());
        }
    };
    match *v {
        Value::Point { x, y } => all(1, &[x, y]),
        Value::Vector { x, y } => all(2, &[x, y]),
        Value::Line { a, b, c } => all(3, &[a, b, c]),
        Value::Ray { x, y, a, b, c } => all(4, &[x, y, a, b, c]),
        Value::Segment { x1, y1, x2, y2 } => all(5, &[x1, y1, x2, y2]),
        Value::Circle { cx, cy, r } => all(6, &[cx, cy, r]),
        Value::Number(n) => all(7, &[n]),
        Value::Bool(b) => all(8, &[b as u8 as f64]),
        Value::Polygon { area } => all(9, &[area]),
        Value::PolyLine { length } => all(10, &[length]),
        // Its definition is in the fingerprints.
        Value::Function => all(12, &[]),
        Value::Undefined => all(13, &[]),
        Value::Arc { length } => all(19, &[length]),
        Value::Sector { area } => all(20, &[area]),
        // A list's elements and a conic's matrix are read with the
        // construction (`feed`), and a list element is never one of them;
        // a variant added to the kernel later (`Value` is non-exhaustive)
        // is never the same until it is fed here.
        _ => all(14, &[FRESH.fetch_add(1, Ordering::Relaxed) as f64]),
    }
}
