//! Construction kernel: a dependency graph of geometric objects.
//!
//! Every object is either *free* (a point or a number the user can change)
//! or *dependent* (defined by a rule over other objects). New definitions
//! read earlier objects. A redefinition can change the evaluation order
//! while keeping the same object IDs.
//!
//! When a free object changes, only its descendants are recomputed, in
//! dependency order, and a branch is cut as soon as a recomputed value turns
//! out to be unchanged.
//!
//! # Definitions with any number of parents
//!
//! [`Def`] and [`Value`] are `Copy` and have a fixed size, so a definition
//! that reads any number of objects (the vertices of a polygon, for example)
//! keeps them in a side table of the construction and holds an [`IdList`],
//! a small `Copy` handle to one stored list:
//!
//! - [`Construction::add_with_list`] stores the list, passes its handle to a
//!   closure that makes the definition, and adds the object. When the object
//!   is rejected the list is removed again, so a failed call leaves the
//!   construction as it was.
//! - [`Construction::list`] reads a stored list back.
//! - [`Def::list`] gives the handle a definition holds, and [`Def::with_list`]
//!   replaces it. To copy such a definition into another construction, store
//!   `list(h)` there with `add_with_list` and pass `def.with_list(new_h)`.
//!
//! Stored lists never change. The objects of a list are parents like any
//! other: they are linked into the dependency graph when the object is added,
//! so incremental updates treat them exactly like fixed parents.

#[path = "algo/angle.rs"]
pub mod angle;
pub mod animation;
#[path = "algo/axis.rs"]
mod axis;
#[path = "control.rs"]
pub mod control;
pub mod image;
#[path = "algo/locus.rs"]
pub mod locus;
mod slider;
pub mod text;

use self::angle::{AngleDef, AngleGeometry, AngleSource, AngleStyle};
use self::locus::{LocusData, LocusDef};
use crate::algo::analysis::{AnalysisCache, AnalysisDef};
use crate::algo::arc::{ArcData, ArcDef};
use crate::algo::calculus::{CalcRecord, CalculusDef};
use crate::algo::conic::ConicDef;
use crate::algo::conic_nd::{ConicData, ConicType};
use crate::algo::conic_parts::ConicPartDef;
use crate::algo::general_conic::GeneralConicDef;
use crate::algo::intersect::common_tangents::CommonTangentState;
use crate::algo::intersect::{
    boundary::BoundaryDef,
    line_conic_state::{LineConicDef, LineConicState},
    pair::{PairDef, PairState},
    tangent_state::{TangentDef, TangentState},
};
use crate::algo::lines::{CommonTangentDef, LineDef};
use crate::algo::measure::MeasureDef;
use crate::algo::optimize::OptimizeDef;
use crate::algo::path::{PathDef, PathState};
use crate::algo::point::PointDef;
use crate::algo::polygon::PolygonDef;
use crate::algo::predicate::PredicateDef;
use crate::algo::transform::TransformDef;
use crate::algo::vector::VectorDef;
use crate::algo::{
    self, check_index, join, line_through, Group, Parents, CIRCLE, LINE, LINE_LIKE, NUMBER, POINT,
};
use crate::expr::{self, ExprDef};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, Weak};

/// Handle of an object inside one [`Construction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub(crate) usize);

impl Id {
    /// The slot of a constructed object. Built-in axes have reserved ids
    /// outside the range of construction slots.
    pub fn index(self) -> usize {
        self.0
    }
}

/// Handle of a list of objects stored in one [`Construction`] (see
/// [`Construction::add_with_list`]).
///
/// A handle is only meaningful in the construction that stored the list. A
/// construction checks only that a handle lies within its own lists:
/// [`Construction::list`] reads a handle beyond them as an empty list, and
/// adding a definition with such a handle fails. A handle from another
/// construction that happens to lie within the lists reads whatever objects
/// are stored there. Several definitions may share one handle; a list that
/// no definition reads any more stays stored and does no harm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdList {
    start: u32,
    len: u32,
}

impl IdList {
    /// How many objects the list holds.
    pub fn len(self) -> usize {
        self.len as usize
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.start as usize + self.len as usize
    }
}

/// What an object is, independent of whether it is currently defined.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Point,
    Line,
    Segment,
    Circle,
    Number,
    /// An angle in radians, with its scalar stored in `Value::Number`.
    Angle,
    Vector,
    Ray,
    /// A true or false answer, such as the result of `AreCollinear`.
    Boolean,
    /// A function of `x`, `f(x) = ...`; see [`Construction::function_value`].
    Function,
    /// Text produced by an expression, read with [`Construction::text`].
    Text,
    /// A scalar function of two coordinates, such as one side of an equation.
    FunctionNVar,
    Polygon,
    PolyLine,
    /// A list of values such as `{1, 2, 3}`. Read its elements with
    /// [`Construction::elements`].
    List,
    /// A general conic (`Conic`, `Ellipse`, `Hyperbola`, `Parabola` and
    /// their images); its state is read with [`Construction::conic`].
    Conic,
    /// An arc of a circle or conic; its state is read with
    /// [`Construction::arc`].
    Arc,
    /// A sector of a circle or conic, read like an arc.
    Sector,
    /// The path a point traces while another point moves along a path or a
    /// slider runs through its interval; its points are read with
    /// [`Construction::locus`].
    Locus,
    Curve,
    ImplicitCurve,
    Button,
    InputBox,
    Image,
    PieChart,
    /// A freehand stroke of lines and cubic pieces, frozen at creation;
    /// its points are read with [`Construction::stroke`].
    PenStroke,
}

impl Kind {
    /// Whether this kind can supply a scalar number.
    pub const fn is_number(self) -> bool {
        matches!(self, Self::Number | Self::Angle)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Point => "point",
            Kind::Line => "line",
            Kind::Segment => "segment",
            Kind::Circle => "circle",
            Kind::Number => "number",
            Kind::Angle => "angle",
            Kind::Vector => "vector",
            Kind::Ray => "ray",
            Kind::Boolean => "boolean",
            Kind::Function => "function",
            Kind::Text => "text",
            Kind::FunctionNVar => "function_nvar",
            Kind::Polygon => "polygon",
            Kind::PolyLine => "polyline",
            Kind::List => "list",
            Kind::Conic => "conic",
            Kind::Arc => "arc",
            Kind::Sector => "sector",
            Kind::Locus => "locus",
            Kind::Curve => "curve",
            Kind::ImplicitCurve => "implicit",
            Kind::Button => "button",
            Kind::InputBox => "input box",
            Kind::Image => "image",
            Kind::PieChart => "pie chart",
            Kind::PenStroke => "pen stroke",
        })
    }
}

/// Current numeric state of an object.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Point {
        x: f64,
        y: f64,
    },
    /// Stored coordinates of a projective point. The public value of a
    /// point is its Cartesian projection, when finite.
    #[doc(hidden)]
    HomogeneousPoint {
        x: f64,
        y: f64,
        z: f64,
    },
    /// `a*x + b*y + c = 0` with the raw coefficients GeoGebra's `GeoLine`
    /// holds (its `x`, `y`, `z`) for the same definition. They are not
    /// normalised: the algorithm that makes the line decides their scale, and
    /// GeoGebra's zero and equality tests on lines depend on it. `(a, b)` is
    /// never zero (a line with both zero is undefined). The line's direction
    /// is `(b, -a)`. [`Value::unit_line`] gives the form with `(a, b)` of
    /// unit length.
    Line {
        a: f64,
        b: f64,
        c: f64,
    },
    Segment {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    /// A number. It is never NaN (that is [`Value::Undefined`]) but may be
    /// `∞` or `-∞`, which the reference keeps as a defined value.
    Number(f64),
    Vector {
        x: f64,
        y: f64,
    },
    /// A ray from the start point `(x, y)` along the line `a*x + b*y + c = 0`,
    /// stored like [`Value::Line`]; it points along the direction `(b, -a)`.
    Ray {
        x: f64,
        y: f64,
        a: f64,
        b: f64,
        c: f64,
    },
    /// A boolean object. GeoGebra's `GeoBoolean` is undefined only for the
    /// prover's "unknown" answer, so the kernel reports `Undefined` only when
    /// a predicate cannot decide.
    Bool(bool),
    Button,
    InputBox,
    Image,
    PieChart,
    /// A defined function; its values come from
    /// [`Construction::function_value`].
    Function,
    /// The parameter interval of a defined Cartesian curve.
    ImplicitCurve,
    Curve {
        start: f64,
        end: f64,
    },
    Text,
    FunctionNVar,
    /// A polygon with its signed area: positive when the vertices run
    /// counter-clockwise. The vertices are not copied into the value; read
    /// them with [`Construction::vertices`].
    Polygon {
        area: f64,
    },
    /// A polyline with its length. Its vertices are read with
    /// [`Construction::vertices`] like those of a polygon.
    PolyLine {
        length: f64,
    },
    /// A list with its number of elements. The elements are not copied
    /// into the value; read them with [`Construction::elements`].
    List {
        len: u32,
    },
    /// A defined general conic of type `ty`. Its matrix and parts are not
    /// copied into the value; read them with [`Construction::conic`].
    Conic {
        ty: ConicType,
    },
    /// A defined arc with its length. Its conic and parameters are read
    /// with [`Construction::arc`].
    Arc {
        length: f64,
    },
    /// A defined sector with its area, read like an arc.
    Sector {
        area: f64,
    },
    /// A defined locus with its number of points. The points are not
    /// copied into the value; read them with [`Construction::locus`].
    Locus {
        len: u32,
    },
    /// A defined pen stroke with its number of points, subpath breaks
    /// included. The points are read with [`Construction::stroke`].
    PenStroke {
        len: u32,
    },
    /// The object does not exist for the current positions
    /// (for example two circles that no longer meet).
    Undefined,
}

impl Value {
    /// `GeoPoint.updateCoords` makes the scale positive for finite points.
    pub(crate) fn homogeneous_point(x: f64, y: f64, z: f64) -> Self {
        if x.is_nan() || y.is_nan() || z.is_nan() {
            return Self::Undefined;
        }
        let (x, y, z) = if z < 0.0 && !point_at_infinity(x, y, z) {
            (-x, -y, -z)
        } else {
            (x, y, z)
        };
        if z == 1.0 && x.is_finite() && y.is_finite() {
            Self::Point { x, y }
        } else {
            Self::HomogeneousPoint { x, y, z }
        }
    }

    pub(crate) fn point_coords(self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Point { x, y } => Some((x, y, 1.0)),
            Self::HomogeneousPoint { x, y, z } => Some((x, y, z)),
            _ => None,
        }
    }

    pub(crate) fn cartesian(self) -> Self {
        match self {
            Self::HomogeneousPoint { x, y, z } if !point_at_infinity(x, y, z) => {
                let (x, y) = (x / z, y / z);
                if x.is_finite() && y.is_finite() {
                    return Self::Point { x, y };
                }
                Self::Undefined
            }
            Self::HomogeneousPoint { .. } => Self::Undefined,
            Self::Arc { length } if !length.is_finite() => Self::Undefined,
            Self::Sector { area } if !area.is_finite() => Self::Undefined,
            _ => self,
        }
    }

    pub fn is_defined(&self) -> bool {
        !matches!(self, Value::Undefined)
    }

    /// For a line, `(a, b, c)` scaled so that `(a, b)` has length 1, with the
    /// same orientation. `None` for every other value.
    pub fn unit_line(&self) -> Option<(f64, f64, f64)> {
        match *self {
            Value::Line { a, b, c } => {
                let len = a.hypot(b);
                (len > 0.0).then(|| (a / len, b / len, c / len))
            }
            _ => None,
        }
    }

    /// Whether `other` is this value bit for bit. Unlike `==` it tells `0`
    /// from `-0`: a dependent `1 / v` turns that sign into the sign of an
    /// infinity, so the update must not take the two for the same value.
    pub(crate) fn identical(&self, other: &Value) -> bool {
        fn parts(v: &Value) -> [f64; 5] {
            match *v {
                Value::Point { x, y } | Value::Vector { x, y } => [x, y, 0.0, 0.0, 0.0],
                Value::HomogeneousPoint { x, y, z } => [x, y, z, 0.0, 0.0],
                Value::Line { a, b, c } => [a, b, c, 0.0, 0.0],
                Value::Segment { x1, y1, x2, y2 } => [x1, y1, x2, y2, 0.0],
                Value::Circle { cx, cy, r } => [cx, cy, r, 0.0, 0.0],
                Value::Ray { x, y, a, b, c } => [x, y, a, b, c],
                Value::Curve { start, end } => [start, end, 0.0, 0.0, 0.0],
                Value::Number(v)
                | Value::Polygon { area: v }
                | Value::PolyLine { length: v }
                | Value::Arc { length: v }
                | Value::Sector { area: v } => [v, 0.0, 0.0, 0.0, 0.0],
                Value::Bool(_)
                | Value::Button
                | Value::InputBox
                | Value::Image
                | Value::PieChart
                | Value::Function
                | Value::Text
                | Value::FunctionNVar
                | Value::List { .. }
                | Value::Conic { .. }
                | Value::Locus { .. }
                | Value::PenStroke { .. }
                | Value::ImplicitCurve
                | Value::Undefined => [0.0; 5],
            }
        }
        self == other && parts(self).map(f64::to_bits) == parts(other).map(f64::to_bits)
    }

    fn is_valid(&self) -> bool {
        match *self {
            Value::Point { x, y } => !(x.is_nan() || y.is_nan()),
            Value::HomogeneousPoint { x, y, z } => !(x.is_nan() || y.is_nan() || z.is_nan()),
            Value::Line { a, b, c } => a.is_finite() && b.is_finite() && c.is_finite(),
            Value::Segment { x1, y1, x2, y2 } => {
                x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()
            }
            Value::Circle { cx, cy, r } => cx.is_finite() && cy.is_finite() && !r.is_nan(),
            Value::Number(v) => !v.is_nan(),
            Value::Vector { x, y } => !(x.is_nan() || y.is_nan()),
            Value::Ray { x, y, a, b, c } => [x, y, a, b, c].iter().all(|v| v.is_finite()),
            Value::Bool(_) | Value::Button | Value::InputBox | Value::Image | Value::PieChart => {
                true
            }
            Value::Polygon { area } => area.is_finite(),
            Value::PolyLine { length } => length.is_finite(),
            Value::Arc { length } => length.is_finite(),
            Value::Sector { area } => area.is_finite(),
            Value::Curve { start, end } => start.is_finite() && end.is_finite(),
            Value::Function
            | Value::Text
            | Value::FunctionNVar
            | Value::ImplicitCurve
            | Value::List { .. }
            | Value::Conic { .. }
            | Value::Locus { .. }
            | Value::PenStroke { .. }
            | Value::Undefined => true,
        }
    }
}

/// The hidden label of an output whose label was freed.
fn freed_label(id: Id) -> String {
    format!("\0freed{}", id.0)
}

/// Starts the hidden label of a first output whose label was freed.
const FREED_OWNER: &str = "\0freed owner ";

fn freed_owner_label(label: &str) -> String {
    format!("{FREED_OWNER}{label}")
}

/// `DoubleUtil.isEpsilon(z, x, y)` for non-NaN homogeneous coordinates.
pub(crate) fn point_at_infinity(x: f64, y: f64, z: f64) -> bool {
    let tol = crate::tol::STANDARD_PRECISION;
    z.abs() <= tol && z.abs() <= x.abs() * tol && z.abs() <= y.abs() * tol
}

/// How an object is defined.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    /// The built-in horizontal coordinate axis.
    XAxis,
    /// The built-in vertical coordinate axis.
    YAxis,
    /// A point the user can drag.
    FreePoint {
        x: f64,
        y: f64,
    },
    /// A number the user can change (a slider).
    Number(f64),
    Boolean(bool),
    /// A geometric `If`: condition and alternative pairs, then an optional else object.
    Conditional {
        args: IdList,
        kind: Kind,
    },
    Button,
    InputBox(Option<Id>),
    Image(IdList),
    Chart(algo::charts::ChartDef),
    /// A pen stroke; its frozen points are kept by the construction (see
    /// [`Construction::stroke`]) and it reads no objects.
    PenStroke,
    /// A free angle in radians, such as `c = 30°`: a number the user can
    /// change that `floor`, `ceil` and `round` treat in whole degrees
    /// (GeoGebra's independent `GeoAngle`).
    Angle(f64),
    /// Line through two points.
    Line(Id, Id),
    /// Segment between two points.
    Segment(Id, Id),
    Midpoint(Id, Id),
    /// Distance between two points.
    Distance(Id, Id),
    /// Line through a point, perpendicular to a line or segment.
    Perpendicular {
        through: Id,
        to: Id,
    },
    /// Line through a point, parallel to a line or segment.
    Parallel {
        through: Id,
        to: Id,
    },
    /// Circle with a centre, passing through a point.
    CircleThrough {
        center: Id,
        through: Id,
    },
    /// Circle with a centre and a radius given by a number object.
    CircleRadius {
        center: Id,
        radius: Id,
    },
    IntersectLines(Id, Id),
    /// `which` is 0 or 1: the first or second point along the line's direction.
    IntersectLineCircle {
        line: Id,
        circle: Id,
        which: u8,
    },
    /// `which` is 0 or 1: the point to the left or to the right of the
    /// direction from the first centre to the second. `special` is state of
    /// the intersection from the moment it was created: whether the circles
    /// had a common point by construction then. A new intersection passes
    /// `None` and [`Construction::add`] decides it; a definition read back
    /// with [`Construction::def`] carries the decision, and `add` keeps it,
    /// so a construction rebuilt from its definitions is the same. `add` also
    /// gives a second intersection of the same circles the order and the flag
    /// of the first.
    IntersectCircles {
        c1: Id,
        c2: Id,
        which: u8,
        special: Option<bool>,
    },
    /// One output of two conics, with its branch history in the construction.
    IntersectConics(PairDef),
    /// One labelled or selected output of an intersection of a carrier and conic.
    IntersectLineConic(LineConicDef),
    /// One output of an intersection with a polygon or polyline.
    IntersectBoundary(BoundaryDef),
    /// Bisectors and tangents.
    Lines(LineDef),
    /// Circles and their parts.
    Conic(ConicDef),
    /// Numbers measured on objects.
    Measure(MeasureDef),
    /// An angle command, a styled angle, or the point made by a fixed angle.
    AngleObject(AngleDef),
    /// Reflections, rotations and dilations.
    Transform(TransformDef),
    /// Vectors, rays and translations.
    Vector(VectorDef),
    /// Boolean tests such as `AreCollinear`.
    Predicate(PredicateDef),
    /// `Prove` and `ProveDetails`; the construction keeps the result (see
    /// [`Construction::proof`]).
    Prove(algo::prover::ProveDef),
    /// Points from `ClosestPoint` and `Trilinear`.
    Point(PointDef),
    /// An arithmetic expression; the tree is kept by the construction, so
    /// this handle is valid only there. Rebuild it elsewhere with
    /// [`Construction::expression`] and [`Construction::add_expression`].
    Expr(ExprDef),
    /// Polygons, polylines and what is measured on them.
    Polygon(PolygonDef),
    /// General conics and transformations of conics; the construction keeps
    /// a [`ConicData`] record for each (see [`Construction::conic`]).
    GeneralConic(GeneralConicDef),
    /// Objects computed from a general conic, such as its intersection
    /// points with a line.
    ConicPart(ConicPartDef),
    /// Points on paths and their parameters; a point on a path keeps a
    /// [`PathState`] in the construction (see [`Construction::path_state`]).
    Path(PathDef),
    /// Arcs and sectors and what is measured on them; an arc or sector
    /// keeps an [`ArcData`] record in the construction (see
    /// [`Construction::arc`]).
    Arc(ArcDef),
    /// `Locus(Q, P)` and `Locus(Q, slider)`; the construction keeps the
    /// points of each locus (see [`Construction::locus`]).
    Locus(LocusDef),
    /// `Maximize(dep, indep)` / `Minimize(dep, indep)` over a bounded
    /// number; the search itself runs in
    /// [`algo::optimize::record`], as a locus record does.
    Optimize(OptimizeDef),
    /// A locus built from points or a segment graph.
    Discrete(algo::discrete::DiscreteDef),
    /// A point, number or circle computed from a function or curve.
    Analysis(AnalysisDef),
    /// A line or conic defined by an equation in `x` and `y`
    /// (`a: 2x + 3y = 6`, `y = m x + b`); the construction keeps the trees
    /// of its coefficients (see [`crate::expr`]). Like [`Def::Expr`], the
    /// handle is valid only in the construction that made it.
    Equation(EquationDef),
    /// Handle for a polynomial curve or a variable-degree equation.
    ImplicitCurve(u32),
    /// The calculus commands of the function group: the integrals, the
    /// derivatives, the Riemann sums and the polynomial queries. An
    /// object whose value samples a tree keeps it in the construction's
    /// calculus records (see [`Construction::calculus_record`]). The
    /// objects it reads are `u32` indices, so the five inputs of a sum
    /// still fit the fixed size of a definition.
    Calculus(CalculusDef),
}

/// A line or conic defined by an equation in `x` and `y`: which of the two
/// it is, and where the construction keeps its coefficient trees (see
/// [`Construction::equation_coefficients`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquationDef {
    pub(crate) conic: bool,
    pub(crate) index: u32,
}

impl EquationDef {
    /// Whether the equation makes a conic; a line otherwise.
    pub fn is_conic(&self) -> bool {
        self.conic
    }
}

/// The record of one [`Def::Equation`] object: the indices of the entries
/// of its coefficient trees, three for a line and six for a conic, in the
/// equation form's order.
#[derive(Clone, Debug, PartialEq)]
pub struct EquationRecord {
    pub(crate) coeffs: Vec<u32>,
    pub(crate) sides: [u32; 2],
}

/// The presentation of a line or conic, independent of its coefficients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EquationForm {
    Implicit,
    Explicit,
    General,
    Specific,
    Parametric(String),
    User,
    Vertex,
    Conic,
}

impl EquationForm {
    /// The style and optional parameter from a saved equation.
    pub fn from_xml(style: &str, parameter: Option<&str>) -> Option<Self> {
        Some(match style {
            "implicit" => Self::Implicit,
            "explicit" => Self::Explicit,
            "general" => Self::General,
            "specific" => Self::Specific,
            "parametric" => Self::Parametric(parameter.unwrap_or("t").to_owned()),
            "user" => Self::User,
            "vertex" => Self::Vertex,
            "conic" => Self::Conic,
            _ => return None,
        })
    }

    /// The style name used in saved files.
    pub fn xml_style(&self) -> &'static str {
        match self {
            Self::Implicit => "implicit",
            Self::Explicit => "explicit",
            Self::General => "general",
            Self::Specific => "specific",
            Self::Parametric(_) => "parametric",
            Self::User => "user",
            Self::Vertex => "vertex",
            Self::Conic => "conic",
        }
    }
}

impl Def {
    pub fn kind(&self) -> Kind {
        match self {
            Def::FreePoint { .. }
            | Def::Midpoint(..)
            | Def::IntersectLines(..)
            | Def::IntersectLineCircle { .. }
            | Def::IntersectCircles { .. } => Kind::Point,
            Def::XAxis
            | Def::YAxis
            | Def::Line(..)
            | Def::Perpendicular { .. }
            | Def::Parallel { .. } => Kind::Line,
            Def::Segment(..) => Kind::Segment,
            Def::CircleThrough { .. } | Def::CircleRadius { .. } => Kind::Circle,
            Def::Number(_) | Def::Distance(..) => Kind::Number,
            Def::Boolean(_) => Kind::Boolean,
            Def::Conditional { kind, .. } => *kind,
            Def::Button => Kind::Button,
            Def::InputBox(_) => Kind::InputBox,
            Def::Image(_) => Kind::Image,
            Def::Chart(d) => d.kind(),
            Def::PenStroke => Kind::PenStroke,
            Def::Angle(_)
            | Def::Measure(MeasureDef::Angle(..))
            | Def::Polygon(PolygonDef::Angle { .. }) => Kind::Angle,
            Def::AngleObject(d) => d.kind(),
            Def::Lines(d) => d.kind(),
            Def::Conic(d) => d.kind(),
            Def::Measure(d) => d.kind(),
            Def::Transform(d) => d.kind(),
            Def::Vector(d) => d.kind(),
            Def::Predicate(d) => d.kind(),
            Def::Prove(d) => d.kind(),
            Def::Point(d) => d.kind(),
            Def::IntersectConics(d) => d.kind(),
            Def::IntersectLineConic(d) => d.kind(),
            Def::IntersectBoundary(d) => d.kind(),
            Def::Expr(d) => d.kind(),
            Def::Polygon(d) => d.kind(),
            Def::GeneralConic(d) => d.kind(),
            Def::ConicPart(d) => d.kind(),
            Def::Path(d) => d.kind(),
            Def::Arc(d) => d.kind(),
            Def::ImplicitCurve(_) => Kind::ImplicitCurve,
            Def::Equation(d) => {
                if d.is_conic() {
                    Kind::Conic
                } else {
                    Kind::Line
                }
            }
            Def::Locus(d) => d.kind(),
            Def::Optimize(d) => d.kind(),
            Def::Calculus(d) => d.kind(),
            Def::Discrete(_) => Kind::Locus,
            Def::Analysis(d) => d.kind(),
        }
    }

    /// The stored list of parents this definition reads, if it has one.
    pub fn list(&self) -> Option<IdList> {
        match self {
            Def::Image(list) => Some(*list),
            Def::Conditional { args, .. } => Some(*args),
            Def::Chart(d) => Some(d.args),
            Def::Polygon(d) => d.list(),
            Def::GeneralConic(d) => d.list(),
            _ => None,
        }
    }

    /// The same definition reading the list `list` instead of its own.
    /// A definition without a list is returned unchanged.
    pub fn with_list(self, list: IdList) -> Def {
        match self {
            Def::Image(_) => Def::Image(list),
            Def::Conditional { kind, .. } => Def::Conditional { args: list, kind },
            Def::Chart(mut d) => {
                d.args = list;
                Def::Chart(d)
            }
            Def::Polygon(d) => Def::Polygon(d.with_list(list)),
            Def::GeneralConic(d) => Def::GeneralConic(d.with_list(list)),
            other => other,
        }
    }

    pub fn is_free(&self) -> bool {
        matches!(
            self,
            Def::FreePoint { .. }
                | Def::Number(_)
                | Def::Boolean(_)
                | Def::Button
                | Def::InputBox(None)
                | Def::Angle(_)
                | Def::Vector(VectorDef::Free { .. })
        ) || matches!(*self, Def::AngleObject(d) if d.is_free())
    }

