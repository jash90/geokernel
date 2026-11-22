//! View-dependent chart meshes, kept until the chart, view or style changes.

use crate::interact::ViewTransform;
use eframe::egui::epaint::{RectShape, TessellationOptions, Tessellator};
use eframe::egui::{vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use geo_kernel::algo::charts::{ChartData, ChartMark, Pie};
use geo_kernel::kernel::DrawingStyle;
use geo_kernel::{Construction, Id, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MARKS_PER_FRAME: usize = 384;
const SCAN_PER_FRAME: usize = 2 * ChartData::DRAW_LIMIT;

#[derive(Default)]
struct Cache {
    generation: u64,
    entries: HashMap<Id, Entry>,
    candidates: Vec<Projected>,
}

#[derive(PartialEq)]
struct Key {
    revision: u64,
    view: ViewTransform,
    rect: Rect,
    color: Color32,
    thickness: u8,
    pixels_per_point: f32,
    options: TessellationOptions,
    draw_limit: usize,
    scan_limit: usize,
}

struct Entry {
    key: Option<Key>,
    mesh: Arc<Mesh>,
    label: Option<Pos2>,
}

#[cfg(test)]
thread_local! {
    static BUILDS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn builds() -> u64 {
    BUILDS.with(std::cell::Cell::get)
}

pub(super) struct Frame<'a> {
    painter: &'a Painter,
    rect: Rect,
    view: ViewTransform,
    cache: Arc<Mutex<Cache>>,
    remaining: usize,
    scan_remaining: usize,
    pending: usize,
    pixels_per_point: f32,
    options: TessellationOptions,
}

impl<'a> Frame<'a> {
    pub(super) fn new(
        painter: &'a Painter,
        rect: Rect,
        c: &Construction,
        view: &ViewTransform,
        generation: u64,
        visible: &dyn Fn(Id) -> bool,
    ) -> Option<Self> {
        if c.charts().len() == 0 {
            return None;
        }
        let cache = painter.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(eframe::egui::Id::unique(
                "construction chart meshes",
            ))
            .clone()
        });
        {
            let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
            if cache.generation != generation {
                cache.generation = generation;
                cache.entries.clear();
            }
        }
        let pending = c
            .charts()
            .filter(|(id, _)| visible(*id) && c.value(*id).is_defined())
            .count();
        Some(Self {
            painter,
            rect,
            view: *view,
            cache,
            remaining: MARKS_PER_FRAME,
            scan_remaining: SCAN_PER_FRAME,
            pending,
            pixels_per_point: painter.ctx().pixels_per_point(),
            options: painter.ctx().tessellation_options(|o| *o),
        })
    }

    pub(super) fn draw(
        &mut self,
        c: &Construction,
        id: Id,
        chart: &ChartData,
        color: Color32,
    ) -> Option<Pos2> {
        let n = chart
            .pie
            .as_ref()
            .map_or(chart.marks.len(), |pie| pie.fractions.len())
            .min(ChartData::DRAW_LIMIT);
        let draw_limit = n.min(self.remaining.div_ceil(self.pending.max(1)));
        let scan_limit = n.min(self.scan_remaining.div_ceil(self.pending.max(1)));
        self.pending = self.pending.saturating_sub(1);
        self.remaining -= draw_limit;
        self.scan_remaining -= scan_limit;
        if draw_limit == 0 || scan_limit == 0 {
            return None;
        }
        let DrawingStyle {
            line_thickness,
            show_label,
            ..
        } = c.drawing_style(id);
        let key = Key {
            revision: chart.revision(),
            view: self.view,
            rect: self.rect,
            color,
            thickness: line_thickness,
            pixels_per_point: self.pixels_per_point,
            options: self.options,
            draw_limit,
            scan_limit,
        };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let Cache {
            entries,
            candidates,
            ..
        } = &mut *cache;
        let drawing = entries.entry(id).or_insert_with(|| Entry {
            key: None,
            mesh: Arc::new(Mesh::default()),
            label: None,
        });
        if drawing.key.as_ref() != Some(&key) {
            if Arc::get_mut(&mut drawing.mesh).is_none() {
                drawing.mesh = Arc::new(Mesh::default());
            }
            let mesh = Arc::get_mut(&mut drawing.mesh).expect("owned chart mesh");
            mesh.vertices.clear();
            mesh.indices.clear();
            drawing.label = build(&key, chart, candidates, mesh);
            drawing.key = Some(key);
        }
        if !drawing.mesh.is_empty() {
            self.painter.add(Shape::Mesh(drawing.mesh.clone()));
        }
        drawing
            .label
            .filter(|_| show_label && c.value(id) == Value::PieChart)
    }
}

