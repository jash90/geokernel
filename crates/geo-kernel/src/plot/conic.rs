// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawConic.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! A conic cut to the view. Each curve is written in its eigenvector
//! system around its midpoint (the vertex of a parabola): an ellipse as
//! `(a cos t, b sin t)`, each branch of a hyperbola as `(±a cosh t,
//! b sinh t)`, a parabola as `(t² / 2p, t)`. The parameter values where the
//! curve crosses a side of the area (plus a margin) are solved for in
//! closed form; between two crossings the curve is either inside or
//! outside, so only the inside ranges are sampled. A curve far off the view
//! costs four small equations, a huge one only the few points on the view.
//!
//! The thresholds past which a conic is not drawn and where the label goes
//! are those of `DrawConic`; the curves themselves are polylines fine
//! enough that no chord is more than [`FLATNESS`] pixels off the curve,
//! where `DrawConic` hands Java2D an ellipse shape, a Bézier parabola and a
//! hyperbola sampled at a fixed number of points.

use std::f64::consts::TAU;

use super::{inside, Cutter, Frame, Path, PAD};
use crate::algo::conic_nd::{ConicData, ConicType};
use crate::svg::View;

/// `DrawConic.HUGE_RADIUS`: an ellipse whose half axis is longer than this
/// many pixels, or a parabola whose parameter is larger than this, is not
/// drawn.
pub const HUGE_RADIUS: f64 = 1e12;

/// How far, in pixels, a chord may run from the curve.
pub const FLATNESS: f64 = 0.25;

/// The most points one visible range is sampled with.
const MAX_POINTS: usize = 8192;

/// How far beyond the cut rectangle, in pixels, the visible ranges reach,
/// so that rounding in the crossings never loses a piece of the curve
/// inside: the final cut is done point by point.
const RANGE_MARGIN: f64 = 2.0 * PAD;

/// A conic as it is drawn: polylines in screen pixels, the single point of
/// a conic that is one point, the dot of an ellipse smaller than a pixel,
/// and where its label goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    pub paths: Vec<Path>,
    pub point: Option<(f64, f64)>,
    /// An ellipse too small to show a chord: `DrawConic` still strokes its
    /// shape, which is a dot of the line width.
    pub dot: Option<(f64, f64)>,
    pub label: Option<(f64, f64)>,
}

/// The conic `d` on the area `frame` describes.
pub fn plot(d: &ConicData, frame: &Frame) -> Plot {
    let scale = frame.scale();
    if !(d.defined && scale.is_finite() && scale > 0.0 && finite_data(d)) {
        return Plot::default();
    }
    let curve = Eigen::new(d);
    let range_rect = frame.visible(RANGE_MARGIN);
    let mut out = Plot::default();
    match d.ty {
        ConicType::Circle | ConicType::Ellipse => {
            let [a, b] = d.half_axes;
            if a * scale > HUGE_RADIUS || b * scale > HUGE_RADIUS || !(a > 0.0 && b > 0.0) {
                return out;
            }
            let c = |t: f64| curve.at(a * t.cos(), b * t.sin());
            let ranges = ellipse_ranges(&curve, a, b, &range_rect);
            let step = (8.0 * FLATNESS / (a.max(b) * scale)).sqrt();
            let mut cutter = Cutter::new(frame);
            for (t0, t1) in ranges {
                sample_uniform(&mut cutter, &c, t0, t1, step);
            }
            out.paths = cutter.finish();
            if out.paths.is_empty() && a.max(b) * scale < 1.0 {
                let [mx, my] = d.midpoint;
                if inside(&frame.visible(PAD), (mx, my)) {
                    out.dot = Some(frame.world_to_screen(mx, my));
                }
            }
            out.label = Some(if d.ty == ConicType::Circle {
                // `DrawConic.updateCircle`, in screen pixels.
                let (mx, my) = frame.world_to_screen(d.midpoint[0], d.midpoint[1]);
                let r = a * scale;
                (java_int(mx - r / 2.0), java_int(my - r * 0.85) + 20.0)
            } else {
                // `DrawConic.updateEllipse`.
                curve.label(frame, -a / 2.0, b * 0.85 - 20.0 / scale)
            });
        }
        ConicType::Hyperbola => {
            let [a, b] = d.half_axes;
            if !(a > 0.0 && b > 0.0) {
                return out;
            }
            let mut cutter = Cutter::new(frame);
            for s in [1.0, -1.0] {
                let c = |t: f64| curve.at(s * a * t.cosh(), b * t.sinh());
                for (t0, t1) in hyperbola_ranges(&curve, s * a, b, &range_rect) {
                    sample_hyperbola(&mut cutter, &c, t0, t1, a.hypot(b) * scale);
                }
            }
            out.paths = cutter.finish();
            // `DrawConic.updateHyperbolaLabelCoords`.
            out.label = Some(curve.label(frame, 2.0 * a, 1.7 * b - 20.0 / scale));
        }
        ConicType::Parabola => {
            let p = d.p;
            if p > HUGE_RADIUS || p.is_nan() || p <= 0.0 {
                return out;
            }
            let c = |t: f64| curve.at(t * t / (2.0 * p), t);
            let step = (8.0 * FLATNESS * p / scale).sqrt();
            let mut cutter = Cutter::new(frame);
            for (t0, t1) in parabola_ranges(&curve, p, &range_rect) {
                sample_uniform(&mut cutter, &c, t0, t1, step);
            }
            out.paths = cutter.finish();
            // `DrawConic.updateParabolaLabelCoords`.
            out.label = Some(curve.label(frame, 2.0 * p, 2.0 * p - 20.0 / scale));
        }
        ConicType::IntersectingLines | ConicType::ParallelLines | ConicType::DoubleLine => {
            // `DrawConic` hands these to `DrawLine`; a double line is one
            // line drawn twice, so it is drawn once.
            let count = if d.ty == ConicType::DoubleLine { 1 } else { 2 };
            for l in &d.lines[..count] {
                out.paths.extend(line_paths(*l, frame));
            }
            out.label = out.paths.first().map(|path| {
                let (a, b) = (path[0], path[path.len() - 1]);
                (java_int((a.0 + b.0) / 2.0), java_int((a.1 + b.1) / 2.0))
            });
        }
        ConicType::SinglePoint => {
            // `DrawConic` draws it with a `DrawPoint`.
            let [x, y] = d.midpoint;
            if inside(&frame.visible(PAD), (x, y)) {
                out.point = Some(frame.world_to_screen(x, y));
            }
        }
        ConicType::Empty | ConicType::NotClassified => {}
    }
    out
}

