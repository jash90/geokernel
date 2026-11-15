//! Grid and tick arithmetic with no UI types: 1-2-5 steps, tick positions and
//! tick captions.

/// The smallest 1-2-5 step (times a power of ten) that is at least `min_step`.
pub fn nice_step(min_step: f64) -> f64 {
    if !(min_step.is_finite() && min_step > 0.0) {
        return 1.0;
    }
    let exp = min_step.log10().floor();
    let base = 10f64.powf(exp);
    let m = min_step / base;
    // A little slack so that exact multiples such as 0.2 do not jump a step.
    let mantissa = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|&s| m <= s * (1.0 + 1e-9))
        .unwrap_or(10.0);
    mantissa * base
}

/// The grid of one zoom level: a major step (with captions) and a minor step
/// that divides it, or `None` when the minor lines would be too dense.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSteps {
    pub major: f64,
    pub minor: Option<f64>,
}

/// Picks the steps for `scale` pixels per world unit. Major lines are at least
/// `min_major_px` apart; minor lines at least `min_minor_px`.
pub fn grid_steps(scale: f64, min_major_px: f64, min_minor_px: f64) -> GridSteps {
    let major = nice_step(min_major_px / scale);
    // 1 and 5 split into five parts, 2 into four (0.5 steps).
    let lead = major / 10f64.powf(major.log10().floor());
    let parts = if (lead - 2.0).abs() < 1e-9 { 4.0 } else { 5.0 };
    let minor = major / parts;
    GridSteps {
        major,
        minor: (minor * scale >= min_minor_px).then_some(minor),
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
    fn steps_follow_one_two_five() {
        assert_eq!(nice_step(0.8), 1.0);
        assert_eq!(nice_step(1.0), 1.0);
        assert_eq!(nice_step(1.1), 2.0);
        assert_eq!(nice_step(2.1), 5.0);
        assert_eq!(nice_step(5.1), 10.0);
        assert!((nice_step(0.03) - 0.05).abs() < 1e-12);
        assert!((nice_step(130.0) - 200.0).abs() < 1e-9);
        assert_eq!(nice_step(f64::NAN), 1.0);
        assert_eq!(nice_step(-1.0), 1.0);
    }

    #[test]
    fn standard_view_has_unit_major_and_fifth_minor_lines() {
        let g = grid_steps(50.0, 40.0, 8.0);
        assert_eq!(g.major, 1.0);
        assert!((g.minor.unwrap() - 0.2).abs() < 1e-12);
    }

    #[test]
    fn step_two_splits_into_halves_and_dense_minor_lines_are_dropped() {
        let g = grid_steps(25.0, 40.0, 8.0);
        assert_eq!(g.major, 2.0);
        assert!((g.minor.unwrap() - 0.5).abs() < 1e-12);
        let g = grid_steps(5.0, 40.0, 8.0);
        assert_eq!(g.major, 10.0);
        assert_eq!(g.minor, Some(2.0));
        let g = grid_steps(2.0, 40.0, 8.0);
        assert_eq!(g.major, 20.0);
        assert_eq!(g.minor, Some(5.0));
        let g = grid_steps(50.0, 40.0, 11.0);
        assert_eq!(g.minor, None);
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
