//! The document the window edits: construction, the script lines that built
//! it, selection, visibility, undo and the cached object list. No UI types.

use crate::describe;
use crate::history::History;
use crate::labels::next_label;
use crate::scriptlog::{normalize, Log, Setting};
use crate::undo::Undo;
use geo_kernel::algo::measure::MeasureDef;
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
use std::collections::HashMap;

/// What the algebra list shows for one object.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: Id,
    pub label: String,
    pub kind: Kind,
    /// Free objects take one line: `A = (-5.64, 4.44)`.
    pub free: bool,
    /// First line: `A = (1, 2)` or `f = Line(A, B)`. A free number's head
    /// ends at the `=`; its value is edited in place.
    pub head: String,
    /// Second line of a dependent object, without the leading `=`.
    pub value: Option<String>,
    pub free_number: Option<f64>,
    /// The other objects created by the same line (deleted together).
    pub siblings: Vec<String>,
}

/// A drag in progress: all moves of it are one undo step.
struct Gesture {
    snapshot: Log,
    committed: bool,
}

#[derive(Default)]
pub struct Workbench {
    construction: Construction,
    log: Log,
    version: u64,
    /// Counts the times the construction was rebuilt or replaced. Object ids
    /// are only valid within one such period.
    rebuilds: u64,
    pub selected: Option<Id>,
    pub history: History,
    undo: Undo<Log>,
    gesture: Option<Gesture>,
    /// A problem to tell the user (for example a failed rebuild).
    pub notice: Option<String>,
    rows: Vec<Row>,
    rows_version: Option<u64>,
    rows_built: u64,
}

impl Workbench {
    pub fn from_script(src: &str) -> Result<Self, String> {
        let mut w = Workbench::default();
        w.load_script(src)?;
        Ok(w)
    }

    pub fn construction(&self) -> &Construction {
        &self.construction
    }

    #[cfg(test)]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// See the field of the same name.
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    /// Replaces the whole document. On error the document is unchanged.
    pub fn load_script(&mut self, src: &str) -> Result<(), String> {
        let (c, log) = Log::load(src)?;
        self.construction = c;
        self.log = log;
        self.selected = None;
        self.undo.clear();
        self.gesture = None;
        self.notice = None;
        self.version += 1;
        self.rebuilds += 1;
        Ok(())
    }

    /// Runs what the user typed: `f = Line(A, B)`, `f: Line(A, B)`, or just a
    /// definition, which gets the next free label of its kind.
    pub fn run_line(&mut self, input: &str) -> Result<(), String> {
        let line = self.with_label(normalize(input));
        self.run_lines(std::slice::from_ref(&line))?;
        if !line.is_empty() {
            self.history.push(input.trim());
        }
        Ok(())
    }

    /// Adds `label = ` to a bare definition such as `Midpoint(A, B)` or
    /// `(1, 2)`. The label class follows the kind of the result, found by
    /// running the line on a scratch copy.
    fn with_label(&self, line: String) -> String {
        if line.is_empty() || line.contains('=') {
            return line;
        }
        let kind = self.log.rebuild().ok().and_then(|(mut scratch, _)| {
            let before = scratch.len();
            geo_kernel::script::exec(&mut scratch, &format!("Probe_ = {line}")).ok()?;
            scratch.ids().nth(before).map(|id| scratch.kind(id))
        });
        let label = next_label(kind.unwrap_or(Kind::Line), |l| {
            self.construction.id(l).is_some()
        });
        format!("{label} = {line}")
    }

    /// Runs several script lines as one undo step. If one fails, nothing of
    /// the whole group stays.
    pub fn run_lines(&mut self, lines: &[String]) -> Result<(), String> {
        let snapshot = self.log.clone();
        let n0 = self.construction.len();
        let mut failure = None;
        for raw in lines {
            let line = normalize(raw);
            if line.is_empty() {
                continue;
            }
            let before = self.construction.len();
            let result = geo_kernel::script::exec(&mut self.construction, &line);
            if self.construction.len() > before {
                self.log.record(&self.construction, before, &line);
            }
            if let Err(e) = result {
                failure = Some(e);
                break;
            }
        }
        let changed = self.construction.len() != n0;
        match failure {
            Some(e) => {
                if changed {
                    let _ = self.restore(snapshot);
                }
                Err(e)
            }
            None => {
                if changed {
                    self.undo.record(snapshot);
                    self.version += 1;
                }
                Ok(())
            }
        }
    }

    /// Replays `log` into a new construction, keeping the selection by label.
    /// If the replay fails the previous state stays and `notice` says why.
    fn restore(&mut self, log: Log) -> Result<(), String> {
        let (c, mut kept) = match log.rebuild() {
            Ok(r) => r,
            Err(e) => {
                self.notice = Some(format!("Nie można odtworzyć konstrukcji: {e}"));
                return Err(e);
            }
        };
        let selected = self
            .selected
            .filter(|id| id.index() < self.construction.len())
            .map(|id| self.construction.label(id).to_owned());
        self.selected = selected.and_then(|l| c.id(&l));
        // A label can be reused later; it must not come back hidden.
        kept.hidden.retain(|l| c.id(l).is_some());
        self.construction = c;
        self.log = kept;
        self.gesture = None;
        self.notice = None;
        self.version += 1;
        self.rebuilds += 1;
        Ok(())
    }

