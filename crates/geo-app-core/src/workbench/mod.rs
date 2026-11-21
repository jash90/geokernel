//! The document the window edits: construction, the script lines that built
//! it, selection, visibility, undo and the cached object list. No UI types.

#[cfg(test)]
use crate::describe;
use crate::history::History;
use crate::labels::next_label;
use crate::scriptlog::{normalize, Log, Setting};
use crate::snap::check_decimal_fraction;
use crate::style::{Palette, Sides};
use crate::undo::Undo;
pub use file::{import_notice, rejected_notice, SNAPSHOT_VERSION};
use geo_kernel::kernel::{Construction, Id, Kind, Value};
use std::collections::BTreeMap;
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

/// A construction tool in use: the state before its first pick
/// (`Kernel.storeStateForModeStarting`). The points it places stay in the
/// construction as pending points with no undo step of their own; the
/// completed use is one step (`storeUndoInfoAndStateForModeStarting`), and
/// a use that ends unfinished takes its points away again
/// (`restoreStateForModeStarting`).
struct ToolUse {
    snapshot: Log,
    changed: bool,
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
    tool_use: Option<ToolUse>,
    /// A problem to tell the user (for example a failed rebuild).
    pub notice: Option<String>,
    /// The labels moved away from their place, by object label: the offset
    /// in pixels (`GeoElement.labelOffsetX/Y`). A label drag is no undo step
    /// (`moveLabel`), so they are kept outside the journal: undo leaves them,
    /// and a label that goes away takes its offset along. Not written to
    /// files yet.
    label_offsets: BTreeMap<String, (f64, f64)>,
    /// The algebra rows, the values each was made from, and the state they
    /// were brought up to date with (see `rows.rs`).
    rows: Vec<Row>,
    row_values: Vec<rows::RowKey>,
    rows_at: Option<(u64, u64)>,
    rows_built: u64,
    rows_formatted: u64,
    /// The sides of the polygons, for the colours; made on first use after
    /// the objects changed (see `objects_changed`).
    sides: OnceLock<Sides>,
    /// Tests make every rebuild fail with this message while it is set.
    #[cfg(test)]
    pub fail_rebuilds: Option<String>,
    /// Tests make every replay run a journal whose last definition is
    /// broken (see `Log::broken`), so it fails inside `rebuild`.
    #[cfg(test)]
    pub break_replay: Option<bool>,
    /// Tests make every undo step a full copy of the journal, and every
    /// rebuild record its lines anew, as before the journal shared its
    /// storage, to compare the two.
    #[cfg(test)]
    pub(crate) unshared: bool,
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

