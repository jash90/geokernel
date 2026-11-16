// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/CurvePlotter.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/CurveSegmentPlotter.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/CurveSegmentInfo.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/SegmentParams.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/CurvePlotterStack.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/plot/GeneralPathClippedForCurvePlotter.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/draw/DrawParametricCurve.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/roots/RealRootUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoFunction.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The graph of a function of x over the visible x range, plotted the way
//! GeoGebra's `CurvePlotter` does with the gap rule `Gap.MOVE_TO`: bisect
//! until consecutive points are at most 10 pixels apart and the path bends
//! by less than 10 degrees, never join two points across a jump (a pole of
//! `1/x` or `tan(x)`, a step of `floor(x)`), leave a gap where the function
//! is undefined (`sqrt(x)` for negative `x`), and leave out the segments
//! that lie off the view.

use super::{Cutter, Frame, Path};
use crate::svg::View;

/// `CurvePlotter.MIN_SAMPLE_POINTS`.
const MIN_SAMPLE_POINTS: f64 = 80.0;
/// `CurveSegmentInfo.MAX_PIXEL_DISTANCE`.
const MAX_PIXEL_DISTANCE: f64 = 10.0;
/// `SegmentParams.MAX_DEFINED_BISECTIONS`.
const MAX_DEFINED_BISECTIONS: usize = 16;
/// `CurveSegmentPlotter.LENGTH`.
const LENGTH: usize = MAX_DEFINED_BISECTIONS + 1;
/// `CurveSegmentPlotter.MAX_PROBLEM_BISECTIONS`.
const MAX_PROBLEM_BISECTIONS: u32 = 8;
/// `CurveSegmentPlotter.MAX_CONTINUITY_BISECTIONS`.
const MAX_CONTINUITY_BISECTIONS: u32 = 8;
/// `CurveSegmentPlotter.MAX_JUMP`, in pixels.
const MAX_JUMP: f64 = 5.0;
/// `SegmentParams.MAX_ZERO_COUNT`.
const MAX_ZERO_COUNT: u32 = 1000;
/// `GeoFunction.getMinDistX`, in pixels.
const MIN_DIST_X: f64 = 0.1;
/// `EuclidianStatic.CLIP_DISTANCE`, in pixels.
const CLIP_DISTANCE: f64 = 5.0;
/// `GeneralPathClippedForCurvePlotter.MIN_PIXEL_DISTANCE`.
const MIN_PIXEL_DISTANCE: f64 = 0.5;
/// `RealRootUtil.ITER_MAX`.
const ITER_MAX: u32 = 100;
/// `Kernel.STANDARD_PRECISION`.
const STANDARD_PRECISION: f64 = 1e-8;
/// `Kernel.MAX_DOUBLE_PRECISION`.
const MAX_DOUBLE_PRECISION: f64 = 1e-15;

/// The graph: its paths in screen pixels and where its label goes (the
/// first point on the view, `CurveSegmentPlotter.calculateLabelPosition`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    pub paths: Vec<Path>,
    pub label: Option<(f64, f64)>,
}

/// Plots `f` over the x range the frame shows (`DrawParametricCurve` for a
/// `GeoFunction` without an interval). `f` gives `NaN` or an infinity where
/// the function is not defined.
pub fn plot(f: &mut dyn FnMut(f64) -> f64, frame: &Frame) -> Plot {
    // `CurvePlotter`: at least this many samples across the range.
    let min_samples = MIN_SAMPLE_POINTS.max(((frame.width() as i64) / 6) as f64);
    plot_sampled(f, frame, min_samples)
}

/// [`plot`] starting from `min_samples` samples across the range.
fn plot_sampled(f: &mut dyn FnMut(f64) -> f64, frame: &Frame, min_samples: f64) -> Plot {
    let v = frame.visible(0.0);
    let (min, max) = (v.xmin, v.xmax);
    if !(min.is_finite() && max.is_finite() && frame.scale().is_finite() && frame.scale() > 0.0) {
        return Plot::default();
    }
    let mut p = Plotter {
        f,
        frame,
        xmin: v.xmin,
        xmax: v.xmax,
        ymin: v.ymin,
        ymax: v.ymax,
        scale: frame.scale(),
        max_bend: (10.0 * (std::f64::consts::PI / 180.0)).tan(),
        max_bend_off_screen: (45.0 * (std::f64::consts::PI / 180.0)).tan(),
        gp: Gp::default(),
    };
    if is_equal(min, max, STANDARD_PRECISION) {
        // `DrawParametricCurve`: a single point only places the label.
        let y = (p.f)(min);
        let (sx, sy) = frame.world_to_screen(min, y);
        return Plot {
            paths: Vec::new(),
            label: Some((java_int(sx), java_int(sy))),
        };
    }
    let max_param_step = (max - min).abs() / min_samples;
    let label = p.segment(min, max, 0, max_param_step, true);
    let mut cutter = Cutter::new(frame);
    for (point, line) in p.gp.points {
        if line {
            cutter.line_to(point);
        } else {
            cutter.move_to(point);
        }
    }
    Plot {
        paths: cutter.finish(),
        label,
    }
}

