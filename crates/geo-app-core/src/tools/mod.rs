// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/gui/toolbar/ToolBar.java (the tool groups)
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The construction tools as a state machine, with no UI types.
//!
//! A click on the drawing area is reported as [`ToolState::click`]. The state
//! remembers the objects picked so far; after the last pick it returns the
//! script lines to run, exactly as if the user had typed them. A click on
//! empty space places a free point first, as a change of its own
//! ([`Outcome::Place`]). Tools that need
//! a number (a radius, an angle, a factor) first return
//! [`Outcome::AskNumber`]; the answer goes to [`ToolState::finish_number`].

mod recipe;

use crate::describe::fixed;
use crate::labels::next_label;
use geo_kernel::kernel::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Move,
    Point,
    Intersect,
    Midpoint,
    Line,
    Segment,
    Ray,
    Vector,
    Perpendicular,
    Parallel,
    Bisector,
    AngleBisector,
    Tangents,
    Polar,
    Polygon,
    RegularPolygon,
    Circle,
    CircleRadius,
    Circle3,
    Angle,
    Distance,
    ReflectLine,
    ReflectPoint,
    ReflectCircle,
    Rotate,
    Translate,
    Dilate,
    MoveView,
    ZoomIn,
    ZoomOut,
    ShowHide,
    Delete,
}

/// The tool groups of the toolbar, in the order of the reference toolbar.
/// Groups whose tools the kernel cannot serve yet (conics, sliders and
/// texts) are left out.
pub const GROUPS: &[&[Tool]] = &[
    &[Tool::Move],
    &[Tool::Point, Tool::Intersect, Tool::Midpoint],
    &[Tool::Line, Tool::Segment, Tool::Ray, Tool::Vector],
    &[
        Tool::Perpendicular,
        Tool::Parallel,
        Tool::Bisector,
        Tool::AngleBisector,
        Tool::Tangents,
        Tool::Polar,
    ],
    &[Tool::Polygon, Tool::RegularPolygon],
    &[Tool::Circle, Tool::CircleRadius, Tool::Circle3],
    &[Tool::Angle, Tool::Distance],
    &[
        Tool::ReflectLine,
        Tool::ReflectPoint,
        Tool::ReflectCircle,
        Tool::Rotate,
        Tool::Translate,
        Tool::Dilate,
    ],
    &[
        Tool::MoveView,
        Tool::ZoomIn,
        Tool::ZoomOut,
        Tool::ShowHide,
        Tool::Delete,
    ],
];

impl Tool {
    /// Every tool, group by group.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn all() -> impl Iterator<Item = Tool> {
        GROUPS.iter().flat_map(|g| g.iter().copied())
    }

    /// The index of the group the tool belongs to.
    pub fn group(self) -> usize {
        GROUPS
            .iter()
            .position(|g| g.contains(&self))
            .expect("every tool is in a group")
    }

    /// The tool's name in the drop-down list and the tooltip.
    pub fn caption(self) -> &'static str {
        crate::text::tool_caption(self)
    }

    /// Tools that act on the view or on whole objects instead of building
    /// something from picks.
    pub fn is_direct(self) -> bool {
        matches!(
            self,
            Tool::Move
                | Tool::MoveView
                | Tool::ZoomIn
                | Tool::ZoomOut
                | Tool::ShowHide
                | Tool::Delete
        )
    }
}

/// One click's contribution to a tool: an object of the construction (a
/// point placed by the click is added before it is picked).
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Object { label: String, kind: Kind },
}

impl Pick {
    fn kind(&self) -> Kind {
        let Pick::Object { kind, .. } = self;
        *kind
    }

    pub fn label(&self) -> &str {
        let Pick::Object { label, .. } = self;
        label
    }
}

/// The number a tool asks for before it can finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberRequest {
    /// The dialog's title.
    pub title: &'static str,
    /// What the field holds.
    pub prompt: &'static str,
    pub default: f64,
    /// The value is an angle typed in degrees.
    pub degrees: bool,
}

/// What a click did.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The pick was taken; more are needed.
    Pending,
    /// Run these script lines as one change.
    Run(Vec<String>),
    /// Add this free point as its own change, then go on with `then`.
    Place { line: String, then: Box<Outcome> },
    /// The picks are complete; ask for a number, then call
    /// [`ToolState::finish_number`].
    AskNumber(NumberRequest),
    /// Delete the object (and its dependents).
    Delete(String),
    /// Show or hide the object.
    ToggleVisible(String),
    /// Select this existing object (the point tool clicked on a point).
    Select(String),
    /// Nothing happened: the click was not something the tool can use.
    Ignored,
}

#[derive(Clone, Debug)]
pub struct ToolState {
    tool: Tool,
    picks: Vec<Pick>,
    /// Set while the dialog for the number is open.
    asking: Option<NumberRequest>,
}

impl ToolState {
    pub fn new(tool: Tool) -> Self {
        ToolState {
            tool,
            picks: Vec::new(),
            asking: None,
        }
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switches tool and drops any half-finished picks.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.reset();
    }

    /// The picks made so far (for drawing a preview).
    pub fn picks(&self) -> &[Pick] {
        &self.picks
    }

    /// The number the tool waits for, if any.
    pub fn asking(&self) -> Option<NumberRequest> {
        self.asking
    }

    fn picked(&self, label: &str) -> bool {
        self.picks
            .iter()
            .any(|p| matches!(p, Pick::Object { label: l, .. } if l == label))
    }

    /// Whether picking `label` now closes the polygon being drawn.
    fn closes_on(&self, label: &str) -> bool {
        self.tool == Tool::Polygon
            && self.picks.len() >= 3
            && matches!(self.picks.first(), Some(Pick::Object { label: l, .. }) if l == label)
    }

