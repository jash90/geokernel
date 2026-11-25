//! Math lists become baseline-relative boxes and a portable paint list.
//! Spacing, styles, scripts and fractions follow the structure of Appendix G
//! of Donald Knuth's The TeXbook, with em-based parameters for bundled fonts.

mod font;
mod layout;
mod parser;
mod svg;

pub use svg::write_svg;

pub type Color = [u8; 3];
pub type Point = [f32; 2];

pub const FALLBACK_COLOR: Color = [176, 44, 113];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Style {
    #[default]
    Display,
    Text,
    Script,
    ScriptScript,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Face {
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub origin: Point,
    pub x: Point,
    pub y: Point,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0],
            x: [1.0, 0.0],
            y: [0.0, 1.0],
        }
    }
}

impl Transform {
    pub fn vector(self, [x, y]: Point) -> Point {
        [self.x[0] * x + self.y[0] * y, self.x[1] * x + self.y[1] * y]
    }

    pub fn point(self, point: Point) -> Point {
        let p = self.vector(point);
        [p[0] + self.origin[0], p[1] + self.origin[1]]
    }

    fn compose(self, child: Self) -> Self {
        Self {
            origin: self.point(child.origin),
            x: self.vector(child.x),
            y: self.vector(child.y),
        }
    }

    pub fn translation_only(self) -> bool {
        self.x == [1.0, 0.0] && self.y == [0.0, 1.0]
    }
}

#[derive(Debug)]
pub enum Shape {
    Glyphs { text: String, size: f32, face: Face },
    Stroke { points: Vec<Point>, width: f32 },
    Fill { points: Vec<Point> },
}

#[derive(Debug)]
pub struct Ink {
    pub shape: Shape,
    pub transform: Transform,
    /// None uses the object's foreground.
    pub color: Option<Color>,
}

#[derive(Debug, Default)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub ink: Vec<Ink>,
    pub has_fallback: bool,
}

impl Layout {
    pub fn size(&self) -> Point {
        [self.width, self.height + self.depth]
    }
}

/// Layout coordinates start at the left edge and first baseline. Positive y
/// goes down. The logical rectangle extends from -height to +depth.
pub fn layout(source: &str, size: f32, style: Style, face: Face) -> Layout {
    let size = if size.is_finite() {
        size.clamp(1.0, 256.0)
    } else {
        16.0
    };
    layout::layout(&parser::parse(source), size, style, face)
}

/// The widest line of plain text in the bundled face.
pub fn plain_width(text: &str, size: f32, face: Face) -> f32 {
    text.lines()
        .map(|line| font::measure(line, size, face).0)
        .fold(0.0, f32::max)
}

/// Measures plain (non-math) text with the same bundled font metrics the
/// renderer uses: one `(width, height, depth)` per line, as a [`Layout`]
/// has them. Callers that place plain text next to formulas (the SVG
/// export) agree with the formula renderer about a text's size.
pub fn plain_lines(text: &str, size: f32, face: Face) -> Vec<(f32, f32, f32)> {
    text.lines()
        .map(|line| font::measure(line, size, face))
        .collect()
}

#[cfg(test)]
mod tests;
