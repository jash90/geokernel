//! The state of the window: the document, the camera, the active tool, the
//! input row and the open panels, and what a finished tool or the input row
//! does to them. The window (`geo-app`) draws it and feeds it input.

use crate::errors;
use crate::geom::Rect;
use crate::gesture::Gesture;
use crate::interact::{Drag, Hold, ViewTransform};
use crate::keyboard::Keyboard;
use crate::layout::{Overlays, Regions};
use crate::metrics::Metrics;
use crate::snap::Snap;
use crate::tools::{Outcome, Tool, ToolState, GROUPS};
use crate::view::Camera;
use crate::workbench::Workbench;
use geo_kernel::kernel::Id;
use std::cell::RefCell;

/// Which parts of the plane decoration are shown.
#[derive(Clone, Copy, Debug)]
pub struct Decor {
    pub grid: bool,
    pub axes: bool,
}

/// A change of view requested outside the drawing code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRequest {
    /// The view a document opens with.
    Initial,
    Standard,
    /// Fit all objects.
    Fit,
}

/// The rules where the window deliberately differs from the reference app
/// (the owner's choice), one named switch each. `OURS` is what the window
/// runs; `REFERENCE` follows the reference app, for tests that compare
/// with its observed behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerRules {
    /// `hit::SIDE_TAKES_THE_POLYGON`.
    pub side_takes_the_polygon: bool,
    /// `interact::REFUSED_DRAG_NOTICE`.
    pub refused_drag_notice: bool,
    /// `interact::EVERY_DRAG_IS_A_STEP`.
    pub every_drag_is_a_step: bool,
}

impl OwnerRules {
    pub const OURS: OwnerRules = OwnerRules {
        side_takes_the_polygon: crate::hit::SIDE_TAKES_THE_POLYGON,
        refused_drag_notice: crate::interact::REFUSED_DRAG_NOTICE,
        every_drag_is_a_step: crate::interact::EVERY_DRAG_IS_A_STEP,
    };
    pub const REFERENCE: OwnerRules = OwnerRules {
        side_takes_the_polygon: false,
        refused_drag_notice: false,
        every_drag_is_a_step: false,
    };
}

pub struct App {
    pub wb: Workbench,
    /// The owner's rules where the window differs from the reference app.
    pub rules: OwnerRules,
    /// `None` until the first frame, when the size of the area is known.
    pub camera: Option<Camera>,
    pub view_request: Option<ViewRequest>,
    pub drag: Option<Drag>,
    /// The pointer automaton of the graphics view.
    pub gesture: Gesture,
    /// The sizes of the input profile chosen at start.
    pub metrics: Metrics,
    pub tool: ToolState,
    /// The tool each toolbar group shows (the last one chosen in it).
    pub group_tools: Vec<Tool>,
    pub input: String,
    pub input_error: Option<String>,
    pub tool_error: Option<String>,
    pub load_error: Option<String>,
    pub decor: Decor,
    /// The object under the pointer in the drawing area.
    pub hovered: Option<Id>,
    pub focus_input: bool,
    /// The input row's cursor (in characters), kept while the on-screen
    /// keyboard takes the focus for a moment.
    pub input_cursor: usize,
    /// Put the input row's cursor here when it next has the focus.
    pub cursor_request: Option<usize>,
    pub keyboard: Keyboard,
    /// The graphics style bar is open.
    pub style_bar: bool,
    /// The text of the number dialog.
    pub number_text: String,
    /// Where the regions were in the last frame.
    pub regions: Regions,
    /// The widgets over the graphics view in the last frame.
    pub overlays: Overlays,
    /// `Workbench::rebuilds` as the drawing last saw it.
    pub seen_rebuilds: u64,
    /// The drawing area and its transform as last painted, and as they were
    /// when the primary button went down: a press is hit-tested against
    /// what the user saw, even if the area changes in that frame.
    pub view: Option<(Rect, ViewTransform)>,
    pub press_view: Option<(Rect, ViewTransform)>,
    /// What the construction tool's press did, until its release.
    pub tool_press: Option<crate::toolpress::ToolPress>,
    /// The delete tool's drag is erasing (spec E11).
    pub erasing: bool,
    /// A text field had the keyboard in the previous frame.
    pub was_typing: bool,
    /// Where the labels were painted in the last frame, in the area's
    /// coordinates and painting order (the buffer is reused).
    pub label_rects: RefCell<Vec<(Id, Rect)>>,
}

