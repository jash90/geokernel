//! Draws the numeric plots held by the construction.

use crate::interact::ViewTransform;
use eframe::egui::{vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use geo_kernel::algo::charts::{ChartMark, Pie};
use geo_kernel::{Construction, Id, Value};

pub(super) fn paint(
    painter: &Painter,
    rect: Rect,
    c: &Construction,
    view: &ViewTransform,
    visible: &dyn Fn(Id) -> bool,
    color: impl Fn(Id) -> Color32,
) {
    let screen = |(x, y)| {
        let (x, y) = view.world_to_screen(x, y);
        rect.min + vec2(x as f32, y as f32)
    };
    for (id, chart) in c.charts() {
        if !visible(id) || !c.value(id).is_defined() {
            continue;
        }
        if c.value(id) == Value::PieChart {
            if let Some(pie) = &chart.pie {
                paint_pie(painter, rect, view, screen(pie.center), pie);
            }
            continue;
        }
        let color = color(id);
        let stroke = Stroke::new(1.5, color);
        if let Some(stack) = &chart.dot_stack {
            for at in stack.dots(view.scale()) {
                let p = screen(at);
                if rect.expand(5.0).contains(p) {
                    painter.circle_filled(p, 5.0, color);
                }
            }
            continue;
        }
        for &mark in &chart.marks {
            match mark {
                ChartMark::Bar {
                    left,
                    right,
                    height,
                } => {
                    let bar = Rect::from_two_pos(screen((left, 0.0)), screen((right, height)));
                    if bar.is_finite() && bar.intersects(rect) && height != 0.0 {
                        painter.rect(
                            bar,
                            0.0,
                            color.gamma_multiply(0.24),
                            stroke,
                            StrokeKind::Middle,
                        );
                    }
                }
                ChartMark::Box {
                    x: [left, right],
                    y: [bottom, top],
                } => {
                    let bounds = Rect::from_two_pos(screen((left, bottom)), screen((right, top)));
                    if bounds.is_finite() && bounds.intersects(rect) {
                        painter.rect_stroke(bounds, 0.0, stroke, StrokeKind::Middle);
                    }
                }
                ChartMark::Segment(ends) => {
                    let ends = ends.map(screen);
                    let bounds = Rect::from_points(&ends);
                    if bounds.is_finite() && bounds.intersects(rect) {
                        painter.line_segment(ends, stroke);
                    }
                }
                ChartMark::Point { at, open } => {
                    let p = screen(at);
                    if rect.expand(5.0).contains(p) {
                        if open {
                            painter.circle_stroke(p, 4.0, stroke);
                        } else {
                            painter.circle_filled(p, 4.0, color);
                        }
                    }
                }
                ChartMark::Cross(at) => {
                    let p = screen(at);
                    if rect.expand(4.0).contains(p) {
                        for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                            painter.line_segment([p - d, p + d], stroke);
                        }
                    }
                }
            }
        }
    }
}

fn paint_pie(painter: &Painter, rect: Rect, view: &ViewTransform, center: Pos2, pie: &Pie) {
    let radius = (view.scale() * pie.radius) as f32;
    if !radius.is_finite()
        || radius <= 0.0
        || !Rect::from_center_size(center, vec2(2.0 * radius, 2.0 * radius)).intersects(rect)
    {
        return;
    }
    let colors = [
        Color32::from_rgb(101, 87, 210),
        Color32::from_rgb(224, 191, 0),
        Color32::from_rgb(59, 180, 166),
        Color32::from_rgb(218, 106, 157),
        Color32::from_rgb(59, 28, 50),
        Color32::from_rgb(255, 140, 112),
    ];
    let mut turn = -0.25;
    let mut mesh = Mesh::default();
    for (i, &fraction) in pie.fractions.iter().enumerate() {
        if fraction <= 0.0 || !fraction.is_finite() {
            continue;
        }
        let color = Color32::WHITE.lerp_to_gamma(
            colors[i % colors.len()],
            0.6f32.powi((i / colors.len()) as i32),
        );
        let start = mesh.vertices.len() as u32;
        let steps = (fraction * f64::from(radius).sqrt() * 5.0)
            .ceil()
            .clamp(1.0, 256.0) as u32;
        mesh.colored_vertex(center, color);
        for k in 0..=steps {
            let angle = std::f64::consts::TAU * (turn + fraction * f64::from(k) / f64::from(steps));
            let (sin, cos) = angle.sin_cos();
            mesh.colored_vertex(center + vec2(cos as f32, sin as f32) * radius, color);
            if k != 0 {
                mesh.add_triangle(start, start + k, start + k + 1);
            }
        }
        turn += fraction;
    }
    if !mesh.is_empty() {
        painter.add(Shape::mesh(mesh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headless, view::Camera};
    use eframe::egui;

    #[test]
    fn histogram_meshes_follow_a_density_change() {
        let mut c = geo_kernel::script::load(
            "scale = 2\nchart = Histogram({0, 2, 5}, {0, 1, 2, 5}, true, scale)",
        )
        .unwrap();
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let view = Camera::STANDARD.transform(800.0, 600.0);
        let ctx = egui::Context::default();
        let frame = |c: &Construction| {
            let input = headless::raw_input(canvas, 0.0, 1.0 / 60.0, Vec::new());
            let mut out = ctx.run_ui(input, |ui| {
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                paint(&painter, canvas, c, &view, &|_| true, |_| Color32::GREEN);
            });
            out.textures_delta.clear();
            out.shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Rect(rect) => Some(rect.rect),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let before = frame(&c);
        c.set_number(c.id("scale").unwrap(), 3.0).unwrap();
        let after = frame(&c);
        assert_eq!(before.len(), 2);
        assert_eq!(after.len(), before.len());
        for (before, after) in before.into_iter().zip(after) {
            assert_eq!(before.width(), after.width());
            assert!(before.height() < after.height());
        }
    }
}
