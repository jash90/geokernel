// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java
//   source/shared/common/src/main/java/org/geogebra/common/main/GlobalKeyDispatcher.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The view rules the camera alone decides: the wheel and keyboard zoom
//! factors, the pinch, the axis rescale and the zoom rectangle, each
//! following the reference app except where the owner keeps another
//! behaviour (one named switch each, `OwnerRules`).

use crate::view::{Camera, ScaleLimits};

mod touch;
pub use touch::TouchTarget;

/// The reference app's wheel zoom factor (`EuclidianView
/// .MOUSE_WHEEL_ZOOM_FACTOR`).
pub const WHEEL_ZOOM_FACTOR: f64 = 1.1;
/// With Alt held the wheel zooms this many times as far
/// (`wrapMouseWheelMoved`: the factor multiplied by 1.5).
pub const WHEEL_ALT_FACTOR: f64 = 1.5;
/// The owner's wheel rule (spec D2) makes the zoom proportional to the
/// scroll amount, with this many scroll points per notch, as a browser's
/// wheel reports one.
pub const WHEEL_POINTS_PER_NOTCH: f64 = 100.0;
/// The keyboard zoom (`GlobalKeyDispatcher`: Ctrl + plus and Ctrl + minus)
/// steps by the wheel factor about the last pointer position.
pub const KEY_ZOOM_FACTOR: f64 = WHEEL_ZOOM_FACTOR;

/// The scroll of one frame as a number of wheel notches: line units count
/// one notch a line, point (pixel) units `WHEEL_POINTS_PER_NOTCH` points
/// one. Horizontal scrolling zooms nothing (`MouseTouchGestureControllerW
/// .onMouseWheel`: only `deltaY != 0`).
pub fn wheel_notches(unit_lines: bool, delta_y: f64, delta_x: f64) -> f64 {
    if !(delta_y.is_finite() && delta_y != 0.0 && delta_x.is_finite()) {
        return 0.0;
    }
    if unit_lines {
        delta_y
    } else {
        delta_y / WHEEL_POINTS_PER_NOTCH
    }
}

/// The zoom factor of one frame's wheel events (`wrapMouseWheelMoved`).
/// The reference app zooms one fixed `WHEEL_ZOOM_FACTOR` per event,
/// whatever its delta — only its sign decides in or out
/// (`fixed_per_event`, `events` the signed count of the frame's events,
/// zoom in positive); `alt` multiplies the base by `WHEEL_ALT_FACTOR`.
/// The owner keeps the zoom proportional to the scroll amount (spec D2),
/// `WHEEL_ZOOM_FACTOR` to the `notches` of the frame.
pub fn wheel_zoom_factor(fixed_per_event: bool, notches: f64, events: f64, alt: bool) -> f64 {
    let base = if alt {
        WHEEL_ZOOM_FACTOR * WHEEL_ALT_FACTOR
    } else {
        WHEEL_ZOOM_FACTOR
    };
    base.powf(if fixed_per_event { events } else { notches })
}

/// The dead bands of a pinch: the zoom waits for `zoom` pixels of changed
/// finger distance and the pan for `pan` pixels of a moved midpoint. Which
/// pair applies is an owner's rule (`OwnerRules::pinch_bands`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinchBands {
    /// Pixels of changed finger distance before a zoom step applies.
    pub zoom: f64,
    /// Pixels of a moved midpoint before a pan step applies.
    pub pan: f64,
}

impl PinchBands {
    /// The owner's rule (spec H4): no band, the view follows the fingers
    /// exactly.
    pub const OURS: PinchBands = PinchBands {
        zoom: 0.0,
        pan: 0.0,
    };
    /// The reference app's bands (`MINIMAL_PIXEL_DIFFERENCE_FOR_ZOOM` 10
    /// and `MIN_MOVE` 5 in `twoTouchMoveCommon`).
    pub const REFERENCE: PinchBands = PinchBands {
        zoom: 10.0,
        pan: 5.0,
    };
}

/// A pinch of two fingers in progress: what the reference app keeps of
/// one (`MouseTouchGestureController`: `oldDistance`, `oldCenterX/Y`),
/// for the dead bands of its steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinch {
    /// The bands this pinch steps in (the owner's rule when it began).
    bands: PinchBands,
    /// The finger distance the last zoom step applied (`oldDistance`).
    distance: f64,
    /// The midpoint the last pan step applied (`oldCenter`).
    center: (f64, f64),
}