impl App {
    /// How new and dragged free points are captured to the grid at `scale`
    /// pixels per unit (on while the grid or the axes are shown).
    pub fn snap(&self, scale: f64) -> Snap {
        Snap::new(&self.metrics, scale, self.decor.grid || self.decor.axes)
    }

    pub fn new(wb: Workbench, load_error: Option<String>) -> Self {
        App {
            wb,
            rules: OwnerRules::OURS,
            camera: None,
            view_request: Some(ViewRequest::Initial),
            drag: None,
            gesture: Gesture::new(&Metrics::desktop()),
            metrics: Metrics::desktop(),
            tool: ToolState::new(Tool::Move),
            group_tools: GROUPS.iter().map(|g| g[0]).collect(),
            input: String::new(),
            input_error: None,
            tool_error: None,
            load_error,
            decor: Decor {
                grid: true,
                axes: true,
            },
            hovered: None,
            focus_input: false,
            input_cursor: 0,
            cursor_request: None,
            keyboard: Keyboard::default(),
            style_bar: false,
            number_text: String::new(),
            regions: Regions::new(Rect::ZERO, false, &Metrics::desktop()),
            overlays: Overlays::new(),
            seen_rebuilds: 0,
            view: None,
            press_view: None,
            tool_press: None,
            erasing: false,
            was_typing: false,
            label_rects: RefCell::new(Vec::new()),
        }
    }

    /// The object whose label was painted at `(sx, sy)` in the area in the
    /// last frame, the first in painting order
    /// (`EuclidianView.getLabelHit`: `labelRectangle.contains`, no
    /// threshold).
    pub fn label_at(&self, (sx, sy): (f64, f64)) -> Option<Id> {
        let p = crate::geom::Point::new(sx as f32, sy as f32);
        self.label_rects
            .borrow()
            .iter()
            .find(|(id, r)| id.index() < self.wb.construction().len() && r.contains(p))
            .map(|&(id, _)| id)
    }

