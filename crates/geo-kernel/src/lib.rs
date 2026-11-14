//! geo-kernel: a dynamic-geometry construction kernel.
//!
//! * [`kernel`] holds the construction (a dependency graph) and recomputes
//!   only what depends on the object that moved.
//! * [`algo`] holds the rules that compute one object from others.
//! * [`tol`] holds the numeric tolerances.
//! * [`script`] parses a small input-bar style language.
//! * [`svg`] draws a construction to an SVG string.

pub mod algo;
pub mod kernel;
pub mod script;
pub mod svg;
pub mod tol;

pub use kernel::{Construction, Def, Error, Id, Kind, Value};
