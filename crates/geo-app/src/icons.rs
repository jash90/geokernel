//! Glyphs drawn with `egui::Painter` shapes. Every icon fits a 24 x 24 box
//! (scaled by `half / 12`) around its centre.

use eframe::egui::{epaint::PathShape, vec2, Color32, Painter, Pos2, Shape, Stroke};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Menu,
    Algebra,
    Tools,
    Undo,
    Redo,
    Gear,
    ZoomIn,
    ZoomOut,
    Home,
    Plus,
    Kebab,
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
        Icon::Algebra => {
            pen.poly(
                &[(-8.0, -10.0), (8.0, -10.0), (8.0, 10.0), (-8.0, 10.0)],
                true,
            );
            pen.fill_poly(
                &[(-5.0, -7.0), (5.0, -7.0), (5.0, -3.0), (-5.0, -3.0)],
                color,
            );
            for y in [1.0, 5.5] {
                for x in [-4.0, 0.0, 4.0] {
                    pen.dot(x, y, 1.2, color);
                }
            }
        }
        Icon::Tools => {
            pen.poly(&[(-10.0, 9.0), (2.0, 9.0), (-4.0, -4.0)], true);
            pen.circle(4.0, 3.0, 6.5);
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
        Icon::Gear => {
            let mut pts = Vec::new();
            for k in 0..8 {
                let base = k as f32 * 45.0;
                for (da, r) in [(-13.0, 7.5), (-8.0, 10.5), (8.0, 10.5), (13.0, 7.5)] {
                    let a = (base + da) * PI / 180.0;
                    pts.push((r * a.cos(), r * a.sin()));
                }
            }
            pen.poly(&pts, true);
            pen.circle(0.0, 0.0, 3.5);
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            pen.circle(-2.0, -2.0, 7.0);
            pen.line((3.0, 3.0), (10.0, 10.0));
            pen.line((-5.5, -2.0), (1.5, -2.0));
            if icon == Icon::ZoomIn {
                pen.line((-2.0, -5.5), (-2.0, 1.5));
            }
        }
        Icon::Home => {
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
        Icon::Plus => {
            pen.line((-6.0, 0.0), (6.0, 0.0));
            pen.line((0.0, -6.0), (0.0, 6.0));
        }
        Icon::Kebab => {
            for y in [-6.0, 0.0, 6.0] {
                pen.dot(0.0, y, 1.8, color);
            }
        }
    }
}