    /// Abandons the drag in progress: the point or the object goes back, or
    /// the label (whose drag is no undo step) gets its old offset back.
    pub fn cancel_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            if let Hold::Label { old, .. } = drag.hold() {
                self.wb.set_label_offset(drag.id(), old);
            }
            self.wb.cancel_gesture();
        }
    }

    /// Chooses the input profile (for example the touch one).
    pub fn set_metrics(&mut self, metrics: Metrics) {
        self.metrics = metrics;
        self.gesture = Gesture::new(&metrics);
    }

    /// Makes `tool` the active tool and the one its group shows. A tool use
    /// in progress ends unfinished: its pending points go
    /// (`EuclidianController.initNewMode`).
    pub fn select_tool(&mut self, tool: Tool) {
        self.reset_tool();
        self.tool.set_tool(tool);
        self.group_tools[tool.group()] = tool;
        self.tool_error = None;
    }

    /// Ends the use of the active tool unfinished: its picks are dropped and
    /// the points it placed are taken away, with no undo step (Escape, a
    /// tool switch, a cancelled number dialog, a failed final step;
    /// `Kernel.restoreStateForInitNewMode`).
    pub fn reset_tool(&mut self) {
        self.tool.reset();
        self.tool_press = None;
        self.wb.cancel_tool_use();
        // An eraser stroke stops here (the reference's drag goes to the new
        // mode's handler); what it deleted so far stays as one undo step
        // (`storeUndoAfterDrag` runs at the release whatever the mode).
        if self.erasing {
            self.erasing = false;
            self.wb.end_gesture();
        }
    }

    /// Undo from the toolbar or the keyboard. In the middle of a tool use it
    /// only takes the use back: the pending points go and the picks are
    /// dropped, and no step is undone (`Kernel.undo`,
    /// `restoreStateForModeStarting`).
    pub fn undo(&mut self) -> bool {
        if self.tool_in_use() {
            self.reset_tool();
            return true;
        }
        self.wb.undo()
    }

    /// Redo; a tool use in progress ends unfinished first.
    pub fn redo(&mut self) -> bool {
        if !self.wb.can_redo() {
            return false;
        }
        self.reset_tool();
        self.wb.redo()
    }

    /// Whether undo has something to do: a step, or a tool use to take back.
    pub fn can_undo(&self) -> bool {
        self.tool_in_use() || self.wb.can_undo()
    }

    /// The active tool holds picks or waits for its number.
    fn tool_in_use(&self) -> bool {
        !self.tool.picks().is_empty() || self.tool.asking().is_some()
    }

    /// Leaves the active tool for the move tool, clears the selection and
    /// closes the style bar: Escape, or the close button of the tool hint.
    pub fn leave_tool(&mut self) {
        self.select_tool(Tool::Move);
        self.wb.selected = None;
        self.style_bar = false;
    }

    /// Runs the input row's text.
    pub fn submit_input(&mut self) {
        match self.wb.run_line(&self.input) {
            Ok(()) => {
                // A tool use that does not finish ends (its points went
                // with it, `Workbench::run_lines`); a failed line changes
                // nothing, the use goes on.
                self.tool.reset();
                self.tool_press = None;
                self.input.clear();
                self.input_cursor = 0;
                self.input_error = None;
                self.wb.selected = None;
            }
            Err(e) => self.input_error = Some(errors::polish(&e)),
        }
        self.focus_input = true;
    }

    /// Runs what a tool's pick asks for. The lines of a construction tool
    /// run inside its tool use; once the tool holds no picks any more (it
    /// made its object) the use is one undo step
    /// (`EuclidianController.processMode` stores the undo info when the
    /// kernel changed). A failed step ends the use unfinished.
    pub fn apply_outcome(&mut self, outcome: Outcome) {
        self.apply_pick(outcome);
        if !self.tool_in_use() {
            self.wb.end_tool_use();
        }
    }

    fn apply_pick(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Place { line, at, then } => match self.wb.run_tool_lines(&[line]) {
                Ok(()) => {
                    self.tool_error = None;
                    let new = self.wb.construction().ids().last();
                    if let (Some(id), Some((x, y))) = (new, at) {
                        self.wb.move_pending_point(id, x, y);
                    }
                    self.wb.selected = new;
                    self.apply_pick(*then);
                }
                Err(e) => {
                    self.tool_error = Some(errors::polish(&e));
                    self.reset_tool();
                }
            },
            Outcome::Run(lines) => match self.wb.run_tool_lines(&lines) {
                Ok(()) => {
                    self.tool_error = None;
                    self.wb.selected = self.wb.construction().ids().last();
                }
                Err(e) => {
                    self.tool_error = Some(errors::polish(&e));
                    self.reset_tool();
                }
            },
            Outcome::AskNumber(request) => {
                self.number_text = crate::describe::n2(request.default);
                self.tool_error = None;
            }
            Outcome::Delete(label) => {
                if let Some(id) = self.wb.construction().id(&label) {
                    self.wb.delete(id);
                }
                self.tool_error = None;
            }
            Outcome::ToggleVisible(label) => {
                if let Some(id) = self.wb.construction().id(&label) {
                    self.wb.toggle_visible(id);
                }
            }
            Outcome::Select(label) => {
                self.wb.selected = self.wb.construction().id(&label);
                self.tool_error = None;
            }
            Outcome::Pending => self.tool_error = None,
            Outcome::Ignored => {}
        }
    }
}

/// A number typed by a person: a decimal comma is accepted, a degree sign
/// is ignored.
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('°').trim().replace(',', ".");
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::parse_number;

    #[test]
    fn typed_numbers_accept_a_comma_and_a_degree_sign() {
        assert_eq!(parse_number(" 2,5 "), Some(2.5));
        assert_eq!(parse_number("45°"), Some(45.0));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
