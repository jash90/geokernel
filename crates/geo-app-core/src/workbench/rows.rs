//! The algebra rows, kept up to date with as little work as each change
//! needs.
//!
//! A row's texts follow from the object's label, its definition line and
//! its value. So:
//!
//! - after the construction was replaced or rebuilt (open, undo, redo,
//!   delete) the objects are renumbered: an object keeps its row (with the
//!   new id) when its label, kind, definition line, the other objects of
//!   that line and its value are those the row was made from, and only the
//!   other rows are made anew. Telling which is one pass over the journal
//!   and one comparison per object, without formatting a number;
//! - objects added at the end (a typed line, a tool) get new rows, and the
//!   rows before them stay;
//! - after a move or a new number only the rows whose value changed (by its
//!   bits, as the kernel compares) are made anew, and nothing when no value
//!   changed. Telling which values changed is one comparison of plain
//!   values per object, without formatting or allocating. A conic's value
//!   holds only its type, so its matrix is compared as well.

use super::{Row, Workbench};
use crate::describe;
use crate::scriptlog::{GrownOutputs, Log};
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
#[cfg(test)]
use std::collections::HashMap;

/// Whether two values are the same bit for bit (`0` and `-0` differ, a
/// `NaN` equals itself). A kind of value this function does not know is
/// taken as changed.
pub(crate) fn same_value(a: Value, b: Value) -> bool {
    let s = |x: f64, y: f64| x.to_bits() == y.to_bits();
    match (a, b) {
        (Value::Point { x, y }, Value::Point { x: x2, y: y2 })
        | (Value::Vector { x, y }, Value::Vector { x: x2, y: y2 }) => s(x, x2) && s(y, y2),
        (
            Value::Line { a, b, c },
            Value::Line {
                a: a2,
                b: b2,
                c: c2,
            },
        ) => s(a, a2) && s(b, b2) && s(c, c2),
        (
            Value::Segment { x1, y1, x2, y2 },
            Value::Segment {
                x1: u1,
                y1: v1,
                x2: u2,
                y2: v2,
            },
        ) => s(x1, u1) && s(y1, v1) && s(x2, u2) && s(y2, v2),
        (Value::Circle { cx, cy, r }, Value::Circle { cx: x, cy: y, r: q }) => {
            s(cx, x) && s(cy, y) && s(r, q)
        }
        (Value::Number(n), Value::Number(m)) => s(n, m),
        (
            Value::Ray { x, y, a, b, c },
            Value::Ray {
                x: x2,
                y: y2,
                a: a2,
                b: b2,
                c: c2,
            },
        ) => s(x, x2) && s(y, y2) && s(a, a2) && s(b, b2) && s(c, c2),
        (Value::Bool(p), Value::Bool(q)) => p == q,
        (Value::Polygon { area }, Value::Polygon { area: a2 }) => s(area, a2),
        (Value::PolyLine { length }, Value::PolyLine { length: l2 }) => s(length, l2),
        (Value::Function, Value::Function)
        | (Value::FunctionNVar, Value::FunctionNVar)
        | (Value::Undefined, Value::Undefined) => true,
        _ => false,
    }
}

/// What a row was made from: the value and, for a conic, the bits of its
/// matrix (zero for any other kind).
pub(crate) type RowKey = (Value, [u64; 6]);

/// The [`RowKey`] of `id`, without allocating.
fn row_key(c: &Construction, id: Id) -> RowKey {
    let value = c.value(id);
    let matrix = match value {
        Value::Conic { .. } => c
            .conic(id)
            .map_or([u64::MAX; 6], |d| d.matrix.map(f64::to_bits)),
        _ => [0; 6],
    };
    (value, matrix)
}

/// Whether two row keys are the same bit for bit.
fn same_key(a: RowKey, b: RowKey) -> bool {
    match (a.0, b.0) {
        (Value::Conic { ty }, Value::Conic { ty: ty2 }) => ty == ty2 && a.1 == b.1,
        _ => same_value(a.0, b.0),
    }
}