    /// The objects this definition reads. An expression reads the objects
    /// of its tree, which [`expr::parents`] lists, and an equation those of
    /// its coefficients ([`expr::equation::parents`]).
    pub(crate) fn parents(&self) -> Parents {
        match *self {
            Def::XAxis
            | Def::YAxis
            | Def::FreePoint { .. }
            | Def::Number(_)
            | Def::Boolean(_)
            | Def::Button
            | Def::Conditional { .. }
            | Def::InputBox(None)
            | Def::Image(_)
            | Def::Chart(_)
            | Def::PenStroke
            | Def::Angle(_)
            | Def::Expr(_)
            | Def::Equation(_)
            | Def::ImplicitCurve(_) => Parents::none(),
            Def::InputBox(Some(id)) => Parents::of(&[id]),
            Def::Line(p, q)
            | Def::Segment(p, q)
            | Def::Midpoint(p, q)
            | Def::Distance(p, q)
            | Def::IntersectLines(p, q) => Parents::of(&[p, q]),
            Def::Perpendicular { through, to } | Def::Parallel { through, to } => {
                Parents::of(&[through, to])
            }
            Def::CircleThrough { center, through } => Parents::of(&[center, through]),
            Def::CircleRadius { center, radius } => Parents::of(&[center, radius]),
            Def::IntersectLineCircle { line, circle, .. } => Parents::of(&[line, circle]),
            Def::IntersectCircles { c1, c2, .. } => Parents::of(&[c1, c2]),
            Def::IntersectConics(d) => d.parents(),
            Def::IntersectLineConic(d) => d.parents(),
            Def::IntersectBoundary(d) => d.parents(),
            Def::Lines(d) => d.parents(),
            Def::Conic(d) => d.parents(),
            Def::Measure(d) => d.parents(),
            Def::AngleObject(d) => d.parents(),
            Def::Transform(d) => d.parents(),
            Def::Vector(d) => d.parents(),
            Def::Predicate(d) => d.parents(),
            Def::Prove(d) => Parents::of(&[d.statement]),
            Def::Point(d) => d.parents(),
            Def::Polygon(d) => d.parents(),
            Def::GeneralConic(d) => d.parents(),
            Def::ConicPart(d) => d.parents(),
            Def::Path(d) => d.parents(),
            Def::Arc(d) => d.parents(),
            Def::Locus(d) => d.parents(),
            Def::Optimize(d) => d.parents(),
            Def::Calculus(d) => d.parents(),
            Def::Discrete(d) => d.parents(),
            Def::Analysis(d) => d.parents(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    EmptyLabel,
    DuplicateLabel(String),
    UnknownObject,
    WrongKind {
        label: String,
        expected: &'static [Kind],
        found: Kind,
    },
    NotFree(String),
    BadIndex(u8),
    NotFinite,
    InvalidTransformList,
    EvaluationLimit,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyLabel => write!(f, "object name is empty"),
            Error::DuplicateLabel(l) => write!(f, "object `{l}` already exists"),
            Error::UnknownObject => write!(f, "unknown object"),
            Error::WrongKind {
                label,
                expected,
                found,
            } => {
                let names: Vec<String> = expected.iter().map(|k| k.to_string()).collect();
                write!(
                    f,
                    "`{label}` is a {found}, expected a {}",
                    names.join(" or ")
                )
            }
            Error::NotFree(l) => write!(
                f,
                "`{l}` is a dependent object and cannot be changed directly"
            ),
            Error::BadIndex(i) => write!(f, "intersection index {i} is out of range"),
            Error::NotFinite => write!(f, "coordinates must be finite numbers"),
            Error::EvaluationLimit => {
                write!(f, "expression validation exceeded the evaluation limit")
            }
            Error::InvalidTransformList => write!(
                f,
                "the list transform has no reusable geometry for its first element"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// A state to return to with [`Construction::rollback`].
#[derive(Clone, Debug)]
pub struct Checkpoint {
    len: usize,
    lists: usize,
    exprs: usize,
    frozen_line_starts: usize,
    proof_intersections: usize,
    image_assets: usize,
    step: usize,
    /// How many rollbacks the construction had made when it was taken.
    generation: u64,
    protocol: Option<ProtocolCheckpoint>,
}

/// Appending a construction element can update earlier protocol dependents.
/// Save the first prior record touched between checkpoints, as part of the
/// actual update. A construction holds a weak reference so these records
/// are released when the last checkpoint is dropped.
type ProtocolHistory = Mutex<Vec<ProtocolFrame>>;

#[derive(Clone, Debug)]
struct ProtocolCheckpoint {
    history: Arc<ProtocolHistory>,
    frame: usize,
    _live: Arc<()>,
}

#[derive(Debug)]
struct ProtocolFrame {
    live: Weak<()>,
    before: BTreeMap<usize, ProtocolRecord>,
}

impl ProtocolFrame {
    fn merge_unused_tail(frames: &mut Vec<Self>) {
        while frames.len() > 1
            && frames
                .last()
                .is_some_and(|frame| frame.live.strong_count() == 0)
        {
            let unused = frames.pop().expect("unused frame");
            let earlier = &mut frames.last_mut().expect("earlier frame").before;
            for (i, record) in unused.before {
                earlier.entry(i).or_insert(record);
            }
        }
    }
}

#[derive(Debug)]
struct ProtocolRecord {
    value: Value,
    fields: ProtocolFields,
}

#[derive(Debug)]
enum ProtocolFields {
    Circle(Option<CircleRecord>),
    Conic(Box<ConicData>),
    Arc(Box<ArcData>),
    Path(PathState),
    Vertex,
    ListExpression(ExprDef, Box<expr::ListRecords>),
    InequalityVertex {
        pending: bool,
        history: Option<expr::inequality::Helpers>,
    },
}

enum Propagation<'a> {
    Live,
    Redefinition {
        refresh: DefinitionRefresh,
        rebuild_records: bool,
    },
    Reload,
    RecordProtocol(&'a mut BTreeMap<usize, ProtocolRecord>),
    RestoreProtocol(&'a [usize]),
}

type CircleRecord = (f64, f64, f64);

/// [`Construction::rollback`] refused a checkpoint: the construction was
/// rolled back since it was taken, or it is longer than the construction
/// (it belongs to another one). Nothing was changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaleCheckpoint;

impl fmt::Display for StaleCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the checkpoint is stale: the construction was rolled back since it was taken"
        )
    }
}

impl std::error::Error for StaleCheckpoint {}

/// A definition's place in the construction protocol. An output stores
/// its representative's def index, and follows its position when it moves.
#[derive(Clone, Copy)]
enum ProtocolEntry {
    Hidden,
    Element(u32),
    Output(u32),
}

/// The latest entry of a pair is live. Earlier entries retain the state
/// needed when the outputs added after them are rolled back.
#[derive(Clone, Debug)]
struct LineConicOutput {
    owner: usize,
    key: (Id, Id),
    prior: Option<usize>,
    state: LineConicState,
}

#[derive(Clone, Copy)]
struct TangentOutput {
    owner: usize,
    helper: usize,
    state: TangentState,
}

/// The branch history of a line/conic intersection or tangent output.
#[derive(Clone, Copy, Debug)]
pub struct IntersectionState(IntersectionHistory);

#[derive(Clone, Copy, Debug)]
enum IntersectionHistory {
    LineConic(LineConicDef, LineConicState),
    Tangent(TangentDef, TangentState),
}

impl IntersectionState {
    /// Whether the states describe the same output of the same operands.
    pub fn same_definition(self, other: Self) -> bool {
        match (self.0, other.0) {
            (IntersectionHistory::LineConic(a, _), IntersectionHistory::LineConic(b, _)) => {
                a.key() == b.key() && a.output == b.output
            }
            (IntersectionHistory::Tangent(a, _), IntersectionHistory::Tangent(b, _)) => {
                a.same_algorithm(b) && a.which == b.which
            }
            _ => false,
        }
    }

    /// Whether both states assign the same geometric branches to the slots.
    pub fn same_branch(self, other: Self) -> bool {
        match (self.0, other.0) {
            (IntersectionHistory::LineConic(_, a), IntersectionHistory::LineConic(_, b)) => {
                a.same_branch(b)
            }
            (IntersectionHistory::Tangent(_, a), IntersectionHistory::Tangent(_, b)) => {
                a.same_branch(b)
            }
            _ => false,
        }
    }
}

/// A geometric construction.
#[derive(Default)]
pub struct Construction {
    defs: Vec<Def>,
    values: Vec<Value>,
    labels: Vec<String>,
    /// For each object, the objects that read it directly.
    children: Vec<Children>,
    /// Empty while creation order is also evaluation order.
    evaluation_order: EvaluationOrder,
    /// For each object, what its rule needs from the definitions of other
    /// objects, decided when it is created (see [`algo::intersect::Hint`]).
    hints: Vec<algo::intersect::Hint>,
    /// Permutations restored from the positions of existing intersections
    /// when the construction is rebuilt by a redefinition.
    intersection_swaps: BTreeSet<(Id, Id)>,
    by_label: HashMap<String, Id>,
    replacement_label: Option<String>,
    images: image::Images,
    charts: std::collections::BTreeMap<Id, algo::charts::ChartData>,
    proofs: std::collections::BTreeMap<Id, algo::prover::ProofData>,
    proof_intersections: algo::prover::IntersectionTranslations,
    /// The line coefficients a conditional segment took from a selected
    /// line (`GeoSegment.set(GeoLine)` copies the coefficients and keeps the
    /// end points). Kept outside [`Value`] so that it stays 48 bytes; absent
    /// while the coefficients are those of the end points.
    segment_carriers: std::collections::BTreeMap<Id, (f64, f64, f64)>,
    /// The animations of the sliders, see [`Construction::animate`].
    animations: std::collections::BTreeMap<Id, animation::Animation>,
    /// Whether the animation timer runs (`AnimationManager.isRunning`); a
    /// playing slider moves only while it does.
    animation_running: bool,
    /// Objects commands asked to hide (see
    /// [`Construction::take_hidden_requests`]).
    hidden_requests: Vec<Id>,
    /// The frozen points of the pen strokes.
    pub(crate) strokes: std::collections::BTreeMap<Id, algo::stroke::StrokeData>,
    slider_styles: std::collections::BTreeMap<Id, control::SliderStyle>,
    checkbox_styles: std::collections::BTreeMap<Id, control::CheckboxStyle>,
    slider_labels: BTreeMap<Id, control::SliderLabel>,
    number_rounding: control::NumberRounding,
    free_booleans: Vec<Id>,
    drawing_styles: std::collections::BTreeMap<Id, DrawingStyle>,
    drawing_layers: u16,
    text_presentations: HashMap<Id, text::TextOverrides>,
    /// Selected equation forms and their display revisions.
    equation_forms: HashMap<Id, (EquationForm, u64)>,
    /// Original expressions whose structure is lost when bound to a value.
    construction_sources: HashMap<Id, String>,
    /// The stored lists of parents, one after the other (see [`IdList`]).
    lists: Vec<Id>,
    /// For each family of generated labels (see [`Construction::label_hint`]),
    /// the first index that may still be free. Behind a mutex because the
    /// label searches only borrow the construction.
    label_hints: std::sync::Mutex<HashMap<String, usize>>,
    // Scratch state for updates, kept to avoid allocating on every drag.
    dirty: DirtySet,
    /// Trees of the expression objects.
    pub(crate) exprs: expr::Table,
    /// How many times [`Construction::rollback`] has run: a checkpoint taken
    /// before the last rollback is stale. Changed by nothing else.
    generation: u64,
    protocol_history: Mutex<Weak<ProtocolHistory>>,
    /// The conic records of the [`Def::GeneralConic`] objects, with the
    /// index of each object, in creation order.
    conics: Vec<(usize, ConicData)>,
    /// The output permutation and previous positions of conic intersections.
    conic_intersections: Vec<(usize, PairState)>,
    line_conic_outputs: Vec<LineConicOutput>,
    tangent_outputs: Vec<TangentOutput>,
    /// Most recently added output for each pair owns its solver state.
    line_conic_pairs: BTreeMap<(Id, Id), usize>,
    continuous_conic_intersections: bool,
    // Edges that must send an update even when their source value stays
    // equal, along the input graph of a circle intersected with itself.
    self_circle_updates: BTreeMap<Id, BTreeSet<Id>>,
    self_circle_outputs: Vec<Id>,
    pub(crate) function_roots: algo::intersect::functions::FunctionRootCache,
    pub(crate) inequality_helpers: expr::inequality::VertexHelpers,
    pub(crate) clips: algo::polygon_clip::ClipCache,
    pub(crate) analysis: AnalysisCache,
    last_update_requires_replay: bool,
    /// The states of the points on paths, with the index of each point, in
    /// creation order.
    paths: Vec<(usize, PathState)>,
    /// The last circle written by a circle command whose setter now
    /// returns before writing (see [`algo::circle::keeps_record`]), by
    /// owner. Used by the point optimizer and readers of the conic record.
    circle_records: Vec<(usize, CircleRecord)>,
    rigid_replacements: Vec<(Id, Id, Def)>,
    /// Helper intersection states for common tangents starting on an arc.
    limited_common_tangents: Vec<(usize, CommonTangentState)>,
    /// The records of the arcs and sectors, with the index of each object,
    /// in creation order.
    arcs: Vec<(usize, ArcData)>,
    /// The records of the [`Def::Equation`] objects, with the index of each
    /// object, in creation order (see [`EquationRecord`]).
    pub(crate) equations: Vec<(usize, EquationRecord)>,
    pub(crate) implicit_curves: Vec<(usize, expr::equation::implicit::Record)>,
    /// The points of the loci, with the index of each object, in creation
    /// order.
    loci: Vec<(usize, LocusData)>,
    /// The calculus records: the tree or list elements of the calculus
    /// objects that keep one (`algo::calculus`), by owner index.
    calculus: Vec<(usize, CalcRecord)>,
    /// The graphics window used to sample loci. A headless construction
    /// uses the reference window.
    locus_view: Option<[f64; 6]>,
    /// The graphics window the view-dependent objects read (`Corner`,
    /// `AxisStepX`, `AxisStepY`); the app keeps it current, a headless
    /// construction uses the reference window.
    view_state: Option<[f64; 6]>,
    /// Definitions that read the graphics window, in creation order.
    view_readers: Vec<usize>,
    /// The number of construction elements through the current step.
    step: usize,
    /// The def indices of the construction elements (`ceList`) in protocol
    /// order. A polygon and its sides share one construction element.
    elements: Vec<u32>,
    /// Membership in `elements` for each def.
    element_of: Vec<ProtocolEntry>,
    /// The indices of the objects that read the construction step, so that
    /// adding an object or `SetConstructionStep` can recompute them.
    protocol: Vec<usize>,
    /// Line starts installed by the path mover, with their insertion order
    /// so a rollback can remove starts added to pre-existing lines. The
    /// `Option<Id>` is the locus that installed the start, so a rebuild that
    /// drops the locus knows which starts it has to place itself; a start a
    /// rebuild installed carries no locus.
    pub(crate) frozen_line_starts: HashMap<Id, (f64, f64, usize, Option<Id>)>,
    /// The intervals of the free numbers made by `Slider`, with the index of
    /// each number, in creation order.
    sliders: Vec<(usize, f64, f64)>,
    saved_slider_bounds: HashMap<Id, [Option<slider::Bound>; 2]>,
    slider_default_slots: BTreeMap<Id, usize>,
    controls: Vec<(Id, control::Control)>,
    pub(crate) object_scripts: HashMap<String, Vec<control::ControlScript>>,
}

// Small adjacency lists are cheap to iterate and append to. Large families
// use a tree so moving a child to another parent costs logarithmic work.
#[derive(Clone, Debug, PartialEq)]
enum Children {
    Small(Vec<Id>),
    Large(BTreeSet<Id>),
}

impl Default for Children {
    fn default() -> Self {
        Self::Small(Vec::new())
    }
}

impl Children {
    fn iter(&self) -> impl DoubleEndedIterator<Item = Id> + '_ {
        let (small, large) = match self {
            Self::Small(ids) => (ids.as_slice(), None),
            Self::Large(ids) => (&[][..], Some(ids.iter())),
        };
        small.iter().chain(large.into_iter().flatten()).copied()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        match self {
            Self::Small(ids) => ids.len(),
            Self::Large(ids) => ids.len(),
        }
    }

    fn insert(&mut self, id: Id) {
        const SMALL_LIMIT: usize = 64;
        match self {
            Self::Small(ids) => {
                if ids.last().is_none_or(|&last| last < id) {
                    ids.push(id);
                } else if let Err(slot) = ids.binary_search(&id) {
                    ids.insert(slot, id);
                }
                if ids.len() > SMALL_LIMIT {
                    *self = Self::Large(ids.iter().copied().collect());
                }
            }
            Self::Large(ids) => {
                ids.insert(id);
            }
        }
    }

    fn remove(&mut self, id: Id) {
        match self {
            Self::Small(ids) => {
                if let Ok(slot) = ids.binary_search(&id) {
                    ids.remove(slot);
                }
            }
            Self::Large(ids) => {
                ids.remove(&id);
            }
        }
    }
}

#[derive(Default)]
struct EvaluationOrder {
    ids: Vec<usize>,
    ranks: Vec<usize>,
}

const MAX_REDEFINITION_EDGES: usize = 1_000_000;

impl EvaluationOrder {
    fn discard_since(&mut self, len: usize) {
        if self
            .ids
            .get(len..)
            .is_some_and(|tail| tail.iter().any(|&id| id < len))
        {
            self.ids.retain(|&id| id < len);
            for (rank, &id) in self.ids.iter().enumerate() {
                self.ranks[id] = rank;
            }
        } else {
            self.ids.truncate(len);
        }
        self.ranks.truncate(len);
    }
}

enum DefinitionRefresh {
    All,
    Selected(DirtySet),
}

/// The objects waiting to be recomputed during an update; empty between
/// updates. `levels[0]` has one bit per object. Each further level has one
/// bit per word of the level below, set whenever that word is not zero, and
/// the last level is a single word. Bits above level 0 are cleared lazily:
/// one may stay set for a word that has become zero, until a search meets
/// it. Finding the next dirty object then costs a few reads per level
/// however far away it is, and a drag with few dirty objects touches little
/// more than level 0.
#[derive(Clone, Default)]
struct DirtySet {
    levels: Vec<Vec<u64>>,
    /// How many objects are in the set.
    len: usize,
}

impl DirtySet {
    /// Makes room for `n` objects. A construction only grows, and level 0 is
    /// zero between updates, so new words are clean; when a level has to be
    /// added the upper levels start over from zero.
    fn fit(&mut self, n: usize) {
        let words = n.div_ceil(64).max(1);
        if self.levels.first().map_or(0, Vec::len) == words {
            return;
        }
        let mut sizes = vec![words];
        while sizes[sizes.len() - 1] > 1 {
            sizes.push(sizes[sizes.len() - 1].div_ceil(64));
        }
        if sizes.len() == self.levels.len() {
            for (level, size) in self.levels.iter_mut().zip(sizes) {
                level.resize(size, 0);
            }
        } else {
            self.levels = sizes.into_iter().map(|size| vec![0; size]).collect();
        }
    }

    fn contains(&self, i: usize) -> bool {
        self.levels[0][i / 64] & (1 << (i % 64)) != 0
    }

    fn insert(&mut self, i: usize) {
        let word = &mut self.levels[0][i / 64];
        let bit = 1 << (i % 64);
        if *word & bit != 0 {
            return;
        }
        let was = *word;
        *word |= bit;
        self.len += 1;
        if was != 0 {
            // the levels above already mark this word
            return;
        }
        let mut pos = i / 64;
        for level in &mut self.levels[1..] {
            let word = &mut level[pos / 64];
            let was = *word;
            *word |= 1 << (pos % 64);
            if was != 0 {
                break;
            }
            pos /= 64;
        }
    }

    /// Removes and returns the smallest index in the set, given that none
    /// is below `from`.
    fn pop_first(&mut self, from: usize) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        // `pos` is a position at `level`. A dirty object at or after `from`
        // exists and is marked on every level, so the search ends before it
        // runs past the end of a level.
        let mut level = 0;
        let mut pos = from;
        loop {
            let bits = self.levels[level][pos / 64] & (u64::MAX << (pos % 64));
            if bits == 0 {
                // nothing more in this word: go on from the next word
                level += 1;
                pos = pos / 64 + 1;
                continue;
            }
            pos = pos / 64 * 64 + bits.trailing_zeros() as usize;
            if level == 0 {
                break;
            }
            if self.levels[level - 1][pos] == 0 {
                // a stale mark: clear it and look further on this level
                self.levels[level][pos / 64] &= !(1 << (pos % 64));
                continue;
            }
            level -= 1;
            pos *= 64;
        }
        self.levels[0][pos / 64] &= !(1 << (pos % 64));
        self.len -= 1;
        Some(pos)
    }
}

/// Display properties stored outside the construction's fixed-size values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawingStyle {
    pub layer: u8,
    pub show_label: bool,
    pub line_thickness: u8,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            layer: 0,
            show_label: true,
            line_thickness: 3,
        }
    }
}

impl Construction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub(crate) fn definition_revision(&self) -> u64 {
        self.generation
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// All objects in creation order, excluding the built-in axes.
    pub fn ids(&self) -> impl Iterator<Item = Id> {
        self.ids_from(0)
    }

    /// Objects appended from a storage index, in creation order.
    pub fn ids_from(&self, from: usize) -> impl Iterator<Item = Id> {
        (from..self.defs.len()).map(Id)
    }

    pub fn id(&self, label: &str) -> Option<Id> {
        self.by_label
            .get(label)
            .copied()
            .or_else(|| axis::lookup(label))
    }

    pub(crate) fn resolve_label(&self, label: &str) -> Option<Id> {
        self.id(label).or_else(|| {
            let (base, subscript) = label.rsplit_once('_')?;
            let alternate = if let Some(index) = subscript
                .strip_prefix('{')
                .and_then(|index| index.strip_suffix('}'))
            {
                if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                format!("{base}_{index}")
            } else {
                if subscript.is_empty() || !subscript.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                format!("{base}_{{{subscript}}}")
            };
            self.id(&alternate)
        })
    }

    pub fn label(&self, id: Id) -> &str {
        self.labels
            .get(id.0)
            .map_or_else(|| axis::get(id).expect("object id").label, String::as_str)
    }

    pub fn def(&self, id: Id) -> Def {
        let def = self
            .defs
            .get(id.0)
            .copied()
            .unwrap_or_else(|| axis::get(id).expect("object id").def);
        match def {
            Def::IntersectConics(d) => {
                let slot = self
                    .conic_intersection_slot(id.0)
                    .expect("conic intersection state");
                Def::IntersectConics(d.with_state(&self.conic_intersections[slot].1))
            }
            Def::IntersectLineConic(d) => {
                let slot = self.line_conic_slot(id.0).expect("line/conic output");
                let mut state = self.line_conic_outputs[self.line_conic_pairs[&d.key()]].state;
                state.set_nearest(self.line_conic_outputs[slot].state.nearest());
                Def::IntersectLineConic(d.with_state(&state))
            }
            Def::Lines(LineDef::CommonTangent(d)) => {
                self.common_tangent_slot(id.0).map_or(def, |slot| {
                    Def::Lines(LineDef::CommonTangent(
                        d.with_state(&self.limited_common_tangents[slot].1),
                    ))
                })
            }
            _ => def,
        }
    }

    pub fn kind(&self, id: Id) -> Kind {
        match self.def(id) {
            Def::ImplicitCurve(index) => self.implicit_curves[index as usize].1.kind,
            def => def.kind(),
        }
    }

    /// Whether `id` names an object of this construction, an axis included.
    /// An id of a deleted object (the construction shrank, or a rebuild
    /// renumbered everything after it) is not one, and the accessors above
    /// panic on it.
    pub fn exists(&self, id: Id) -> bool {
        id.0 < self.defs.len() || axis::get(id).is_some()
    }

    pub fn value(&self, id: Id) -> Value {
        self.stored_value(id).cartesian()
    }

    /// An object's selected equation form, if it has one.
    pub fn equation_form(&self, id: Id) -> Option<&EquationForm> {
        self.equation_forms.get(&id).map(|(form, _)| form)
    }

    /// A cache key for the equation's display state within this construction.
    pub fn equation_display_revision(&self, id: Id) -> Option<u64> {
        self.equation_forms.get(&id).map(|(_, revision)| *revision)
    }

    /// Changes the display form and refreshes texts that read it.
    pub fn set_equation_form(&mut self, id: Id, form: EquationForm) -> Result<usize, Error> {
        const EQUATIONS: &[Kind] = &[
            Kind::Line,
            Kind::Ray,
            Kind::Circle,
            Kind::Conic,
            Kind::ImplicitCurve,
        ];
        let kind = self.defs.get(id.0).ok_or(Error::UnknownObject)?.kind();
        if !EQUATIONS.contains(&kind) {
            return Err(Error::WrongKind {
                label: self.label(id).to_owned(),
                expected: EQUATIONS,
                found: kind,
            });
        }
        let revision = match self.equation_forms.get(&id) {
            Some((previous, _)) if *previous == form => return Ok(0),
            Some((_, revision)) => revision.wrapping_add(1),
            None => 0,
        };
        self.equation_forms.insert(id, (form, revision));
        if self.children[id.0].is_empty() {
            return Ok(0);
        }
        Ok(self.propagate(id))
    }

    /// Includes homogeneous coordinates so algorithms that read the raw
    /// fields see scale changes even when the Cartesian value is unchanged.
    pub fn stored_value(&self, id: Id) -> Value {
        self.values
            .get(id.0)
            .copied()
            .unwrap_or_else(|| axis::get(id).expect("object id").value)
    }

    /// The interval of an angle, including an unbounded numeric angle.
    /// The coordinate style a point or vector prints in, set by
    /// `ToComplex`, `ToPolar` and `ToPoint` (`Kernel.COORD_*`).
    pub fn coordinate_mode(&self, id: Id) -> Option<crate::algo::point::CoordMode> {
        match self.def(id) {
            Def::Point(PointDef::Convert { mode, .. })
            | Def::Vector(VectorDef::Convert { mode, .. }) => Some(mode),
            // `Product` of points over a range multiplies them as complex
            // numbers and shows the result as one (`AlgoFoldExpression`)
            Def::Expr(d) if expr::is_coordinate_product(self, d) => {
                Some(crate::algo::point::CoordMode::Complex)
            }
            _ => None,
        }
    }

    pub fn angle_style(&self, id: Id) -> Option<AngleStyle> {
        match *self.defs.get(id.0)? {
            Def::AngleObject(d) if d.kind() == Kind::Angle => Some(d.style),
            Def::Angle(_) => Some(AngleStyle::Unbounded),
            Def::Expr(d) if d.kind() == Kind::Angle => Some(d.angle_style),
            Def::Measure(MeasureDef::Angle(..)) | Def::Polygon(PolygonDef::Angle { .. }) => {
                Some(AngleStyle::Anticlockwise)
            }
            _ => None,
        }
    }

    fn angle_source(&self, id: Id) -> Option<AngleSource> {
        match *self.defs.get(id.0)? {
            Def::Angle(v) => Some(AngleSource::Free(v)),
            Def::AngleObject(d) if d.kind() == Kind::Angle => Some(d.source),
            Def::Measure(MeasureDef::Angle(a, b, d)) => Some(AngleSource::Points(a, b, d)),
            Def::Polygon(PolygonDef::Angle { poly, k, internal }) => Some(AngleSource::Polygon {
                poly,
                index: k,
                interior: internal,
            }),
            _ => None,
        }
    }

    /// The drawable sector of an angle with a finite vertex and nonzero sweep.
    pub fn angle_geometry(&self, id: Id) -> Option<AngleGeometry> {
        let source = self.angle_source(id)?;
        source.drawing(self, self.number(id)?, self.angle_style(id)?)
    }