/// Java's `(int)` of a double: towards zero, saturating, `NaN` to 0.
/// Whether an open conic (hyperbola or parabola) has any part near the
/// frame. Such a curve cannot lie inside a rectangle, so it reaches the
/// rectangle exactly when it crosses one of its edges: on each edge the
/// equation is a quadratic in the edge parameter. Nothing is allocated, so
/// the test can run every frame. Other types answer `true`; their bounds are
/// cheaper to test from outside.
pub fn reaches(d: &ConicData, frame: &Frame) -> bool {
    if !matches!(d.ty, ConicType::Hyperbola | ConicType::Parabola) {
        return true;
    }
    let scale = frame.scale();
    if !(d.defined && scale.is_finite() && scale > 0.0) {
        return false;
    }
    let r = frame.visible(RANGE_MARGIN);
    let m = d.matrix;
    let q = |x: f64, y: f64| {
        m[0] * x * x + m[1] * y * y + m[2] + 2.0 * (m[3] * x * y + m[4] * x + m[5] * y)
    };
    let corners = [
        (r.xmin, r.ymin),
        (r.xmax, r.ymin),
        (r.xmax, r.ymax),
        (r.xmin, r.ymax),
    ];
    (0..4).any(|i| {
        let ((x0, y0), (x1, y1)) = (corners[i], corners[(i + 1) % 4]);
        // q along the edge: a t² + b t + c for t in [0, 1].
        let (c, end, mid) = (q(x0, y0), q(x1, y1), q(0.5 * (x0 + x1), 0.5 * (y0 + y1)));
        let a = 2.0 * (end + c) - 4.0 * mid;
        let b = end - c - a;
        let t = -b / (2.0 * a);
        let low = if a != 0.0 && t > 0.0 && t < 1.0 {
            (a * t + b) * t + c
        } else {
            c
        };
        // Anything not finite is kept: the cull must never hide a curve.
        !(c.is_finite() && end.is_finite() && low.is_finite())
            || c.signum() != end.signum()
            || c == 0.0
            || end == 0.0
            || low.signum() != c.signum()
            || low == 0.0
    })
}

fn java_int(v: f64) -> f64 {
    (v as i32) as f64
}

fn finite_data(d: &ConicData) -> bool {
    d.half_axes.iter().all(|v| v.is_finite())
        && d.midpoint.iter().all(|v| v.is_finite())
        && d.eigenvec.iter().flatten().all(|v| v.is_finite())
        && d.p.is_finite()
}

/// The eigenvector system of a conic: `(u, v)` stands for the point
/// `midpoint + u e0 + v e1`.
pub(crate) struct Eigen {
    pub(crate) b: [f64; 2],
    pub(crate) e0: [f64; 2],
    pub(crate) e1: [f64; 2],
}