/// A whole-pixel place, as the reference app counts its pointer places
/// (`GPoint`; the web rounds the event offset: `(int) Math.round(...)`).
fn pixel(v: f64) -> f64 {
    v.round()
}

/// The distance and midpoint of fingers at whole-pixel places
/// (`twoTouchMove`: the int truncation of the pointers' own).
fn measured(a: (f64, f64), b: (f64, f64)) -> (f64, (f64, f64)) {
    let (x1, y1) = (pixel(a.0), pixel(a.1));
    let (x2, y2) = (pixel(b.0), pixel(b.1));
    (
        (x1 - x2).hypot(y1 - y2),
        // `(int) (x1 + x2) / 2`: whole pixels.
        (((x1 + x2) as i64 / 2) as f64, ((y1 + y2) as i64 / 2) as f64),
    )
}

impl Pinch {
    /// The pinch begins with the fingers at `a` and `b`
    /// (`twoTouchStart`): the first step's bases. It steps in `bands`,
    /// the rule of the window it began in.
    pub fn start(a: (f64, f64), b: (f64, f64), bands: PinchBands) -> Pinch {
        let (distance, center) = measured(a, b);
        Pinch {
            bands,
            distance,
            center,
        }
    }

    /// One finger moved; `a` and `b` are where the fingers are now. Returns
    /// the zoom factor, the whole-pixel midpoint it applies about and the
    /// pan in pixels, the factor `1` and the pan zero when that step does
    /// not apply, for the caller to put on the camera.
    ///
    /// The reference app (`twoTouchMoveCommon`): a zoom step applies only
    /// once the finger distance changed by more than the band's `zoom`
    /// pixels, a pan only once the midpoint moved more than its `pan`, and
    /// each step keeps its own base — the distance or the midpoint it
    /// applied — as the next step's, so the fingers run ahead of the view
    /// (`onPinch` zooms about the new midpoint,
    /// `translateCoordSystemInPixels` pans by the raw pixel delta).
    ///
    /// The owner's rule (spec H4, bands of 0) tracks the fingers exactly:
    /// every move zooms by its ratio about the new midpoint and the pan
    /// carries the world point of the old midpoint under the new one
    /// (the zoom already moved it, so the pan goes by the zoomed delta).
    pub fn step(&mut self, a: (f64, f64), b: (f64, f64)) -> (f64, (f64, f64), (f64, f64)) {
        let (distance, center) = measured(a, b);
        let mut factor = 1.0;
        if distance > 0.0 && (distance - self.distance).abs() > self.bands.zoom {
            factor = distance / self.distance;
            self.distance = distance;
        }
        let (dx, dy) = (center.0 - self.center.0, center.1 - self.center.1);
        let mut pan = (0.0, 0.0);
        if dx.hypot(dy) > self.bands.pan {
            pan = if self.bands == PinchBands::OURS {
                // Exactly under the fingers: the world point of the old
                // midpoint ends where the new one is.
                (factor * dx, factor * dy)
            } else {
                (dx, dy)
            };
            self.center = center;
        }
        (factor, center, pan)
    }
}

/// One move of a pinch applied to `cam` (see [`Pinch::step`]): the zoom
/// about the new midpoint, then the pan. The area is `width` x `height`,
/// the scale bounds `limits`.
pub fn pinch_move(
    cam: &mut Camera,
    pinch: &mut Pinch,
    a: (f64, f64),
    b: (f64, f64),
    width: f64,
    height: f64,
    limits: ScaleLimits,
) {
    let (factor, center, pan) = pinch.step(a, b);
    cam.zoom_at(factor, center.0, center.1, width, height, limits);
    cam.pan_pixels(pan.0, pan.1);
}

/// One axis of the view as the screen lays it out: the numbers
/// `setScaleAxis` reads.
#[derive(Clone, Copy, Debug)]
pub struct ScreenAxis {
    /// The screen position of the origin on this axis (`xZero` / `yZero`).
    pub zero: f64,
    /// The extent of the area along this axis.
    pub size: f64,
    /// The world bound of the view at the small end of this axis.
    pub min: f64,
    /// The world bound at its large end.
    pub max: f64,
}

