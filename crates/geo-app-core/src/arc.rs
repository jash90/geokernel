//! The visible part of a very large circle, with no UI types.
//!
//! A circle whose radius is thousands of pixels cannot be handed to the
//! painter as a circle (it would be tessellated at that size, or dropped).
//! Only the arc near the drawing area matters, so it is drawn as a polyline.

/// Circles with a larger radius (in pixels) are drawn as an arc.
pub const ARC_RADIUS: f64 = 3000.0;

/// Points of the part of the circle with centre `(cx, cy)` and radius `r`
/// (pixels) that can be inside `[0, w] x [0, h]`, in order. `None` when the
/// circle does not come near the area.
pub fn visible_arc(cx: f64, cy: f64, r: f64, w: f64, h: f64) -> Option<Vec<(f64, f64)>> {
    if !(cx.is_finite() && cy.is_finite() && r.is_finite() && r > 0.0) {
        return None;
    }
    let (vx, vy) = (0.5 * w, 0.5 * h);
    // Every point of the area is within `reach` of its middle.
    let reach = 0.5 * w.hypot(h) + 20.0;
    let d = (vx - cx).hypot(vy - cy);
    if (d - r).abs() > reach {
        return None;
    }
    let theta0 = (vy - cy).atan2(vx - cx);
    // Angular half-width of the circle points that are within `reach` of the middle.
    let half = if d < 1e-9 {
        std::f64::consts::PI
    } else {
        ((r * r + d * d - reach * reach) / (2.0 * r * d))
            .clamp(-1.0, 1.0)
            .acos()
    };
    let n = ((2.0 * half * r / 40.0).ceil() as usize).clamp(8, 4000);
    Some(
        (0..=n)
            .map(|i| {
                let a = theta0 - half + 2.0 * half * i as f64 / n as f64;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 1000.0;
    const H: f64 = 800.0;

    fn inside(p: (f64, f64)) -> bool {
        (0.0..=W).contains(&p.0) && (0.0..=H).contains(&p.1)
    }

    #[test]
    fn a_huge_circle_through_the_area_gives_a_short_arc_that_crosses_it() {
        // Radius one billion pixels, its rim passing through the middle.
        let r = 1e9;
        let pts = visible_arc(500.0, 400.0 - r, r, W, H).unwrap();
        assert!(pts.len() <= 4001);
        assert!(pts.iter().any(|&p| inside(p)));
        // The arc spans the whole area from side to side.
        let xs = pts.iter().map(|p| p.0);
        let (lo, hi) = (
            xs.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
        );
        assert!(lo < 0.0 && hi > W);
        // And it stays close to the area.
        assert!(pts.iter().all(|p| p.0.abs() < 5.0 * W));
    }

    #[test]
    fn a_circle_far_from_the_area_has_no_arc() {
        assert!(visible_arc(0.0, -1e9, 1e5, W, H).is_none());
        assert!(visible_arc(f64::NAN, 0.0, 1e5, W, H).is_none());
    }

    #[test]
    fn a_big_circle_around_the_area_still_gets_its_rim_when_it_is_near() {
        let pts = visible_arc(500.0, 400.0, 6000.0, W, H);
        assert!(pts.is_none(), "rim is 6000 px away, not near");
        let pts = visible_arc(500.0, 400.0 + 3500.0, 3500.0 + 100.0, W, H).unwrap();
        assert!(pts.iter().any(|&p| inside(p)));
    }
}