impl Eigen {
    fn new(d: &ConicData) -> Eigen {
        Eigen {
            b: d.midpoint,
            e0: d.eigenvec[0],
            e1: d.eigenvec[1],
        }
    }

    #[inline]
    pub(crate) fn at(&self, u: f64, v: f64) -> (f64, f64) {
        (
            self.b[0] + u * self.e0[0] + v * self.e1[0],
            self.b[1] + u * self.e0[1] + v * self.e1[1],
        )
    }

    fn label(&self, frame: &Frame, u: f64, v: f64) -> (f64, f64) {
        let (x, y) = self.at(u, v);
        let (sx, sy) = frame.world_to_screen(x, y);
        (java_int(sx), java_int(sy))
    }

    /// The four sides of `r` as lines `alpha u + beta v = gamma`.
    fn sides(&self, r: &View) -> [(f64, f64, f64); 4] {
        let (bx, by) = (self.b[0], self.b[1]);
        [
            (self.e0[0], self.e1[0], r.xmin - bx),
            (self.e0[0], self.e1[0], r.xmax - bx),
            (self.e0[1], self.e1[1], r.ymin - by),
            (self.e0[1], self.e1[1], r.ymax - by),
        ]
    }
}

/// The real roots of `a x² + b x + c = 0`, computed without cancellation;
/// a linear equation when `a` is zero.
fn quadratic(a: f64, b: f64, c: f64) -> [Option<f64>; 2] {
    if a == 0.0 {
        return [(b != 0.0).then(|| -c / b), None];
    }
    let disc = b * b - 4.0 * a * c;
    if disc.is_nan() || disc < 0.0 {
        return [None, None];
    }
    let q = -0.5 * (b + b.signum() * disc.sqrt());
    if q == 0.0 {
        // b and c are zero
        return [Some(0.0), None];
    }
    [Some(q / a), Some(c / q)]
}

/// The parameter ranges of `t` in `[0, 2pi]` for which the ellipse
/// `(a cos t, b sin t)` lies inside `r`, in increasing order.
pub(crate) fn ellipse_ranges(curve: &Eigen, a: f64, b: f64, r: &View) -> Vec<(f64, f64)> {
    let mut cuts: Vec<f64> = Vec::with_capacity(8);
    for (alpha, beta, gamma) in curve.sides(r) {
        // alpha a cos t + beta b sin t = R cos(t - phi)
        let (p, q) = (alpha * a, beta * b);
        let big_r = p.hypot(q);
        if big_r.is_nan() || big_r <= 0.0 || gamma.abs() > big_r {
            continue;
        }
        let phi = q.atan2(p);
        let half = (big_r * big_r - gamma * gamma).max(0.0).sqrt().atan2(gamma);
        for t in [phi - half, phi + half] {
            cuts.push(t.rem_euclid(TAU));
        }
    }
    let at = |t: f64| curve.at(a * t.cos(), b * t.sin());
    if cuts.is_empty() {
        return if inside(r, at(0.0)) {
            vec![(0.0, TAU)]
        } else {
            Vec::new()
        };
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    // Arcs between consecutive crossings, the last one wrapping past 2pi.
    let n = cuts.len();
    let mut arcs: Vec<(f64, f64)> = Vec::with_capacity(n);
    for i in 0..n {
        let t0 = cuts[i];
        let t1 = if i + 1 < n {
            cuts[i + 1]
        } else {
            cuts[0] + TAU
        };
        if t1 > t0 && inside(r, at(0.5 * (t0 + t1))) {
            arcs.push((t0, t1));
        }
    }
    // Join arcs that meet; the wrapping arc may join the first.
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(arcs.len());
    for arc in arcs {
        match merged.last_mut() {
            Some(last) if last.1 == arc.0 => last.1 = arc.1,
            _ => merged.push(arc),
        }
    }
    if merged.len() >= 2 {
        let last = merged[merged.len() - 1];
        if last.1 == merged[0].0 + TAU {
            merged[0].0 = last.0 - TAU;
            merged.pop();
        }
    }
    merged
}

/// The ranges between consecutive crossings `cuts` whose middle `at` puts
/// inside `r`. Before the first and after the last crossing the curve runs
/// off to infinity, so those parts are never inside.
fn open_ranges(mut cuts: Vec<f64>, r: &View, at: impl Fn(f64) -> (f64, f64)) -> Vec<(f64, f64)> {
    cuts.retain(|t| t.is_finite());
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut out: Vec<(f64, f64)> = Vec::new();
    for w in cuts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if inside(r, at(0.5 * (t0 + t1))) {
            match out.last_mut() {
                Some(last) if last.1 == t0 => last.1 = t1,
                _ => out.push((t0, t1)),
            }
        }
    }
    out
}