    /// Change an angle's interval and update objects that
    /// use it. Its source and raw sweep are preserved.
    pub fn set_angle_style(&mut self, id: Id, style: AngleStyle) -> Result<usize, Error> {
        let old = self.angle_style(id).ok_or(Error::UnknownObject)?;
        if old == style {
            return Ok(0);
        }
        if let Def::Expr(mut d) = self.def(id) {
            d.angle_style = style;
            self.defs[id.0] = Def::Expr(d);
            self.values[id.0] = expr::value(d, expr::raw(self, d));
            return Ok(self.propagate(id));
        }
        let source = self
            .angle_source(id)
            .ok_or_else(|| Error::NotFree(self.label(id).to_owned()))?;
        if style == AngleStyle::Unbounded && source.has_geometry() {
            return Err(Error::NotFree(self.label(id).to_owned()));
        }
        let def = Def::AngleObject(AngleDef { source, style });
        self.defs[id.0] = def;
        self.values[id.0] = self.compute(def, id.0);
        Ok(self.propagate(id))
    }

    /// `±∞` when `id` is an expression number whose value is infinite.
    /// GeoGebra counts such a number as defined: [`Self::value`] reports
    /// it as `Value::Number` of that infinity, and expressions that read
    /// it use the infinite value (`1 / (r - a)` with `a = ∞` is `-0`).
    pub fn infinite_number(&self, id: Id) -> Option<f64> {
        match self.defs.get(id.0)? {
            &Def::Expr(d) if d.kind().is_number() && d.angle_style == AngleStyle::Unbounded => {
                match expr::raw(self, d) {
                    expr::Raw::Num(v) if v.is_infinite() => Some(v),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The coordinates of the expression vector `id` when one of them is
    /// infinite and neither is NaN. These coordinates are also kept in
    /// [`Self::value`], as they are defined in GeoGebra.
    pub fn infinite_vector(&self, id: Id) -> Option<(f64, f64)> {
        match self.defs.get(id.0)? {
            &Def::Expr(d) if d.kind() == Kind::Vector => match expr::raw(self, d) {
                expr::Raw::Vec(x, y)
                    if (x.is_infinite() || y.is_infinite()) && !(x.is_nan() || y.is_nan()) =>
                {
                    Some((x, y))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The current value of a numeric expression over the objects of the
    /// construction, which stays unchanged. `None` when the text is not such
    /// an expression.
    pub fn numeric_value(&self, source: &str) -> Option<f64> {
        expr::argument_number(self, source)
    }

    /// Whether the kernel evaluates `source` as numeric arithmetic over the
    /// objects of the construction, and its current value: `None` when the
    /// text is no such expression or holds anything the kernel parses
    /// without evaluating it (a list, a text, a command of the computer
    /// algebra code); otherwise `Some`, with `NaN` while an input is
    /// undefined.
    pub fn arithmetic_value(&self, source: &str) -> Option<f64> {
        expr::arithmetic_number(self, source)
    }

    /// The value of the function `id` at `x`, evaluated as GeoGebra's
    /// `GeoFunction.value` does; it may be infinite or `NaN` where the
    /// function is not defined. `None` when `id` is not a defined function.
    pub fn function_value(&self, id: Id, x: f64) -> Option<f64> {
        expr::function_value(self, id, x)
    }

    /// Every object `id` reads directly: its parents, the objects its rule
    /// or expression also reads and its list (wider than `inputs`). An
    /// object may appear more than once. A drawing that caches what it made
    /// from `id` follows these to tell whether `id` can have changed.
    pub fn reads(&self, id: Id) -> Vec<Id> {
        match self.defs.get(id.0) {
            Some(def) => {
                let mut ids = self.read_ids(def);
                ids.extend(self.saved_text_anchor(id));
                ids
            }
            None => Vec::new(),
        }
    }

    /// The result of a `Prove` or `ProveDetails` object.
    pub fn proof(&self, id: Id) -> Option<&algo::prover::ProofData> {
        self.proofs.get(&id)
    }

    pub fn chart(&self, id: Id) -> Option<&algo::charts::ChartData> {
        self.charts.get(&id)
    }

    pub fn charts(&self) -> impl ExactSizeIterator<Item = (Id, &algo::charts::ChartData)> {
        self.charts.iter().map(|(&id, data)| (id, data))
    }

    pub fn drawing_style(&self, id: Id) -> DrawingStyle {
        self.drawing_styles.get(&id).copied().unwrap_or_else(|| {
            let mut style = DrawingStyle::default();
            if self.exists(id) && self.kind(id) == Kind::PieChart {
                style.show_label = false;
                style.line_thickness = 0;
            }
            style
        })
    }

    pub fn set_drawing_style(&mut self, id: Id, mut style: DrawingStyle) {
        style.layer = style.layer.min(9);
        self.drawing_layers |= 1 << style.layer;
        self.drawing_styles.insert(id, style);
    }

    pub fn drawing_layers(&self) -> impl Iterator<Item = u8> {
        let layers = self.drawing_layers | 1;
        (0..=9).filter(move |layer| layers & (1 << layer) != 0)
    }

    /// Classic drawing order: layer, object-class priority, construction order.
    pub fn drawing_order(&self, id: Id) -> (u8, u8, Id) {
        let priority = match self.kind(id) {
            Kind::Image => 20,
            Kind::List => 40,
            Kind::Polygon => 50,
            Kind::PolyLine => 51,
            Kind::Circle | Kind::Conic | Kind::Arc | Kind::Sector => 70,
            Kind::Number | Kind::Angle | Kind::PieChart => 80,
            Kind::Function | Kind::Curve => 90,
            Kind::Line => 100,
            Kind::Ray | Kind::Segment => 110,
            Kind::Vector => 120,
            Kind::Locus => 130,
            Kind::Point => 140,
            _ => 150,
        };
        (self.drawing_style(id).layer, priority, id)
    }

    /// The vertices of a polygon or polyline in order, `None` for a vertex
    /// that is undefined. Nothing for any other object, and nothing for a
    /// regular polygon that is undefined.
    pub fn vertices(&self, id: Id) -> algo::polygon::Vertices<'_> {
        algo::polygon::Vertices::of(self, id)
    }

    /// How many ids the stored lists hold together.
    #[cfg(test)]
    pub(crate) fn stored_ids(&self) -> usize {
        self.lists.len()
    }

    /// The objects computed from `id`.
    pub(crate) fn children(&self, id: Id) -> impl DoubleEndedIterator<Item = Id> + '_ {
        self.children.get(id.0).into_iter().flat_map(Children::iter)
    }

    pub(crate) fn children_slice(&self, id: Id) -> Option<&[Id]> {
        match self.children.get(id.0)? {
            Children::Small(ids) => Some(ids),
            Children::Large(_) => None,
        }
    }

    pub(crate) fn child_count(&self, id: Id) -> usize {
        self.children.get(id.0).map_or(0, Children::len)
    }

    /// The IDs whose current rules read this object.
    pub fn direct_dependents(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        self.children(id)
    }

    /// Checks a definition and computes the value it would have if it were
    /// added now, without adding it.
    pub(crate) fn evaluate(&self, def: Def) -> Result<Value, Error> {
        self.check(&def)?;
        let def = algo::intersect::prepare(self, def);
        let hint = algo::intersect::hint(self, &def);
        Ok(self.compute_with(def, |_| hint).cartesian())
    }

    fn record_proof(&mut self, id: Id, def: algo::prover::ProveDef) -> Value {
        let mut intersections = std::mem::take(&mut self.proof_intersections);
        let data = def.record(self, &mut intersections);
        self.proof_intersections = intersections;
        let value = data.value;
        self.proofs.insert(id, data);
        value
    }

    pub(crate) fn rewrite_proof_statement(&mut self, statement: Id) -> bool {
        if !matches!(self.def(statement), Def::Expr(_)) {
            return false;
        }
        let mut intersections = std::mem::take(&mut self.proof_intersections);
        let can_rewrite = algo::prover::can_rewrite_statement(self, statement, &mut intersections);
        self.proof_intersections = intersections;
        can_rewrite && expr::replace_distance_operands(self, statement)
    }

    /// The current length of the construction and of its lists, to return to
    /// with [`Construction::rollback`].
    ///
    /// The contract: a checkpoint can be rolled back to as long as no
    /// rollback has run since it was taken, whatever was added in between.
    /// Any rollback, to this checkpoint or to another one, makes every
    /// checkpoint taken before it stale, even one that it did not cut (a
    /// later rollback to an earlier checkpoint is refused too): take a new
    /// checkpoint after each rollback. A checkpoint belongs to the
    /// construction it was taken from.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            len: self.defs.len(),
            lists: self.lists.len(),
            exprs: self.exprs.len(),
            frozen_line_starts: self.frozen_line_starts.len(),
            proof_intersections: self.proof_intersections.checkpoint(),
            image_assets: self.images.asset_count(),
            step: self.step,
            generation: self.generation,
            protocol: self.protocol_checkpoint(),
        }
    }

    fn protocol_checkpoint(&self) -> Option<ProtocolCheckpoint> {
        if self
            .protocol
            .iter()
            .all(|&i| self.children(Id(i)).next().is_none())
        {
            return None;
        }
        let mut reference = self
            .protocol_history
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let history = reference.upgrade().unwrap_or_else(|| {
            let history = Arc::new(ProtocolHistory::default());
            *reference = Arc::downgrade(&history);
            history
        });
        let mut frames = history.lock().unwrap_or_else(|e| e.into_inner());
        ProtocolFrame::merge_unused_tail(&mut frames);
        let live = Arc::new(());
        let frame = frames.len();
        frames.push(ProtocolFrame {
            live: Arc::downgrade(&live),
            before: BTreeMap::new(),
        });
        drop(frames);
        Some(ProtocolCheckpoint {
            history,
            frame,
            _live: live,
        })
    }

    fn protocol_record(&self, i: usize) -> Option<ProtocolRecord> {
        let fields = match self.defs[i] {
            Def::Expr(d) if expr::list_record_revision(self, d).is_some() => {
                ProtocolFields::ListExpression(d, Box::new(expr::list_records(self, d)))
            }
            Def::GeneralConic(_) | Def::Equation(_) => {
                ProtocolFields::Conic(Box::new(self.conics[self.conic_slot(i)?].1))
            }
            Def::Arc(_) => ProtocolFields::Arc(Box::new(self.arcs[self.arc_slot(i)?].1)),
            Def::Path(_) => ProtocolFields::Path(self.paths[self.path_slot(i)?].1),
            Def::Polygon(PolygonDef::RegularVertex { .. }) => ProtocolFields::Vertex,
            Def::Point(PointDef::InequalityVertex { owner, .. }) => {
                ProtocolFields::InequalityVertex {
                    pending: self.inequality_helpers.pending.contains(&Id(i)),
                    history: (owner == Id(i))
                        .then(|| self.inequality_helpers.history(owner))
                        .flatten(),
                }
            }
            def if def.kind() == Kind::Circle => {
                let circle = self
                    .circle_records
                    .binary_search_by_key(&i, |&(owner, _)| owner)
                    .ok()
                    .map(|slot| self.circle_records[slot].1);
                ProtocolFields::Circle(circle)
            }
            _ => return None,
        };
        Some(ProtocolRecord {
            value: self.values[i],
            fields,
        })
    }

    /// Where to start searching a family of generated labels (`c0, c1, ...`,
    /// `p_{1}, p_{2}, ...`): every index below it is taken by an object of
    /// the construction (or reserved), so the search for the first free label
    /// gives the same result as one that starts at the beginning, without
    /// scanning the labels again for every new object.
    pub(crate) fn label_hint(&self, family: &str) -> usize {
        let hints = self.label_hints.lock().unwrap_or_else(|e| e.into_inner());
        hints.get(family).copied().unwrap_or(0)
    }

    /// Records that every index below `start` of `family` is taken.
    pub(crate) fn set_label_hint(&self, family: &str, start: usize) {
        let mut hints = self.label_hints.lock().unwrap_or_else(|e| e.into_inner());
        let slot = hints.entry(family.to_string()).or_insert(0);
        *slot = (*slot).max(start);
    }

    /// Removes every object added after `at`, newest first, together with
    /// the edges from its parents, the lists and the expression entries
    /// stored since. No object may have been moved in between.
    ///
    /// `at` must have been taken from this construction after its last
    /// rollback (see [`Construction::checkpoint`]); otherwise the
    /// construction is left exactly as it is and [`StaleCheckpoint`] is
    /// returned. A rollback that succeeds makes every checkpoint taken so far
    /// stale, `at` included. Checking costs four comparisons; adding objects
    /// and updating them pay nothing for it.
    pub fn rollback(&mut self, at: Checkpoint) -> Result<(), StaleCheckpoint> {
        if at.generation != self.generation
            || at.len > self.defs.len()
            || at.lists > self.lists.len()
            || at.exprs > self.exprs.len()
        {
            return Err(StaleCheckpoint);
        }
        self.discard_since(at);
        self.invalidate_checkpoints();
        Ok(())
    }

    /// The work of [`Construction::rollback`], for an all-or-nothing step
    /// inside one call of the kernel (a command that makes several objects):
    /// `at` was taken in that call, so every checkpoint the caller holds lies
    /// at or before it and stays valid, and the generation does not change.
    pub(crate) fn discard_since(&mut self, at: Checkpoint) {
        debug_assert_eq!(at.generation, self.generation, "a checkpoint of this call");
        self.controls
            .truncate(self.controls.partition_point(|(id, _)| id.0 < at.len));
        self.saved_slider_bounds.retain(|id, bounds| {
            let keep = id.0 < at.len
                && bounds
                    .iter()
                    .flatten()
                    .all(|bound| bound.exists_before(at.len));
            if !keep {
                if self
                    .sliders
                    .binary_search_by_key(&id.0, |&(owner, ..)| owner)
                    .is_err()
                {
                    self.slider_default_slots.remove(id);
                }
                for parent in bounds
                    .iter()
                    .flatten()
                    .filter_map(|bound| bound.dependency())
                {
                    self.children[parent.index()].remove(*id);
                }
            }
            keep
        });
        self.images.discard_since(at.len, at.image_assets);
        self.charts.split_off(&Id(at.len));
        self.proofs.split_off(&Id(at.len));
        self.proof_intersections.truncate(at.proof_intersections);
        self.segment_carriers.split_off(&Id(at.len));
        self.strokes.split_off(&Id(at.len));
        self.animations.split_off(&Id(at.len));
        if !self.animations.values().any(|a| a.playing) {
            self.animation_running = false;
        }
        self.hidden_requests.retain(|id| id.0 < at.len);
        self.slider_styles.split_off(&Id(at.len));
        self.slider_default_slots.split_off(&Id(at.len));
        self.slider_labels.split_off(&Id(at.len));
        self.checkbox_styles.split_off(&Id(at.len));
        self.free_booleans
            .truncate(self.free_booleans.partition_point(|id| id.0 < at.len));
        self.drawing_styles.split_off(&Id(at.len));
        self.equation_forms.retain(|id, _| id.0 < at.len);
        self.construction_sources.retain(|id, _| id.0 < at.len);
        self.frozen_line_starts
            .retain(|_, &mut (_, _, order, _)| order < at.frozen_line_starts);
        // A saved text start point is a dependency. Keep its presentation
        // until its edge has been unregistered, as in GeoText.doRemove.
        for id in (at.len..self.defs.len()).rev().map(Id) {
            for parent in self.reads(id) {
                if parent.0 < at.len {
                    self.children[parent.0].remove(id);
                }
            }
        }
        self.discard_text_presentations(at.len);
        let mut first_removed = self.elements.len();
        while self.defs.len() > at.len {
            self.defs.pop();
            self.values.pop();
            self.children.pop();
            self.hints.pop();
            if let Some(ProtocolEntry::Element(index)) = self.element_of.pop() {
                first_removed = first_removed.min(index as usize);
            }
            if let Some(label) = self.labels.pop() {
                self.by_label.remove(&label);
                self.object_scripts.remove(&label);
            }
        }
        self.evaluation_order.discard_since(at.len);
        if self
            .self_circle_outputs
            .last()
            .is_some_and(|id| id.0 >= at.len)
        {
            self.self_circle_outputs.retain(|id| id.0 < at.len);
            self.rebuild_self_circle_updates();
        }
        // A removed object may have opened an element before a surviving
        // one if it was created after navigating back. Normally only the
        // tail changes, so a brief probe need not scan the whole protocol.
        let elements_changed = first_removed < self.elements.len();
        if elements_changed {
            if self.elements[first_removed..]
                .iter()
                .all(|&id| id as usize >= at.len)
            {
                self.elements.truncate(first_removed);
            } else {
                self.elements.retain(|&id| (id as usize) < at.len);
                self.reindex_elements_from(first_removed);
            }
        }
        let step = at.step.min(self.elements.len());
        let step_changed = self.step != step;
        self.step = step;
        self.protocol
            .truncate(self.protocol.partition_point(|&i| i < at.len));
        self.view_readers
            .truncate(self.view_readers.partition_point(|&i| i < at.len));
        self.lists.truncate(at.lists.min(self.lists.len()));
        self.exprs.truncate(at.exprs, at.len);
        self.inequality_helpers.truncate(at.len);
        self.function_roots.clear();
        self.clips.clear();
        self.analysis.clear();
        self.analysis.truncate(at.len);
        self.conic_intersections.truncate(
            self.conic_intersections
                .partition_point(|&(owner, _)| owner < at.len),
        );
        self.tangent_outputs.truncate(
            self.tangent_outputs
                .partition_point(|output| output.owner < at.len),
        );
        while self
            .line_conic_outputs
            .last()
            .is_some_and(|output| output.owner >= at.len)
        {
            let output = self.line_conic_outputs.pop().expect("line/conic output");
            match output.prior {
                Some(prior) => {
                    self.line_conic_pairs.insert(output.key, prior);
                }
                None => {
                    self.line_conic_pairs.remove(&output.key);
                }
            }
        }
        while self
            .conics
            .last()
            .is_some_and(|&(owner, _)| owner >= at.len)
        {
            self.conics.pop();
        }
        while self.paths.last().is_some_and(|&(owner, _)| owner >= at.len) {
            self.paths.pop();
        }
        self.circle_records.truncate(
            self.circle_records
                .partition_point(|&(owner, _)| owner < at.len),
        );
        while self.arcs.last().is_some_and(|&(owner, _)| owner >= at.len) {
            self.arcs.pop();
        }
        self.limited_common_tangents.truncate(
            self.limited_common_tangents
                .partition_point(|&(owner, _)| owner < at.len),
        );
        while self
            .equations
            .last()
            .is_some_and(|&(owner, _)| owner >= at.len)
        {
            self.equations.pop();
        }
        self.implicit_curves.truncate(
            self.implicit_curves
                .partition_point(|(owner, _)| *owner < at.len),
        );
        while self.loci.last().is_some_and(|&(owner, _)| owner >= at.len) {
            self.loci.pop();
        }
        while self
            .calculus
            .last()
            .is_some_and(|&(owner, _)| owner >= at.len)
        {
            self.calculus.pop();
        }
        while self
            .sliders
            .last()
            .is_some_and(|&(owner, _, _)| owner >= at.len)
        {
            self.sliders.pop();
        }
        while self
            .rigid_replacements
            .last()
            .is_some_and(|&(owner, _, _)| owner.0 >= at.len)
        {
            let (_, id, prior) = self.rigid_replacements.pop().expect("replacement");
            if id.0 >= at.len {
                continue;
            }
            let def = self.defs[id.0];
            for parent in def.parents().as_slice() {
                self.children[parent.0].remove(id);
            }
            if let Some(slot) = self.path_slot(id.0) {
                self.paths.remove(slot);
            }
            self.defs[id.0] = prior;
            self.values[id.0] = self.compute(prior, id.0);
            self.propagate(id);
        }
        // Labels became free again, so the hints may be too high.
        self.label_hints
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let records = at.protocol.map_or_else(BTreeMap::new, |checkpoint| {
            let mut frames = checkpoint.history.lock().unwrap_or_else(|e| e.into_inner());
            let mut records = std::mem::take(&mut frames[checkpoint.frame].before);
            for frame in frames.drain(checkpoint.frame + 1..) {
                for (i, record) in frame.before {
                    records.entry(i).or_insert(record);
                }
            }
            records
        });
        if (step_changed || elements_changed || !records.is_empty()) && !self.protocol.is_empty() {
            let mut dirty = std::mem::take(&mut self.dirty);
            dirty.fit(self.len());
            let restored = self.restore_protocol_records(records, &mut dirty);
            self.queue_protocol_readers(&mut dirty);
            self.propagate_dirty_with(dirty, 0, Propagation::RestoreProtocol(&restored));
        }
    }

    fn restore_protocol_records(
        &mut self,
        records: BTreeMap<usize, ProtocolRecord>,
        dirty: &mut DirtySet,
    ) -> Vec<usize> {
        let mut restored = Vec::with_capacity(records.len());
        let len = self.len();
        for (i, record) in records.into_iter().take_while(|(i, _)| *i < len) {
            let recompute = matches!(record.fields, ProtocolFields::ListExpression(..));
            match record.fields {
                ProtocolFields::Circle(Some(circle)) => {
                    self.store_circle_record(i, circle);
                }
                ProtocolFields::Circle(None) => {
                    self.forget_circle_record(i);
                }
                ProtocolFields::Conic(conic) => {
                    if let Some(slot) = self.conic_slot(i) {
                        self.conics[slot].1 = *conic;
                    }
                }
                ProtocolFields::Arc(arc) => {
                    if let Some(slot) = self.arc_slot(i) {
                        self.arcs[slot].1 = *arc;
                    }
                }
                ProtocolFields::Path(path) => {
                    if let Some(slot) = self.path_slot(i) {
                        self.paths[slot].1 = path;
                    }
                }
                ProtocolFields::Vertex => {}
                ProtocolFields::ListExpression(def, fields) => {
                    // The expression must also rebuild its raw result and
                    // element slots from the restored helper records.
                    expr::restore_list_records(self, def, *fields);
                    dirty.insert(self.evaluation_rank(Id(i)));
                }
                ProtocolFields::InequalityVertex { pending, history } => {
                    let id = Id(i);
                    if matches!(self.def(id), Def::Point(PointDef::InequalityVertex { owner, .. }) if owner == id)
                    {
                        self.inequality_helpers.restore_history(id, history);
                    }
                    if pending {
                        self.inequality_helpers.pending.insert(id);
                    } else {
                        self.inequality_helpers.pending.remove(&id);
                    }
                }
            }
            self.values[i] = record.value;
            if !recompute {
                restored.push(i);
            }
            for child in self.children[i].iter() {
                dirty.insert(self.evaluation_rank(child));
            }
        }
        restored
    }

    fn invalidate_checkpoints(&mut self) {
        self.generation += 1;
        *self
            .protocol_history
            .get_mut()
            .unwrap_or_else(|e| e.into_inner()) = Weak::new();
    }

    /// Stages a single definition and installs it at the existing object ID.
    pub(crate) fn redefine(
        &mut self,
        id: Id,
        define: impl FnOnce(&mut Self) -> Result<Option<Id>, String>,
    ) -> Result<Option<Id>, String> {
        let label = self.label(id).to_owned();
        let refuse = |reason| format!("object `{label}` already exists: {reason}");
        let supported = |def| {
            matches!(
                def,
                Def::Number(_)
                    | Def::Boolean(_)
                    | Def::Angle(_)
                    | Def::FreePoint { .. }
                    | Def::Vector(VectorDef::Free { .. })
                    | Def::Expr(_)
                    | Def::Measure(MeasureDef::Slope(_))
                    | Def::Path(PathDef::On { .. } | PathDef::OnParam { .. })
            )
        };
        let old = self.def(id);
        let locus_inputs = self.locus_order_inputs().map_err(refuse)?;
        let dependents = self
            .replacement_dependents(id, &locus_inputs)
            .map_err(refuse)?;
        let old_reads: BTreeMap<_, _> = dependents
            .into_iter()
            .chain(std::iter::once(id))
            .map(|child| {
                let mut parents = self.reads(child);
                parents.sort_unstable();
                parents.dedup();
                (child, parents)
            })
            .collect();

        let checkpoint = self.checkpoint();
        let scripts = self.object_scripts.remove(&label);
        self.by_label.remove(&label);
        self.replacement_label = Some(label.clone());
        let result = define(self);

        let replacement = (|| {
            let new = result
                .map_err(|error| {
                    if error.contains(&format!("unknown object `{label}`")) {
                        refuse("a replacement cannot read itself")
                    } else {
                        error
                    }
                })?
                .ok_or_else(|| refuse("a replacement must define an object"))?;
            if !supported(old)
                || self.slider_interval(id).is_some()
                || self.saved_slider_bounds.contains_key(&id)
            {
                return Err(refuse("this definition cannot be replaced in place"));
            }
            if new.0 < checkpoint.len
                || self.len() != new.0 + 1
                || self.by_label.get(&label) != Some(&new)
                || self.labels[checkpoint.len..new.0]
                    .iter()
                    .any(|aux| !aux.starts_with('\0'))
            {
                return Err(refuse("a replacement must have one visible output"));
            }
            let def = self.defs[new.0];
            if !supported(def) {
                return Err(refuse("this replacement cannot be installed in place"));
            }
            if def.kind() != old.kind() && !self.children[id.0].is_empty() {
                return Err(refuse("a dependent object requires the original kind"));
            }
            let mut remaining = MAX_REDEFINITION_EDGES;
            let mut pending = vec![new];
            let mut seen = BTreeSet::new();
            while let Some(owner) = pending.pop() {
                for parent in self.reads(owner) {
                    remaining = remaining.checked_sub(1).ok_or_else(|| {
                        refuse("the dependency graph exceeds the redefinition budget")
                    })?;
                    if old_reads.contains_key(&parent) {
                        return Err(refuse("a replacement cannot introduce a dependency cycle"));
                    }
                    if parent.0 >= checkpoint.len && parent.0 < self.len() && seen.insert(parent) {
                        pending.push(parent);
                    }
                }
            }
            // Read the prospective definitions before committing. Reordering
            // must account for new indirect reads as well as the replacement's
            // own inputs, so a rejected cycle leaves the live graph intact.
            self.defs[id.0] = def;
            let changed: BTreeMap<_, _> = old_reads
                .iter()
                .filter_map(|(&child, old_reads)| {
                    let mut parents = self.reads(child);
                    parents.sort_unstable();
                    parents.dedup();
                    (parents != *old_reads).then_some((child, parents))
                })
                .collect();
            self.defs[id.0] = old;
            let order = changed
                .iter()
                .any(|(&child, parents)| {
                    parents.iter().any(|&parent| {
                        axis::get(parent).is_none()
                            && self.evaluation_rank(parent) >= self.evaluation_rank(child)
                    })
                })
                .then(|| self.replacement_order(&changed, &locus_inputs, new.0))
                .transpose()
                .map_err(refuse)?;
            Ok((new, def, changed, order))
        })();
        self.replacement_label = None;

        let (new, mut def, changed, order) = match replacement {
            Ok(staged) => staged,
            Err(error) => {
                self.discard_since(checkpoint);
                if let Some(scripts) = scripts {
                    self.object_scripts.insert(label.clone(), scripts);
                }
                self.by_label.insert(label, id);
                return Err(error);
            }
        };

        let rebuilds = !self.inequality_helpers.outputs().is_empty()
            && self.replacement_rebuilds_construction(id, def);

        let value = self.values[new.0];
        let hint = self.hints[new.0];
        let path = self.path_state(new);
        let mut source = self.construction_sources.remove(&new);
        if matches!(old, Def::Number(_))
            && matches!(def, Def::Expr(d) if d.kind() == Kind::Number && expr::undefined_literal(self, d))
        {
            def = Def::Number(f64::NAN);
            source = Some("?".into());
        }
        let decimal = expr::free_decimal(self, new).cloned();
        let mut kept = self.checkpoint();
        kept.len = new.0;
        kept.step = checkpoint.step;
        kept.protocol = checkpoint.protocol;
        self.discard_since(kept);
        if let Some(scripts) = scripts {
            self.object_scripts.insert(label.clone(), scripts);
        }
        self.by_label.insert(label, id);

        let slot = self.paths.partition_point(|(owner, _)| *owner < id.0);
        if self
            .paths
            .get(slot)
            .is_some_and(|(owner, _)| *owner == id.0)
        {
            self.paths.remove(slot);
        }
        if let Some(path) = path {
            self.paths.insert(slot, (id.0, path));
        }
        match (
            self.free_booleans.binary_search(&id),
            matches!(def, Def::Boolean(_)),
        ) {
            (Ok(index), false) => {
                self.free_booleans.remove(index);
            }
            (Err(index), true) => self.free_booleans.insert(index, id),
            _ => {}
        }
        // a redefined object is a new circle without a record
        self.forget_circle_record(id.0);
        self.defs[id.0] = def;
        self.values[id.0] = value;
        self.hints[id.0] = hint;
        for (child, new) in changed {
            let old = &old_reads[&child];
            for &parent in old {
                if new.binary_search(&parent).is_err() {
                    if let Some(children) = self.children.get_mut(parent.0) {
                        children.remove(child);
                    }
                }
            }
            for parent in new {
                if old.binary_search(&parent).is_err() {
                    if let Some(children) = self.children.get_mut(parent.0) {
                        children.insert(child);
                    }
                }
            }
        }
        expr::replace_free_decimal(self, id, decimal);
        self.construction_sources.remove(&id);
        if let Some(source) = source {
            self.remember_construction_source(id, &source);
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        if let Some(order) = order {
            self.evaluation_order = order;
            self.refresh_circle_hints(&mut dirty);
        }
        for child in self.children(id) {
            dirty.insert(self.evaluation_rank(child));
        }
        for child in old_reads.keys().copied() {
            if matches!(
                self.def(child),
                Def::Locus(LocusDef::Path { .. } | LocusDef::Slider { .. })
            ) {
                dirty.insert(self.evaluation_rank(child));
            }
        }
        // Same-kind replacements of independent values are assignments
        // in AlgebraProcessor. A change of definition rebuilds from XML.
        let rebuild_records = !(old.is_free() && def.is_free() && old.kind() == def.kind());
        if rebuild_records {
            for (i, &def) in self.defs.iter().enumerate() {
                if matches!(def, Def::Expr(d) if expr::reset_list_records(self, d)) {
                    dirty.insert(self.evaluation_rank(Id(i)));
                }
            }
            for &i in self
                .circle_records
                .iter()
                .map(|(i, _)| i)
                .chain(self.conics.iter().map(|(i, _)| i))
                .chain(self.arcs.iter().map(|(i, _)| i))
            {
                dirty.insert(self.evaluation_rank(Id(i)));
            }
        }
        // Redefinition rebuilds the reference's symbolic helpers, including
        // those for intersections that do not read the edited definition.
        self.proof_intersections = Default::default();
        self.rebuild_self_circle_updates();
        let refresh = if rebuilds {
            // Restart independent vertex algorithms without refreshing the
            // definitions of their descendants as if those definitions changed.
            let refresh = DefinitionRefresh::Selected(dirty.clone());
            self.restart_inequality_vertices(&mut dirty);
            refresh
        } else {
            DefinitionRefresh::All
        };
        self.propagate_dirty_with(
            dirty,
            0,
            Propagation::Redefinition {
                refresh,
                rebuild_records,
            },
        );
        if rebuilds {
            self.inequality_helpers.pending.clear();
        }
        // A checkpoint cannot restore a changed definition.
        self.invalidate_checkpoints();
        Ok(Some(id))
    }

    /// AlgebraProcessor updates independent values in place. Construction.replace
    /// also skips the XML rebuild for identical definitions, compatible command
    /// edits on constant inputs, and objects without dependents.
    fn replacement_rebuilds_construction(&self, id: Id, replacement: Def) -> bool {
        if self.children[id.0].is_empty() {
            return false;
        }
        let old = self.def(id);
        let independent = |def| expr::independent_definition(self, def);
        if (old.kind() == replacement.kind() && independent(old) && independent(replacement))
            || expr::same_definition(self, old, replacement)
        {
            return false;
        }
        if let (
            Def::Path(PathDef::OnParam { path, param }),
            Def::Path(PathDef::OnParam {
                path: new_path,
                param: new_param,
            }),
        ) = (old, replacement)
        {
            if path != new_path {
                return true;
            }
            let anonymous = |id| self.label(id).starts_with('\0');
            // The command's XML shows the definition of an anonymous input.
            // Its freshly allocated slot does not change that definition.
            return !(anonymous(param)
                && anonymous(new_param)
                && (expr::same_definition(self, self.def(param), self.def(new_param))
                    || (independent(self.def(param)) && independent(self.def(new_param)))));
        }
        true
    }

    fn refresh_circle_hints(&mut self, dirty: &mut DirtySet) {
        let mut first_points = HashMap::new();
        for i in 0..self.len() {
            let Def::IntersectCircles { c1, c2, .. } = self.defs[i] else {
                continue;
            };
            if c1 != c2 {
                continue;
            }
            let rank = self.evaluation_rank(Id(i));
            // One scan per circle, over edges already bounded by the reorder.
            let point = *first_points
                .entry(c1)
                .or_insert_with(|| algo::intersect::first_point_on(self, c1));
            let hint = algo::intersect::point_hint(
                point.filter(|&point| self.evaluation_rank(point) < rank),
            );
            if hint == self.hints[i] {
                continue;
            }
            let old = self.reads(Id(i));
            self.hints[i] = hint;
            let new = self.reads(Id(i));
            for parent in old.into_iter().filter(|parent| !new.contains(parent)) {
                self.children[parent.0].remove(Id(i));
            }
            for parent in new {
                if let Some(children) = self.children.get_mut(parent.0) {
                    children.insert(Id(i));
                }
            }
            dirty.insert(rank);
        }
    }

    /// Loci watch free inputs during edits. Redefinition also needs the
    /// point and driver they sample, which define which inputs to watch.
    fn locus_order_inputs(&self) -> Result<BTreeMap<Id, Vec<Id>>, &'static str> {
        if self.loci.len() > MAX_REDEFINITION_EDGES / 2 {
            return Err("the loci exceed the redefinition budget");
        }
        let mut inputs = BTreeMap::<_, Vec<_>>::new();
        for &(i, _) in &self.loci {
            if let Def::Locus(
                LocusDef::Path {
                    point,
                    mover: driver,
                }
                | LocusDef::Slider {
                    point,
                    slider: driver,
                },
            ) = self.defs[i]
            {
                inputs.entry(point).or_default().push(Id(i));
                inputs.entry(driver).or_default().push(Id(i));
            }
        }
        Ok(inputs)
    }

    fn replacement_dependents(
        &self,
        start: Id,
        locus_inputs: &BTreeMap<Id, Vec<Id>>,
    ) -> Result<BTreeSet<Id>, &'static str> {
        let mut remaining = MAX_REDEFINITION_EDGES;
        let mut pending = vec![start];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            for child in self
                .children(id)
                .chain(locus_inputs.get(&id).into_iter().flatten().copied())
            {
                remaining = remaining
                    .checked_sub(1)
                    .ok_or("the dependent objects exceed the redefinition budget")?;
                if seen.insert(child) {
                    pending.push(child);
                }
            }
        }
        Ok(seen)
    }

    /// Prepares a replacement's order before any existing object is changed.
    fn replacement_order(
        &self,
        changed: &BTreeMap<Id, Vec<Id>>,
        locus_inputs: &BTreeMap<Id, Vec<Id>>,
        len: usize,
    ) -> Result<EvaluationOrder, &'static str> {
        const MAX_OBJECTS: usize = 250_000;
        if len > MAX_OBJECTS {
            return Err("the dependency graph exceeds the redefinition budget");
        }
        let mut added = vec![Vec::new(); len];
        let mut remaining = MAX_REDEFINITION_EDGES;
        for (&child, parents) in changed {
            for parent in parents {
                if let Some(children) = added.get_mut(parent.0) {
                    remaining = remaining
                        .checked_sub(1)
                        .ok_or("the dependency graph exceeds the redefinition budget")?;
                    children.push(child);
                }
            }
        }
        let successors = |i: usize| {
            self.children[i]
                .iter()
                .filter(|child| child.0 < len && !changed.contains_key(child))
                .chain(locus_inputs.get(&Id(i)).into_iter().flatten().copied())
                .chain(added[i].iter().copied())
        };
        let mut remaining = MAX_REDEFINITION_EDGES;
        let mut indegree = vec![0; len];
        for i in 0..len {
            for child in successors(i) {
                remaining = remaining
                    .checked_sub(1)
                    .ok_or("the dependency graph exceeds the redefinition budget")?;
                indegree[child.0] += 1;
            }
        }
        let mut ready: BinaryHeap<_> = (0..len)
            .filter(|&i| indegree[i] == 0)
            .map(Reverse)
            .collect();
        let mut ids = Vec::with_capacity(len);
        let mut ranks = vec![0; len];
        while let Some(Reverse(i)) = ready.pop() {
            ranks[i] = ids.len();
            ids.push(i);
            for child in successors(i) {
                indegree[child.0] -= 1;
                if indegree[child.0] == 0 {
                    ready.push(Reverse(child.0));
                }
            }
        }
        if ids.len() != len {
            return Err("a replacement cannot introduce a dependency cycle");
        }
        Ok(EvaluationOrder { ids, ranks })
    }

    /// The position of an object in dependency order.
    pub fn evaluation_rank(&self, id: Id) -> usize {
        self.evaluation_order
            .ranks
            .get(id.0)
            .copied()
            .unwrap_or(id.0)
    }

    pub(crate) fn evaluation_id(&self, rank: usize) -> usize {
        self.evaluation_order.ids.get(rank).copied().unwrap_or(rank)
    }

    pub(crate) fn predecessor_storage_len(&self, id: Id) -> usize {
        if self.evaluation_order.ids.is_empty() && self.replacement_label.is_none() {
            id.0 + 1
        } else {
            self.len()
        }
    }

    /// Every object `def` reads: its parents, the objects its rule also
    /// reads (for example the line an intersection was prepared with), and
    /// its list. An object may appear more than once.
    pub(crate) fn read_ids(&self, def: &Def) -> Vec<Id> {
        if let Def::Locus(d) = *def {
            return locus::reads(self, d);
        }
        let mut ids = algo::intersect::hidden_parents(self, def);
        ids.extend(algo::path::hidden_parents(self, def));
        ids.extend(algo::vector::hidden_parents(self, def));
        ids.extend(algo::predicate::hidden_parents(self, def));
        ids.extend(algo::polygon::hidden_parents(self, def));
        ids.extend(algo::measure::hidden_parents(self, def));
        if let Def::Point(point) = *def {
            ids.extend_from_slice(point.hidden_parents(self));
        }
        ids.extend(angle::hidden_parents(self, def));
        ids.extend(expr::parents(self, def));
        if let Def::Equation(d) = *def {
            ids.extend(expr::equation::parents(self, d));
        }
        if let Def::ImplicitCurve(index) = *def {
            ids.extend(expr::equation::implicit::parents(self, index));
        }
        ids.extend_from_slice(def.parents().as_slice());
        if let Some(list) = def.list() {
            ids.extend_from_slice(self.list(list));
        }
        if let Def::Optimize(d) = *def {
            if let Some(watched) = d.watched() {
                ids.extend_from_slice(self.list(watched));
            }
        }
        ids
    }

    /// Adds an object and computes its value.
    pub fn add(&mut self, label: &str, def: Def) -> Result<Id, Error> {
        self.add_with_path_state(label, def, None, None)
    }

    /// Adds another output of the object `owner`: the same value as
    /// [`Construction::add`], but the protocol counts it with its owner's
    /// construction element instead of opening one of its own.
    pub(crate) fn add_output(&mut self, label: &str, def: Def, owner: Id) -> Result<Id, Error> {
        self.add_with_path_state(label, def, None, Some(owner))
    }

    /// `GeoElement.remove` of an output that its command no longer shows
    /// (`AlgoRoots.initPoints`): the object stays in the construction but
    /// loses its label, which a later object can take. Refused for an
    /// object that is already unlabelled or that other objects read.
    pub(crate) fn free_output_label(&mut self, id: Id) -> bool {
        self.free_label_as(id, freed_label(id))
    }

    /// [`Self::free_output_label`] for the first output of a command, which
    /// the slot lines of the journal name: it keeps the label it had in its
    /// hidden label (see [`Self::freed_owner`]). Refused while another freed
    /// first output keeps the same label.
    pub(crate) fn free_owner_label(&mut self, id: Id) -> bool {
        let Some(label) = self.labels.get(id.0) else {
            return false;
        };
        let freed = freed_owner_label(label);
        !self.by_label.contains_key(&freed) && self.free_label_as(id, freed)
    }

    fn free_label_as(&mut self, id: Id, freed: String) -> bool {
        let Some(label) = self.labels.get(id.0) else {
            return false;
        };
        if label.starts_with('\0') || !self.children[id.0].is_empty() {
            return false;
        }
        let old = std::mem::replace(&mut self.labels[id.0], freed.clone());
        self.by_label.remove(&old);
        self.object_scripts.remove(&old);
        self.by_label.insert(freed, id);
        // a freed label lies below the start of its family's search
        self.label_hints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        true
    }

    /// The first output that [`Self::free_owner_label`] freed from `label`.
    pub(crate) fn freed_owner(&self, label: &str) -> Option<Id> {
        self.by_label.get(&freed_owner_label(label)).copied()
    }

    /// The label a first output had before [`Self::free_owner_label`].
    pub(crate) fn former_owner_label(&self, id: Id) -> Option<&str> {
        self.labels.get(id.0)?.strip_prefix(FREED_OWNER)
    }

    /// Whether other objects read `id`.
    pub(crate) fn has_dependents(&self, id: Id) -> bool {
        self.children.get(id.0).is_some_and(|c| !c.is_empty())
    }

    /// Gives the output `id` the free label `label`; the label it had is
    /// freed. Refused if `label` is taken or if other objects read `id`
    /// under a label it has.
    pub(crate) fn relabel_output(&mut self, id: Id, label: &str) -> Result<(), Error> {
        if label.is_empty() || label.starts_with('\0') {
            return Err(Error::EmptyLabel);
        }
        let old = self.label(id).to_owned();
        if old == label {
            return Ok(());
        }
        if self.resolve_label(label).is_some() {
            return Err(Error::DuplicateLabel(label.to_string()));
        }
        if !old.starts_with('\0') && !self.free_output_label(id) {
            return Err(Error::DuplicateLabel(old));
        }
        let hidden = std::mem::replace(&mut self.labels[id.0], label.to_string());
        self.by_label.remove(&hidden);
        self.by_label.insert(label.to_string(), id);
        Ok(())
    }

    /// Whether `id` is an output whose label [`Self::free_output_label`] or
    /// [`Self::free_owner_label`] freed.
    pub fn is_freed_output(&self, id: Id) -> bool {
        self.labels
            .get(id.0)
            .is_some_and(|label| *label == freed_label(id) || label.starts_with(FREED_OWNER))
    }

    fn add_with_path_state(
        &mut self,
        label: &str,
        def: Def,
        saved_path: Option<PathState>,
        owner: Option<Id>,
    ) -> Result<Id, Error> {
        if label.is_empty() {
            return Err(Error::EmptyLabel);
        }
        if self.resolve_label(label).is_some() {
            return Err(Error::DuplicateLabel(label.to_string()));
        }
        if let Some(axis) = axis::definition(def) {
            return Err(Error::DuplicateLabel(axis.label.to_string()));
        }
        self.check(&def)?;
        let mut def = algo::intersect::prepare(self, def);
        if let Def::Optimize(d) = def {
            def = Def::Optimize(d.attach(self)?);
        }

        let id = Id(self.defs.len());
        // compute reads the hint of the new object from `hints`
        self.hints.push(algo::intersect::hint(self, &def));
        let value = match def {
            Def::Lines(LineDef::TangentPoint { .. } | LineDef::TangentLine { .. })
            | Def::ConicPart(
                ConicPartDef::TangentPoint { .. } | ConicPartDef::TangentLine { .. },
            ) => {
                self.add_tangent_output(id, TangentDef::of(def).expect("tangent definition"), owner)
            }
            Def::Expr(d) => match expr::initialize(self, id, d) {
                Ok(value) => value,
                Err(error) => {
                    self.hints.pop();
                    return Err(error);
                }
            },
            Def::IntersectConics(d) => {
                let mut state = self.create_conic_intersection(d);
                let value = state.value(d, self).unwrap_or(Value::Undefined);
                self.conic_intersections.push((id.0, state));
                value
            }
            Def::IntersectLineConic(d) => self.add_line_conic_output(id, d),
            Def::IntersectLineCircle { .. }
            | Def::ConicPart(ConicPartDef::IntersectLine { .. }) => {
                let d = LineConicDef::from_indexed(def).expect("indexed line/conic output");
                self.add_line_conic_output(id, d)
            }
            Def::GeneralConic(d) => {
                let data = d.record(self, &ConicData::new());
                self.conics.push((id.0, data));
                algo::general_conic::value(d.kind(), &data)
            }
            Def::Path(d) if d.has_state() => {
                let state = saved_path.unwrap_or_else(|| d.create(self));
                self.paths.push((id.0, state));
                PathDef::value(&state)
            }
            Def::Arc(d) if d.has_record() => {
                let record = d.record(self, None);
                self.arcs.push((id.0, record));
                algo::arc::value(d.kind() == Kind::Sector, &record)
            }
            Def::Lines(LineDef::CommonTangent(d)) if d.has_state(self) => {
                let (state, value) = d.create(self);
                self.limited_common_tangents.push((id.0, state));
                value.unwrap_or(Value::Undefined)
            }
            Def::ImplicitCurve(index) => expr::equation::implicit::update(self, index),
            Def::Equation(d) => {
                let (value, record) = crate::expr::equation::initialize(self, d);
                if let Some(record) = record {
                    self.conics.push((id.0, record));
                }
                value
            }
            Def::Chart(d) => {
                let mut data = d.compute(self);
                data.bump_revision();
                let value = data.value;
                self.charts.insert(id, data);
                value
            }
            Def::Prove(d) => self.record_proof(id, d),
            Def::Conditional {
                args,
                kind: Kind::Segment,
            } => {
                let initial = self.conditional_segment_copy(args);
                let (value, carrier) = self.conditional_segment(args, initial);
                self.set_segment_carrier(id, carrier);
                value
            }
            Def::Locus(d) => {
                let data = locus::record(self, d, id);
                self.store_locus(id, data)
            }
            Def::Optimize(d) => {
                let d = algo::optimize::disabled(self, d).unwrap_or(d);
                def = Def::Optimize(d);
                algo::optimize::record(self, d)
            }
            Def::Calculus(d) if d.has_record() => {
                let (value, record) = algo::calculus::initialize(self, &d);
                self.calculus.push((id.0, record));
                value
            }
            Def::Discrete(d) => self.store_locus(id, d.record(self, None)),
            _ => self.compute(def, id.0),
        };
        if let Def::IntersectBoundary(intersection) = def {
            if let Some(saved) = intersection.with_root(value) {
                def = Def::IntersectBoundary(saved);
            }
        }
        if let Def::Point(PointDef::InequalityVertex { source, owner, .. }) = def {
            self.inequality_helpers.attach(source, owner, id);
        }
        // An object that is read twice is still one child.
        let mut parents = self.read_ids(&def);
        parents.sort_unstable();
        parents.dedup();
        for parent in parents {
            if let Some(children) = self.children.get_mut(parent.0) {
                children.insert(id);
            }
        }
        if matches!(def, Def::Boolean(_)) {
            self.free_booleans.push(id);
        }
        if !value.is_defined() {
            if let Some(record) = algo::circle::invalid_record(self, def) {
                self.circle_records.push((id.0, record));
            }
        }
        self.defs.push(def);
        self.values.push(value);
        if let Def::IntersectCircles { c1, c2, .. } = def {
            if c1 == c2 {
                self.self_circle_outputs.push(id);
                self.track_self_circle_update(c1, id);
            }
        }
        if def.kind() == Kind::Image {
            self.register_transformed_image(id, def);
        }
        self.labels.push(label.to_string());
        self.children.push(Children::default());
        if !self.evaluation_order.ids.is_empty() {
            self.evaluation_order.ids.push(id.0);
            self.evaluation_order.ranks.push(id.0);
        }
        self.by_label.insert(label.to_string(), id);
        let element = if let Some(owner) = owner {
            self.output_entry(owner)
        } else if label.starts_with('\0') || self.replacement_label.as_deref() == Some(label) {
            ProtocolEntry::Hidden
        } else {
            let index = self.step;
            self.elements.insert(index, id.0 as u32);
            self.step += 1;
            self.reindex_elements_from(self.step);
            ProtocolEntry::Element(index as u32)
        };
        self.element_of.push(element);
        let reads_step = matches!(def, Def::Measure(MeasureDef::ConstructionStep(_)));
        if reads_step {
            self.protocol.push(id.0);
        }
        if matches!(
            def,
            Def::Point(
                crate::algo::point::PointDef::ViewCorner { .. }
                    | crate::algo::point::PointDef::DrawableCorner { .. }
            ) | Def::Measure(MeasureDef::AxisStep { .. })
        ) {
            self.view_readers.push(id.0);
        }
        if reads_step || matches!(element, ProtocolEntry::Element(_)) {
            self.update_protocol();
        }
        Ok(id)
    }

    /// The step of the construction protocol (`Construction.getStep`).
    pub fn step(&self) -> i32 {
        self.step as i32 - 1
    }

    /// `CmdSetConstructionStep`: moves the protocol step, clamped to the
    /// construction, and recomputes the objects that read it. `Math.round`
    /// returns a long, so clamp before narrowing the index.
    pub fn set_construction_step(&mut self, value: f64) {
        let index = crate::expr::math::java_round(value - 1.0);
        if index < -1 {
            return;
        }
        let step = (index.min(self.elements.len() as i64 - 1) + 1) as usize;
        if step != self.step {
            self.step = step;
            self.update_protocol();
        }
    }

    /// The position of the construction element `id` belongs to
    /// (`GeoElement.getConstructionIndex`: its own for a free object, its
    /// algorithm's for a dependent one).
    pub(crate) fn element_of(&self, id: Id) -> i32 {
        match self.element_of.get(id.0) {
            Some(ProtocolEntry::Element(index)) => *index as i32,
            Some(ProtocolEntry::Output(owner)) => self.element_of(Id(*owner as usize)),
            _ => -1,
        }
    }

    fn output_entry(&self, owner: Id) -> ProtocolEntry {
        match self.element_of[owner.0] {
            ProtocolEntry::Hidden => ProtocolEntry::Hidden,
            ProtocolEntry::Element(_) => ProtocolEntry::Output(owner.0 as u32),
            entry @ ProtocolEntry::Output(_) => entry,
        }
    }

    fn reindex_elements_from(&mut self, start: usize) {
        for (position, &id) in self.elements.iter().enumerate().skip(start) {
            self.element_of[id as usize] = ProtocolEntry::Element(position as u32);
        }
    }

    pub(crate) fn has_protocol_readers(&self) -> bool {
        !self.protocol.is_empty()
    }

    /// Recomputes the objects that read the construction step and whatever
    /// depends on them (`updateAllConstructionProtocolAlgorithms`).
    pub(crate) fn update_protocol(&mut self) {
        if self.protocol.is_empty() {
            return;
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        self.queue_protocol_readers(&mut dirty);
        let history = self
            .protocol_history
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .upgrade();
        if let Some(history) = history {
            let mut frames = history.lock().unwrap_or_else(|e| e.into_inner());
            ProtocolFrame::merge_unused_tail(&mut frames);
            let frame = frames.last_mut().expect("active checkpoint");
            self.propagate_dirty_with(dirty, 0, Propagation::RecordProtocol(&mut frame.before));
        } else {
            self.propagate_dirty(dirty, 0);
        }
    }

    fn queue_protocol_readers(&self, dirty: &mut DirtySet) {
        for &i in &self.protocol {
            dirty.insert(self.evaluation_rank(Id(i)));
        }
    }

    /// Installs rigid constraints on existing free points after all outputs
    /// of the polygon have been validated and added.
    pub(crate) fn rigidify_points(&mut self, owner: Id, defs: Vec<(Id, Def)>) {
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        for (id, def) in defs {
            let prior = self.defs[id.0];
            debug_assert!(matches!(prior, Def::FreePoint { .. }));
            self.rigid_replacements.push((owner, id, prior));
            self.defs[id.0] = def;
            for parent in def.parents().as_slice() {
                let children = &mut self.children[parent.0];
                children.insert(id);
            }
            if let Def::Path(path) = def {
                let slot = self.paths.partition_point(|&(other, _)| other < id.0);
                self.paths.insert(slot, (id.0, path.create(self)));
            }
            dirty.insert(self.evaluation_rank(id));
        }
        self.propagate_dirty(dirty, 0);
    }

    /// The elements of the list object `id`; `None` when it is not a list
    /// or the list is undefined.
    pub fn elements(&self, id: Id) -> Option<expr::List<'_>> {
        expr::list(self, id)
    }

    /// The tree of the expression object `id`, owned, so that it can be added
    /// to another construction with [`Self::add_expression`]; `None` when
    /// `id` is not an expression object. Its [`Def::Expr`] cannot be used
    /// for that: the handle is valid only here (see [`expr`]). A free number
    /// that still has the exact decimal of the literal or constant
    /// expression it was made from is handed out too, with that decimal;
    /// [`Self::add`] makes a free number without one.
    pub fn expression(&self, id: Id) -> Option<expr::Expression> {
        expr::expression(self, id)
    }

    /// Adds the expression `e` as `label`, checking its tree against this
    /// construction; see [`expr::Expression`].
    pub fn add_expression(&mut self, label: &str, e: &expr::Expression) -> Result<Id, Error> {
        expr::add_expression(self, label, e)
    }

    /// Stores `ids` as a list of parents and adds the object that `make`
    /// defines from its handle (see the [module docs](self)). When the object
    /// is rejected, the list is removed again and the error is returned.
    pub fn add_with_list(
        &mut self,
        label: &str,
        ids: &[Id],
        make: impl FnOnce(IdList) -> Def,
    ) -> Result<Id, Error> {
        let start = self.lists.len();
        let list = self.store_list(ids)?;
        let result = self.add(label, make(list));
        if result.is_err() {
            self.lists.truncate(start);
        }
        result
    }

    pub(crate) fn store_list(&mut self, ids: &[Id]) -> Result<IdList, Error> {
        let start = self.lists.len();
        let list = match (u32::try_from(start), u32::try_from(ids.len())) {
            (Ok(start), Ok(len)) if start.checked_add(len).is_some() => IdList { start, len },
            _ => return Err(Error::UnknownObject),
        };
        self.lists.extend_from_slice(ids);
        Ok(list)
    }

    /// The objects of a stored list. A handle that this construction did not
    /// give out reads as an empty list.
    pub fn list(&self, list: IdList) -> &[Id] {
        self.lists.get(list.range()).unwrap_or(&[])
    }

    /// The objects the definition of `id` takes as its inputs, in the order
    /// of the command: its parents, then its stored list (a polygon's
    /// vertices). An expression has none, and the objects a rule only reads
    /// on the side (such as the line an intersection was prepared with) are
    /// not inputs. An unknown id has none.
    pub fn inputs(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        let def = self.defs.get(id.0);
        let parents = def.map_or_else(Parents::none, Def::parents);
        let list = def.and_then(Def::list).map_or(&[][..], |l| self.list(l));
        (0..parents.as_slice().len())
            .map(move |i| parents.as_slice()[i])
            .chain(list.iter().copied())
    }

    /// Checks that `list` was stored here and that every object in it exists
    /// and has one of the `expected` kinds.
    pub(crate) fn expect_list(&self, list: IdList, expected: &'static [Kind]) -> Result<(), Error> {
        let ids = self.lists.get(list.range()).ok_or(Error::UnknownObject)?;
        for &id in ids {
            self.expect(id, expected)?;
        }
        Ok(())
    }

    /// Moves a free point or a free vector and recomputes everything that
    /// depends on it. Returns how many dependent objects were recomputed.
    ///
    /// The count includes objects that read the moved object only in a case
    /// their current state does not take. For example, `Intersect(k, k, 1)`
    /// reads the first point on `k` by construction, but uses it only when
    /// its `special` flag is set; with the flag false, moving that point
    /// still recomputes the intersection, which does not change.
    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> Result<usize, Error> {
        self.last_update_requires_replay = false;
        if let Some(axis) = axis::get(id) {
            return Err(Error::NotFree(axis.label.to_string()));
        }
        let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        if let Def::Path(
            d @ (PathDef::On { .. }
            | PathDef::InRegion { .. }
            | PathDef::RigidEndpoint { .. }
            | PathDef::FixedEndpoint { .. }),
        ) = *def
        {
            return self.move_on_path(id, d, x, y);
        }
        // The value of a free point or vector is always its coordinates.
        let (def, new, old) = match *def {
            Def::FreePoint { x: ox, y: oy } => {
                (Def::FreePoint { x, y }, Value::Point { x, y }, (ox, oy))
            }
            Def::Vector(VectorDef::Free { x: ox, y: oy }) => (
                Def::Vector(VectorDef::Free { x, y }),
                Value::Vector { x, y },
                (ox, oy),
            ),
            _ => return Err(Error::NotFree(self.labels[id.0].clone())),
        };
        if !(x.is_finite() && y.is_finite()) {
            return Err(Error::NotFinite);
        }
        // Bit equality, as in `set_number`: `0` to `-0` is a change.
        let same = |a: f64, b: f64| a.to_bits() == b.to_bits();
        if same(old.0, x) && same(old.1, y) {
            return Ok(0);
        }
        self.construction_sources.remove(&id);
        self.defs[id.0] = def;
        self.values[id.0] = new;
        Ok(self.propagate(id))
    }

    /// Moves free points together, recomputing their shared descendants once.
    pub fn move_points(&mut self, points: &[(Id, f64, f64)]) -> Result<usize, Error> {
        for &(id, x, y) in points {
            if let Some(axis) = axis::get(id) {
                return Err(Error::NotFree(axis.label.to_string()));
            }
            let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
            if !matches!(def, Def::FreePoint { .. }) {
                return Err(Error::NotFree(self.labels[id.0].clone()));
            }
            if !(x.is_finite() && y.is_finite()) {
                return Err(Error::NotFinite);
            }
        }

        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.defs.len());
        let mut first = self.defs.len();
        for &(id, x, y) in points {
            let Def::FreePoint { x: old_x, y: old_y } = self.defs[id.0] else {
                unreachable!("all points were validated before moving");
            };
            if old_x.to_bits() == x.to_bits() && old_y.to_bits() == y.to_bits() {
                continue;
            }
            self.construction_sources.remove(&id);
            self.defs[id.0] = Def::FreePoint { x, y };
            self.values[id.0] = Value::Point { x, y };
            first = first.min(self.evaluation_rank(id));
            for child in self.children[id.0].iter() {
                dirty.insert(self.evaluation_rank(child));
            }
        }
        Ok(self.propagate_dirty(dirty, first))
    }

