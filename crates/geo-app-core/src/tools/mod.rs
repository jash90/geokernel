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
//! empty space places a new point first ([`Outcome::Place`]), which the
//! window keeps as a pending point of the tool use until the use completes
//! (one undo step) or is cancelled (the point goes). Tools that need
//! a number (a radius, an angle, a factor) first return
//! [`Outcome::AskNumber`]; the answer goes to [`ToolState::finish_number`].

pub mod freehand;
mod recipe;

pub(crate) use recipe::intersection_pair;

use crate::describe::exact;
use crate::labels::next_label;
use geo_kernel::kernel::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Relation,
    RigidPolygon,
    SegmentFixed,
    AngleFixed,
    Extremum,
    Roots,
    PointOnObject,
    AttachDetach,
    Move,
    Point,
    Intersect,
    Midpoint,
    Line,
    Segment,
    Ray,
    Vector,
    VectorFromPoint,
    Perpendicular,
    Parallel,
    Bisector,
    AngleBisector,
    Tangents,
    Polar,
    Polygon,
    RegularPolygon,
    Polyline,
    VectorPolygon,
    Circle,
    CircleRadius,
    Circle3,
    Semicircle,
    CircularArc,
    CircumcircularArc,
    CircularSector,
    CircumcircularSector,
    Compasses,
    Ellipse,
    Hyperbola,
    Parabola,
    Conic5,
    Angle,
    Distance,
    Area,
    Slope,
    Locus,
    CreateList,
    ReflectLine,
    ReflectPoint,
    ReflectCircle,
    Rotate,
    Translate,
    Dilate,
    Slider,
    Checkbox,
    InputBox,
    Text,
    PenStroke,
    FreehandShape,
    FreehandFunction,
    FunctionInspector,
    Image,
    MoveRotate,
    ShowHideLabel,
    CopyVisualStyle,
    MoveView,
    ZoomIn,
    ZoomOut,
    ShowHide,
    Delete,
}

/// The tool groups of the toolbar, in the order of the reference toolbar.
pub const GROUPS: &[&[Tool]] = &[
    &[
        Tool::Move,
        Tool::MoveRotate,
        Tool::FreehandShape,
        Tool::FreehandFunction,
    ],
    &[
        Tool::Point,
        Tool::PointOnObject,
        Tool::Intersect,
        Tool::Midpoint,
        Tool::AttachDetach,
    ],
    &[
        Tool::Line,
        Tool::Segment,
        Tool::SegmentFixed,
        Tool::Ray,
        Tool::Vector,
        Tool::VectorFromPoint,
    ],
    &[
        Tool::Perpendicular,
        Tool::Parallel,
        Tool::Bisector,
        Tool::AngleBisector,
        Tool::Tangents,
        Tool::Polar,
    ],
    &[
        Tool::Polygon,
        Tool::RigidPolygon,
        Tool::RegularPolygon,
        Tool::Polyline,
        Tool::VectorPolygon,
    ],
    &[
        Tool::Circle,
        Tool::CircleRadius,
        Tool::Compasses,
        Tool::Circle3,
        Tool::Semicircle,
        Tool::CircularArc,
        Tool::CircumcircularArc,
        Tool::CircularSector,
        Tool::CircumcircularSector,
    ],
    &[Tool::Ellipse, Tool::Hyperbola, Tool::Parabola, Tool::Conic5],
    &[
        Tool::Angle,
        Tool::AngleFixed,
        Tool::Relation,
        Tool::Extremum,
        Tool::Roots,
        Tool::Distance,
        Tool::Area,
        Tool::Slope,
        Tool::CreateList,
        Tool::Locus,
        Tool::FunctionInspector,
    ],
    &[
        Tool::ReflectLine,
        Tool::ReflectPoint,
        Tool::ReflectCircle,
        Tool::Rotate,
        Tool::Translate,
        Tool::Dilate,
    ],
    &[
        Tool::Slider,
        Tool::Checkbox,
        Tool::InputBox,
        Tool::Text,
        Tool::Image,
        Tool::PenStroke,
    ],
    &[
        Tool::MoveView,
        Tool::ZoomIn,
        Tool::ZoomOut,
        Tool::ShowHide,
        Tool::ShowHideLabel,
        Tool::CopyVisualStyle,
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

    pub fn draws_stroke(self) -> bool {
        matches!(
            self,
            Self::PenStroke | Self::FreehandShape | Self::FreehandFunction
        )
    }

    /// Tools that act on the view or on whole objects instead of building
    /// something from picks.
    pub fn is_direct(self) -> bool {
        matches!(
            self,
            Tool::Move
                | Tool::Slider
                | Tool::Checkbox
                | Tool::InputBox
                | Tool::CreateList
                | Tool::Text
                | Tool::PenStroke
                | Tool::MoveView
                | Tool::ZoomIn
                | Tool::ZoomOut
                | Tool::ShowHide
                | Tool::Delete
                | Tool::MoveRotate
                | Tool::FreehandShape
                | Tool::FreehandFunction
                | Tool::FunctionInspector
                | Tool::Image
                | Tool::ShowHideLabel
                | Tool::CopyVisualStyle
                | Tool::AttachDetach
                | Tool::Relation
        )
    }
}

pub(crate) fn locus_pick_is_valid(
    c: &geo_kernel::Construction,
    first: Option<&Pick>,
    id: geo_kernel::Id,
) -> bool {
    match first {
        Some(first) => c
            .id(first.label())
            .is_some_and(|first| c.can_trace_locus(first, id) || c.can_trace_locus(id, first)),
        None => (c.kind(id) == Kind::Point && !c.def(id).is_free()) || c.has_slider_bounds(id),
    }
}

pub(crate) const MAX_RIGID_VERTICES: usize = 1000;

pub const MAX_PLACED_TEXT_CHARS: usize = 4096;

/// One click's contribution to a tool: an object of the construction (a
/// point placed by the click is added before it is picked).
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Object { label: String, kind: Kind },
}

