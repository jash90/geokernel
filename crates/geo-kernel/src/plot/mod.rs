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

/// The part of the segment from `a` to `b` inside `r` (Liang and Barsky).
/// `None` when no part is inside.
fn cut(a: (f64, f64), b: (f64, f64), r: &View) -> Option<Cut> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a.0 - r.xmin),
        (dx, r.xmax - a.0),
        (-dy, a.1 - r.ymin),
        (dy, r.ymax - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    if t0 > t1 || t0.is_nan() || t1.is_nan() {
        return None;
    }
    let at = |t: f64| {
        if t == 0.0 {
            a
        } else if t == 1.0 {
            b
        } else {
            (a.0 + t * dx, a.1 + t * dy)
        }
    };
    Some((at(t0), at(t1), t0 > 0.0, t1 < 1.0))
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
