//! Conversions between the core's screen types and egui's. Each one copies
//! fields; nothing is allocated or looked up.

use eframe::egui::{Color32, Pos2, Rect};
use geo_app_core::geom;

/// A core value as its egui counterpart.
pub trait ToEgui {
    type Egui;
    fn egui(self) -> Self::Egui;
}

/// An egui value as its core counterpart.
pub trait ToCore {
    type Core;
    fn core(self) -> Self::Core;
}

impl ToEgui for geom::Point {
    type Egui = Pos2;
    #[inline]
    fn egui(self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }
}

impl ToEgui for geom::Rect {
    type Egui = Rect;
    #[inline]
    fn egui(self) -> Rect {
        Rect::from_min_max(self.min.egui(), self.max.egui())
    }
}

impl ToEgui for geom::Rgba {
    type Egui = Color32;
    #[inline]
    fn egui(self) -> Color32 {
        rgba(self)
    }
}

impl ToCore for Pos2 {
    type Core = geom::Point;
    #[inline]
    fn core(self) -> geom::Point {
        geom::Point::new(self.x, self.y)
    }
}

impl ToCore for Rect {
    type Core = geom::Rect;
    #[inline]
    fn core(self) -> geom::Rect {
        geom::Rect::from_min_max(self.min.core(), self.max.core())
    }
}

/// A core colour as an egui colour, usable in constants.
#[inline]
pub const fn rgba(c: geom::Rgba) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip() {
        let r = Rect::from_min_max(Pos2::new(1.5, -2.0), Pos2::new(3.25, 8.0));
        assert_eq!(r.core().egui(), r);
        assert_eq!(
            rgba(geom::Rgba::from_rgb(1, 2, 3)),
            Color32::from_rgb(1, 2, 3)
        );
    }
}