/// `EuclidianController.setScaleAxis` (from `scaleXAxis` and, with the
/// values swapped for the screen's downward y, `scaleYAxis`): the world
/// value `grabbed` — where the press landed on the axis — follows the
/// pointer to `mouse`, counted from the pixel of the axis origin, and the
/// scale follows it; the origin stands still while it is on the screen.
pub fn axis_scale(
    cam: &Camera,
    x_axis: bool,
    axis: ScreenAxis,
    grabbed: f64,
    mouse: f64,
    limits: ScaleLimits,
) -> Camera {
    // The y axis swaps min and max and negates the scale, as the screen's
    // y points down (`scaleYAxis`).
    let (min, max, sign) = if x_axis {
        (axis.min, axis.max, 1.0)
    } else {
        (axis.max, axis.min, -1.0)
    };
    // Check if zero is on the screen.
    let (mut zero_px, mut zero_rw) = (axis.zero, 0.0);
    if zero_px < 0.0 {
        zero_px = 0.0;
        zero_rw = min;
    } else if zero_px > axis.size {
        zero_px = axis.size;
        zero_rw = max;
    }
    // Take care when we get close to the origin
    // (`MIN_MOUSE_MOVE_FOR_AXIS_SCALE`, 2 px).
    const MIN_MOUSE_MOVE: f64 = 2.0;
    let mut m = mouse;
    if (mouse - zero_px).abs() < MIN_MOUSE_MOVE {
        m = zero_px + MIN_MOUSE_MOVE * f64::from(mouse > zero_px).copysign(mouse - zero_px);
        if mouse == zero_px {
            m = zero_px - MIN_MOUSE_MOVE;
        }
    }
    let scale = sign * (m - zero_px) / (grabbed - zero_rw);
    if !scale.is_finite() || scale <= 0.0 {
        return *cam;
    }
    let scale = limits.clamp(scale);
    // Move zero if off screen.
    let mut new_zero = axis.zero;
    if new_zero < 0.0 {
        new_zero = -zero_rw * scale;
    } else if new_zero > axis.size {
        new_zero = axis.size - zero_rw * scale;
    }
    let mut out = *cam;
    if x_axis {
        out.xscale = scale;
        out.cx = (0.5 * axis.size - new_zero) / scale;
    } else {
        out.yscale = scale;
        out.cy = (new_zero - 0.5 * axis.size) / scale;
    }
    out
}

/// An axis a drag rescales, with the world value the press grabbed on it
/// (`xTemp` / `yTemp` of `mousePressedTranslatedView`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisDrag {
    /// The x axis (`true`) or the y axis.
    pub x: bool,
    /// The world value on that axis where the press landed.
    pub grabbed: f64,
}

/// `EuclidianController.ZOOM_RECTANGLE_SNAP_RATIO` (1.2).
pub const ZOOM_RECTANGLE_SNAP_RATIO: f64 = 1.2;
/// `EuclidianController.ZOOM_RECT_THRESHOLD` (30 px): a zoom rectangle
/// narrower than this on either side zooms nothing.
pub const ZOOM_RECT_THRESHOLD: f64 = 30.0;

/// The zoom rectangle of the zoom in tool (`updateSelectionRectangle`
/// with the ratio kept for the zoom tools): a rectangle whose ratio is
/// within `ZOOM_RECTANGLE_SNAP_RATIO` of the view's own is widened to that
/// ratio. `from` is the press and `to` the pointer, in screen pixels of a
/// `width` x `height` area; the rectangle comes back as (min x, min y,
/// max x, max y).
pub fn zoom_rectangle(
    from: (f64, f64),
    to: (f64, f64),
    width: f64,
    height: f64,
) -> (f64, f64, f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let (mut w, mut h) = (dx, dy);
    // The zoom rectangle should have the same aspect ratio as the view.
    let ratio = width / height;
    let new_ratio = if dy == 0.0 { ratio } else { (dx / dy).abs() };
    if new_ratio < (ratio * ZOOM_RECTANGLE_SNAP_RATIO).abs()
        && ratio.abs() < new_ratio * ZOOM_RECTANGLE_SNAP_RATIO
    {
        if dx.abs() >= dy.abs() * ratio {
            h = (dx.abs() / ratio).copysign(dy);
        } else {
            w = (dy.abs() * ratio).copysign(dx);
        }
    }
    let (x0, x1) = if w >= 0.0 {
        (from.0, from.0 + w)
    } else {
        (from.0 + w, from.0)
    };
    let (y0, y1) = if h >= 0.0 {
        (from.1, from.1 + h)
    } else {
        (from.1 + h, from.1)
    };
    (x0, y0, x1, y1)
}

