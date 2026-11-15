//! Where the regions of the window go: the toolbar across the top, the
//! algebra view on the left, the graphics view on the right and, when open,
//! the on-screen keyboard across the bottom. Pure geometry, used by the
//! window and by the tests.

use eframe::egui::{pos2, Rect};

pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const ALGEBRA_WIDTH: f32 = 380.0;
/// The bar between the algebra and the graphics view.
pub const SPLITTER: f32 = 8.0;
pub const KEYBOARD_HEIGHT: f32 = 222.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regions {
    pub toolbar: Rect,
    pub algebra: Rect,
    pub splitter: Rect,
    pub graphics: Rect,
    pub keyboard: Option<Rect>,
}

impl Regions {
    pub fn new(window: Rect, keyboard_open: bool) -> Regions {
        let toolbar = Rect::from_min_max(
            window.min,
            pos2(window.right(), window.top() + TOOLBAR_HEIGHT),
        );
        // A small window keeps the toolbar and at least a sliver of each view.
        let keyboard_height = KEYBOARD_HEIGHT.min((window.height() - TOOLBAR_HEIGHT) * 0.5);
        let keyboard = (keyboard_open && keyboard_height > 0.0).then(|| {
            Rect::from_min_max(
                pos2(window.left(), window.bottom() - keyboard_height),
                window.max,
            )
        });
        let bottom = keyboard.map_or(window.bottom(), |k| k.top());
        let algebra_width = ALGEBRA_WIDTH.min((window.width() - SPLITTER) * 0.5);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_tile_the_window() {
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(1728.0, 1050.0));
        let r = Regions::new(w, false);
        assert_eq!(r.toolbar.height(), TOOLBAR_HEIGHT);
        assert_eq!(r.algebra.width(), ALGEBRA_WIDTH);
        assert_eq!(r.graphics.left(), ALGEBRA_WIDTH + SPLITTER);
        assert_eq!(r.graphics.bottom(), 1050.0);
        assert_eq!(r.keyboard, None);
        let k = Regions::new(w, true);
        assert_eq!(k.keyboard.unwrap().height(), KEYBOARD_HEIGHT);
        assert_eq!(k.graphics.bottom(), 1050.0 - KEYBOARD_HEIGHT);
        assert_eq!(k.algebra.bottom(), k.graphics.bottom());
    }

    #[test]
    fn a_small_window_keeps_both_views() {
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 200.0));
        let r = Regions::new(w, true);
        assert!(r.graphics.width() > 0.0 && r.algebra.width() > 0.0);
        assert!(r.graphics.height() > 0.0);
    }
}
