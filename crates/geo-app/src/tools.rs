//! The construction tools as a state machine, with no UI types.
//!
//! A click on the drawing area is reported as [`ToolState::click`]. The state
//! remembers the objects picked so far; after the last pick it returns the
//! script lines to run, exactly as if the user had typed them.

use crate::describe::n2;
use crate::labels::next_label;
use geo_kernel::kernel::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Move,
    Point,
    Line,
    Segment,
    Ray,
    Circle,
    Midpoint,
    Perpendicular,
    Parallel,
    Bisector,
    Intersect,
    Delete,
}

impl Tool {
    pub const ALL: [Tool; 12] = [
        Tool::Move,
        Tool::Point,
        Tool::Line,
        Tool::Segment,
        Tool::Ray,
        Tool::Circle,
        Tool::Midpoint,
        Tool::Perpendicular,
        Tool::Parallel,
        Tool::Bisector,
        Tool::Intersect,
        Tool::Delete,
    ];

    /// The caption under the tool's button.
    pub fn caption(self) -> &'static str {
        match self {
            Tool::Move => "Przesuń",
            Tool::Point => "Punkt",
            Tool::Line => "Prosta",
            Tool::Segment => "Odcinek",
            Tool::Ray => "Półprosta",
            Tool::Circle => "Okrąg",
            Tool::Midpoint => "Środek",
            Tool::Perpendicular => "Prostopadła",
            Tool::Parallel => "Równoległa",
            Tool::Bisector => "Symetralna",
            Tool::Intersect => "Przecięcie",
            Tool::Delete => "Usuń",
        }
    }

    /// The command that builds the result from its (up to two) arguments.
    fn command(self) -> Option<&'static str> {
        Some(match self {
            Tool::Line => "Line",
            Tool::Segment => "Segment",
            Tool::Ray => "Ray",
            Tool::Circle => "Circle",
            Tool::Midpoint => "Midpoint",
            Tool::Perpendicular => "Perpendicular",
            Tool::Parallel => "Parallel",
            Tool::Bisector => "PerpendicularBisector",
            Tool::Intersect => "Intersect",
            Tool::Move | Tool::Point | Tool::Delete => return None,
        })
    }

    /// What kind of object the finished tool creates.
    fn result_kind(self) -> Kind {
        match self {
            Tool::Point | Tool::Midpoint | Tool::Intersect => Kind::Point,
            Tool::Segment => Kind::Segment,
            Tool::Ray => Kind::Ray,
            Tool::Circle => Kind::Circle,
            _ => Kind::Line,
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Point,
    /// A line, segment or ray.
    Straight,
    Circle,
    Other,
}

fn class_of(kind: Kind) -> Class {
    match kind {
        Kind::Point => Class::Point,
        Kind::Line | Kind::Segment | Kind::Ray => Class::Straight,
        Kind::Circle => Class::Circle,
        _ => Class::Other,
    }
}

impl Pick {
    fn class(&self) -> Class {
        match self {
            Pick::Object { kind, .. } => class_of(*kind),
            Pick::New { .. } => Class::Point,
        }
    }
}

/// What a click did.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The pick was taken; more are needed.
    Pending,
    /// Run these script lines as one change.
    Run(Vec<String>),
    /// Delete the object (and its dependents).
    Delete(String),
    /// Nothing happened: the click was not something the tool can use.
    Ignored,
}

#[derive(Clone, Debug)]
pub struct ToolState {
    tool: Tool,
    picks: Vec<Pick>,
}

impl ToolState {
    pub fn new(tool: Tool) -> Self {
        ToolState {
            tool,
            picks: Vec::new(),
        }
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switches tool and drops any half-finished picks.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.picks.clear();
    }

    /// The picks made so far (for drawing a preview).
    pub fn picks(&self) -> &[Pick] {
        &self.picks
    }