/// One view change followed for the undo rule
/// (`OwnerRules::view_changes_are_steps`): the camera before the gesture —
/// a drag, a pinch, or a run of wheel or keyboard zoom frames. The window
/// asks for the step when the gesture ends; one that changed nothing is
/// none. Only one is open at a time: a gesture that a second one replaces
/// is handed out first, as its own step.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewGesture {
    before: Option<Camera>,
    /// The open gesture is a run of wheel frames, ended by the first
    /// frame without a zoom — not a drag or a pinch, which end when they
    /// end. Under the reference's lazy rule the run's step waits for the
    /// next canvas press (spec F3), so the run then stays open.
    run: bool,
}

impl ViewGesture {
    /// A gesture begins with the camera `cam`; the one before it, if any,
    /// ends here: its camera is returned for its own undo step.
    pub fn begin(&mut self, cam: Camera) -> Option<Camera> {
        self.run = false;
        self.before.replace(cam)
    }

    /// The gesture ends with the camera `cam` now: the camera to record a
    /// step from, if the view changed.
    pub fn end(&mut self, cam: Camera) -> Option<Camera> {
        let before = self.before.take()?;
        (before != cam).then_some(before)
    }

    /// The gesture is abandoned: the camera to put back, if the view
    /// changed. Nothing is recorded.
    pub fn cancel(&mut self, cam: Camera) -> Option<Camera> {
        let before = self.before.take()?;
        (before != cam).then_some(before)
    }

    /// Whether a gesture is open.
    pub fn is_open(&self) -> bool {
        self.before.is_some()
    }