/// The graph as [`plot`] draws it in `frame`, together with what lies up to
/// `margin` pixels around the area, so that a drawing kept from this frame
/// can be moved by up to `margin` pixels: the paths of [`plot`], then the
/// graph over each of four strips around the area, each plotted on its own
/// as the area of a frame of the same scale, from as many samples a pixel
/// as the area (the strips left and right of
/// the area reach `margin` pixels above and below it; those above and
/// below are as wide as the area). Where the area and a strip meet, both
/// draw `PAD` pixels into the other. The label is that of [`plot`]. Paths
/// are in the screen pixels of `frame`.
pub fn plot_with_margin(f: &mut dyn FnMut(f64) -> f64, frame: &Frame, margin: f64) -> Plot {
    let min_samples = MIN_SAMPLE_POINTS.max(((frame.width() as i64) / 6) as f64);
    let mut out = plot_sampled(f, frame, min_samples);
    if !(margin > 0.0 && margin.is_finite()) {
        return out;
    }
    let (w, h, m) = (frame.width(), frame.height(), margin);
    for (x0, y0, x1, y1) in [
        (-m, -m, 0.0, h + m),
        (w, -m, w + m, h + m),
        (0.0, -m, w, 0.0),
        (0.0, h, w, h + m),
    ] {
        let (xmin, ymax) = frame.screen_to_world(x0, y0);
        let (xmax, ymin) = frame.screen_to_world(x1, y1);
        let strip = Frame::fit(
            View {
                xmin,
                ymin,
                xmax,
                ymax,
            },
            x1 - x0,
            y1 - y0,
        );
        // A strip starts from as many samples a pixel as the area: a strip
        // of the area's width from as many as it, a narrow one from fewer.
        let samples = (min_samples * (x1 - x0) / w).max(2.0);
        for path in plot_sampled(f, &strip, samples).paths {
            out.paths.push(
                path.into_iter()
                    .map(|(sx, sy)| {
                        let (x, y) = strip.screen_to_world(sx, sy);
                        frame.world_to_screen(x, y)
                    })
                    .collect(),
            );
        }
    }
    out
}

/// Java's `(int)` of a double: towards zero, saturating, `NaN` to 0.
fn java_int(v: f64) -> f64 {
    (v as i32) as f64
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal(x: f64, y: f64, eps: f64) -> bool {
    x == y || (x - eps < y && y < x + eps)
}

/// `DoubleUtil.isGreater(x, y, eps)`.
fn is_greater(x: f64, y: f64, eps: f64) -> bool {
    x > y + eps
}

/// `CurveSegmentPlotter.isUndefined`.
fn undefined(v: f64) -> bool {
    v.is_nan() || v.is_infinite()
}

fn undefined2(e: [f64; 2]) -> bool {
    undefined(e[0]) || undefined(e[1])
}

/// The path `GeneralPathClippedForCurvePlotter` collects: world points,
/// each starting a polyline (`false`) or continuing it (`true`).
#[derive(Default)]
struct Gp {
    points: Vec<((f64, f64), bool)>,
    line_drawn: bool,
}

struct Plotter<'a, 'f> {
    f: &'a mut (dyn FnMut(f64) -> f64 + 'f),
    frame: &'a Frame,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    scale: f64,
    max_bend: f64,
    max_bend_off_screen: f64,
    gp: Gp,
}

/// `CurveSegmentInfo` after an update.
#[derive(Clone, Copy, Default)]
struct Info {
    off_screen: bool,
    reached_min_step: bool,
    distance_ok: bool,
    angle_ok: bool,
}

impl Info {
    fn invalid(&self) -> bool {
        !self.angle_ok || !self.distance_ok
    }
}

/// One entry of `CurvePlotterStack`.
#[derive(Clone, Copy, Default)]
struct Item {
    dyadic: i64,
    depth: usize,
    on_screen: bool,
    eval: [f64; 2],
}