    /// Whether the next click may pick an existing object of this kind
    /// (`label` is checked so that the same object is not taken twice).
    pub fn accepts_object(&self, label: &str, kind: Kind) -> bool {
        let class = class_of(kind);
        match self.tool {
            Tool::Move | Tool::Point => false,
            Tool::Delete => true,
            _ if self
                .picks
                .iter()
                .any(|p| matches!(p, Pick::Object { label: l, .. } if l == label)) =>
            {
                false
            }
            _ => self.accepts_class(class, kind),
        }
    }

    /// Whether the next click may create a new point on empty space.
    pub fn accepts_new_point(&self) -> bool {
        match self.tool {
            Tool::Move | Tool::Delete | Tool::Intersect => false,
            Tool::Point => true,
            _ => self.accepts_class(Class::Point, Kind::Point),
        }
    }

    fn accepts_class(&self, class: Class, kind: Kind) -> bool {
        let have = |c: Class| self.picks.iter().any(|p| p.class() == c);
        match self.tool {
            Tool::Line
            | Tool::Segment
            | Tool::Ray
            | Tool::Circle
            | Tool::Midpoint
            | Tool::Bisector => class == Class::Point,
            Tool::Perpendicular => match class {
                Class::Point => !have(Class::Point),
                Class::Straight => !have(Class::Straight),
                _ => false,
            },
            Tool::Parallel => match class {
                Class::Point => !have(Class::Point),
                Class::Straight => kind != Kind::Ray && !have(Class::Straight),
                _ => false,
            },
            Tool::Intersect => matches!(class, Class::Straight | Class::Circle),
            Tool::Move | Tool::Point | Tool::Delete => false,
        }
    }

