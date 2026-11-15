//! The visible ranges of conics checked against dense sampling, and the
//! drawn polylines checked against the curves.

use std::f64::consts::TAU;

use super::conic::{
    ellipse_ranges, hyperbola_ranges, parabola_ranges, plot, Eigen, Plot, FLATNESS, HUGE_RADIUS,
};
use super::{inside, Frame, PAD};
use crate::algo::conic_nd::{ConicData, ConicType};
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

/// A small deterministic generator.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn eigen(b: [f64; 2], angle: f64) -> Eigen {
    let (s, c) = angle.sin_cos();
    Eigen {
        b,
        e0: [c, s],
        e1: [-s, c],
    }
}

fn rect() -> View {
    View {
        xmin: -4.0,
        ymin: -3.0,
        xmax: 4.0,
        ymax: 3.0,
    }
}

fn contains(ranges: &[(f64, f64)], t: f64, period: Option<f64>) -> bool {
    ranges.iter().any(|&(t0, t1)| match period {
        Some(p) => [t - p, t, t + p].iter().any(|&s| s >= t0 && s <= t1),
        None => t >= t0 && t <= t1,
    })
}

/// Samples the parameter densely and compares, away from the borders
/// between inside and outside, the samples inside `r` with the ranges.
fn compare(
    ranges: &[(f64, f64)],
    at: impl Fn(f64) -> (f64, f64),
    r: &View,
    t_lo: f64,
    t_hi: f64,
    period: Option<f64>,
) {
    const N: usize = 20_000;
    let ts: Vec<f64> = (0..=N)
        .map(|i| t_lo + (t_hi - t_lo) * i as f64 / N as f64)
        .collect();
    let ins: Vec<bool> = ts.iter().map(|&t| inside(r, at(t))).collect();
    for i in 1..N {
        if ins[i - 1] == ins[i] && ins[i] == ins[i + 1] {
            assert_eq!(
                contains(ranges, ts[i], period),
                ins[i],
                "t = {} inside = {} ranges {ranges:?}",
                ts[i],
                ins[i]
            );
        }
    }
    for w in ranges.windows(2) {
        assert!(w[0].1 < w[1].0, "ranges overlap or touch: {ranges:?}");
    }
}

#[test]
fn ellipse_ranges_match_sampling() {
    let mut g = Lcg(7);
    let r = rect();
    for _ in 0..300 {
        let c = eigen([g.range(-8.0, 8.0), g.range(-6.0, 6.0)], g.range(0.0, TAU));
        let (a, b) = (g.range(0.05, 12.0), g.range(0.05, 12.0));
        let ranges = ellipse_ranges(&c, a, b, &r);
        compare(
            &ranges,
            |t| c.at(a * t.cos(), b * t.sin()),
            &r,
            0.0,
            TAU,
            Some(TAU),
        );
    }
}

#[test]
fn hyperbola_ranges_match_sampling() {
    let mut g = Lcg(11);
    let r = rect();
    for _ in 0..300 {
        let c = eigen([g.range(-8.0, 8.0), g.range(-6.0, 6.0)], g.range(0.0, TAU));
        let (a, b) = (g.range(0.05, 6.0), g.range(0.05, 6.0));
        for s in [1.0, -1.0] {
            let ranges = hyperbola_ranges(&c, s * a, b, &r);
            let at = |t: f64| c.at(s * a * t.cosh(), b * t.sinh());
            compare(&ranges, at, &r, -8.0, 8.0, None);
        }
    }
}

#[test]
fn parabola_ranges_match_sampling() {
    let mut g = Lcg(13);
    let r = rect();
    for _ in 0..300 {
        let c = eigen([g.range(-8.0, 8.0), g.range(-6.0, 6.0)], g.range(0.0, TAU));
        let p = g.range(0.02, 8.0);
        let ranges = parabola_ranges(&c, p, &r);
        let at = |t: f64| c.at(t * t / (2.0 * p), t);
        compare(&ranges, at, &r, -60.0, 60.0, None);
    }
}