impl Plotter<'_, '_> {
    /// `GeoFunction.evaluateCurve`: `(t, f(t))`.
    fn eval(&mut self, t: f64) -> [f64; 2] {
        [t, (self.f)(t)]
    }

    /// `EuclidianView.isOnView`.
    fn on_view(&self, e: [f64; 2]) -> bool {
        e[0] >= self.xmin && e[0] <= self.xmax && e[1] >= self.ymin && e[1] <= self.ymax
    }

    /// `EuclidianView.getOnScreenDiff`.
    fn diff(&self, a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
        [(b[0] - a[0]) * self.scale, (b[1] - a[1]) * self.scale]
    }

    /// `EuclidianView.isSegmentOffView`.
    fn segment_off_view(&self, p1: [f64; 2], p2: [f64; 2]) -> bool {
        let tol = CLIP_DISTANCE / self.scale;
        (is_greater(self.ymin, p1[1], tol) && is_greater(self.ymin, p2[1], tol))
            || (is_greater(p1[1], self.ymax, tol) && is_greater(p2[1], self.ymax, tol))
            || (is_greater(self.xmin, p1[0], tol) && is_greater(self.xmin, p2[0], tol))
            || (is_greater(p1[0], self.xmax, tol) && is_greater(p2[0], self.xmax, tol))
    }

    /// `CurveSegmentInfo.update`.
    fn info(&self, left: [f64; 2], right: [f64; 2], diff: [f64; 2], prev: [f64; 2]) -> Info {
        let off_screen = self.segment_off_view(left, right);
        let distance_ok = off_screen
            || (diff[0].abs() <= MAX_PIXEL_DISTANCE && diff[1].abs() <= MAX_PIXEL_DISTANCE);
        let bend = if off_screen {
            self.max_bend_off_screen
        } else {
            self.max_bend
        };
        Info {
            off_screen,
            reached_min_step: diff[0].abs() < MIN_DIST_X,
            distance_ok,
            angle_ok: angle_ok(prev, diff, bend),
        }
    }

    /// `GeneralPathClippedForCurvePlotter.drawTo`: a point closer than half
    /// a pixel to the last one is left out unless it changes between
    /// drawing and moving.
    fn draw_to(&mut self, e: [f64; 2], line: bool) {
        let (x, y) = (e[0], e[1]);
        if y.is_nan() {
            // `GeneralPathClipped.addPoint`.
            return;
        }
        let Some(&((cx, cy), _)) = self.gp.points.last() else {
            self.gp.points.push(((x, y), false));
            self.gp.line_drawn = false;
            return;
        };
        let (sx, sy) = self.frame.world_to_screen(x, y);
        let (scx, scy) = self.frame.world_to_screen(cx, cy);
        let distant =
            !is_equal(sx, scx, MIN_PIXEL_DISTANCE) || !is_equal(sy, scy, MIN_PIXEL_DISTANCE);
        if !distant && line == self.gp.line_drawn {
            return;
        }
        self.gp.points.push(((x, y), line));
        self.gp.line_drawn = line;
    }

    /// `CurveSegmentPlotter.isContinuousAround`: whether `f` is defined at
    /// `t - eps` and `t + eps` with values close enough to bridge the
    /// undefined `f(t)`, in which case `eval_t` gets their mean.
    fn continuous_around(&mut self, t: f64, eps: f64, eval_t: &mut [f64; 2]) -> bool {
        let e = self.eval(t + eps);
        let oldy = e[1];
        if !undefined2(e) {
            let e = self.eval(t - eps);
            if !undefined2(e) {
                if (oldy - e[1]).abs() * self.scale < MAX_JUMP {
                    eval_t[1] = (oldy + e[1]) * 0.5;
                    return true;
                }
                // A function in x is never bridged otherwise.
                return false;
            }
        }
        false
    }

    /// `CurveSegmentPlotter.isContinuous`.
    fn continuous(&mut self, from: f64, to: f64, max_iterations: u32) -> bool {
        let (mut t_min, mut t_max) = (from, to);
        if is_equal(t_min, t_max, MAX_DOUBLE_PRECISION) {
            return true;
        }
        let left = self.eval(t_min);
        if undefined2(left) {
            return false;
        }
        let right = self.eval(t_max);
        if undefined2(right) {
            return false;
        }
        let dist_max = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
        let initial = dist_max(left, right);
        let eps = initial * 0.9;
        let mut dist = f64::INFINITY;
        let mut iterations = 0;
        while iterations < max_iterations && dist > eps {
            iterations += 1;
            let m = (t_min + t_max) / 2.0;
            let middle = self.eval(m);
            let dl = dist_max(left, middle);
            let dr = dist_max(right, middle);
            if dl > dr {
                dist = dl;
                t_max = m;
            } else {
                dist = dr;
                t_min = m;
            }
            if is_equal(t_min, t_max, MAX_DOUBLE_PRECISION) {
                return true;
            }
        }
        dist <= eps
    }