    /// A one-line Polish hint for what the next click should be.
    pub fn hint(&self) -> &'static str {
        let n = self.picks.len();
        let first_is_point = self
            .picks
            .first()
            .is_some_and(|p| p.class() == Class::Point);
        match (self.tool, n) {
            (Tool::Move, _) => "Przeciągnij punkt, aby go przesunąć",
            (Tool::Point, _) => "Kliknij, aby umieścić punkt",
            (Tool::Line, 0) => "Kliknij pierwszy punkt prostej",
            (Tool::Line, _) => "Kliknij drugi punkt prostej",
            (Tool::Segment, 0) => "Kliknij pierwszy koniec odcinka",
            (Tool::Segment, _) => "Kliknij drugi koniec odcinka",
            (Tool::Ray, 0) => "Kliknij początek półprostej",
            (Tool::Ray, _) => "Kliknij punkt na półprostej",
            (Tool::Circle, 0) => "Kliknij środek okręgu",
            (Tool::Circle, _) => "Kliknij punkt na okręgu",
            (Tool::Midpoint, 0) => "Kliknij pierwszy punkt",
            (Tool::Midpoint, _) => "Kliknij drugi punkt",
            (Tool::Perpendicular | Tool::Parallel, 0) => "Kliknij punkt lub prostą",
            (Tool::Perpendicular | Tool::Parallel, _) if first_is_point => "Kliknij prostą",
            (Tool::Perpendicular | Tool::Parallel, _) => "Kliknij punkt",
            (Tool::Bisector, 0) => "Kliknij pierwszy koniec odcinka",
            (Tool::Bisector, _) => "Kliknij drugi koniec odcinka",
            (Tool::Intersect, 0) => "Kliknij pierwszy obiekt",
            (Tool::Intersect, _) => "Kliknij drugi obiekt",
            (Tool::Delete, _) => "Kliknij obiekt do usunięcia",
        }
    }

    /// Forgets the picks (after an error, or on Escape).
    pub fn reset(&mut self) {
        self.picks.clear();
    }

    /// Handles a click. `hit` is the existing object under the pointer that
    /// the tool may use (see [`Self::accepts_object`]), `world` the pointer
    /// position, `is_taken` tells whether a label is in use.
    pub fn click(
        &mut self,
        hit: Option<(&str, Kind)>,
        world: (f64, f64),
        is_taken: &dyn Fn(&str) -> bool,
    ) -> Outcome {
        match self.tool {
            Tool::Move => return Outcome::Ignored,
            Tool::Delete => {
                return match hit {
                    Some((label, _)) => Outcome::Delete(label.to_owned()),
                    None => Outcome::Ignored,
                }
            }
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
                x: world.0,
                y: world.1,
            },
            _ => return Outcome::Ignored,
        };
        self.picks.push(pick);
        let needed = if self.tool == Tool::Point { 1 } else { 2 };
        if self.picks.len() < needed {
            return Outcome::Pending;
        }
        let lines = self.build(is_taken);
        self.picks.clear();
        Outcome::Run(lines)
    }

    fn build(&self, is_taken: &dyn Fn(&str) -> bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut allocated: Vec<String> = Vec::new();
        let fresh = |kind: Kind, allocated: &mut Vec<String>| {
            let label = next_label(kind, |l| is_taken(l) || allocated.iter().any(|a| a == l));
            allocated.push(label.clone());
            label
        };
        let mut args: Vec<String> = Vec::new();
        for pick in &self.picks {
            match pick {
                Pick::Object { label, .. } => args.push(label.clone()),
                Pick::New { x, y } => {
                    let label = fresh(Kind::Point, &mut allocated);
                    lines.push(format!("{label} = ({}, {})", n2(*x), n2(*y)));
                    args.push(label);
                }
            }
        }
        if let Some(command) = self.tool.command() {
            // Perpendicular and Parallel take the point first, then the line.
            if matches!(self.tool, Tool::Perpendicular | Tool::Parallel)
                && self
                    .picks
                    .first()
                    .is_some_and(|p| p.class() == Class::Straight)
            {
                args.swap(0, 1);
            }
            let label = fresh(self.tool.result_kind(), &mut allocated);
            lines.push(format!("{label} = {command}({})", args.join(", ")));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> bool {
        false
    }

    fn obj(label: &str, kind: Kind) -> Option<(&str, Kind)> {
        Some((label, kind))
    }

    #[test]
    fn point_tool_creates_a_labelled_free_point() {
        let mut s = ToolState::new(Tool::Point);
        let taken = |l: &str| l == "A";
        assert_eq!(
            s.click(None, (1.234, -0.001), &taken),
            Outcome::Run(vec!["B = (1.23, 0)".into()])
        );
        // Clicking an existing point does nothing.
        assert_eq!(
            s.click(obj("A", Kind::Point), (0.0, 0.0), &taken),
            Outcome::Ignored
        );
    }

    #[test]
    fn line_tool_over_two_new_points_runs_three_lines() {
        let mut s = ToolState::new(Tool::Line);
        assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Pending);
        assert_eq!(s.hint(), "Kliknij drugi punkt prostej");
        let out = s.click(None, (3.0, 4.0), &none);
        assert_eq!(
            out,
            Outcome::Run(vec![
                "A = (0, 0)".into(),
                "B = (3, 4)".into(),
                "f = Line(A, B)".into()
            ])
        );
        assert!(s.picks().is_empty());
        assert_eq!(s.hint(), "Kliknij pierwszy punkt prostej");
    }

    #[test]
    fn existing_points_are_reused_and_not_picked_twice() {
        let mut s = ToolState::new(Tool::Segment);
        assert_eq!(
            s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A"),
            Outcome::Pending
        );
        assert!(!s.accepts_object("A", Kind::Point));
        assert!(s.accepts_object("B", Kind::Point));
        // The same point again falls through to a new point? No: the click
        // landed on a point, so it is ignored.
        assert_eq!(
            s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A"),
            Outcome::Ignored
        );
        let out = s.click(obj("B", Kind::Point), (1.0, 1.0), &|l| l == "A" || l == "B");
        assert_eq!(out, Outcome::Run(vec!["f = Segment(A, B)".into()]));
    }

    #[test]
    fn circle_tool_uses_a_circle_label() {
        let mut s = ToolState::new(Tool::Circle);
        s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "A");
        let out = s.click(None, (2.0, 0.0), &|l| l == "A");
        assert_eq!(
            out,
            Outcome::Run(vec!["B = (2, 0)".into(), "c = Circle(A, B)".into()])
        );
    }

    #[test]
    fn perpendicular_takes_point_and_line_in_either_order() {
        let mut s = ToolState::new(Tool::Perpendicular);
        assert!(s.accepts_object("f", Kind::Line));
        assert!(s.accepts_object("A", Kind::Point));
        assert!(!s.accepts_object("c", Kind::Circle));
        assert_eq!(
            s.click(obj("f", Kind::Line), (0.0, 0.0), &|l| l == "f" || l == "A"),
            Outcome::Pending
        );
        assert_eq!(s.hint(), "Kliknij punkt");
        assert!(!s.accepts_object("g", Kind::Line), "needs a point now");
        let out = s.click(obj("A", Kind::Point), (0.0, 0.0), &|l| l == "f" || l == "A");
        assert_eq!(out, Outcome::Run(vec!["g = Perpendicular(A, f)".into()]));
    }

    #[test]
    fn perpendicular_with_a_new_point_after_the_line() {
        let mut s = ToolState::new(Tool::Perpendicular);
        s.click(obj("f", Kind::Line), (0.0, 0.0), &|l| l == "f");
        let out = s.click(None, (1.0, 2.0), &|l| l == "f");
        assert_eq!(
            out,
            Outcome::Run(vec!["A = (1, 2)".into(), "g = Perpendicular(A, f)".into()])
        );
    }

    #[test]
    fn parallel_rejects_rays() {
        let s = ToolState::new(Tool::Parallel);
        assert!(s.accepts_object("f", Kind::Segment));
        assert!(!s.accepts_object("r", Kind::Ray));
    }

    #[test]
    fn intersect_needs_two_distinct_objects_and_makes_a_point() {
        let mut s = ToolState::new(Tool::Intersect);
        assert!(!s.accepts_new_point());
        assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Ignored);
        assert_eq!(
            s.click(obj("h", Kind::Line), (0.0, 0.0), &none),
            Outcome::Pending
        );
        assert_eq!(
            s.click(obj("h", Kind::Line), (0.0, 0.0), &none),
            Outcome::Ignored
        );
        assert!(!s.accepts_object("A", Kind::Point));
        let out = s.click(obj("i", Kind::Line), (0.0, 0.0), &|l| l == "h" || l == "i");
        assert_eq!(out, Outcome::Run(vec!["A = Intersect(h, i)".into()]));
    }

    #[test]
    fn clicking_a_line_with_the_line_tool_places_a_new_point() {
        let mut s = ToolState::new(Tool::Line);
        let out = s.click(obj("f", Kind::Line), (2.0, 1.0), &none);
        assert_eq!(out, Outcome::Pending);
        assert_eq!(s.picks()[0], Pick::New { x: 2.0, y: 1.0 });
    }

    #[test]
    fn delete_and_move_tools() {
        let mut s = ToolState::new(Tool::Delete);
        assert_eq!(
            s.click(obj("A", Kind::Point), (0.0, 0.0), &none),
            Outcome::Delete("A".into())
        );
        assert_eq!(s.click(None, (0.0, 0.0), &none), Outcome::Ignored);
        let mut m = ToolState::new(Tool::Move);
        assert_eq!(m.click(None, (0.0, 0.0), &none), Outcome::Ignored);
    }

    #[test]
    fn switching_tool_drops_picks() {
        let mut s = ToolState::new(Tool::Line);
        s.click(None, (0.0, 0.0), &none);
        assert_eq!(s.picks().len(), 1);
        s.set_tool(Tool::Circle);
        assert!(s.picks().is_empty());
    }

    #[test]
    fn labels_allocated_inside_one_run_do_not_collide() {
        let mut s = ToolState::new(Tool::Bisector);
        s.click(None, (0.0, 0.0), &|l| l == "A");
        let out = s.click(None, (1.0, 0.0), &|l| l == "A");
        assert_eq!(
            out,
            Outcome::Run(vec![
                "B = (0, 0)".into(),
                "C = (1, 0)".into(),
                "f = PerpendicularBisector(B, C)".into()
            ])
        );
    }
}