#[derive(Clone, Copy)]
enum Projected {
    Bar(Rect),
    Box(Rect),
    Segment([Pos2; 2]),
    Point(Pos2, bool),
    Cross(Pos2),
}

fn build(
    key: &Key,
    chart: &ChartData,
    candidates: &mut Vec<Projected>,
    mesh: &mut Mesh,
) -> Option<Pos2> {
    #[cfg(test)]
    BUILDS.with(|n| n.set(n.get() + 1));

    let screen = |(x, y)| {
        let (x, y) = key.view.world_to_screen(x, y);
        key.rect.min + vec2(x as f32, y as f32)
    };
    let stroke = Stroke::new(f32::from(key.thickness) * 0.5, key.color);
    let mut tess = Tessellator::new(key.pixels_per_point, key.options, [1, 1], Vec::new());
    tess.set_clip_rect(key.rect);
    if let Some(pie) = &chart.pie {
        let center = screen(pie.center);
        let (x, y) = pie.center;
        let radius = vec2(
            screen((x + pie.radius, y)).x - center.x,
            center.y - screen((x, y + pie.radius)).y,
        );
        pie_mesh(key, &mut tess, mesh, center, radius, pie, stroke)
    } else {
        candidates.clear();
        let margin = key.rect.expand(5.0);
        let width = chart.marks.len().min(ChartData::DRAW_LIMIT);
        // Spread the scan over the whole source when other charts are also visible.
        let stride = if chart.dot_stack.is_some() {
            1
        } else {
            width.div_ceil(key.scan_limit).max(1)
        };
        let radius = if chart.dot_stack.is_some() { 5.0 } else { 4.0 };
        for mark in chart
            .render_marks(key.view.scale())
            .step_by(stride)
            .take(key.scan_limit)
        {
            let rect = key.rect;
            let placed = match mark {
                ChartMark::Bar {
                    left,
                    right,
                    height,
                } if height != 0.0 => {
                    let bounds = Rect::from_two_pos(screen((left, 0.0)), screen((right, height)));
                    (bounds.is_finite() && bounds.intersects(rect))
                        .then_some(Projected::Bar(bounds))
                }
                ChartMark::Box {
                    x: [left, right],
                    y: [bottom, top],
                } => {
                    let bounds = Rect::from_two_pos(screen((left, bottom)), screen((right, top)));
                    (bounds.is_finite() && bounds.intersects(rect))
                        .then_some(Projected::Box(bounds))
                }
                ChartMark::Segment(ends) => {
                    let ends = ends.map(screen);
                    let bounds = Rect::from_points(&ends);
                    (bounds.is_finite() && bounds.intersects(rect))
                        .then_some(Projected::Segment(ends))
                }
                ChartMark::Point { at, open } => {
                    let p = screen(at);
                    margin.contains(p).then_some(Projected::Point(p, open))
                }
                ChartMark::Cross(at) => {
                    let p = screen(at);
                    margin.contains(p).then_some(Projected::Cross(p))
                }
                _ => None,
            };
            if let Some(placed) = placed {
                candidates.push(placed);
            }
        }
        let count = candidates.len().min(key.draw_limit);
        for k in 0..count {
            let mark = candidates[k * candidates.len() / count];
            let shape = match mark {
                Projected::Bar(rect) => Shape::Rect(RectShape::new(
                    rect,
                    0.0,
                    key.color.gamma_multiply(0.24),
                    stroke,
                    StrokeKind::Middle,
                )),
                Projected::Box(rect) => Shape::rect_stroke(rect, 0.0, stroke, StrokeKind::Middle),
                Projected::Segment(ends) => Shape::line_segment(ends, stroke),
                Projected::Point(p, false) => Shape::circle_filled(p, radius, key.color),
                Projected::Point(p, true) => Shape::circle_stroke(p, radius, stroke),
                Projected::Cross(p) => {
                    for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                        tess.tessellate_shape(Shape::line_segment([p - d, p + d], stroke), mesh);
                    }
                    continue;
                }
            };
            tess.tessellate_shape(shape, mesh);
        }
        None
    }
}

