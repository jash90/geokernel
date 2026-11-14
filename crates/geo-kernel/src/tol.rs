//! Numeric tolerances.
//!
//! The three `*_PRECISION` constants have the same values as the constants
//! of the same names in GeoGebra's `Kernel`, so that ported algorithms make
//! the same decisions as the original.

/// GeoGebra's default tolerance for "is this zero" and "are these equal".
pub const STANDARD_PRECISION: f64 = 1e-8;
/// GeoGebra's loosest tolerance.
pub const MIN_PRECISION: f64 = 1e-5;
/// GeoGebra's tightest tolerance.
pub const MAX_PRECISION: f64 = 1e-12;

/// Below this, lengths and determinants are treated as zero.
/// Used by the algorithms that were not ported from GeoGebra.
pub const EPS: f64 = 1e-12;
/// Relative slack that lets tangent circles and lines still intersect.
/// Used by the algorithms that were not ported from GeoGebra.
pub const TANGENT_TOL: f64 = 1e-9;
