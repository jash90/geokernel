//! The document the window edits: construction, the script lines that built
//! it, selection, visibility, undo and the cached object list. No UI types.

#[cfg(test)]
use crate::describe;
use crate::history::History;
use crate::interact::TextMove;
use crate::labels::{next_label, next_numbered};
use crate::scriptlog::{exec_definition, normalize, Log, Setting};
use crate::snap::check_decimal_fraction;
use crate::style::{Palette, Sides};
use crate::undo::Undo;
use crate::view::Camera;
pub use file::{import_notice, rejected_notice, SNAPSHOT_VERSION};
pub use geo_import::export::View as FileView;
use geo_kernel::kernel::text::{TextOverrides, TextPosition};
use geo_kernel::kernel::{Construction, Def, Id, Kind, Value};
use geo_kernel::script::saved::Property;
use geo_kernel::script::text::Properties as TextProperties;
use geo_kernel::script::{find_top_level, BoundaryOutputs};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    /// Typeset definition (functions) or value (matrices), when available.
    pub formula: Option<Box<str>>,
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

/// One undo state: the journal, and with it the camera of the drawing
/// before the change — the reference app's undo state carries the view
/// settings next to the construction (`AppState`, spec F3), so undoing a
/// step also puts the view back. A `view_only` step changed no object:
/// the journal it holds is the document's very one and is not replayed.
#[derive(Clone, Debug)]
struct UndoState {
    log: Log,
    cas: crate::cas::Table,
    cas_only: bool,
    camera: Option<Camera>,
    view_only: bool,
}

#[derive(Default)]
pub struct Workbench {
    construction: Construction,
    cas: crate::cas::Table,
    log: Log,
    boundary_outputs: BoundaryOutputs,
    file_settings: geo_import::Settings,
    file_import_losses: (usize, usize),
    file_object_styles: HashMap<String, geo_import::export::ObjectStyle>,
    file_slider_animation: BTreeMap<String, geo_import::export::SliderAnimation>,
    file_view: Option<geo_import::export::View>,
    file_cas_visible: Option<bool>,
    file_spreadsheet_view: Option<geo_import::SpreadsheetView>,
    version: u64,
    /// Counts the times the construction was rebuilt or replaced. Object ids
    /// are only valid within one such period.
    rebuilds: u64,
    pub selected: Option<Id>,
    pub history: History,
    undo: Undo<UndoState>,
    gesture: Option<Gesture>,
    style_before: Option<UndoState>,
    tool_use: Option<ToolUse>,
    /// The camera of the drawing, as the window last reported it for the
    /// undo steps (see `set_camera`).
    camera: Option<Camera>,
    /// The camera an undo or redo step restored, for the window to take
    /// over (see `take_restored_view`).
    restored_view: Option<Camera>,
    /// A problem to tell the user (for example a failed rebuild).
    pub notice: Option<String>,
    /// The labels moved away from their place, by object label: the offset
    /// in pixels (`GeoElement.labelOffsetX/Y`). A label drag is no undo step
    /// (`moveLabel`), so they are kept outside the journal: undo leaves them,
    /// and a label that goes away takes its offset along. Included in .ggb export.
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

    pub fn cas_rows(&self) -> &[std::sync::Arc<crate::cas::Entry>] {
        self.cas.rows()
    }

    /// Submits a CAS row at a zero-based index, including the next empty row.
    pub fn submit_cas(&mut self, index: usize, input: &str) -> bool {
        let mut before = self.state(self.copy(&self.log));
        before.cas_only = true;
        if !self.cas.submit(&self.construction, index, input) {
            return false;
        }
        self.record_undo(before);
        true
    }

    /// Deletes a CAS row and its dependents.
    pub fn delete_cas(&mut self, index: usize) -> bool {
        let mut before = self.state(self.copy(&self.log));
        before.cas_only = true;
        if !self.cas.delete(index) {
            return false;
        }
        self.record_undo(before);
        true
    }

    pub fn construction(&self) -> &Construction {
        &self.construction
    }

    pub fn set_locus_view(&mut self, view: crate::interact::ViewTransform) {
        let v = view.view();
        self.construction.set_locus_view(
            v.xmin,
            v.xmax,
            v.ymin,
            v.ymax,
            view.xscale(),
            view.yscale(),
        );
    }

