//! Construction kernel: a dependency graph of geometric objects.
//!
//! Every object is either *free* (a point or a number the user can change)
//! or *dependent* (defined by a rule over earlier objects). An object can
//! only refer to objects created before it, so creation order is already a
//! topological order and an update never has to sort the graph.
//!
//! When a free object changes, only its descendants are recomputed, in
//! creation order, and a branch is cut as soon as a recomputed value turns
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

use crate::algo::arc::{ArcData, ArcDef};
use crate::algo::conic::ConicDef;
use crate::algo::conic_nd::{ConicData, ConicType};
use crate::algo::conic_parts::ConicPartDef;
use crate::algo::general_conic::GeneralConicDef;
use crate::algo::lines::LineDef;
use crate::algo::measure::MeasureDef;
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
use std::collections::HashMap;
use std::fmt;

/// Handle of an object inside one [`Construction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub(crate) usize);

impl Id {
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
    Vector,
    Ray,
    /// A true or false answer, such as the result of `AreCollinear`.
    Boolean,
    /// A function of `x`, `f(x) = ...`; see [`Construction::function_value`].
    Function,
    Polygon,
    PolyLine,
    /// A list of numbers, points, vectors, booleans or lists, such as
    /// `{1, 2, 3}`; its elements are read with [`Construction::elements`].
    List,
    /// A general conic (`Conic`, `Ellipse`, `Hyperbola`, `Parabola` and
    /// their images); its state is read with [`Construction::conic`].
    Conic,
    /// An arc of a circle or conic; its state is read with
    /// [`Construction::arc`].
    Arc,
    /// A sector of a circle or conic, read like an arc.
    Sector,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Point => "point",
            Kind::Line => "line",
            Kind::Segment => "segment",
            Kind::Circle => "circle",
            Kind::Number => "number",
            Kind::Vector => "vector",
            Kind::Ray => "ray",
            Kind::Boolean => "boolean",
            Kind::Function => "function",
            Kind::Polygon => "polygon",
            Kind::PolyLine => "polyline",
            Kind::List => "list",
            Kind::Conic => "conic",
            Kind::Arc => "arc",
            Kind::Sector => "sector",
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
    /// A defined function; its values come from
    /// [`Construction::function_value`].
    Function,
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
    /// The object does not exist for the current positions
    /// (for example two circles that no longer meet).
    Undefined,
}

impl Value {
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

    fn is_finite(&self) -> bool {
        match *self {
            Value::Point { x, y } => x.is_finite() && y.is_finite(),
            Value::Line { a, b, c } => a.is_finite() && b.is_finite() && c.is_finite(),
            Value::Segment { x1, y1, x2, y2 } => {
                x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()
            }
            Value::Circle { cx, cy, r } => cx.is_finite() && cy.is_finite() && r.is_finite(),
            Value::Number(v) => v.is_finite(),
            Value::Vector { x, y } => x.is_finite() && y.is_finite(),
            Value::Ray { x, y, a, b, c } => [x, y, a, b, c].iter().all(|v| v.is_finite()),
            Value::Bool(_) => true,
            Value::Polygon { area } => area.is_finite(),
            Value::PolyLine { length } => length.is_finite(),
            Value::Arc { length } => length.is_finite(),
            Value::Sector { area } => area.is_finite(),
            Value::Function | Value::List { .. } | Value::Conic { .. } | Value::Undefined => true,
        }
    }
}

/// How an object is defined.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    /// A point the user can drag.
    FreePoint {
        x: f64,
        y: f64,
    },
    /// A number the user can change (a slider).
    Number(f64),
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
    /// Bisectors and tangents.
    Lines(LineDef),
    /// Circles and their parts.
    Conic(ConicDef),
    /// Numbers measured on objects.
    Measure(MeasureDef),
    /// Reflections, rotations and dilations.
    Transform(TransformDef),
    /// Vectors, rays and translations.
    Vector(VectorDef),
    /// Boolean tests such as `AreCollinear`.
    Predicate(PredicateDef),
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
}