    /// Deletes an object and everything that depends on it, as one undo step.
    /// A line that created several objects (such as `Tangent`) is deleted as a
    /// whole: the kernel cannot keep one output of it.
    pub fn delete(&mut self, id: Id) -> bool {
        if id.index() >= self.construction.len() {
            return false;
        }
        let label = self.construction.label(id).to_owned();
        let snapshot = self.log.clone();
        let mut next = snapshot.clone();
        if !next.remove(&label) {
            return false;
        }
        if self.restore(next).is_err() {
            return false;
        }
        self.undo.record(snapshot);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.undo(self.log.clone()) else {
            return false;
        };
        if self.restore(prev.clone()).is_err() {
            // Put the stacks back as they were.
            self.undo.redo(prev);
            return false;
        }
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.undo.redo(self.log.clone()) else {
            return false;
        };
        if self.restore(next.clone()).is_err() {
            self.undo.undo(next);
            return false;
        }
        true
    }

    /// Marks the start of a drag: its moves become a single undo step.
    pub fn begin_gesture(&mut self) {
        self.gesture = Some(Gesture {
            snapshot: self.log.clone(),
            committed: false,
        });
    }

    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    /// Abandons the drag in progress and restores the state before it.
    pub fn cancel_gesture(&mut self) {
        let Some(g) = self.gesture.take() else {
            return;
        };
        if g.committed {
            self.undo.discard_last();
            let _ = self.restore(g.snapshot);
        }
    }

    /// Records the undo step for a change that is about to be logged.
    fn note_change(&mut self) {
        match &mut self.gesture {
            Some(g) => {
                if !g.committed {
                    self.undo.record(g.snapshot.clone());
                    g.committed = true;
                }
            }
            None => self.undo.record(self.log.clone()),
        }
    }

    pub fn is_visible(&self, id: Id) -> bool {
        id.index() >= self.construction.len()
            || !self.log.hidden.contains(self.construction.label(id))
    }

    /// Shows or hides an object in the drawing, as one undo step.
    pub fn toggle_visible(&mut self, id: Id) {
        if id.index() >= self.construction.len() {
            return;
        }
        let label = self.construction.label(id).to_owned();
        self.undo.record(self.log.clone());
        if !self.log.hidden.remove(&label) {
            self.log.hidden.insert(label);
        }
    }

    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> bool {
        if id.index() >= self.construction.len() {
            return false;
        }
        let before = self.construction.value(id);
        if self.construction.move_point(id, x, y).is_err() {
            return false;
        }
        if self.construction.value(id) == before {
            return false;
        }
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log.record_set(&label, Setting::Point(x, y));
        self.version += 1;
        true
    }

    pub fn set_number(&mut self, id: Id, v: f64) -> bool {
        if id.index() >= self.construction.len() {
            return false;
        }
        let before = self.construction.value(id);
        if self.construction.set_number(id, v).is_err() {
            return false;
        }
        if self.construction.value(id) == before {
            return false;
        }
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log.record_set(&label, Setting::Number(v));
        self.version += 1;
        true
    }

    pub fn rows(&mut self) -> &[Row] {
        if self.rows_version != Some(self.version) {
            self.rebuild_rows();
        }
        &self.rows
    }

    #[cfg(test)]
    pub fn rows_built(&self) -> u64 {
        self.rows_built
    }

    fn rebuild_rows(&mut self) {
        let c = &self.construction;
        let rhs: HashMap<&str, &str> = self.log.definitions().collect();
        let groups: Vec<&[String]> = self.log.multi_output().collect();
        self.rows.clear();
        self.rows.extend(c.ids().map(|id| {
            let value = c.value(id);
            let label = c.label(id).to_owned();
            let kind = c.kind(id);
            let free = c.def(id).is_free();
            let text = match (c.def(id), value) {
                // Angles are kept in radians and shown in degrees.
                (Def::Measure(MeasureDef::Angle(..)), Value::Number(n)) => describe::degrees(n),
                _ => describe::value_text(&value),
            };
            let free_number = match (c.def(id), value) {
                (Def::Number(_), Value::Number(n)) => Some(n),
                _ => None,
            };
            let (head, second) = if free_number.is_some() {
                (format!("{label} ="), None)
            } else if free {
                (format!("{label} = {text}"), None)
            } else {
                // A side made by a polygon names the polygon, as in the
                // reference: `a = Segment(A, B, q1)`.
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
                // `label = definition` for every kind, as in the reference.
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
            }
        }));
        self.rows_version = Some(self.version);
        self.rows_built += 1;
    }
}

#[cfg(test)]
mod tests;
