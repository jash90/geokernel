// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/DrawGrid.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Grid and tick arithmetic with no UI types: the numbering step, tick
//! positions and tick captions.

use crate::snap::check_decimal_fraction;

/// The axis numbering step at `scale` pixels per unit, allowing one number
/// per `max_pix` pixels (`EuclidianView.setAxesIntervals`, `maxPix` 100):
/// with `units = max_pix / scale`, `pot` its power of ten and
/// `n = units / pot`, the step is `5 pot` for `n > 5`, `2 pot` for `n > 2`
/// and `pot` otherwise. The major grid step is the same
/// (`DEFAULT_GRID_DIST_FACTOR` 1).
pub fn numbering_step(scale: f64, max_pix: f64) -> f64 {
    let units = max_pix / scale;
    if !(units.is_finite() && units > 0.0) {
        return 1.0;
    }
    let exp = units.log10().floor();
    // "see #2682": both are cleaned of binary noise.
    let pot = check_decimal_fraction(10f64.powf(exp));
    let n = check_decimal_fraction(units / pot);
    if n > 5.0 {
        5.0 * pot
    } else if n > 2.0 {
        2.0 * pot
    } else {
        pot
    }
}

/// The grid of one zoom level: a major step (with captions) and the minor
/// step that divides it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSteps {
    pub major: f64,
    pub minor: f64,
}

/// `DrawGrid.drawVerticalGridLinear` / `drawHorizontalGridLinear` with the
/// default grid of the reference app (`GRID_CARTESIAN_WITH_SUBGRID`): the
/// major lines are the axis numbering step apart (one number per
/// `numbering_px`, see [`numbering_step`]), and every major cell is split
/// into five, whatever the step and however dense (major cells are 40 to
/// 100 px, so minor lines are 8 to 20 px apart).
pub fn grid_steps(scale: f64, numbering_px: f64) -> GridSteps {
    let major = numbering_step(scale, numbering_px);
    GridSteps {
        major,
        minor: major / 5.0,
    }
}

/// Multiples of `step` in `[min, max]`, as integer indices (value = index * step)
/// so that no rounding error accumulates.
pub fn multiples(min: f64, max: f64, step: f64) -> std::ops::Range<i64> {
    if !(min.is_finite() && max.is_finite() && step > 0.0) {
        return 0..0;
    }
    let first = (min / step).ceil() as i64;
    let last = (max / step).floor() as i64;
    // Guard against absurd counts if the caller passes a tiny step.
    if last - first > 10_000 {
        return 0..0;
    }
    first..last + 1
}

/// Caption of the tick `index * step`: as many decimals as the step needs (so
/// `1.0` for a step of 0.5), and never a negative zero.
pub fn caption(index: i64, step: f64) -> String {
    let value = index as f64 * step;
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10()).ceil().clamp(0.0, 12.0) as usize
    };
    let s = format!("{value:.decimals$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_owned()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbering_step_follows_set_axes_intervals() {
        // units = 100 / scale; n > 5 -> 5 pot, n > 2 -> 2 pot, else pot.
        assert_eq!(numbering_step(50.0, 100.0), 1.0);
        assert_eq!(numbering_step(112.5, 100.0), 0.5);
        // units 5: n = 5 is not above 5 (a 1-2-5 step of at least 48 px,
        // the rule before, gave 5).
        assert_eq!(numbering_step(20.0, 100.0), 2.0);
        assert_eq!(numbering_step(30.0, 100.0), 2.0);
        assert_eq!(numbering_step(100.0, 100.0), 1.0);
        assert_eq!(numbering_step(101.0, 100.0), 0.5);
        // units 0.05: n = 5 after `checkDecimalFraction` (5.000000000000001
        // before), so 2 pot.
        assert_eq!(numbering_step(2000.0, 100.0), 0.02);
        assert_eq!(numbering_step(1999.0, 100.0), 0.05);
        assert_eq!(numbering_step(0.5, 100.0), 100.0);
        assert_eq!(numbering_step(0.4, 100.0), 200.0);
        assert_eq!(numbering_step(f64::NAN, 100.0), 1.0);
        assert_eq!(numbering_step(-1.0, 100.0), 1.0);
    }

    #[test]
    fn standard_view_has_unit_major_and_fifth_minor_lines() {
        let g = grid_steps(50.0, 100.0);
        assert_eq!(g.major, 1.0);
        assert!((g.minor - 0.2).abs() < 1e-12);
    }

    /// Was `step_two_splits_into_halves_and_dense_minor_lines_are_dropped`
    /// (a 2-step split into four, no minor lines under 9 px):
    /// `DrawGrid` always splits into five (`n = 5`) and drops none.
    #[test]
    fn every_major_cell_splits_into_five() {
        let g = grid_steps(25.0, 100.0);
        assert_eq!(g.major, 2.0);
        assert!((g.minor - 0.4).abs() < 1e-12);
        assert_eq!(
            grid_steps(5.0, 100.0),
            GridSteps {
                major: 10.0,
                minor: 2.0
            }
        );
        assert_eq!(
            grid_steps(2.0, 100.0),
            GridSteps {
                major: 20.0,
                minor: 4.0
            }
        );
        // The densest grid: at 200 px per unit the step is 0.2 (40 px), so
        // minor lines are 8 px apart, and kept (they were dropped).
        let g = grid_steps(200.0, 100.0);
        assert_eq!(g.major, 0.2);
        assert!((g.minor * 200.0 - 8.0).abs() < 1e-9);
    }

    #[test]
    fn multiples_cover_the_range_exactly() {
        assert_eq!(multiples(-2.5, 3.2, 1.0), -2..4);
        assert_eq!(multiples(0.1, 0.9, 1.0).count(), 0);
        assert_eq!(multiples(f64::NAN, 1.0, 1.0).count(), 0);
        assert_eq!(multiples(0.0, 1e12, 1e-3).count(), 0);
    }

    #[test]
    fn captions_have_the_right_decimals_and_no_negative_zero() {
        assert_eq!(caption(3, 1.0), "3");
        assert_eq!(caption(-4, 2.0), "-8");
        assert_eq!(caption(3, 0.2), "0.6");
        assert_eq!(caption(-1, 0.05), "-0.05");
        assert_eq!(caption(0, 0.5), "0.0");
        assert_eq!(caption(-0, 1.0), "0");
        assert_eq!(caption(2, 500.0), "1000");
    }
}