    /// The polygon through the picks so far, closed by the last click.
    fn close_polygon(&mut self, is_taken: &dyn Fn(&str) -> bool) -> Outcome {
        self.build(None, is_taken)
    }

    fn kinds(&self) -> Vec<Kind> {
        self.picks.iter().map(Pick::kind).collect()
    }

    /// Whether the next click may pick an existing object of this kind
    /// (`label` is checked so that the same object is not taken twice).
    pub fn accepts_object(&self, label: &str, kind: Kind) -> bool {
        match self.tool {
            Tool::Move | Tool::MoveView | Tool::ZoomIn | Tool::ZoomOut => false,
            Tool::Delete | Tool::ShowHide => true,
            _ if self.asking.is_some() => false,
            // The first vertex again closes the polygon.
            Tool::Polygon if self.closes_on(label) => true,
            _ if self.picked(label) => false,
            tool => recipe::accepts(tool, &self.kinds(), Some(kind)),
        }
    }

    /// Whether the next click may create a new point on empty space.
    pub fn accepts_new_point(&self) -> bool {
        !self.tool.is_direct()
            && self.asking.is_none()
            && recipe::accepts(self.tool, &self.kinds(), None)
    }

    /// A one-line Polish hint for what the next click should be.
    pub fn hint(&self) -> &'static str {
        crate::text::hint(recipe::hint(self.tool, &self.kinds()))
    }

    /// Forgets the picks (after an error, on Escape, or a cancelled dialog).
    pub fn reset(&mut self) {
        self.picks.clear();
        self.asking = None;
    }

    /// Handles a click. `hit` is the existing object under the pointer that
    /// the tool may use (see [`Self::accepts_object`]), `world` the pointer
    /// position, `decimals` how many decimals a new point's coordinates get
    /// (see `describe::click_decimals`), `is_taken` tells whether a label is
    /// in use.
    ///
    /// A click that lands on an existing point never creates a new point: it
    /// picks the point if the tool can use it, and is ignored otherwise.
    pub fn click(
        &mut self,
        hit: Option<(&str, Kind)>,
        world: (f64, f64),
        decimals: usize,
        is_taken: &dyn Fn(&str) -> bool,
    ) -> Outcome {
        match (self.tool, hit) {
            (Tool::Move | Tool::MoveView | Tool::ZoomIn | Tool::ZoomOut, _) => {
                return Outcome::Ignored
            }
            (Tool::Delete, Some((label, _))) => return Outcome::Delete(label.to_owned()),
            (Tool::ShowHide, Some((label, _))) => return Outcome::ToggleVisible(label.to_owned()),
            (Tool::Delete | Tool::ShowHide, None) => return Outcome::Ignored,
            (Tool::Point, Some((label, Kind::Point))) => return Outcome::Select(label.to_owned()),
            _ if self.asking.is_some() => return Outcome::Ignored,
            _ => {}
        }
        // A point placed on empty space (or on an object the tool cannot
        // take) is added to the construction at once, as its own step, as in
        // the reference: cancelling the tool later keeps it.
        let (pick, place) = match hit {
            Some((label, kind)) if self.accepts_object(label, kind) => (
                Pick::Object {
                    label: label.to_owned(),
                    kind,
                },
                None,
            ),
            _ if self.accepts_new_point() && !matches!(hit, Some((_, Kind::Point))) => {
                let label = next_label(Kind::Point, |l| is_taken(l) || self.picked(l));
                let line = format!(
                    "{label} = ({}, {})",
                    fixed(world.0, decimals),
                    fixed(world.1, decimals)
                );
                (
                    Pick::Object {
                        label,
                        kind: Kind::Point,
                    },
                    Some(line),
                )
            }
            _ => return Outcome::Ignored,
        };
        let closes = matches!(&pick, Pick::Object { label, .. } if self.closes_on(label));
        let then = if closes {
            self.close_polygon(is_taken)
        } else {
            self.picks.push(pick);
            match recipe::finish(self.tool, &self.kinds()) {
                None => Outcome::Pending,
                Some(recipe::Finish::Ask(request)) => {
                    self.asking = Some(request);
                    Outcome::AskNumber(request)
                }
                Some(recipe::Finish::Now) => self.build(None, is_taken),
            }
        };
        match place {
            Some(line) => Outcome::Place {
                line,
                then: Box::new(then),
            },
            None => then,
        }
    }

    /// The final lines of the tool from its picks, which are then dropped.
    /// The picked labels count as taken: a point placed by this very click
    /// is not in the construction yet.
    fn build(&mut self, number: Option<f64>, is_taken: &dyn Fn(&str) -> bool) -> Outcome {
        let picks = std::mem::take(&mut self.picks);
        let taken = |l: &str| is_taken(l) || picks.iter().any(|p| p.label() == l);
        let lines = recipe::build(self.tool, &picks, number, &taken);
        if lines.is_empty() {
            // The point tool: the placed point was all it makes.
            Outcome::Pending
        } else {
            Outcome::Run(lines)
        }
    }

    /// Completes a tool that asked for a number. `value` is in the unit the
    /// request named (degrees for angles). Returns [`Outcome::Ignored`] when
    /// the tool was not waiting.
    pub fn finish_number(&mut self, value: f64, is_taken: &dyn Fn(&str) -> bool) -> Outcome {
        let Some(request) = self.asking.take() else {
            return Outcome::Ignored;
        };
        let value = if request.degrees {
            value.to_radians()
        } else {
            value
        };
        self.build(Some(value), is_taken)
    }
}

#[cfg(test)]
mod tests;