fn conic(ty: ConicType, b: [f64; 2], angle: f64, half_axes: [f64; 2], p: f64) -> ConicData {
    let mut d = ConicData::new();
    let (s, c) = angle.sin_cos();
    d.ty = ty;
    d.defined = true;
    d.midpoint = b;
    d.eigenvec = [[c, s], [-s, c]];
    d.half_axes = half_axes;
    d.p = p;
    d
}

/// The implicit equation of `d` in its eigenvector system and its
/// gradient, to measure how far a point is from the curve.
fn implicit(d: &ConicData, (x, y): (f64, f64)) -> (f64, [f64; 2]) {
    let (dx, dy) = (x - d.midpoint[0], y - d.midpoint[1]);
    let [e0, e1] = d.eigenvec;
    let u = dx * e0[0] + dy * e0[1];
    let v = dx * e1[0] + dy * e1[1];
    let [a, b] = d.half_axes;
    match d.ty {
        ConicType::Ellipse | ConicType::Circle => (
            (u / a).powi(2) + (v / b).powi(2) - 1.0,
            [2.0 * u / (a * a), 2.0 * v / (b * b)],
        ),
        ConicType::Hyperbola => (
            (u / a).powi(2) - (v / b).powi(2) - 1.0,
            [2.0 * u / (a * a), -2.0 * v / (b * b)],
        ),
        ConicType::Parabola => (v * v - 2.0 * d.p * u, [-2.0 * d.p, 2.0 * v]),
        _ => unreachable!(),
    }
}

/// Every drawn point lies within the flatness of the curve and inside the
/// padded area.
fn check_on_curve(d: &ConicData, fr: &Frame, p: &Plot) {
    for &(sx, sy) in p.paths.iter().flatten() {
        assert!(sx >= -PAD - 1e-6 && sx <= fr.width() + PAD + 1e-6, "{sx}");
        assert!(sy >= -PAD - 1e-6 && sy <= fr.height() + PAD + 1e-6, "{sy}");
        let (f, g) = implicit(d, fr.screen_to_world(sx, sy));
        let dist = f.abs() / g[0].hypot(g[1]) * fr.scale();
        assert!(dist <= FLATNESS + 0.05, "{dist} px off the curve");
    }
}

fn distance_to_paths(p: &Plot, q: (f64, f64)) -> f64 {
    let mut best = f64::INFINITY;
    for path in &p.paths {
        for w in path.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            best = best.min((a.0 + t * dx - q.0).hypot(a.1 + t * dy - q.1));
        }
    }
    best
}

/// Every densely sampled point of the curve inside the area is covered by
/// a drawn segment.
fn check_covered(p: &Plot, fr: &Frame, samples: impl Iterator<Item = (f64, f64)>) {
    for (x, y) in samples {
        let (sx, sy) = fr.world_to_screen(x, y);
        if sx >= 0.0 && sx <= fr.width() && sy >= 0.0 && sy <= fr.height() {
            let d = distance_to_paths(p, (sx, sy));
            assert!(
                d <= FLATNESS + 0.05,
                "({sx}, {sy}) is {d} px from the drawing"
            );
        }
    }
}

#[test]
fn ellipses_are_drawn_on_the_curve_and_completely() {
    let fr = standard();
    let mut g = Lcg(17);
    for i in 0..40 {
        let ty = if i % 5 == 0 {
            ConicType::Circle
        } else {
            ConicType::Ellipse
        };
        let a = g.range(0.1, 20.0);
        let b = if ty == ConicType::Circle {
            a
        } else {
            g.range(0.1, 20.0)
        };
        let d = conic(
            ty,
            [g.range(-12.0, 12.0), g.range(-9.0, 9.0)],
            g.range(0.0, TAU),
            [a, b],
            0.0,
        );
        let p = plot(&d, &fr);
        check_on_curve(&d, &fr, &p);
        let [e0, e1] = d.eigenvec;
        let samples = (0..4000).map(|k| {
            let t = TAU * k as f64 / 4000.0;
            let (u, v) = (a * t.cos(), b * t.sin());
            (
                d.midpoint[0] + u * e0[0] + v * e1[0],
                d.midpoint[1] + u * e0[1] + v * e1[1],
            )
        });
        check_covered(&p, &fr, samples);
        assert_eq!(p, plot(&d, &fr));
    }
}

