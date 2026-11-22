// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianControllerCompanion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Grid capturing of the points a click places and of a dragged free point,
//! with no UI types, following the reference app's
//! `EuclidianController.transformCoords` in its default mode
//! (`POINT_CAPTURING_AUTOMATIC`, `EuclidianSettings`):
//!
//! - capturing is on only while the grid or the axes are shown
//!   (`EuclidianView.isGridOrAxesShown`);
//! - a position is taken to the nearest crossing of the major grid when it
//!   is less than `0.125` of the grid step from it on both axes (a square
//!   box, strict `<`, `POINT_CAPTURING_GRID` from
//!   `EuclidianControllerCompanion.getPointCapturingPercentage`); there is
//!   no capturing on one axis alone and none at minor grid crossings;
//! - the grid step is the axis numbering step
//!   (`ticks::numbering_step`, `EuclidianView.setAxesIntervals`);
//! - the result is cleaned of binary noise at the fifth decimal
//!   (`DoubleUtil.checkDecimalFraction`, applied by
//!   `EuclidianControllerCompanion.createNewPoint` and `.movePoint` whether
//!   or not the position was captured).

use crate::metrics::Metrics;
use crate::ticks;

/// `Math.round` of Java: halves go up, also for negative numbers.
fn java_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// The multiple of `step` nearest to `x` (`Kernel.roundToScale`).
pub fn round_to_scale(x: f64, step: f64) -> f64 {
    if step == 1.0 {
        java_round(x)
    } else {
        java_round(x / step) * step
    }
}

/// `v`, or the decimal fraction with five decimals within 1e-13 of it
/// (`DoubleUtil.checkDecimalFraction` with `Kernel.INV_MIN_PRECISION` 1e5
/// and `Kernel.STANDARD_PRECISION` 1e-8 on the scaled value), so that
/// `3 * 0.1` is stored as `0.3`.
pub fn check_decimal_fraction(v: f64) -> f64 {
    const INV_MIN_PRECISION: f64 = 1e5;
    const STANDARD_PRECISION: f64 = 1e-8;
    let frac = v * INV_MIN_PRECISION;
    let round = java_round(frac);
    // `DoubleUtil.isEqual(frac, round, STANDARD_PRECISION)`.
    if frac == round || (frac - STANDARD_PRECISION < round && round < frac + STANDARD_PRECISION) {
        round / INV_MIN_PRECISION
    } else {
        v
    }
}

/// How positions are captured at one zoom level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// The major grid step of each axis (`axesIntervalX` and -`Y`), or
    /// `None` while capturing is off.
    step: Option<(f64, f64)>,
    /// The half-width of the capturing box, as a fraction of the step.
    fraction: f64,
}

impl Snap {
    /// No capturing; positions are only cleaned (`check_decimal_fraction`).
    pub const OFF: Snap = Snap {
        step: None,
        fraction: 0.0,
    };

    /// Capturing at `xscale` / `yscale` pixels per unit, on while
    /// `grid_or_axes` (the grid or the axes are shown). Each axis takes
    /// its own step (`setAxesIntervals` runs per axis), so with the scales
    /// apart the capturing box is square in pixels, not in world units.
    pub fn new(metrics: &Metrics, xscale: f64, yscale: f64, grid_or_axes: bool) -> Snap {
        if !grid_or_axes {
            return Snap::OFF;
        }
        Snap {
            step: Some((
                ticks::numbering_step(xscale, metrics.numbering_px),
                ticks::numbering_step(yscale, metrics.numbering_px),
            )),
            fraction: metrics.grid_capture,
        }
    }

    /// The grid step positions are captured to, if capturing is on.
    pub fn step(&self) -> Option<(f64, f64)> {
        self.step
    }

