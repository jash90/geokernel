//! The graphs of functions with poles, steps, gaps and wild oscillation.

use super::function::{plot, Plot};
use super::{Frame, PAD};
use crate::svg::View;

const W: f64 = 800.0;
const H: f64 = 600.0;

fn frame(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> Frame {
    Frame::fit(
        View {
            xmin,
            ymin,
            xmax,
            ymax,
        },
        W,
        H,
    )
}

fn standard() -> Frame {
    frame(-10.0, 10.0, -7.5, 7.5)
}

fn run(f: impl Fn(f64) -> f64, frame: &Frame) -> Plot {
    let mut g = |x: f64| f(x);
    plot(&mut g, frame)
}

fn points(p: &Plot) -> impl Iterator<Item = (f64, f64)> + '_ {
    p.paths.iter().flatten().copied()
}

fn segments(p: &Plot) -> impl Iterator<Item = ((f64, f64), (f64, f64))> + '_ {
    p.paths
        .iter()
        .flat_map(|path| path.windows(2).map(|w| (w[0], w[1])))
}

/// Every point lies in the area plus the padding, every path has two
/// points or more, and the plot does not depend on anything but its input.
fn check_common(f: impl Fn(f64) -> f64 + Copy, frame: &Frame) -> Plot {
    let p = run(f, frame);
    for (x, y) in points(&p) {
        assert!(x.is_finite() && y.is_finite());
        assert!(x >= -PAD - 1e-6 && x <= frame.width() + PAD + 1e-6, "x {x}");
        assert!(
            y >= -PAD - 1e-6 && y <= frame.height() + PAD + 1e-6,
            "y {y}"
        );
    }
    for path in &p.paths {
        assert!(path.len() >= 2);
    }
    assert!(points(&p).count() < 20_000, "{} points", points(&p).count());
    assert_eq!(p, run(f, frame), "the plot is deterministic");
    p
}

/// The smallest pixel distance from `(sx, sy)` to the graph near it,
/// sampled finely.
fn distance_to_graph(f: impl Fn(f64) -> f64, frame: &Frame, (sx, sy): (f64, f64)) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..=4000 {
        let px = sx - 20.0 + 40.0 * i as f64 / 4000.0;
        let (x, _) = frame.screen_to_world(px, 0.0);
        let y = f(x);
        if y.is_finite() {
            let (gx, gy) = frame.world_to_screen(x, y);
            best = best.min((gx - sx).hypot(gy - sy));
        }
    }
    best
}

fn crosses(p: &Plot, sx: f64) -> bool {
    segments(p).any(|(a, b)| (a.0 - sx) * (b.0 - sx) < 0.0)
}

#[test]
fn a_parabola_follows_the_graph_across_the_view() {
    let fr = standard();
    let f = |x: f64| x * x;
    let p = check_common(f, &fr);
    assert!(!p.paths.is_empty());
    for pt in points(&p) {
        let d = distance_to_graph(f, &fr, pt);
        assert!(d < 1.0, "point {pt:?} is {d} px off the graph");
    }
    // Both arms reach the top of the area.
    assert!(points(&p).filter(|q| q.1 <= 0.0).count() >= 2);
    let label = p.label.expect("a label");
    assert!(label.0 >= 0.0 && label.0 <= W && label.1 >= 0.0 && label.1 <= H);
}

#[test]
fn the_pole_of_one_over_x_is_never_joined() {
    let fr = standard();
    let f = |x: f64| 1.0 / x;
    let p = check_common(f, &fr);
    let (zero, _) = fr.world_to_screen(0.0, 0.0);
    assert!(!crosses(&p, zero), "a segment crosses x = 0");
    assert!(points(&p).any(|q| q.0 < zero - 100.0));
    assert!(points(&p).any(|q| q.0 > zero + 100.0));
    for pt in points(&p) {
        assert!(distance_to_graph(f, &fr, pt) < 1.0, "{pt:?}");
    }
}

#[test]
fn tangent_poles_are_never_joined() {
    let fr = standard();
    let f = f64::tan;
    let p = check_common(f, &fr);
    for k in -4..=3 {
        let pole = std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::PI;
        if pole.abs() < 10.0 {
            let (sx, _) = fr.world_to_screen(pole, 0.0);
            assert!(!crosses(&p, sx), "a segment crosses the pole at {pole}");
        }
    }
    // Every branch on the view is drawn.
    assert!(p.paths.len() >= 7, "{} paths", p.paths.len());
}

