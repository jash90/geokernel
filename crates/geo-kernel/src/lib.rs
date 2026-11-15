//! geo-kernel: a dynamic-geometry construction kernel.
//!
//! * [`kernel`] holds the construction (a dependency graph) and recomputes
//!   only what depends on the object that moved.
//! * [`algo`] holds the rules that compute one object from others.
//! * [`tol`] holds the numeric tolerances.
//! * [`script`] parses a small input-bar style language.
//! * [`expr`] holds arithmetic expressions such as `c = a + 2*b`.
//! * [`svg`] draws a construction to an SVG string.

pub mod algo;
pub mod expr;
pub mod kernel;
pub mod script;
pub mod svg;
pub mod tol;

pub use algo::conic_nd::{ConicData, ConicType};
pub use algo::path::PathState;
pub use kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
