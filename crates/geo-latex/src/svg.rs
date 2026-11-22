use std::fmt::Write;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;

use crate::{font, Color, Layout, Point, Shape, Transform};

/// Appends vector ink at the given top-left position. Bundled font outlines
/// keep the file independent of the SVG viewer's installed fonts.
pub fn write_svg(out: &mut String, layout: &Layout, x: f64, y: f64, color: Color) {
    let _ = writeln!(out, r#"<g transform="translate({x:.2} {y:.2})">"#);
    for ink in &layout.ink {
        let mut transform = ink.transform;
        transform.origin[1] += layout.height;
        let color = hex(ink.color.unwrap_or(color));
        match &ink.shape {
            Shape::Glyphs { text, size, face } => {
                let mut path = String::new();
                let mut x = 0.0;
                let mut last = None;
                for ch in text.chars() {
                    let (font, id) = font::glyph(ch, *face);
                    if let Some(old) = last {
                        x += font::kern(old, ch, *size, *face);
                    }
                    if let Some(glyph) = font.outline_glyphs().get(id) {
                        let settings =
                            DrawSettings::unhinted(Size::new(*size), LocationRef::default());
                        let _ = glyph.draw(
                            settings,
                            &mut SvgPen {
                                path: &mut path,
                                x,
                                italic: face.italic,
                            },
                        );
                    }
                    x += font::advance(font, id, *size);
                    last = Some(ch);
                }
                if path.is_empty() {
                    continue;
                }
                let [a, b] = transform.x;
                let [c, d] = transform.y;
                let [x, y] = transform.origin;
                let _ = write!(
                    out,
                    r#"<path fill="{color}" transform="matrix({a:.5} {b:.5} {c:.5} {d:.5} {x:.2} {y:.2})""#
                );
                if face.bold {
                    let _ = write!(
                        out,
                        r#" stroke="{color}" stroke-width="{:.2}" stroke-linejoin="round""#,
                        size * 0.035
                    );
                }
                let _ = writeln!(out, r#" d="{path}"/>"#);
            }
            Shape::Stroke { points, width } => {
                let mut path = String::new();
                poly(&mut path, points, Transform::default());
                let [a, b] = transform.x;
                let [c, d] = transform.y;
                let [x, y] = transform.origin;
                let _ = writeln!(
                    out,
                    r#"<path d="{path}" transform="matrix({a:.5} {b:.5} {c:.5} {d:.5} {x:.2} {y:.2})" fill="none" stroke="{color}" stroke-width="{width:.2}" stroke-linecap="round" stroke-linejoin="round"/>"#
                );
            }
            Shape::Fill { points } => {
                let mut path = String::new();
                poly(&mut path, points, transform);
                let _ = writeln!(out, r#"<path d="{path}Z" fill="{color}"/>"#);
            }
        }
    }
    let _ = writeln!(out, "</g>");
}

fn hex([r, g, b]: Color) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn poly(out: &mut String, points: &[Point], transform: Transform) {
    for (i, &p) in points.iter().enumerate() {
        let [x, y] = transform.point(p);
        let command = if i == 0 { 'M' } else { 'L' };
        let _ = write!(out, "{command}{x:.2},{y:.2}");
    }
}

struct SvgPen<'a> {
    path: &'a mut String,
    x: f32,
    italic: bool,
}

impl SvgPen<'_> {
    fn points(&mut self, command: char, points: &[(f32, f32)]) {
        self.path.push(command);
        for (i, &(x, y)) in points.iter().enumerate() {
            let x = self.x + x + if self.italic { 0.20 * y } else { 0.0 };
            let separator = if i == 0 { "" } else { " " };
            let _ = write!(self.path, "{separator}{x:.2},{:.2}", -y);
        }
    }
}

impl OutlinePen for SvgPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.points('M', &[(x, y)]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.points('L', &[(x, y)]);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.points('Q', &[(cx, cy), (x, y)]);
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.points('C', &[(ax, ay), (bx, by), (x, y)]);
    }
    fn close(&mut self) {
        self.path.push('Z');
    }
}
