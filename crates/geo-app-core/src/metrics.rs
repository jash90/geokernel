//! Sizes that depend on how the window is used: a mouse is precise and
//! hovers, a finger is wide and does not. One profile is chosen when the
//! window starts and read by reference afterwards.

/// `App.DEFAULT_THRESHOLD` (3 px) and `App.TOUCH_FACTOR` (3).
const DEFAULT_THRESHOLD: f64 = 3.0;
const TOUCH_FACTOR: f64 = 3.0;
/// `DrawPoint.SELECTION_RADIUS_MIN` (12 px).
const SELECTION_RADIUS_MIN: f64 = 12.0;

/// `EuclidianConstants.DRAGGING_DELAY` (100 ms), the same for every pointer.
const DRAGGING_DELAY: f64 = 0.1;
/// `EuclidianController.DRAG_THRESHOLD` (10 pixels), the same for every
/// pointer.
const DRAG_THRESHOLD: f32 = 10.0;

/// The sizes of one input profile, in points (egui's unit, about one
/// density-independent pixel on a phone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Moves of the pointer that come less than this many seconds after the
    /// press are dropped (`EuclidianController.shouldCancelDrag` with
    /// `EuclidianConstants.DRAGGING_DELAY`, 100 ms). There is no distance a
    /// drag must go first: the first move after this delay takes the dragged
    /// object straight to the pointer.
    pub drag_delay: f64,
    /// A release whose drag went no further than this from the press, on
    /// either axis, is a click (`EuclidianController.DRAG_THRESHOLD`, 10
    /// pixels, checked in `isDraggingBeyondThreshold`). It decides only the
    /// click; the motion is applied either way.
    pub drag_threshold: f32,
    /// egui's own click rules for the widgets around the drawing (buttons,
    /// rows, menus), which never take part in the drawing's gestures: how far
    /// a press may move and how long it may last and still click a widget.
    pub widget_click_dist: f32,
    pub widget_click_time: f64,
    /// The half-width of the square around the pointer that an object must
    /// meet to be hit, for the mouse and for a finger
    /// (`App.getCapturingThreshold`: `DEFAULT_THRESHOLD` 3, times
    /// `TOUCH_FACTOR` 3 for touch). It follows the pointer of each press,
    /// not the profile, so both profiles hold the same numbers.
    pub mouse_threshold: f64,
    pub touch_threshold: f64,
    /// A finger that hits nothing tries again with its threshold times this
    /// (`HitDetector.setHits(GPoint, PointerEventType)`: 3 x 9 = 27).
    pub touch_retry_factor: f64,
    /// A point is hit within the threshold plus this
    /// (`DrawPoint.SELECTION_RADIUS_MIN`, 12).
    pub point_selection_min: f64,
    /// The pointer can hover (a mouse). Without hover the window shows in
    /// the open what a mouse user finds by pointing, such as the hint of the
    /// move tool.
    pub hover: bool,
    /// The height of the toolbar across the top.
    pub toolbar_height: f32,
    /// The size of a tool group button, and the distance between the left
    /// edges of two neighbours.
    pub toolbar_button: f32,
    pub toolbar_pitch: f32,
    /// The width of the algebra view beside the graphics view.
    pub algebra_width: f32,
    /// The height of the on-screen keyboard.
    pub keyboard_height: f32,
    /// A window narrower than this is laid out for portrait: the algebra
    /// view goes under the graphics view and the tool groups scroll.
    pub portrait_below: f32,
}

impl Metrics {
    /// The mouse profile: the drawing's gesture rules of the reference app,
    /// egui's click distance and time for the widgets, the window's own hit
    /// radii and layout.
    pub const fn desktop() -> Metrics {
        Metrics {
            drag_delay: DRAGGING_DELAY,
            drag_threshold: DRAG_THRESHOLD,
            widget_click_dist: 6.0,
            widget_click_time: 0.8,
            mouse_threshold: DEFAULT_THRESHOLD,
            touch_threshold: DEFAULT_THRESHOLD * TOUCH_FACTOR,
            touch_retry_factor: TOUCH_FACTOR,
            point_selection_min: SELECTION_RADIUS_MIN,
            hover: true,
            toolbar_height: 52.0,
            toolbar_button: 40.0,
            toolbar_pitch: 45.0,
            algebra_width: 380.0,
            keyboard_height: 222.0,
            // Never: a narrow desktop window keeps its layout.
            portrait_below: 0.0,
        }
    }