/// The parameter ranges for which the branch `(a cosh t, b sinh t)` (`a`
/// negative for the left branch) lies inside `r`.
pub(crate) fn hyperbola_ranges(curve: &Eigen, a: f64, b: f64, r: &View) -> Vec<(f64, f64)> {
    let mut cuts = Vec::with_capacity(8);
    for (alpha, beta, gamma) in curve.sides(r) {
        // With w = e^t: (alpha a + beta b) w² - 2 gamma w + (alpha a - beta b) = 0.
        let (p, q) = (alpha * a, beta * b);
        for w in quadratic(p + q, -2.0 * gamma, p - q).into_iter().flatten() {
            if w > 0.0 {
                cuts.push(w.ln());
            }
        }
    }
    open_ranges(cuts, r, |t| curve.at(a * t.cosh(), b * t.sinh()))
}

/// The parameter ranges for which the parabola `(t² / 2p, t)` lies inside
/// `r`.
pub(crate) fn parabola_ranges(curve: &Eigen, p: f64, r: &View) -> Vec<(f64, f64)> {
    let mut cuts = Vec::with_capacity(8);
    for (alpha, beta, gamma) in curve.sides(r) {
        // alpha t² / 2p + beta t = gamma
        cuts.extend(
            quadratic(alpha / (2.0 * p), beta, -gamma)
                .into_iter()
                .flatten(),
        );
    }
    open_ranges(cuts, r, |t| curve.at(t * t / (2.0 * p), t))
}

/// Samples `c` on `[t0, t1]` at steps of at most `step`.
fn sample_uniform(
    cutter: &mut Cutter,
    c: &impl Fn(f64) -> (f64, f64),
    t0: f64,
    t1: f64,
    step: f64,
) {
    let len = t1 - t0;
    if len.is_nan() || len < 0.0 {
        return;
    }
    let n = if step > 0.0 {
        (len / step).ceil().clamp(1.0, MAX_POINTS as f64) as usize
    } else {
        MAX_POINTS
    };
    cutter.move_to(c(t0));
    for i in 1..=n {
        let t = if i == n {
            t1
        } else {
            t0 + len * (i as f64 / n as f64)
        };
        cutter.line_to(c(t));
    }
}

/// Samples a hyperbola branch on `[t0, t1]`. The second derivative of
/// `(a cosh t, b sinh t)` is at most `hypot(a, b) cosh t` long (`size` in
/// pixels), and a chord runs at most an eighth of that times the squared
/// step from its arc, so the step shrinks as `1 / sqrt(cosh t)` away from
/// the vertex.
fn sample_hyperbola(
    cutter: &mut Cutter,
    c: &impl Fn(f64) -> (f64, f64),
    t0: f64,
    t1: f64,
    size: f64,
) {
    if t1.is_nan() || t0.is_nan() || t1 < t0 {
        return;
    }
    let step_at = |t: f64| (8.0 * FLATNESS / (size * t.abs().cosh())).sqrt();
    cutter.move_to(c(t0));
    let mut t = t0;
    let mut count = 0;
    while t < t1 && count < MAX_POINTS {
        let first = step_at(t);
        // The bend is largest at the end of the step farther from 0.
        let far = if t >= 0.0 { t + first } else { t };
        let step = step_at(far).min(first).max((t1 - t0) / MAX_POINTS as f64);
        t = (t + step).min(t1);
        cutter.line_to(c(t));
        count += 1;
    }
    if t < t1 {
        cutter.line_to(c(t1));
    }
}

/// The line `l = [x, y, z]` (`x X + y Y + z = 0`) cut to the area.
fn line_paths(l: [f64; 3], frame: &Frame) -> Vec<Path> {
    let n = l[0].hypot(l[1]);
    if !(n > 0.0 && n.is_finite() && l[2].is_finite()) {
        return Vec::new();
    }
    let (a, b, k) = (l[0] / n, l[1] / n, l[2] / n);
    let r = frame.visible(RANGE_MARGIN);
    let (cx, cy) = (0.5 * (r.xmin + r.xmax), 0.5 * (r.ymin + r.ymax));
    let reach = (r.xmax - r.xmin).hypot(r.ymax - r.ymin);
    let dist = a * cx + b * cy + k;
    if dist.is_nan() || dist.abs() > reach {
        return Vec::new();
    }
    let (fx, fy) = (cx - a * dist, cy - b * dist);
    let mut cutter = Cutter::new(frame);
    cutter.move_to((fx - reach * b, fy + reach * a));
    cutter.line_to((fx + reach * b, fy - reach * a));
    cutter.finish()
}
