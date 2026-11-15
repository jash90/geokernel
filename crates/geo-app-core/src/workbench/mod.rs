//! The document the window edits: construction, the script lines that built
//! it, selection, visibility, undo and the cached object list. No UI types.

#[cfg(test)]
use crate::describe;
use crate::history::History;
use crate::labels::next_label;
use crate::scriptlog::{normalize, Log, Setting};
use crate::style::Sides;
use crate::undo::Undo;
pub use file::{import_notice, rejected_notice, SNAPSHOT_VERSION};
use geo_kernel::kernel::{Construction, Id, Kind, Value};
use std::sync::OnceLock;

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
    /// The free number is an angle: kept in radians, shown and set in
    /// degrees.
    pub angle: bool,
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
    /// The algebra rows, the values each was made from, and the state they
    /// were brought up to date with (see `rows.rs`).
    rows: Vec<Row>,
    row_values: Vec<Value>,
    rows_at: Option<(u64, u64)>,
    rows_built: u64,
    rows_formatted: u64,
    /// The sides of the polygons, for the colours; made on first use after
    /// the objects changed (see `objects_changed`).
    sides: OnceLock<Sides>,
    /// Tests make every rebuild fail with this message while it is set.
    #[cfg(test)]
    pub fail_rebuilds: Option<String>,
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

    /// The sides of the polygons of the construction, for
    /// [`crate::style::color_in`]. Made once after objects were added or the
    /// construction replaced, so a frame colours every object in linear
    /// time.
    pub fn sides(&self) -> &Sides {
        self.sides.get_or_init(|| Sides::of(&self.construction))
    }

    /// Forgets what is derived from the set of objects (not their values):
    /// called whenever objects were added, or the construction replaced.
    fn objects_changed(&mut self) {
        self.sides = OnceLock::new();
    }

    #[cfg(test)]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The definition of the line that created `label`.
    pub fn definition(&self, label: &str) -> Option<&str> {
        self.log.rhs_of(label)
    }

    /// See the field of the same name.
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    /// Replaces the whole document. On error the document is unchanged.
    pub fn load_script(&mut self, src: &str) -> Result<(), String> {
        // A byte order mark (from a Windows editor) is not part of line 1.
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        let (c, log) = Log::load(src)?;
        self.replace(c, log);
        Ok(())
    }

    /// Makes `c` and `log` the whole document.
    fn replace(&mut self, c: Construction, log: Log) {
        self.construction = c;
        self.objects_changed();
        self.log = log;
        self.selected = None;
        self.undo.clear();
        self.gesture = None;
        self.notice = None;
        self.version += 1;
        self.rebuilds += 1;
    }

    /// Replaces the whole document with an imported file: its script, the
    /// labels it hides, and a notice to show (for what was left out).
    pub fn load_imported(
        &mut self,
        src: &str,
        hidden: &[String],
        notice: Option<String>,
    ) -> Result<(), String> {
        self.load_script(src)?;
        for label in hidden {
            if self.construction.id(label).is_some() {
                self.log.hidden.insert(label.clone());
            }
        }
        self.notice = notice;
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
            if let Err(e) = crate::scriptlog::check_names(&line) {
                failure = Some(e);
                break;
            }
            let before = self.construction.len();
            // Only lines that succeeded go into the log: a failed one would
            // fail again on every replay.
            match geo_kernel::script::exec(&mut self.construction, &line) {
                Ok(_) => {
                    if self.construction.len() > before {
                        self.log.record(&self.construction, before, &line);
                    }
                }
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        let changed = self.construction.len() != n0;
        if changed {
            self.objects_changed();
        }
        match failure {
            Some(e) => {
                if changed {
                    if let Err(why) = self.restore(snapshot) {
                        self.keep_unrevertable(&why);
                    }
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

    /// After a rollback that failed: the change stays and cannot be undone.
    ///
    /// A rollback replays lines that all ran before, yet the replay can still
    /// fail: after a deletion an automatic label taken at the time may be free
    /// again (so a line names its outputs differently), and a line whose
    /// inputs are gone is skipped only when it made nothing. Replaying the
    /// same log fails the same way every time, so the step is not offered for
    /// undo (the older steps stay) and the redo steps, which no longer follow
    /// from this state, are dropped. The log holds only lines that
    /// succeeded; a failing command with several outputs may have left some
    /// of them in the construction, so it is rebuilt from that log once more
    /// (if that fails too, they stay until the document is reloaded).
    fn keep_unrevertable(&mut self, why: &str) {
        if self.construction.len() != self.log.rebuild().map_or(usize::MAX, |r| r.0.len()) {
            let _ = self.restore(self.log.clone());
        }
        self.undo.clear_redo();
        self.gesture = None;
        self.version += 1;
        self.notice = Some(format!("Nie można cofnąć ostatniej zmiany: {why}"));
    }

    /// Replays `log` into a new construction, keeping the selection by label.
    /// If the replay fails the previous state stays and `notice` says why.
    fn restore(&mut self, log: Log) -> Result<(), String> {
        #[cfg(test)]
        if let Some(e) = self.fail_rebuilds.clone() {
            self.notice = Some(format!("Nie można odtworzyć konstrukcji: {e}"));
            return Err(e);
        }
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
        self.objects_changed();
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
            // The drag's undo step goes either way: on success the drag is
            // gone, on failure it stays but could not be undone either.
            self.undo.discard_last();
            if let Err(why) = self.restore(g.snapshot) {
                self.keep_unrevertable(&why);
            }
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
        // Compared by bits, as the kernel does: `0` to `-0` is a change.
        let same = match self.construction.value(id) {
            Value::Point { x: ox, y: oy } | Value::Vector { x: ox, y: oy } => {
                ox.to_bits() == x.to_bits() && oy.to_bits() == y.to_bits()
            }
            _ => false,
        };
        if self.construction.move_point(id, x, y).is_err() || same {
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
        // Compared by bits, as the kernel does. A number that still has the
        // exact decimal it was typed with loses it on any set, which can
        // change what reads it in the last bits even when the value stays:
        // that set is a change too, and it goes into the journal.
        let same = matches!(self.construction.value(id), Value::Number(old) if old.to_bits() == v.to_bits())
            && self.construction.expression(id).is_none();
        if self.construction.set_number(id, v).is_err() || same {
            return false;
        }
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log.record_set(&label, Setting::Number(v));
        self.version += 1;
        true
    }
}

mod file;
mod rows;
#[cfg(test)]
mod tests;