    /// The touch profile. Provisional: the numbers follow the usual 48 dp
    /// touch target and have not been tuned on a device yet.
    pub const fn touch() -> Metrics {
        Metrics {
            drag_delay: DRAGGING_DELAY,
            drag_threshold: DRAG_THRESHOLD,
            widget_click_dist: 12.0,
            widget_click_time: 0.8,
            mouse_threshold: DEFAULT_THRESHOLD,
            touch_threshold: DEFAULT_THRESHOLD * TOUCH_FACTOR,
            touch_retry_factor: TOUCH_FACTOR,
            point_selection_min: SELECTION_RADIUS_MIN,
            hover: false,
            toolbar_height: 60.0,
            toolbar_button: 48.0,
            toolbar_pitch: 54.0,
            algebra_width: 320.0,
            keyboard_height: 222.0,
            portrait_below: 600.0,
        }
    }
}

impl Default for Metrics {
    fn default() -> Metrics {
        Metrics::desktop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_profile_keeps_the_sizes_the_window_always_had() {
        let d = Metrics::desktop();
        // Hit testing follows the reference app (was point 11, curve 7 and
        // grab 11 px radii): a 3 px square around the mouse, 9 px around a
        // finger with a retry at 27 (`App.getCapturingThreshold`,
        // `HitDetector.setHits`), points within the threshold plus 12
        // (`DrawPoint.SELECTION_RADIUS_MIN`).
        assert_eq!((d.mouse_threshold, d.touch_threshold), (3.0, 9.0));
        assert_eq!((d.touch_retry_factor, d.point_selection_min), (3.0, 12.0));
        // The drawing's gestures follow the reference app: a 100 ms delay
        // before the first move counts (`EuclidianConstants.DRAGGING_DELAY`)
        // and a 10 pixel click threshold (`EuclidianController
        // .DRAG_THRESHOLD`); the widgets keep egui's click rules.
        assert_eq!((d.drag_delay, d.drag_threshold), (0.1, 10.0));
        assert_eq!((d.widget_click_dist, d.widget_click_time), (6.0, 0.8));
        assert_eq!(
            (d.toolbar_height, d.toolbar_button, d.toolbar_pitch),
            (52.0, 40.0, 45.0)
        );
        assert_eq!((d.algebra_width, d.keyboard_height), (380.0, 222.0));
        assert!(d.hover);
    }

    #[test]
    fn touch_targets_are_larger_than_mouse_ones() {
        let (d, t) = (Metrics::desktop(), Metrics::touch());
        // The reference app uses one delay and one click threshold for every
        // pointer; only egui's widget clicks allow a wider wobble.
        assert_eq!(
            (t.drag_delay, t.drag_threshold),
            (d.drag_delay, d.drag_threshold)
        );
        assert!(t.widget_click_dist > d.widget_click_dist);
        // The hit thresholds follow the pointer of each press, not the
        // profile (`App.getCapturingThreshold(PointerEventType)`).
        assert_eq!(
            (t.mouse_threshold, t.touch_threshold, t.point_selection_min),
            (d.mouse_threshold, d.touch_threshold, d.point_selection_min)
        );
        assert!(d.touch_threshold > d.mouse_threshold);
        assert!(t.toolbar_button > d.toolbar_button && t.toolbar_pitch > t.toolbar_button);
        assert!(d.hover && !t.hover);
        assert_eq!(
            d.portrait_below, 0.0,
            "the desktop layout never turns portrait"
        );
    }
}
