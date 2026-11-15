//! The construction tools as a state machine, with no UI types.
//!
//! A click on the drawing area is reported as [`ToolState::click`]. The state
//! remembers the objects picked so far; after the last pick it returns the
//! script lines to run, exactly as if the user had typed them. Tools that need
//! a number (a radius, an angle, a factor) first return
//! [`Outcome::AskNumber`]; the answer goes to [`ToolState::finish_number`].

mod recipe;

use crate::describe::fixed;
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
        match self {
            Tool::Move => "Przesuń",
            Tool::Point => "Punkt",
            Tool::Intersect => "Przecięcie",
            Tool::Midpoint => "Środek",
            Tool::Line => "Prosta",
            Tool::Segment => "Odcinek",
            Tool::Ray => "Półprosta",
            Tool::Vector => "Wektor",
            Tool::Perpendicular => "Prostopadła",
            Tool::Parallel => "Równoległa",
            Tool::Bisector => "Symetralna",
            Tool::AngleBisector => "Dwusieczna",
            Tool::Tangents => "Styczne",
            Tool::Polar => "Biegunowa",
            Tool::Circle => "Okrąg: środek i punkt",
            Tool::CircleRadius => "Okrąg: środek i promień",
            Tool::Circle3 => "Okrąg przez trzy punkty",
            Tool::Polygon => "Wielokąt",
            Tool::RegularPolygon => "Wielokąt foremny",
            Tool::Angle => "Kąt",
            Tool::Distance => "Odległość lub długość",
            Tool::ReflectLine => "Symetria osiowa",
            Tool::ReflectPoint => "Symetria środkowa",
            Tool::ReflectCircle => "Inwersja",
            Tool::Rotate => "Obrót",
            Tool::Translate => "Przesunięcie o wektor",
            Tool::Dilate => "Jednokładność",
            Tool::MoveView => "Przesuń widok",
            Tool::ZoomIn => "Powiększ",
            Tool::ZoomOut => "Pomniejsz",
            Tool::ShowHide => "Pokaż lub ukryj",
            Tool::Delete => "Usuń",
        }
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

/// One click's contribution to a tool.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Object {
        label: String,
        kind: Kind,
    },
    /// A free point to be created at these world coordinates.
    New {
        x: f64,
        y: f64,
    },
}

impl Pick {
    fn kind(&self) -> Kind {
        match self {
            Pick::Object { kind, .. } => *kind,
            Pick::New { .. } => Kind::Point,
        }
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

fn rounded(v: f64, decimals: usize) -> f64 {
    fixed(v, decimals).parse().unwrap_or(v)
}

#[derive(Clone, Debug)]
pub struct ToolState {
    tool: Tool,
    picks: Vec<Pick>,
    /// Set while the dialog for the number is open.
    asking: Option<NumberRequest>,
    /// How near (in world units) a click must be to a new point not made
    /// yet to count as a click on it; the canvas sets it from the zoom.
    tolerance: Option<f64>,
}

impl ToolState {
    pub fn new(tool: Tool) -> Self {
        ToolState {
            tool,
            picks: Vec::new(),
            asking: None,
            tolerance: None,
        }
    }

    /// Sets how near a click must be to a pending new point to land on it.
    pub fn set_tolerance(&mut self, world: f64) {
        self.tolerance = Some(world).filter(|t| t.is_finite() && *t > 0.0);
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
    fn close_polygon(&mut self, decimals: usize, is_taken: &dyn Fn(&str) -> bool) -> Outcome {
        let lines = recipe::build(self.tool, &self.picks, None, decimals, is_taken);
        self.picks.clear();
        Outcome::Run(lines)
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
        recipe::hint(self.tool, &self.kinds())
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
        let pick = match hit {
            Some((label, kind)) if self.accepts_object(label, kind) => Pick::Object {
                label: label.to_owned(),
                kind,
            },
            // The tool wants a point but the click landed on something else
            // (or on an object it cannot take): place a new point there.
            _ if self.accepts_new_point() && !matches!(hit, Some((_, Kind::Point))) => Pick::New {
                x: rounded(world.0, decimals),
                y: rounded(world.1, decimals),
            },
            _ => return Outcome::Ignored,
        };
        if matches!(&pick, Pick::Object { label, .. } if self.closes_on(label)) {
            return self.close_polygon(decimals, is_taken);
        }
        if let Pick::New { x, y } = pick {
            // A second click on a new point that is not made yet (a double
            // click) must not make a second point on top of it. Without a
            // tolerance from the canvas, ten steps of `decimals` stand in
            // (five pixels at the standard zoom).
            let near = self
                .tolerance
                .unwrap_or(10.0 * 10f64.powi(-(decimals as i32)));
            let twice = self.picks.iter().any(|p| match p {
                Pick::New { x: px, y: py } => (x - px).hypot(y - py) < near,
                Pick::Object { .. } => false,
            });
            let on_first = matches!(self.picks.first(), Some(Pick::New { x: px, y: py })
                if (x - px).hypot(y - py) < near);
            if self.tool == Tool::Polygon && self.picks.len() >= 3 && on_first {
                return self.close_polygon(decimals, is_taken);
            }
            if twice {
                return Outcome::Ignored;
            }
        }
        self.picks.push(pick);
        match recipe::finish(self.tool, &self.kinds()) {
            None => Outcome::Pending,
            Some(recipe::Finish::Ask(request)) => {
                self.asking = Some(request);
                Outcome::AskNumber(request)
            }
            Some(recipe::Finish::Now) => {
                let lines = recipe::build(self.tool, &self.picks, None, decimals, is_taken);
                self.picks.clear();
                Outcome::Run(lines)
            }
        }
    }

    /// Completes a tool that asked for a number. `value` is in the unit the
    /// request named (degrees for angles). Returns [`Outcome::Ignored`] when
    /// the tool was not waiting.
    pub fn finish_number(
        &mut self,
        value: f64,
        decimals: usize,
        is_taken: &dyn Fn(&str) -> bool,
    ) -> Outcome {
        let Some(request) = self.asking.take() else {
            return Outcome::Ignored;
        };
        let value = if request.degrees {
            value.to_radians()
        } else {
            value
        };
        let lines = recipe::build(self.tool, &self.picks, Some(value), decimals, is_taken);
        self.picks.clear();
        Outcome::Run(lines)
    }
}

#[cfg(test)]
mod tests;