#[test]
fn a_square_root_leaves_a_gap_for_negative_x() {
    let fr = standard();
    let f = f64::sqrt;
    let p = check_common(f, &fr);
    let scale = fr.scale();
    for (sx, sy) in points(&p) {
        let (x, _) = fr.screen_to_world(sx, sy);
        assert!(x >= -0.5 / scale, "a point at x = {x}");
    }
    // The graph starts at the origin.
    let (zx, zy) = fr.world_to_screen(0.0, 0.0);
    assert!(points(&p).any(|q| (q.0 - zx).abs() < 1.0 && (q.1 - zy).abs() < 3.0));
}

#[test]
fn the_steps_of_floor_are_not_joined() {
    let fr = standard();
    let p = check_common(f64::floor, &fr);
    for (a, b) in segments(&p) {
        assert!((a.1 - b.1).abs() < 0.5, "a segment from {a:?} to {b:?}");
    }
    assert!(p.paths.len() >= 15, "{} paths", p.paths.len());
}

#[test]
fn sine_of_one_over_x_stays_bounded() {
    let fr = standard();
    let f = |x: f64| (1.0 / x).sin();
    let p = check_common(f, &fr);
    assert!(!p.paths.is_empty());
}

#[test]
fn a_constant_is_one_horizontal_line() {
    let fr = standard();
    let p = check_common(|_| 2.0, &fr);
    assert_eq!(p.paths.len(), 1);
    let path = &p.paths[0];
    let (_, y) = fr.world_to_screen(0.0, 2.0);
    assert!(path.iter().all(|q| (q.1 - y).abs() < 1e-6));
    assert!(path[0].0 <= 0.0 && path[path.len() - 1].0 >= W);
    // `CurvePlotter` samples at least `max(80, width / 6)` points.
    assert!(path.len() <= 300, "{} points", path.len());
}

#[test]
fn a_function_defined_nowhere_draws_nothing() {
    let fr = standard();
    let p = check_common(|_| f64::NAN, &fr);
    assert!(p.paths.is_empty());
    assert_eq!(p.label, None);
}

#[test]
fn a_graph_off_the_view_draws_nothing() {
    let fr = standard();
    let p = check_common(|x| x + 1000.0, &fr);
    assert!(p.paths.is_empty());
}

#[test]
fn extreme_zooms_do_not_panic() {
    for (half, cx) in [(1e-6, 0.3), (1e-6, 1e6), (1e9, 0.0), (1e9, 1e12)] {
        let fr = frame(cx - half, cx + half, -0.75 * half, 0.75 * half);
        for f in [
            (|x: f64| x * x) as fn(f64) -> f64,
            |x| 1.0 / x,
            f64::tan,
            f64::sqrt,
            f64::floor,
            |x| (1.0 / x).sin(),
        ] {
            check_common(f, &fr);
        }
    }
}

#[test]
fn a_double_pole_is_never_joined_and_both_sides_rise() {
    let fr = standard();
    let f = |x: f64| 1.0 / ((x - 1.0) * (x - 1.0));
    let p = check_common(f, &fr);
    let (pole, _) = fr.world_to_screen(1.0, 0.0);
    assert!(!crosses(&p, pole), "a segment crosses x = 1");
    // Both sides reach the top of the area.
    assert!(points(&p).any(|q| q.0 < pole && q.1 <= 0.0));
    assert!(points(&p).any(|q| q.0 > pole && q.1 <= 0.0));
    for pt in points(&p) {
        assert!(distance_to_graph(f, &fr, pt) < 1.0, "{pt:?}");
    }
}

#[test]
fn the_jump_of_sign_is_not_joined() {
    let fr = standard();
    let f = f64::signum;
    let p = check_common(f, &fr);
    let (zero, _) = fr.world_to_screen(0.0, 0.0);
    let (_, top) = fr.world_to_screen(0.0, 1.0);
    let (_, bottom) = fr.world_to_screen(0.0, -1.0);
    // No segment runs from one level to the other.
    for (a, b) in segments(&p) {
        assert!(
            (a.1 - b.1).abs() < 1.0,
            "a segment from {a:?} to {b:?} joins the jump"
        );
    }
    assert!(points(&p).any(|q| q.0 < zero - 100.0 && (q.1 - bottom).abs() < 0.5));
    assert!(points(&p).any(|q| q.0 > zero + 100.0 && (q.1 - top).abs() < 0.5));
}

