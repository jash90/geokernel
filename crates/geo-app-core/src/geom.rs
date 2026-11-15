//! Small value types for positions, rectangles and colours on the screen, in
//! the units of the window (points). They mirror the few operations the
//! logic needs and convert to the UI toolkit's types by copying fields.

/// A position on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// An axis-aligned rectangle; `min` is the top left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub const ZERO: Rect = Rect::from_min_max(Point::new(0.0, 0.0), Point::new(0.0, 0.0));

    #[inline]
    pub const fn from_min_max(min: Point, max: Point) -> Rect {
        Rect { min, max }
    }

    #[inline]
    pub fn left(&self) -> f32 {
        self.min.x
    }

    #[inline]
    pub fn right(&self) -> f32 {
        self.max.x
    }

    #[inline]
    pub fn top(&self) -> f32 {
        self.min.y
    }

    #[inline]
    pub fn bottom(&self) -> f32 {
        self.max.y
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Whether `p` is inside or on the border.
    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        self.min.x <= p.x && p.x <= self.max.x && self.min.y <= p.y && p.y <= self.max.y
    }
}

/// An opaque or translucent colour, 8 bits per channel, premultiplied by
/// alpha (for an opaque colour that makes no difference).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const WHITE: Rgba = Rgba::from_rgb(255, 255, 255);
    pub const BLACK: Rgba = Rgba::from_rgb(0, 0, 0);

    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rect_contains_its_border() {
        let r = Rect::from_min_max(Point::new(1.0, 2.0), Point::new(4.0, 6.0));
        assert_eq!((r.width(), r.height()), (3.0, 4.0));
        assert!(r.contains(Point::new(1.0, 6.0)));
        assert!(!r.contains(Point::new(0.9, 3.0)));
    }
}