    /// `RealRootUtil.getDefinitionBorder`.
    fn definition_border(&mut self, a: f64, b: f64) -> f64 {
        let (mut left, mut right) = (a, b);
        let mut left_def = false;
        let mut iter = 0;
        while iter < ITER_MAX && left != right {
            let fl = (self.f)(left);
            let fr = (self.f)(right);
            left_def = !undefined(fl);
            let right_def = !undefined(fr);
            if left_def == right_def {
                return f64::NAN;
            }
            iter += 1;
            let m = (left + right) * 0.5;
            let m_def = !undefined((self.f)(m));
            if m_def == left_def {
                left = m;
            } else {
                right = m;
            }
        }
        if left_def {
            left
        } else {
            right
        }
    }

    /// `RealRootUtil.getDefinedInterval`.
    fn defined_interval_of(&mut self, a: f64, b: f64) -> (f64, f64) {
        let fa_nan = undefined((self.f)(a));
        let fb_nan = undefined((self.f)(b));
        if fa_nan && fb_nan {
            let m = (a + b) * 0.5;
            if (self.f)(m).is_nan() {
                (f64::NAN, f64::NAN)
            } else {
                (self.definition_border(a, m), self.definition_border(m, b))
            }
        } else if fa_nan {
            (self.definition_border(a, b), b)
        } else if fb_nan {
            (a, self.definition_border(a, b))
        } else {
            (a, b)
        }
    }

    /// `CurveSegmentPlotter.getDefinedInterval`.
    fn defined_interval(&mut self, a: f64, b: f64) -> (f64, f64) {
        let a_def = !undefined2(self.eval(a));
        let b_def = !undefined2(self.eval(b));
        if a_def != b_def {
            let (i0, i1) = self.defined_interval_of(a, b);
            (
                if undefined(i0) { a } else { i0 },
                if undefined(i1) { b } else { i1 },
            )
        } else {
            (a, b)
        }
    }

    /// A `CurveSegmentPlotter` for `[t_min, t_max]`: plots it and returns
    /// the label position it found.
    fn segment(
        &mut self,
        t_min: f64,
        t_max: f64,
        interval_depth: u32,
        max_param_step: f64,
        calc_label: bool,
    ) -> Option<(f64, f64)> {
        let mut seg = Segment {
            t_min,
            t_max,
            interval_depth,
            max_param_step,
            need_label: calc_label,
            label: None,
        };
        // `start`
        let eval_left = self.eval(t_min);
        if undefined2(eval_left) {
            return seg.problem_interval(self, t_min);
        }
        let eval_right = self.eval(t_max);
        if undefined2(eval_right) {
            return seg.problem_interval(self, t_min);
        }
        let on_screen = self.on_view(eval_right);
        // `firstPoint` with `Gap.MOVE_TO`.
        self.draw_to(eval_left, false);
        // `plot`
        match seg.bisect(self, eval_left, eval_right, on_screen) {
            Some(left) => seg.problem_interval(self, left),
            None => seg.label,
        }
    }
}

/// `CurveSegmentInfo.isAngleOK`.
fn angle_ok(v: [f64; 2], w: [f64; 2], bend: f64) -> bool {
    let inner = v[0] * w[0] + v[1] * w[1];
    if undefined(inner) {
        true
    } else if inner <= 0.0 {
        false
    } else {
        (v[0] * w[1] - v[1] * w[0]).abs() < bend * inner
    }
}

/// The state of one `CurveSegmentPlotter`.
struct Segment {
    t_min: f64,
    t_max: f64,
    interval_depth: u32,
    max_param_step: f64,
    need_label: bool,
    label: Option<(f64, f64)>,
}

