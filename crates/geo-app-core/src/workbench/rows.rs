//! The algebra rows, kept up to date with as little work as each change
//! needs.
//!
//! A row's texts follow from the object's definition line (fixed while the
//! construction keeps its objects) and from its value. So:
//!
//! - after the construction was replaced or rebuilt (open, undo, redo,
//!   delete) every row is made anew, in one pass;
//! - objects added at the end (a typed line, a tool) get new rows, and the
//!   rows before them stay;
//! - after a move or a new number only the rows whose value changed (by its
//!   bits, as the kernel compares) are made anew, and nothing when no value
//!   changed. Telling which values changed is one comparison of plain
//!   values per object, without formatting or allocating. A conic's value
//!   holds only its type, so its matrix is compared as well.

use super::{Row, Workbench};
use crate::describe;
use crate::scriptlog::Log;
use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
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
        (Value::Function, Value::Function) | (Value::Undefined, Value::Undefined) => true,
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

/// What the rows take from the journal: the definition of each label and
/// the line (as an index into `groups`) that made it with other objects.
struct Lines<'a> {
    rhs: HashMap<&'a str, &'a str>,
    groups: Vec<&'a [String]>,
    group_of: HashMap<&'a str, usize>,
}

impl<'a> Lines<'a> {
    fn of(log: &'a Log) -> Self {
        let groups: Vec<&[String]> = log.multi_output().collect();
        let mut group_of = HashMap::new();
        for (i, g) in groups.iter().enumerate() {
            for l in g.iter() {
                // The first line that names a label, as a search would find.
                group_of.entry(l.as_str()).or_insert(i);
            }
        }
        Lines {
            rhs: log.definitions().collect(),
            groups,
            group_of,
        }
    }
}

/// The value text of `id` and, for a free number or angle, its number.
fn value_parts(c: &Construction, id: Id) -> (String, Option<f64>) {
    let value = c.value(id);
    let text = match (c.def(id), value) {
        // Angles are kept in radians and shown in degrees.
        (Def::Measure(MeasureDef::Angle(..)), Value::Number(n)) => describe::degrees(n),
        _ => describe::object_text(c, id),
    };
    // A free angle is a free number shown and set in degrees.
    let free_number = match (c.def(id), value) {
        (Def::Number(_), Value::Number(n)) => Some(n),
        (Def::Angle(_), Value::Number(n)) => Some(n),
        _ => None,
    };
    (text, free_number)
}

/// The row of `id` made from scratch.
fn make_row(c: &Construction, id: Id, lines: &Lines) -> Row {
    let label = c.label(id).to_owned();
    let free = c.def(id).is_free();
    let group = lines.group_of.get(label.as_str()).map(|&i| lines.groups[i]);
    let siblings = group
        .map(|g| g.iter().filter(|l| **l != label).cloned().collect())
        .unwrap_or_default();
    let mut row = Row {
        id,
        kind: c.kind(id),
        free,
        head: String::new(),
        value: None,
        free_number: None,
        angle: matches!(c.def(id), Def::Angle(_)),
        siblings,
        label,
    };
    let (text, free_number) = value_parts(c, id);
    if free_number.is_none() && !free {
        // A side made by a polygon names the polygon, as in the
        // reference: `a = Segment(A, B, q1)`.
        let polygon = group
            .filter(|g| g[0] != row.label)
            .map(|g| g[0].as_str())
            .filter(|&p| c.id(p).is_some_and(|p| c.kind(p) == Kind::Polygon));
        let side = match (c.def(id), polygon) {
            (Def::Segment(p, q), Some(poly)) => {
                Some(format!("Segment({}, {}, {poly})", c.label(p), c.label(q)))
            }
            _ => None,
        };
        // `label = definition` for every kind, as in the reference.
        row.head = match side
            .as_deref()
            .or(lines.rhs.get(row.label.as_str()).copied())
        {
            Some(rhs) => format!("{} = {rhs}", row.label),
            None => row.label.clone(),
        };
    }
    set_value(&mut row, text, free_number);
    row
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
    } else if row.kind == Kind::Function {
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
        let c = &self.construction;
        let replaced = self.rows_at.is_none_or(|(r, _)| r != self.rebuilds);
        let mut changed = false;
        if replaced || self.rows.len() > c.len() {
            changed = !self.rows.is_empty();
            self.rows.clear();
            self.row_values.clear();
        }
        // Rows whose value changed.
        for ((id, row), old) in c.ids().zip(&mut self.rows).zip(&mut self.row_values) {
            let key = row_key(c, id);
            if same_key(*old, key) {
                continue;
            }
            *old = key;
            let (text, free_number) = value_parts(c, id);
            self.rows_formatted += 1;
            changed |= set_value(row, text, free_number);
        }
        // Rows of new objects.
        if self.rows.len() < c.len() {
            let lines = Lines::of(&self.log);
            for id in c.ids().skip(self.rows.len()) {
                self.rows.push(make_row(c, id, &lines));
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

    /// The rows made the way they were before they were cached: every row
    /// anew, searching the multi-object lines for each. The oracle of the
    /// row tests.
    #[cfg(test)]
    pub(crate) fn rows_reference(&self) -> Vec<Row> {
        let c = &self.construction;
        let rhs: HashMap<&str, &str> = self.log.definitions().collect();
        let groups: Vec<&[String]> = self.log.multi_output().collect();
        c.ids()
            .map(|id| {
                let value = c.value(id);
                let label = c.label(id).to_owned();
                let kind = c.kind(id);
                let free = c.def(id).is_free();
                let text = match (c.def(id), value) {
                    (Def::Measure(MeasureDef::Angle(..)), Value::Number(n)) => describe::degrees(n),
                    _ => describe::object_text(c, id),
                };
                let free_number = match (c.def(id), value) {
                    (Def::Number(_), Value::Number(n)) => Some(n),
                    (Def::Angle(_), Value::Number(n)) => Some(n),
                    _ => None,
                };
                let (head, second) = if free_number.is_some() {
                    (format!("{label} ="), None)
                } else if kind == Kind::Function {
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
                    angle: matches!(c.def(id), Def::Angle(_)),
                }
            })
            .collect()
    }
}
