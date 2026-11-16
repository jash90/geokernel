// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/GeneralPathClipped.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Curves cut to a view, as polylines in screen coordinates: general conics
//! ([`conic`]) and functions of x ([`function`]). No UI types: the SVG
//! writer and the window draw the same polylines.
//!
//! Every point of a returned path lies inside the area plus [`PAD`] pixels
//! on each side, so a path can be handed to a painter that works in `f32`
//! at any zoom. The paths are cut against that rectangle segment by
//! segment in world coordinates, where the numbers stay finite.

pub mod conic;
pub mod function;

#[cfg(test)]
mod conic_tests;
#[cfg(test)]
mod function_tests;

use crate::svg::View;

/// How far outside the area, in pixels, a path may run before it is cut:
/// `GeneralPathClipped`'s padding for GeoGebra's default line thickness 5
/// (`lineThickness + 5`), more than half of any stroke drawn.
pub const PAD: f64 = 10.0;

/// World coordinates beyond this are pulled in before cutting, so that the
/// differences of two coordinates stay finite. The visible part of a path
/// does not move by more than a few units in the last place.
const MAX_WORLD: f64 = 1e300;

/// A path in screen pixels, `y` growing downwards.
pub type Path = Vec<(f64, f64)>;

/// How the plane maps to a `width` x `height` pixel area: `view` fitted
/// into it, centred, with one scale for both axes. The window's view
/// transform and the SVG writer use the same arithmetic, so a curve lands
/// on the same pixels as the points it passes through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    view: View,
    scale: f64,
    ox: f64,
    oy: f64,
    width: f64,
    height: f64,
}

impl Frame {
    pub fn fit(view: View, width: f64, height: f64) -> Frame {
        let vw = (view.xmax - view.xmin).max(f64::MIN_POSITIVE);
        let vh = (view.ymax - view.ymin).max(f64::MIN_POSITIVE);
        let scale = (width / vw).min(height / vh);
        Frame {
            view,
            scale,
            ox: 0.5 * (width - vw * scale),
            oy: 0.5 * (height - vh * scale),
            width,
            height,
        }
    }

    /// Pixels per world unit.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    #[inline]
    pub fn world_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.ox + (x - self.view.xmin) * self.scale,
            self.height - self.oy - (y - self.view.ymin) * self.scale,
        )
    }

    #[inline]
    pub fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.view.xmin + (sx - self.ox) / self.scale,
            self.view.ymin + (self.height - self.oy - sy) / self.scale,
        )
    }

    /// The same scale on an area grown by `margin` pixels on each side: a
    /// point lands `margin` pixels right of and below where `self` puts it
    /// (up to rounding).
    pub fn grown(&self, margin: f64) -> Frame {
        Frame::fit(
            self.visible(margin),
            self.width + 2.0 * margin,
            self.height + 2.0 * margin,
        )
    }

    /// How far, in pixels, every point moved from `self` to `to` when both
    /// have the same area and the same scale (to a relative 1e-9: a pan
    /// recomputes the scale from the moved view, which rounds; the error is
    /// then under 1e-5 pixels across 10 000 pixels): a pan. `None`
    /// otherwise.
    pub fn shift_to(&self, to: &Frame) -> Option<(f64, f64)> {
        let same_scale = (to.scale - self.scale).abs() <= 1e-9 * self.scale;
        if !(same_scale && to.width == self.width && to.height == self.height) {
            return None;
        }
        let d = (
            (to.ox - self.ox) + (self.view.xmin - to.view.xmin) * self.scale,
            (self.oy - to.oy) + (to.view.ymin - self.view.ymin) * self.scale,
        );
        (d.0.is_finite() && d.1.is_finite()).then_some(d)
    }

    /// The rectangle of the plane the whole area shows, grown by `margin`
    /// pixels on each side.
    pub fn visible(&self, margin: f64) -> View {
        let (xmin, ymax) = self.screen_to_world(-margin, -margin);
        let (xmax, ymin) = self.screen_to_world(self.width + margin, self.height + margin);
        View {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }
}

/// Whether `(x, y)` lies in `r`, borders included.
pub(crate) fn inside(r: &View, (x, y): (f64, f64)) -> bool {
    x >= r.xmin && x <= r.xmax && y >= r.ymin && y <= r.ymax
}

/// The start and end of a cut segment, and whether each was moved onto the
/// border.
type Cut = ((f64, f64), (f64, f64), bool, bool);