fn pie_mesh(
    key: &Key,
    tess: &mut Tessellator,
    mesh: &mut Mesh,
    center: Pos2,
    radius: Vec2,
    pie: &Pie,
    stroke: Stroke,
) -> Option<Pos2> {
    if !center.is_finite()
        || !radius.is_finite()
        || radius.min_elem() <= 0.0
        || !Rect::from_center_size(center, radius * 2.0).intersects(key.rect)
    {
        return None;
    }
    let at = |turn: f64| {
        let (sin, cos) = (std::f64::consts::TAU * turn).sin_cos();
        center + vec2(cos as f32 * radius.x, sin as f32 * radius.y)
    };
    let count = key.scan_limit.min(pie.fractions.len());
    let step = count.div_ceil(key.draw_limit).max(1);
    let mut turn = -0.25;
    for (group, fractions) in pie.fractions[..count].chunks(step).enumerate() {
        let fraction = fractions.iter().copied().sum::<f64>();
        if fraction > 0.0 {
            let [r, g, b] = Pie::slice_color(group * step);
            let color = Color32::from_rgb(r, g, b).gamma_multiply(f32::from(key.color.a()) / 255.0);
            let first = mesh.vertices.len() as u32;
            let steps = (fraction * f64::from(radius.max_elem()).sqrt() * 5.0)
                .ceil()
                .clamp(1.0, 256.0) as u32;
            mesh.colored_vertex(center, color);
            for step in 0..=steps {
                mesh.colored_vertex(
                    at(turn + fraction * f64::from(step) / f64::from(steps)),
                    color,
                );
                if step != 0 {
                    mesh.add_triangle(first, first + step, first + step + 1);
                }
            }
        }
        turn += fraction;
    }
    if stroke.width > 0.0 {
        let mut turn = -0.25;
        for fractions in pie.fractions[..count].chunks(step) {
            turn += fractions.iter().copied().sum::<f64>();
            if turn.is_finite() {
                tess.tessellate_shape(Shape::line_segment([center, at(turn)], stroke), mesh);
            }
        }
        tess.tessellate_shape(Shape::ellipse_stroke(center, radius, stroke), mesh);
    }
    Some(center + radius * std::f32::consts::FRAC_1_SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headless, view::Camera};
    use eframe::egui;

    #[test]
    fn pie_uses_both_screen_radii_and_draws_its_rim_and_rays() {
        let c = geo_kernel::script::load("p = PieChart({1, 1, 1, 1})").unwrap();
        let pie = c.chart(c.id("p").unwrap()).unwrap().pie.as_ref().unwrap();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
        let center = rect.center();
        let radius = vec2(90.0, 30.0);
        let key = Key {
            revision: 0,
            view: Camera::STANDARD.transform(400.0, 300.0),
            rect,
            color: Color32::BLACK,
            thickness: 3,
            pixels_per_point: 1.0,
            options: TessellationOptions::default(),
            draw_limit: 4,
            scan_limit: 4,
        };
        let mut tess = Tessellator::new(1.0, key.options, [1, 1], Vec::new());
        let mut fill = Mesh::default();
        let label = pie_mesh(
            &key,
            &mut tess,
            &mut fill,
            center,
            radius,
            pie,
            Stroke::NONE,
        );
        assert_eq!(fill.calc_bounds().size(), radius * 2.0);
        assert_eq!(
            label,
            Some(center + radius * std::f32::consts::FRAC_1_SQRT_2)
        );
        let mut outlined = Mesh::default();
        pie_mesh(
            &key,
            &mut tess,
            &mut outlined,
            center,
            radius,
            pie,
            Stroke::new(1.5, Color32::BLACK),
        );
        assert!(outlined.indices.len() > fill.indices.len());
        assert!(outlined.calc_bounds().contains_rect(fill.calc_bounds()));
    }

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
                let mut frame = Frame::new(&painter, canvas, c, &view, 0, &|_| true).unwrap();
                let id = c.id("chart").unwrap();
                frame.draw(c, id, c.chart(id).unwrap(), Color32::GREEN);
            });
            out.textures_delta.clear();
            out.shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let before = frame(&c);
        let built = builds();
        assert_eq!(before, frame(&c));
        assert_eq!(builds(), built);
        c.set_number(c.id("scale").unwrap(), 3.0).unwrap();
        let after = frame(&c);
        assert_eq!(builds(), built + 1);
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), before.len());
        for (before, after) in before.into_iter().zip(after) {
            assert_eq!(before.width(), after.width());
            assert!(before.height() < after.height());
        }
    }
}