impl Pick {
    pub fn kind(&self) -> Kind {
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
    /// The picks changed without finishing the tool use.
    SelectionChanged,
    /// Run these script lines as one change.
    Run(Vec<String>),
    /// Compare objects without changing the construction.
    Relation([String; 2]),
    /// Copy an existing polygon using the current view's offset.
    RigidCopy(String),
    /// Build a vector polygon, positioning each vertex relative to the first.
    VectorPolygon { picks: Vec<Pick>, copy_offset: f64 },
    /// Add this new point (a pending point of the tool use), put it at
    /// `at` when given (a point on a path goes where the pointer was), then
    /// go on with `then`.
    Place {
        line: String,
        at: Option<(f64, f64)>,
        then: Box<Outcome>,
    },
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

/// What a new point made by a click is (`EuclidianController.getNewPoint`,
/// chosen by the window from what lies under the pointer).
#[derive(Clone, Debug, PartialEq)]
pub enum NewPoint {
    /// A free point at `(x, y)`, already captured to the grid.
    Free(f64, f64),
    /// `Point(path)`, put at `(x, y)` (the kernel projects it onto the path).
    OnPath { path: String, x: f64, y: f64 },
    /// `PointIn(region)` at the pointer.
    InRegion { region: String, x: f64, y: f64 },
    /// `Intersect(a, b)`, or `Intersect(a, b, n)` for one of two points.
    Intersection {
        a: String,
        b: String,
        index: Option<u8>,
    },
}

/// Whether a construction tool makes a new point on the objects under the
/// pointer (on a path, or at the crossing of two) rather than a free one:
/// the reference's `createNewPointForModePoint` and
/// `createNewPointForModeOther` allow paths and intersections; the angle and
/// midpoint tools (`createNewPoint(hits, false, false, ...)`) and the
/// perpendicular and parallel tools before a line is picked do not.
pub fn new_points_on_objects(tool: Tool, picks: &[Pick]) -> bool {
    match tool {
        Tool::Point
        | Tool::PointOnObject
        | Tool::SegmentFixed
        | Tool::Line
        | Tool::Segment
        | Tool::Ray
        | Tool::Vector
        | Tool::Circle
        | Tool::CircleRadius
        | Tool::Circle3
        | Tool::Semicircle
        | Tool::CircularArc
        | Tool::CircumcircularArc
        | Tool::CircularSector
        | Tool::CircumcircularSector
        | Tool::Ellipse
        | Tool::Hyperbola
        | Tool::Conic5
        | Tool::Polyline
        | Tool::Polygon
        | Tool::RegularPolygon => true,
        Tool::Compasses => picks
            .iter()
            .any(|p| matches!(p.kind(), Kind::Circle | Kind::Segment)),
        Tool::Perpendicular | Tool::Parallel => picks
            .iter()
            .any(|p| matches!(p.kind(), Kind::Line | Kind::Segment)),
        _ => false,
    }
}

/// Whether the tool makes its object from a press, a drag and a release as
/// well as from two clicks: the release is the second point
/// (`EuclidianController.isModeCreatingObjectsByDrag`).
pub fn creates_by_drag(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Line
            | Tool::Segment
            | Tool::Ray
            | Tool::Vector
            | Tool::Circle
            | Tool::Semicircle
            | Tool::RegularPolygon
    )
}

#[derive(Clone, Debug)]
pub struct ToolState {
    tool: Tool,
    picks: Vec<Pick>,
    /// Set while the dialog for the number is open.
    asking: Option<NumberRequest>,
    control_at: Option<(f64, f64)>,
    text_anchor: Option<String>,
    attachment_region: bool,
    recent_attachment: Option<(f64, f64)>,
}

impl ToolState {
    pub fn new(tool: Tool) -> Self {
        ToolState {
            tool,
            picks: Vec::new(),
            asking: None,
            control_at: None,
            text_anchor: None,
            attachment_region: false,
            recent_attachment: None,
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

    /// The canvas location the open slider or checkbox dialog will use.
    pub fn control_request(&self) -> Option<(f64, f64)> {
        self.control_at
    }

    pub fn begin_control(&mut self, at: (f64, f64)) {
        if matches!(
            self.tool,
            Tool::Slider | Tool::Checkbox | Tool::InputBox | Tool::Text
        ) && self.control_at.is_none()
        {
            self.control_at = Some(at);
        }
    }

    pub fn begin_text(&mut self, at: (f64, f64), anchor: Option<&str>) {
        if self.tool == Tool::Text && self.control_at.is_none() {
            self.begin_control(at);
            self.text_anchor = anchor.map(str::to_owned);
        }
    }

    pub fn finish_slider(
        &mut self,
        min: f64,
        max: f64,
        step: f64,
        taken: impl Fn(&str) -> bool,
    ) -> Outcome {
        use geo_kernel::kernel::control::SliderStyle;
        use geo_kernel::script::saved::Property;

        if self.tool != Tool::Slider
            || !(min.is_finite() && max.is_finite() && min < max && step.is_finite() && step > 0.0)
        {
            return Outcome::Ignored;
        }
        let Some(position) = self.control_at.take() else {
            return Outcome::Ignored;
        };
        let label = next_label(Kind::Number, taken);
        let value = 1.0_f64.clamp(min, max);
        let definition = format!("{label} = $slider({value}, {min}, {max})");
        let style = Property::SliderStyle {
            label,
            style: SliderStyle {
                position: Some(position),
                step: Some(step),
                ..SliderStyle::default()
            },
        };
        Outcome::Run(vec![definition, style.to_string()])
    }

    pub fn finish_checkbox(&mut self, caption: &str, taken: impl Fn(&str) -> bool) -> Outcome {
        use geo_kernel::kernel::control::{encode_text, CheckboxStyle};
        use geo_kernel::script::saved::Property;

        if self.tool != Tool::Checkbox {
            return Outcome::Ignored;
        }
        let Some((x, y)) = self.control_at.take() else {
            return Outcome::Ignored;
        };
        let label = next_label(Kind::Boolean, taken);
        let definition = format!("{label} = $checkBox(true, {})", encode_text(caption));
        let style = Property::CheckboxStyle {
            label,
            style: CheckboxStyle {
                position: Some([x as i32, y as i32]),
                fixed: false,
            },
        };
        Outcome::Run(vec![definition, style.to_string()])
    }

    pub fn finish_input_box(
        &mut self,
        caption: &str,
        linked: Option<&str>,
        taken: impl Fn(&str) -> bool,
    ) -> Outcome {
        use geo_kernel::kernel::control::{bounded_input_box_text, encode_text, InputBoxStyle};
        use geo_kernel::script::saved::Property;

        if self.tool != Tool::InputBox {
            return Outcome::Ignored;
        }
        let Some((x, y)) = self.control_at.take() else {
            return Outcome::Ignored;
        };
        let label = next_label(Kind::InputBox, taken);
        let caption = encode_text(bounded_input_box_text(caption));
        let linked = linked.unwrap_or_default();
        let definition = format!("{label} = $inputBox({linked}, {caption})");
        let style = Property::InputBoxStyle {
            label,
            style: InputBoxStyle {
                position: [x as i32, y as i32],
                ..InputBoxStyle::default()
            },
        };
        Outcome::Run(vec![definition, style.to_string()])
    }

    pub fn finish_text(&mut self, text: &str, taken: impl Fn(&str) -> bool) -> Outcome {
        use geo_kernel::kernel::text::{TextOverrides, TextPosition};
        use geo_kernel::script::text::{literal, Properties};

        if self.tool != Tool::Text || text.trim().is_empty() {
            return Outcome::Ignored;
        }
        let Some((x, y)) = self.control_at.take() else {
            return Outcome::Ignored;
        };
        let label = next_label(Kind::Text, taken);
        let text = text
            .char_indices()
            .nth(MAX_PLACED_TEXT_CHARS)
            .map_or(text, |(at, _)| &text[..at]);
        let anchor = self.text_anchor.take();
        let properties = Properties {
            label: &label,
            overrides: TextOverrides {
                position: Some(
                    anchor
                        .as_deref()
                        .map_or(TextPosition::World { x, y }, TextPosition::Point),
                ),
                ..TextOverrides::default()
            },
        };
        Outcome::Run(vec![
            format!("{label} = {}", literal(text)),
            properties.to_string(),
        ])
    }

    fn picked(&self, label: &str) -> bool {
        self.picks
            .iter()
            .any(|p| matches!(p, Pick::Object { label: l, .. } if l == label))
    }

    /// Whether picking `label` again completes the chain of vertices.
    fn closes_on(&self, label: &str) -> bool {
        matches!(
            self.tool,
            Tool::Polygon | Tool::Polyline | Tool::VectorPolygon | Tool::RigidPolygon
        ) && self.picks.len() >= 3
            && matches!(self.picks.first(), Some(Pick::Object { label: l, .. }) if l == label)
    }

    fn kinds(&self) -> Vec<Kind> {
        self.picks.iter().map(Pick::kind).collect()
    }

    /// Whether the next click may pick or deselect an existing object.
    pub fn accepts_object(&self, label: &str, kind: Kind) -> bool {
        match self.tool {
            Tool::Move | Tool::MoveView | Tool::ZoomIn | Tool::ZoomOut => false,
            Tool::Delete | Tool::ShowHide | Tool::ShowHideLabel | Tool::CopyVisualStyle => true,
            Tool::FunctionInspector => kind == Kind::Function,
            Tool::Image | Tool::MoveRotate => kind == Kind::Point,
            _ if self.asking.is_some() => false,
            _ if self.picked(label) => true,
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
        self.control_at = None;
        self.text_anchor = None;
        self.attachment_region = false;
        self.recent_attachment = None;
    }

    /// Handles a click. `hit` is the existing object under the pointer that
    /// the tool may use (see [`Self::accepts_object`]), `world` the position
    /// a new point takes (already captured to the grid, see `snap`), kept at
    /// full precision (the reference app stores what `transformCoords` gives
    /// and only prints two decimals), `is_taken` tells whether a label is in
    /// use.
    ///
    /// A click that lands on an existing point never creates a new point: it
    /// picks the point if the tool can use it, and is ignored otherwise.
    pub fn click(
        &mut self,
        hit: Option<(&str, Kind)>,
        world: (f64, f64),
        is_taken: &dyn Fn(&str) -> bool,
    ) -> Outcome {
        self.click_with(hit, &NewPoint::Free(world.0, world.1), is_taken)
    }

    /// [`Self::click`] where a new point is `new_point`.
    pub fn click_with(
        &mut self,
        hit: Option<(&str, Kind)>,
        new_point: &NewPoint,
        is_taken: &dyn Fn(&str) -> bool,
    ) -> Outcome {
        match (self.tool, hit) {
            (Tool::Move | Tool::MoveView | Tool::ZoomIn | Tool::ZoomOut, _) => {
                return Outcome::Ignored
            }
            (Tool::Delete, Some((label, _))) => return Outcome::Delete(label.to_owned()),
            (Tool::ShowHide, Some((label, _))) => return Outcome::ToggleVisible(label.to_owned()),
            (Tool::Delete | Tool::ShowHide, None) => return Outcome::Ignored,
            (Tool::Point | Tool::PointOnObject, Some((label, Kind::Point))) => {
                return Outcome::Select(label.to_owned())
            }
            (Tool::AttachDetach, _) => return Outcome::Ignored,
            _ if self.asking.is_some() => return Outcome::Ignored,
            _ => {}
        }
        if let Some((label, kind)) = hit {
            if self.tool == Tool::Area && matches!(kind, Kind::Polygon | Kind::Arc) {
                return Outcome::Ignored;
            }
            if self.tool == Tool::RigidPolygon && kind == Kind::Polygon && self.picks.is_empty() {
                return Outcome::RigidCopy(label.to_owned());
            }
            if self.tool == Tool::VectorPolygon && kind == Kind::Polygon {
                self.picks = vec![Pick::Object {
                    label: label.to_owned(),
                    kind,
                }];
                return self.build(None, is_taken);
            }
            let compass_center =
                self.tool == Tool::Compasses && self.picks.len() == 2 && kind == Kind::Point;
            if !self.closes_on(label) && !compass_center {
                if let Some(i) = self.picks.iter().position(|pick| pick.label() == label) {
                    self.picks.remove(i);
                    return Outcome::SelectionChanged;
                }
            }
        }
        // A point placed on empty space (or on an object the tool cannot
        // take) is added to the construction at once, as a pending point of
        // the tool use (the window runs it so): cancelling the tool takes it
        // away again.
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
                let (rhs, at) = match new_point {
                    NewPoint::Free(x, y) => (format!("({}, {})", exact(*x), exact(*y)), None),
                    NewPoint::OnPath { path, x, y } => (format!("Point({path})"), Some((*x, *y))),
                    NewPoint::InRegion { region, x, y } => {
                        (format!("PointIn({region})"), Some((*x, *y)))
                    }
                    NewPoint::Intersection { a, b, index: None } => {
                        (format!("Intersect({a}, {b})"), None)
                    }
                    NewPoint::Intersection {
                        a,
                        b,
                        index: Some(n),
                    } => (format!("Intersect({a}, {b}, {n})"), None),
                };
                (
                    Pick::Object {
                        label: label.clone(),
                        kind: Kind::Point,
                    },
                    Some((format!("{label} = {rhs}"), at)),
                )
            }
            _ => return Outcome::Ignored,
        };
        let closes = matches!(&pick, Pick::Object { label, .. } if self.closes_on(label));
        let then = if closes {
            self.build(None, is_taken)
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
            Some((line, at)) => Outcome::Place {
                line,
                at,
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
        if self.tool == Tool::Relation {
            return Outcome::Relation([picks[0].label().to_owned(), picks[1].label().to_owned()]);
        }
        if self.tool == Tool::VectorPolygon {
            return Outcome::VectorPolygon {
                picks,
                copy_offset: 0.0,
            };
        }
        let taken = |l: &str| is_taken(l) || picks.iter().any(|p| p.label() == l);
        let lines = recipe::build(self.tool, &picks, number, &taken);
        if lines.is_empty() {
            // The point tool: the placed point was all it makes.
            Outcome::Pending
        } else {
            Outcome::Run(lines)
        }
    }

    pub fn valid_number(&self, value: f64) -> bool {
        value.is_finite() && (self.tool != Tool::SegmentFixed || value >= 0.0)
    }

    pub(crate) fn set_attachment_pick(&mut self, pick: Pick, region: bool) -> Outcome {
        self.attachment_region = region;
        if self.picked(pick.label()) {
            self.picks.clear();
            Outcome::SelectionChanged
        } else {
            self.picks.push(pick);
            Outcome::Pending
        }
    }

    pub(crate) fn attachment_region(&self) -> bool {
        self.attachment_region
    }

    pub(crate) fn remember_attachment(&mut self, pos: (f64, f64)) {
        self.recent_attachment = Some(pos);
    }

    pub(crate) fn attachment_at(&self, pos: (f64, f64)) -> bool {
        self.recent_attachment == Some(pos)
    }

    pub(crate) fn observe_attachment_pointer(&mut self, pos: (f64, f64)) {
        if !self.attachment_at(pos) {
            self.recent_attachment = None;
        }
    }

    /// Completes a tool that asked for a number. `value` is in the unit the
    /// request named (degrees for angles). Returns [`Outcome::Ignored`] when
    /// the tool was not waiting.
    pub fn finish_number(&mut self, value: f64, is_taken: &dyn Fn(&str) -> bool) -> Outcome {
        if !self.valid_number(value) {
            return Outcome::Ignored;
        }
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
