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
pub mod intersect;
pub mod lines;
pub mod measure;
pub mod transform;
pub mod vector;

use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::tol::EPS;

pub(crate) const POINT: &[Kind] = &[Kind::Point];
pub(crate) const LINE: &[Kind] = &[Kind::Line];
pub(crate) const LINE_LIKE: &[Kind] = &[Kind::Line, Kind::Segment];
pub(crate) const CIRCLE: &[Kind] = &[Kind::Circle];
pub(crate) const NUMBER: &[Kind] = &[Kind::Number];

/// The most objects one definition can read.
pub(crate) const MAX_PARENTS: usize = 4;

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

/// Normalised line through two points, directed from `p` to `q`.
pub(crate) fn line_through(p: (f64, f64), q: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let len = length(dx, dy);
    if len < EPS {
        return None;
    }
    let (a, b) = (-dy / len, dx / len);
    Some((a, b, -(a * p.0 + b * p.1)))
}
