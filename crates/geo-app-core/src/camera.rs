// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianController.java
//   source/shared/common/src/main/java/org/geogebra/common/euclidian/EuclidianView.java
//   source/shared/common/src/main/java/org/geogebra/common/main/GlobalKeyDispatcher.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The view rules the camera alone decides: the wheel and keyboard zoom
//! factors, the pinch, the axis rescale, the zoom rectangle and the undo
//! rule for view changes, each following the reference app except where
//! the owner keeps another behaviour (one named switch each).

use crate::geom::Point;
use crate::view::{Camera, MAX_SCALE, MIN_SCALE};

/// The reference app's wheel zoom factor (`EuclidianView
/// .MOUSE_WHEEL_ZOOM_FACTOR`).
pub const WHEEL_ZOOM_FACTOR: f64 = 1.1;
/// With Alt held the wheel zooms this many times as far
/// (`wrapMouseWheelMoved`: the factor multiplied by 1.5).
pub const WHEEL_ALT_FACTOR: f64 = 1.5;
/// The reference app zooms one wheel event by `WHEEL_ZOOM_FACTOR` whatever
/// its delta. The owner keeps the zoom proportional to the scroll amount
/// (spec D2), with this many scroll points per notch, as a browser's wheel
/// reports one.
pub const WHEEL_POINTS_PER_NOTCH: f64 = 100.0;
/// The keyboard zoom (`GlobalKeyDispatcher`: Ctrl + plus and Ctrl + minus)
/// steps by the wheel factor about the last pointer position.
pub const KEY_ZOOM_FACTOR: f64 = WHEEL_ZOOM_FACTOR;

/// The owner's switch (spec F3): every view change — a pan, a pinch, a
/// wheel or keyboard zoom, an axis rescale, a zoom rectangle, a zoom-tool
/// click, a zoom to fit, the standard view — is one undo step right after
/// the gesture that made it. The reference app stores a view change at the
/// next canvas press or when the pointer leaves the canvas (the wheel), at
/// the end of the tool (a zoom-tool click), at the release of a drag, and
/// never for the zoom buttons. `false` would follow that.
pub const VIEW_CHANGES_ARE_STEPS: bool = true;

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

/// The zoom factor of `notches` wheel notches of one frame: the owner's
/// proportional rule, `WHEEL_ZOOM_FACTOR` to the notches (x1.1 for one
/// notch, as one event of the reference app zooms), and `alt` multiplies
/// the base by `WHEEL_ALT_FACTOR` (`wrapMouseWheelMoved`).
pub fn wheel_factor(notches: f64, alt: bool) -> f64 {
    let base = if alt {
        WHEEL_ZOOM_FACTOR * WHEEL_ALT_FACTOR
    } else {
        WHEEL_ZOOM_FACTOR
    };
    base.powf(notches)
}

/// The owner's choice (spec H4): a pinch tracks the fingers exactly —
/// every frame's ratio about the frame's midpoint, however little the
/// distance changed. The reference app applies a zoom step only once the
/// finger distance changed by more than this many pixels
/// (`twoTouchMoveCommon`), and keeps its last applied distance as the next
/// step's base, which is why a spread of 1.5 was measured as 1.485;
/// `10.0` follows that.
pub const PINCH_DEAD_BAND: f64 = 0.0;
/// The same rule for the midpoint's movement (`MIN_MOVE`, 5 px): the
/// reference app pans only once it moved more than this. `0.0`, as the
/// owner keeps it, pans with every frame.
pub const PINCH_PAN_DEAD_BAND: f64 = 0.0;

/// The owner's choice (spec D13): a pan follows both axes as the pointer
/// moves them. The reference app locks a pan to one axis — within the
/// first 40 px (`DISTANCE_THRESHOLD`) a move of more than 5 px
/// (`INITIAL_DISTANCE_THRESHOLD`) within ±25° of an axis
/// (`ANGLE_OPENING`) locks that axis until the other component exceeds
/// 50 px (`MOVE_VIEW_THRESHOLD`); `SnapController`. `true` would follow
/// it; the pan path implements no lock while this is `false`.
pub const PAN_AXIS_LOCK: bool = false;