    /// A frame of a wheel or keyboard zoom: a run is the consecutive
    /// frames that zoomed. `store_at_the_end` is the owner's rule (the
    /// step is recorded when the run ends — the first frame after it that
    /// zoomed none, right after the gesture); without it the reference's
    /// lazy rule holds (spec F3): the run only ends at the next canvas
    /// press, and the step — one for every view change since — is stored
    /// there (`storeUndoInfoIfSetCoordSystemOccurred`), so the wheel alone
    /// adds no undo step.
    pub fn zoom_run(
        &mut self,
        cam_before: Camera,
        zoomed: bool,
        store_at_the_end: bool,
    ) -> Option<Camera> {
        if zoomed {
            self.before.get_or_insert(cam_before);
            self.run = true;
            None
        } else if self.run {
            self.run = false;
            if store_at_the_end {
                self.end(cam_before)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    /// The wheel: x1.1 for one notch under the owner's proportional rule
    /// (spec D2), x1.1 a event whatever the delta under the reference's
    /// (`wrapMouseWheelMoved`: only the sign decides); Alt strengthens
    /// the zoom, horizontal scrolling zooms nothing.
    #[test]
    fn the_wheel_zooms_a_notch_or_an_event_of_x11() {
        // The owner's rule: proportional to the scroll amount.
        assert!(close(
            wheel_zoom_factor(false, wheel_notches(false, 100.0, 0.0), 1.0, false),
            1.1
        ));
        assert!(close(
            wheel_zoom_factor(false, wheel_notches(false, -100.0, 0.0), -1.0, false),
            1.0 / 1.1
        ));
        assert!(close(
            wheel_zoom_factor(false, wheel_notches(false, 30.0, 0.0), 1.0, false),
            1.1f64.powf(0.3)
        ));
        assert!(close(
            wheel_zoom_factor(false, wheel_notches(true, 1.0, 0.0), 1.0, false),
            1.1
        ));
        assert!(close(
            wheel_zoom_factor(false, wheel_notches(false, 100.0, 0.0), 1.0, true),
            1.65
        ));
        for dx in [-2.0, 0.0, 5.0] {
            assert_eq!(wheel_notches(false, 0.0, dx), 0.0);
        }
        assert_eq!(wheel_notches(false, f64::NAN, 0.0), 0.0);
        // The reference's: one x1.1 an event, only the sign of the delta
        // decides; two events of a frame zoom twice as far, however small
        // their deltas are.
        assert!(close(wheel_zoom_factor(true, 0.0, 1.0, false), 1.1));
        assert!(close(wheel_zoom_factor(true, 0.0, -1.0, false), 1.0 / 1.1));
        assert!(close(wheel_zoom_factor(true, 0.0, 2.0, false), 1.1 * 1.1));
        assert!(close(wheel_zoom_factor(true, 0.0, 0.0, false), 1.0));
        assert!(close(wheel_zoom_factor(true, 0.0, 1.0, true), 1.65));
    }

    /// A pinch keeps the world points under the fingers (the owner's rule,
    /// spec H4, no dead band): spreading 200 to 300 px about a midpoint at
    /// (312, 247) scales by 1.5 about it and leaves the point under the
    /// midpoint there; a midpoint that moves carries that point along.
    #[test]
    fn a_pinch_is_exactly_under_the_fingers() {
        let mut cam = Camera::uniform(0.0, 0.0, 50.0);
        let mut pinch = Pinch::start((212.0, 247.0), (412.0, 247.0), PinchBands::OURS);
        for k in 0..15 {
            // The fingers spread in steps of 3 1/3 px each, as the recorded
            // observation moved them.
            let d = 200.0 + (20.0 / 3.0) * (k + 1) as f64;
            pinch_move(
                &mut cam,
                &mut pinch,
                (312.0 - 0.5 * d, 247.0),
                (312.0 + 0.5 * d, 247.0),
                892.0,
                747.0,
                ScaleLimits::OURS,
            );
        }
        assert!(close(cam.xscale, 75.0) && close(cam.yscale, 75.0));
        let (sx, sy) = cam.transform(892.0, 747.0).world_to_screen(-2.68, 2.53);
        assert!(
            (sx - 312.0).abs() < 1e-9 && (sy - 247.0).abs() < 1e-9,
            "{sx} {sy}"
        );
        // The midpoint follows: the same world point under the fingers that
        // moved.
        let mut cam = Camera::uniform(0.0, 0.0, 50.0);
        let mut pinch = Pinch::start((212.0, 247.0), (412.0, 247.0), PinchBands::OURS);
        pinch_move(
            &mut cam,
            &mut pinch,
            (210.0, 280.0),
            (510.0, 280.0),
            892.0,
            747.0,
            ScaleLimits::OURS,
        );
        let (sx, sy) = cam.transform(892.0, 747.0).world_to_screen(-2.68, 2.53);
        assert!(
            (sx - 360.0).abs() < 1e-9 && (sy - 280.0).abs() < 1e-9,
            "{sx} {sy}"
        );
    }

    /// The reference app's own pinch (`twoTouchMoveCommon` with its dead
    /// bands, `MINIMAL_PIXEL_DIFFERENCE_FOR_ZOOM` 10 and `MIN_MOVE` 5):
    /// a zoom step applies only once the finger distance changed by more
    /// than 10 px, and the base moves to what applied, so the recorded
    /// spread of 200 to 300 px — each finger moving in steps of 3 1/3 px,
    /// one event a finger with the other one standing — zooms by 1.485
    /// and not 1.5: the observation F5 measured the scale as 74.25
    /// from 50.
    #[test]
    fn the_reference_pinch_lags_behind_its_fingers() {
        let mut cam = Camera::uniform(0.0, 0.0, 50.0);
        // Whole pixels, as the reference counts the fingers (`GPoint`).
        let px = |v: f64| (446.0 + v * 50.0).round();
        let (mut left, mut right) = (px(-4.68), px(-0.68));
        let mut pinch = Pinch::start((left, 247.0), (right, 247.0), PinchBands::REFERENCE);
        for k in 0..15 {
            // One finger moves at a time, the other standing where it was:
            // the left one first, as the recorded events came.
            let step = |cam: &mut Camera, pinch: &mut Pinch, a, b| {
                pinch_move(cam, pinch, a, b, 892.0, 747.0, ScaleLimits::REFERENCE);
            };
            left = px(-4.68 - (k + 1) as f64 / 15.0);
            step(&mut cam, &mut pinch, (left, 247.0), (right, 247.0));
            right = px(-0.68 + (k + 1) as f64 / 15.0);
            step(&mut cam, &mut pinch, (left, 247.0), (right, 247.0));
        }
        assert!(close(cam.xscale, 74.25), "{}", cam.xscale);
    }

    /// `setScaleAxis`: the grabbed tick follows the pointer; observations
    /// B4v (50 -> 66.67 with the origin standing at the centre) and the
    /// same drag left of the origin (33.33).
    #[test]
    fn an_axis_drag_rescales_about_the_origin() {
        let cam = Camera::STANDARD;
        let out = axis_scale(
            &cam,
            true,
            ScreenAxis {
                zero: 446.0,
                size: 892.0,
                min: -8.92,
                max: 8.92,
            },
            3.0,
            646.0,
            ScaleLimits::OURS,
        );
        assert!(close(out.xscale, 200.0 / 3.0), "{}", out.xscale);
        assert!(close(out.yscale, 50.0));
        assert!(close(out.cx, 0.0));
        let out = axis_scale(
            &cam,
            true,
            ScreenAxis {
                zero: 446.0,
                size: 892.0,
                min: -8.92,
                max: 8.92,
            },
            -3.0,
            346.0,
            ScaleLimits::OURS,
        );
        assert!(close(out.xscale, 100.0 / 3.0), "{}", out.xscale);
        assert!(close(out.cx, 0.0));
        // The y axis: dragging the tick at world 3 down to the pixel of
        // world 2 scales by 2/3 with the sign handled (`scaleYAxis`).
        let out = axis_scale(
            &cam,
            false,
            ScreenAxis {
                zero: 373.5,
                size: 747.0,
                min: -7.47,
                max: 7.47,
            },
            3.0,
            273.5,
            ScaleLimits::OURS,
        );
        assert!(close(out.yscale, 100.0 / 3.0), "{}", out.yscale);
        assert!(close(out.xscale, 50.0));
        // Near the origin the pointer counts from 2 px away
        // (`MIN_MOUSE_MOVE_FOR_AXIS_SCALE`); a tick dragged across the
        // origin would flip the scale's sign, which the view refuses.
        let out = axis_scale(
            &cam,
            true,
            ScreenAxis {
                zero: 446.0,
                size: 892.0,
                min: -8.92,
                max: 8.92,
            },
            0.01,
            446.0,
            ScaleLimits::OURS,
        );
        assert_eq!(out, cam);
        let out = axis_scale(
            &cam,
            true,
            ScreenAxis {
                zero: 446.0,
                size: 892.0,
                min: -8.92,
                max: 8.92,
            },
            2.0,
            447.0,
            ScaleLimits::OURS,
        );
        assert!(close(out.xscale, 2.0 / 2.0), "{}", out.xscale);
    }

    /// `updateSelectionRectangle` for the zoom tools: a rectangle near the
    /// view's ratio takes that ratio on; one far from it stays as drawn.
    #[test]
    fn a_zoom_rectangle_snaps_near_the_views_ratio() {
        // 892 x 747: ratio 1.194. A square is within 1.2 of it and becomes
        // the view's ratio; a 2:1 rectangle is not.
        let (x0, y0, x1, y1) = zoom_rectangle((100.0, 100.0), (200.0, 200.0), 892.0, 747.0);
        assert!(
            ((x1 - x0) / (y1 - y0) - 892.0 / 747.0).abs() < 1e-9,
            "{:?}",
            (x1 - x0, y1 - y0)
        );
        let (x0, y0, x1, y1) = zoom_rectangle((100.0, 100.0), (300.0, 200.0), 892.0, 747.0);
        assert_eq!((x0, y0, x1, y1), (100.0, 100.0, 300.0, 200.0));
        // A flat drag keeps its sign and corner: the square snaps to the
        // view's ratio too, and the start corner (200, 200) stands.
        let (x0, y0, x1, y1) = zoom_rectangle((200.0, 200.0), (100.0, 100.0), 892.0, 747.0);
        assert!(close(x0, 200.0 - 100.0 * 892.0 / 747.0), "{x0}");
        assert_eq!((y0, x1, y1), (100.0, 200.0, 200.0));
    }

    /// The view-gesture tracker: a step when the view changed, none when
    /// it did not, the open one handed out when a second begins.
    #[test]
    fn a_view_gesture_is_one_step_when_it_changed_the_view() {
        let mut g = ViewGesture::default();
        let cam = Camera::STANDARD;
        assert_eq!(g.begin(cam), None);
        assert!(g.is_open());
        let zoomed = Camera::uniform(0.0, 0.0, 55.0);
        assert_eq!(g.end(zoomed), Some(cam));
        assert!(!g.is_open());
        // Nothing changed: no step.
        g.begin(cam);
        assert_eq!(g.end(cam), None);
        // A cancelled gesture goes back without a step.
        g.begin(cam);
        assert_eq!(g.cancel(zoomed), Some(cam));
        // A gesture replaced by another is handed out first.
        g.begin(cam);
        assert_eq!(g.begin(zoomed), Some(cam));
        assert_eq!(g.end(zoomed), None);
    }

    /// A run of wheel frames under the owner's rule: the step comes with
    /// the first frame after it that zoomed none. Under the reference's
    /// (spec F3) the run adds no step at all: it waits, still open, for
    /// the next canvas press, and the one step it stores there covers
    /// every view change since.
    #[test]
    fn a_wheel_run_steps_at_its_end_or_waits_for_the_press() {
        let mut g = ViewGesture::default();
        let cam = Camera::STANDARD;
        assert_eq!(g.zoom_run(cam, true, true), None);
        assert_eq!(
            g.zoom_run(Camera::uniform(0.0, 0.0, 55.0), true, true),
            None
        );
        let after = Camera::uniform(0.0, 0.0, 60.5);
        assert_eq!(g.zoom_run(after, false, true), Some(cam));
        assert_eq!(g.zoom_run(after, false, true), None);
        // A run that changed nothing is no step.
        assert_eq!(g.zoom_run(cam, true, true), None);
        assert_eq!(g.zoom_run(cam, false, true), None);
        // The reference's rule: the wheel alone adds no step. The run stays
        // open — a second run goes on counting from the same camera — and
        // the press (any `end`) stores the one step.
        let mut g = ViewGesture::default();
        assert_eq!(g.zoom_run(cam, true, false), None);
        assert_eq!(
            g.zoom_run(Camera::uniform(0.0, 0.0, 55.0), true, false),
            None
        );
        assert_eq!(
            g.zoom_run(Camera::uniform(0.0, 0.0, 60.5), false, false),
            None
        );
        assert!(g.is_open(), "the run waits for the press");
        assert_eq!(
            g.zoom_run(Camera::uniform(0.0, 0.0, 66.55), true, false),
            None
        );
        assert_eq!(
            g.zoom_run(Camera::uniform(0.0, 0.0, 73.2), false, false),
            None
        );
        // The press ends it: one step, from the camera the wheel began at.
        assert_eq!(g.end(Camera::uniform(0.0, 0.0, 73.2)), Some(cam));
    }

    /// The owner's switches, as this window runs them against the
    /// reference app's (`OwnerRules`): the wheel proportional with a notch
    /// of x1.1 against one fixed x1.1 an event, the view changes undo
    /// steps right after their gesture against the reference's lazy
    /// storing, the pinch exact under the fingers against its dead bands,
    /// the pan never locked to an axis, the zoom limits our own. The
    /// settings are constants, so the assertions are; they pin them, and
    /// the lint that flags constant assertions is allowed here for that.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_owner_switches_are_set_to_our_behaviour() {
        use crate::app::OwnerRules;
        assert_eq!(WHEEL_ZOOM_FACTOR, 1.1);
        assert_eq!(WHEEL_POINTS_PER_NOTCH, 100.0);
        let ours = OwnerRules::OURS;
        let reference = OwnerRules::REFERENCE;
        assert!(ours.view_changes_are_steps);
        assert!(!reference.view_changes_are_steps);
        assert!(!ours.wheel_zoom_fixed_per_event);
        assert!(reference.wheel_zoom_fixed_per_event);
        assert_eq!(
            ours.pinch_bands,
            PinchBands {
                zoom: 0.0,
                pan: 0.0
            }
        );
        assert_eq!(
            reference.pinch_bands,
            PinchBands {
                zoom: 10.0,
                pan: 5.0
            }
        );
        assert!(!ours.pan_axis_lock);
        assert!(reference.pan_axis_lock);
        assert_eq!((ours.scale_limits.min, ours.scale_limits.max), (1e-6, 1e9));
        assert_eq!(
            (reference.scale_limits.min, reference.scale_limits.max),
            (1e-15, 1e15)
        );
    }
}