impl Def {
    pub fn kind(&self) -> Kind {
        match self {
            Def::FreePoint { .. }
            | Def::Midpoint(..)
            | Def::IntersectLines(..)
            | Def::IntersectLineCircle { .. }
            | Def::IntersectCircles { .. } => Kind::Point,
            Def::Line(..) | Def::Perpendicular { .. } | Def::Parallel { .. } => Kind::Line,
            Def::Segment(..) => Kind::Segment,
            Def::CircleThrough { .. } | Def::CircleRadius { .. } => Kind::Circle,
            Def::Number(_) | Def::Angle(_) | Def::Distance(..) => Kind::Number,
            Def::Lines(d) => d.kind(),
            Def::Conic(d) => d.kind(),
            Def::Measure(d) => d.kind(),
            Def::Transform(d) => d.kind(),
            Def::Vector(d) => d.kind(),
            Def::Predicate(d) => d.kind(),
            Def::Point(d) => d.kind(),
            Def::Expr(d) => d.kind(),
            Def::Polygon(d) => d.kind(),
            Def::GeneralConic(d) => d.kind(),
            Def::ConicPart(d) => d.kind(),
            Def::Path(d) => d.kind(),
            Def::Arc(d) => d.kind(),
        }
    }

    /// The stored list of parents this definition reads, if it has one.
    pub fn list(&self) -> Option<IdList> {
        match self {
            Def::Polygon(d) => d.list(),
            Def::GeneralConic(d) => d.list(),
            _ => None,
        }
    }

    /// The same definition reading the list `list` instead of its own.
    /// A definition without a list is returned unchanged.
    pub fn with_list(self, list: IdList) -> Def {
        match self {
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
                | Def::Angle(_)
                | Def::Vector(VectorDef::Free { .. })
        )
    }

    /// The objects this definition reads. An expression reads the objects
    /// of its tree, which [`expr::parents`] lists.
    pub(crate) fn parents(&self) -> Parents {
        match *self {
            Def::FreePoint { .. } | Def::Number(_) | Def::Angle(_) | Def::Expr(_) => {
                Parents::none()
            }
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
            Def::Lines(d) => d.parents(),
            Def::Conic(d) => d.parents(),
            Def::Measure(d) => d.parents(),
            Def::Transform(d) => d.parents(),
            Def::Vector(d) => d.parents(),
            Def::Predicate(d) => d.parents(),
            Def::Point(d) => d.parents(),
            Def::Polygon(d) => d.parents(),
            Def::GeneralConic(d) => d.parents(),
            Def::ConicPart(d) => d.parents(),
            Def::Path(d) => d.parents(),
            Def::Arc(d) => d.parents(),
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
        }
    }
}

impl std::error::Error for Error {}

/// A state to return to with [`Construction::rollback`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    len: usize,
    lists: usize,
    exprs: usize,
    /// How many rollbacks the construction had made when it was taken.
    generation: u64,
}

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

/// A geometric construction.
#[derive(Default)]
pub struct Construction {
    defs: Vec<Def>,
    values: Vec<Value>,
    labels: Vec<String>,
    /// For each object, the objects that read it directly.
    children: Vec<Vec<Id>>,
    /// For each object, what its rule needs from the definitions of other
    /// objects, decided when it is created (see [`algo::intersect::Hint`]).
    hints: Vec<algo::intersect::Hint>,
    by_label: HashMap<String, Id>,
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
    /// The conic records of the [`Def::GeneralConic`] objects, with the
    /// index of each object, in creation order.
    conics: Vec<(usize, ConicData)>,
    /// The states of the points on paths, with the index of each point, in
    /// creation order.
    paths: Vec<(usize, PathState)>,
    /// The records of the arcs and sectors, with the index of each object,
    /// in creation order.
    arcs: Vec<(usize, ArcData)>,
}

/// The objects waiting to be recomputed during an update; empty between
/// updates. `levels[0]` has one bit per object. Each further level has one
/// bit per word of the level below, set whenever that word is not zero, and
/// the last level is a single word. Bits above level 0 are cleared lazily:
/// one may stay set for a word that has become zero, until a search meets
/// it. Finding the next dirty object then costs a few reads per level
/// however far away it is, and a drag with few dirty objects touches little
/// more than level 0.
#[derive(Default)]
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