/// A pinch of two fingers: the world points under the fingers when the
/// pinch began stay under them — the view scales by the ratio of the
/// finger distances about the midpoint, which follows its own movement
/// (spec H4). `from` is the midpoint and the distance of the fingers as
/// they were, `to` as they are. A change inside the dead bands moves
/// nothing (a band of 0, the owner's, skips only the changes that are no
/// change).
pub fn pinch_to(
    cam: &Camera,
    from: (Point, f64),
    to: (Point, f64),
    width: f64,
    height: f64,
) -> Camera {
    let (a, d0) = from;
    let (b, d1) = to;
    let mut out = *cam;
    if d0 > 0.0 && d1.is_finite() && d1 > 0.0 {
        let world = cam
            .transform(width, height)
            .screen_to_world(a.x as f64, a.y as f64);
        // Scale about the new midpoint, then put the world point of the old
        // one under it: it ends up where the fingers are.
        if (d1 - d0).abs() > PINCH_DEAD_BAND {
            out.zoom_at(d1 / d0, b.x as f64, b.y as f64, width, height);
        }
        if ((b.x - a.x) as f64).hypot((b.y - a.y) as f64) > PINCH_PAN_DEAD_BAND {
            let at = out
                .transform(width, height)
                .world_to_screen(world.0, world.1);
            out.pan_pixels(b.x as f64 - at.0, b.y as f64 - at.1);
        }
    }
    out
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
    let scale = scale.clamp(MIN_SCALE, MAX_SCALE);
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

/// One view change followed for the undo rule (`VIEW_CHANGES_ARE_STEPS`):
/// the camera before the gesture — a drag, a pinch, or a run of wheel or
/// keyboard zoom frames. The window asks for the step when the gesture
/// ends; one that changed nothing is none. Only one is open at a time: a
/// gesture that a second one replaces is handed out first, as its own
/// step.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewGesture {
    before: Option<Camera>,
    /// The open gesture is a run of wheel frames, ended by the first
    /// frame without a zoom — not a drag or a pinch, which end when they
    /// end.
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
    /// frames that zoomed, and its step is recorded when the run ends —
    /// the first frame after it that zoomed none (the owner's rule: right
    /// after the gesture).
    pub fn zoom_run(&mut self, cam_before: Camera, zoomed: bool) -> Option<Camera> {
        if zoomed {
            self.before.get_or_insert(cam_before);
            self.run = true;
            None
        } else if self.run {
            self.run = false;
            self.end(cam_before)
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

    /// The owner's wheel rule: x1.1 for one notch, proportional to the
    /// scroll amount, stronger with Alt, nothing for horizontal scrolling.
    #[test]
    fn the_wheel_zoom_is_proportional_with_a_notch_of_x11() {
        assert!(close(
            wheel_factor(wheel_notches(false, 100.0, 0.0), false),
            1.1
        ));
        assert!(close(
            wheel_factor(wheel_notches(false, -100.0, 0.0), false),
            1.0 / 1.1
        ));
        assert!(close(
            wheel_factor(wheel_notches(false, 30.0, 0.0), false),
            1.1f64.powf(0.3)
        ));
        assert!(close(
            wheel_factor(wheel_notches(true, 1.0, 0.0), false),
            1.1
        ));
        assert!(close(
            wheel_factor(wheel_notches(false, 100.0, 0.0), true),
            1.65
        ));
        for dx in [-2.0, 0.0, 5.0] {
            assert_eq!(wheel_notches(false, 0.0, dx), 0.0);
        }
        assert_eq!(wheel_notches(false, f64::NAN, 0.0), 0.0);
    }

    /// A pinch keeps the world points under the fingers (the owner's rule,
    /// spec H4): spreading 200 to 300 px about a midpoint at (312, 247)
    /// scales by 1.5 about it and leaves the point under the midpoint
    /// there.
    #[test]
    fn a_pinch_is_exactly_under_the_fingers() {
        let cam = Camera::uniform(0.0, 0.0, 50.0);
        let out = pinch_to(
            &cam,
            (Point::new(312.0, 247.0), 200.0),
            (Point::new(312.0, 247.0), 300.0),
            892.0,
            747.0,
        );
        assert!(close(out.xscale, 75.0) && close(out.yscale, 75.0));
        let (sx, sy) = out.transform(892.0, 747.0).world_to_screen(-2.68, 2.53);
        assert!(
            (sx - 312.0).abs() < 1e-9 && (sy - 247.0).abs() < 1e-9,
            "{sx} {sy}"
        );
        // The midpoint follows: the same world point under the fingers that
        // moved.
        let out = pinch_to(
            &cam,
            (Point::new(312.0, 247.0), 200.0),
            (Point::new(360.0, 280.0), 300.0),
            892.0,
            747.0,
        );
        let (sx, sy) = out.transform(892.0, 747.0).world_to_screen(-2.68, 2.53);
        assert!(
            (sx - 360.0).abs() < 1e-9 && (sy - 280.0).abs() < 1e-9,
            "{sx} {sy}"
        );
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

    /// A run of wheel frames: the step waits for the first frame after it
    /// that zoomed none.
    #[test]
    fn a_wheel_run_ends_at_the_first_frame_without_a_zoom() {
        let mut g = ViewGesture::default();
        let cam = Camera::STANDARD;
        assert_eq!(g.zoom_run(cam, true), None);
        assert_eq!(g.zoom_run(Camera::uniform(0.0, 0.0, 55.0), true), None);
        let after = Camera::uniform(0.0, 0.0, 60.5);
        assert_eq!(g.zoom_run(after, false), Some(cam));
        assert_eq!(g.zoom_run(after, false), None);
        // A run that changed nothing is no step.
        assert_eq!(g.zoom_run(cam, true), None);
        assert_eq!(g.zoom_run(cam, false), None);
    }

    /// The owner's switches, as this window runs them: the wheel
    /// proportional with a notch of x1.1, the view changes undo steps right
    /// after their gesture, the pinch exact under the fingers, the pan
    /// never locked to an axis, the zoom limits our own. The switches are
    /// constants, so the assertions are; they pin the settings, and the
    /// lint that flags constant assertions is allowed here for that.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_owner_switches_are_set_to_our_behaviour() {
        assert_eq!(WHEEL_ZOOM_FACTOR, 1.1);
        assert_eq!(WHEEL_POINTS_PER_NOTCH, 100.0);
        assert!(VIEW_CHANGES_ARE_STEPS);
        assert_eq!(PINCH_DEAD_BAND, 0.0);
        assert_eq!(PINCH_PAN_DEAD_BAND, 0.0);
        assert!(!PAN_AXIS_LOCK);
        assert_eq!((MIN_SCALE, MAX_SCALE), (1e-6, 1e9));
    }
}