    /// [`Self::move_point`] for a point made by `Point(path)`:
    /// `GeoPoint.setCoords`, which projects the point onto its path. A move
    /// to the point's own coordinates is not skipped: the projection can
    /// still change the state (the parameter kept for an undefined path, or
    /// the parameter of a point that sits off a path that changed shape).
    /// Only a move that leaves the state the same, bit for bit, recomputes
    /// nothing, because everything downstream reads only that state.
    fn move_on_path(&mut self, id: Id, d: PathDef, x: f64, y: f64) -> Result<usize, Error> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(Error::NotFinite);
        }
        let slot = self.path_slot(id.0).ok_or(Error::UnknownObject)?;
        let Some(new) = d.moved(self, &self.paths[slot].1, x, y) else {
            return Err(Error::NotFree(self.labels[id.0].clone()));
        };
        if new.same(&self.paths[slot].1) {
            return Ok(0);
        }
        self.paths[slot].1 = new;
        self.values[id.0] = PathDef::value(&new);
        Ok(self.propagate(id))
    }

    /// Restores the coordinates saved for a generated regular-polygon vertex.
    /// AlgoPolygonRegularND.java leaves the saved point alone if an endpoint
    /// is undefined, and replaces its coordinates when both are defined.
    pub(crate) fn restore_regular_vertex(&mut self, id: Id, x: f64, y: f64) -> Result<(), Error> {
        let saved = if x.is_nan() && y.is_nan() {
            Value::Undefined
        } else if x.is_finite() && y.is_finite() {
            Value::Point { x, y }
        } else {
            return Err(Error::NotFinite);
        };
        let Def::Polygon(PolygonDef::RegularVertex { a, b, n, k }) = self.def(id) else {
            return Err(Error::NotFree(self.label(id).into()));
        };
        if let Some(value) = algo::polygon::regular_vertex(self, a, b, n, k, Some(saved)) {
            if !value.identical(&self.values[id.0]) {
                self.values[id.0] = value;
                self.propagate(id);
            }
        }
        Ok(())
    }

    /// Changes a free number and recomputes everything that depends on it.
    /// Returns how many dependent objects were recomputed.
    pub fn set_number(&mut self, id: Id, v: f64) -> Result<usize, Error> {
        Ok(self.set_number_if_changed(id, v)?.unwrap_or(0))
    }

    /// Sets the number, returning the number of recomputed objects when the
    /// value or its definition changed.
    pub fn set_number_if_changed(&mut self, id: Id, v: f64) -> Result<Option<usize>, Error> {
        self.update_number(id, v, true)
    }

    /// A numeric assignment to a slider sets its value outside the drag interval too.
    pub(crate) fn assign_slider_number(&mut self, id: Id, v: f64) -> Result<usize, Error> {
        Ok(self.update_number(id, v, false)?.unwrap_or(0))
    }

    fn update_number(
        &mut self,
        id: Id,
        v: f64,
        clamp_slider: bool,
    ) -> Result<Option<usize>, Error> {
        self.last_update_requires_replay = false;
        if let Some(axis) = axis::get(id) {
            return Err(Error::NotFree(axis.label.to_string()));
        }
        let def = *self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        let new_def = match def {
            Def::Number(_) => Def::Number(v),
            Def::Angle(_) => Def::Angle(v),
            Def::AngleObject(d) if d.is_free() => Def::AngleObject(AngleDef {
                source: AngleSource::Free(v),
                ..d
            }),
            _ => return Err(Error::NotFree(self.labels[id.0].clone())),
        };
        if v.is_nan() || (!matches!(new_def, Def::Number(_)) && !v.is_finite()) {
            return Err(Error::NotFinite);
        }
        // A slider keeps its value in its active interval.
        let (new_def, v) = if clamp_slider && matches!(new_def, Def::Number(_)) {
            let v = self.clamp_slider(id, v);
            (Def::Number(v), v)
        } else {
            (new_def, v)
        };
        // A changed number loses its exact decimal (`GeoNumeric.setValue`),
        // which changes what reads it even when the value stays.
        let had_decimal = expr::forget_decimal(self, id);
        let had_source = self.construction_sources.remove(&id).is_some();
        // Bit equality: `0` to `-0` is a change (`1/a` flips its sign).
        let old = match def {
            Def::Number(old)
            | Def::Angle(old)
            | Def::AngleObject(AngleDef {
                source: AngleSource::Free(old),
                ..
            }) => old,
            _ => unreachable!("the definition is a free scalar"),
        };
        if !had_decimal
            && !had_source
            && old.to_bits() == v.to_bits()
            && self.values[id.0].is_defined()
        {
            return Ok(None);
        }
        self.defs[id.0] = new_def;
        self.values[id.0] = self.compute(new_def, id.0);
        // a value set by hand is where an animation goes on from
        self.reset_animation_position(id);
        Ok(Some(self.propagate(id)))
    }

    /// True when the last coordinate or value update must replay in order
    /// to preserve retained geometry, hidden intersection starts,
    /// inequality vertex helpers or dependent slider clamps.
    pub fn last_update_requires_replay(&self) -> bool {
        self.last_update_requires_replay
    }

    /// Rebuilds inequality-vertex algorithms at the current input values,
    /// as when their definitions are read from a saved construction.
    pub fn reset_inequality_vertex_helpers(&mut self) -> bool {
        let mut unlimited = u64::MAX;
        self.reset_inequality_vertex_helpers_with_budget(&mut unlimited)
            .expect("unlimited helper reset")
    }

    /// Starts fresh only for vertex slots evaluated since the last reset.
    /// A checkpoint can therefore repeat at most the slot work its edits
    /// scheduled. Descendants update as usual when a vertex changes.
    pub fn refresh_inequality_vertex_helpers(&mut self) -> bool {
        if self.inequality_helpers.pending.is_empty() {
            return false;
        }
        let pending = std::mem::take(&mut self.inequality_helpers.pending);
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        expr::charge(pending.len() as u64);
        for id in pending {
            dirty.insert(self.evaluation_rank(id));
        }
        self.inequality_helpers.refreshing = true;
        self.propagate_dirty(dirty, 0);
        self.inequality_helpers.refreshing = false;
        true
    }

    /// Rebuilds the helpers in a scratch construction during replay. Charges
    /// the slots before scheduling them and checks the solver work between
    /// recomputations. If the budget runs out, the caller must discard the
    /// partially replayed construction.
    pub fn reset_inequality_vertex_helpers_with_budget(
        &mut self,
        remaining: &mut u64,
    ) -> Option<bool> {
        let slots = self.inequality_helpers.outputs().len() as u64;
        if slots == 0 {
            return Some(false);
        }
        if slots > *remaining {
            return None;
        }
        let start = expr::work_done();
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        self.restart_inequality_vertices(&mut dirty);
        self.propagate_dirty_while(dirty, 0, Propagation::Live, || {
            expr::work_done().wrapping_sub(start) <= *remaining
        })?;
        self.inequality_helpers.pending.clear();
        *remaining = remaining.checked_sub(expr::work_done().wrapping_sub(start))?;
        Some(true)
    }

    fn restart_inequality_vertices(&mut self, dirty: &mut DirtySet) {
        let outputs = self.inequality_helpers.outputs();
        expr::charge(outputs.len() as u64);
        for &id in outputs {
            dirty.insert(self.evaluation_rank(id));
        }
        self.inequality_helpers.clear_history();
    }

    /// Recomputes the descendants of `start` in dependency order.
    ///
    /// A set of dirty indices ([`DirtySet`]) gives the order: the smallest
    /// is taken next. Because children always have larger ranks than their
    /// parents, by the time an object is reached every parent that could
    /// still change has already been handled. Unlike a heap, inserting and
    /// taking cost a few bit operations, which matters when a point has tens
    /// of thousands of children.
    fn propagate(&mut self, start: Id) -> usize {
        // Taken out so that `compute` can borrow `self`. If `compute` panics
        // the set is lost, and `fit` gives a clean one on the next update.
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.defs.len());
        for child in self.children[start.0].iter() {
            dirty.insert(self.evaluation_rank(child));
        }
        self.propagate_dirty(dirty, self.evaluation_rank(start))
    }

    fn propagate_dirty(&mut self, dirty: DirtySet, from: usize) -> usize {
        self.propagate_dirty_with(dirty, from, Propagation::Live)
    }

    fn propagate_dirty_with(
        &mut self,
        dirty: DirtySet,
        from: usize,
        mode: Propagation<'_>,
    ) -> usize {
        self.propagate_dirty_while(dirty, from, mode, || true)
            .expect("unlimited propagation")
    }

    fn propagate_dirty_while(
        &mut self,
        mut dirty: DirtySet,
        mut from: usize,
        mut mode: Propagation<'_>,
        mut within_budget: impl FnMut() -> bool,
    ) -> Option<usize> {
        let rebuild_records = matches!(
            mode,
            Propagation::Redefinition {
                rebuild_records: true,
                ..
            } | Propagation::Reload
        );
        let redefining = matches!(mode, Propagation::Redefinition { .. });
        self.last_update_requires_replay = false;
        self.inequality_helpers.begin_update();
        self.function_roots.clear();
        self.clips.clear();
        self.analysis.clear();
        let mut recomputed = 0;
        let mut restored_pairs = BTreeSet::new();
        while let Some(rank) = dirty.pop_first(from) {
            if !within_budget() {
                return None;
            }
            from = rank + 1;
            let i = self.evaluation_id(rank);
            if matches!(mode, Propagation::RestoreProtocol(restored) if restored.binary_search(&i).is_ok())
            {
                continue;
            }
            let mut def = self.defs[i];
            let saved = match &mut mode {
                Propagation::RecordProtocol(before) => {
                    if let Def::Point(PointDef::InequalityVertex { owner, .. }) = def {
                        // An output can choose a shared helper even when its
                        // coordinates stay the same.
                        for index in [owner.0, i] {
                            before.entry(index).or_insert_with(|| {
                                self.protocol_record(index)
                                    .expect("inequality vertex record")
                            });
                        }
                        None
                    } else if !before.contains_key(&i) {
                        self.protocol_record(i)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let refresh_definitions = match &mode {
                Propagation::Redefinition { refresh, .. } => match refresh {
                    DefinitionRefresh::All => true,
                    DefinitionRefresh::Selected(definitions) => definitions.contains(rank),
                },
                _ => false,
            };
            recomputed += 1;
            if let Some(children) = self.self_circle_updates.get(&Id(i)) {
                for &child in children {
                    dirty.insert(self.evaluation_rank(child));
                }
            }
            if refresh_definitions {
                // A rule can use the structure of an ancestor even when
                // intermediate objects have exactly the same value.
                for child in self.children[i].iter() {
                    let rank = self.evaluation_rank(child);
                    dirty.insert(rank);
                    if let Propagation::Redefinition {
                        refresh: DefinitionRefresh::Selected(definitions),
                        ..
                    } = &mut mode
                    {
                        definitions.insert(rank);
                    }
                }
                def = self.refresh_intersection_definition(i, def, &mut restored_pairs);
            }
            match def {
                Def::Number(_) if self.saved_slider_bounds.contains_key(&Id(i)) => {
                    if !self.update_slider_interval(Id(i)) {
                        continue;
                    }
                }
                Def::Lines(LineDef::TangentPoint { .. } | LineDef::TangentLine { .. })
                | Def::ConicPart(
                    ConicPartDef::TangentPoint { .. } | ConicPartDef::TangentLine { .. },
                ) => {
                    if !self.update_tangent(i, TangentDef::of(def).expect("tangent definition")) {
                        continue;
                    }
                }
                Def::Expr(d) => {
                    if !self.update_expr(i, d) {
                        continue;
                    }
                }
                Def::GeneralConic(d) => {
                    if !self.update_conic(i, d, rebuild_records) {
                        continue;
                    }
                }
                Def::IntersectConics(d) => {
                    if !self.update_conic_intersection(i, d, refresh_definitions) {
                        continue;
                    }
                }
                Def::IntersectLineConic(d) => {
                    if !self.update_line_conic(i, d, refresh_definitions) {
                        continue;
                    }
                }
                Def::IntersectLineCircle { .. }
                | Def::ConicPart(ConicPartDef::IntersectLine { .. }) => {
                    let d = LineConicDef::from_indexed(def).expect("indexed line/conic output");
                    if !self.update_line_conic(i, d, refresh_definitions) {
                        continue;
                    }
                }
                Def::Path(d) if d.has_state() => {
                    if !self.update_path(i, d) {
                        continue;
                    }
                }
                Def::Arc(d) if d.has_record() => {
                    if !self.update_arc(i, d, rebuild_records) {
                        continue;
                    }
                }
                Def::Lines(LineDef::CommonTangent(d)) if d.has_state(self) => {
                    if !self.update_common_tangent(i, d) {
                        continue;
                    }
                }
                Def::ImplicitCurve(index) => {
                    self.values[i] = expr::equation::implicit::update(self, index);
                }
                Def::Equation(d) => {
                    if !self.update_equation(i, d) {
                        continue;
                    }
                }
                Def::Conditional {
                    args,
                    kind: Kind::Segment,
                } => {
                    let id = Id(i);
                    let previous = (self.values[i], self.segment_carriers.get(&id).copied());
                    let (value, carrier) = self.conditional_segment(args, previous);
                    if value.identical(&previous.0) && carrier == previous.1 {
                        continue;
                    }
                    self.values[i] = value;
                    self.set_segment_carrier(id, carrier);
                }
                // A redefinition can change the structure the proof reads
                // (a free point put on a path): the reference rebuilds it.
                Def::Prove(d) if refresh_definitions => {
                    self.proof_intersections.replay_rank = Some(rank);
                    // A redefined statement is rewritten again, and the
                    // reference's rebuild then reads the rewritten Boolean.
                    if self.rewrite_proof_statement(d.statement) {
                        if let Def::Expr(e) = self.defs[d.statement.0] {
                            self.update_expr(d.statement.0, e);
                        }
                    }
                    self.values[i] = self.record_proof(Id(i), d);
                    self.proof_intersections.replay_rank = None;
                }
                // The result does not depend on positions.
                Def::Prove(_) => continue,
                Def::Chart(d) => {
                    let mut data = d.compute(self);
                    let id = Id(i);
                    if self.chart(id).is_some_and(|old| old.same(&data)) {
                        continue;
                    }
                    data.bump_revision();
                    self.values[i] = data.value;
                    self.charts.insert(id, data);
                }
                Def::Locus(d) => {
                    if !self.update_locus(i, |c| locus::record(c, d, Id(i))) {
                        continue;
                    }
                }
                Def::Discrete(d) => {
                    if !self.update_locus(i, |c| d.record(c, c.locus(Id(i)))) {
                        continue;
                    }
                }
                Def::Calculus(d) if d.has_record() => {
                    if !self.update_calculus(i, &d) {
                        continue;
                    }
                }
                Def::Optimize(d) => {
                    let d = match algo::optimize::disabled(self, d) {
                        Some(disabled) => {
                            self.defs[i] = Def::Optimize(disabled);
                            disabled
                        }
                        None => d,
                    };
                    let new = algo::optimize::record(self, d);
                    if new.identical(&self.values[i]) {
                        // Nothing downstream can change because of this
                        // object.
                        continue;
                    }
                    self.values[i] = new;
                }
                _ => {
                    if matches!(def, Def::IntersectBoundary(d) if d.retains_output(self)) {
                        continue;
                    }
                    if matches!(def, Def::Point(PointDef::InequalityVertex { .. }))
                        && !self.inequality_helpers.refreshing
                    {
                        self.inequality_helpers.pending.insert(Id(i));
                    }
                    let new = self.compute(def, i);
                    // Redefinition retains the circle through a point when
                    // either input is undefined, including its type for proofs.
                    let reset_circle_record = rebuild_records
                        && !(redefining && matches!(def, Def::CircleThrough { .. }));
                    let record_changed = self.keep_circle_record(i, def, new, reset_circle_record);
                    if let Def::IntersectBoundary(intersection) = def {
                        if let Some(saved) = intersection.with_root(new) {
                            self.defs[i] = Def::IntersectBoundary(saved);
                            self.last_update_requires_replay = true;
                        }
                    }
                    if new.identical(&self.values[i])
                        && !record_changed
                        && !matches!(
                            def.kind(),
                            Kind::Curve | Kind::Image | Kind::Polygon | Kind::PolyLine
                        )
                        && !matches!(
                            def,
                            Def::Conditional {
                                kind: Kind::Circle
                                    | Kind::Conic
                                    | Kind::Arc
                                    | Kind::Sector
                                    | Kind::Locus,
                                ..
                            }
                        )
                    {
                        // An undefined endpoint leaves this vertex at its
                        // previous coordinates, so replay the edit that set them.
                        if matches!(def, Def::Polygon(PolygonDef::RegularVertex { a, b, .. })
                            if new.is_defined() && (self.point(a).is_none() || self.point(b).is_none()))
                        {
                            self.last_update_requires_replay = true;
                        }
                        // Direct placement can define Point(circle) using
                        // retained geometry. The next path algorithm update
                        // clears it even if the circle's record stayed equal.
                        if def.kind() == Kind::Circle && !new.is_defined() {
                            for child in self.children[i].iter() {
                                if matches!(self.def(child), Def::Path(PathDef::On { path }) if path == Id(i))
                                    && self.value(child).is_defined()
                                {
                                    dirty.insert(self.evaluation_rank(child));
                                }
                            }
                        }
                        continue;
                    }
                    // Shape records can change while their summary values stay equal.
                    self.values[i] = new;
                    if !new.is_defined()
                        && (record_changed
                            || matches!(def, Def::Polygon(PolygonDef::RegularVertex { .. })))
                    {
                        self.last_update_requires_replay = true;
                    }
                }
            }
            if let (Propagation::RecordProtocol(before), Some(saved)) = (&mut mode, saved) {
                before.insert(i, saved);
            }
            if !refresh_definitions {
                for child in self.children[i].iter() {
                    dirty.insert(self.evaluation_rank(child));
                }
            }
        }

        self.last_update_requires_replay |= self.inequality_helpers.changed();
        self.dirty = dirty;
        Some(recomputed)
    }

    fn rebuild_self_circle_updates(&mut self) {
        self.self_circle_updates.clear();
        for index in 0..self.self_circle_outputs.len() {
            let output = self.self_circle_outputs[index];
            if let Def::IntersectCircles { c1, c2, .. } = self.def(output) {
                if c1 == c2 {
                    self.track_self_circle_update(c1, output);
                }
            }
        }
    }

    fn track_self_circle_update(&mut self, circle: Id, output: Id) {
        let mut pending = vec![(circle, output)];
        let mut remaining = MAX_REDEFINITION_EDGES;
        while let Some((input, dependent)) = pending.pop() {
            if !self
                .self_circle_updates
                .entry(input)
                .or_default()
                .insert(dependent)
            {
                continue;
            }
            let reads = self.reads(input);
            let Some(next) = remaining.checked_sub(reads.len()) else {
                break;
            };
            remaining = next;
            pending.extend(reads.into_iter().map(|parent| (parent, input)));
        }
    }

    fn refresh_intersection_definition(
        &mut self,
        i: usize,
        def: Def,
        restored_pairs: &mut BTreeSet<(Id, Id)>,
    ) -> Def {
        use algo::intersect::{circles, circles_special_point};
        let (def, key, swap) = match def {
            Def::IntersectLineCircle { .. } | Def::IntersectLineConic(_) => {
                let (line, circle, which) = match def {
                    Def::IntersectLineCircle {
                        line,
                        circle,
                        which,
                    } => (line, circle, which),
                    Def::IntersectLineConic(d)
                        if self.kind(d.line) == Kind::Line
                            && self.kind(d.conic) == Kind::Circle =>
                    {
                        let which = match d.output {
                            algo::intersect::pair::Output::Index(i)
                            | algo::intersect::pair::Output::Slot(i) => i as u8,
                            _ => 0,
                        };
                        if which >= 2 {
                            return def;
                        }
                        (d.line, d.conic, which)
                    }
                    _ => return def,
                };
                let previous = self.hints[i];
                let hint = algo::intersect::hint(self, &def);
                self.hints[i] = hint;
                let key = (line, circle);
                let swap = (!restored_pairs.contains(&key))
                    .then(|| self.restored_line_circle_swap(i, previous, hint, key, which))
                    .flatten();
                (def, key, swap)
            }
            Def::IntersectCircles { c1, c2, which, .. } => {
                let hint = self.hints[i];
                let shared = circles_special_point(self, c1, c2, hint);
                let special = shared.is_some();
                let key = (c1, c2);
                let swap = if restored_pairs.contains(&key) {
                    None
                } else if let Some(point) = shared {
                    // The rebuild restores the labelled slot from the file.
                    // A different point in slot one sends the shared point
                    // to slot two even when it is closer to slot one.
                    (which == 0 && c1 != c2).then(|| {
                        self.point(Id(i)).zip(self.point(point)).is_some_and(
                            |((x, y), (px, py))| {
                                (x - px).hypot(y - py) > crate::tol::STANDARD_PRECISION
                            },
                        )
                    })
                } else {
                    self.closest_restored_slot(i, key, |slot| {
                        circles(self, c1, c2, which ^ slot, false, hint)
                    })
                };
                (
                    Def::IntersectCircles {
                        c1,
                        c2,
                        which,
                        special: Some(special),
                    },
                    key,
                    swap,
                )
            }
            _ => return def,
        };
        if let Some(swap) = swap {
            restored_pairs.insert(key);
            if swap {
                self.intersection_swaps.insert(key);
            } else {
                self.intersection_swaps.remove(&key);
            }
        }
        self.defs[i] = def;
        def
    }

    fn restored_line_circle_swap(
        &self,
        i: usize,
        previous: algo::intersect::Hint,
        hint: algo::intersect::Hint,
        key: (Id, Id),
        which: u8,
    ) -> Option<bool> {
        use algo::intersect::{line_circle, Hint};
        let (line, circle) = key;
        if matches!(hint, Hint::Point(_)) && previous.keeps_line_circle_point(self, circle) {
            // Both old and new hints describe points on the circle. Keep
            // the same named intersection if dependency order picks the
            // other point first after the rebuild.
            return Some(self.intersection_swaps.contains(&key) ^ (previous != hint));
        }
        self.closest_restored_slot(i, key, |slot| {
            line_circle(self, line, circle, which ^ slot, hint)
        })
    }

    fn closest_restored_slot(
        &self,
        i: usize,
        key: (Id, Id),
        candidate: impl Fn(u8) -> Option<Value>,
    ) -> Option<bool> {
        let (x, y) = self.point(Id(i))?;
        let distance = |value: Value| {
            let Value::Point { x: px, y: py } = value.cartesian() else {
                return None;
            };
            Some((x - px).hypot(y - py))
        };
        let a = candidate(0).and_then(distance)?;
        let b = candidate(1).and_then(distance)?;
        Some(b < a || a == b && self.intersection_swaps.contains(&key))
    }

    fn has_text_corner(&self, id: Id) -> bool {
        self.children(id).any(|child| {
            matches!(self.def(child), Def::Point(PointDef::DrawableCorner { source, .. }) if source == id)
        })
    }

    /// Recomputes the expression object `i`; tells whether its children
    /// must follow. An expression compares its raw result, infinities kept.
    /// Kept out of line so that the loop of [`Self::propagate`] stays small.
    #[inline(never)]
    fn update_expr(&mut self, i: usize, d: ExprDef) -> bool {
        let revision = expr::list_record_revision(self, d);
        let (value, changed) = expr::update(self, Id(i), d);
        self.values[i] = value;
        let record_changed = revision.is_some() && revision != expr::list_record_revision(self, d);
        self.last_update_requires_replay |= record_changed;
        changed || record_changed || d.kind() == Kind::Text && self.has_text_corner(Id(i))
    }

    /// Recomputes the conic record of object `i` from its previous record;
    /// tells whether its children must follow: whether the record changed,
    /// bit for bit. Out of line like [`Self::update_expr`].
    #[inline(never)]
    fn update_conic(&mut self, i: usize, d: GeneralConicDef, rebuild: bool) -> bool {
        let Some(slot) = self.conic_slot(i) else {
            return false;
        };
        let fresh = ConicData::new();
        let previous = if rebuild {
            &fresh
        } else {
            &self.conics[slot].1
        };
        let new = d.record(self, previous);
        if new.same(&self.conics[slot].1) {
            return false;
        }
        self.conics[slot].1 = new;
        self.values[i] = algo::general_conic::value(d.kind(), &new);
        self.last_update_requires_replay |= !new.defined;
        true
    }

    /// Match intersection branches of two `Kind::Circle` objects to their
    /// previous positions on edits. Other conics retain their default mode.
    pub fn set_conic_intersections_continuous(&mut self, continuous: bool) {
        self.continuous_conic_intersections = continuous;
    }

    fn conic_intersection_slot(&self, i: usize) -> Option<usize> {
        self.conic_intersections
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
    }

    fn common_tangent_slot(&self, i: usize) -> Option<usize> {
        self.limited_common_tangents
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
    }

    #[inline(never)]
    fn update_common_tangent(&mut self, i: usize, d: CommonTangentDef) -> bool {
        let slot = self
            .common_tangent_slot(i)
            .expect("limited common tangent state");
        let mut state = std::mem::take(&mut self.limited_common_tangents[slot].1);
        let value = state.update(self, d).unwrap_or(Value::Undefined);
        self.limited_common_tangents[slot].1 = state;
        let changed = !value.identical(&self.values[i]);
        self.values[i] = value;
        changed
    }

    fn create_conic_intersection(&self, d: PairDef) -> PairState {
        let previous = d
            .previous(self)
            .and_then(|id| self.conic_intersection_slot(id.0))
            .map(|slot| &self.conic_intersections[slot].1);
        d.create(self, previous, self.continuous_conic_intersections)
    }

    pub(crate) fn preview_conic_intersection(&self, d: PairDef) -> Option<Value> {
        let d = d.oriented(self);
        self.create_conic_intersection(d).value(d, self)
    }

    #[inline(never)]
    fn update_conic_intersection(
        &mut self,
        i: usize,
        d: PairDef,
        refresh_definitions: bool,
    ) -> bool {
        let Some(slot) = self.conic_intersection_slot(i) else {
            return false;
        };
        // The latest output carries the parent solver state. This also lets
        // an indexed output added after the full intersection initialize the
        // parent's permutation. Removing it on rollback leaves the earlier
        // parent state intact.
        let latest = d
            .previous(self)
            .and_then(|id| self.conic_intersection_slot(id.0))
            .unwrap_or(slot);
        let mut parent = std::mem::take(&mut self.conic_intersections[latest].1);
        parent.refresh(d, self, self.continuous_conic_intersections);
        if refresh_definitions {
            parent.refresh_definition(d, self);
        }
        let new = if latest == slot {
            parent.value(d, self)
        } else {
            let mut output = parent.for_output(&self.conic_intersections[slot].1);
            let value = output.value(d, self);
            self.conic_intersections[slot].1 = output;
            value
        }
        .unwrap_or(Value::Undefined);
        self.conic_intersections[latest].1 = parent;
        let changed = !new.identical(&self.values[i]);
        self.values[i] = new;
        changed
    }

    /// Saves the branch history of a line/conic intersection or tangent.
    /// Replaying a script at different initial positions can choose other
    /// slots from the same geometric solutions.
    pub fn intersection_state(&self, id: Id) -> Option<IntersectionState> {
        let def = *self.defs.get(id.0)?;
        let history = if let Some(d) = TangentDef::of(def) {
            let slot = self.tangent_slot(id.0)?;
            IntersectionHistory::Tangent(
                d,
                self.tangent_outputs[self.tangent_outputs[slot].helper].state,
            )
        } else {
            let d = match def {
                Def::IntersectLineConic(d) => d,
                _ => LineConicDef::from_indexed(def)?,
            };
            let slot = self.line_conic_slot(id.0)?;
            let mut state = self.line_conic_outputs[self.line_conic_pairs[&d.key()]].state;
            state.set_nearest(self.line_conic_outputs[slot].state.nearest());
            IntersectionHistory::LineConic(d, state)
        };
        Some(IntersectionState(history))
    }

    /// Restores the branch history of an output rebuilt from the same
    /// definition. The solver evaluates the current operands again.
    pub fn restore_intersection_state(
        &mut self,
        id: Id,
        state: IntersectionState,
    ) -> Result<usize, Error> {
        let target = self.intersection_state(id).ok_or(Error::UnknownObject)?;
        if !state.same_definition(target) {
            return Err(Error::NotFree(self.labels[id.0].clone()));
        }
        let value = match (state.0, target.0) {
            (
                IntersectionHistory::LineConic(_, mut state),
                IntersectionHistory::LineConic(d, _),
            ) => {
                let slot = self.line_conic_slot(id.0).expect("line/conic output");
                let latest = self.line_conic_pairs[&d.key()];
                let value = state.restore(d, self);
                self.line_conic_outputs[slot]
                    .state
                    .set_nearest(state.nearest());
                state.set_nearest(self.line_conic_outputs[latest].state.nearest());
                self.line_conic_outputs[latest].state = state;
                value
            }
            (IntersectionHistory::Tangent(_, mut state), IntersectionHistory::Tangent(d, _)) => {
                let slot = self.tangent_slot(id.0).expect("tangent output");
                let helper = self.tangent_outputs[slot].helper;
                state.restore(self, d);
                self.tangent_outputs[helper].state = state;
                state.value(d)
            }
            _ => return Err(Error::NotFree(self.labels[id.0].clone())),
        };
        let value = value.unwrap_or(Value::Undefined);
        if value.identical(&self.values[id.0]) {
            return Ok(0);
        }
        self.values[id.0] = value;
        Ok(self.propagate(id))
    }

    fn tangent_slot(&self, i: usize) -> Option<usize> {
        self.tangent_outputs
            .binary_search_by_key(&i, |output| output.owner)
            .ok()
    }

    fn add_tangent_output(&mut self, id: Id, d: TangentDef, owner: Option<Id>) -> Value {
        let slot = self.tangent_outputs.len();
        let helper = owner
            .filter(|owner| {
                TangentDef::of(self.defs[owner.0]).is_some_and(|other| other.same_algorithm(d))
            })
            .and_then(|owner| self.tangent_slot(owner.0))
            .map_or(slot, |owner| self.tangent_outputs[owner].helper);
        let mut state = TangentState::default();
        let value = if helper == slot {
            state.refresh(self, d, self.continuous_conic_intersections);
            state.value(d)
        } else {
            self.tangent_outputs[helper].state.value(d)
        };
        self.tangent_outputs.push(TangentOutput {
            owner: id.0,
            helper,
            state,
        });
        value.unwrap_or(Value::Undefined)
    }

    pub(crate) fn tangent_contact(&self, line: Id, conic: Id) -> Option<algo::intersect::Pt> {
        let d = TangentDef::of(*self.defs.get(line.0)?)?;
        if d.conic != conic {
            return None;
        }
        let slot = self.tangent_slot(line.0)?;
        let mut state = self.tangent_outputs[self.tangent_outputs[slot].helper].state;
        state.refresh(self, d, self.continuous_conic_intersections);
        Some(state.contact(d))
    }

    #[inline(never)]
    fn update_tangent(&mut self, i: usize, d: TangentDef) -> bool {
        let slot = self.tangent_slot(i).expect("tangent output");
        let helper = self.tangent_outputs[slot].helper;
        let mut state = self.tangent_outputs[helper].state;
        state.refresh(self, d, self.continuous_conic_intersections);
        let new = state.value(d).unwrap_or(Value::Undefined);
        self.tangent_outputs[helper].state = state;
        let changed = !new.identical(&self.values[i]);
        self.values[i] = new;
        changed
    }

    fn line_conic_slot(&self, i: usize) -> Option<usize> {
        self.line_conic_outputs
            .binary_search_by_key(&i, |output| output.owner)
            .ok()
    }

    pub(crate) fn preview_line_conic(&self, d: LineConicDef) -> Option<Value> {
        let mut state = self.line_conic_pairs.get(&d.key()).map_or_else(
            || LineConicState::create(d, self, None, self.continuous_conic_intersections),
            |&slot| self.line_conic_outputs[slot].state,
        );
        state.refresh(d, self, self.continuous_conic_intersections, false);
        state.value(d, self)
    }

    fn add_line_conic_output(&mut self, id: Id, d: LineConicDef) -> Value {
        let prior = self.line_conic_pairs.get(&d.key()).copied();
        let mut state = LineConicState::create(
            d,
            self,
            prior.map(|slot| self.line_conic_outputs[slot].state),
            self.continuous_conic_intersections,
        );
        state.label_output(d);
        let value = state.value(d, self).unwrap_or(Value::Undefined);
        self.line_conic_pairs
            .insert(d.key(), self.line_conic_outputs.len());
        self.line_conic_outputs.push(LineConicOutput {
            owner: id.0,
            key: d.key(),
            prior,
            state,
        });
        value
    }

    #[inline(never)]
    fn update_line_conic(&mut self, i: usize, d: LineConicDef, redefined: bool) -> bool {
        let slot = self.line_conic_slot(i).expect("line/conic output");
        let latest = self.line_conic_pairs[&d.key()];
        let mut state = self.line_conic_outputs[latest].state;
        if redefined && self.kind(d.line) == Kind::Line && self.kind(d.conic) == Kind::Circle {
            state.restore_order(self.intersection_swaps.contains(&d.key()));
        }
        state.refresh(d, self, self.continuous_conic_intersections, redefined);
        let nearest = state.nearest();
        state.set_nearest(self.line_conic_outputs[slot].state.nearest());
        let new = state.value(d, self).unwrap_or(Value::Undefined);
        if slot != latest {
            self.line_conic_outputs[slot]
                .state
                .set_nearest(state.nearest());
            state.set_nearest(nearest);
        }
        self.line_conic_outputs[latest].state = state;
        let changed = !new.identical(&self.values[i]);
        self.values[i] = new;
        changed
    }

    /// Recomputes the state of the point on a path `i`; tells whether its
    /// children must follow: whether the state changed, bit for bit. Out of
    /// line like [`Self::update_expr`].
    #[inline(never)]
    fn update_path(&mut self, i: usize, d: PathDef) -> bool {
        let Some(slot) = self.path_slot(i) else {
            return false;
        };
        let new = d.update(self, &self.paths[slot].1);
        if new.same(&self.paths[slot].1) {
            return false;
        }
        self.paths[slot].1 = new;
        self.values[i] = PathDef::value(&new);
        true
    }

    /// Recomputes the record of the arc or sector `i`; tells whether its
    /// children must follow: whether the record changed or is undefined.
    /// Out of line like [`Self::update_expr`].
    #[inline(never)]
    fn update_arc(&mut self, i: usize, d: ArcDef, rebuild: bool) -> bool {
        let Some(slot) = self.arc_slot(i) else {
            return false;
        };
        let new = d.record(self, (!rebuild).then_some(&self.arcs[slot].1));
        if new.same(&self.arcs[slot].1) {
            // A point can be placed directly on an undefined arc. Its next
            // AlgoPointOnPath update clears it even when the arc stays undefined.
            return !new.defined;
        }
        self.last_update_requires_replay |=
            !new.defined || !new.is_curved() || new.endpoint_parameters_changed(&self.arcs[slot].1);
        self.arcs[slot].1 = new;
        self.values[i] = algo::arc::value(d.kind() == Kind::Sector, &new);
        true
    }

    /// Recomputes the calculus object `i`; tells whether its children
    /// must follow. A function always counts as changed, like an
    /// expression function (see `expr::store`).
    fn update_calculus(&mut self, i: usize, d: &CalculusDef) -> bool {
        let Some(slot) = self.calculus_slot(i) else {
            return false;
        };
        let (value, record) = algo::calculus::refresh(self, d);
        self.values[i] = value;
        self.calculus[slot].1 = record;
        true
    }

    /// Recomputes the equation object `i` from its coefficients; tells
    /// whether its children must follow: whether any coefficient changed,
    /// bit for bit, or its display uses the original expression sides.
    /// A conic is classified again from its record, as
    /// `AlgoDependentConic.compute` calls `setCoeffs`. Out of line like
    /// [`Self::update_expr`].
    #[inline(never)]
    fn update_equation(&mut self, i: usize, d: EquationDef) -> bool {
        let prev = if d.is_conic() {
            self.conic_slot(i)
                .map(|slot| self.conics[slot].1)
                .unwrap_or_default()
        } else {
            ConicData::new()
        };
        let (changed, value, record) = crate::expr::equation::recompute(self, d, prev);
        let user_form =
            if let Some((EquationForm::User, revision)) = self.equation_forms.get_mut(&Id(i)) {
                *revision = revision.wrapping_add(1);
                true
            } else {
                false
            };
        if !changed {
            return user_form;
        }
        if let Some(record) = record {
            if let Some(slot) = self.conic_slot(i) {
                self.conics[slot].1 = record;
            }
        }
        self.values[i] = value;
        true
    }

    /// Where the record of object `i` is kept in `arcs`.
    fn arc_slot(&self, i: usize) -> Option<usize> {
        self.arcs.binary_search_by_key(&i, |&(owner, _)| owner).ok()
    }

    /// The slot of the calculus record of object `i`, if it has one.
    pub(crate) fn calculus_slot(&self, i: usize) -> Option<usize> {
        self.calculus
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
    }

    /// The calculus record of object `id`: the tree its function value
    /// samples, or the elements of its coefficient list. `None` for any
    /// other object.
    pub(crate) fn calculus_record(&self, id: Id) -> Option<&CalcRecord> {
        self.calculus_slot(id.0).map(|slot| &self.calculus[slot].1)
    }

    /// The record of the arc or sector `id`: its conic, the range of its
    /// parameter and its measures. `None` for any other object.
    pub fn arc(&self, id: Id) -> Option<ArcData> {
        let source = self.conditional_source(id)?;
        let Def::Arc(d) = self.defs.get(source.0)? else {
            return None;
        };
        if !d.has_record() {
            return None;
        }
        let arc = self.arcs[self.arc_slot(source.0)?].1;
        Some(if source == id {
            arc
        } else {
            arc.conditional_copy()
        })
    }

    /// Where the state of object `i` is kept in `paths`.
    fn path_slot(&self, i: usize) -> Option<usize> {
        self.paths
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
    }

    /// The circle that `id` still holds while its circle command leaves
    /// the last record in place (`GeoConicND` keeps the half axes,
    /// midpoint and type and only clears `defined`). `None` for a defined
    /// circle, for one that never had a record and for other objects.
    pub(crate) fn circle_record(&self, id: Id) -> Option<ConicData> {
        let slot = self.circle_records.partition_point(|&(o, _)| o < id.0);
        match self.circle_records.get(slot) {
            Some(&(owner, (cx, cy, r))) if owner == id.0 => Some(ConicData::circle(cx, cy, r)),
            _ => None,
        }
    }

    /// A circle whose Cartesian centre is infinite still has a defined
    /// reference value, even though the kernel cannot draw its geometry.
    pub(crate) fn has_infinite_circle_center(&self, id: Id) -> bool {
        matches!(algo::circle::invalid_record(self, self.def(id)), Some((x, y, r))
            if (x.is_infinite() || y.is_infinite()) && !r.is_nan())
    }

    /// Keeps the record of a circle command for [`Self::circle_record`],
    /// before `new` replaces the value of object `i`.
    fn keep_circle_record(&mut self, i: usize, def: Def, new: Value, rebuilt: bool) -> bool {
        if new.is_defined() {
            return !self.values[i].is_defined()
                && matches!(new, Value::Circle { .. })
                && self.forget_circle_record(i);
        }
        if let Some(record) = algo::circle::invalid_record(self, def) {
            return self.store_circle_record(i, record);
        }
        match algo::circle::keeps_record(self, def) {
            None => false,
            Some(false) => self.forget_circle_record(i),
            Some(true) if rebuilt => self.forget_circle_record(i),
            Some(true) => match self.values[i] {
                Value::Circle { cx, cy, r } => self.store_circle_record(i, (cx, cy, r)),
                _ => false,
            },
        }
    }

    fn store_circle_record(&mut self, i: usize, record: (f64, f64, f64)) -> bool {
        match self
            .circle_records
            .binary_search_by_key(&i, |&(owner, _)| owner)
        {
            Ok(slot) => {
                let (x, y, r) = self.circle_records[slot].1;
                if (x.to_bits(), y.to_bits(), r.to_bits())
                    == (record.0.to_bits(), record.1.to_bits(), record.2.to_bits())
                {
                    return false;
                }
                self.circle_records[slot].1 = record;
            }
            Err(slot) => self.circle_records.insert(slot, (i, record)),
        }
        true
    }

    /// Reloading an undo snapshot rebuilds circles from the current inputs.
    /// A setter that returns early in that state has no earlier circle to keep.
    pub fn reset_circle_records(&mut self) -> bool {
        if self.circle_records.is_empty() {
            return false;
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        let mut changed = false;
        let mut records = std::mem::take(&mut self.circle_records);
        records.retain_mut(|(i, record)| {
            let fresh = algo::circle::invalid_record(self, self.defs[*i]);
            if let Some(fresh) = fresh {
                if record.0.to_bits() == fresh.0.to_bits()
                    && record.1.to_bits() == fresh.1.to_bits()
                    && (record.2.to_bits() == fresh.2.to_bits()
                        || (record.2.is_nan() && fresh.2.is_nan()))
                {
                    return true;
                }
                *record = fresh;
            }
            changed = true;
            for child in self.children[*i].iter() {
                dirty.insert(self.evaluation_rank(child));
            }
            fresh.is_some()
        });
        self.circle_records = records;
        self.propagate_dirty(dirty, 0);
        changed
    }

    /// Reloading an undo snapshot creates fresh arc records in dependency order.
    pub fn reset_arc_records(&mut self) -> bool {
        if self.arcs.is_empty() {
            return false;
        }
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        for &(i, _) in &self.arcs {
            dirty.insert(self.evaluation_rank(Id(i)));
        }
        self.propagate_dirty_with(dirty, 0, Propagation::Reload);
        true
    }

    /// Reloading an undo snapshot creates Transpose outputs with fresh flags.
    pub fn reset_transpose_flags(&mut self) -> bool {
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.len());
        let mut changed = false;
        for (i, &def) in self.defs.iter().enumerate() {
            if matches!(def, Def::Expr(d) if expr::reset_transpose_flags(self, d)) {
                dirty.insert(self.evaluation_rank(Id(i)));
                changed = true;
            }
        }
        if changed {
            self.propagate_dirty(dirty, 0);
        } else {
            self.dirty = dirty;
        }
        changed
    }

    fn forget_circle_record(&mut self, i: usize) -> bool {
        if let Ok(slot) = self
            .circle_records
            .binary_search_by_key(&i, |&(owner, _)| owner)
        {
            self.circle_records.remove(slot);
            true
        } else {
            false
        }
    }

    /// The state of the point on a path `id` (made by `Point(path)` or
    /// `Point(path, t)`): it depends on how the point was moved, so a
    /// construction built again from the definitions gets it back only with
    /// [`Self::restore_path_state`]. `None` for any other object.
    pub fn path_state(&self, id: Id) -> Option<PathState> {
        match self.defs.get(id.0)? {
            Def::Path(d) if d.has_state() => self.path_slot(id.0).map(|slot| self.paths[slot].1),
            _ => None,
        }
    }

    pub(crate) fn add_saved_path_point(
        &mut self,
        label: &str,
        path: Id,
        x: f64,
        y: f64,
        bounds: Option<[f64; 2]>,
    ) -> Result<Id, Error> {
        if bounds.is_some_and(|[min, max]| min >= max || self.kind(path) != Kind::Function) {
            return Err(Error::NotFinite);
        }
        let state = PathState::saved(self, path, x, y, bounds);
        self.add_with_path_state(label, Def::Path(PathDef::On { path }), Some(state), None)
    }

    pub(crate) fn add_saved_curve_point(
        &mut self,
        label: &str,
        path: Id,
        t: f64,
    ) -> Result<Id, Error> {
        if self.kind(path) != Kind::Curve || !t.is_finite() {
            return Err(Error::NotFinite);
        }
        let state = PathState::saved_curve(self, path, t);
        self.add_with_path_state(label, Def::Path(PathDef::On { path }), Some(state), None)
    }

    /// Gives the point on a path `id` the state `state` read from another
    /// construction with [`Self::path_state`], and recomputes everything
    /// that depends on it. Returns how many dependent objects were
    /// recomputed. The point's value follows from the state as it is, and
    /// the next change of its path moves it from there.
    pub fn restore_path_state(&mut self, id: Id, state: PathState) -> Result<usize, Error> {
        let def = *self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        let slot = match def {
            Def::Path(d) if d.has_state() => self.path_slot(id.0),
            _ => None,
        }
        .ok_or_else(|| Error::NotFree(self.labels[id.0].clone()))?;
        if !state.is_valid() {
            return Err(Error::NotFinite);
        }
        if state.same(&self.paths[slot].1) {
            return Ok(0);
        }
        self.paths[slot].1 = state;
        self.values[id.0] = PathDef::value(&state);
        Ok(self.propagate(id))
    }

    /// The state the point on a path `id` gets when its path is followed
    /// from the state `from` (`AlgoPointOnPath.compute`), without changing
    /// the construction: what an update of the point computes when its
    /// state before was `from`. For checks of the update order; `None` for
    /// any other object.
    pub fn follow_path_from(&self, id: Id, from: PathState) -> Option<PathState> {
        match *self.defs.get(id.0)? {
            Def::Path(d) if d.has_state() => Some(d.update(self, &from)),
            _ => None,
        }
    }

    /// Whether [`Self::move_point`] accepts `id`: a free point, a free
    /// vector, or a point made by `Point(path)`, which moves along its path.
    pub fn is_movable(&self, id: Id) -> bool {
        matches!(
            self.defs.get(id.0),
            Some(
                Def::FreePoint { .. }
                    | Def::Vector(VectorDef::Free { .. })
                    | Def::Path(
                        PathDef::On { .. }
                            | PathDef::InRegion { .. }
                            | PathDef::RigidEndpoint { .. }
                            | PathDef::FixedEndpoint { .. }
                    )
            )
        )
    }

    /// Where the record of object `i` is kept in `conics`.
    fn conic_slot(&self, i: usize) -> Option<usize> {
        self.conics
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
    }

    /// The conic state of `id`, as GeoGebra's `GeoConicND` would hold it:
    /// the record of a general conic or of a transformed circle, and for
    /// any other defined circle the record GeoGebra's circle commands make.
    /// `None` for other objects and for an undefined circle that keeps no
    /// record. A record may be undefined ([`ConicData::defined`]).
    pub fn conic(&self, id: Id) -> Option<ConicData> {
        let id = self.conditional_source(id)?;
        match self.defs.get(id.0)? {
            Def::ImplicitCurve(index)
                if self.kind(id) == Kind::Conic && self.value(id).is_defined() =>
            {
                self.implicit_curves[*index as usize].1.conic
            }
            Def::GeneralConic(_) | Def::Equation(_) => {
                self.conic_slot(id.0).map(|slot| self.conics[slot].1)
            }
            _ => match self.values[id.0] {
                Value::Circle { cx, cy, r } => Some(ConicData::circle(cx, cy, r)),
                _ => None,
            },
        }
    }

    /// The stored fields a reader sees when it does not check definedness.
    /// General conics already carry these fields in their current record.
    pub(crate) fn conic_fields(&self, id: Id) -> Option<ConicData> {
        self.conic(id).or_else(|| {
            let mut data = self.circle_record(id)?;
            // A NaN radius writes a defined empty quadric. A negative
            // radius leaves earlier geometry in an undefined record.
            data.defined = data.half_axes[0].is_nan();
            Some(data)
        })
    }

    /// The active source of a conditional object. Branches precede their
    /// result in the construction.
    pub(crate) fn conditional_source(&self, mut id: Id) -> Option<Id> {
        let end = expr::carrier_deadline();
        loop {
            let Some(Def::Conditional { args, .. }) = self.defs.get(id.0) else {
                return Some(id);
            };
            if !self.values[id.0].is_defined() {
                return None;
            }
            let branch = expr::conditional_branch(self, self.list(*args), end)?;
            if branch.0 >= id.0 {
                return None;
            }
            id = branch;
        }
    }

    /// The current contents of a text object.
    pub fn is_independent_text(&self, id: Id) -> bool {
        self.defs
            .get(id.0)
            .is_some_and(|def| def.kind() == Kind::Text)
            && expr::is_independent_text(self, id)
    }

    /// Original source retained for definitions that need it for display.
    pub fn construction_source(&self, id: Id) -> Option<&str> {
        self.construction_sources.get(&id).map(String::as_str)
    }

    pub(crate) fn remember_construction_source(&mut self, id: Id, source: &str) {
        self.construction_sources.insert(id, source.to_owned());
    }

    /// The displayed value of a symbolic computation.
    pub fn symbolic_value(&self, id: Id) -> Option<String> {
        expr::symbolic_value(self, id)
    }

    pub fn text(&self, id: Id) -> Option<&str> {
        expr::text_value(self, id)
    }

    /// The current numerical findings of a Relation command.
    pub fn relation_reports(&self, id: Id) -> Option<Vec<algo::predicate::RelationReport>> {
        expr::relation_reports(self, id)
    }

    /// Evaluates a two-variable function at the given coordinates.
    pub fn function_value_xy(&self, id: Id, x: f64, y: f64) -> Option<f64> {
        if !matches!(self.value(id), Value::FunctionNVar) {
            return None;
        }
        Some(expr::IntersectionWork::new().sample_system(self, id, x, &[y]))
    }

    /// The coefficient entries of the equation object `id` (see
    /// [`Def::Equation`]), `None` for any other object. The entries are
    /// indices into the construction's expression table; a number of
    /// `Vec<u32>` length 3 makes a line and length 6 a conic.
    pub fn equation_coefficients(&self, id: Id) -> Option<Vec<u32>> {
        match self.defs.get(id.0)? {
            Def::Equation(d) => self
                .equations
                .get(d.index as usize)
                .map(|(_, r)| r.coeffs.clone()),
            _ => None,
        }
    }

    pub(crate) fn check(&self, def: &Def) -> Result<(), Error> {
        match *def {
            Def::XAxis
            | Def::YAxis
            | Def::Boolean(_)
            | Def::Button
            | Def::InputBox(None)
            | Def::PenStroke => Ok(()),
            Def::Chart(d) => d.check(self),
            Def::Prove(d) => d.check(self),
            Def::Image(list) => self.expect_list(list, POINT),
            Def::Conditional { args, kind } => {
                let ids = self.list(args);
                let expected = expr::conditional_kind(kind).ok_or(Error::UnknownObject)?;
                if ids.len() < 2 {
                    return Err(Error::UnknownObject);
                }
                let (pairs, fallback) = ids.as_chunks::<2>();
                for &[condition, branch] in pairs {
                    self.expect(condition, &[Kind::Boolean])?;
                    self.expect(branch, expected)?;
                }
                for &branch in fallback {
                    self.expect(branch, expected)?;
                }
                Ok(())
            }
            Def::InputBox(Some(linked)) => {
                if self.defs.get(linked.0).is_some() || axis::get(linked).is_some() {
                    Ok(())
                } else {
                    Err(Error::UnknownObject)
                }
            }
            Def::FreePoint { x, y } => {
                if x.is_finite() && y.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            Def::Number(v) | Def::Angle(v) => {
                if v.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            Def::Line(p, q) | Def::Segment(p, q) | Def::Midpoint(p, q) | Def::Distance(p, q) => {
                self.expect(p, POINT)?;
                self.expect(q, POINT)
            }
            Def::Perpendicular { through, to } | Def::Parallel { through, to } => {
                self.expect(through, POINT)?;
                self.expect(to, LINE_LIKE)
            }
            Def::CircleThrough { center, through } => {
                self.expect(center, POINT)?;
                self.expect(through, POINT)
            }
            Def::CircleRadius { center, radius } => {
                self.expect(center, POINT)?;
                self.expect(radius, NUMBER)
            }
            Def::IntersectLines(l, m) => {
                self.expect(l, LINE)?;
                self.expect(m, LINE)
            }
            Def::IntersectLineCircle {
                line,
                circle,
                which,
            } => {
                self.expect(line, LINE)?;
                self.expect(circle, CIRCLE)?;
                check_index(which)
            }
            Def::IntersectCircles { c1, c2, which, .. } => {
                self.expect(c1, CIRCLE)?;
                self.expect(c2, CIRCLE)?;
                check_index(which)
            }
            Def::IntersectConics(d) => d.check(self),
            Def::IntersectLineConic(d) => d.check(self),
            Def::IntersectBoundary(d) => d.check(self),
            Def::Lines(d) => d.check(self),
            Def::Conic(d) => d.check(self),
            Def::Measure(d) => d.check(self),
            Def::AngleObject(d) => d.check(self),
            Def::Transform(d) => d.check(self),
            Def::Vector(d) => d.check(self),
            Def::Predicate(d) => d.check(self),
            Def::Point(d) => d.check(self),
            // the tree was checked when it was parsed or handed in
            Def::Expr(d) => expr::check_handle(self, d),
            Def::Polygon(d) => d.check(self),
            Def::GeneralConic(d) => d.check(self),
            Def::ConicPart(d) => d.check(self),
            Def::Path(d) => d.check(self),
            Def::Arc(d) => d.check(self),
            // the coefficient entries were checked when they were parsed
            Def::Equation(d) => crate::expr::equation::check(self, d),
            Def::ImplicitCurve(index) => self
                .implicit_curves
                .get(index as usize)
                .filter(|(owner, _)| *owner == self.len())
                .map(|_| ())
                .ok_or(Error::UnknownObject),
            Def::Locus(d) => d.check(self),
            Def::Optimize(d) => d.check(self),
            Def::Calculus(d) => d.check(self),
            Def::Discrete(d) => d.check(self),
            Def::Analysis(d) => d.check(self),
        }
    }

    pub(crate) fn expect(&self, id: Id, expected: &'static [Kind]) -> Result<(), Error> {
        if !self.exists(id) {
            return Err(Error::UnknownObject);
        }
        let found = self.kind(id);
        if expected.contains(&found) || (found == Kind::Angle && expected.contains(&Kind::Number)) {
            Ok(())
        } else {
            Err(Error::WrongKind {
                label: self.label(id).to_string(),
                expected,
                found,
            })
        }
    }

    /// `def` is the definition of the object at index `i`.
    fn compute(&self, def: Def, i: usize) -> Value {
        if let Def::Analysis(AnalysisDef::Spline(input)) = def {
            return self
                .analysis
                .spline_object(self, i, input)
                .unwrap_or(Value::Undefined);
        }
        if let Def::PenStroke = def {
            return self
                .strokes
                .get(&Id(i))
                .map_or(Value::Undefined, algo::stroke::StrokeData::value);
        }
        if let Def::Polygon(PolygonDef::RegularVertex { a, b, n, k }) = def {
            return algo::polygon::regular_vertex(self, a, b, n, k, self.values.get(i).copied())
                .unwrap_or(Value::Undefined);
        }
        self.compute_with(def, |c| c.hints[i])
    }

    /// Computes `def` with the hint that `hint` gives, which is only asked
    /// for by intersections.
    fn compute_with(&self, def: Def, hint: impl Fn(&Self) -> algo::intersect::Hint) -> Value {
        match self.try_compute(def, hint) {
            Some(v) if v.is_valid() => v,
            _ => Value::Undefined,
        }
    }

    /// [`Self::try_compute`] for a command inside an expression. Out of
    /// line, so that `try_compute` stays inlined in the update loop.
    /// `hint` is the intersection hint taken when the expression was
    /// parsed; it stands for the entry in `hints` that an object has.
    #[inline(never)]
    pub(crate) fn command_value(&self, def: Def, hint: algo::intersect::Hint) -> Option<Value> {
        let hint = if matches!(def, Def::IntersectLineCircle { .. }) {
            algo::intersect::Hint::Lookup
        } else {
            hint
        };
        self.try_compute(def, |_| hint).map(Value::cartesian)
    }

    /// `None` means the object is undefined for the current inputs.
    #[inline(always)]
    fn try_compute(
        &self,
        def: Def,
        hint: impl Fn(&Self) -> algo::intersect::Hint,
    ) -> Option<Value> {
        match def {
            Def::XAxis | Def::YAxis => axis::definition(def).map(|axis| axis.value),
            Def::FreePoint { x, y } => Some(Value::Point { x, y }),
            Def::Number(v) | Def::Angle(v) => Some(Value::Number(v)),
            Def::Boolean(v) => Some(Value::Bool(v)),
            Def::Conditional { args, kind } => {
                let branch =
                    expr::conditional_branch(self, self.list(args), expr::carrier_deadline())?;
                match (kind, self.stored_value(branch)) {
                    // `GeoLine.set` copies the coefficients of a segment.
                    (Kind::Line, Value::Segment { .. }) => {
                        let (x, y, z) = self.raw_carrier(branch)?;
                        algo::line_value(x, y, z)
                    }
                    (_, value) => Some(value),
                }
            }
            Def::Button => Some(Value::Button),
            Def::InputBox(_) => Some(Value::InputBox),
            Def::Chart(d) => Some(d.compute(self).value),
            // the stroke keeps its value in its record (see `compute`)
            Def::PenStroke => None,
            Def::Image(list) => self
                .list(list)
                .iter()
                .all(|&id| self.stored_value(id).is_defined())
                .then_some(Value::Image),
            Def::Line(p, q) => algo::basic::line(self, p, q),
            Def::Segment(p, q) => algo::basic::segment(self, p, q),
            Def::Midpoint(p, q) => algo::basic::midpoint(self, p, q),
            Def::Distance(p, q) => algo::basic::distance(self, p, q),
            Def::Perpendicular { through, to } => algo::basic::perpendicular(self, through, to),
            Def::Parallel { through, to } => algo::basic::parallel(self, through, to),
            Def::CircleThrough { center, through } => algo::circle::through(self, center, through),
            Def::CircleRadius { center, radius } => algo::circle::with_radius(self, center, radius),
            Def::IntersectLines(l, m) => algo::intersect::lines(self, l, m),
            Def::IntersectLineCircle {
                line,
                circle,
                which,
            } => {
                let swapped = self.intersection_swaps.contains(&(line, circle));
                algo::intersect::line_circle(
                    self,
                    line,
                    circle,
                    which ^ u8::from(swapped),
                    hint(self),
                )
            }
            Def::IntersectCircles {
                c1,
                c2,
                which,
                special,
            } => {
                let swapped = self.intersection_swaps.contains(&(c1, c2));
                algo::intersect::circles(
                    self,
                    c1,
                    c2,
                    which ^ u8::from(swapped),
                    special == Some(true),
                    hint(self),
                )
            }
            Def::IntersectConics(d) => d.compute(self),
            Def::IntersectLineConic(d) => d.compute(self),
            Def::IntersectBoundary(d) => d.compute(self),
            Def::Lines(d) => d.compute(self),
            Def::Conic(d) => d.compute(self),
            Def::Measure(d) => d.compute(self),
            Def::AngleObject(d) => d.compute(self),
            Def::Transform(d) => d.compute(self),
            Def::Vector(d) => d.compute(self),
            Def::Predicate(d) => d.compute(self),
            Def::Prove(_) => None,
            Def::Point(d) => d.compute(self),
            Def::Expr(d) => Some(expr::current(self, d)),
            Def::Polygon(d) => d.compute(self),
            Def::GeneralConic(d) => d.compute(self),
            Def::ConicPart(d) => d.compute(self),
            Def::Path(d) => d.compute(self),
            Def::Arc(d) => d.compute(self),
            Def::Equation(d) => Some(expr::equation::current(self, d)),
            Def::ImplicitCurve(_) => None,
            Def::Locus(d) => d.compute(self),
            Def::Optimize(d) => d.compute(self),
            Def::Calculus(d) => d.compute(self),
            Def::Discrete(_) => None,
            Def::Analysis(d) => d.compute(self),
        }
    }

    pub(crate) fn point(&self, id: Id) -> Option<(f64, f64)> {
        self.cartesian_point(id)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
    }

    /// A Cartesian point can have an infinite coordinate with weight 1.
    /// Projective points at infinity have no Cartesian coordinates.
    pub(crate) fn cartesian_point(&self, id: Id) -> Option<(f64, f64)> {
        match self.value(id) {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }
    }

    /// A line with `(a, b)` of unit length (see [`Value::unit_line`]).
    #[allow(dead_code)] // part of the accessor set; every rule here reads lines raw
    pub(crate) fn line(&self, id: Id) -> Option<(f64, f64, f64)> {
        self.value(id).unit_line()
    }

    /// A line's raw coefficients, as GeoGebra holds them (see [`Value::Line`]).
    /// For code that follows an upstream formula on raw coefficients.
    pub(crate) fn raw_line(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.value(id) {
            Value::Line { a, b, c } => Some((a, b, c)),
            _ => None,
        }
    }

    /// The infinite line that a line or a segment lies on, with `(a, b)` of
    /// unit length. `None` for a segment whose end points coincide.
    #[allow(dead_code)] // part of the accessor set; every rule here reads carriers raw
    pub(crate) fn carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.value(id) {
            Value::Segment { .. } if self.segment_carriers.contains_key(&id) => {
                let (a, b, c) = self.segment_carriers[&id];
                Value::Line { a, b, c }.unit_line()
            }
            Value::Segment { x1, y1, x2, y2 } => line_through((x1, y1), (x2, y2)),
            v => v.unit_line(),
        }
    }

    /// The raw coefficients of the line that a line, ray or segment lies on. For
    /// a segment they are those of `GeoVec3D.lineThroughPoints` from its start
    /// to its end point, which is what GeoGebra's `GeoSegment` holds; they are
    /// all zero when the end points coincide.
    pub(crate) fn raw_carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.value(id) {
            Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => Some((a, b, c)),
            Value::Segment { .. } if self.segment_carriers.contains_key(&id) => {
                Some(self.segment_carriers[&id])
            }
            Value::Segment { x1, y1, x2, y2 } => Some(join((x1, y1), (x2, y2))),
            _ => None,
        }
    }

    /// The coefficients a conditional segment took from a selected line,
    /// when they are not those of its end points.
    pub(crate) fn segment_carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.value(id) {
            Value::Segment { .. } => self.segment_carriers.get(&id).copied(),
            _ => None,
        }
    }

    fn set_segment_carrier(&mut self, id: Id, carrier: Option<(f64, f64, f64)>) {
        match carrier {
            Some(carrier) => self.segment_carriers.insert(id, carrier),
            None => self.segment_carriers.remove(&id),
        };
    }

    /// The state of a new conditional segment: `AlgoIf` starts from a copy
    /// of `GeoList.getGenericElement(alternatives)`, the last alternative of
    /// the leading run that a segment can be set from.
    fn conditional_segment_copy(&self, args: IdList) -> (Value, Option<(f64, f64, f64)>) {
        let ids = self.list(args);
        let mut generic = None;
        for (k, &id) in ids.iter().enumerate() {
            let alternative = k % 2 == 1 || k + 1 == ids.len();
            if !alternative {
                continue;
            }
            if self.kind(id) != Kind::Segment {
                break;
            }
            generic = Some(id);
        }
        let Some(id) = generic else {
            return (Value::Undefined, None);
        };
        (self.stored_value(id), self.segment_carrier(id))
    }

    /// `AlgoIf.compute` for a segment output, from the state the output
    /// holds. A selected segment brings the coefficients it carries. A false condition first applies the else
    /// alternative, or makes the output undefined when there is none, before
    /// the next condition is tested. `GeoSegment.set` copies a segment
    /// whole; from a line it copies only the coefficients and keeps the end
    /// points and the defined flag, so a segment made undefined stays so
    /// until a segment is selected.
    fn conditional_segment(
        &self,
        args: IdList,
        mut state: (Value, Option<(f64, f64, f64)>),
    ) -> (Value, Option<(f64, f64, f64)>) {
        let ids = self.list(args);
        let set = |state: &mut (Value, Option<(f64, f64, f64)>), id: Id| {
            let value = self.stored_value(id);
            *state = match (self.kind(id), value) {
                (_, value) if !value.is_defined() => (Value::Undefined, None),
                (Kind::Segment, value) => (value, self.segment_carrier(id)),
                (_, Value::Line { a, b, c }) if state.0.is_defined() => (state.0, Some((a, b, c))),
                _ => (Value::Undefined, None),
            };
        };
        let (pairs, fallback) = ids.as_chunks::<2>();
        for &[condition, branch] in pairs {
            if self.value(condition) == Value::Bool(true) {
                set(&mut state, branch);
                return state;
            }
            match fallback.first() {
                Some(&last) => set(&mut state, last),
                None => state = (Value::Undefined, None),
            }
        }
        state
    }

    pub(crate) fn circle(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.value(id) {
            Value::Circle { cx, cy, r } => Some((cx, cy, r)),
            _ => None,
        }
    }

    pub(crate) fn number(&self, id: Id) -> Option<f64> {
        match self.value(id) {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_fixed_length_segment_endpoint_moves_on_its_circle() {
        let mut c = crate::script::load(
            "A = (0, 0)\nn = 2\ns = Segment(A, n)\nC = (1, 1)\nt = Segment(C, 4)",
        )
        .unwrap();
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        let n = c.id("n").unwrap();
        let s = c.id("s").unwrap();
        let t = c.id("t").unwrap();
        assert_eq!(
            c.value(t),
            Value::Segment {
                x1: 1.0,
                y1: 1.0,
                x2: 5.0,
                y2: 1.0
            }
        );
        assert_eq!(c.point(b), Some((2.0, 0.0)));
        assert!(c.is_movable(b));
        c.move_point(b, 0.0, 4.0).unwrap();
        let (x, y) = c.point(b).unwrap();
        assert!(x.abs() < 1e-12);
        assert_eq!(y, 2.0);
        assert_eq!(
            c.value(s),
            Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: x,
                y2: y
            }
        );

        c.set_number(n, 3.0).unwrap();
        c.move_point(a, -2.0, 0.0).unwrap();
        let (x, y) = c.point(b).unwrap();
        assert!((x + 2.0).abs() < 1e-12);
        assert_eq!(y, 3.0);
        assert_eq!(
            c.value(s),
            Value::Segment {
                x1: -2.0,
                y1: 0.0,
                x2: x,
                y2: y
            }
        );
    }

    #[test]
    fn edits_after_redefinitions_match_a_rebuild_in_rank_order() {
        use crate::script::{exec, load};
        use std::collections::BTreeMap;

        for offset in [0, 1, 2] {
            let initial = [
                "O = (0, 0)",
                "c = Circle(O, 5)",
                "A = (-6, 0)",
                "B = (6, 0)",
                "g = Line(A, B)",
                "X = Intersect(g, c, 1)",
                "H = Intersect(c, c, 1)",
                "v: x = 0",
                "Y = Intersect(v, c, 1)",
                "Q = (-6, 0)",
                "R = (-6, 0)",
                "a = 2",
                "b = a + 1",
                "n = 4",
            ];
            let mut c = load(&initial.join("\n")).unwrap();
            let mut definitions = BTreeMap::new();
            let remember = |definitions: &mut BTreeMap<String, String>, line: &str| {
                let name = crate::script::assignment_label(line).unwrap();
                definitions.insert(name.to_owned(), line.to_owned());
            };
            for line in initial {
                remember(&mut definitions, line);
            }
            let check = |c: &Construction, definitions: &BTreeMap<String, String>| {
                let mut ranked: Vec<_> = c.ids().collect();
                ranked.sort_unstable_by_key(|&id| c.evaluation_rank(id));
                let mut fresh = Construction::new();
                for (rank, id) in ranked.into_iter().enumerate() {
                    let label = c.label(id);
                    assert_eq!(c.evaluation_rank(id), rank);
                    let rebuilt = exec(&mut fresh, &definitions[label]).unwrap().unwrap();
                    assert!(
                        c.value(id).identical(&fresh.value(rebuilt)),
                        "{label}: {:?} vs {:?}",
                        c.value(id),
                        fresh.value(rebuilt)
                    );
                    for child in c.direct_dependents(id) {
                        assert!(
                            c.evaluation_rank(child) > rank,
                            "{} -> {}",
                            label,
                            c.label(child)
                        );
                    }
                    for parent in c
                        .reads(id)
                        .into_iter()
                        .filter(|parent| parent.index() < c.len())
                    {
                        assert!(
                            c.direct_dependents(parent).any(|child| child == id),
                            "missing edge {} -> {label}",
                            c.label(parent)
                        );
                    }
                }
            };
            check(&c, &definitions);
            for line in [
                "A = Q + (0, 0)",
                "Q = R + (0, 0)",
                "a = 2 n",
                "A = R + (1, 0)",
            ] {
                exec(&mut c, line).unwrap();
                remember(&mut definitions, line);
                check(&c, &definitions);
            }
            let checkpoint = c.checkpoint();
            exec(&mut c, "unused = 9").unwrap();
            c.rollback(checkpoint).unwrap();
            assert!(exec(&mut c, "a = b + 1").is_err());
            check(&c, &definitions);
            let x = f64::from(offset);
            for (label, y) in [("R", 1.0), ("O", 2.0), ("R", 8.0), ("O", 0.0)] {
                c.move_point(c.id(label).unwrap(), x, y).unwrap();
                remember(&mut definitions, &format!("{label} = ({x}, {y})"));
                check(&c, &definitions);
            }
            c.set_number(c.id("n").unwrap(), x).unwrap();
            remember(&mut definitions, &format!("n = {x}"));
            check(&c, &definitions);
        }
    }

    #[test]
    fn rigid_frame_installs_constraints_in_evaluation_order() {
        let mut c = crate::script::load("A = (0, 0)\nC = (1, 2)\nB = (4, 0)\nn = 2").unwrap();
        for line in ["C = (1, n)", "C = (1, 2)"] {
            crate::script::exec(&mut c, line).unwrap();
        }
        let pivot = c.id("B").unwrap();
        let vertex = c.id("C").unwrap();
        assert!(vertex < pivot && c.evaluation_rank(pivot) < c.evaluation_rank(vertex));
        let checkpoint = c.checkpoint();
        crate::script::exec(&mut c, "p = RigidPolygon(A, B, C)").unwrap();
        assert_eq!(c.rigid_replacements[0].1, pivot);
        assert_eq!(c.rigid_replacements[1].1, vertex);
        c.rollback(checkpoint).unwrap();
        assert!(c.def(vertex).is_free());
    }

    #[test]
    fn a_locus_follows_the_free_inputs_of_its_redefined_point() {
        use crate::script::{exec, load};

        let mut c = load("P = Point(xAxis)\na = 1\nQ = (x(P), a)\nl = Locus(Q, P)\nb = 2").unwrap();
        let b = c.id("b").unwrap();
        let locus = c.id("l").unwrap();
        exec(&mut c, "Q = (x(P), 1 / b)").unwrap();
        assert!(c.direct_dependents(b).any(|id| id == locus));
        for b_value in [2, 0, 4] {
            c.set_number(b, f64::from(b_value)).unwrap();
            let rebuilt = load(&format!(
                "P = Point(xAxis)\nb = {b_value}\nQ = (x(P), 1 / b)\nl = Locus(Q, P)"
            ))
            .unwrap();
            let points = |c: &Construction| c.locus(c.id("l").unwrap()).map(|l| l.points.clone());
            assert_eq!(points(&c), points(&rebuilt));
        }
    }

    #[test]
    fn replacement_preserves_scripts_and_saved_presentation() {
        use crate::kernel::control::{encode_text, ControlScript};
        use crate::script::{self, saved::Property};

        let caption = encode_text("Keep this caption");
        let mut c = script::load(&format!(
            "b = $checkBox(true, {caption})\nd = If(b, 1, 0)\nt = \"first\""
        ))
        .unwrap();
        let b = c.id("b").unwrap();
        let t = c.id("t").unwrap();
        let scripts = vec![ControlScript {
            language: "script".into(),
            event: "update".into(),
            text: "SetValue(n, 2)".into(),
        }];
        Property::ObjectScripts {
            label: "b".into(),
            scripts: scripts.clone(),
        }
        .apply(&mut c)
        .unwrap();
        let style = super::DrawingStyle {
            layer: 4,
            show_label: false,
            line_thickness: 6,
        };
        c.set_drawing_style(b, style);
        script::exec(&mut c, "$textProperties(t, true, 20, 1, 4, 5, world, 2, 3)").unwrap();
        let presentation = c.text_presentation(t).unwrap();

        assert_eq!(script::exec(&mut c, "b = false"), Ok(Some(b)));
        assert_eq!(c.object_scripts("b"), scripts);
        assert_eq!(script::exec(&mut c, r#"t = "second""#), Ok(Some(t)));
        let error = script::exec(&mut c, "b = (1, 1)").unwrap_err();
        assert!(error.contains("a dependent object requires the original kind"));
        assert!(script::exec(&mut c, r#"t = t + "third""#).is_err());
        assert_eq!(c.object_scripts("b"), scripts);
        assert_eq!(c.drawing_style(b), style);
        assert_eq!(c.control(b).unwrap().caption, "Keep this caption");
        assert_eq!(c.text_presentation(t), Some(presentation));
        assert_eq!(c.text(t), Some("second"));
    }

    #[test]
    fn redefinition_moves_edges_in_a_large_child_family() {
        use crate::script;

        let mut c = script::load("b = 1\na = 2").unwrap();
        let a = c.id("a").unwrap();
        let b = c.id("b").unwrap();
        for i in 0..256 {
            script::exec(&mut c, &format!("v{i} = a + 1")).unwrap();
        }
        for i in 0..256 {
            script::exec(&mut c, &format!("v{i} = b + 2")).unwrap();
        }
        assert_eq!(c.children(a).count(), 0);
        assert_eq!(c.children(b).count(), 256);
        let v = c.id("v255").unwrap();
        c.set_number(a, 0.0).unwrap();
        assert_eq!(c.value(v), super::Value::Number(3.0));
        c.set_number(b, 4.0).unwrap();
        assert_eq!(c.value(v), super::Value::Number(6.0));
    }

    use super::*;
    use crate::algo::length;

    #[test]
    fn the_dirty_set_gives_indices_in_order_and_ends_empty() {
        let mut d = DirtySet::default();
        d.fit(10_000);
        for i in [9_999, 70, 5, 4_096, 71, 8_191] {
            d.insert(i);
        }
        d.insert(70);
        let mut got = Vec::new();
        let mut from = 0;
        while let Some(i) = d.pop_first(from) {
            got.push(i);
            from = i + 1;
            if i == 70 {
                // inserted while taking, above the current index
                d.insert(4_097);
            }
        }
        assert_eq!(got, [5, 70, 71, 4_096, 4_097, 8_191, 9_999]);
        assert_eq!(d.len, 0);
        assert!(d.levels[0].iter().all(|&w| w == 0));
        d.fit(300_000);
        assert_eq!(
            d.levels.iter().map(Vec::len).collect::<Vec<_>>(),
            [4_688, 74, 2, 1]
        );
        d.insert(299_999);
        assert_eq!(d.pop_first(3), Some(299_999));
        assert_eq!(d.pop_first(3), None);
    }

    fn pt(c: &mut Construction, label: &str, x: f64, y: f64) -> Id {
        c.add(label, Def::FreePoint { x, y }).unwrap()
    }

    fn xy(c: &Construction, id: Id) -> (f64, f64) {
        match c.value(id) {
            Value::Point { x, y } => (x, y),
            other => panic!("{} is not a defined point: {other:?}", c.label(id)),
        }
    }

    fn dist(c: &Construction, p: Id, q: Id) -> f64 {
        let (x1, y1) = xy(c, p);
        let (x2, y2) = xy(c, q);
        length(x2 - x1, y2 - y1)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Triangle ABC with its circumcentre built from two perpendicular bisectors.
    fn circumcentre() -> (Construction, [Id; 3], Id) {
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 6.0, 0.0);
        let cc = pt(&mut c, "C", 2.0, 5.0);
        let ab = c.add("c", Def::Segment(a, b)).unwrap();
        let ac = c.add("b", Def::Segment(a, cc)).unwrap();
        let mab = c.add("Mc", Def::Midpoint(a, b)).unwrap();
        let mac = c.add("Mb", Def::Midpoint(a, cc)).unwrap();
        let s1 = c
            .add(
                "sc",
                Def::Perpendicular {
                    through: mab,
                    to: ab,
                },
            )
            .unwrap();
        let s2 = c
            .add(
                "sb",
                Def::Perpendicular {
                    through: mac,
                    to: ac,
                },
            )
            .unwrap();
        let o = c.add("O", Def::IntersectLines(s1, s2)).unwrap();
        (c, [a, b, cc], o)
    }

    #[test]
    fn circumcentre_is_equidistant_and_follows_a_drag() {
        let (mut c, [a, b, cc], o) = circumcentre();
        let check = |c: &Construction| {
            let r = dist(c, o, a);
            assert!(close(r, dist(c, o, b)));
            assert!(close(r, dist(c, o, cc)));
        };
        check(&c);
        let before = xy(&c, o);

        c.move_point(a, -1.0, 2.0).unwrap();
        check(&c);
        assert_ne!(before, xy(&c, o));
    }

    #[test]
    fn line_meets_circle_in_order_along_the_line() {
        let mut c = Construction::new();
        let o = pt(&mut c, "O", 0.0, 0.0);
        let r = pt(&mut c, "R", 5.0, 0.0);
        let p = pt(&mut c, "P", -10.0, 3.0);
        let q = pt(&mut c, "Q", 10.0, 3.0);
        let k = c
            .add(
                "k",
                Def::CircleThrough {
                    center: o,
                    through: r,
                },
            )
            .unwrap();
        let l = c.add("l", Def::Line(p, q)).unwrap();
        let x0 = c
            .add(
                "X0",
                Def::IntersectLineCircle {
                    line: l,
                    circle: k,
                    which: 0,
                },
            )
            .unwrap();
        let x1 = c
            .add(
                "X1",
                Def::IntersectLineCircle {
                    line: l,
                    circle: k,
                    which: 1,
                },
            )
            .unwrap();

        let (ax, ay) = xy(&c, x0);
        let (bx, by) = xy(&c, x1);
        assert!(close(ax, -4.0) && close(ay, 3.0));
        assert!(close(bx, 4.0) && close(by, 3.0));
    }

    #[test]
    fn tangent_line_still_touches_the_circle() {
        let mut c = Construction::new();
        let o = pt(&mut c, "O", 0.0, 0.0);
        let r = pt(&mut c, "R", 5.0, 0.0);
        let p = pt(&mut c, "P", -10.0, 5.0);
        let q = pt(&mut c, "Q", 10.0, 5.0);
        let k = c
            .add(
                "k",
                Def::CircleThrough {
                    center: o,
                    through: r,
                },
            )
            .unwrap();
        let l = c.add("l", Def::Line(p, q)).unwrap();
        let t = c
            .add(
                "T",
                Def::IntersectLineCircle {
                    line: l,
                    circle: k,
                    which: 0,
                },
            )
            .unwrap();
        let (x, y) = xy(&c, t);
        assert!(close(x, 0.0) && close(y, 5.0));
    }

    #[test]
    fn circles_lose_and_regain_their_intersection() {
        let mut c = Construction::new();
        let o1 = pt(&mut c, "O1", 0.0, 0.0);
        let o2 = pt(&mut c, "O2", 8.0, 0.0);
        let r = c.add("r", Def::Number(5.0)).unwrap();
        let k1 = c
            .add(
                "k1",
                Def::CircleRadius {
                    center: o1,
                    radius: r,
                },
            )
            .unwrap();
        let k2 = c
            .add(
                "k2",
                Def::CircleRadius {
                    center: o2,
                    radius: r,
                },
            )
            .unwrap();
        let p = c
            .add(
                "P",
                Def::IntersectCircles {
                    c1: k1,
                    c2: k2,
                    which: 0,
                    special: None,
                },
            )
            .unwrap();
        let q = c
            .add(
                "Q",
                Def::IntersectCircles {
                    c1: k1,
                    c2: k2,
                    which: 1,
                    special: None,
                },
            )
            .unwrap();
        let m = c.add("M", Def::Midpoint(p, q)).unwrap();

        let (px, py) = xy(&c, p);
        let (qx, qy) = xy(&c, q);
        assert!(close(px, 4.0) && close(py, 3.0));
        assert!(close(qx, 4.0) && close(qy, -3.0));

        // Too far apart: the points and everything built on them vanish.
        c.move_point(o2, 20.0, 0.0).unwrap();
        assert_eq!(c.value(p), Value::Undefined);
        assert_eq!(c.value(q), Value::Undefined);
        assert_eq!(c.value(m), Value::Undefined);

        // Still apart: k2 changes, P and Q stay undefined, so M is not touched.
        assert_eq!(c.move_point(o2, 21.0, 0.0).unwrap(), 3);

        // Back in range: everything is defined again.
        c.move_point(o2, 8.0, 0.0).unwrap();
        let (mx, my) = xy(&c, m);
        assert!(close(mx, 4.0) && close(my, 0.0));

        // A smaller radius also separates them.
        c.set_number(r, 1.0).unwrap();
        assert_eq!(c.value(m), Value::Undefined);
    }

    #[test]
    fn only_descendants_are_recomputed() {
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 4.0, 0.0);
        let cc = pt(&mut c, "C", 1.0, -3.0);
        let d = pt(&mut c, "D", 1.0, 3.0);
        let e = pt(&mut c, "E", 9.0, 9.0);
        let f = pt(&mut c, "F", 7.0, 7.0);
        let l = c.add("l", Def::Line(a, b)).unwrap();
        let m = c.add("m", Def::Line(cc, d)).unwrap();
        let x = c.add("X", Def::IntersectLines(l, m)).unwrap();
        c.add("G", Def::Midpoint(e, f)).unwrap();

        assert_eq!(c.move_point(cc, 2.0, -3.0).unwrap(), 2); // m, X
        assert_eq!(c.move_point(a, 0.0, 1.0).unwrap(), 2); // l, X
        assert_eq!(c.move_point(e, 5.0, 5.0).unwrap(), 1); // G
        assert_eq!(c.move_point(e, 5.0, 5.0).unwrap(), 0); // no change
        assert!(c.value(x).is_defined());
    }

    #[test]
    fn a_conditional_segment_takes_the_coefficients_of_a_selected_line() {
        let mut c = crate::script::load(
            "n = -1\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = Segment(A, B)\n\
             g = Line(A, C)\ns = If(n > 0, r, g)",
        )
        .unwrap();
        let (n, b, s) = (c.id("n").unwrap(), c.id("B").unwrap(), c.id("s").unwrap());
        // The copy of `r` made at creation keeps its end points.
        let kept = Value::Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 4.0,
            y2: 0.0,
        };
        assert_eq!(c.value(s), kept);
        assert_eq!(c.raw_carrier(s), Some((-3.0, 0.0, 0.0)));
        c.move_point(b, 5.0, 1.0).unwrap();
        assert_eq!(c.value(s), kept);
        c.set_number(n, 1.0).unwrap();
        assert_eq!(
            c.value(s),
            Value::Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 5.0,
                y2: 1.0
            }
        );
        assert_eq!(c.raw_carrier(s), Some((-1.0, 5.0, 0.0)));
        assert_eq!(c.segment_carrier(s), None);
    }

    #[test]
    fn a_conditional_segment_without_an_else_stays_undefined_for_a_line() {
        let mut c = crate::script::load(
            "n = -2\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = Segment(A, B)\n\
             g = Line(A, C)\ns = If(n > 0, r, n > -5, g)",
        )
        .unwrap();
        let (n, s) = (c.id("n").unwrap(), c.id("s").unwrap());
        // The false first condition made the copy undefined before `g` was set.
        assert_eq!(c.value(s), Value::Undefined);
        c.set_number(n, 1.0).unwrap();
        assert!(c.value(s).is_defined());
        c.set_number(n, -2.0).unwrap();
        assert_eq!(c.value(s), Value::Undefined);
    }

    #[test]
    fn a_hidden_checkbox_has_its_corners_at_the_view_origin() {
        let c = crate::script::load(
            "b = true\nP = Vertex(b, 3)\nQ = Vertex(b, 5)\nR = Vertex(1 > 0, 1)",
        )
        .unwrap();
        let [xmin, _, _, ymax, _, _] = c.view();
        assert_eq!(
            c.value(c.id("P").unwrap()),
            Value::Point { x: xmin, y: ymax }
        );
        assert_eq!(c.value(c.id("Q").unwrap()), Value::Undefined);
        assert_eq!(c.value(c.id("R").unwrap()), Value::Undefined);
    }

    #[test]
    fn parallel_lines_have_no_intersection() {
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 4.0, 1.0);
        let p = pt(&mut c, "P", 0.0, 2.0);
        let l = c.add("l", Def::Line(a, b)).unwrap();
        let m = c.add("m", Def::Parallel { through: p, to: l }).unwrap();
        let x = c.add("X", Def::IntersectLines(l, m)).unwrap();
        assert_eq!(c.value(x), Value::Undefined);
    }

    #[test]
    fn lines_hold_geogebra_raw_coefficients() {
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 1.0, 2.0);
        let b = pt(&mut c, "B", 4.0, 6.0);
        let p = pt(&mut c, "P", 0.0, 1.0);
        let l = c.add("l", Def::Line(a, b)).unwrap();
        let s = c.add("s", Def::Segment(a, b)).unwrap();
        let q = c.add("q", Def::Parallel { through: p, to: s }).unwrap();
        // `GeoVec3D.lineThroughPoints`: (Ay - By, Bx - Ax, Ax By - Ay Bx).
        assert_eq!(
            c.value(l),
            Value::Line {
                a: -4.0,
                b: 3.0,
                c: -2.0
            }
        );
        assert_eq!(c.raw_line(l), Some((-4.0, 3.0, -2.0)));
        assert_eq!(c.raw_carrier(s), Some((-4.0, 3.0, -2.0)));
        assert_eq!(c.raw_line(s), None);
        assert_eq!(c.raw_carrier(q), Some((-4.0, 3.0, -3.0)));
        // The unit form keeps the orientation.
        let (ua, ub, uc) = c.line(l).unwrap();
        assert!(close(ua, -0.8) && close(ub, 0.6) && close(uc, -0.4));
        let (sa, sb, sc) = c.carrier(s).unwrap();
        assert!(close(sa, ua) && close(sb, ub) && close(sc, uc));
        assert_eq!(c.value(l).unit_line(), c.line(l));
        assert_eq!(c.value(a).unit_line(), None);
    }

    #[test]
    fn a_line_is_undefined_when_both_raw_coefficients_are_tiny() {
        // GeoGebra's `GeoLine.isDefined` tests x and y against 1e-8.
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 1.0, 1.0);
        let l = c.add("l", Def::Line(a, b)).unwrap();
        let s = c.add("s", Def::Segment(a, b)).unwrap();
        let p = c
            .add("p", Def::Perpendicular { through: b, to: s })
            .unwrap();
        c.move_point(b, 5e-9, -5e-9).unwrap();
        assert_eq!(c.value(l), Value::Undefined);
        assert_eq!(c.value(p), Value::Undefined);
        assert_eq!(c.raw_carrier(s), Some((5e-9, 5e-9, 0.0)));
        c.move_point(b, 2e-8, 0.0).unwrap();
        assert!(c.value(l).is_defined() && c.value(p).is_defined());
    }

    #[test]
    fn perpendiculars_are_rescaled_by_powers_of_two() {
        // `AlgoOrthoLinePointLine` keeps some coefficient within [1e-2, 1e7].
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 0.001, 0.0);
        let p = pt(&mut c, "P", 0.0, 0.0);
        let l = c.add("l", Def::Line(a, b)).unwrap();
        let n = c
            .add("n", Def::Perpendicular { through: p, to: l })
            .unwrap();
        // l is (0, 0.001, 0); the perpendicular (-0.001, 0, 0) doubles to
        // (-0.016, 0, 0).
        assert_eq!(c.raw_line(n), Some((-0.016, 0.0, 0.0)));
        // Only when all three leave the range: (-3e7, 3e7, 0) through
        // (0, 0) gives (-3e7, -3e7, 0), kept as it is...
        c.move_point(b, 3e7, 3e7).unwrap();
        assert_eq!(c.raw_line(n), Some((-3e7, -3e7, 0.0)));
        // ...and through (1, 0) gives (-3e7, -3e7, 3e7), halved twice.
        c.move_point(p, 1.0, 0.0).unwrap();
        assert_eq!(c.raw_line(n), Some((-7.5e6, -7.5e6, 7.5e6)));
    }

    #[test]
    fn listed_parents_are_parents_and_a_rejected_list_is_removed() {
        use crate::algo::polygon::PolygonDef;
        let mut c = Construction::new();
        let p: Vec<Id> = (0..6)
            .map(|i| pt(&mut c, &format!("P{i}"), i as f64, (i * i) as f64))
            .collect();
        let k = c
            .add(
                "k",
                Def::CircleThrough {
                    center: p[0],
                    through: p[1],
                },
            )
            .unwrap();
        let area = |list| Def::Polygon(PolygonDef::AreaPoints(list));
        // a circle in the list: rejected, and the list is not kept
        assert!(matches!(
            c.add_with_list("bad", &[p[0], p[1], k, p[2], p[3]], area),
            Err(Error::WrongKind { .. })
        ));
        assert!(c.lists.is_empty());
        // P1 twice: still one child of P1
        let a = c
            .add_with_list("a", &[p[0], p[1], p[2], p[1], p[4], p[5]], area)
            .unwrap();
        assert_eq!(c.lists.len(), 6);
        assert_eq!(c.children(p[1]).collect::<Vec<_>>(), &[k, a]);
        assert_eq!(c.children(p[3]).collect::<Vec<_>>(), Vec::<Id>::new());
        let list = c.def(a).list().unwrap();
        assert_eq!(list.len(), 6);
        assert_eq!(c.list(list), &[p[0], p[1], p[2], p[1], p[4], p[5]]);
        let before = c.value(a);
        assert_eq!(c.move_point(p[5], 10.0, -3.0).unwrap(), 1);
        assert_ne!(c.value(a), before);
        assert_eq!(c.move_point(p[3], 10.0, -3.0).unwrap(), 0);
        // a handle this construction did not give out
        let mut other = Construction::new();
        pt(&mut other, "Q", 0.0, 0.0);
        assert_eq!(other.list(list), Vec::<Id>::new());
        assert_eq!(other.add("a", c.def(a)), Err(Error::UnknownObject));
        // a copy in the same construction may share the list
        let b = c.add("b", c.def(a)).unwrap();
        assert_eq!(c.value(b), c.value(a));
        assert_eq!(c.children(p[1]).collect::<Vec<_>>(), &[k, a, b]);
    }

    #[test]
    fn invalid_input_is_rejected() {
        let mut c = Construction::new();
        let a = pt(&mut c, "A", 0.0, 0.0);
        let b = pt(&mut c, "B", 1.0, 0.0);
        let k = c
            .add(
                "k",
                Def::CircleThrough {
                    center: a,
                    through: b,
                },
            )
            .unwrap();
        let m = c.add("M", Def::Midpoint(a, b)).unwrap();

        assert!(matches!(
            c.add("l", Def::Line(a, k)),
            Err(Error::WrongKind { .. })
        ));
        assert_eq!(
            c.add("A", Def::FreePoint { x: 1.0, y: 1.0 }),
            Err(Error::DuplicateLabel("A".into()))
        );
        assert_eq!(c.move_point(m, 1.0, 1.0), Err(Error::NotFree("M".into())));
        assert_eq!(c.move_point(a, f64::NAN, 0.0), Err(Error::NotFinite));
        // A rejected object leaves the construction untouched.
        assert_eq!(c.len(), 4);
    }

    /// Everything a rejected line or a rollback must leave as it was.
    #[derive(Debug, PartialEq)]
    struct Snapshot {
        labels: Vec<String>,
        by_label: Vec<(String, Id)>,
        children: Vec<Children>,
        lists: Vec<Id>,
        values: Vec<Value>,
    }

    fn snapshot(c: &Construction) -> Snapshot {
        let mut by_label: Vec<_> = c.by_label.iter().map(|(l, &id)| (l.clone(), id)).collect();
        by_label.sort();
        Snapshot {
            labels: c.labels.clone(),
            by_label,
            children: c.children.clone(),
            lists: c.lists.clone(),
            values: c.values.clone(),
        }
    }

    #[test]
    fn rollback_unregisters_a_saved_text_anchor() {
        use super::text::{TextOverrides, TextPosition};

        let mut c = crate::script::load("A = (0, 0)").unwrap();
        let a = c.id("A").unwrap();
        let at = c.checkpoint();
        let t = crate::script::exec(&mut c, r#"t = "abc""#)
            .unwrap()
            .unwrap();
        c.apply_text_properties(
            t,
            TextOverrides {
                position: Some(TextPosition::Point(a)),
                ..TextOverrides::default()
            },
        )
        .unwrap();
        assert_eq!(c.direct_dependents(a).collect::<Vec<_>>(), [t]);

        c.rollback(at).unwrap();
        assert_eq!(c.move_point(a, 1.0, 1.0), Ok(0));
        assert!(c.id("t").is_none());
    }

    #[test]
    fn rollback_removes_objects_edges_labels_and_lists() {
        use crate::algo::polygon::PolygonDef;
        let mut c = Construction::new();
        let p: Vec<Id> = (0..5)
            .map(|i| pt(&mut c, &format!("P{i}"), i as f64, (i * i) as f64))
            .collect();
        let area = |list| Def::Polygon(PolygonDef::AreaPoints(list));
        c.add_with_list("a", &p, area).unwrap();
        c.add("s", Def::Segment(p[0], p[1])).unwrap();
        let before = snapshot(&c);
        let at = c.checkpoint();
        let poly = c
            .add_with_list("q", &[p[0], p[2], p[4]], |l| {
                Def::Polygon(PolygonDef::Polygon(l))
            })
            .unwrap();
        c.add("t", Def::Segment(p[2], p[4])).unwrap();
        c.add("r", Def::Polygon(PolygonDef::Area(poly))).unwrap();
        c.add_with_list("b", &[p[1], p[1], p[3], p[4], p[0]], area)
            .unwrap();
        assert_ne!(snapshot(&c), before);
        c.rollback(at).unwrap();
        assert_eq!(snapshot(&c), before);
        assert_eq!(c.hints.len(), c.len());
        assert_eq!(c.len(), 7);
        assert_eq!(c.id("q"), None);
        // the construction still works after a rollback
        let t = c.add("t", Def::Segment(p[3], p[4])).unwrap();
        assert_eq!(
            c.children(p[3]).collect::<Vec<_>>(),
            &[c.id("a").unwrap(), t]
        );
        assert_eq!(c.move_point(p[3], 1.0, 1.0).unwrap(), 2);
    }

    #[test]
    fn a_checkpoint_from_before_a_rollback_is_refused_and_changes_nothing() {
        use crate::script::{exec, load};
        let mut c = load("A = (0, 0)\nB = (4, 0)\n").unwrap();
        let early = c.checkpoint();
        for line in ["C = (2, 3)", "n = 1 + 2", "q = Polygon(A, B, C)"] {
            exec(&mut c, line).unwrap();
        }
        let late = c.checkpoint();
        c.rollback(early).unwrap();
        // More objects, expressions and lists than `late` had.
        for line in [
            "C = (1, 1)",
            "m = 3 * 4",
            "r = Polygon(A, B, C)",
            "s = Segment(A, B)",
            "t = m + 6",
            "u = Polygon(A, C, B)",
        ] {
            exec(&mut c, line).unwrap();
        }
        assert!(c.len() > late.len && c.lists.len() > late.lists);
        assert!(c.exprs.len() > late.exprs);
        let before = snapshot(&c);
        let exprs = c.exprs.len();
        assert_eq!(c.rollback(late), Err(StaleCheckpoint));
        assert_eq!(snapshot(&c), before);
        assert_eq!(c.exprs.len(), exprs);
        // Everything still works: the expressions and the lists that the
        // refused rollback would have cut.
        let a = c.id("A").unwrap();
        c.move_point(a, -2.0, 0.0).unwrap();
        assert_eq!(c.value(c.id("t").unwrap()), Value::Number(18.0));
        assert!(matches!(c.value(c.id("u").unwrap()), Value::Polygon { .. }));
        // A checkpoint taken after the rollback is good, once.
        let fresh = c.checkpoint();
        exec(&mut c, "v = 7").unwrap();
        c.rollback(fresh.clone()).unwrap();
        assert_eq!(c.id("v"), None);
        assert_eq!(c.rollback(fresh), Err(StaleCheckpoint));
        // Even a checkpoint that the last rollback did not cut is stale.
        let first = c.checkpoint();
        exec(&mut c, "w = 1").unwrap();
        let second = c.checkpoint();
        exec(&mut c, "k = 2").unwrap();
        c.rollback(second).unwrap();
        assert_eq!(c.rollback(first), Err(StaleCheckpoint));
        assert!(c.id("w").is_some());
    }

    #[test]
    fn a_checkpoint_longer_than_the_construction_is_refused() {
        use crate::script::load;
        let long = load("A = (0, 0)\nB = (1, 0)\nn = 2 + 2\n").unwrap();
        let mut short = load("A = (0, 0)\n").unwrap();
        let before = snapshot(&short);
        assert_eq!(short.rollback(long.checkpoint()), Err(StaleCheckpoint));
        assert_eq!(snapshot(&short), before);
    }

    #[test]
    fn an_all_or_nothing_step_keeps_the_callers_checkpoint_valid() {
        use crate::script::exec;
        let mut c = Construction::new();
        exec(&mut c, "A = (0, 0)").unwrap();
        let outer = c.checkpoint();
        exec(&mut c, "B = (1, 0)").unwrap();
        // What a command that makes several objects does when one fails.
        let inner = c.checkpoint();
        exec(&mut c, "C = (2, 0)").unwrap();
        c.discard_since(inner);
        assert_eq!(c.id("C"), None);
        c.rollback(outer).unwrap();
        assert_eq!((c.len(), c.id("B")), (1, None));
    }

    #[test]
    fn rollback_restores_a_circle_before_the_protocol_advanced() {
        let mut c = crate::script::load(
            "O = (0, 0)
s = ConstructionStep()
r = If(s > 6, 3, -2)
\
             k = Circle(O, r)
j = Circle(O, 3)
t = AreEqual(k, j)
",
        )
        .unwrap();
        let t = c.id("t").unwrap();
        assert_eq!(c.value(t), Value::Bool(false));
        let checkpoint = c.checkpoint();
        crate::script::exec(&mut c, "P = (1, 1)").unwrap();
        assert_eq!(c.value(t), Value::Bool(true));
        c.rollback(checkpoint).unwrap();
        assert_eq!(c.value(t), Value::Bool(false));
    }

    #[test]
    fn rollback_restores_inequality_helpers_before_the_protocol_advanced() {
        let mut c = crate::script::load(concat!(
            "s = ConstructionStep()\na = 0\nn = If(s > 5, 0.5, a)\n",
            "q(x,y) = n*y > x^n && x > 4 && y > 0\nP = Vertex(q)",
        ))
        .unwrap();
        let a = c.id("a").unwrap();
        let p = c.id("P_{1}").unwrap();
        let checkpoint = c.checkpoint();
        crate::script::exec(&mut c, "Z = (1, 1)").unwrap();
        assert_eq!(c.number(c.id("n").unwrap()), Some(0.5));

        c.rollback(checkpoint).unwrap();
        assert_eq!(c.number(c.id("n").unwrap()), Some(0.0));
        c.set_number(a, 2.0).unwrap();
        assert_eq!(c.point(p), Some((4.0, 8.0)));
    }

    #[test]
    fn rollback_leaves_an_unrelated_placed_path_point_alone() {
        let mut c = crate::script::load(
            "s = ConstructionStep()\nO = (0, 0)\nr = 2\nk = Circle(O, r)\nQ = Point(k)",
        )
        .unwrap();
        let q = c.id("Q").unwrap();
        c.set_number(c.id("r").unwrap(), -2.0).unwrap();
        c.move_point(q, 0.0, 10.0).unwrap();
        let placed = c.point(q).unwrap();
        assert_eq!(placed.1, 2.0);
        let checkpoint = c.checkpoint();
        crate::script::exec(&mut c, "P = (1, 1)").unwrap();
        c.rollback(checkpoint).unwrap();
        assert_eq!(c.point(q), Some(placed));
    }

    #[test]
    fn nested_checkpoints_restore_a_protocol_dependents_placement() {
        let mut c = crate::script::load(
            "s = ConstructionStep()\nO = (0, 0)\nr = If(s < 6, 2, 5 - s)\n             k = Circle(O, r)\nQ = Point(k)\nP = (1, 1)",
        )
        .unwrap();
        let q = c.id("Q").unwrap();
        c.move_point(q, 0.0, 10.0).unwrap();
        let placed = c.point(q).unwrap();
        let outer = c.checkpoint();
        crate::script::exec(&mut c, "B = (2, 2)").unwrap();
        assert_eq!(c.point(q), None);
        let inner = c.checkpoint();
        crate::script::exec(&mut c, "C = (3, 3)").unwrap();
        c.discard_since(inner);
        assert_eq!(c.point(q), None);
        c.rollback(outer).unwrap();
        assert_eq!(c.point(q), Some(placed));
    }

    #[test]
    fn checkpoints_ignore_unrelated_geometry_during_ten_thousand_appends() {
        let mut c =
            crate::script::load("s = ConstructionStep()\nO = (0, 0)\nk = Circle(O, 2)").unwrap();
        for i in 0..10_000 {
            let checkpoint = c.checkpoint();
            assert!(checkpoint.protocol.is_none());
            c.add(&format!("P{i}"), Def::FreePoint { x: 1.0, y: 2.0 })
                .unwrap();
        }
        assert_eq!(c.number(c.id("s").unwrap()), Some(10_003.0));
    }

    #[test]
    fn redefining_the_radius_rebuilds_the_circle_record() {
        let mut c = crate::script::load(
            "O = (0, 0)
r = 2
k = Circle(O, r)
j = Circle(O, 2)
\
             m = 1
t = AreEqual(k, j)
",
        )
        .unwrap();
        let t = c.id("t").unwrap();
        assert_eq!(c.value(t), Value::Bool(true));
        crate::script::exec(&mut c, "r = -m").unwrap();
        assert_eq!(c.value(t), Value::Bool(false));
    }

    #[test]
    fn rollback_restores_step_indices_readers_and_dependents() {
        for rollback in [false, true] {
            let mut c = crate::script::load(
                "s = ConstructionStep()\nA = (0, 0)\na = ConstructionStep(A)\nd = 2 * s\n",
            )
            .unwrap();
            let s = c.id("s").unwrap();
            let a = c.id("a").unwrap();
            let d = c.id("d").unwrap();
            c.set_construction_step(1.0);
            let checkpoint = c.checkpoint();

            crate::script::exec(&mut c, "B = (1, 1)").unwrap();
            let abandoned = crate::script::exec(&mut c, "t = ConstructionStep()")
                .unwrap()
                .unwrap();
            assert_eq!(c.number(a), Some(4.0));
            assert_eq!(c.number(d), Some(6.0));
            c.set_construction_step(f64::INFINITY);
            assert_eq!(c.number(s), Some(6.0));

            if rollback {
                c.rollback(checkpoint).unwrap();
            } else {
                c.discard_since(checkpoint);
            }
            assert_eq!(c.step(), 0);
            assert_eq!(c.number(s), Some(1.0));
            assert_eq!(c.number(a), Some(2.0));
            assert_eq!(c.number(d), Some(2.0));
            assert_eq!(c.protocol, [s.0, a.0]);

            c.add("b", Def::Number(4.0)).unwrap();
            assert_eq!(c.add("q", Def::Number(7.0)), Ok(abandoned));
            c.set_construction_step(f64::INFINITY);
            assert_eq!(c.number(abandoned), Some(7.0));
            assert_eq!(c.number(d), Some(12.0));
        }
    }

    #[test]
    fn a_rejected_line_with_several_objects_changes_nothing() {
        use crate::script::{exec, load};
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (5, 3)\nD = (2, 5)\nE = (-1, 3)\n\
             q = Polygon(A, B, C, D, E)\nV_1 = (9, 9)\nn = 4\n",
        )
        .unwrap();
        assert!(!c.lists.is_empty());
        let before = snapshot(&c);
        for line in [
            "A = Area(A, B, C, D, E)",
            "q = Polygon(A, B, C)",
            "q = Polygon(A, B, n)",
            "n = Vertex(q)",
            "V_1 = InteriorAngles(q)",
            "A = Angle(q)",
            "w = Polygon(A, B, 2000)",
        ] {
            assert!(exec(&mut c, line).is_err(), "{line}");
            assert_eq!(snapshot(&c), before, "{line}");
        }
        // `V_1` is taken, so the vertices are `V_{2}`, ...
        exec(&mut c, "V = Vertex(q)").unwrap();
        assert_eq!(c.label(Id(c.len() - 5)), "V_{2}");
    }
}