    /// Publishes the current graphics view for the objects that read it
    /// (`Corner`, `AxisStepX`, `AxisStepY`), recomputing their dependents
    /// when the window or scale changed. Updating view readers bumps the version,
    /// so the algebra rows of the view-dependent objects (and of whatever
    /// reads them) are formatted anew on the next refresh, both after a
    /// pan or zoom and after a rebuild restored the view.
    pub fn set_view(&mut self, view: crate::interact::ViewTransform) {
        let v = view.view();
        if self
            .construction
            .set_view(v.xmin, v.xmax, v.ymin, v.ymax, view.xscale(), view.yscale())
        {
            self.version += 1;
        }
    }

    /// Objects exposed to the algebra view and document clients.
    pub fn object_ids(&self) -> impl Iterator<Item = Id> + '_ {
        crate::scriptlog::object_ids(&self.construction)
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
        Palette::new(&self.construction, sides, key).with_file_styles(&self.file_object_styles)
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

    /// The undo state of a journal snapshot: the journal and the camera
    /// as it was when the change began.
    fn state(&self, log: Log) -> UndoState {
        UndoState {
            log,
            cas: self.cas.clone(),
            cas_only: false,
            camera: self.camera,
            view_only: false,
        }
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
                log.rebuild_reference(&self.construction)
            } else {
                log.rebuild_with_text_anchors(Some(&self.construction))
            };
        }
        log.rebuild_with_text_anchors(Some(&self.construction))
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
            .chain(self.undo.states().map(|s| &s.log))
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

    /// Revision of the construction and its edited properties.
    pub(crate) fn version(&self) -> u64 {
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
        self.boundary_outputs = log.boundary_outputs(&c);
        self.construction = c;
        self.cas = crate::cas::Table::default();
        self.objects_changed();
        self.log = log;
        self.label_offsets.clear();
        self.file_settings = geo_import::Settings::default();
        self.file_import_losses = (0, 0);
        self.style_before = None;
        self.file_object_styles.clear();
        self.file_slider_animation.clear();
        self.file_view = None;
        self.file_cas_visible = None;
        self.file_spreadsheet_view = None;
        self.selected = None;
        self.undo.clear();
        self.gesture = None;
        self.tool_use = None;
        self.camera = None;
        self.restored_view = None;
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
        self.load_imported_with_conditions(src, &[], hidden, &[], notice)
    }

    fn load_imported_with_conditions(
        &mut self,
        src: &str,
        assets: &[geo_kernel::script::saved::Property],
        hidden: &[String],
        conditions: &[(String, String)],
        notice: Option<String>,
    ) -> Result<(), String> {
        let (mut c, mut log) = Log::load_with_assets(src, assets)?;
        for label in hidden {
            if c.id(label).is_some() {
                log.hidden.insert(label.clone());
            }
        }
        for (target, source) in conditions {
            log.set_condition(&mut c, target, source)?;
        }
        self.replace(c, log);
        self.notice = notice;
        Ok(())
    }

    /// Runs what the user typed: `f = Line(A, B)`, `f: Line(A, B)`, or just a
    /// definition, which gets the next free label of its kind.
    pub fn run_line(&mut self, input: &str) -> Result<(), String> {
        let line = normalize(input);
        if self.construction.id(&line).is_some() {
            return Ok(());
        }
        // The label is chosen once the open tool use is taken back, so that
        // it does not skip the labels of its points.
        let line = self.outside_tool_use(|w| {
            let line = w.with_label(line);
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
        if line.is_empty()
            || find_top_level(&line, '=').is_some()
            || geo_kernel::script::is_construction_step_command(&line)
            || geo_kernel::spreadsheet::is_fill_line(&line)
        {
            return line;
        }
        let shape = self.log.rebuild().ok().and_then(|(mut scratch, _)| {
            let id = exec_definition(&mut scratch, &format!("Probe_ = {line}")).ok()??;
            let id = scratch.id("Probe_").unwrap_or(id);
            let matrix = scratch.elements(id).is_some_and(|list| list.is_matrix());
            Some((scratch.kind(id), matrix))
        });
        let is_taken = |label: &str| self.construction.id(label).is_some();
        let label = match shape {
            Some((Kind::List, true)) => next_numbered("m", is_taken),
            Some((kind, _)) => next_label(kind, is_taken),
            None => next_label(Kind::Line, is_taken),
        };
        format!("{label} = {line}")
    }

    /// Runs several script lines as one undo step. If one fails, nothing of
    /// the whole group stays. A tool use in progress does not finish: its
    /// points go first (see `outside_tool_use`).
    pub fn run_lines(&mut self, lines: &[String]) -> Result<(), String> {
        if lines.iter().all(|line| {
            let line = normalize(line);
            line.is_empty() || self.construction.id(&line).is_some()
        }) {
            return Ok(());
        }
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
        self.finish_style_edits();
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
        self.finish_style_edits();
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
                self.record_undo(self.state(t.snapshot));
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
        let mut updated = false;
        let mut redefined = false;
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
            if in_tool {
                use geo_kernel::script::saved;
                if let Some(property) = saved::parse(&line) {
                    match property.and_then(|property| {
                        property.apply(&mut self.construction)?;
                        Ok(property)
                    }) {
                        Ok(property) => {
                            self.log.record_saved_property(property);
                            updated = true;
                        }
                        Err(error) => {
                            failure = Some(error);
                            break;
                        }
                    }
                    continue;
                }
            }
            let before = self.construction.len();
            let fills_cells = geo_kernel::spreadsheet::is_fill_line(&line);
            let step_before = self.construction.step();
            let target = geo_kernel::script::assignment_target(&self.construction, &line);
            let slider = geo_kernel::script::slider_assignment_target(&self.construction, &line);
            let assigns_slider = slider.is_some();
            self.construction.take_hidden_requests();
            // Only lines that succeeded go into the log: a failed one would
            // fail again on every replay.
            match exec_definition(&mut self.construction, &line) {
                Ok(first) => {
                    let added = self.construction.len() > before;
                    let mutated = !added
                        && (assigns_slider
                            || fills_cells
                            || self.construction.step() != step_before);
                    let replaced = target.is_some() && first == target;
                    if added || mutated || replaced {
                        // A stroke keeps the points of its creation: the
                        // journal replays them, not the smoothing in
                        // another view.
                        let line = first
                            .and_then(|id| {
                                geo_kernel::script::frozen_stroke_line(
                                    &self.construction,
                                    id,
                                    &line,
                                )
                            })
                            .unwrap_or(line);
                        self.log.record_new_definition(
                            &mut self.construction,
                            before,
                            first,
                            &line,
                        );
                        self.log.apply_hidden_requests(&mut self.construction);
                        if let Some(first) = first.filter(|_| added) {
                            self.boundary_outputs.record(
                                &self.construction,
                                &line,
                                first,
                                self.construction.len() - before,
                            );
                        }
                        if let Some(first) = first.filter(|_| replaced) {
                            self.grow_boundary_outputs(std::iter::once(first));
                        }
                    }
                    if self.construction.step() != step_before {
                        self.log
                            .grow_after_step(&mut self.construction, &mut self.boundary_outputs);
                    }
                    if let Some(id) = slider.filter(|_| mutated) {
                        self.grow_boundary_outputs(std::iter::once(id));
                    }
                    updated |= mutated;
                    redefined |= replaced;
                }
                Err(e) => {
                    updated |= fills_cells;
                    failure = Some(e);
                    break;
                }
            }
        }
        let objects_added = self.construction.len() != n0;
        if objects_added || redefined {
            self.objects_changed();
        }
        let changed = objects_added || updated || redefined;
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
                        _ => {
                            let state = self.state(snapshot);
                            self.record_undo(state);
                        }
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
        self.restore_state(log, false)
    }

    fn restore_state(&mut self, log: Log, snapshot: bool) -> Result<(), String> {
        #[cfg(test)]
        if let Some(e) = self.fail_rebuilds.clone() {
            self.notice = Some(format!("Nie można odtworzyć konstrukcji: {e}"));
            return Err(e);
        }
        let (mut c, mut kept) = match self.replay(&log) {
            Ok(r) => r,
            Err(e) => {
                self.notice = Some(format!("Nie można odtworzyć konstrukcji: {e}"));
                return Err(e);
            }
        };
        if snapshot {
            kept.reset_snapshot_records(&mut c);
            kept.reset_inequality_helpers(&mut c);
        }
        self.install_replay(c, kept);
        Ok(())
    }

    fn install_replay(&mut self, mut c: Construction, mut kept: Log) {
        // a replay plays nothing again: the sliders keep where they were
        c.carry_animations_from(&self.construction);
        let selected = self
            .selected
            .filter(|id| id.index() < self.construction.len())
            .map(|id| self.construction.label(id).to_owned());
        self.selected = selected.and_then(|l| c.id(&l));
        // A label can be reused later; it must not come back hidden, nor
        // with its label moved.
        kept.hidden.retain(|l| c.id(l).is_some());
        self.label_offsets.retain(|l, _| c.id(l).is_some());
        self.file_object_styles.retain(|l, _| c.id(l).is_some());
        self.file_slider_animation.retain(|l, _| c.id(l).is_some());
        self.boundary_outputs = kept.boundary_outputs(&c);
        self.construction = c;
        self.objects_changed();
        self.log = kept;
        self.gesture = None;
        self.notice = None;
        self.version += 1;
        self.rebuilds += 1;
    }

    pub fn edit_spreadsheet_cell(
        &mut self,
        address: geo_kernel::spreadsheet::Address,
        input: &str,
    ) -> Result<(), String> {
        if input.is_empty() {
            let Some(id) = self.construction.spreadsheet_cell(address) else {
                return Ok(());
            };
            let label = self.construction.label(id);
            // A fill can own multiple cells in a single journal entry.
            if self
                .log
                .define_entry(label)
                .is_some_and(|(_, labels)| labels.len() == 1)
                && self.delete(id)
            {
                return Ok(());
            }
        }
        self.run_line(&geo_kernel::spreadsheet::edit_line(address, input))
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
        // Where the entry about to go was, so the start points it froze on
        // the lines that stay are written at the same place again (GeoGebra
        // installed them on the lines themselves and keeps them when the
        // locus goes).
        let Some((at, made)) = next.define_entry(label) else {
            return false;
        };
        let made: Vec<String> = made.to_vec();
        // A deletion inside a gesture (the eraser's stroke) keeps it: the
        // whole stroke is one undo step.
        // Only the starts the deleted entry froze are written into the
        // journal: every locus that stays freezes its start again as its
        // own line replays. What is written survives every later rebuild
        // (further deletions, the undo and redo of them), exactly as the
        // construction being replaced had.
        let mut placed = 0;
        for (line, x, y, owner) in self.construction.frozen_line_starts() {
            if made.iter().any(|m| m == &owner) {
                next.keep_frozen_start(at + 1 + placed, &line, x, y);
                placed += 1;
            }
        }
        if !next.remove(label, &self.construction) {
            return false;
        }
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
        self.style_before.is_some() || self.tool_use_pending() || self.undo.can_undo()
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
        self.finish_style_edits();
        if self.tool_use.is_some() {
            return self.cancel_tool_use();
        }
        let mut current = self.state(self.copy(&self.log));
        // A view-only step holds the very journal the document has, so the
        // state kept for redo is view-only too: it never replays either.
        current.view_only = self.undo.undo_is_view_only(|s| s.view_only);
        current.cas_only = self.undo.undo_is_view_only(|s| s.cas_only);
        let Some(prev) = self.undo.undo(current) else {
            return false;
        };
        if self.step_back(prev.clone()).is_err() {
            // Put the stacks back exactly as they were: the step taken
            // above goes back as the redo state it was taken for.
            self.undo.redo(prev);
            return false;
        }
        true
    }

    /// Goes to the state `next` of a step: the journal is replayed — a
    /// `view_only` step changed no object and holds the very journal the
    /// document has, so it needs none — and the camera it names is handed
    /// to the window. On error the document stays as it was.
    fn step_back(&mut self, next: UndoState) -> Result<(), String> {
        if next.view_only || next.cas_only {
            self.notice = None;
        } else {
            self.restore_state(next.log, true)?;
        }
        self.cas = next.cas;
        self.restored_view = next.camera.filter(|c| Some(*c) != self.camera);
        Ok(())
    }

    /// Steps forward; a tool use in progress is cancelled first (its pending
    /// points were never a step that the redo steps follow from).
    pub fn redo(&mut self) -> bool {
        if !self.undo.can_redo() {
            return false;
        }
        self.cancel_tool_use();
        let mut current = self.state(self.copy(&self.log));
        // As in `undo`: the way back over a view-only step is view-only.
        current.view_only = self.undo.redo_is_view_only(|s| s.view_only);
        current.cas_only = self.undo.redo_is_view_only(|s| s.cas_only);
        let Some(next) = self.undo.redo(current) else {
            return false;
        };
        if self.step_back(next.clone()).is_err() {
            // Put the stacks back exactly as they were: the redo step
            // taken above goes back where it came from.
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
        self.finish_style_edits();
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

    /// Whether a drag's gesture is open (its moves become its one undo
    /// step). A drag that a pinch suspends and that then goes on opens a
    /// new one.
    pub fn gesture_open(&self) -> bool {
        self.gesture.is_some()
    }

    /// The camera of the drawing, as the window reports it every frame, so
    /// that every undo step carries the view it began from (the reference
    /// app's undo state holds the view settings, spec F3).
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = Some(camera);
    }

    /// The camera as the window last reported it.
    pub fn camera(&self) -> Option<Camera> {
        self.camera
    }

    /// Records one undo step for a view change that changed no object: the
    /// journal stays as it is and the step brings the camera `before` back
    /// (`OwnerRules::view_changes_are_steps`; the reference app stores view
    /// changes lazily, spec F3). A view that ended where it began is no
    /// step. A run of them never evicts a construction step
    /// (`Undo::record_view`).
    pub fn record_view_step(&mut self, before: Camera) {
        self.finish_style_edits();
        if self.camera.is_some_and(|now| now != before) {
            let mut state = self.state(self.copy(&self.log));
            state.camera = Some(before);
            state.view_only = true;
            self.undo.record_view(state, |s| s.view_only);
        }
    }

    /// The camera that an undo or redo step restored, once: the window
    /// takes it over as the view it draws.
    pub fn take_restored_view(&mut self) -> Option<Camera> {
        self.restored_view.take()
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
                let state = self.state(snapshot);
                self.record_undo(state);
                if let Some(g) = &mut self.gesture {
                    g.committed = true;
                }
            }
            Some(_) => {}
            None => {
                let before = self.state(before(self));
                self.record_undo(before);
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

    /// Selected sides and generated vertices owned by a selected polygon.
    pub fn selected_polygon_outputs(
        &self,
        selected: &BTreeSet<Id>,
        work_left: &mut usize,
    ) -> Option<BTreeSet<Id>> {
        self.log
            .selected_polygon_outputs(self.construction(), selected, work_left)
    }

    pub fn is_visible(&self, id: Id) -> bool {
        if id.index() >= self.construction.len() {
            return true;
        }
        self.log.is_visible(&self.construction, id)
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
                w.record_undo(w.state(w.copy(&w.log)));
                if !w.log.hidden.remove(&label) {
                    w.log.hidden.insert(label);
                }
            }
            Ok(())
        });
    }

    pub fn toggle_label(&mut self, id: Id) {
        self.toggle_label_in_mode(id);
        self.finish_style_edits();
    }

    pub(crate) fn toggle_label_in_mode(&mut self, id: Id) {
        let c = self.construction();
        if id.index() >= c.len()
            || matches!(c.kind(id), Kind::Image | Kind::Locus | Kind::PenStroke)
        {
            return;
        }
        let mut style = c.drawing_style(id);
        style.show_label = !style.show_label;
        style.label_edited = true;
        self.edit_drawing_style(id, style);
    }

    pub(crate) fn copy_drawing_style(&mut self, source: Id, target: Id) {
        let mut style = self.construction.drawing_style(source);
        let color = self.palette().color(source);
        style.color = Some([color.r, color.g, color.b]);
        style.label_edited = true;
        style.sector_outline_only = self.construction.drawing_style(target).sector_outline_only;
        self.edit_drawing_style(target, style);
    }

    fn edit_drawing_style(&mut self, id: Id, style: geo_kernel::kernel::DrawingStyle) {
        if style == self.construction.drawing_style(id) {
            return;
        }
        if self.style_before.is_none() {
            self.style_before = Some(self.state(self.copy(&self.log)));
            self.undo.clear_redo();
        }
        let label = self.construction.label(id).to_owned();
        self.construction.set_drawing_style(id, style);
        self.log
            .record_saved_property(Property::DrawingStyle { label, style });
        self.version += 1;
    }

    pub(crate) fn finish_style_edits(&mut self) {
        if let Some(before) = self.style_before.take() {
            self.undo.record(before);
        }
    }

    fn record_undo(&mut self, before: UndoState) {
        self.finish_style_edits();
        self.undo.record(before);
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
            Value::Point { x, y } | Value::Vector { x, y } => Some([x, y, 1.0].map(f64::to_bits)),
            Value::HomogeneousPoint { x, y, z } => Some([x, y, z].map(f64::to_bits)),
            _ => None,
        };
        let old = bits(self.construction.stored_value(id));
        let same = old == Some([x, y, 1.0].map(f64::to_bits));
        let Ok(recomputed) = self.construction.move_point(id, x, y) else {
            return false;
        };
        if same {
            return false;
        }
        // A point on a path goes where the kernel projects the position: it
        // may stay where it was.
        if bits(self.construction.stored_value(id)) == old {
            return false;
        }
        // The journal does not have the move yet: the undo step is taken
        // from it as it is.
        match &mut self.tool_use {
            Some(t) if in_tool => t.changed = true,
            _ => self.note_change(),
        }
        let label = self.construction.label(id).to_owned();
        self.log
            .record_update(&self.construction, &label, Setting::Point(x, y));
        if recomputed > 0 {
            self.grow_boundary_outputs(std::iter::once(id));
        }
        self.version += 1;
        true
    }

    fn grow_boundary_outputs(&mut self, changed: impl Iterator<Item = Id> + Clone) {
        let before = self.construction.len();
        let relabelled = self.log.grow_boundary_outputs(
            &mut self.construction,
            &mut self.boundary_outputs,
            changed,
        );
        if relabelled || self.construction.len() != before {
            self.objects_changed();
            // Existing outputs acquire new siblings for the same command.
            self.rows_at = None;
        }
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
        let free = |c: &Construction, id: Id| {
            id.index() < c.len() && matches!(c.def(id), Def::FreePoint { .. })
        };
        if self.tool_use_pending() && ids.iter().any(|&id| free(&self.construction, id)) {
            self.cancel_tool_use();
        }

        let mut positions = Vec::new();
        for &id in ids {
            if !free(&self.construction, id) {
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
            if x1.is_finite()
                && y1.is_finite()
                && (x.to_bits() != x1.to_bits() || y.to_bits() != y1.to_bits())
            {
                positions.push((id, x1, y1));
            }
        }
        self.move_free_points(&positions)
    }

    pub fn rotate(&mut self, id: Id, center: (f64, f64), angle: f64) -> bool {
        if angle == 0.0 || !angle.is_finite() {
            return false;
        }
        let c = self.construction();
        let Some(points) = crate::interact::rotation_points(c, id) else {
            return false;
        };
        let (sin, cos) = angle.sin_cos();
        let positions: Vec<_> = points
            .into_iter()
            .filter_map(|point| {
                let Value::Point { x, y } = c.value(point) else {
                    return None;
                };
                let dx = x - center.0;
                let dy = y - center.1;
                let nx = center.0 + dx * cos - dy * sin;
                let ny = center.1 + dx * sin + dy * cos;
                (nx.is_finite() && ny.is_finite() && (nx != x || ny != y))
                    .then_some((point, nx, ny))
            })
            .collect();
        self.move_free_points(&positions)
    }

    fn move_free_points(&mut self, positions: &[(Id, f64, f64)]) -> bool {
        if positions.is_empty() {
            return false;
        }
        let Ok(recomputed) = self.construction.move_points(positions) else {
            return false;
        };
        self.note_change();
        self.log.record_move_points(&self.construction, positions);
        if recomputed > 0 {
            self.grow_boundary_outputs(positions.iter().map(|&(id, _, _)| id));
        }
        self.version += 1;
        true
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

    /// Moves a text while keeping the other presentation properties and its definition.
    pub fn move_text(&mut self, id: Id, movement: TextMove) {
        let Some(style) = self.construction.text_presentation(id).filter(|s| !s.fixed) else {
            return;
        };
        let (position, offset) = match (movement, style.position) {
            (TextMove::World { x, y }, TextPosition::Origin | TextPosition::World { .. })
                if x.is_finite() && y.is_finite() =>
            {
                let old = match style.position {
                    TextPosition::World { x, y } => (x, y),
                    _ => (0.0, 0.0),
                };
                if (x, y) == old {
                    return;
                }
                (Some(TextPosition::World { x, y }), None)
            }
            (TextMove::Screen { x, y }, TextPosition::Screen { x: old_x, y: old_y }) => {
                let (x, y) = (f64::from(x), f64::from(y));
                if (x, y) == (old_x, old_y) {
                    return;
                }
                (Some(TextPosition::Screen { x, y }), None)
            }
            (TextMove::Offset(offset), TextPosition::Point(_)) if offset != style.offset => {
                (None, Some(offset))
            }
            _ => return,
        };
        let label = self.construction.label(id).to_owned();
        let props = TextProperties {
            label: &label,
            overrides: TextOverrides {
                position,
                offset,
                ..TextOverrides::default()
            },
        };
        if props.apply(&mut self.construction).is_ok() {
            self.note_change();
            self.log.record_text_properties(props);
        }
    }

    /// Moves the slider track without changing the slider's number.
    pub fn move_slider(&mut self, id: Id, (x, y): (f64, f64)) {
        let style = self.construction.slider_style(id);
        if style.fixed
            || self.construction.slider_interval(id).is_none()
            || !x.is_finite()
            || !y.is_finite()
        {
            return;
        }
        let position = if style.absolute {
            (x.round().max(0.0), y.round().max(0.0))
        } else {
            (x, y)
        };
        if position == self.construction.slider_position(id, style) {
            return;
        }
        let mut style = style.clone();
        style.position = Some(position);
        self.note_change();
        self.construction.set_slider_style(id, style.clone());
        self.log
            .record_saved_property(geo_kernel::script::saved::Property::SliderStyle {
                label: self.construction.label(id).to_owned(),
                style,
            });
        self.version += 1;
    }

    /// Moves an unlocked checkbox without toggling it or its dependents.
    pub fn move_checkbox(&mut self, id: Id, position: [i32; 2]) {
        let mut style = self.construction.checkbox_style(id);
        let [x, y] = position.map(f64::from);
        if style.fixed
            || self
                .construction
                .checkbox_position(id)
                .is_none_or(|p| p == (x, y))
        {
            return;
        }
        style.position = Some(position);
        self.note_change();
        self.construction.set_checkbox_style(id, style);
        self.log
            .record_saved_property(geo_kernel::script::saved::Property::CheckboxStyle {
                label: self.construction.label(id).to_owned(),
                style,
            });
        self.version += 1;
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
        let Ok(recomputed) = self.construction.set_number(id, v) else {
            return false;
        };
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log
            .record_update(&self.construction, &label, Setting::Number(v));
        if recomputed > 0 {
            self.grow_boundary_outputs(std::iter::once(id));
        }
        self.version += 1;
        true
    }

    /// Whether a slider of the construction is playing.
    pub fn is_animating(&self) -> bool {
        self.construction.is_animating()
    }

    /// Whether something can be played or paused: a slider has an animation.
    pub fn has_animation(&self) -> bool {
        self.construction.has_animations()
    }

    /// Plays or pauses the animation, as the reference's play button does
    /// (`AnimationManager.startAnimation` and `stopAnimation`). Pausing
    /// keeps every slider where it is; playing goes on from there.
    pub fn set_animation_running(&mut self, running: bool) {
        self.construction.set_animation_running(running);
        self.version += 1;
    }

    /// One animation frame of the playing sliders at `frame_rate` frames per
    /// second (`AnimationManager.sliderStep`). The values are not journaled,
    /// as the reference does not store animation frames as undo steps.
    /// Returns whether a value changed.
    pub fn animate(&mut self, frame_rate: f64) -> bool {
        let changed = self.construction.animate(frame_rate);
        if changed {
            self.version += 1;
        }
        changed
    }

    pub fn set_boolean(&mut self, id: Id, value: bool) -> bool {
        if id.index() >= self.construction.len()
            || self.construction.value(id) == Value::Bool(value)
        {
            return false;
        }
        if self.tool_use_pending() {
            self.cancel_tool_use();
        }
        let Ok(recomputed) = self.construction.set_boolean(id, value) else {
            return false;
        };
        self.note_change();
        let label = self.construction.label(id).to_owned();
        self.log
            .record_update(&self.construction, &label, Setting::Boolean(value));
        if recomputed > 0 {
            self.grow_boundary_outputs(std::iter::once(id));
        }
        self.version += 1;
        true
    }
}

mod editor;
mod file;
mod rows;
#[cfg(test)]
mod tests;
mod vector_polygon;
