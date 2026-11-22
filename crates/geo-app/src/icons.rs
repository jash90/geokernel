//! Glyphs drawn with `egui::Painter` shapes. Every icon fits a 24 x 24 box
//! (scaled by `half / 12`) around its centre.

use eframe::egui::{epaint::PathShape, vec2, Color32, Painter, Pos2, Shape, Stroke};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Menu,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    /// Show all objects.
    Fit,
    Fullscreen,
    /// The toggle of the graphics style bar.
    StyleBar,
    /// The standard view.
    Home,
    Axes,
    Grid,
    Plus,
    Kebab,
    Backspace,
    Left,
    Right,
    Enter,
    Shift,
    /// Play the animation.
    Play,
    /// Pause the animation.
    Pause,
}

/// A drawing context: centre, unit (pixels per icon-box unit) and stroke.
pub(crate) struct Pen<'a> {
    pub painter: &'a Painter,
    pub c: Pos2,
    pub u: f32,
    pub color: Color32,
    pub width: f32,
}

impl Pen<'_> {
    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        self.c + vec2(x, y) * self.u
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(self.width, self.color)
    }

    pub fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.painter
            .line_segment([self.p(a.0, a.1), self.p(b.0, b.1)], self.stroke());
    }

    pub fn poly(&self, pts: &[(f32, f32)], closed: bool) {
        let points = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::Path(PathShape {
            points,
            closed,
            fill: Color32::TRANSPARENT,
            stroke: self.stroke().into(),
        }));
    }

    pub fn fill_poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    pub fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.p(x, y), r * self.u, self.stroke());
    }

    pub fn dot(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.painter.circle_filled(self.p(x, y), r * self.u, fill);
    }

    /// Points of an arc around `(cx, cy)`, angles in degrees (y axis up).
    pub fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
        let n = 16;
        (0..=n)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / n as f32) * PI / 180.0;
                (cx + r * a.cos(), cy - r * a.sin())
            })
            .collect()
    }
}

pub fn paint(painter: &Painter, icon: Icon, c: Pos2, half: f32, color: Color32) {
    let pen = Pen {
        painter,
        c,
        u: half / 12.0,
        color,
        width: (half / 12.0 * 1.8).max(1.2),
    };
    match icon {
        Icon::Menu => {
            for y in [-6.0, 0.0, 6.0] {
                pen.line((-9.0, y), (9.0, y));
            }
        }
        Icon::Undo | Icon::Redo => {
            let s = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let arc: Vec<(f32, f32)> = Pen::arc(0.0, 2.0, 8.0, 15.0, 180.0)
                .into_iter()
                .map(|(x, y)| (x * s, y))
                .collect();
            pen.poly(&arc, false);
            pen.fill_poly(&[(-8.0 * s, 8.5), (-12.5 * s, 1.5), (-3.5 * s, 1.5)], color);
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            pen.circle(-2.0, -2.0, 7.0);
            pen.line((3.0, 3.0), (10.0, 10.0));
            pen.line((-5.5, -2.0), (1.5, -2.0));
            if icon == Icon::ZoomIn {
                pen.line((-2.0, -5.5), (-2.0, 1.5));
            }
        }
        Icon::Fullscreen => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                pen.poly(
                    &[
                        (sx * 4.0, sy * 9.0),
                        (sx * 9.0, sy * 9.0),
                        (sx * 9.0, sy * 4.0),
                    ],
                    false,
                );
            }
        }
        Icon::Fit => {
            pen.poly(&[(-9.0, -7.0), (9.0, -7.0), (9.0, 7.0), (-9.0, 7.0)], true);
            pen.dot(-3.0, 1.0, 2.2, color);
            pen.line((-3.0, 1.0), (4.0, -3.0));
            pen.dot(4.0, -3.0, 2.2, color);
        }
        Icon::StyleBar => {
            for (y, w) in [(-6.0, 12.0), (0.0, 7.0), (6.0, 12.0)] {
                pen.line((-10.0, y), (-10.0 + w, y));
            }
            pen.fill_poly(&[(3.0, -1.0), (11.0, -1.0), (7.0, 4.0)], color);
        }
        Icon::Home => {
            pen.poly(&[(-9.0, -1.0), (0.0, -9.0), (9.0, -1.0)], false);
            pen.poly(&[(-6.0, -3.0), (-6.0, 9.0), (6.0, 9.0), (6.0, -3.0)], false);
        }
        Icon::Axes => {
            pen.line((-10.0, 4.0), (10.0, 4.0));
            pen.line((-4.0, 10.0), (-4.0, -10.0));
            pen.fill_poly(&[(11.0, 4.0), (7.0, 1.5), (7.0, 6.5)], color);
            pen.fill_poly(&[(-4.0, -11.0), (-6.5, -7.0), (-1.5, -7.0)], color);
        }
        Icon::Grid => {
            for v in [-9.0, -3.0, 3.0, 9.0] {
                pen.line((v, -9.0), (v, 9.0));
                pen.line((-9.0, v), (9.0, v));
            }
        }
        Icon::Backspace => {
            pen.poly(
                &[
                    (-11.0, 0.0),
                    (-5.0, -7.0),
                    (10.0, -7.0),
                    (10.0, 7.0),
                    (-5.0, 7.0),
                ],
                true,
            );
            pen.line((-1.0, -3.5), (6.0, 3.5));
            pen.line((-1.0, 3.5), (6.0, -3.5));
        }
        Icon::Left => pen.poly(&[(3.0, -7.0), (-4.0, 0.0), (3.0, 7.0)], false),
        Icon::Right => pen.poly(&[(-3.0, -7.0), (4.0, 0.0), (-3.0, 7.0)], false),
        Icon::Enter => {
            pen.poly(&[(9.0, -7.0), (9.0, 3.0), (-8.0, 3.0)], false);
            pen.poly(&[(-3.0, -2.0), (-8.0, 3.0), (-3.0, 8.0)], false);
        }
        Icon::Shift => pen.poly(
            &[
                (0.0, -9.0),
                (9.0, 0.0),
                (4.0, 0.0),
                (4.0, 8.0),
                (-4.0, 8.0),
                (-4.0, 0.0),
                (-9.0, 0.0),
            ],
            true,
        ),
        Icon::Plus => {
            pen.line((-6.0, 0.0), (6.0, 0.0));
            pen.line((0.0, -6.0), (0.0, 6.0));
        }
        Icon::Kebab => {
            for y in [-6.0, 0.0, 6.0] {
                pen.dot(0.0, y, 1.8, color);
            }
        }
        Icon::Play => pen.poly(&[(-4.0, -7.0), (7.0, 0.0), (-4.0, 7.0)], true),
        Icon::Pause => {
            pen.line((-3.5, -6.5), (-3.5, 6.5));
            pen.line((3.5, -6.5), (3.5, 6.5));
        }
    }
}
