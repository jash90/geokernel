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

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;

/// Below this, lengths and determinants are treated as zero.
const EPS: f64 = 1e-12;
/// Relative slack that lets tangent circles and lines still intersect.
const TANGENT_TOL: f64 = 1e-9;

/// Handle of an object inside one [`Construction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(usize);

impl Id {
    pub fn index(self) -> usize {
        self.0
    }
}

/// What an object is, independent of whether it is currently defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Point,
    Line,
    Segment,
    Circle,
    Number,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Point => "point",
            Kind::Line => "line",
            Kind::Segment => "segment",
            Kind::Circle => "circle",
            Kind::Number => "number",
        })
    }
}

/// Current numeric state of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Point {
        x: f64,
        y: f64,
    },
    /// `a*x + b*y + c = 0` with `(a, b)` of unit length.
    /// The line's direction is `(b, -a)`.
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
    /// The object does not exist for the current positions
    /// (for example two circles that no longer meet).
    Undefined,
}

impl Value {
    pub fn is_defined(&self) -> bool {
        !matches!(self, Value::Undefined)
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
            Value::Undefined => true,
        }
    }
}

/// How an object is defined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    /// A point the user can drag.
    FreePoint {
        x: f64,
        y: f64,
    },
    /// A number the user can change (a slider).
    Number(f64),
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
    /// direction from the first centre to the second.
    IntersectCircles {
        c1: Id,
        c2: Id,
        which: u8,
    },
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
            Def::Number(_) | Def::Distance(..) => Kind::Number,
        }
    }

    pub fn is_free(&self) -> bool {
        matches!(self, Def::FreePoint { .. } | Def::Number(_))
    }

    /// The objects this definition reads (at most two).
    fn parents(&self) -> ([Id; 2], usize) {
        match *self {
            Def::FreePoint { .. } | Def::Number(_) => ([Id(0); 2], 0),
            Def::Line(p, q)
            | Def::Segment(p, q)
            | Def::Midpoint(p, q)
            | Def::Distance(p, q)
            | Def::IntersectLines(p, q) => ([p, q], 2),
            Def::Perpendicular { through, to } | Def::Parallel { through, to } => {
                ([through, to], 2)
            }
            Def::CircleThrough { center, through } => ([center, through], 2),
            Def::CircleRadius { center, radius } => ([center, radius], 2),
            Def::IntersectLineCircle { line, circle, .. } => ([line, circle], 2),
            Def::IntersectCircles { c1, c2, .. } => ([c1, c2], 2),
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

/// A geometric construction.
#[derive(Default)]
pub struct Construction {
    defs: Vec<Def>,
    values: Vec<Value>,
    labels: Vec<String>,
    /// For each object, the objects that read it directly.
    children: Vec<Vec<Id>>,
    by_label: HashMap<String, Id>,
    // Scratch state for updates, kept to avoid allocating on every drag.
    mark: Vec<u32>,
    epoch: u32,
    heap: BinaryHeap<Reverse<usize>>,
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

    /// Adds an object and computes its value.
    pub fn add(&mut self, label: &str, def: Def) -> Result<Id, Error> {
        if label.is_empty() {
            return Err(Error::EmptyLabel);
        }
        if self.by_label.contains_key(label) {
            return Err(Error::DuplicateLabel(label.to_string()));
        }
        self.check(&def)?;

        let id = Id(self.defs.len());
        let value = self.compute(def);
        let (parents, n) = def.parents();
        for k in 0..n {
            if k == 1 && parents[0] == parents[1] {
                continue;
            }
            self.children[parents[k].0].push(id);
        }
        self.defs.push(def);
        self.values.push(value);
        self.labels.push(label.to_string());
        self.children.push(Vec::new());
        self.mark.push(0);
        self.by_label.insert(label.to_string(), id);
        Ok(id)
    }

    /// Moves a free point and recomputes everything that depends on it.
    /// Returns how many dependent objects were recomputed.
    pub fn move_point(&mut self, id: Id, x: f64, y: f64) -> Result<usize, Error> {
        let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        if !matches!(def, Def::FreePoint { .. }) {
            return Err(Error::NotFree(self.labels[id.0].clone()));
        }
        if !(x.is_finite() && y.is_finite()) {
            return Err(Error::NotFinite);
        }
        let new = Value::Point { x, y };
        if self.values[id.0] == new {
            return Ok(0);
        }
        self.defs[id.0] = Def::FreePoint { x, y };
        self.values[id.0] = new;
        Ok(self.propagate(id))
    }

    /// Changes a free number and recomputes everything that depends on it.
    /// Returns how many dependent objects were recomputed.
    pub fn set_number(&mut self, id: Id, v: f64) -> Result<usize, Error> {
        let def = self.defs.get(id.0).ok_or(Error::UnknownObject)?;
        if !matches!(def, Def::Number(_)) {
            return Err(Error::NotFree(self.labels[id.0].clone()));
        }
        if !v.is_finite() {
            return Err(Error::NotFinite);
        }
        let new = Value::Number(v);
        if self.values[id.0] == new {
            return Ok(0);
        }
        self.defs[id.0] = Def::Number(v);
        self.values[id.0] = new;
        Ok(self.propagate(id))
    }

    /// Recomputes the descendants of `start` in creation order.
    ///
    /// A min-heap of dirty indices gives the order. Because children always
    /// have larger indices than their parents, by the time an object is
    /// popped every parent that could still change has already been handled.
    fn propagate(&mut self, start: Id) -> usize {
        self.epoch = self.epoch.wrapping_add(1);
        let epoch = self.epoch;
        let mut heap = std::mem::take(&mut self.heap);

        for &child in &self.children[start.0] {
            if self.mark[child.0] != epoch {
                self.mark[child.0] = epoch;
                heap.push(Reverse(child.0));
            }
        }

        let mut recomputed = 0;
        while let Some(Reverse(i)) = heap.pop() {
            let new = self.compute(self.defs[i]);
            recomputed += 1;
            if new == self.values[i] {
                // Nothing downstream can change because of this object.
                continue;
            }
            self.values[i] = new;
            for &child in &self.children[i] {
                if self.mark[child.0] != epoch {
                    self.mark[child.0] = epoch;
                    heap.push(Reverse(child.0));
                }
            }
        }

        self.heap = heap;
        recomputed
    }

    fn check(&self, def: &Def) -> Result<(), Error> {
        const POINT: &[Kind] = &[Kind::Point];
        const LINE: &[Kind] = &[Kind::Line];
        const LINE_LIKE: &[Kind] = &[Kind::Line, Kind::Segment];
        const CIRCLE: &[Kind] = &[Kind::Circle];
        const NUMBER: &[Kind] = &[Kind::Number];

        match *def {
            Def::FreePoint { x, y } => {
                if x.is_finite() && y.is_finite() {
                    Ok(())
                } else {
                    Err(Error::NotFinite)
                }
            }
            Def::Number(v) => {
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
            Def::IntersectCircles { c1, c2, which } => {
                self.expect(c1, CIRCLE)?;
                self.expect(c2, CIRCLE)?;
                check_index(which)
            }
        }
    }

    fn expect(&self, id: Id, expected: &'static [Kind]) -> Result<(), Error> {
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

    fn compute(&self, def: Def) -> Value {
        match self.try_compute(def) {
            Some(v) if v.is_finite() => v,
            _ => Value::Undefined,
        }
    }

    /// `None` means the object is undefined for the current inputs.
    fn try_compute(&self, def: Def) -> Option<Value> {
        Some(match def {
            Def::FreePoint { x, y } => Value::Point { x, y },
            Def::Number(v) => Value::Number(v),
            Def::Line(p, q) => {
                let (a, b, c) = line_through(self.point(p)?, self.point(q)?)?;
                Value::Line { a, b, c }
            }
            Def::Segment(p, q) => {
                let (x1, y1) = self.point(p)?;
                let (x2, y2) = self.point(q)?;
                Value::Segment { x1, y1, x2, y2 }
            }
            Def::Midpoint(p, q) => {
                let (x1, y1) = self.point(p)?;
                let (x2, y2) = self.point(q)?;
                Value::Point {
                    x: 0.5 * (x1 + x2),
                    y: 0.5 * (y1 + y2),
                }
            }
            Def::Distance(p, q) => {
                let (x1, y1) = self.point(p)?;
                let (x2, y2) = self.point(q)?;
                Value::Number(length(x2 - x1, y2 - y1))
            }
            Def::Perpendicular { through, to } => {
                let (px, py) = self.point(through)?;
                let (a, b, _) = self.carrier(to)?;
                // The new line runs along the old normal (a, b).
                let (na, nb) = (-b, a);
                Value::Line {
                    a: na,
                    b: nb,
                    c: -(na * px + nb * py),
                }
            }
            Def::Parallel { through, to } => {
                let (px, py) = self.point(through)?;
                let (a, b, _) = self.carrier(to)?;
                Value::Line {
                    a,
                    b,
                    c: -(a * px + b * py),
                }
            }
            Def::CircleThrough { center, through } => {
                let (cx, cy) = self.point(center)?;
                let (px, py) = self.point(through)?;
                Value::Circle {
                    cx,
                    cy,
                    r: length(px - cx, py - cy),
                }
            }
            Def::CircleRadius { center, radius } => {
                let (cx, cy) = self.point(center)?;
                let r = self.number(radius)?;
                if r < 0.0 {
                    return None;
                }
                Value::Circle { cx, cy, r }
            }
            Def::IntersectLines(l, m) => {
                let (a1, b1, c1) = self.line(l)?;
                let (a2, b2, c2) = self.line(m)?;
                let det = a1 * b2 - a2 * b1;
                if det.abs() < EPS {
                    return None; // parallel
                }
                Value::Point {
                    x: (b1 * c2 - b2 * c1) / det,
                    y: (a2 * c1 - a1 * c2) / det,
                }
            }
            Def::IntersectLineCircle {
                line,
                circle,
                which,
            } => {
                let (a, b, c) = self.line(line)?;
                let (cx, cy, r) = self.circle(circle)?;
                // Signed distance from the centre to the line.
                let d = a * cx + b * cy + c;
                let h2 = r * r - d * d;
                if h2 < -TANGENT_TOL * (r * r).max(1.0) {
                    return None;
                }
                let h = h2.max(0.0).sqrt();
                let (fx, fy) = (cx - a * d, cy - b * d);
                let s = if which == 0 { -h } else { h };
                Value::Point {
                    x: fx + s * b,
                    y: fy - s * a,
                }
            }
            Def::IntersectCircles { c1, c2, which } => {
                let (x1, y1, r1) = self.circle(c1)?;
                let (x2, y2, r2) = self.circle(c2)?;
                let (dx, dy) = (x2 - x1, y2 - y1);
                let d = length(dx, dy);
                if d < EPS {
                    return None; // concentric
                }
                let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
                let h2 = r1 * r1 - along * along;
                if h2 < -TANGENT_TOL * (r1 * r1).max(1.0) {
                    return None;
                }
                let h = h2.max(0.0).sqrt();
                let (ux, uy) = (dx / d, dy / d);
                let (bx, by) = (x1 + along * ux, y1 + along * uy);
                let s = if which == 0 { h } else { -h };
                Value::Point {
                    x: bx - s * uy,
                    y: by + s * ux,
                }
            }
        })
    }

    fn point(&self, id: Id) -> Option<(f64, f64)> {
        match self.values[id.0] {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }
    }

    fn line(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Line { a, b, c } => Some((a, b, c)),
            _ => None,
        }
    }

    /// The infinite line that a line or a segment lies on.
    fn carrier(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Line { a, b, c } => Some((a, b, c)),
            Value::Segment { x1, y1, x2, y2 } => line_through((x1, y1), (x2, y2)),
            _ => None,
        }
    }

    fn circle(&self, id: Id) -> Option<(f64, f64, f64)> {
        match self.values[id.0] {
            Value::Circle { cx, cy, r } => Some((cx, cy, r)),
            _ => None,
        }
    }

    fn number(&self, id: Id) -> Option<f64> {
        match self.values[id.0] {
            Value::Number(v) => Some(v),
            _ => None,
        }
    }
}

fn check_index(which: u8) -> Result<(), Error> {
    if which < 2 {
        Ok(())
    } else {
        Err(Error::BadIndex(which))
    }
}

/// Euclidean length. Plain `sqrt` is several times faster than `hypot`,
/// and coordinates here are nowhere near the range where it would overflow.
#[inline]
fn length(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// Normalised line through two points, directed from `p` to `q`.
fn line_through(p: (f64, f64), q: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let len = length(dx, dy);
    if len < EPS {
        return None;
    }
    let (a, b) = (-dy / len, dx / len);
    Some((a, b, -(a * p.0 + b * p.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