impl Construction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// All objects in creation order.
    pub fn ids(&self) -> impl Iterator<Item = Id> {
        (0..self.defs.len()).map(Id)
    }

    pub fn id(&self, label: &str) -> Option<Id> {
        self.by_label.get(label).copied()
    }

    pub fn label(&self, id: Id) -> &str {
        &self.labels[id.0]
    }

    pub fn def(&self, id: Id) -> Def {
        self.defs[id.0]
    }

    pub fn kind(&self, id: Id) -> Kind {
        self.defs[id.0].kind()
    }

    pub fn value(&self, id: Id) -> Value {
        self.values[id.0]
    }

    /// `±∞` when `id` is an expression number whose value is infinite.
    /// [`Self::value`] reports such a number undefined, but GeoGebra counts
    /// it as defined, and expressions that read it use the infinite value
    /// (`1 / (r - a)` with `a = ∞` is `-0`).
    pub fn infinite_number(&self, id: Id) -> Option<f64> {
        match self.defs.get(id.0)? {
            &Def::Expr(d) if d.kind() == Kind::Number => match expr::raw(self, d) {
                expr::Raw::Num(v) if v.is_infinite() => Some(v),
                _ => None,
            },
            _ => None,
        }
    }

    /// The value of the function `id` at `x`, evaluated as GeoGebra's
    /// `GeoFunction.value` does; it may be infinite or `NaN` where the
    /// function is not defined. `None` when `id` is not a defined function.
    pub fn function_value(&self, id: Id, x: f64) -> Option<f64> {
        expr::function_value(self, id, x)
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
    pub(crate) fn children(&self, id: Id) -> &[Id] {
        &self.children[id.0]
    }

    /// Checks a definition and computes the value it would have if it were
    /// added now, without adding it.
    pub(crate) fn evaluate(&self, def: Def) -> Result<Value, Error> {
        self.check(&def)?;
        let def = algo::intersect::prepare(self, def);
        let hint = algo::intersect::hint(self, &def);
        Ok(self.compute_with(def, |_| hint))
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
            generation: self.generation,
        }
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
        self.generation += 1;
        Ok(())
    }

    /// The work of [`Construction::rollback`], for an all-or-nothing step
    /// inside one call of the kernel (a command that makes several objects):
    /// `at` was taken in that call, so every checkpoint the caller holds lies
    /// at or before it and stays valid, and the generation does not change.
    pub(crate) fn discard_since(&mut self, at: Checkpoint) {
        debug_assert_eq!(at.generation, self.generation, "a checkpoint of this call");
        while self.defs.len() > at.len {
            let id = self.defs.len() - 1;
            let def = self.defs[id];
            for parent in self.read_ids(&def) {
                // Children are linked in creation order: the newest are last.
                let children = &mut self.children[parent.0];
                while children.last().is_some_and(|c| c.0 >= id) {
                    children.pop();
                }
            }
            self.defs.pop();
            self.values.pop();
            self.children.pop();
            self.hints.pop();
            if let Some(label) = self.labels.pop() {
                self.by_label.remove(&label);
            }
        }
        self.lists.truncate(at.lists.min(self.lists.len()));
        self.exprs.truncate(at.exprs, at.len);
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
        while self.arcs.last().is_some_and(|&(owner, _)| owner >= at.len) {
            self.arcs.pop();
        }
        // Labels became free again, so the hints may be too high.
        self.label_hints
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Every object `def` reads: its parents, the objects its rule also
    /// reads (for example the line an intersection was prepared with), and
    /// its list. An object may appear more than once.
    pub(crate) fn read_ids(&self, def: &Def) -> Vec<Id> {
        let mut ids = algo::intersect::hidden_parents(self, def);
        ids.extend(algo::path::hidden_parents(self, def));
        ids.extend(algo::vector::hidden_parents(self, def));
        ids.extend(algo::predicate::hidden_parents(self, def));
        ids.extend(algo::polygon::hidden_parents(self, def));
        ids.extend(expr::parents(self, def));
        ids.extend_from_slice(def.parents().as_slice());
        if let Some(list) = def.list() {
            ids.extend_from_slice(self.list(list));
        }
        ids
    }

    /// Adds an object and computes its value.
    pub fn add(&mut self, label: &str, def: Def) -> Result<Id, Error> {
        if label.is_empty() {
            return Err(Error::EmptyLabel);
        }
        if self.by_label.contains_key(label) {
            return Err(Error::DuplicateLabel(label.to_string()));
        }
        self.check(&def)?;
        let def = algo::intersect::prepare(self, def);

        let id = Id(self.defs.len());
        // compute reads the hint of the new object from `hints`
        self.hints.push(algo::intersect::hint(self, &def));
        let value = match def {
            Def::Expr(d) => expr::update(self, d).0,
            Def::GeneralConic(d) => {
                let data = d.record(self, &ConicData::new());
                self.conics.push((id.0, data));
                algo::general_conic::value(d.kind(), &data)
            }
            Def::Path(d) if d.has_state() => {
                let state = d.create(self);
                self.paths.push((id.0, state));
                PathDef::value(&state)
            }
            Def::Arc(d) if d.has_record() => {
                let record = d.record(self);
                self.arcs.push((id.0, record));
                algo::arc::value(d.kind() == Kind::Sector, &record)
            }
            _ => self.compute(def, id.0),
        };
        // An object that is read twice is still one child.
        let mut parents = self.read_ids(&def);
        parents.sort_unstable();
        parents.dedup();
        for parent in parents {
            self.children[parent.0].push(id);
        }
        self.defs.push(def);
        self.values.push(value);
        self.labels.push(label.to_string());
        self.children.push(Vec::new());
        self.by_label.insert(label.to_string(), id);
        Ok(id)
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
        let list = match (u32::try_from(start), u32::try_from(ids.len())) {
            (Ok(start), Ok(len)) if start.checked_add(len).is_some() => IdList { start, len },
            _ => return Err(Error::UnknownObject),
        };
        self.lists.extend_from_slice(ids);
        let result = self.add(label, make(list));
        if result.is_err() {
            self.lists.truncate(start);
        }
        result
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
        let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        if let Def::Path(d @ PathDef::On { .. }) = *def {
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
        self.defs[id.0] = def;
        self.values[id.0] = new;
        Ok(self.propagate(id))
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

    /// Changes a free number and recomputes everything that depends on it.
    /// Returns how many dependent objects were recomputed.
    pub fn set_number(&mut self, id: Id, v: f64) -> Result<usize, Error> {
        let def = *self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        let new_def = match def {
            Def::Number(_) => Def::Number(v),
            Def::Angle(_) => Def::Angle(v),
            _ => return Err(Error::NotFree(self.labels[id.0].clone())),
        };
        if !v.is_finite() {
            return Err(Error::NotFinite);
        }
        // A changed number loses its exact decimal (`GeoNumeric.setValue`),
        // which changes what reads it even when the value stays.
        let had_decimal = expr::forget_decimal(self, id);
        // Bit equality: `0` to `-0` is a change (`1/a` flips its sign).
        if !had_decimal
            && matches!(self.values[id.0], Value::Number(old) if old.to_bits() == v.to_bits())
        {
            return Ok(0);
        }
        self.defs[id.0] = new_def;
        self.values[id.0] = Value::Number(v);
        Ok(self.propagate(id))
    }

    /// Recomputes the descendants of `start` in creation order.
    ///
    /// A set of dirty indices ([`DirtySet`]) gives the order: the smallest
    /// is taken next. Because children always have larger indices than their
    /// parents, by the time an object is reached every parent that could
    /// still change has already been handled. Unlike a heap, inserting and
    /// taking cost a few bit operations, which matters when a point has tens
    /// of thousands of children.
    fn propagate(&mut self, start: Id) -> usize {
        // Taken out so that `compute` can borrow `self`. If `compute` panics
        // the set is lost, and `fit` gives a clean one on the next update.
        let mut dirty = std::mem::take(&mut self.dirty);
        dirty.fit(self.defs.len());
        for &child in &self.children[start.0] {
            dirty.insert(child.0);
        }

        let mut recomputed = 0;
        let mut from = start.0;
        while let Some(i) = dirty.pop_first(from) {
            // children only add larger indices
            from = i + 1;
            let def = self.defs[i];
            recomputed += 1;
            match def {
                Def::Expr(d) => {
                    if !self.update_expr(i, d) {
                        continue;
                    }
                }
                Def::GeneralConic(d) => {
                    if !self.update_conic(i, d) {
                        continue;
                    }
                }
                Def::Path(d) if d.has_state() => {
                    if !self.update_path(i, d) {
                        continue;
                    }
                }
                Def::Arc(d) if d.has_record() => {
                    if !self.update_arc(i, d) {
                        continue;
                    }
                }
                _ => {
                    let new = self.compute(def, i);
                    if new == self.values[i] {
                        // Nothing downstream can change because of this object.
                        continue;
                    }
                    self.values[i] = new;
                }
            }
            for &child in &self.children[i] {
                dirty.insert(child.0);
            }
        }

        self.dirty = dirty;
        recomputed
    }

    /// Recomputes the expression object `i`; tells whether its children
    /// must follow. An expression compares its raw result, infinities kept.
    /// Kept out of line so that the loop of [`Self::propagate`] stays small.
    #[inline(never)]
    fn update_expr(&mut self, i: usize, d: ExprDef) -> bool {
        if expr::has_lists(self, d) {
            let (value, changed) = expr::update_lists(self, d);
            self.values[i] = value;
            return changed;
        }
        let out = expr::evaluate(self, d);
        self.values[i] = expr::value(d, out.raw);
        expr::store(self, d, out)
    }

    /// Recomputes the conic record of object `i` from its previous record;
    /// tells whether its children must follow: whether the record changed,
    /// bit for bit. Out of line like [`Self::update_expr`].
    #[inline(never)]
    fn update_conic(&mut self, i: usize, d: GeneralConicDef) -> bool {
        let Some(slot) = self.conic_slot(i) else {
            return false;
        };
        let new = d.record(self, &self.conics[slot].1);
        if new.same(&self.conics[slot].1) {
            return false;
        }
        self.conics[slot].1 = new;
        self.values[i] = algo::general_conic::value(d.kind(), &new);
        true
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
    /// children must follow: whether the record changed, bit for bit. Out
    /// of line like [`Self::update_expr`].
    #[inline(never)]
    fn update_arc(&mut self, i: usize, d: ArcDef) -> bool {
        let Some(slot) = self.arc_slot(i) else {
            return false;
        };
        let new = d.record(self);
        if new.same(&self.arcs[slot].1) {
            return false;
        }
        self.arcs[slot].1 = new;
        self.values[i] = algo::arc::value(d.kind() == Kind::Sector, &new);
        true
    }

    /// Where the record of object `i` is kept in `arcs`.
    fn arc_slot(&self, i: usize) -> Option<usize> {
        self.arcs.binary_search_by_key(&i, |&(owner, _)| owner).ok()
    }

    /// The record of the arc or sector `id`: its conic, the range of its
    /// parameter and its measures. `None` for any other object.
    pub fn arc(&self, id: Id) -> Option<ArcData> {
        match self.defs.get(id.0)? {
            Def::Arc(d) if d.has_record() => self.arc_slot(id.0).map(|slot| self.arcs[slot].1),
            _ => None,
        }
    }

    /// Where the state of object `i` is kept in `paths`.
    fn path_slot(&self, i: usize) -> Option<usize> {
        self.paths
            .binary_search_by_key(&i, |&(owner, _)| owner)
            .ok()
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
                    | Def::Path(PathDef::On { .. })
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
        match self.defs.get(id.0)? {
            Def::GeneralConic(_) => self.conic_slot(id.0).map(|slot| self.conics[slot].1),
            _ => match self.values[id.0] {
                Value::Circle { cx, cy, r } => Some(ConicData::circle(cx, cy, r)),
                _ => None,
            },
        }
    }

    pub(crate) fn check(&self, def: &Def) -> Result<(), Error> {
        match *def {
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
            Def::Lines(d) => d.check(self),
            Def::Conic(d) => d.check(self),
            Def::Measure(d) => d.check(self),
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
        }
    }

    pub(crate) fn expect(&self, id: Id, expected: &'static [Kind]) -> Result<(), Error> {
        let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        let found = def.kind();
        if expected.contains(&found) {
            Ok(())
        } else {
            Err(Error::WrongKind {
                label: self.labels[id.0].clone(),
                expected,
                found,
            })
        }
    }

    /// `def` is the definition of the object at index `i`.
    fn compute(&self, def: Def, i: usize) -> Value {
        self.compute_with(def, |c| c.hints[i])
    }

    /// Computes `def` with the hint that `hint` gives, which is only asked
    /// for by intersections.
    fn compute_with(&self, def: Def, hint: impl Fn(&Self) -> algo::intersect::Hint) -> Value {
        match self.try_compute(def, hint) {
            Some(v) if v.is_finite() => v,
            _ => Value::Undefined,
        }
    }

    /// [`Self::try_compute`] for a command inside an expression. Out of
    /// line, so that `try_compute` stays inlined in the update loop.
    /// `hint` is the intersection hint taken when the expression was
    /// parsed; it stands for the entry in `hints` that an object has.
    #[inline(never)]
    pub(crate) fn command_value(&self, def: Def, hint: algo::intersect::Hint) -> Option<Value> {
        self.try_compute(def, |_| hint)
    }

    /// `None` means the object is undefined for the current inputs.
    #[inline(always)]
    fn try_compute(
        &self,
        def: Def,
        hint: impl Fn(&Self) -> algo::intersect::Hint,
    ) -> Option<Value> {
        match def {
            Def::FreePoint { x, y } => Some(Value::Point { x, y }),
            Def::Number(v) | Def::Angle(v) => Some(Value::Number(v)),
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
            } => algo::intersect::line_circle(self, line, circle, which, hint(self)),
            Def::IntersectCircles {
                c1,
                c2,
                which,
                special,
            } => algo::intersect::circles(self, c1, c2, which, special == Some(true), hint(self)),
            Def::Lines(d) => d.compute(self),
            Def::Conic(d) => d.compute(self),
            Def::Measure(d) => d.compute(self),
            Def::Transform(d) => d.compute(self),
            Def::Vector(d) => d.compute(self),
            Def::Predicate(d) => d.compute(self),
            Def::Point(d) => d.compute(self),
            Def::Expr(d) => Some(expr::current(self, d)),
            Def::Polygon(d) => d.compute(self),
            Def::GeneralConic(d) => d.compute(self),
            Def::ConicPart(d) => d.compute(self),
            Def::Path(d) => d.compute(self),
            Def::Arc(d) => d.compute(self),
        }
    }

    pub(crate) fn point(&self, id: Id) -> Option<(f64, f64)> {
        match self.values[id.0] {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }
    }

    /// A line with `(a, b)` of unit length (see [`Value::unit_line`]).
    #[allow(dead_code)] // part of the accessor set; every rule here reads lines raw
    pub(crate) fn line(&self, id: Id) -> Option<(f64, f64, f64)> {
        self.values[id.0].unit_line()
    }

    /// A line's raw coefficients, as GeoGebra holds them (see [`Value::Line`]).
    /// For code that follows an upstream formula on raw coefficients.
    pub(crate) fn raw_line(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Line { a, b, c } => Some((a, b, c)),
            _ => None,
        }
    }

    /// The infinite line that a line or a segment lies on, with `(a, b)` of
    /// unit length. `None` for a segment whose end points coincide.
    #[allow(dead_code)] // part of the accessor set; every rule here reads carriers raw
    pub(crate) fn carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Segment { x1, y1, x2, y2 } => line_through((x1, y1), (x2, y2)),
            v => v.unit_line(),
        }
    }

    /// The raw coefficients of the line that a line, ray or segment lies on. For
    /// a segment they are those of `GeoVec3D.lineThroughPoints` from its start
    /// to its end point, which is what GeoGebra's `GeoSegment` holds; they are
    /// all zero when the end points coincide.
    pub(crate) fn raw_carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => Some((a, b, c)),
            Value::Segment { x1, y1, x2, y2 } => Some(join((x1, y1), (x2, y2))),
            _ => None,
        }
    }

    pub(crate) fn circle(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Circle { cx, cy, r } => Some((cx, cy, r)),
            _ => None,
        }
    }

    pub(crate) fn number(&self, id: Id) -> Option<f64> {
        match self.values[id.0] {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(c.children(p[1]), &[k, a]);
        assert_eq!(c.children(p[3]), &[] as &[Id]);
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
        assert_eq!(other.list(list), &[] as &[Id]);
        assert_eq!(other.add("a", c.def(a)), Err(Error::UnknownObject));
        // a copy in the same construction may share the list
        let b = c.add("b", c.def(a)).unwrap();
        assert_eq!(c.value(b), c.value(a));
        assert_eq!(c.children(p[1]), &[k, a, b]);
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
        children: Vec<Vec<Id>>,
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
        assert_eq!(c.children(p[3]), &[c.id("a").unwrap(), t]);
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
        c.rollback(fresh).unwrap();
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
