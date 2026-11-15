// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The rules that compute one object from others, one module per group.
//!
//! The first definitions of the kernel are plain variants of [`Def`] and
//! their rules are free functions in [`basic`], [`circle`] and
//! [`intersect`]. Every later group has its own enum that implements
//! [`Group`] and is wrapped by one variant of [`Def`], so a new command is
//! added by editing the file of its group only.
//!
//! [`Def`]: crate::kernel::Def

pub mod basic;
pub mod circle;
pub mod conic;
pub mod conic_nd;
pub mod general_conic;
pub mod intersect;
pub mod lines;
pub mod measure;
pub mod point;
pub mod polygon;
pub mod predicate;
pub mod transform;
pub mod vector;

use crate::kernel::{Construction, Error, Id, IdList, Kind, Value};
use crate::tol::{EPS, STANDARD_PRECISION};

pub(crate) const POINT: &[Kind] = &[Kind::Point];
pub(crate) const LINE: &[Kind] = &[Kind::Line];
pub(crate) const LINE_LIKE: &[Kind] = &[Kind::Line, Kind::Segment];
pub(crate) const CIRCLE: &[Kind] = &[Kind::Circle];
pub(crate) const NUMBER: &[Kind] = &[Kind::Number];

/// The most objects one definition can read.
pub(crate) const MAX_PARENTS: usize = 6;

/// The objects a definition reads, without allocating.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Parents {
    ids: [Id; MAX_PARENTS],
    len: usize,
}

impl Parents {
    pub(crate) fn none() -> Self {
        Parents {
            ids: [Id(0); MAX_PARENTS],
            len: 0,
        }
    }

    /// Panics when `ids` is longer than [`MAX_PARENTS`].
    pub(crate) fn of(ids: &[Id]) -> Self {
        let mut p = Parents::none();
        p.ids[..ids.len()].copy_from_slice(ids);
        p.len = ids.len();
        p
    }

    pub(crate) fn as_slice(&self) -> &[Id] {
        &self.ids[..self.len]
    }
}

/// A group of definitions with its own enum.
pub(crate) trait Group: Copy {
    /// The kind of object this definition creates.
    fn kind(&self) -> Kind;

    /// The objects this definition reads.
    fn parents(&self) -> Parents;

    /// The stored list of further objects this definition reads, if any
    /// (see [`Construction::add_with_list`]).
    fn list(&self) -> Option<IdList> {
        None
    }

    /// The same definition reading `list` instead of its own list.
    fn with_list(self, _list: IdList) -> Self {
        self
    }

    /// Checks that the parents exist and have the right kinds.
    fn check(&self, c: &Construction) -> Result<(), Error>;

    /// The current value; `None` means undefined for the current inputs.
    /// Must not allocate: it runs on every drag.
    fn compute(&self, c: &Construction) -> Option<Value>;

    /// Parses `func(args...)` from a script line.
    ///
    /// Returns `None` when this group does not handle the command with
    /// these arguments, so that another group or the built-in commands can
    /// try. Groups are asked before the built-in commands.
    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>>;

    /// Parses a command that creates several objects, such as the two
    /// tangents of `Tangent(P, c)`, into one definition per output in
    /// GeoGebra's output order. The script layer gives them their labels.
    ///
    /// Returns `None` like [`Group::parse`]; it is asked before `parse`.
    fn parse_multi(
        _c: &Construction,
        _func: &str,
        _args: &[&str],
    ) -> Option<Result<Vec<Self>, String>> {
        None
    }
}

/// Checks an intersection index: 0 is the first point, 1 the second.
pub(crate) fn check_index(which: u8) -> Result<(), Error> {
    if which < 2 {
        Ok(())
    } else {
        Err(Error::BadIndex(which))
    }
}

/// Euclidean length. Plain `sqrt` is several times faster than `hypot`,
/// and coordinates here are nowhere near the range where it would overflow.
#[inline]
pub(crate) fn length(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// A line value from GeoGebra's raw coefficients `(x, y, z)`, kept as they
/// are. Undefined like `GeoLine.isDefined`: a NaN, or `x` and `y` both zero
/// within the standard precision (`DoubleUtil.isZero`).
pub(crate) fn line_value(x: f64, y: f64, z: f64) -> Option<Value> {
    let zero = |v: f64| -STANDARD_PRECISION < v && v < STANDARD_PRECISION;
    if x.is_nan() || y.is_nan() || z.is_nan() || (zero(x) && zero(y)) {
        return None;
    }
    Some(Value::Line { a: x, b: y, c: z })
}

/// `GeoVec3D.lineThroughPoints(P, Q, g)` for two finite points: the raw
/// coefficients of the line from `p` to `q`, all zero when they coincide.
#[inline]
pub(crate) fn join(p: (f64, f64), q: (f64, f64)) -> (f64, f64, f64) {
    (p.1 - q.1, q.0 - p.0, p.0 * q.1 - p.1 * q.0)
}

/// `GeoLine.getNormalizedCoefficients(ret, max, min)`: scales the raw
/// coefficients by powers of two until one of them lies in `[min, max]`.
/// Coefficients that are all zero or not all finite are returned as they are.
pub(crate) fn normalized_coefficients(g: (f64, f64, f64), max: f64, min: f64) -> (f64, f64, f64) {
    let zero = |v: f64| -STANDARD_PRECISION < v && v < STANDARD_PRECISION;
    let (mut x, mut y, mut z) = g;
    if (zero(x) && zero(y) && zero(z)) || !(x.is_finite() && y.is_finite() && z.is_finite()) {
        return g;
    }
    while x.abs() < min && y.abs() < min && z.abs() < min {
        x *= 2.0;
        y *= 2.0;
        z *= 2.0;
    }
    while x.abs() > max && y.abs() > max && z.abs() > max {
        x /= 2.0;
        y /= 2.0;
        z /= 2.0;
    }
    (x, y, z)
}

/// Normalised line through two points, directed from `p` to `q`.
#[allow(dead_code)] // used by `Construction::carrier`
pub(crate) fn line_through(p: (f64, f64), q: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let len = length(dx, dy);
    if len < EPS {
        return None;
    }
    let (a, b) = (-dy / len, dx / len);
    Some((a, b, -(a * p.0 + b * p.1)))
}