    /// The colours of the objects. The sides of the polygons they need are
    /// made once after objects were added or the construction replaced,
    /// and are read only while the document is in the state they were made
    /// for (`rebuilds` and the number of objects), so a frame colours every
    /// object in linear time and never from a stale cache.
    pub fn palette(&self) -> Palette<'_> {
        let key = (self.rebuilds, self.construction.len());
        let sides = self
            .sides
            .get_or_init(|| Sides::of(&self.construction, key));
        Palette::new(&self.construction, sides, key)
    }

    /// A copy of `log` for an undo step: shares its storage (see
    /// `scriptlog`), so it costs a pointer.
    fn copy(&self, log: &Log) -> Log {
        #[cfg(test)]
        if self.unshared {
            return log.deep_copy();
        }
        log.clone()
    }

    /// `log` replayed (see `Log::rebuild`).
    fn replay(&self, log: &Log) -> Result<(Construction, Log), String> {
        #[cfg(test)]
        if self.break_replay.is_some() || self.unshared {
            let log = match self.break_replay {
                Some(wrong_label) => log.broken(wrong_label),
                None => log.clone(),
            };
            return if self.unshared {
                log.rebuild_reference()
            } else {
                log.rebuild()
            };
        }
        log.rebuild()
    }

    /// See the field of the same name.
    #[cfg(test)]
    pub(crate) fn set_unshared(&mut self, on: bool) {
        self.unshared = on;
    }

    /// The journal entries the document holds with its undo history:
    /// `(stored, listed)`, where `stored` counts each shared entry once and
    /// `listed` counts every entry of every state, as full copies took.
    #[cfg(test)]
    pub(crate) fn history_entries(&self) -> (usize, usize) {
        let logs: Vec<&Log> = std::iter::once(&self.log)
            .chain(self.undo.states())
            .chain(self.gesture.as_ref().map(|g| &g.snapshot))
            .chain(self.tool_use.as_ref().map(|t| &t.snapshot))
            .collect();
        let listed = logs.iter().map(|l| l.len()).sum();
        (Log::distinct_entries(logs), listed)
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
        self.label_offsets.clear();
        self.selected = None;
        self.undo.clear();
        self.gesture = None;
        self.tool_use = None;
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
        // The label is chosen once the open tool use is taken back, so that
        // it does not skip the labels of its points.
        let line = self.outside_tool_use(|w| {
            let line = w.with_label(normalize(input));
            w.exec_lines(std::slice::from_ref(&line), false)
                .map(|()| line)
        })?;
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
    /// the whole group stays. A tool use in progress does not finish: its
    /// points go first (see `outside_tool_use`).
    pub fn run_lines(&mut self, lines: &[String]) -> Result<(), String> {
        self.outside_tool_use(|w| w.exec_lines(lines, false))
    }

    /// Runs `change`, a change that does not come from the open tool use
    /// (typed input, a row's delete or show/hide), after taking that use
    /// back: a tool use that does not finish removes its points and records
    /// no undo step (the rule of `restoreStateForModeStarting`; the
    /// reference stores such a change with the pending points in it,
    /// `AlgebraProcessor` / `DefaultDeletionExecutor` calling
    /// `storeUndoInfo`, while the mode's own undo point would still remove
    /// them, which leaves no consistent state to follow). When `change`
    /// fails, nothing happened: the use and its points are put back as
    /// they were.
    fn outside_tool_use<T>(
        &mut self,
        change: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let Some(t) = self.tool_use.take() else {
            return change(self);
        };
        if !t.changed {
            return change(self);
        }
        let pending = self.copy(&self.log);
        if let Err(why) = self.restore(self.copy(&t.snapshot)) {
            self.keep_unrevertable(&why);
            return change(self);
        }
        change(self).inspect_err(|_| match self.restore(pending) {
            Ok(()) => self.tool_use = Some(t),
            Err(why) => self.keep_unrevertable(&why),
        })
    }

    /// Runs the lines of a construction tool's pick inside its tool use
    /// (opened here if it is not): they become part of the one undo step of
    /// the completed use, or go with it when it is cancelled. If one fails,
    /// nothing of these lines stays (what earlier picks placed does).
    pub fn run_tool_lines(&mut self, lines: &[String]) -> Result<(), String> {
        self.begin_tool_use();
        self.exec_lines(lines, true)
    }

    /// Opens a tool use when none is open (see `ToolUse`).
    pub fn begin_tool_use(&mut self) {
        if self.tool_use.is_none() {
            self.tool_use = Some(ToolUse {
                snapshot: self.copy(&self.log),
                changed: false,
            });
        }
    }

    /// The open tool use placed something that is not an undo step yet.
    pub fn tool_use_pending(&self) -> bool {
        self.tool_use.as_ref().is_some_and(|t| t.changed)
    }

    /// Completes the open tool use: what it made is one undo step
    /// (`EuclidianController.storeUndoInfo` after `processMode`). A use that
    /// changed nothing records nothing.
    pub fn end_tool_use(&mut self) {
        if let Some(t) = self.tool_use.take() {
            if t.changed {
                self.undo.record(t.snapshot);
            }
        }
    }

    /// Ends the open tool use unfinished: the points it placed go, and no
    /// undo step is recorded (`Kernel.restoreStateForInitNewMode`, Escape or
    /// a tool switch; `Kernel.undo` in the middle of a tool). Returns
    /// whether anything was taken away.
    pub fn cancel_tool_use(&mut self) -> bool {
        let Some(t) = self.tool_use.take() else {
            return false;
        };
        if !t.changed {
            return false;
        }
        if let Err(why) = self.restore(t.snapshot) {
            self.keep_unrevertable(&why);
        }
        true
    }

    fn exec_lines(&mut self, lines: &[String], in_tool: bool) -> Result<(), String> {
        let snapshot = self.copy(&self.log);
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
                    match &mut self.tool_use {
                        Some(t) if in_tool => t.changed = true,
                        _ => self.undo.record(snapshot),
                    }
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
        if self.construction.len() != self.replay(&self.log).map_or(usize::MAX, |r| r.0.len()) {
            let _ = self.restore(self.copy(&self.log));
        }
        self.undo.clear_redo();
        self.gesture = None;
        self.tool_use = None;
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
        let (c, mut kept) = match self.replay(&log) {
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
        // A label can be reused later; it must not come back hidden, nor
        // with its label moved.
        kept.hidden.retain(|l| c.id(l).is_some());
        self.label_offsets.retain(|l, _| c.id(l).is_some());
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
        // An object the open tool use placed goes with the use.
        self.outside_tool_use(|w| match w.construction.id(&label) {
            Some(_) if w.delete_label(&label) => Ok(()),
            Some(_) => Err(String::new()),
            None => Ok(()),
        })
        .is_ok()
    }

    fn delete_label(&mut self, label: &str) -> bool {
        let snapshot = self.copy(&self.log);
        let mut next = self.copy(&snapshot);
        if !next.remove(label) {
            return false;
        }
        // A deletion inside a gesture (the eraser's stroke) keeps it: the
        // whole stroke is one undo step.
        let gesture = self.gesture.take();
        let restored = self.restore(next);
        self.gesture = gesture;
        if restored.is_err() {
            return false;
        }
        self.note_change_from(snapshot);
        true
    }

    /// Whether `undo` does something: a step to go back to, or the pending
    /// points of a tool use to take away.
    pub fn can_undo(&self) -> bool {
        self.tool_use_pending() || self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// The undo and redo steps held.
    pub fn undo_depth(&self) -> (usize, usize) {
        self.undo.depth()
    }

    /// Steps back. In the middle of a tool use this only takes its pending
    /// points away and is no real undo (`Kernel.undo` calls
    /// `restoreStateForModeStarting`).
    pub fn undo(&mut self) -> bool {
        if self.tool_use.is_some() {
            return self.cancel_tool_use();
        }
        let Some(prev) = self.undo.undo(self.copy(&self.log)) else {
            return false;
        };
        if self.restore(self.copy(&prev)).is_err() {
            // Put the stacks back as they were.
            self.undo.redo(prev);
            return false;
        }
        true
    }

    /// Steps forward; a tool use in progress is cancelled first (its pending
    /// points were never a step that the redo steps follow from).
    pub fn redo(&mut self) -> bool {
        if !self.undo.can_redo() {
            return false;
        }
        self.cancel_tool_use();
        let Some(next) = self.undo.redo(self.copy(&self.log)) else {
            return false;
        };
        if self.restore(self.copy(&next)).is_err() {
            self.undo.undo(next);
            return false;
        }
        true
    }

    /// Marks the start of a drag: its moves become a single undo step.
    /// A tool use in progress does not finish: its points go first (see
    /// `outside_tool_use`). The point tool's use is complete by then (its
    /// press placed the point), so its drag is a step of its own, as in the
    /// reference (the point's `storeUndoInfo` and the drag's update action
    /// at the release, `wrapMouseReleasedND`).
    pub fn begin_gesture(&mut self) {
        self.cancel_tool_use();
        self.gesture = Some(Gesture {
            snapshot: self.copy(&self.log),
            committed: false,
        });
    }

    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    /// Ends the drag in progress keeping its changes but not its undo step
    /// (the reference app's drag that stayed within the click threshold,
    /// spec F2; `interact::EVERY_DRAG_IS_A_STEP` off).
    pub fn end_gesture_without_step(&mut self) {
        if self.gesture.take().is_some_and(|g| g.committed) {
            self.undo.discard_last();
        }
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
        self.note_change_with(|w| w.copy(&w.log));
    }

    /// Records the undo step for a change made from the state `before`:
    /// the gesture's one step if a gesture is in progress, else a step of
    /// its own.
    fn note_change_from(&mut self, before: Log) {
        self.note_change_with(move |_| before);
    }

    fn note_change_with(&mut self, before: impl FnOnce(&Self) -> Log) {
        match &self.gesture {
            Some(g) if !g.committed => {
                let snapshot = self.copy(&g.snapshot);
                self.undo.record(snapshot);
                if let Some(g) = &mut self.gesture {
                    g.committed = true;
                }
            }
            Some(_) => {}
            None => {
                let before = before(self);
                self.undo.record(before);
            }
        }
    }

    /// The polygon whose command also made the segment `id` (one of its
    /// sides, `Segment(A, B, poly)` in the reference), if any.
    pub fn polygon_of_side(&self, id: Id) -> Option<Id> {
        let c = &self.construction;
        if id.index() >= c.len() || c.kind(id) != Kind::Segment {
            return None;
        }
        let label = c.label(id);
        self.log
            .multi_output()
            .find(|g| g[0] != label && g.iter().any(|l| l == label))
            .and_then(|g| c.id(&g[0]))
            .filter(|&p| c.kind(p) == Kind::Polygon)
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
        let _ = self.outside_tool_use(|w| {
            // An object the open tool use placed went with it.
            if w.construction.id(&label).is_some() {
                w.undo.record(w.copy(&w.log));
                if !w.log.hidden.remove(&label) {
                    w.log.hidden.insert(label);
                }
            }
            Ok(())
        });
    }

    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> bool {
        self.set_point(id, x, y, false)
    }

    /// Puts a point that the open tool use placed at `(x, y)` (a point on a
    /// path created where the pointer was), as part of that use.
    pub fn move_pending_point(&mut self, id: Id, x: f64, y: f64) -> bool {
        self.begin_tool_use();
        self.set_point(id, x, y, true)
    }

    fn set_point(&mut self, id: Id, x: f64, y: f64, in_tool: bool) -> bool {
        if !in_tool && self.tool_use_pending() {
            // A drag is not part of the use: the use does not finish. The
            // point may be one of its own, gone with it.
            let label = self.construction.label(id).to_owned();
            self.cancel_tool_use();
            match self.construction.id(&label) {
                Some(again) if again == id => {}
                _ => return false,
            }
        }
        if id.index() >= self.construction.len() {
            return false;
        }
        // Compared by bits, as the kernel does: `0` to `-0` is a change.
        let bits = |v: Value| match v {
            Value::Point { x, y } | Value::Vector { x, y } => Some((x.to_bits(), y.to_bits())),
            _ => None,
        };
        let old = bits(self.construction.value(id));
        let same = old == Some((x.to_bits(), y.to_bits()));
        if self.construction.move_point(id, x, y).is_err() || same {
            return false;
        }
        // A point on a path goes where the kernel projects the position: it
        // may stay where it was.
        if bits(self.construction.value(id)) == old {
            return false;
        }
        // The journal does not have the move yet: the undo step is taken
        // from it as it is.
        match &mut self.tool_use {
            Some(t) if in_tool => t.changed = true,
            _ => self.note_change(),
        }
        let label = self.construction.label(id).to_owned();
        self.log.record_set(&label, Setting::Point(x, y));
        self.version += 1;
        true
    }

    /// The point that `rhs` (such as `Intersect(c, f, 2)`) would make now,
    /// without adding it: the line runs on the construction and is rolled
    /// back at once. `None` when it fails or makes no defined point.
    pub fn probe_point(&mut self, rhs: &str) -> Option<(f64, f64)> {
        let c = &mut self.construction;
        let at = c.checkpoint();
        let before = c.len();
        let made = geo_kernel::script::exec(c, &format!("Probe_ = {rhs}")).is_ok();
        let point = match c.ids().nth(before).map(|id| c.value(id)) {
            Some(Value::Point { x, y }) if made && x.is_finite() && y.is_finite() => Some((x, y)),
            _ => None,
        };
        // A checkpoint taken just above, with nothing moved since: the
        // rollback cannot be refused.
        let _ = c.rollback(at);
        point
    }

    /// Translates the free points `ids` by `(dx, dy)` within the drag in
    /// progress (`GeoPoint.movePoint` without an end position: a coordinate
    /// that moved by more than `Kernel.MIN_PRECISION` is cleaned at the
    /// fifth decimal, `checkDecimalFraction`). Returns whether anything
    /// moved.
    pub fn translate(&mut self, ids: &[Id], (dx, dy): (f64, f64)) -> bool {
        const MIN_PRECISION: f64 = 1e-5;
        let mut moved = false;
        for &id in ids {
            if id.index() >= self.construction.len() {
                continue;
            }
            let Value::Point { x, y } = self.construction.value(id) else {
                continue;
            };
            let (mut x1, mut y1) = (x + dx, y + dy);
            if dx.abs() > MIN_PRECISION {
                x1 = check_decimal_fraction(x1);
            }
            if dy.abs() > MIN_PRECISION {
                y1 = check_decimal_fraction(y1);
            }
            moved |= self.move_point(id, x1, y1);
        }
        moved
    }

    /// The offset of the label of `id` from its place, in pixels.
    pub fn label_offset(&self, id: Id) -> Option<(f64, f64)> {
        if self.label_offsets.is_empty() || id.index() >= self.construction.len() {
            return None;
        }
        self.label_offsets.get(self.construction.label(id)).copied()
    }

    /// Moves the label of `id` to `offset` pixels from its place; no undo
    /// step (`EuclidianController.moveLabel`).
    pub fn set_label_offset(&mut self, id: Id, offset: (f64, f64)) {
        if id.index() >= self.construction.len() {
            return;
        }
        let label = self.construction.label(id).to_owned();
        if offset == (0.0, 0.0) {
            self.label_offsets.remove(&label);
        } else {
            self.label_offsets.insert(label, offset);
        }
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
        if same {
            return false;
        }
        if self.tool_use_pending() {
            // Not part of the open tool use (its points come after every
            // number): the use does not finish.
            self.cancel_tool_use();
        }
        if self.construction.set_number(id, v).is_err() {
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
