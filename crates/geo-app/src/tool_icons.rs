//! Icons of the construction tools, drawn with shapes (24 x 24 box).

use crate::icons::Pen;
use crate::theme;
use crate::tools::Tool;
use eframe::egui::{Color32, Painter, Pos2};

pub fn paint(painter: &Painter, tool: Tool, c: Pos2, half: f32, color: Color32, accent: Color32) {
    let pen = Pen {
        painter,
        c,
        u: half / 12.0,
        color,
        width: (half / 12.0 * 1.7).max(1.2),
    };
    let dot = |x: f32, y: f32| pen.dot(x, y, 2.6, accent);
    match tool {
        Tool::Move => {
            let arrow = [
                (-6.0, -10.0),
                (-6.0, 8.0),
                (-1.8, 4.2),
                (1.2, 10.5),
                (4.0, 9.0),
                (1.2, 3.0),
                (6.5, 3.0),
            ];
            pen.fill_poly(&arrow, theme::WHITE);
            pen.poly(&arrow, true);
        }
        Tool::Point => {
            pen.circle(0.0, 0.0, 8.0);
            pen.dot(0.0, 0.0, 3.8, accent);
        }
        Tool::Line => {
            pen.line((-10.0, 8.0), (10.0, -8.0));
            dot(-4.0, 4.0);
            dot(4.0, -4.0);
        }
        Tool::Segment => {
            pen.line((-8.0, 6.0), (8.0, -6.0));
            dot(-8.0, 6.0);
            dot(8.0, -6.0);
        }
        Tool::Ray => {
            pen.line((-8.0, 6.0), (10.0, -7.0));
            pen.poly(&[(4.0, -8.5), (10.0, -7.0), (6.5, -2.0)], false);
            dot(-8.0, 6.0);
            dot(0.0, 2.7);
        }
        Tool::Circle => {
            pen.circle(0.0, 0.0, 9.0);
            dot(0.0, 0.0);
            dot(6.4, -6.4);
        }
        Tool::Midpoint => {
            pen.line((-9.0, 0.0), (9.0, 0.0));
            dot(-9.0, 0.0);
            dot(9.0, 0.0);
            pen.dot(0.0, 0.0, 3.0, theme::DEPENDENT_FILL);
        }
        Tool::Perpendicular => {
            pen.line((-10.0, 5.0), (10.0, 5.0));
            pen.line((0.0, -10.0), (0.0, 10.0));
            pen.poly(&[(-4.0, 5.0), (-4.0, 1.0), (0.0, 1.0)], false);
        }
        Tool::Parallel => {
            pen.line((-10.0, 2.0), (10.0, -6.0));
            pen.line((-10.0, 10.0), (10.0, 2.0));
            dot(-2.0, -1.2);
        }
        Tool::Bisector => {
            pen.line((-9.0, 6.0), (9.0, 6.0));
            pen.line((0.0, -10.0), (0.0, 10.0));
            pen.line((-4.5, 3.5), (-4.5, 8.5));
            pen.line((4.5, 3.5), (4.5, 8.5));
            dot(-9.0, 6.0);
            dot(9.0, 6.0);
        }
        Tool::Intersect => {
            pen.line((-9.0, -9.0), (9.0, 9.0));
            pen.line((-9.0, 9.0), (9.0, -9.0));
            pen.dot(0.0, 0.0, 3.2, theme::DEPENDENT_FILL);
        }
        Tool::Delete => {
            pen.line((-8.0, -5.0), (8.0, -5.0));
            pen.poly(
                &[(-2.5, -5.0), (-2.5, -9.0), (2.5, -9.0), (2.5, -5.0)],
                false,
            );
            pen.poly(
                &[(-5.5, -3.0), (5.5, -3.0), (4.5, 10.0), (-4.5, 10.0)],
                true,
            );
            pen.line((-1.5, 0.0), (-1.5, 7.0));
            pen.line((1.5, 0.0), (1.5, 7.0));
        }
    }
}
