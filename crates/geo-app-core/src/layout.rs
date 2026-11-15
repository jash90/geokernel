//! Where the regions of the window go: the toolbar across the top, the
//! algebra view on the left, the graphics view on the right and, when open,
//! the on-screen keyboard across the bottom. A window narrower than the
//! profile's `portrait_below` puts the algebra view under the graphics view
//! instead. Pure geometry, used by the window and by the tests.

use crate::geom::{Point, Rect};
use crate::metrics::Metrics;

fn pos2(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// The bar between the algebra and the graphics view.
pub const SPLITTER: f32 = 8.0;

/// The share of the height under the toolbar the algebra view takes in the
/// portrait layout.
const PORTRAIT_ALGEBRA_SHARE: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regions {
    pub toolbar: Rect,
    pub algebra: Rect,
    pub splitter: Rect,
    pub graphics: Rect,
    pub keyboard: Option<Rect>,
    /// The algebra view is under the graphics view, and the splitter is a
    /// horizontal bar.
    pub portrait: bool,
}

impl Regions {
    pub fn new(window: Rect, keyboard_open: bool, m: &Metrics) -> Regions {
        let toolbar = Rect::from_min_max(
            window.min,
            pos2(window.right(), window.top() + m.toolbar_height),
        );
        // A small window keeps the toolbar and at least a sliver of each view.
        let keyboard_height = m
            .keyboard_height
            .min((window.height() - m.toolbar_height) * 0.5);
        let keyboard = (keyboard_open && keyboard_height > 0.0).then(|| {
            Rect::from_min_max(
                pos2(window.left(), window.bottom() - keyboard_height),
                window.max,
            )
        });
        let bottom = keyboard.map_or(window.bottom(), |k| k.top());
        if window.width() < m.portrait_below {
            return Regions::portrait(window, toolbar, keyboard, bottom);
        }
        let algebra_width = m.algebra_width.min((window.width() - SPLITTER) * 0.5);
        let algebra = Rect::from_min_max(
            pos2(window.left(), toolbar.bottom()),
            pos2(window.left() + algebra_width, bottom),
        );
        let splitter = Rect::from_min_max(
            pos2(algebra.right(), toolbar.bottom()),
            pos2(algebra.right() + SPLITTER, bottom),
        );
        let graphics = Rect::from_min_max(
            pos2(splitter.right(), toolbar.bottom()),
            pos2(window.right(), bottom),
        );
        Regions {
            toolbar,
            algebra,
            splitter,
            graphics,
            keyboard,
            portrait: false,
        }
    }

    /// The graphics view under the toolbar, then the splitter, then the
    /// algebra view, all across the full width.
    fn portrait(window: Rect, toolbar: Rect, keyboard: Option<Rect>, bottom: f32) -> Regions {
        let free = (bottom - toolbar.bottom() - SPLITTER).max(0.0);
        let algebra_top = bottom - free * PORTRAIT_ALGEBRA_SHARE;
        let algebra = Rect::from_min_max(
            pos2(window.left(), algebra_top),
            pos2(window.right(), bottom),
        );
        let splitter = Rect::from_min_max(
            pos2(window.left(), algebra_top - SPLITTER),
            pos2(window.right(), algebra_top),
        );
        let graphics = Rect::from_min_max(
            pos2(window.left(), toolbar.bottom()),
            pos2(window.right(), splitter.top()),
        );
        Regions {
            toolbar,
            algebra,
            splitter,
            graphics,
            keyboard,
            portrait: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESKTOP: Metrics = Metrics::desktop();

    #[test]
    fn regions_tile_the_window() {
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(1728.0, 1050.0));
        let r = Regions::new(w, false, &DESKTOP);
        assert_eq!(r.toolbar.height(), DESKTOP.toolbar_height);
        assert_eq!(r.algebra.width(), DESKTOP.algebra_width);
        assert_eq!(r.graphics.left(), DESKTOP.algebra_width + SPLITTER);
        assert_eq!(r.graphics.bottom(), 1050.0);
        assert_eq!(r.keyboard, None);
        assert!(!r.portrait);
        let k = Regions::new(w, true, &DESKTOP);
        assert_eq!(k.keyboard.unwrap().height(), DESKTOP.keyboard_height);
        assert_eq!(k.graphics.bottom(), 1050.0 - DESKTOP.keyboard_height);
        assert_eq!(k.algebra.bottom(), k.graphics.bottom());
    }

    #[test]
    fn a_small_window_keeps_both_views() {
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 200.0));
        let r = Regions::new(w, true, &DESKTOP);
        assert!(r.graphics.width() > 0.0 && r.algebra.width() > 0.0);
        assert!(r.graphics.height() > 0.0);
        assert!(!r.portrait, "the desktop profile never turns portrait");
    }

    #[test]
    fn a_narrow_touch_window_puts_the_algebra_view_under_the_graphics_view() {
        let touch = Metrics::touch();
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 800.0));
        let r = Regions::new(w, false, &touch);
        assert!(r.portrait);
        assert_eq!(r.graphics.width(), 400.0);
        assert_eq!(r.algebra.width(), 400.0);
        assert_eq!(r.graphics.top(), touch.toolbar_height);
        assert_eq!(r.splitter.top(), r.graphics.bottom());
        assert_eq!(r.algebra.top(), r.splitter.bottom());
        assert_eq!(r.algebra.bottom(), 800.0);
        assert!(r.graphics.height() > r.algebra.height());
        // With the keyboard open both views end above it.
        let k = Regions::new(w, true, &touch);
        assert_eq!(k.algebra.bottom(), k.keyboard.unwrap().top());
        // A wide touch window keeps the side-by-side layout.
        let wide = Rect::from_min_max(pos2(0.0, 0.0), pos2(1200.0, 800.0));
        assert!(!Regions::new(wide, false, &touch).portrait);
    }
}
