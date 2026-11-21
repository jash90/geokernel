//! Icons of the tools, drawn with shapes in a 24 x 24 box: dark strokes,
//! blue given points and red for what the tool creates.

use crate::icons::Pen;
use crate::theme;
use crate::tools::Tool;
use eframe::egui::{Color32, Painter, Pos2};

const INK: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
const POINT: Color32 = theme::FREE_POINT;
const RESULT: Color32 = Color32::from_rgb(0xe0, 0x30, 0x30);

pub fn paint(painter: &Painter, tool: Tool, c: Pos2, half: f32) {
    let u = half / 12.0;
    let ink = Pen {
        painter,
        c,
        u,
        color: INK,
        width: (u * 1.5).max(1.0),
    };
    let red = Pen {
        color: RESULT,
        ..ink
    };
    let dot = |x: f32, y: f32| ink.dot(x, y, 2.4, POINT);
    let new_dot = |x: f32, y: f32| ink.dot(x, y, 2.4, RESULT);
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
            ink.fill_poly(&arrow, theme::WHITE);
            ink.poly(&arrow, true);
        }
        Tool::Point => {
            dot(-3.0, 3.0);
            red.line((2.0, -10.0), (2.0, -4.0));
            red.line((-1.0, -7.0), (5.0, -7.0));
        }
        Tool::Intersect => {
            ink.line((-10.0, -8.0), (10.0, 8.0));
            ink.line((-10.0, 8.0), (10.0, -8.0));
            new_dot(0.0, 0.0);
        }
        Tool::Midpoint => {
            ink.line((-10.0, 6.0), (10.0, -6.0));
            dot(-10.0, 6.0);
            dot(10.0, -6.0);
            new_dot(0.0, 0.0);
        }
        Tool::Line => {
            ink.line((-11.0, 9.0), (11.0, -9.0));
            dot(-5.0, 4.1);
            dot(5.0, -4.1);
        }
        Tool::Segment => {
            ink.line((-8.0, 7.0), (8.0, -7.0));
            dot(-8.0, 7.0);
            dot(8.0, -7.0);
        }
        Tool::Ray => {
            ink.line((-8.0, 7.0), (11.0, -9.0));
            dot(-8.0, 7.0);
            dot(1.5, -1.0);
        }
        Tool::Vector | Tool::VectorFromPoint => {
            ink.line((-9.0, 7.0), (8.0, -6.0));
            ink.fill_poly(&[(10.0, -8.0), (3.0, -6.5), (7.0, -1.5)], INK);
            dot(-9.0, 7.0);
            if tool == Tool::VectorFromPoint {
                red.line((-7.0, 1.0), (1.0, -5.0));
            }
        }
        Tool::Perpendicular => {
            ink.line((-11.0, 7.0), (11.0, 7.0));
            red.line((0.0, -11.0), (0.0, 11.0));
            dot(0.0, -5.0);
        }
        Tool::Parallel => {
            ink.line((-11.0, 9.0), (11.0, 1.0));
            red.line((-11.0, -1.0), (11.0, -9.0));
            dot(0.0, -5.0);
        }
        Tool::Bisector => {
            ink.line((-9.0, 6.0), (9.0, 6.0));
            dot(-9.0, 6.0);
            dot(9.0, 6.0);
            red.line((0.0, -11.0), (0.0, 11.0));
        }
        Tool::AngleBisector => {
            ink.line((-10.0, 9.0), (10.0, 9.0));
            ink.line((-10.0, 9.0), (6.0, -9.0));
            red.line((-10.0, 9.0), (11.0, -0.5));
            dot(-10.0, 9.0);
        }
        Tool::Tangents => {
            ink.circle(-2.0, 2.0, 7.0);
            red.line((-11.0, -6.6), (11.0, -6.6));
            red.line((-9.0, 9.0), (11.0, -6.6));
            dot(11.0, -6.6);
        }
        Tool::Polar => {
            ink.circle(-3.0, 0.0, 7.5);
            red.line((2.0, -11.0), (2.0, 11.0));
            dot(9.0, 0.0);
        }
        Tool::Circle => {
            ink.circle(0.0, 0.0, 9.0);
            dot(0.0, 0.0);
            dot(6.4, -6.4);
        }
        Tool::CircleRadius => {
            ink.circle(0.0, 0.0, 9.0);
            red.line((0.0, 0.0), (9.0, 0.0));
            dot(0.0, 0.0);
        }
        Tool::Circle3 => {
            ink.circle(0.0, 0.0, 9.0);
            dot(-9.0, 0.0);
            dot(6.4, -6.4);
            dot(4.5, 7.8);
        }
        Tool::Polygon | Tool::VectorPolygon => {
            let pts = [(-10.0, 6.0), (-3.0, -9.0), (10.0, -3.0), (5.0, 9.0)];
            painter.add(eframe::egui::Shape::convex_polygon(
                pts.iter()
                    .map(|&(x, y)| c + eframe::egui::vec2(x, y) * u)
                    .collect(),
                crate::theme::POLYGON.gamma_multiply(0.15),
                eframe::egui::Stroke::NONE,
            ));
            red.poly(&pts, true);
            for (x, y) in pts {
                dot(x, y);
            }
            if tool == Tool::VectorPolygon {
                ink.line((-5.0, 4.0), (4.0, -3.0));
                ink.fill_poly(&[(5.0, -4.0), (0.0, -2.0), (3.0, 1.0)], INK);
            }
        }
        Tool::RegularPolygon => {
            let pts: Vec<(f32, f32)> = (0..5)
                .map(|i| {
                    let a = (i as f32 * 72.0 - 90.0).to_radians();
                    (10.0 * a.cos(), 10.0 * a.sin())
                })
                .collect();
            red.poly(&pts, true);
            dot(pts[3].0, pts[3].1);
            dot(pts[2].0, pts[2].1);
        }
        Tool::Semicircle => {
            red.poly(&Pen::arc(0.0, 4.0, 9.0, 0.0, 180.0), false);
            dot(-9.0, 4.0);
            dot(9.0, 4.0);
        }
        Tool::CircularArc
        | Tool::CircumcircularArc
        | Tool::CircularSector
        | Tool::CircumcircularSector => {
            let arc = Pen::arc(0.0, 3.0, 9.0, 0.0, 145.0);
            red.poly(&arc, false);
            let end = arc[arc.len() - 1];
            if matches!(tool, Tool::CircularSector | Tool::CircumcircularSector) {
                red.poly(&[arc[0], (0.0, 3.0), end], false);
            }
            let middle = if matches!(tool, Tool::CircumcircularArc | Tool::CircumcircularSector) {
                arc[arc.len() / 2]
            } else {
                (0.0, 3.0)
            };
            for (x, y) in [arc[0], middle, end] {
                dot(x, y);
            }
        }
        Tool::Compasses => {
            ink.poly(&[(-9.0, 9.0), (0.0, -9.0), (9.0, 9.0)], false);
            dot(0.0, -9.0);
            red.poly(&Pen::arc(-8.0, 9.0, 17.0, 0.0, 27.0), false);
        }
        Tool::Ellipse | Tool::Conic5 => {
            let rim: Vec<_> = Pen::arc(0.0, 0.0, 10.0, 0.0, 360.0)
                .into_iter()
                .map(|(x, y)| (x, y * 0.65))
                .collect();
            red.poly(&rim, true);
            if tool == Tool::Ellipse {
                dot(-6.0, 0.0);
                dot(6.0, 0.0);
                dot(0.0, -6.5);
            } else {
                for i in [1, 4, 7, 10, 13] {
                    dot(rim[i].0, rim[i].1);
                }
            }
        }
        Tool::Hyperbola => {
            for sign in [-1.0, 1.0] {
                red.poly(
                    &[
                        (10.0 * sign, -9.0),
                        (5.0 * sign, -4.0),
                        (3.0 * sign, 0.0),
                        (5.0 * sign, 4.0),
                        (10.0 * sign, 9.0),
                    ],
                    false,
                );
                dot(6.0 * sign, 0.0);
            }
            dot(5.0, -4.0);
        }
        Tool::Parabola => {
            ink.line((-11.0, 8.0), (11.0, 8.0));
            red.poly(
                &[
                    (-9.0, -9.0),
                    (-6.0, 0.0),
                    (-3.0, 4.0),
                    (0.0, 5.0),
                    (3.0, 4.0),
                    (6.0, 0.0),
                    (9.0, -9.0),
                ],
                false,
            );
            dot(0.0, 1.0);
        }
        Tool::Polyline => {
            let pts = [(-10.0, 5.0), (-4.0, -6.0), (3.0, 4.0), (10.0, -5.0)];
            red.poly(&pts, false);
            for (x, y) in pts {
                dot(x, y);
            }
        }
        Tool::Area => {
            let outline = [(-10.0, 8.0), (-6.0, -8.0), (9.0, -4.0), (8.0, 8.0)];
            ink.fill_poly(&outline, RESULT.gamma_multiply(0.25));
            red.poly(&outline, true);
            for x in [-4.0, 0.0, 4.0] {
                ink.line((x - 2.0, 5.0), (x + 2.0, -2.0));
            }
        }
        Tool::Slope => {
            ink.line((-11.0, 9.0), (11.0, -9.0));
            red.poly(&[(-7.0, 5.7), (6.0, 5.7), (6.0, -4.9)], false);
        }
        Tool::Angle => {
            ink.line((-10.0, 8.0), (10.0, 8.0));
            ink.line((-10.0, 8.0), (4.0, -9.0));
            red.poly(&Pen::arc(-10.0, 8.0, 9.0, 0.0, 50.0), false);
            dot(-10.0, 8.0);
        }
        Tool::Distance => {
            ink.line((-10.0, 4.0), (10.0, 4.0));
            ink.line((-10.0, 0.0), (-10.0, 8.0));
            ink.line((10.0, 0.0), (10.0, 8.0));
            red.line((-6.0, -6.0), (6.0, -6.0));
            red.line((-6.0, -9.0), (-6.0, -3.0));
            red.line((6.0, -9.0), (6.0, -3.0));
        }
        Tool::ReflectLine => {
            ink.line((0.0, -11.0), (0.0, 11.0));
            ink.fill_poly(&[(-10.0, 6.0), (-3.0, 6.0), (-3.0, -6.0)], POINT);
            red.poly(&[(10.0, 6.0), (3.0, 6.0), (3.0, -6.0)], true);
        }
        Tool::ReflectPoint => {
            ink.fill_poly(&[(-11.0, -2.0), (-4.0, -2.0), (-4.0, -9.0)], POINT);
            red.poly(&[(11.0, 2.0), (4.0, 2.0), (4.0, 9.0)], true);
            ink.dot(0.0, 0.0, 2.4, INK);
        }
        Tool::ReflectCircle => {
            ink.circle(0.0, 0.0, 8.0);
            dot(3.0, -1.5);
            new_dot(10.5, -5.0);
            ink.dot(0.0, 0.0, 1.6, INK);
        }
        Tool::Rotate => {
            ink.fill_poly(&[(2.0, 9.0), (10.0, 9.0), (10.0, 2.0)], POINT);
            red.poly(&Pen::arc(0.0, 0.0, 9.0, 10.0, 150.0), false);
            red.fill_poly(&[(-9.5, -7.0), (-4.5, -9.0), (-9.0, -2.0)], RESULT);
            ink.dot(0.0, 0.0, 2.2, INK);
        }
        Tool::Translate => {
            ink.fill_poly(&[(-11.0, 10.0), (-4.0, 10.0), (-4.0, 3.0)], POINT);
            red.poly(&[(3.0, -3.0), (10.0, -3.0), (10.0, -10.0)], true);
            ink.line((-8.0, 1.0), (1.0, -7.0));
            ink.fill_poly(&[(3.0, -9.0), (-2.5, -7.5), (1.0, -4.0)], INK);
        }
        Tool::Dilate => {
            ink.fill_poly(&[(-4.0, 3.0), (1.0, 3.0), (1.0, -2.0)], POINT);
            red.poly(&[(-4.0, 3.0), (11.0, 3.0), (11.0, -11.0)], true);
            ink.dot(-10.0, 8.0, 2.2, INK);
        }
        Tool::MoveView => {
            for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                ink.line((0.0, 0.0), (dx * 10.0, dy * 10.0));
                let (tx, ty) = (dx * 11.0, dy * 11.0);
                let (nx, ny) = (-dy * 3.5, dx * 3.5);
                ink.fill_poly(
                    &[
                        (tx, ty),
                        (tx - dx * 5.0 + nx, ty - dy * 5.0 + ny),
                        (tx - dx * 5.0 - nx, ty - dy * 5.0 - ny),
                    ],
                    INK,
                );
            }
        }
        Tool::ZoomIn | Tool::ZoomOut => {
            ink.circle(-2.0, -2.0, 7.5);
            ink.line((3.5, 3.5), (10.5, 10.5));
            ink.line((-6.0, -2.0), (2.0, -2.0));
            if tool == Tool::ZoomIn {
                ink.line((-2.0, -6.0), (-2.0, 2.0));
            }
        }
        Tool::ShowHide => {
            let top = Pen::arc(0.0, 6.0, 12.0, 30.0, 150.0);
            let bottom = Pen::arc(0.0, -6.0, 12.0, 210.0, 330.0);
            ink.poly(&top, false);
            ink.poly(&bottom, false);
            ink.dot(0.0, 0.0, 3.2, POINT);
            red.line((-10.0, 9.0), (10.0, -9.0));
        }
        Tool::Delete => {
            ink.fill_poly(
                &[(-9.0, 2.0), (1.0, -8.0), (9.0, 0.0), (-1.0, 10.0)],
                theme::WHITE,
            );
            ink.poly(&[(-9.0, 2.0), (1.0, -8.0), (9.0, 0.0), (-1.0, 10.0)], true);
            red.fill_poly(
                &[(-9.0, 2.0), (-4.0, -3.0), (4.0, 5.0), (-1.0, 10.0)],
                RESULT,
            );
        }
    }
}