#[test]
fn ln_falls_along_its_asymptote_and_is_a_gap_left_of_zero() {
    let fr = standard();
    let f = f64::ln;
    let p = check_common(f, &fr);
    let (zero, _) = fr.world_to_screen(0.0, 0.0);
    assert!(
        points(&p).all(|q| q.0 >= zero - 1e-6),
        "drawn left of x = 0"
    );
    // It falls far along the asymptote, within a pixel of x = 0, and stops
    // at y = -6.015 here, short of the bottom border at -7.5: the
    // bisection of `CurvePlotter` refines the jump to the undefined part
    // only until two samples are `GeoFunction.getMinDistX` (0.1 px) apart
    // in x, and the last defined sample is ln(0.00244). Upstream ends the
    // graph at the same point for this view.
    let low = points(&p).fold((0.0f64, 0.0f64), |m, q| if q.1 > m.1 { q } else { m });
    let (lx, ly) = fr.screen_to_world(low.0, low.1);
    assert!(
        low.0 - zero < 0.1 && (ly + 6.015).abs() < 1e-3 && (lx - 0.00244).abs() < 1e-5,
        "lowest point {low:?}, ({lx}, {ly}) in the plane"
    );
    for pt in points(&p) {
        if pt.1 < H {
            assert!(distance_to_graph(f, &fr, pt) < 1.0, "{pt:?}");
        }
    }
}

/// The paths that cross the column `sx`, top and bottom of the whole cover.
fn column_cover(p: &Plot, sx: f64) -> (f64, f64) {
    let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
    for (a, b) in segments(p) {
        if (a.0 - sx).abs() < 1.0 && (b.0 - sx).abs() < 1.0 {
            top = top.min(a.1.min(b.1));
            bottom = bottom.max(a.1.max(b.1));
        }
    }
    (top, bottom)
}

#[test]
fn a_nearly_vertical_graph_is_cut_to_the_whole_column() {
    // A slope of 1e20 is vertical on the screen: the line runs from the top
    // border to the bottom one, both ends on the padded rectangle.
    for (fr, slope) in [
        (standard(), 1e20),
        (frame(-9.7, 10.3, -7.5, 7.5), 1e20),
        (frame(-9.7, 10.3, -7.5, 7.5), 1e300),
        (standard(), 1e300),
    ] {
        let f = move |x: f64| slope * x;
        let p = check_common(f, &fr);
        let (zero, _) = fr.world_to_screen(0.0, 0.0);
        let (top, bottom) = column_cover(&p, zero);
        assert!(
            (top + PAD).abs() < 1e-6 && (bottom - H - PAD).abs() < 1e-6,
            "{slope} x in {:?}: column covered from {top} to {bottom}",
            fr.visible(0.0)
        );
    }
}

#[test]
fn a_plot_with_a_margin_is_the_area_plot_and_covers_the_margin() {
    let m = 200.0;
    let fr = frame(-9.3, 10.7, -6.8, 8.2);
    for f in [
        (|x: f64| (3.0 * x).sin() + x / 2.0) as fn(f64) -> f64,
        |x| x * x / 4.0 - 5.0,
        |x| 1.0 / (x - 0.4),
        f64::ln,
    ] {
        let area = run(f, &fr);
        let mut g = |x: f64| f(x);
        let wide = super::function::plot_with_margin(&mut g, &fr, m);
        // The area's paths first, unchanged, and its label.
        assert_eq!(wide.paths[..area.paths.len()], area.paths[..]);
        assert_eq!(wide.label, area.label);
        // Every point of the graph in the area and the margin, away from
        // poles and the end of the domain, is within half a pixel of a path.
        let near = |q: (f64, f64)| {
            segments(&wide).any(|(a, b)| {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len2 = dx * dx + dy * dy;
                let t = if len2 > 0.0 {
                    (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (a.0 + t * dx - q.0).hypot(a.1 + t * dy - q.1) < 0.5
            })
        };
        let mut checked = 0;
        for i in 0..=((W + 2.0 * m) as usize) {
            let sx = i as f64 - m;
            let (x, _) = fr.screen_to_world(sx, 0.0);
            let (x0, _) = fr.screen_to_world(sx - 3.0, 0.0);
            let (x1, _) = fr.screen_to_world(sx + 3.0, 0.0);
            let ys = [f(x0), f(x), f(x1)].map(|y| fr.world_to_screen(x, y).1);
            let calm = ys
                .iter()
                .all(|y| y.is_finite() && *y > -m + PAD && *y < H + m - PAD)
                && (ys[0] - ys[2]).abs() < 60.0;
            if calm {
                checked += 1;
                assert!(near((sx, ys[1])), "({sx}, {}) is not drawn", ys[1]);
            }
        }
        assert!(checked > 300, "{checked} points checked");
    }
}
