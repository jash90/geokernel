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
}

impl Metrics {
    /// The mouse profile: exactly the sizes the window always had (egui's
    /// click distance and time, and the window's own hit radii).
    pub const fn desktop() -> Metrics {
        Metrics {
            drag_slop: 6.0,
            click_time: 0.8,
            point_radius: 11.0,
            curve_radius: 7.0,
            // The drawn radius of a free point (5) plus 6.
            grab_radius: 11.0,
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
        }
    }
}

impl Default for Metrics {
    fn default() -> Metrics {
        Metrics::desktop()
    }
}