#[test]
fn both_branches_of_a_hyperbola_are_drawn() {
    let fr = standard();
    let d = conic(ConicType::Hyperbola, [0.0, 0.0], 0.3, [2.0, 1.0], 0.0);
    let p = plot(&d, &fr);
    check_on_curve(&d, &fr, &p);
    let (cx, _) = fr.world_to_screen(0.0, 0.0);
    // The axis is turned by 0.3 rad, so the branches stay on either side
    // of the centre near the middle of the view.
    assert!(p.paths.iter().flatten().any(|q| q.0 < cx - 50.0));
    assert!(p.paths.iter().flatten().any(|q| q.0 > cx + 50.0));
    let [e0, e1] = d.eigenvec;
    let samples = [1.0, -1.0].into_iter().flat_map(|s| {
        (0..4000).map(move |k| {
            let t = -5.0 + 10.0 * k as f64 / 4000.0;
            let (u, v) = (s * 2.0 * t.cosh(), t.sinh());
            (u * e0[0] + v * e1[0], u * e0[1] + v * e1[1])
        })
    });
    check_covered(&p, &fr, samples);
    // No segment joins the two branches.
    for path in &p.paths {
        let left = path.iter().filter(|q| q.0 < cx).count();
        assert!(left == 0 || left == path.len());
    }
}

#[test]
fn hyperbolas_and_parabolas_follow_the_curve() {
    let fr = standard();
    let mut g = Lcg(19);
    for i in 0..60 {
        let b = [g.range(-12.0, 12.0), g.range(-9.0, 9.0)];
        let angle = g.range(0.0, TAU);
        let d = if i % 2 == 0 {
            conic(
                ConicType::Hyperbola,
                b,
                angle,
                [g.range(0.1, 8.0), g.range(0.1, 8.0)],
                0.0,
            )
        } else {
            conic(
                ConicType::Parabola,
                b,
                angle,
                [0.0, 0.0],
                g.range(0.05, 10.0),
            )
        };
        let p = plot(&d, &fr);
        check_on_curve(&d, &fr, &p);
        let [e0, e1] = d.eigenvec;
        let at = |u: f64, v: f64| (b[0] + u * e0[0] + v * e1[0], b[1] + u * e0[1] + v * e1[1]);
        if d.ty == ConicType::Parabola {
            let samples = (0..8000).map(|k| {
                let t = -60.0 + 120.0 * k as f64 / 8000.0;
                at(t * t / (2.0 * d.p), t)
            });
            check_covered(&p, &fr, samples);
        } else {
            let [ha, hb] = d.half_axes;
            let samples = [1.0, -1.0].into_iter().flat_map(|s| {
                (0..8000).map(move |k| {
                    let t = -6.0 + 12.0 * k as f64 / 8000.0;
                    (s * ha * t.cosh(), hb * t.sinh())
                })
            });
            check_covered(&p, &fr, samples.map(|(u, v)| at(u, v)));
        }
    }
}

