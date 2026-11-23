//! geo-kernel: a dynamic-geometry construction kernel.
//!
//! * [`kernel`] holds the construction (a dependency graph) and recomputes
//!   only what depends on the object that moved.
//! * [`algo`] holds the rules that compute one object from others.
//! * [`tol`] holds the numeric tolerances.
//! * [`equation`] writes the equation of a conic.
//! * [`script`] parses a small input-bar style language.
//! * [`expr`] holds arithmetic expressions such as `c = a + 2*b`.
//! * [`svg`] draws a construction to an SVG string.
//! * [`plot`] cuts conics and function graphs to a view, for the SVG writer
//!   and the window alike.

pub mod algo;
pub mod base64;
mod cas;
pub mod equation;
pub mod expr;
pub mod kernel;
pub mod plot;
pub mod script;
pub mod svg;
pub mod tol;

/// Whether CAS commands are evaluated by Giac in this process (the `giac`
/// feature with the pinned library loaded; docs/cas-giac.md).
pub fn giac_backend_active() -> bool {
    #[cfg(feature = "giac")]
    {
        geo_giac::backend() == geo_giac::Backend::Giac
    }
    #[cfg(not(feature = "giac"))]
    {
        false
    }
}

pub use algo::arc::ArcData;
pub use algo::conic_nd::{ConicData, ConicType};
pub use algo::path::PathState;
pub use kernel::locus::{LocusData, LocusDef, LocusPoint};
pub use kernel::{Construction, Def, Error, Id, IdList, Kind, Value};