fn same_row_value(c: &Construction, id: Id, row: &Row, old: RowKey, new: RowKey) -> bool {
    if matches!((old.0, new.0), (Value::Text, Value::Text)) {
        if row.free {
            is_joined(
                &row.head,
                &[&row.label, " = ", c.text(id).unwrap_or_default()],
            )
        } else {
            row.value.as_deref() == c.text(id)
        }
    } else {
        same_key(old, new)
    }
}

fn is_free(c: &Construction, id: Id) -> bool {
    c.def(id).is_free() || (c.kind(id) == Kind::Text && c.is_independent_text(id))
}

/// The number of a free number or angle, edited in place.
fn free_number(c: &Construction, id: Id) -> Option<f64> {
    match c.value(id) {
        Value::Number(n) if c.def(id).is_free() => Some(n),
        _ => None,
    }
}

/// What a row takes from the journal: the definition of the line that made
/// the object and, when that line made several objects, its labels.
#[derive(Clone, Copy, Default)]
struct Source<'a> {
    rhs: Option<&'a str>,
    group: Option<&'a [String]>,
}

/// The sources of the objects of `c` from index `from` on, in one pass
/// over the journal and as a search of it by label finds them: the
/// definition of the newest line that names the object, and the labels of
/// the oldest line with several outputs that names it.
fn sources<'a>(
    c: &Construction,
    log: &'a Log,
    grown: &'a [GrownOutputs<'a>],
    from: usize,
) -> Vec<Source<'a>> {
    let mut out = vec![Source::default(); c.len().saturating_sub(from)];
    for e in log.entries_rev() {
        let multi = e.labels.len() > 1;
        for l in &e.labels {
            let Some(s) = c
                .id(l)
                .and_then(|id| id.index().checked_sub(from))
                .and_then(|i| out.get_mut(i))
            else {
                continue;
            };
            if s.rhs.is_none() {
                s.rhs = Some(e.rhs());
            }
            if multi {
                s.group = Some(e.labels.as_slice());
            }
        }
    }
    for command in grown {
        for label in &command.labels {
            if let Some(src) = c
                .id(label)
                .and_then(|id| id.index().checked_sub(from))
                .and_then(|i| out.get_mut(i))
            {
                src.rhs = Some(command.rhs);
                src.group = Some(&command.labels);
            }
        }
    }
    out
}