impl Segment {
    /// `plotBisectorAlgo`. Returns where the problem interval starts when
    /// it ran into an undefined value that cannot be bridged.
    fn bisect(
        &mut self,
        p: &mut Plotter,
        mut eval_left: [f64; 2],
        mut eval_right: [f64; 2],
        mut on_screen: bool,
    ) -> Option<f64> {
        let (t_min, t_max) = (self.t_min, self.t_max);
        let mut stack = [Item::default(); LENGTH];
        stack[0] = Item {
            dyadic: 1,
            depth: 0,
            on_screen,
            eval: eval_right,
        };
        let mut top = 1usize;
        let mut divisors = [0.0; LENGTH];
        divisors[0] = t_max - t_min;
        for i in 1..LENGTH {
            divisors[i] = divisors[i - 1] / 2.0;
        }
        // `SegmentParams`
        let mut dyad: i64 = 1;
        let mut depth = 0usize;
        let mut t = t_min;
        let mut left = t_min;
        let mut zeros = 0u32;
        let mut diff = p.diff(eval_left, eval_right);
        let probe = p.eval(t_min + divisors[LENGTH - 1]);
        let mut prev_diff = p.diff(eval_left, probe);
        // The `CurveSegmentPlotter` fields `move` and
        // `nextLineToNeedsMoveToFirst`.
        let mut move_to = [0.0; 2];
        let mut move_first = false;
        loop {
            let mut info = p.info(eval_left, eval_right, diff, prev_diff);
            while depth < MAX_DEFINED_BISECTIONS
                && !info.reached_min_step
                && (info.invalid() || divisors[depth] > self.max_param_step)
                && zeros < MAX_ZERO_COUNT
            {
                stack[top] = Item {
                    dyadic: dyad,
                    depth,
                    on_screen,
                    eval: eval_right,
                };
                top += 1;
                // `SegmentParams.progress`
                dyad = 2 * dyad - 1;
                depth += 1;
                t = t_min + dyad as f64 * divisors[depth];
                let mut eval = p.eval(t);
                on_screen = p.on_view(eval);
                if undefined2(eval) && !p.continuous_around(t, divisors[LENGTH - 1], &mut eval) {
                    return Some(left);
                }
                eval_right = eval;
                diff = p.diff(eval_left, eval_right);
                let zero = diff[0].abs() < STANDARD_PRECISION && diff[1].abs() < STANDARD_PRECISION;
                zeros = if zero { zeros + 1 } else { 0 };
                info = p.info(eval_left, eval_right, diff, prev_diff);
            }
            // `drawSegment` with `isLineTo` for `Gap.MOVE_TO`.
            let line_to = if info.off_screen {
                false
            } else if info.invalid() {
                p.continuous(left, t, MAX_CONTINUITY_BISECTIONS)
            } else {
                true
            };
            if line_to {
                if move_first {
                    p.draw_to(move_to, false);
                    move_first = false;
                }
                p.draw_to(eval_right, true);
            } else {
                move_to = eval_right;
                move_first = true;
            }
            eval_left = eval_right;
            left = t;
            if on_screen && self.need_label {
                let (sx, sy) = p.frame.world_to_screen(eval_right[0], eval_right[1]);
                self.label = Some((java_int(sx), java_int(sy)));
                self.need_label = false;
            }
            // Pop the stack and go right.
            top -= 1;
            let item = stack[top];
            on_screen = item.on_screen;
            eval_right = item.eval;
            // `SegmentParams.restoreFromStack`
            prev_diff = diff;
            depth = item.depth + 1;
            dyad = item.dyadic * 2;
            t = t_min + dyad as f64 * divisors[depth.min(LENGTH - 1)];
            diff = p.diff(eval_left, eval_right);
            if top == 0 {
                return None;
            }
        }
    }

    /// `plotProblemInterval`: `f(left)` or `f(t_max)` is undefined, so the
    /// rest of the interval is cut in two halves, each plotted on its own,
    /// first shrunk to the part where the function is defined.
    fn problem_interval(&mut self, p: &mut Plotter, left: f64) -> Option<(f64, f64)> {
        let mut calc_label = self.need_label;
        if self.interval_depth > MAX_PROBLEM_BISECTIONS || left == self.t_max {
            return self.label;
        }
        let split = (left + self.t_max) / 2.0;
        let depth = self.interval_depth + 1;
        let step = self.max_param_step;
        let (min_range, max_range) = if (left - split).abs() > step {
            ((left, split), (split, self.t_max))
        } else {
            (
                p.defined_interval(left, split),
                p.defined_interval(split, self.t_max),
            )
        };
        calc_label = calc_label && self.label.is_none();
        let label_min = p.segment(min_range.0, min_range.1, depth, step, calc_label);
        calc_label = calc_label && label_min.is_none();
        let label_max = p.segment(max_range.0, max_range.1, depth, step, calc_label);
        if self.label.is_none() {
            self.label = label_min.or(label_max);
        }
        self.label
    }
}