/// The part of the segment from `a` to `b` inside `r` (Cohen and
/// Sutherland). `None` when no part is inside.
///
/// Each moved end is put on the border it crosses directly, from the slope
/// of the segment, not through one parameter along the whole segment: for
/// a nearly vertical segment millions of times longer than the area, such a
/// parameter rounds off the visible part. Both ends land in `r`.
fn cut(a: (f64, f64), b: (f64, f64), r: &View) -> Option<Cut> {
    let code = |p: (f64, f64)| -> u8 {
        u8::from(p.0 < r.xmin)
            | u8::from(p.0 > r.xmax) << 1
            | u8::from(p.1 < r.ymin) << 2
            | u8::from(p.1 > r.ymax) << 3
    };
    let (mut s, mut e) = (a, b);
    let (mut cs, mut ce) = (code(s), code(e));
    let (mut s_moved, mut e_moved) = (false, false);
    // Each round puts one end on a border; four rounds put both inside.
    for _ in 0..5 {
        if cs | ce == 0 {
            return Some((s, e, s_moved, e_moved));
        }
        if cs & ce != 0 {
            return None;
        }
        let (p, q, c) = if cs != 0 { (s, e, cs) } else { (e, s, ce) };
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let x_at = |y: f64| p.0 + (y - p.1) * (dx / dy);
        let y_at = |x: f64| p.1 + (x - p.0) * (dy / dx);
        let m = if c & 1 != 0 {
            (r.xmin, y_at(r.xmin))
        } else if c & 2 != 0 {
            (r.xmax, y_at(r.xmax))
        } else if c & 4 != 0 {
            (x_at(r.ymin), r.ymin)
        } else {
            (x_at(r.ymax), r.ymax)
        };
        if m.0.is_nan() || m.1.is_nan() {
            return None;
        }
        if cs != 0 {
            (s, cs, s_moved) = (m, code(m), true);
        } else {
            (e, ce, e_moved) = (m, code(m), true);
        }
    }
    None
}

/// Builds screen paths from polylines given in world coordinates, cutting
/// them to the area plus [`PAD`].
pub(crate) struct Cutter<'a> {
    frame: &'a Frame,
    rect: View,
    paths: Vec<Path>,
    run: Path,
    last: Option<(f64, f64)>,
}

impl<'a> Cutter<'a> {
    pub(crate) fn new(frame: &'a Frame) -> Cutter<'a> {
        Cutter {
            frame,
            rect: frame.visible(PAD),
            paths: Vec::new(),
            run: Vec::new(),
            last: None,
        }
    }

    fn pull_in((x, y): (f64, f64)) -> (f64, f64) {
        (
            x.clamp(-MAX_WORLD, MAX_WORLD),
            y.clamp(-MAX_WORLD, MAX_WORLD),
        )
    }

    /// Starts a new polyline at `p`.
    pub(crate) fn move_to(&mut self, p: (f64, f64)) {
        self.end_run();
        self.last = (p.0.is_finite() && p.1.is_finite()).then(|| Self::pull_in(p));
    }

    /// Continues the polyline to `p`; a point that is not finite breaks it.
    pub(crate) fn line_to(&mut self, p: (f64, f64)) {
        if !(p.0.is_finite() && p.1.is_finite()) {
            self.move_to(p);
            return;
        }
        let p = Self::pull_in(p);
        let Some(a) = self.last.replace(p) else {
            return;
        };
        match cut(a, p, &self.rect) {
            Some((s, e, s_moved, e_moved)) => {
                if s_moved || self.run.is_empty() {
                    self.end_run();
                    self.push(s);
                }
                self.push(e);
                if e_moved {
                    self.end_run();
                }
            }
            None => self.end_run(),
        }
    }

    /// Adds a screen point to the current run unless it repeats the last.
    fn push(&mut self, (x, y): (f64, f64)) {
        let s = self.frame.world_to_screen(x, y);
        if let Some(&(px, py)) = self.run.last() {
            if (s.0 - px).abs() < 1e-3 && (s.1 - py).abs() < 1e-3 {
                return;
            }
        }
        self.run.push(s);
    }

    fn end_run(&mut self) {
        if self.run.len() >= 2 {
            self.paths.push(std::mem::take(&mut self.run));
        } else {
            self.run.clear();
        }
    }

    pub(crate) fn finish(mut self) -> Vec<Path> {
        self.end_run();
        self.paths
    }
}