/// The pieces of the first line of a dependent object's row: `label =
/// definition`, or for a side made by a polygon the side with the polygon
/// named, as in the reference: `a = Segment(A, B, q1)`.
fn head_parts<'a>(
    c: &'a Construction,
    id: Id,
    label: &'a str,
    src: Source<'a>,
) -> ([&'a str; 8], usize) {
    let polygon = src
        .group
        .filter(|g| g[0] != label)
        .map(|g| g[0].as_str())
        .filter(|&p| c.id(p).is_some_and(|p| c.kind(p) == Kind::Polygon));
    match (c.def(id), polygon) {
        (Def::Segment(p, q), Some(poly)) => (
            [
                label,
                " = Segment(",
                c.label(p),
                ", ",
                c.label(q),
                ", ",
                poly,
                ")",
            ],
            8,
        ),
        _ => match src.rhs {
            Some(rhs) => ([label, " = ", rhs, "", "", "", "", ""], 3),
            None => ([label, "", "", "", "", "", "", ""], 1),
        },
    }
}

/// Whether `text` is `parts` joined, without joining them.
fn is_joined(text: &str, parts: &[&str]) -> bool {
    let mut rest = text;
    for p in parts {
        match rest.strip_prefix(p) {
            Some(r) => rest = r,
            None => return false,
        }
    }
    rest.is_empty()
}

/// The labels of the other objects of `src`'s line.
fn siblings<'a>(src: Source<'a>, label: &'a str) -> impl Iterator<Item = &'a String> {
    src.group
        .unwrap_or_default()
        .iter()
        .filter(move |l| l.as_str() != label)
}

/// The value text of `id` and, for a free number or angle, its number.
fn value_parts(c: &Construction, id: Id) -> (String, Option<f64>) {
    let text = match (c.kind(id), c.value(id)) {
        // Angles are kept in radians and shown in degrees.
        (Kind::Angle, Value::Number(n)) => describe::degrees(n),
        _ => describe::object_text(c, id),
    };
    // A free angle is a free number shown and set in degrees.
    (text, free_number(c, id))
}

/// The row of `id` made from scratch.
fn make_row(c: &Construction, id: Id, src: Source) -> Row {
    let label = c.label(id);
    let free = is_free(c, id);
    let mut row = Row {
        id,
        kind: c.kind(id),
        free,
        head: String::new(),
        value: None,
        free_number: None,
        angle: c.kind(id) == Kind::Angle,
        siblings: siblings(src, label).cloned().collect(),
        label: label.to_owned(),
    };
    let (text, free_number) = value_parts(c, id);
    if free_number.is_none() && !free {
        // `label = definition` for every kind, as in the reference.
        let (parts, n) = head_parts(c, id, label, src);
        row.head = parts[..n].concat();
    }
    set_value(&mut row, text, free_number);
    row
}

/// Makes room for `n` items in `v` as pushing them one by one would: a
/// capacity that is a power of two, so that the next objects added do not
/// move the whole list at once.
fn room_for<T>(v: &mut Vec<T>, n: usize) {
    if v.capacity() < n {
        v.reserve(n.next_power_of_two() - v.len());
    }
}

/// A row that holds nothing, to fill a place until its row is put there.
fn placeholder(id: Id) -> Row {
    Row {
        id,
        label: String::new(),
        kind: Kind::Point,
        free: false,
        head: String::new(),
        value: None,
        free_number: None,
        angle: false,
        siblings: Vec::new(),
    }
}

/// Whether `row`, made for an object of an earlier construction from
/// `made_from`, is the row of `id` in `c` with `src` and the key `key`:
/// the same label (the caller's match), kind, definition line, other
/// objects of that line and value texts.
fn still_fits(
    c: &Construction,
    id: Id,
    src: Source,
    row: &Row,
    made_from: RowKey,
    key: RowKey,
) -> bool {
    let free = is_free(c, id);
    let number = free_number(c, id);
    // A free object's or number's head follows from its label and value.
    let same_head = free || number.is_some() || {
        let (parts, n) = head_parts(c, id, &row.label, src);
        is_joined(&row.head, &parts[..n])
    };
    row.kind == c.kind(id)
        && row.free == free
        && row.angle == (c.kind(id) == Kind::Angle)
        && row.free_number == number
        && same_row_value(c, id, row, made_from, key)
        && row.siblings.iter().eq(siblings(src, &row.label))
        && same_head
}

/// Puts the value parts into `row`, whose definition part is in place: a
/// free number's head ends at the `=`, a free object shows its value in the
/// head, a dependent one on the second line. Returns whether a text or the
/// number changed.
fn set_value(row: &mut Row, text: String, free_number: Option<f64>) -> bool {
    let mut changed = row.free_number != free_number;
    row.free_number = free_number;
    if free_number.is_some() {
        let head = format!("{} =", row.label);
        changed |= row.head != head;
        row.head = head;
    } else if matches!(row.kind, Kind::Function | Kind::FunctionNVar) {
        // A function's value is the expression its definition shows.
        changed |= row.value.is_some();
        row.value = None;
    } else if row.free {
        let head = format!("{} = {text}", row.label);
        changed |= row.head != head;
        row.head = head;
    } else {
        changed |= row.value.as_deref() != Some(text.as_str());
        row.value = Some(text);
    }
    changed
}

impl Workbench {
    /// The rows of the algebra list, brought up to date first.
    pub fn rows(&mut self) -> &[Row] {
        self.refresh_rows();
        &self.rows
    }

    /// The rows as the last [`Workbench::rows`] or
    /// [`Workbench::refresh_rows`] left them, for a frame that refreshed them
    /// first and then draws them while reading the rest of the document.
    pub fn current_rows(&self) -> &[Row] {
        &self.rows
    }

    /// How many times the object list changed: a refresh that replaced,
    /// added or removed at least one row (read by the window's tests and its
    /// frame benchmark). A move that changes no value counts nothing.
    pub fn rows_built(&self) -> u64 {
        self.rows_built
    }

    /// How many rows were made, in total: each row whose texts were made
    /// anew counts once.
    pub fn rows_formatted(&self) -> u64 {
        self.rows_formatted
    }

    /// Brings the rows up to date with the construction (see the module
    /// docs for how much that costs).
    pub fn refresh_rows(&mut self) {
        let now = (self.rebuilds, self.version);
        if self.rows_at == Some(now) {
            return;
        }
        let replaced = self.rows_at.is_none_or(|(r, _)| r != self.rebuilds);
        let mut changed = false;
        if replaced {
            changed = !self.rows.is_empty() || !self.construction.is_empty();
            self.carry_rows_over();
        } else if self.rows.len() > self.construction.len() {
            changed = true;
            self.rows.clear();
            self.row_values.clear();
        }
        let c = &self.construction;
        // Rows whose value changed.
        for (row, old) in self.rows.iter_mut().zip(&mut self.row_values) {
            let id = row.id;
            let key = row_key(c, id);
            if same_row_value(c, id, row, *old, key) {
                continue;
            }
            *old = key;
            let (text, free_number) = value_parts(c, id);
            self.rows_formatted += 1;
            changed |= set_value(row, text, free_number);
        }
        // Rows of new objects.
        let from = self.rows.last().map_or(0, |row| row.id.index() + 1);
        if from < c.len() {
            let grown = self.log.grown_groups();
            let sources = sources(c, &self.log, &grown, from);
            for (id, src) in c.ids().skip(from).zip(sources) {
                if crate::scriptlog::is_internal(c, id) {
                    continue;
                }
                self.rows.push(make_row(c, id, src));
                self.row_values.push(row_key(c, id));
                self.rows_formatted += 1;
            }
            changed = true;
        }
        if changed {
            self.rows_built += 1;
        }
        self.rows_at = Some(now);
    }

    /// After the construction was rebuilt or replaced: a row for every
    /// object, the row of the object with the same label before when it
    /// still fits (`still_fits`), with the new id, else one made anew. The
    /// rows are moved within their own storage: rows kept in their order
    /// cost no allocation (a row out of order is made anew; a rebuild keeps
    /// the order of the objects it keeps).
    fn carry_rows_over(&mut self) {
        let c = &self.construction;
        let rows = &mut self.rows;
        // Each object and its old row (by its label), if it still fits and
        // comes after the rows kept before it; `usize::MAX` for none.
        let mut kept: Vec<(Id, usize)> = c.ids().map(|id| (id, usize::MAX)).collect();
        for (j, row) in rows.iter().enumerate() {
            if let Some(id) = c.id(&row.label) {
                kept[id.index()].1 = j;
            }
        }
        let grown = self.log.grown_groups();
        let sources = sources(c, &self.log, &grown, 0);
        let mut last = None;
        let mut count = 0;
        for ((id, k), &src) in kept.iter_mut().zip(&sources) {
            let (id, j) = (*id, *k);
            if crate::scriptlog::is_internal(c, id) {
                *k = usize::MAX;
                continue;
            }
            count += 1;
            let fits = j != usize::MAX
                && last.is_none_or(|l| l < j)
                && still_fits(c, id, src, &rows[j], self.row_values[j], row_key(c, id));
            if fits {
                last = Some(j);
            } else {
                *k = usize::MAX;
            }
        }
        // The kept rows to the front, in order (each moves down or stays).
        let mut n = 0;
        for &(_, j) in kept.iter().filter(|(_, j)| *j != usize::MAX) {
            rows.swap(n, j);
            n += 1;
        }
        rows.truncate(n);
        // Then each to its object's place (up or staying), and new rows in
        // the gaps: the places after `i` are done, the kept rows not placed
        // yet are the first `n`.
        let visible = kept
            .iter()
            .zip(&sources)
            .filter(|&(&(id, _), _)| !crate::scriptlog::is_internal(c, id));
        room_for(rows, count);
        if let Some(&(first, _)) = kept.first() {
            rows.resize_with(count, || placeholder(first));
        }
        let mut i = count;
        for (&(id, j), &src) in visible.rev() {
            i -= 1;
            if j == usize::MAX {
                rows[i] = make_row(c, id, src);
                self.rows_formatted += 1;
            } else {
                n -= 1;
                rows.swap(i, n);
                rows[i].id = id;
            }
        }
        self.row_values.clear();
        room_for(&mut self.row_values, count);
        self.row_values
            .extend(self.rows.iter().map(|row| row_key(c, row.id)));
    }

    /// The rows made the way they were before they were cached: every row
    /// anew, searching the multi-object lines for each. The oracle of the
    /// row tests.
    #[cfg(test)]
    pub(crate) fn rows_reference(&self) -> Vec<Row> {
        let c = &self.construction;
        let grown = self.log.grown_groups();
        let mut rhs: HashMap<&str, &str> = self.log.definitions().collect();
        for group in &grown {
            rhs.extend(group.labels.iter().map(|label| (label.as_str(), group.rhs)));
        }
        let groups: Vec<&[String]> = grown
            .iter()
            .map(|group| group.labels.as_slice())
            .chain(self.log.multi_output())
            .collect();
        self.object_ids()
            .map(|id| {
                let value = c.value(id);
                let label = c.label(id).to_owned();
                let kind = c.kind(id);
                let free = c.def(id).is_free();
                let text = match (kind, value) {
                    (Kind::Angle, Value::Number(n)) => describe::degrees(n),
                    _ => describe::object_text(c, id),
                };
                let free_number = match value {
                    Value::Number(n) if free => Some(n),
                    _ => None,
                };
                let (head, second) = if free_number.is_some() {
                    (format!("{label} ="), None)
                } else if matches!(kind, Kind::Function | Kind::FunctionNVar) {
                    let head = match rhs.get(label.as_str()) {
                        Some(rhs) => format!("{label} = {rhs}"),
                        None => label.clone(),
                    };
                    (head, None)
                } else if free {
                    (format!("{label} = {text}"), None)
                } else {
                    let polygon = groups
                        .iter()
                        .find(|g| g.contains(&label) && g[0] != label)
                        .map(|g| g[0].as_str())
                        .filter(|&p| c.id(p).is_some_and(|p| c.kind(p) == Kind::Polygon));
                    let side = match (c.def(id), polygon) {
                        (Def::Segment(p, q), Some(poly)) => {
                            Some(format!("Segment({}, {}, {poly})", c.label(p), c.label(q)))
                        }
                        _ => None,
                    };
                    match side.as_deref().or(rhs.get(label.as_str()).copied()) {
                        Some(rhs) => (format!("{label} = {rhs}"), Some(text)),
                        None => (label.clone(), Some(text)),
                    }
                };
                let siblings = groups
                    .iter()
                    .find(|g| g.contains(&label))
                    .map(|g| g.iter().filter(|l| **l != label).cloned().collect())
                    .unwrap_or_default();
                Row {
                    id,
                    label,
                    kind,
                    free,
                    head,
                    value: second,
                    siblings,
                    free_number,
                    angle: c.kind(id) == Kind::Angle,
                }
            })
            .collect()
    }
}