    /// The world position `(x, y)` captured to the grid, if close enough
    /// to a crossing, and cleaned (the cartesian branch of
    /// `transformCoords`, then `checkDecimalFraction`): where a new point
    /// goes, and where a dragged free point goes.
    pub fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        let (x, y) = self.capture(p, (0.0, 0.0));
        (check_decimal_fraction(x), check_decimal_fraction(y))
    }

    /// The cartesian branch of `transformCoords` with the reference app's
    /// `transformCoordsOffset`: the position plus `offset` is what is
    /// captured (in a drag of a dependent object, the position its first
    /// free input point would take), and the offset is taken off again in
    /// both branches. No cleaning.
    pub fn capture(&self, (x, y): (f64, f64), offset: (f64, f64)) -> (f64, f64) {
        let (x, y) = (x + offset.0, y + offset.1);
        if let Some((step_x, step_y)) = self.step {
            let (gx, gy) = (round_to_scale(x, step_x), round_to_scale(y, step_y));
            if (gx - x).abs() < step_x * self.fraction && (gy - y).abs() < step_y * self.fraction {
                return (gx - offset.0, gy - offset.1);
            }
        }
        (x - offset.0, y - offset.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> Snap {
        Snap::new(&Metrics::desktop(), 50.0, 50.0, true)
    }

    #[test]
    fn a_position_inside_the_square_box_is_captured() {
        // At 50 px per unit the step is 1 and the box half-width 0.125
        // (6.25 px): 6 px off is captured, 7 px is not (E1); a diagonal
        // (6, 5.5) px is (the box is a square, E1d).
        let s = standard();
        assert_eq!(s.step(), Some((1.0, 1.0)));
        assert_eq!(s.apply((2.12, 3.11)), (2.0, 3.0));
        assert_eq!(s.apply((2.12, 2.89)), (2.0, 3.0));
        assert_eq!(s.apply((2.14, 2.99)), (2.14, 2.99));
        // Strict `<`: exactly on the edge of the box is not captured.
        assert_eq!(s.apply((-1.125, 4.0)), (-1.125, 4.0));
    }

    #[test]
    fn there_is_no_capturing_on_one_axis_or_at_minor_crossings() {
        let s = standard();
        // Near x = 2 but far from y = 3 (E1x).
        assert_eq!(s.apply((2.1, 3.39)), (2.1, 3.39));
        // Near the minor crossing (2.2, 3) only (E1m).
        assert_eq!(s.apply((2.22, 2.99)), (2.22, 2.99));
    }

    #[test]
    fn the_box_follows_the_grid_step_of_the_zoom() {
        // 112.5 px per unit: the step is 0.5 and the box 7.03 px (E3).
        let s = Snap::new(&Metrics::desktop(), 112.5, 112.5, true);
        assert_eq!(s.step(), Some((0.5, 0.5)));
        assert_eq!(s.apply((1.56, -0.44)), (1.5, -0.5));
        assert_eq!(s.apply((1.63, -0.5)), (1.63, -0.5));
    }

    /// With the scales apart each axis captures by its own step
    /// (`setAxesIntervals` per axis, spec D1): the box is square in
    /// pixels, not in world units.
    #[test]
    fn each_axis_takes_its_own_step_when_the_scales_are_apart() {
        // 100 px per unit on x (step 1) and 400 on y (step 0.2): 0.05 off
        // on x is inside its 0.125 box, and 0.02 off on y inside the 0.025
        // box of the 0.2 step, but 0.03 off on y is outside it, where
        // 0.125 off on x would still be taken.
        let s = Snap::new(&Metrics::desktop(), 100.0, 400.0, true);
        assert_eq!(s.step(), Some((1.0, 0.2)));
        assert_eq!(s.apply((2.05, 1.18)), (2.0, 1.2));
        assert_eq!(s.apply((2.05, 1.03)), (2.05, 1.03));
    }

    #[test]
    fn capturing_is_off_without_grid_and_axes() {
        let s = Snap::new(&Metrics::desktop(), 50.0, 50.0, false);
        assert_eq!(s, Snap::OFF);
        assert_eq!(s.apply((2.01, 2.99)), (2.01, 2.99));
    }

    #[test]
    fn positions_lose_binary_noise_at_the_fifth_decimal() {
        assert_eq!(check_decimal_fraction(3.0 * 0.1), 0.3);
        assert_eq!(check_decimal_fraction(1.234567), 1.234567);
        assert_eq!(Snap::OFF.apply((0.1 + 0.2, -0.7)), (0.3, -0.7));
        // Captured to a step of 0.1: 7 * 0.1 is 0.7000000000000001.
        assert_eq!(round_to_scale(0.69, 0.1), 7.0 * 0.1);
        assert_eq!(check_decimal_fraction(round_to_scale(0.69, 0.1)), 0.7);
    }

    #[test]
    fn an_offset_captures_the_shifted_position() {
        // The first free point would land at (2.1, 0.9): captured to (2, 1),
        // so the pointer position comes back shifted by (-0.1, 0.1).
        let s = standard();
        let (x, y) = s.capture((3.1, 1.4), (-1.0, -0.5));
        assert!(
            (x - 3.0).abs() < 1e-12 && (y - 1.5).abs() < 1e-12,
            "{x} {y}"
        );
        // Not captured: the offset goes on and comes off again.
        assert_eq!(s.capture((3.3, 1.4), (-1.0, -0.5)), (3.3, 1.4));
    }

    #[test]
    fn halves_round_up_as_in_java() {
        assert_eq!(round_to_scale(-2.5, 1.0), -2.0);
        assert_eq!(round_to_scale(2.5, 1.0), 3.0);
        assert_eq!(round_to_scale(-0.75, 0.5), -0.5);
    }
}
