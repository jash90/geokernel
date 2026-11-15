//! Sizes that depend on how the window is used: a mouse is precise and
//! hovers, a finger is wide and does not. One profile is chosen when the
//! window starts and read by reference afterwards.

/// The sizes of one input profile, in points (egui's unit, about one
/// density-independent pixel on a phone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// How far the pointer may move from the press before the press becomes
    /// a drag; a release within it is a tap.
    pub drag_slop: f32,
    /// A press held longer than this, in seconds, is no longer a tap: it
    /// becomes a drag where it is.
    pub click_time: f64,
    /// How far from a point a press still picks it.
    pub point_radius: f64,
    /// How far from a line or curve a press still picks it.
    pub curve_radius: f64,
    /// How far from a free point a press still grabs it for a drag.
    pub grab_radius: f64,
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
    /// The mouse profile: exactly the sizes the window always had (egui's
    /// click distance and time, the window's own hit radii and layout).
    pub const fn desktop() -> Metrics {
        Metrics {
            drag_slop: 6.0,
            click_time: 0.8,
            point_radius: 11.0,
            curve_radius: 7.0,
            // The drawn radius of a free point (5) plus 6.
            grab_radius: 11.0,
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
            drag_slop: 12.0,
            click_time: 0.8,
            point_radius: 22.0,
            curve_radius: 16.0,
            grab_radius: 24.0,
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
    fn touch_targets_are_larger_than_mouse_ones() {
        let (d, t) = (Metrics::desktop(), Metrics::touch());
        assert!(t.drag_slop > d.drag_slop && t.point_radius > d.point_radius);
        assert!(t.curve_radius > d.curve_radius && t.grab_radius > d.grab_radius);
        assert!(t.toolbar_button > d.toolbar_button && t.toolbar_pitch > t.toolbar_button);
        assert_eq!(
            d.portrait_below, 0.0,
            "the desktop layout never turns portrait"
        );
    }
}