#[test]
fn degenerate_conics() {
    let fr = standard();
    let mut d = conic(
        ConicType::IntersectingLines,
        [0.0, 0.0],
        0.0,
        [0.0, 0.0],
        0.0,
    );
    d.lines = [[1.0, -1.0, 0.0], [1.0, 1.0, -2.0]];
    let p = plot(&d, &fr);
    assert_eq!(p.paths.len(), 2);
    assert!(p.label.is_some());

    d.ty = ConicType::ParallelLines;
    d.lines = [[0.0, 1.0, -1.0], [0.0, 1.0, 1.0]];
    let p = plot(&d, &fr);
    assert_eq!(p.paths.len(), 2);
    for path in &p.paths {
        assert!(path[0].0 <= 0.0 && path[path.len() - 1].0 >= W || path[0].0 >= W);
    }

    d.ty = ConicType::DoubleLine;
    let p = plot(&d, &fr);
    assert_eq!(p.paths.len(), 1);

    // A line off the view.
    d.lines = [[0.0, 1.0, -100.0], [0.0, 1.0, -100.0]];
    assert!(plot(&d, &fr).paths.is_empty());

    d.ty = ConicType::SinglePoint;
    d.midpoint = [1.0, 2.0];
    let p = plot(&d, &fr);
    assert!(p.paths.is_empty());
    assert_eq!(p.point, Some(fr.world_to_screen(1.0, 2.0)));
    d.midpoint = [100.0, 2.0];
    assert_eq!(plot(&d, &fr).point, None);

    for ty in [ConicType::Empty, ConicType::NotClassified] {
        d.ty = ty;
        assert_eq!(plot(&d, &fr), Plot::default());
    }
    d.ty = ConicType::Ellipse;
    d.half_axes = [1.0, 1.0];
    d.defined = false;
    assert_eq!(plot(&d, &fr), Plot::default());
}

#[test]
fn a_huge_ellipse_costs_only_the_visible_arc() {
    let fr = standard();
    // A radius of about 4e10 pixels whose top passes through the view.
    let r = 1e9;
    let d = conic(ConicType::Circle, [0.0, -r], 0.0, [r, r], 0.0);
    let p = plot(&d, &fr);
    assert!(!p.paths.is_empty());
    let count: usize = p.paths.iter().map(Vec::len).sum();
    assert!(count < 200, "{count} points");
    let (_, y) = fr.world_to_screen(0.0, 0.0);
    for q in p.paths.iter().flatten() {
        assert!((q.1 - y).abs() < 1.0, "{q:?}");
    }
    // Past `HUGE_RADIUS` pixels it is not drawn.
    let r = 2.0 * HUGE_RADIUS / fr.scale();
    let d = conic(ConicType::Circle, [0.0, -r], 0.0, [r, r], 0.0);
    assert!(plot(&d, &fr).paths.is_empty());
    let d = conic(
        ConicType::Parabola,
        [0.0, 0.0],
        0.0,
        [0.0, 0.0],
        2.0 * HUGE_RADIUS,
    );
    assert!(plot(&d, &fr).paths.is_empty());
}

#[test]
fn a_conic_off_the_view_draws_nothing() {
    let fr = standard();
    let d = conic(ConicType::Ellipse, [100.0, 0.0], 0.0, [3.0, 2.0], 0.0);
    let p = plot(&d, &fr);
    assert!(p.paths.is_empty());
    // A view inside an ellipse shows none of it.
    let d = conic(ConicType::Ellipse, [0.0, 0.0], 0.0, [300.0, 200.0], 0.0);
    assert!(plot(&d, &fr).paths.is_empty());
}

#[test]
fn extreme_zooms_do_not_panic() {
    for (half, cx) in [(1e-6, 0.3), (1e-6, 1e6), (1e9, 0.0), (1e9, 1e12)] {
        let fr = frame(cx - half, cx + half, -0.75 * half, 0.75 * half);
        for d in [
            conic(ConicType::Ellipse, [0.0, 0.0], 0.4, [3.0, 2.0], 0.0),
            conic(ConicType::Hyperbola, [0.0, 0.0], 0.4, [3.0, 2.0], 0.0),
            conic(ConicType::Parabola, [0.0, 0.0], 0.4, [0.0, 0.0], 0.5),
            conic(ConicType::Ellipse, [cx, 0.0], 0.4, [half, half / 3.0], 0.0),
        ] {
            let p = plot(&d, &fr);
            let count: usize = p.paths.iter().map(Vec::len).sum();
            assert!(count < 40_000, "{count} points");
            for &(x, y) in p.paths.iter().flatten() {
                assert!(x.is_finite() && y.is_finite());
                assert!((-PAD - 1e-6..=W + PAD + 1e-6).contains(&x));
            }
        }
    }
}
