//! The document the window edits: construction, the script lines that built
//! it, selection, visibility, undo and the cached object list. No UI types.

use crate::describe;
use crate::history::History;
use crate::labels::next_label;
use crate::scriptlog::{normalize, Log};
use crate::undo::Undo;
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
use std::collections::HashSet;

/// What the algebra list shows for one object.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: Id,
    pub label: String,
    pub kind: Kind,
    /// Free objects take one line: `A = (-5.64, 4.44)`.
    pub free: bool,
    /// First line: `A = (1, 2)` or `f: Line(A, B)`. A free number's head
    /// ends at the `=`; its value is edited in place.
    pub head: String,
    /// Second line of a dependent object, without the leading `=`.
    pub value: Option<String>,
    pub free_number: Option<f64>,
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
    pub selected: Option<Id>,
    pub history: History,
    hidden: HashSet<String>,
    undo: Undo<Log>,
    gesture: Option<Gesture>,
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

    /// Replaces the whole document. On error the document is unchanged.
    pub fn load_script(&mut self, src: &str) -> Result<(), String> {
        let (c, log) = Log::load(src)?;
        self.construction = c;
        self.log = log;
        self.selected = None;
        self.hidden.clear();
        self.undo.clear();
        self.gesture = None;
        self.version += 1;
        Ok(())
    }

    /// Runs what the user typed: `f = Line(A, B)`, `f: Line(A, B)`, or just a
    /// definition, which gets the next free label.
    pub fn run_line(&mut self, input: &str) -> Result<(), String> {
        let line = self.with_label(normalize(input));
        self.run_lines(std::slice::from_ref(&line))?;
        if !line.is_empty() {
            self.history.push(input.trim());
        }
        Ok(())
    }

    /// Adds `label = ` to a bare definition such as `Midpoint(A, B)` or `(1, 2)`.
    fn with_label(&self, line: String) -> String {
        if line.is_empty() || line.contains('=') {
            return line;
        }
        let func = line.split('(').next().unwrap_or("").trim();
        let kind = match func {
            "" => Kind::Point,
            "Midpoint" | "Intersect" => Kind::Point,
            "Circle" => Kind::Circle,
            f if f.chars().next().is_some_and(char::is_uppercase) => Kind::Line,
            _ => return line,
        };
        let label = next_label(kind, |l| self.construction.id(l).is_some());
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
                    self.restore(snapshot);
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

    /// Rebuilds the construction from `log`, keeping the selection by label.
    fn restore(&mut self, log: Log) {
        let selected = self
            .selected
            .map(|id| self.construction.label(id).to_owned());
        let (c, kept) = log.rebuild();
        self.selected = selected.and_then(|l| c.id(&l));
        self.construction = c;
        self.log = kept;
        self.version += 1;
    }

    /// Deletes an object and everything that depends on it, as one undo step.
    pub fn delete(&mut self, id: Id) -> bool {
        let label = self.construction.label(id).to_owned();
        let snapshot = self.log.clone();
        if !self.log.remove(&label) {
            return false;
        }
        let rest = std::mem::take(&mut self.log);
        self.restore(rest);
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
        match self.undo.undo(self.log.clone()) {
            Some(prev) => {
                self.restore(prev);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.undo.redo(self.log.clone()) {
            Some(next) => {
                self.restore(next);
                true
            }
            None => false,
        }
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
        !self.hidden.contains(self.construction.label(id))
    }

    pub fn toggle_visible(&mut self, id: Id) {
        let label = self.construction.label(id).to_owned();
        if !self.hidden.remove(&label) {
            self.hidden.insert(label);
        }
    }

    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> bool {
        let before = self.construction.value(id);
        if self.construction.move_point(id, x, y).is_err() {
            return false;
        }
        if self.construction.value(id) == before {
            return false;
        }
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log.set_rhs(&label, &format!("({x}, {y})"));
        self.version += 1;
        true
    }

    pub fn set_number(&mut self, id: Id, v: f64) -> bool {
        let before = self.construction.value(id);
        if self.construction.set_number(id, v).is_err() {
            return false;
        }
        if self.construction.value(id) == before {
            return false;
        }
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log.set_rhs(&label, &format!("{v}"));
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
        self.rows.clear();
        self.rows.extend(c.ids().map(|id| {
            let value = c.value(id);
            let label = c.label(id).to_owned();
            let kind = c.kind(id);
            let free = c.def(id).is_free();
            let text = describe::value_text(&value);
            let free_number = match (c.def(id), value) {
                (Def::Number(_), Value::Number(n)) => Some(n),
                _ => None,
            };
            let (head, second) = if free_number.is_some() {
                (format!("{label} ="), None)
            } else if free {
                (format!("{label} = {text}"), None)
            } else {
                match self.log.rhs_of(&label) {
                    Some(rhs) if kind == Kind::Point => (format!("{label} = {rhs}"), Some(text)),
                    Some(rhs) => (format!("{label}: {rhs}"), Some(text)),
                    None => (label.clone(), Some(text)),
                }
            };
            Row {
                id,
                label,
                kind,
                free,
                head,
                value: second,
                free_number,
            }
        }));
        self.rows_version = Some(self.version);
        self.rows_built += 1;
    }
}

#[cfg(test)]
mod tests;
