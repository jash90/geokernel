//! The construction tools as a state machine, with no UI types.
//!
//! A click on the drawing area is reported as [`ToolState::click`]. The state
//! remembers the objects picked so far; after the last pick it returns the
//! script lines to run, exactly as if the user had typed them.

use crate::describe::fixed;
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

fn rounded(v: f64, decimals: usize) -> f64 {
    fixed(v, decimals).parse().unwrap_or(v)
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
    /// Select this existing object (the point tool clicked on a point).
    Select(String),
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
                Class::Straight => kind != Kind::Ray && !have(Class::Straight),
                _ => false,
            },
            Tool::Parallel => match class {
                Class::Point => !have(Class::Point),
                Class::Straight => kind != Kind::Ray && !have(Class::Straight),
                _ => false,
            },
            // The kernel intersects whole lines and circles only.
            Tool::Intersect => matches!(kind, Kind::Line | Kind::Circle),
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
        match self.tool {
            Tool::Move => return Outcome::Ignored,
            Tool::Delete => {
                return match hit {
                    Some((label, _)) => Outcome::Delete(label.to_owned()),
                    None => Outcome::Ignored,
                }
            }
            Tool::Point => {
                if let Some((label, Kind::Point)) = hit {
                    return Outcome::Select(label.to_owned());
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
                x: rounded(world.0, decimals),
                y: rounded(world.1, decimals),
            },
            _ => return Outcome::Ignored,
        };
        self.picks.push(pick);
        let needed = if self.tool == Tool::Point { 1 } else { 2 };
        if self.picks.len() < needed {
            return Outcome::Pending;
        }
        let lines = self.build(decimals, is_taken);
        self.picks.clear();
        Outcome::Run(lines)
    }

    fn build(&self, decimals: usize, is_taken: &dyn Fn(&str) -> bool) -> Vec<String> {
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
                    lines.push(format!(
                        "{label} = ({}, {})",
                        fixed(*x, decimals),
                        fixed(*y, decimals)
                    ));
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
mod tests;
