//! The state of the window: the document, the camera, the active tool, the
//! input row and the open panels, and what a finished tool or the input row
//! does to them. The window (`geo-app`) draws it and feeds it input.

use crate::errors;
use crate::geom::Rect;
use crate::gesture::Gesture;
use crate::interact::{Drag, ViewTransform};
use crate::keyboard::Keyboard;
use crate::layout::Regions;
use crate::metrics::Metrics;
use crate::tools::{Outcome, Tool, ToolState, GROUPS};
use crate::view::Camera;
use crate::workbench::Workbench;
use geo_kernel::kernel::Id;

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

pub struct App {
    pub wb: Workbench,
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
    /// `Workbench::rebuilds` as the drawing last saw it.
    pub seen_rebuilds: u64,
    /// The drawing area and its transform as last painted, and as they were
    /// when the primary button went down: a press is hit-tested against
    /// what the user saw, even if the area changes in that frame.
    pub view: Option<(Rect, ViewTransform)>,
    pub press_view: Option<(Rect, ViewTransform)>,
    /// A text field had the keyboard in the previous frame.
    pub was_typing: bool,
}

impl App {
    pub fn new(wb: Workbench, load_error: Option<String>) -> Self {
        App {
            wb,
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
            seen_rebuilds: 0,
            view: None,
            press_view: None,
            was_typing: false,
        }
    }

    /// Chooses the input profile (for example the touch one).
    pub fn set_metrics(&mut self, metrics: Metrics) {
        self.metrics = metrics;
        self.gesture = Gesture::new(&metrics);
    }

    /// Makes `tool` the active tool and the one its group shows.
    pub fn select_tool(&mut self, tool: Tool) {
        self.tool.set_tool(tool);
        self.group_tools[tool.group()] = tool;
        self.tool_error = None;
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
                self.input.clear();
                self.input_cursor = 0;
                self.input_error = None;
                self.wb.selected = None;
            }
            Err(e) => self.input_error = Some(errors::polish(&e)),
        }
        self.focus_input = true;
    }

    /// Runs what a finished tool asks for.
    pub fn apply_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Place { line, then } => match self.wb.run_lines(&[line]) {
                Ok(()) => {
                    self.tool_error = None;
                    self.wb.selected = self.wb.construction().ids().last();
                    self.apply_outcome(*then);
                }
                Err(e) => {
                    self.tool_error = Some(errors::polish(&e));
                    self.tool.reset();
                }
            },
            Outcome::Run(lines) => match self.wb.run_lines(&lines) {
                Ok(()) => {
                    self.tool_error = None;
                    self.wb.selected = self.wb.construction().ids().last();
                }
                Err(e) => {
                    self.tool_error = Some(errors::polish(&e));
                    self.tool.reset();
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
